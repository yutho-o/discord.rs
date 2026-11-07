//! `Client`, its builder and its runner.

pub mod builder;
pub mod options;
pub mod runner;
pub mod stats;

pub use builder::ClientBuilder;
pub use options::{ClientOptions, DispatchMode, QueueLimits};
pub use runner::Runner;
pub use stats::ClientStats;

use crate::Bound;
use crate::events::HandlerTable;
use crate::pipeline::budget::Budget;
use crate::pipeline::queue::ShardQueue;
use crate::pipeline::ready::ReadyTracker;
use crate::pipeline::shutdown::{self, Lifecycle};
use dr_cache::Cache;
use dr_gateway::{ShardsHandle, ShutdownMode};
use dr_model::ApplicationId;
use dr_rest::Rest;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio_util::task::TaskTracker;

pub(crate) struct ClientInner {
    pub(crate) rest: Rest,
    pub(crate) cache: Cache,
    pub(crate) handlers: HandlerTable,
    pub(crate) budget: Budget,
    pub(crate) tasks: TaskTracker,
    pub(crate) gateway: OnceLock<ShardsHandle>,
    pub(crate) application_id: OnceLock<ApplicationId>,
    pub(crate) ready: ReadyTracker,
    pub(crate) stats: ClientStats,
    pub(crate) options: ClientOptions,
    pub(crate) queue: OnceLock<Arc<ShardQueue>>,
    pub(crate) lifecycle: Lifecycle,
}

/// The client: REST, cache and gateway together. Not generic, and cheap to clone. Your state lives in
/// each handler's [`crate::Context`], never here.
#[derive(Clone)]
pub struct Client(pub(crate) Arc<ClientInner>);

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl Client {
    /// Starts building a client with this bot token.
    pub fn builder(token: impl Into<String>) -> ClientBuilder<()> {
        ClientBuilder::new(token.into())
    }

    /// The REST client, for endpoints the facade does not wrap.
    pub fn rest(&self) -> &Rest {
        &self.0.rest
    }

    /// The cache.
    pub fn cache(&self) -> &Cache {
        &self.0.cache
    }

    /// Counters for this client.
    pub fn stats(&self) -> &ClientStats {
        &self.0.stats
    }

    /// The application id, known after `READY`.
    pub fn application_id(&self) -> Option<ApplicationId> {
        self.0.application_id.get().copied()
    }

    /// The gateway round trip of a shard, `None` before the first ack or with no gateway running.
    pub fn shard_latency(&self, shard: u32) -> Option<Duration> {
        self.0.gateway.get().and_then(|g| g.latency(shard))
    }

    /// Binds a value to this client. Handles and tests use this.
    pub fn bind<T>(&self, data: T) -> Bound<T> {
        Bound::new(self.clone(), Arc::new(data))
    }

    /// Binds a shared value to this client.
    pub fn bind_arc<T>(&self, data: Arc<T>) -> Bound<T> {
        Bound::new(self.clone(), data)
    }

    /// Stops the client: stop reading, drain the queue up to `shutdown_drain`, wait for in-flight
    /// handlers and `ctx.spawn` tasks, abort the rest, close the sockets. Idempotent, and it makes
    /// `Runner::run` return.
    pub async fn shutdown(&self, mode: ShutdownMode) {
        shutdown::run(self, mode).await;
    }

    /// Sets the application id. `READY` does this, and tests that deliver no `READY` can too. The
    /// first value wins.
    #[doc(hidden)]
    pub fn set_application_id(&self, id: ApplicationId) {
        // A second READY carries the same id, so an error here means nothing.
        let _ = self.0.application_id.set(id);
    }

    /// The settings the client was built with.
    #[doc(hidden)]
    pub fn options(&self) -> &ClientOptions {
        &self.0.options
    }
}
