//! The manager: starts the shards of a client, all of them or a range, with one identify budget.

use crate::GatewayError;
use crate::compress::Compression;
use crate::events::{FrameSink, GatewayCommand, ShardStatus, ShutdownMode};
use crate::info::GatewayInfoProvider;
use crate::session::{MemorySessionStore, SessionStore};
use crate::shard::{IdentifyLimiter, ShardHandle, ShardOptions};
use dr_core::{Endpoints, Secret};
use dr_model::{Intents, UpdatePresence};
use std::collections::BTreeMap;
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
    shard_count: Option<u32>,
    shard_ids: Option<Vec<u32>>,
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
            shard_count: None,
            shard_ids: None,
        }
    }

    /// Finds the URL (`Endpoints::gateway_override`, else the provider, else `GatewayError::Info`)
    /// and the shard count (as set, else Discord's recommendation, else 1 with a URL override), loads
    /// each session and spawns each shard with `shard::spawn`. Shards identify in the order of their
    /// ids, `max_concurrency` at a time, 5 s apart. It returns once the shards are running, not once
    /// they are ready: readiness arrives as `ShardNotice`s.
    pub async fn start(self, sink: Arc<dyn FrameSink>) -> Result<ShardsHandle, GatewayError> {
        self.plan().await?.start(sink).await
    }

    /// The first half of [`ShardManager::start`]: finds the URL, the shard count and the ids to run,
    /// and connects nothing. The caller learns which shards will run before any of them can send
    /// `READY`.
    pub async fn plan(self) -> Result<ShardPlan, GatewayError> {
        let mut o = self.options;
        let info = match (&o.endpoints.gateway_override, &o.info_provider) {
            (Some(_), _) => None,
            (None, Some(provider)) => Some(provider.gateway_info().await.map_err(GatewayError::Info)?),
            (None, None) => return Err(GatewayError::Info("no gateway URL and no info provider".into())),
        };
        let url = match (&o.endpoints.gateway_override, &info) {
            (Some(url), _) => url.clone(),
            (None, Some(info)) => info.url.clone(),
            (None, None) => return Err(GatewayError::Info("no gateway URL".into())),
        };
        let count = o.shard_count.or(info.as_ref().map(|i| i.recommended_shards)).unwrap_or(1).max(1);
        let ids = o.shard_ids.take().unwrap_or_else(|| (0..count).collect());
        if let Some(bad) = ids.iter().find(|id| **id >= count) {
            return Err(GatewayError::UnknownShard { shard: *bad });
        }
        if let Some(info) = &info
            && info.remaining < u32::try_from(ids.len()).unwrap_or(u32::MAX)
        {
            tracing::warn!(
                remaining = info.remaining,
                shards = ids.len(),
                "the identify budget is low for this many shards"
            );
        }
        let limiter = Arc::new(IdentifyLimiter::new(info.as_ref().map_or(1, |i| i.max_concurrency)));
        Ok(ShardPlan { options: o, url, count, ids, limiter })
    }
}

/// Which shards a manager will run, before any connects. Get one from [`ShardManager::plan`].
pub struct ShardPlan {
    options: ShardManagerBuilder,
    url: String,
    count: u32,
    ids: Vec<u32>,
    limiter: Arc<IdentifyLimiter>,
}

impl std::fmt::Debug for ShardPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardPlan").field("count", &self.count).field("ids", &self.ids).finish_non_exhaustive()
    }
}

impl ShardPlan {
    /// The total number of shards of the bot.
    pub fn shard_count(&self) -> u32 {
        self.count
    }

    /// The ids of the shards this process will run, in order.
    pub fn shard_ids(&self) -> &[u32] {
        &self.ids
    }

    /// Loads each session and spawns each shard. Returns once they run, not once they are ready.
    pub async fn start(self, sink: Arc<dyn FrameSink>) -> Result<ShardsHandle, GatewayError> {
        let Self { options: o, url, count, ids, limiter } = self;
        let mut shards = BTreeMap::new();
        for id in ids {
            let mut options = ShardOptions::new(o.token.clone(), o.intents, url.clone(), sink.clone());
            options.shard_id = id;
            options.shard_count = count;
            options.identify_limiter = Some(limiter.clone());
            options.compression = o.compression;
            options.session = o.session_store.load(id).await;
            options.large_threshold = o.large_threshold;
            options.presence = o.presence.clone();
            options.session_store = o.session_store.clone();
            shards.insert(id, Arc::new(crate::shard::spawn(options)?));
        }
        Ok(ShardsHandle { shards: Arc::new(shards), count })
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
    /// Sets the total number of shards. Without it, the count Discord recommends.
    pub fn shard_count(mut self, count: u32) -> Self {
        self.shard_count = Some(count);
        self
    }
    /// Runs only these shards of the total, for a bot split across processes. Each must be below the
    /// shard count.
    pub fn shard_ids(mut self, ids: impl IntoIterator<Item = u32>) -> Self {
        self.shard_ids = Some(ids.into_iter().collect());
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
    shards: Arc<BTreeMap<u32, Arc<ShardHandle>>>,
    count: u32,
}

impl ShardsHandle {
    /// The total number of shards of the bot, including those other processes run.
    pub fn shard_count(&self) -> u32 {
        self.count
    }

    /// The ids of the shards this process runs, in order.
    pub fn shard_ids(&self) -> Vec<u32> {
        self.shards.keys().copied().collect()
    }

    /// The shard that receives the events of `guild`: `(guild_id >> 22) % shard_count`.
    pub fn shard_for(&self, guild: dr_model::GuildId) -> u32 {
        shard_for(guild, self.count)
    }

    /// Queues a command. FIFO, survives reconnects. Fails with `UnknownShard` for a shard this process
    /// does not run, `NotRunning` after a terminal failure, and `PayloadTooLarge` over 4096 bytes.
    pub fn send(&self, shard: u32, command: GatewayCommand) -> Result<(), GatewayError> {
        self.shards.get(&shard).ok_or(GatewayError::UnknownShard { shard })?.send(command)
    }

    /// The status of a shard. An unknown one reads as `Idle`.
    pub fn status(&self, shard: u32) -> ShardStatus {
        self.shards.get(&shard).map_or(ShardStatus::Idle, |s| s.status())
    }

    /// The last heartbeat round trip of a shard, `None` before the first ack.
    pub fn latency(&self, shard: u32) -> Option<Duration> {
        self.shards.get(&shard)?.latency()
    }

    /// Stops every shard and waits for its socket to close. Idempotent.
    pub async fn shutdown(&self, mode: ShutdownMode) {
        futures::future::join_all(self.shards.values().map(|s| s.shutdown(mode))).await;
    }
}

/// The shard that receives the events of `guild` among `count` shards.
pub fn shard_for(guild: dr_model::GuildId, count: u32) -> u32 {
    let count = u64::from(count.max(1));
    u32::try_from((guild.get() >> 22) % count).unwrap_or(0)
}
