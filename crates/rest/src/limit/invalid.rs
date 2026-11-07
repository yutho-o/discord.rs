//! The invalid-request breaker. A process-wide static, because Discord counts per IP: a 10-minute
//! fixed window over 401, 403 and 429. Warn at 5000, and fail with `CircuitOpen` at the limit in
//! `RestConfig`.

use parking_lot::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(600);
const WARN_AT: u32 = 5000;

#[derive(Debug)]
struct Window {
    started: Option<Instant>,
    count: u32,
}

static WINDOW_STATE: Mutex<Window> = Mutex::new(Window { started: None, count: 0 });

fn roll(w: &mut Window, now: Instant) {
    if w.started.is_none_or(|s| now.duration_since(s) >= WINDOW) {
        w.started = Some(now);
        w.count = 0;
    }
}

/// Counts a response. Only 401, 403 and 429 count.
pub fn record(status: u16) {
    if !matches!(status, 401 | 403 | 429) {
        return;
    }
    let now = Instant::now();
    let count = {
        let mut w = WINDOW_STATE.lock();
        roll(&mut w, now);
        w.count = w.count.saturating_add(1);
        w.count
    };
    if count == WARN_AT {
        tracing::warn!(count, "many invalid requests in this 10 minute window, an IP ban is close");
    }
}

/// True when the current window holds at least `limit` invalid requests.
pub fn is_open(limit: u32) -> bool {
    let now = Instant::now();
    let mut w = WINDOW_STATE.lock();
    roll(&mut w, now);
    w.count >= limit
}
