//! The global limit: a lazily opened 1 s window, decremented per attempt, plus a block set by a
//! global 429.

use parking_lot::Mutex;
use std::time::Duration;
use tokio::time::{Instant, sleep_until};

const WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct State {
    remaining: u32,
    window_end: Option<Instant>,
    blocked_until: Option<Instant>,
}

/// Requests per second across the whole client.
#[derive(Debug)]
pub struct GlobalLimit {
    per_second: u32,
    state: Mutex<State>,
}

impl GlobalLimit {
    /// A limit of `per_second` requests. Zero means no window, only the block from a global 429.
    pub fn new(per_second: u32) -> Self {
        Self { per_second, state: Mutex::new(State { remaining: per_second, window_end: None, blocked_until: None }) }
    }

    /// Takes a place, waiting if the window is used up or a global 429 is in force. Cancel-safe: a
    /// place is only taken in the same step that returns.
    pub async fn wait(&self) {
        loop {
            let Some(until) = self.try_take(Instant::now()) else { return };
            sleep_until(until).await;
        }
    }

    fn try_take(&self, now: Instant) -> Option<Instant> {
        let mut s = self.state.lock();
        if let Some(until) = s.blocked_until {
            if now < until {
                return Some(until);
            }
            s.blocked_until = None;
        }
        if self.per_second == 0 {
            return None;
        }
        match s.window_end {
            Some(end) if now < end => {
                if s.remaining == 0 {
                    return Some(end);
                }
            }
            _ => {
                s.window_end = Some(now + WINDOW);
                s.remaining = self.per_second;
            }
        }
        s.remaining -= 1;
        None
    }

    /// Blocks every request on this client for `d`, for a global 429.
    pub fn block_for(&self, d: Duration) {
        let until = Instant::now() + d;
        let mut s = self.state.lock();
        if s.blocked_until.is_none_or(|old| old < until) {
            s.blocked_until = Some(until);
        }
    }
}
