//! The burst lane for interaction callbacks: no queue and no global accounting. A 429 makes later
//! requests of the same key wait.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{Instant, sleep_until};

/// Per-key wait state of the burst lane.
#[derive(Debug, Default)]
pub struct BurstLimiter {
    until: Mutex<HashMap<String, Instant>>,
}

impl BurstLimiter {
    /// An empty limiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits out an earlier 429 of this key, if any. Cancel-safe, since it holds nothing.
    pub async fn wait(&self, key: &str) {
        let until = self.until.lock().get(key).copied();
        if let Some(until) = until {
            sleep_until(until).await;
        }
    }

    /// Learns from a response. Returns how long to wait for a 429, and `None` for anything else.
    pub fn observe(&self, key: &str, status: u16, headers: &[(String, String)]) -> Option<Duration> {
        let now = Instant::now();
        let mut map = self.until.lock();
        map.retain(|_, until| *until > now);
        if status != 429 {
            return None;
        }
        let wait = super::retry_after(headers).unwrap_or(Duration::ZERO);
        map.insert(key.to_owned(), now + wait);
        Some(wait)
    }
}
