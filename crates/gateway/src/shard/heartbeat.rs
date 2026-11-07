//! Heartbeat bookkeeping, with no I/O.

use std::time::Duration;
use tokio::time::Instant;

/// The heartbeat state of one connection.
#[derive(Debug)]
pub(crate) struct Heartbeat {
    interval: Duration,
    awaiting_ack: bool,
    sent_at: Option<Instant>,
    latency: Option<Duration>,
}

impl Heartbeat {
    pub(crate) const fn new(interval: Duration) -> Self {
        Self { interval, awaiting_ack: false, sent_at: None, latency: None }
    }

    pub(crate) const fn interval(&self) -> Duration {
        self.interval
    }

    /// The first beat comes at `interval * jitter`, with `jitter` in `[0, 1)`.
    pub(crate) fn first_delay(&self, jitter: f64) -> Duration {
        self.interval.mul_f64(jitter.clamp(0.0, 1.0))
    }

    pub(crate) const fn on_sent(&mut self, now: Instant) {
        self.awaiting_ack = true;
        self.sent_at = Some(now);
    }

    pub(crate) fn on_ack(&mut self, now: Instant) {
        self.awaiting_ack = false;
        if let Some(sent) = self.sent_at.take() {
            self.latency = Some(now.saturating_duration_since(sent));
        }
    }

    /// True when the last beat was never acknowledged, so the connection is dead.
    pub(crate) const fn is_zombie(&self) -> bool {
        self.awaiting_ack
    }

    pub(crate) const fn latency(&self) -> Option<Duration> {
        self.latency
    }
}
