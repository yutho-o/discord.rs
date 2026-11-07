//! Counters.

use std::sync::atomic::{AtomicU64, Ordering};

/// Counters for one client. Reads are cheap and never wait.
#[derive(Debug, Default)]
pub struct ClientStats {
    events_received: AtomicU64,
    decode_failures: AtomicU64,
    handler_errors: AtomicU64,
    handler_panics: AtomicU64,
    events_shed: AtomicU64,
}

impl ClientStats {
    /// Dispatches that reached the ingest task.
    pub fn events_received(&self) -> u64 {
        self.events_received.load(Ordering::Relaxed)
    }
    /// Dispatches whose body did not decode. They still reached `on_raw`.
    pub fn decode_failures(&self) -> u64 {
        self.decode_failures.load(Ordering::Relaxed)
    }
    /// Handlers that returned an error.
    pub fn handler_errors(&self) -> u64 {
        self.handler_errors.load(Ordering::Relaxed)
    }
    /// Handlers that panicked.
    pub fn handler_panics(&self) -> u64 {
        self.handler_panics.load(Ordering::Relaxed)
    }
    /// Sheddable events dropped under load.
    pub fn events_shed(&self) -> u64 {
        self.events_shed.load(Ordering::Relaxed)
    }

    pub(crate) fn record_event(&self) {
        self.events_received.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record_decode_failure(&self) {
        self.decode_failures.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record_handler_error(&self) {
        self.handler_errors.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record_handler_panic(&self) {
        self.handler_panics.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record_shed(&self, n: u64) {
        self.events_shed.fetch_add(n, Ordering::Relaxed);
    }
}
