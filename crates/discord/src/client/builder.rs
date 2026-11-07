//! `ClientBuilder`.

use crate::client::ClientInner;
use crate::client::options::{ClientOptions, DispatchMode, QueueLimits};
use crate::error::{ConfigError, HandlerFailure, HandlerResult};
use crate::events::{Erased, GatewayEvent, HandlerTable, RawEvent};
use crate::interactions::{AutocompleteInteraction, CommandInteraction, ComponentInteraction, ModalInteraction};
use crate::pipeline::budget::Budget;
use crate::pipeline::ready::ReadyTracker;
use crate::pipeline::shutdown::Lifecycle;
use crate::{Bound, Client, Context, Error, Runner};
use dr_cache::{Cache, CacheConfig};
use dr_core::{Endpoints, Secret};
use dr_gateway::{Compression, SessionStore};
use dr_model::{Intents, UpdatePresence};
use dr_rest::{Rest, Transport};
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio_util::task::TaskTracker;

/// Builds a [`Client`]. `S` is your state, which each handler closure captures: nothing downstream
/// of this builder is generic.
pub struct ClientBuilder<S> {
    state: Arc<S>,
    options: ClientOptions,
    allow_missing_intents: bool,
    cache: CacheConfig,
    transport: Option<Arc<dyn Transport>>,
    table: HandlerTable,
}

impl<S> std::fmt::Debug for ClientBuilder<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder").field("options", &self.options).finish_non_exhaustive()
    }
}

impl ClientBuilder<()> {
    pub(crate) fn new(token: String) -> Self {
        Self {
            state: Arc::new(()),
            options: ClientOptions {
                token: Secret::new(token),
                intents: Intents::empty(),
                endpoints: Endpoints::default(),
                dispatch_mode: DispatchMode::default(),
                compression: Compression::default(),
                presence: None,
                session_store: None,
                shard_count: None,
                shard_ids: None,
                shutdown_drain: Duration::from_secs(10),
                queue_limits: QueueLimits::default(),
            },
            allow_missing_intents: false,
            cache: CacheConfig::standard(),
            transport: None,
            table: HandlerTable::default(),
        }
    }

    /// Sets your state, which every handler gets as `ctx.state`. Only from the untyped start, so it
    /// cannot be set twice. Handlers registered before this keep the `()` state.
    pub fn state<N: Send + Sync + 'static>(self, state: N) -> ClientBuilder<N> {
        ClientBuilder {
            state: Arc::new(state),
            options: self.options,
            allow_missing_intents: self.allow_missing_intents,
            cache: self.cache,
            transport: self.transport,
            table: self.table,
        }
    }
}

impl<S: Send + Sync + 'static> ClientBuilder<S> {
    /// Sets the intents. `build` fails if a registered handler could never fire with these.
    pub fn intents(mut self, intents: Intents) -> Self {
        self.options.intents = intents;
        self
    }
    /// Sets what the cache stores. The default is [`CacheConfig::standard`].
    pub fn cache(mut self, config: CacheConfig) -> Self {
        self.cache = config;
        self
    }
    /// Sets the base URLs. Tests point this at the mock.
    pub fn endpoints(mut self, endpoints: Endpoints) -> Self {
        self.options.endpoints = endpoints;
        self
    }
    /// Replaces the REST transport.
    pub fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }
    /// Lets `build` accept a handler that could never fire with the current intents.
    pub fn allow_missing_intents(mut self) -> Self {
        self.allow_missing_intents = true;
        self
    }
    /// Sets how handlers run.
    pub fn dispatch_mode(mut self, mode: DispatchMode) -> Self {
        self.options.dispatch_mode = mode;
        self
    }
    /// Sets the transport compression.
    pub fn compression(mut self, compression: Compression) -> Self {
        self.options.compression = compression;
        self
    }
    /// Sets the presence sent at identify.
    pub fn presence(mut self, presence: UpdatePresence) -> Self {
        self.options.presence = Some(presence);
        self
    }
    /// Sets the total number of shards. Without it, the client asks Discord for its recommendation.
    /// Above 2 500 guilds, Discord requires sharding.
    pub fn shards(mut self, count: u32) -> Self {
        self.options.shard_count = Some(count);
        self
    }
    /// Runs only these shards of the total, for a bot split across processes. Each must be below the
    /// shard count, or `run` fails.
    pub fn shard_ids(mut self, ids: impl IntoIterator<Item = u32>) -> Self {
        self.options.shard_ids = Some(ids.into_iter().collect());
        self
    }
    /// Replaces the session store. The default is in memory.
    pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.options.session_store = Some(store);
        self
    }
    /// Sets how long shutdown waits for the queue and handlers. The default is 10 s.
    pub fn shutdown_drain(mut self, d: Duration) -> Self {
        self.options.shutdown_drain = d;
        self
    }
    /// Sets the limits of each shard's queue.
    pub fn queue_limits(mut self, limits: QueueLimits) -> Self {
        self.options.queue_limits = limits;
        self
    }

    fn wrap<P, F, Fut>(&self, f: F) -> Erased<P>
    where
        P: Send + 'static,
        F: Fn(Context<S>, P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let state = self.state.clone();
        Arc::new(move |client, shard, payload| {
            let ctx = Context { client, state: state.clone(), shard };
            Box::pin(f(ctx, payload))
        })
    }

    /// Runs `f` for every event of this kind, each in a task of its own. An error or a panic reaches
    /// `on_handler_error`, and the shard is unaffected.
    pub fn on<E, F, Fut>(mut self, _event: E, f: F) -> Self
    where
        E: GatewayEvent,
        F: Fn(Context<S>, E::Payload) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.add::<E>(erased);
        self
    }

    /// Runs `f` for the slash or context menu command with this name. A later handler for the same
    /// name replaces the earlier one.
    pub fn on_command<F, Fut>(mut self, name: &'static str, f: F) -> Self
    where
        F: Fn(Context<S>, Bound<CommandInteraction>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.commands.insert(name, erased);
        self
    }

    /// Runs `f` for components whose `custom_id` starts with this prefix. The first match in
    /// registration order wins.
    pub fn on_component<F, Fut>(mut self, prefix: &'static str, f: F) -> Self
    where
        F: Fn(Context<S>, Bound<ComponentInteraction>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.components.push((prefix, erased));
        self
    }

    /// Runs `f` for modals whose `custom_id` starts with this prefix. The first match in registration
    /// order wins.
    pub fn on_modal<F, Fut>(mut self, prefix: &'static str, f: F) -> Self
    where
        F: Fn(Context<S>, Bound<ModalInteraction>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.modals.push((prefix, erased));
        self
    }

    /// Runs `f` for autocomplete requests of the command with this name.
    pub fn on_autocomplete<F, Fut>(mut self, name: &'static str, f: F) -> Self
    where
        F: Fn(Context<S>, Bound<AutocompleteInteraction>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.autocomplete.insert(name, erased);
        self
    }

    /// Runs `f` for every dispatch, with its raw JSON, including those that failed to decode.
    pub fn on_raw<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Context<S>, RawEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        let erased = self.wrap(f);
        self.table.raw.push(erased);
        self
    }

    /// Runs `f` for every handler that returned an error or panicked. It runs on the failing task, so
    /// it must not block.
    pub fn on_handler_error(mut self, f: impl Fn(HandlerFailure) + Send + Sync + 'static) -> Self {
        self.table.on_failure = Some(Arc::new(f));
        self
    }

    /// Checks the setup and builds the client. Nothing connects until the [`Runner`] runs.
    ///
    /// Fails with [`ConfigError::MissingIntent`] when a handler could never fire, and when the REST
    /// client cannot be set up.
    pub fn build(self) -> Result<Runner, Error> {
        if !self.allow_missing_intents {
            for (event, needs) in self.table.needs() {
                if !needs.is_empty() && !self.options.intents.intersects(*needs) {
                    let have = self.options.intents;
                    return Err(ConfigError::MissingIntent { event, needs: *needs, have }.into());
                }
            }
        }
        let mut rest = Rest::builder().token(self.options.token.clone()).endpoints(self.options.endpoints.clone());
        if let Some(t) = self.transport {
            rest = rest.transport(t);
        }
        let inner = ClientInner {
            rest: rest.build()?,
            cache: Cache::new(self.cache),
            handlers: self.table,
            budget: Budget::new(self.options.dispatch_mode),
            tasks: TaskTracker::new(),
            gateway: OnceLock::new(),
            application_id: OnceLock::new(),
            ready: ReadyTracker::new(),
            stats: crate::ClientStats::default(),
            options: self.options,
            queue: OnceLock::new(),
            lifecycle: Lifecycle::default(),
            collectors: crate::collector::Collectors::default(),
        };
        Ok(Runner { client: Client(Arc::new(inner)) })
    }
}
