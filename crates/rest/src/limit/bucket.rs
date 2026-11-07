//! One rate limit bucket. FIFO waiters, remaining and reset from the last response. Internal to the
//! limiter.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

#[derive(Debug)]
struct BucketState {
    limit: Option<u32>,
    remaining: u32,
    reset_at: Option<Instant>,
}

/// One queue and its limit state. A new bucket has `remaining = 1` and no reset, so exactly one
/// request goes out to learn the real numbers.
#[derive(Debug)]
pub(super) struct Bucket {
    // tokio's semaphore is FIFO, and a dropped acquire gives up its place.
    queue: Arc<Semaphore>,
    state: Mutex<BucketState>,
}

impl Bucket {
    pub(super) fn new() -> Self {
        Self {
            queue: Arc::new(Semaphore::new(1)),
            state: Mutex::new(BucketState { limit: None, remaining: 1, reset_at: None }),
        }
    }

    /// Waits for the front of the queue. `None` only if the semaphore closed, which never happens.
    pub(super) async fn enter(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.queue).acquire_owned().await.ok()
    }

    /// How long until this bucket allows a request. Refills it once the reset has passed.
    pub(super) fn wait_needed(&self, now: Instant) -> Option<Duration> {
        let mut s = self.state.lock();
        if s.remaining > 0 {
            return None;
        }
        match s.reset_at {
            Some(reset) if now < reset => Some(reset - now),
            _ => {
                s.remaining = s.limit.unwrap_or(1).max(1);
                s.reset_at = None;
                None
            }
        }
    }

    /// Counts the request about to be sent.
    pub(super) fn take(&self) {
        let mut s = self.state.lock();
        s.remaining = s.remaining.saturating_sub(1);
    }

    /// Replaces the state from response headers. Missing headers reset it, per 10.2.
    pub(super) fn update(
        &self,
        limit: Option<u32>,
        remaining: Option<u32>,
        reset_after: Option<Duration>,
        now: Instant,
    ) {
        let mut s = self.state.lock();
        s.limit = limit;
        s.remaining = remaining.unwrap_or(1);
        s.reset_at = Some(now + reset_after.unwrap_or(Duration::ZERO));
    }

    /// A local 429: nothing more until `d` has passed.
    pub(super) fn block_for(&self, d: Duration, now: Instant) {
        let mut s = self.state.lock();
        s.remaining = 0;
        s.reset_at = Some(now + d);
    }
}
