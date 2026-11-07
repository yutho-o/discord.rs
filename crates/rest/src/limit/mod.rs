//! The rate limiter. See `ARCHITECTURE.md` 10.2: limiter, global limit and burst, and the
//! invalid-request breaker. `client.rs` calls only [`Limiter::new`].
//!
//! Sublimits: some routes have limits inside their bucket that no header announces, such as two
//! channel renames per 10 minutes. A 429 that is not global while the bucket still has requests left
//! is one of them. It does not block the bucket: only the requests that fall under the sublimit wait,
//! outside the bucket's queue, so the others on the route keep going.

pub mod bucket;
pub mod burst;
pub mod global;
pub mod invalid;

use self::bucket::Bucket;
use self::burst::BurstLimiter;
use self::global::GlobalLimit;
use crate::error::RateLimitInfo;
use crate::route::RouteInfo;
use crate::{RestConfig, RestError};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::{Instant, sleep};

/// Reads a header by lower case name. [`crate::RawResponse`] names are lower case.
pub(crate) fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn seconds(value: Option<&str>) -> Option<Duration> {
    let s = value?.trim().parse::<f64>().ok()?;
    Duration::try_from_secs_f64(s).ok()
}

/// The `Retry-After` header. A 429's JSON body is ignored on purpose, per 10.2.
pub(crate) fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    seconds(header(headers, "retry-after"))
}

#[derive(Debug, Default)]
struct Maps {
    /// `METHOD:route` to the bucket hash Discord told us.
    hashes: HashMap<String, String>,
    /// `hash:major` to its bucket. A route with no known hash uses its own key.
    buckets: HashMap<String, Arc<Bucket>>,
    /// `METHOD:route:major` to the end of its sublimit.
    sublimits: HashMap<String, Instant>,
}

fn sublimit_key(route: &RouteInfo) -> String {
    format!("{}:{}", route.bucket_route, route.major)
}

/// Whether a request falls under its route's sublimit, as upstream decides: on `PATCH /channels/:id`
/// only a change of `name` or `topic`, and on any other route every request, to avoid a flood of
/// 429s.
pub fn has_sublimit(route: &RouteInfo, json: Option<&[u8]>) -> bool {
    if route.bucket_route == "PATCH:/channels/:id" {
        let Some(body) =
            json.and_then(|j| serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(j).ok())
        else {
            return false;
        };
        return body.contains_key("name") || body.contains_key("topic");
    }
    true
}

/// Per-client limiter state: the route to hash map, the buckets and the global window. The breaker
/// is process-wide, in [`invalid`].
#[derive(Debug)]
pub struct Limiter {
    maps: Mutex<Maps>,
    global: GlobalLimit,
    burst: BurstLimiter,
    offset: Duration,
    queue_limit: usize,
    waiting: Arc<AtomicUsize>,
    token_invalid: AtomicBool,
}

/// Releases a queue place on drop, also when a future is cancelled.
#[derive(Debug)]
struct WaitGuard(Arc<AtomicUsize>);

impl Drop for WaitGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A place taken in a bucket. Dropping it without [`Limiter::observe`] returns the place, so a
/// cancelled request leaves nothing held.
#[derive(Debug)]
pub struct Permit {
    route: RouteInfo,
    lane: Lane,
}

#[derive(Debug)]
enum Lane {
    Burst,
    Bucket { bucket: Arc<Bucket>, _place: OwnedSemaphorePermit },
}

impl Limiter {
    /// A limiter using the queue limit and global rate of `config`.
    pub fn new(config: &RestConfig) -> Self {
        Self {
            maps: Mutex::new(Maps::default()),
            global: GlobalLimit::new(config.global_per_second),
            burst: BurstLimiter::new(),
            offset: config.offset,
            queue_limit: config.queue_limit,
            waiting: Arc::new(AtomicUsize::new(0)),
            token_invalid: AtomicBool::new(false),
        }
    }

    fn bucket_for(&self, route: &RouteInfo) -> Arc<Bucket> {
        let mut maps = self.maps.lock();
        let key = match maps.hashes.get(&route.bucket_route) {
            Some(hash) => format!("{hash}:{}", route.major),
            None => format!("{}:{}", route.bucket_route, route.major),
        };
        Arc::clone(maps.buckets.entry(key).or_insert_with(|| Arc::new(Bucket::new())))
    }

    fn rejected(&self, route: &RouteInfo, wait: Duration) -> RestError {
        RestError::RateLimited(RateLimitInfo::new(wait, false, crate::error::mask_route(&route.bucket_route)))
    }

    /// How long the sublimit of `route` still holds, if it does.
    fn sublimit_wait(&self, route: &RouteInfo, now: Instant) -> Option<Duration> {
        let mut maps = self.maps.lock();
        let key = sublimit_key(route);
        match maps.sublimits.get(&key) {
            Some(until) if *until > now => Some(*until - now),
            Some(_) => {
                maps.sublimits.remove(&key);
                None
            }
            None => None,
        }
    }

    /// Waits for room in the route's bucket and the global window. A request that falls under the
    /// route's sublimit (see [`has_sublimit`]) first waits for it to end, outside the queue. Fails
    /// with `QueueFull` over the queue limit, or `RateLimited` when the request rejects instead of
    /// waiting. Cancel-safe.
    pub async fn acquire(&self, route: &RouteInfo, reject: bool, sublimited: bool) -> Result<Permit, RestError> {
        while sublimited && let Some(wait) = self.sublimit_wait(route, Instant::now()) {
            if reject {
                return Err(self.rejected(route, wait));
            }
            sleep(wait + self.offset).await;
        }
        if route.is_burst() {
            self.burst.wait(&route.bucket_route).await;
            return Ok(Permit { route: route.clone(), lane: Lane::Burst });
        }
        if self.waiting.fetch_add(1, Ordering::AcqRel) >= self.queue_limit {
            self.waiting.fetch_sub(1, Ordering::AcqRel);
            return Err(RestError::QueueFull);
        }
        let guard = WaitGuard(Arc::clone(&self.waiting));

        let bucket = self.bucket_for(route);
        let place = bucket.enter().await.ok_or(RestError::Cancelled)?;
        // Re-checked in a loop: the state can only change under this place, except for a global 429.
        while let Some(wait) = bucket.wait_needed(Instant::now()) {
            if reject {
                return Err(self.rejected(route, wait));
            }
            sleep(wait + self.offset).await;
        }
        self.global.wait().await;
        bucket.take();
        drop(guard);
        Ok(Permit { route: route.clone(), lane: Lane::Bucket { bucket, _place: place } })
    }

    /// Learns from a response: `X-RateLimit-*` headers, and for a 429 `Retry-After` and
    /// `X-RateLimit-Global`. Consumes the place. Returns how long to wait for a 429, `None` for
    /// anything else. A 429 with no usable header returns zero, and the caller applies its floor.
    pub fn observe(&self, permit: Permit, status: u16, headers: &[(String, String)]) -> Option<Duration> {
        let now = Instant::now();
        let Permit { route, lane } = permit;
        let bucket = match lane {
            Lane::Burst => {
                return self.burst.observe(&route.bucket_route, status, headers).map(|d| add_offset(d, self.offset));
            }
            Lane::Bucket { bucket, _place } => bucket,
        };

        // Later requests go to the bucket Discord named, while the old one drains by itself. Upstream
        // starts the named one from `remaining = 1`, which earns a 429 when this response already says
        // the bucket is empty, so here it learns from this response too.
        let named = header(headers, "x-ratelimit-bucket").map(|hash| {
            let mut maps = self.maps.lock();
            maps.hashes.insert(route.bucket_route.clone(), hash.to_owned());
            let key = format!("{hash}:{}", route.major);
            Arc::clone(maps.buckets.entry(key).or_insert_with(|| Arc::new(Bucket::new())))
        });
        let named = named.filter(|n| !Arc::ptr_eq(n, &bucket));
        let limit = header(headers, "x-ratelimit-limit").and_then(|v| v.trim().parse().ok());
        let remaining = header(headers, "x-ratelimit-remaining").and_then(|v| v.trim().parse().ok());
        let reset_after = seconds(header(headers, "x-ratelimit-reset-after"));
        bucket.update(limit, remaining, reset_after, now);
        if let Some(named) = &named {
            named.update(limit, remaining, reset_after, now);
        }

        if status != 429 {
            return None;
        }
        let wait = retry_after(headers).unwrap_or(Duration::ZERO);
        if wait.is_zero() {
            return Some(wait);
        }
        if header(headers, "x-ratelimit-global").is_some() {
            self.global.block_for(wait + self.offset);
        } else if remaining.is_some_and(|r: u32| r > 0) {
            // A sublimit: the bucket itself has room, so only this kind of request waits.
            tracing::debug!(route = %crate::error::mask_route(&route.bucket_route), ?wait, "hit a sublimit");
            self.maps.lock().sublimits.insert(sublimit_key(&route), now + wait);
        } else {
            bucket.block_for(wait, now);
            if let Some(named) = &named {
                named.block_for(wait, now);
            }
        }
        Some(wait + self.offset)
    }

    /// True after a 401 on the default token: every later call with that token fails fast.
    pub fn token_invalidated(&self) -> bool {
        self.token_invalid.load(Ordering::Acquire)
    }

    /// Records a 401 on the default token.
    pub fn invalidate_token(&self) {
        self.token_invalid.store(true, Ordering::Release);
    }

    /// Requests waiting in the limiter now. For tests and stats.
    pub fn queued(&self) -> usize {
        self.waiting.load(Ordering::Acquire)
    }
}

fn add_offset(d: Duration, offset: Duration) -> Duration {
    if d.is_zero() { d } else { d + offset }
}
