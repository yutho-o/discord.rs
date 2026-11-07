//! `Runner`, which connects and runs until stopped.

use crate::pipeline::ingest;
use crate::pipeline::queue::ShardQueue;
use crate::{Client, Error};
use dr_core::BoxError;
use dr_gateway::{GatewayError, GatewayInfo, GatewayInfoProvider, ShardManager, ShutdownMode};
use dr_rest::Rest;
use futures::future::BoxFuture;
use std::future::Future;
use std::sync::Arc;

/// Finds the gateway URL through REST.
struct RestInfo(Rest);

impl GatewayInfoProvider for RestInfo {
    fn gateway_info(&self) -> BoxFuture<'_, Result<GatewayInfo, BoxError>> {
        Box::pin(async move {
            let bot = self.0.gateway().get_bot().await?;
            let mut info = GatewayInfo::new(bot.url);
            info.remaining = bot.session_start_limit.remaining;
            info.total = bot.session_start_limit.total;
            info.max_concurrency = bot.session_start_limit.max_concurrency;
            Ok(info)
        })
    }
}

/// A built client, not yet connected. Not generic, and one per client: running consumes it.
#[derive(Debug)]
pub struct Runner {
    pub(crate) client: Client,
}

impl Runner {
    /// The client, for use outside handlers. It works before the gateway connects, except for what
    /// needs `READY`.
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    /// Connects, and runs until `Client::shutdown` or a terminal failure. A terminal close code
    /// returns `Err`.
    pub async fn run(self) -> Result<(), Error> {
        let queue = start(&self.client).await?;
        wait(&self.client, &queue).await
    }

    /// Like [`Runner::run`], and shuts down with [`ShutdownMode::Final`] when `stop` completes. Clean
    /// shutdown on Ctrl-C: `.run_until(tokio::signal::ctrl_c())`.
    pub async fn run_until<F: Future + Send>(self, stop: F) -> Result<(), Error> {
        self.run_until_with(stop, ShutdownMode::Final).await
    }

    /// Like [`Runner::run_until`], with the choice of mode. `Resumable` closes 4200 and flushes the
    /// session store, so the next start can resume.
    pub async fn run_until_with<F: Future + Send>(self, stop: F, mode: ShutdownMode) -> Result<(), Error> {
        let queue = start(&self.client).await?;
        tokio::select! {
            result = wait(&self.client, &queue) => result,
            _ = stop => {
                self.client.shutdown(mode).await;
                Ok(())
            }
        }
    }
}

/// Starts the ingest task, then the shard.
async fn start(client: &Client) -> Result<Arc<ShardQueue>, Error> {
    let options = &client.0.options;
    let queue = Arc::new(ShardQueue::new(options.queue_limits));
    if client.0.lifecycle.is_stopping() {
        return Ok(queue);
    }
    let ingest_task = client.0.tasks.spawn(ingest::run(client.clone(), queue.clone()));
    client.0.lifecycle.set_ingest(ingest_task.abort_handle());
    // A second `Runner` cannot exist, so these are empty.
    let _ = client.0.queue.set(queue.clone());

    let mut manager = ShardManager::builder(options.token.clone(), options.intents)
        .compression(options.compression)
        .info_provider(Arc::new(RestInfo(client.0.rest.clone())))
        .endpoints(options.endpoints.clone());
    if let Some(store) = &options.session_store {
        manager = manager.session_store(store.clone());
    }
    if let Some(presence) = &options.presence {
        manager = manager.presence(presence.clone());
    }
    if let Some(count) = options.shard_count {
        manager = manager.shard_count(count);
    }
    if let Some(ids) = &options.shard_ids {
        manager = manager.shard_ids(ids.iter().copied());
    }
    let plan = manager.build().plan().await?;
    // Before any shard connects, so that no `READY` comes before the tracker knows what to wait for.
    client.0.ready.expect(plan.shard_ids().iter().copied());
    let handle = plan.start(queue.clone()).await?;
    let _ = client.0.gateway.set(handle);
    Ok(queue)
}

/// Waits for a shutdown to finish, or for the shard to fail for good.
async fn wait(client: &Client, queue: &ShardQueue) -> Result<(), Error> {
    tokio::select! {
        () = client.0.lifecycle.finished() => Ok(()),
        code = terminal_code(queue) => {
            client.shutdown(ShutdownMode::Final).await;
            Err(GatewayError::Closed { code }.into())
        }
    }
}

/// The terminal close code, once there is one. A function of its own so that the watch guard, which
/// is not `Send`, ends before anything else is awaited.
async fn terminal_code(queue: &ShardQueue) -> u16 {
    let mut failures = queue.failures();
    let failed = failures.wait_for(Option::is_some).await;
    failed.ok().and_then(|code| *code).unwrap_or(0)
}
