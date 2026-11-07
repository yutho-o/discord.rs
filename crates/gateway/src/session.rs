//! Session persistence. Only the in-memory store exists so far.

use dr_core::Secret;
use futures::future::BoxFuture;
use parking_lot::Mutex;
use std::collections::HashMap;

/// What is needed to resume.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SessionInfo {
    /// The session id. Redacted in `Debug`.
    pub session_id: Secret,
    /// Where to resume.
    pub resume_url: String,
    /// The last sequence seen.
    pub sequence: u64,
    /// The shard.
    pub shard_id: u32,
    /// The shard count it was created under. A different current count means identify.
    pub shard_count: u32,
}

impl SessionInfo {
    /// A session.
    pub fn new(
        session_id: Secret,
        resume_url: impl Into<String>,
        sequence: u64,
        shard_id: u32,
        shard_count: u32,
    ) -> Self {
        Self { session_id, resume_url: resume_url.into(), sequence, shard_id, shard_count }
    }
}

/// Keeps sessions across connections, and across process restarts if the store is durable. Written
/// through on `READY`, `RESUMED`, every 30 s and at shutdown.
pub trait SessionStore: Send + Sync + 'static {
    /// The stored session of a shard.
    fn load(&self, shard: u32) -> BoxFuture<'_, Option<SessionInfo>>;
    /// Stores a shard's session. `None` clears it.
    fn save(&self, shard: u32, info: Option<SessionInfo>) -> BoxFuture<'_, ()>;
}

/// The default store: survives reconnects, not restarts.
#[derive(Debug, Default)]
pub struct MemorySessionStore {
    sessions: Mutex<HashMap<u32, SessionInfo>>,
}

impl MemorySessionStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for MemorySessionStore {
    fn load(&self, shard: u32) -> BoxFuture<'_, Option<SessionInfo>> {
        let found = self.sessions.lock().get(&shard).cloned();
        Box::pin(async move { found })
    }
    fn save(&self, shard: u32, info: Option<SessionInfo>) -> BoxFuture<'_, ()> {
        {
            let mut sessions = self.sessions.lock();
            match info {
                Some(info) => {
                    sessions.insert(shard, info);
                }
                None => {
                    sessions.remove(&shard);
                }
            }
        }
        Box::pin(async {})
    }
}
