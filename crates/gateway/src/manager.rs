//! The manager, over a single shard, `[0, 1]`.

use crate::GatewayError;
use crate::compress::Compression;
use crate::events::{FrameSink, GatewayCommand, ShardStatus, ShutdownMode};
use crate::info::GatewayInfoProvider;
use crate::session::{MemorySessionStore, SessionStore};
use crate::shard::{ShardHandle, ShardOptions};
use dr_core::{Endpoints, Secret};
use dr_model::{Intents, UpdatePresence};
use std::sync::Arc;
use std::time::Duration;

/// Runs the shards of one client.
pub struct ShardManager {
    /// The settings, read by `start`.
    options: ShardManagerBuilder,
}

impl std::fmt::Debug for ShardManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardManager").field("intents", &self.options.intents).finish_non_exhaustive()
    }
}

/// Builds a [`ShardManager`].
pub struct ShardManagerBuilder {
    token: Secret,
    intents: Intents,
    compression: Compression,
    session_store: Arc<dyn SessionStore>,
    info_provider: Option<Arc<dyn GatewayInfoProvider>>,
    endpoints: Endpoints,
    presence: Option<UpdatePresence>,
    large_threshold: Option<u32>,
}

impl std::fmt::Debug for ShardManagerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardManagerBuilder").field("intents", &self.intents).finish_non_exhaustive()
    }
}

impl ShardManager {
    /// A builder for this token and these intents. The token's `Bot ` prefix, if any, is stripped
    /// at identify.
    pub fn builder(token: Secret, intents: Intents) -> ShardManagerBuilder {
        ShardManagerBuilder {
            token,
            intents,
            compression: Compression::default(),
            session_store: Arc::new(MemorySessionStore::new()),
            info_provider: None,
            endpoints: Endpoints::default(),
            presence: None,
            large_threshold: None,
        }
    }

    /// Finds the URL (`Endpoints::gateway_override`, else the provider, else `GatewayError::Info`),
    /// loads the session and spawns the shard with `shard::spawn`. It returns once the shard is
    /// running, not once it is ready: readiness arrives as a `ShardNotice`.
    pub async fn start(self, sink: Arc<dyn FrameSink>) -> Result<ShardsHandle, GatewayError> {
        let o = self.options;
        let url = match (&o.endpoints.gateway_override, &o.info_provider) {
            (Some(url), _) => url.clone(),
            (None, Some(provider)) => provider.gateway_info().await.map_err(GatewayError::Info)?.url,
            (None, None) => return Err(GatewayError::Info("no gateway URL and no info provider".into())),
        };
        let session = o.session_store.load(0).await;
        let mut options = ShardOptions::new(o.token, o.intents, url, sink);
        options.compression = o.compression;
        options.session = session;
        options.large_threshold = o.large_threshold;
        options.presence = o.presence;
        options.session_store = o.session_store;
        Ok(ShardsHandle::single(crate::shard::spawn(options)?))
    }
}

impl ShardManagerBuilder {
    /// Sets the transport compression. The default is `zlib-stream` when the feature is on.
    pub fn compression(mut self, c: Compression) -> Self {
        self.compression = c;
        self
    }
    /// Replaces the session store. The default is in memory.
    pub fn session_store(mut self, s: Arc<dyn SessionStore>) -> Self {
        self.session_store = s;
        self
    }
    /// Sets where the gateway URL comes from.
    pub fn info_provider(mut self, p: Arc<dyn GatewayInfoProvider>) -> Self {
        self.info_provider = Some(p);
        self
    }
    /// Sets the base URLs. A `gateway_override` skips the provider, which is how tests reach the mock.
    pub fn endpoints(mut self, e: Endpoints) -> Self {
        self.endpoints = e;
        self
    }
    /// Sets the presence sent at identify.
    pub fn presence(mut self, p: UpdatePresence) -> Self {
        self.presence = Some(p);
        self
    }
    /// Sets `large_threshold`, sent at identify only when set.
    pub fn large_threshold(mut self, n: u32) -> Self {
        self.large_threshold = Some(n);
        self
    }
    /// Builds the manager. Nothing connects until [`ShardManager::start`].
    pub fn build(self) -> ShardManager {
        ShardManager { options: self }
    }
}

/// A handle to the running shards. Clone it freely.
#[derive(Clone, Debug)]
pub struct ShardsHandle {
    shard: Arc<ShardHandle>,
}

impl ShardsHandle {
    /// Wraps a running shard.
    pub(crate) fn single(shard: ShardHandle) -> Self {
        Self { shard: Arc::new(shard) }
    }

    /// Queues a command. FIFO, survives reconnects. Fails with `UnknownShard` for anything but 0,
    /// `NotRunning` after a terminal failure, and `PayloadTooLarge` over 4096 bytes.
    pub fn send(&self, shard: u32, command: GatewayCommand) -> Result<(), GatewayError> {
        if shard != 0 {
            return Err(GatewayError::UnknownShard { shard });
        }
        self.shard.send(command)
    }

    /// The status of a shard. An unknown one reads as `Idle`.
    pub fn status(&self, shard: u32) -> ShardStatus {
        if shard == 0 { self.shard.status() } else { ShardStatus::Idle }
    }

    /// The last heartbeat round trip of a shard, `None` before the first ack.
    pub fn latency(&self, shard: u32) -> Option<Duration> {
        if shard == 0 { self.shard.latency() } else { None }
    }

    /// Stops every shard and waits for its socket to close. Idempotent.
    pub async fn shutdown(&self, mode: ShutdownMode) {
        self.shard.shutdown(mode).await;
    }
}
