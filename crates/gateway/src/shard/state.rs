//! What the socket task shares with its handle, and what survives a reconnect.

use crate::events::{ShardStatus, ShutdownMode};
use crate::session::SessionInfo;
use dr_core::Secret;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, watch};

const IDLE: u32 = 0;
const CONNECTING: u32 = 1;
const IDENTIFYING: u32 = 2;
const RESUMING: u32 = 3;
const READY: u32 = 4;
/// A stored status at or above this is `Failed(status - FAILED_BASE)`.
const FAILED_BASE: u32 = 1 << 16;
const NO_LATENCY: u64 = u64::MAX;

/// The session being kept alive. Held outside the connection, so a reconnect can resume it.
#[derive(Clone, Debug)]
pub(crate) struct Session {
    pub(crate) id: Secret,
    pub(crate) resume_url: String,
}

/// State shared between the socket task and [`super::ShardHandle`].
#[derive(Debug)]
pub(crate) struct Shared {
    status: AtomicU32,
    latency_us: AtomicU64,
    /// Encoded commands, FIFO. Data held by the shard, so it survives a reconnect.
    pub(crate) queue: Mutex<VecDeque<String>>,
    pub(crate) queue_ready: Notify,
    pub(crate) shutdown: watch::Sender<Option<ShutdownMode>>,
    pub(crate) done: watch::Sender<bool>,
    pub(crate) session: Mutex<Option<Session>>,
    /// The last sequence seen. `None` until the first one, written before dispatch.
    pub(crate) sequence: Mutex<Option<u64>>,
}

impl Shared {
    pub(crate) fn new(session: Option<SessionInfo>) -> Self {
        let sequence = session.as_ref().map(|s| s.sequence);
        let session = session.map(|s| Session { id: s.session_id, resume_url: s.resume_url });
        Self {
            status: AtomicU32::new(IDLE),
            latency_us: AtomicU64::new(NO_LATENCY),
            queue: Mutex::new(VecDeque::new()),
            queue_ready: Notify::new(),
            shutdown: watch::Sender::new(None),
            done: watch::Sender::new(false),
            session: Mutex::new(session),
            sequence: Mutex::new(sequence),
        }
    }

    pub(crate) fn set_status(&self, status: ShardStatus) {
        let raw = match status {
            ShardStatus::Idle => IDLE,
            ShardStatus::Connecting => CONNECTING,
            ShardStatus::Identifying => IDENTIFYING,
            ShardStatus::Resuming => RESUMING,
            ShardStatus::Ready => READY,
            ShardStatus::Failed(code) => FAILED_BASE + u32::from(code),
        };
        self.status.store(raw, Ordering::Release);
    }

    pub(crate) fn status(&self) -> ShardStatus {
        match self.status.load(Ordering::Acquire) {
            CONNECTING => ShardStatus::Connecting,
            IDENTIFYING => ShardStatus::Identifying,
            RESUMING => ShardStatus::Resuming,
            READY => ShardStatus::Ready,
            raw if raw >= FAILED_BASE => ShardStatus::Failed(u16::try_from(raw - FAILED_BASE).unwrap_or(u16::MAX)),
            _ => ShardStatus::Idle,
        }
    }

    pub(crate) fn set_latency(&self, d: Duration) {
        self.latency_us.store(u64::try_from(d.as_micros()).unwrap_or(NO_LATENCY - 1), Ordering::Release);
    }

    pub(crate) fn latency(&self) -> Option<Duration> {
        match self.latency_us.load(Ordering::Acquire) {
            NO_LATENCY => None,
            us => Some(Duration::from_micros(us)),
        }
    }

    /// Advances monotonically: a replayed lower sequence never moves it back.
    pub(crate) fn advance(&self, s: u64) {
        let mut seq = self.sequence.lock();
        if seq.is_none_or(|old| s > old) {
            *seq = Some(s);
        }
    }

    pub(crate) fn sequence(&self) -> Option<u64> {
        *self.sequence.lock()
    }

    pub(crate) fn forget_session(&self) {
        *self.session.lock() = None;
        *self.sequence.lock() = None;
    }

    /// The session to resume, when there is both a session and a sequence.
    pub(crate) fn resumable(&self) -> Option<(Session, u64)> {
        let session = self.session.lock().clone()?;
        Some((session, self.sequence()?))
    }
}
