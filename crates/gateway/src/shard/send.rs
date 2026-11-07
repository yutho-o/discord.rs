//! The send budget of one connection, with no I/O.

use std::time::Duration;
use tokio::time::Instant;

/// Commands per window. Discord allows 120, and upstream's check `sent + 1 >= 115` allows 114.
pub(crate) const PER_WINDOW: u32 = 114;
const WINDOW: Duration = Duration::from_secs(60);
const JITTER_MAX: Duration = Duration::from_millis(1500);

/// A fixed 60 s window. Ops 1, 2 and 6 never come through here.
#[derive(Debug)]
pub(crate) struct SendBudget {
    sent: u32,
    window_end: Instant,
}

impl SendBudget {
    pub(crate) fn new(now: Instant) -> Self {
        Self { sent: 0, window_end: now + WINDOW }
    }

    /// Takes a place, or says how long to wait for the next window, plus up to 1.5 s of jitter.
    pub(crate) fn try_take(&mut self, now: Instant, jitter: f64) -> Result<(), Duration> {
        if now >= self.window_end {
            self.sent = 0;
            self.window_end = now + WINDOW;
        }
        if self.sent >= PER_WINDOW {
            return Err(self.window_end.saturating_duration_since(now) + JITTER_MAX.mul_f64(jitter.clamp(0.0, 1.0)));
        }
        self.sent += 1;
        Ok(())
    }
}
