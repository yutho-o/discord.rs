//! One shard: a socket task that owns the WebSocket, its state and timers.
//!
//! Nothing outside this directory names the submodules, apart from the close code table.

pub mod close;
pub mod heartbeat;
pub mod identify;
pub mod limiter;
pub mod send;
pub mod state;
pub mod task;

pub use self::limiter::IdentifyLimiter;
use self::state::Shared;
use crate::GatewayError;
use crate::compress::Compression;
use crate::events::{FrameSink, GatewayCommand, ShardStatus, ShutdownMode};
use crate::session::{MemorySessionStore, SessionInfo, SessionStore};
use dr_core::Secret;
use dr_model::{GatewayFrame, Intents, UpdatePresence};
use std::sync::Arc;
use std::time::Duration;

/// How to run one shard. Build it with [`ShardOptions::new`] and set the fields that differ, so that
/// a field added later breaks no caller.
#[non_exhaustive]
pub struct ShardOptions {
    /// The bot token. A `Bot ` prefix is stripped at identify.
    pub token: Secret,
    /// The intents.
    pub intents: Intents,
    /// The shard id, below `shard_count`.
    pub shard_id: u32,
    /// The total number of shards of the bot, across every process.
    pub shard_count: u32,
    /// The identify budget shared with the other shards. `None` spaces this shard's own identifies
    /// only.
    pub identify_limiter: Option<Arc<IdentifyLimiter>>,
    /// The transport compression.
    pub compression: Compression,
    /// The WebSocket URL, without a query.
    pub url: String,
    /// A session to try to resume first, used only if its `shard_count` matches.
    pub session: Option<SessionInfo>,
    /// `large_threshold`, sent at identify only when set.
    pub large_threshold: Option<u32>,
    /// The presence sent at identify, only when set.
    pub presence: Option<UpdatePresence>,
    /// Where the session is written through.
    pub session_store: Arc<dyn SessionStore>,
    /// Where frames and notices go.
    pub sink: Arc<dyn FrameSink>,
}

impl ShardOptions {
    /// Options for shard `[0, 1]`, with default compression and an in-memory store.
    pub fn new(token: Secret, intents: Intents, url: impl Into<String>, sink: Arc<dyn FrameSink>) -> Self {
        Self {
            token,
            intents,
            shard_id: 0,
            shard_count: 1,
            identify_limiter: None,
            compression: Compression::default(),
            url: url.into(),
            session: None,
            large_threshold: None,
            presence: None,
            session_store: Arc::new(MemorySessionStore::new()),
            sink,
        }
    }
}

/// Starts the shard's socket task on the current tokio runtime, and returns at once. Connecting,
/// identifying, heartbeats, resuming, reconnecting with the backoff of `ARCHITECTURE.md` 10.1 and the
/// close code table are all the task's. A panic in it is caught by a supervisor, which keeps the
/// session and restarts with backoff.
pub fn spawn(options: ShardOptions) -> Result<ShardHandle, GatewayError> {
    let shared = Arc::new(Shared::new(options.session.clone().filter(|s| s.shard_count == options.shard_count)));
    let connector = crate::connector::ws_connector()?;
    let ctx = Arc::new(task::Ctx { options, shared: Arc::clone(&shared), connector });
    tokio::spawn(task::supervise(ctx));
    Ok(ShardHandle { shared })
}

/// The payload limit of a gateway command.
const MAX_COMMAND_BYTES: usize = 4096;

/// A handle to a running shard.
#[derive(Debug)]
pub struct ShardHandle {
    shared: Arc<Shared>,
}

impl ShardHandle {
    /// Queues a command. Ops other than 1, 2 and 6 wait for Ready or Resumed, in FIFO order, at no
    /// more than 114 per 60 s per connection. The queue survives reconnects. Over 4096 bytes fails
    /// with `PayloadTooLarge` and sends nothing.
    pub fn send(&self, command: GatewayCommand) -> Result<(), GatewayError> {
        if matches!(self.status(), ShardStatus::Failed(_)) || self.shared.shutdown.borrow().is_some() {
            return Err(GatewayError::NotRunning);
        }
        let text = match command {
            GatewayCommand::UpdatePresence(p) => serde_json::to_string(&GatewayFrame::presence(p))?,
            GatewayCommand::RequestGuildMembers(r) => serde_json::to_string(&GatewayFrame::request_guild_members(r))?,
            GatewayCommand::Raw { op, d } => serde_json::to_string(&GatewayFrame { op, d })?,
        };
        if text.len() > MAX_COMMAND_BYTES {
            return Err(GatewayError::PayloadTooLarge { size: text.len(), limit: MAX_COMMAND_BYTES });
        }
        self.shared.queue.lock().push_back(text);
        self.shared.queue_ready.notify_one();
        Ok(())
    }

    /// The current status. Reads an atomic and never waits on the socket task.
    pub fn status(&self) -> ShardStatus {
        self.shared.status()
    }

    /// The last heartbeat round trip, `None` before the first ack.
    pub fn latency(&self) -> Option<Duration> {
        self.shared.latency()
    }

    /// Stops the shard: closes 1000 for `Final`, or 4200 and flushes the session store for
    /// `Resumable`, then waits up to 5 s for the peer's close frame. Idempotent.
    pub async fn shutdown(&self, mode: ShutdownMode) {
        self.shared.shutdown.send_if_modified(|current| {
            if current.is_none() {
                *current = Some(mode);
                true
            } else {
                false
            }
        });
        let mut done = self.shared.done.subscribe();
        // The close itself is bounded inside the task, so this only guards against a stuck store.
        let _ = tokio::time::timeout(Duration::from_secs(20), done.wait_for(|d| *d)).await;
    }
}
