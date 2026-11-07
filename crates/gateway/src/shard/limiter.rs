//! `IdentifyLimiter`: the identify budget shared by every shard of a client.
//!
//! Discord lets `max_concurrency` shards identify at once: shard `id` is in bucket
//! `id % max_concurrency`, and each bucket takes one identify per 5 s. A shard that ignores this is
//! closed with 4009 or rate limited, and with many shards that would cascade.

use parking_lot::Mutex;
use std::time::Duration;
use tokio::time::Instant;

/// Time between two identifies of one bucket.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(5);

/// Hands out identify times, one bucket per `max_concurrency` slot.
#[derive(Debug)]
pub struct IdentifyLimiter {
    buckets: Vec<Mutex<Option<Instant>>>,
}

impl IdentifyLimiter {
    /// A limiter with `max_concurrency` buckets, at least one.
    pub fn new(max_concurrency: u32) -> Self {
        let n = usize::try_from(max_concurrency.max(1)).unwrap_or(1);
        Self { buckets: (0..n).map(|_| Mutex::new(None)).collect() }
    }

    /// Reserves the next identify time of the bucket of `shard`, no earlier than `earliest`. Callers
    /// wait until the returned time, then identify.
    pub fn reserve(&self, shard: u32, earliest: Instant) -> Instant {
        let index = usize::try_from(shard).unwrap_or(0) % self.buckets.len();
        let Some(bucket) = self.buckets.get(index) else { return earliest };
        let mut last = bucket.lock();
        let at = last.map_or(earliest, |previous| earliest.max(previous + IDENTIFY_INTERVAL));
        *last = Some(at);
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bucket_spaces_every_shard() {
        let limiter = IdentifyLimiter::new(1);
        let now = Instant::now();
        let times: Vec<_> = (0..3).map(|shard| limiter.reserve(shard, now)).collect();
        assert_eq!(times, [now, now + IDENTIFY_INTERVAL, now + IDENTIFY_INTERVAL * 2]);
    }

    #[test]
    fn buckets_run_side_by_side() {
        let limiter = IdentifyLimiter::new(2);
        let now = Instant::now();
        let times: Vec<_> = (0..4).map(|shard| limiter.reserve(shard, now)).collect();
        assert_eq!(times, [now, now, now + IDENTIFY_INTERVAL, now + IDENTIFY_INTERVAL]);
        assert_eq!(IdentifyLimiter::new(0).reserve(7, now), now, "zero reads as one");
    }

    #[test]
    fn a_later_request_is_not_pulled_earlier() {
        let limiter = IdentifyLimiter::new(1);
        let now = Instant::now();
        limiter.reserve(0, now);
        let late = now + Duration::from_secs(60);
        assert_eq!(limiter.reserve(0, late), late);
    }
}
