//! Settings.

use dr_core::{Endpoints, Secret};
use dr_gateway::{Compression, SessionStore};
use dr_model::{Intents, UpdatePresence};
use std::sync::Arc;
use std::time::Duration;

/// How handlers run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DispatchMode {
    /// Handlers of one event run concurrently, so order between handlers is not guaranteed. The
    /// order of cache writes is.
    Concurrent {
        /// In-flight event handlers.
        max_in_flight: usize,
        /// In-flight interaction handlers, a separate budget so that a flood of events cannot starve
        /// the 3 s reply.
        interactions: usize,
    },
    /// One handler at a time per shard, still under `catch_unwind`.
    OrderedPerShard,
}

impl Default for DispatchMode {
    fn default() -> Self {
        Self::Concurrent { max_in_flight: 2048, interactions: 512 }
    }
}

/// How much a shard's queue may hold, `ARCHITECTURE.md` 10.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct QueueLimits {
    /// Above this many bytes, sheddable events are dropped and counted.
    pub shed_bytes: usize,
    /// Above this many bytes, the socket stops reading. Heartbeats continue.
    pub pause_bytes: usize,
    /// Paused for longer than this, the socket closes 4200 and resumes.
    pub max_pause: Duration,
}

impl Default for QueueLimits {
    fn default() -> Self {
        Self { shed_bytes: 32 << 20, pause_bytes: 64 << 20, max_pause: Duration::from_secs(30) }
    }
}

/// The settings a client was built with. The runner reads these at start.
#[derive(Clone)]
#[non_exhaustive]
pub struct ClientOptions {
    /// The bot token, as given to `Client::builder`. Identify strips any `Bot ` prefix.
    pub token: Secret,
    /// The intents.
    pub intents: Intents,
    /// The base URLs.
    pub endpoints: Endpoints,
    /// How handlers run.
    pub dispatch_mode: DispatchMode,
    /// The transport compression.
    pub compression: Compression,
    /// The presence sent at identify.
    pub presence: Option<UpdatePresence>,
    /// Where sessions are kept. `None` means in memory.
    pub session_store: Option<Arc<dyn SessionStore>>,
    /// How long shutdown waits for the queue and handlers.
    pub shutdown_drain: Duration,
    /// Limits of the queue.
    pub queue_limits: QueueLimits,
}

impl std::fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientOptions")
            .field("intents", &self.intents)
            .field("endpoints", &self.endpoints)
            .field("dispatch_mode", &self.dispatch_mode)
            .field("compression", &self.compression)
            .field("shutdown_drain", &self.shutdown_drain)
            .field("queue_limits", &self.queue_limits)
            .finish_non_exhaustive()
    }
}
