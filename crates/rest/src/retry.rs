//! The retry loop: the body of `Rest::execute`.
//!
//! Sublimits are handled in the limiter: see `crate::limit`.

use crate::client::RejectOnRateLimit;
use crate::error::{RateLimitInfo, RequestSummary, from_response, mask_route};
use crate::limit::invalid;
use crate::request::{Auth, Method, RequestParts};
use crate::route::{RouteInfo, route_info};
use crate::transport::{RawResponse, TransportErrorKind};
use crate::{Rest, RestError, encode};
use bytes::Bytes;
use dr_model::JsonErrorCode;
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};

/// Consecutive 429s with no `Retry-After` before giving up.
const MAX_BARE_429: u32 = 10;
const BARE_429_FLOOR: Duration = Duration::from_millis(100);

/// Which failures a request may resend after, `ARCHITECTURE.md` 10.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetryClass {
    /// `GET`, `PUT`, `DELETE`. The default for those methods.
    Idempotent,
    /// `POST`, `PATCH`. The default for those methods.
    Unprotected,
    /// Message create with `enforce_nonce`. The client generates a nonce when retries are enabled.
    Protected,
    /// Interaction callbacks. Resend only while the 3 s deadline allows. A 40060 after an ambiguous
    /// attempt counts as success, which is what makes the resend safe.
    Callback,
}

impl RetryClass {
    /// The default for a method.
    pub const fn for_method(method: Method) -> Self {
        match method {
            Method::Get | Method::Put | Method::Delete => Self::Idempotent,
            Method::Post | Method::Patch => Self::Unprotected,
        }
    }

    /// The failure table of 10.2, for a failed send.
    const fn retries_transport(self, kind: TransportErrorKind) -> bool {
        match kind {
            TransportErrorKind::NotSent => true,
            TransportErrorKind::Ambiguous => !matches!(self, Self::Unprotected),
        }
    }

    /// The failure table of 10.2, for a 5xx status.
    const fn retries_status(self, status: u16) -> bool {
        match self {
            Self::Idempotent => true,
            Self::Protected | Self::Callback => matches!(status, 502 | 503),
            Self::Unprotected => false,
        }
    }
}

fn should_reject(rest: &Rest, parts: &RequestParts, route: &RouteInfo) -> bool {
    if let Some(on) = parts.options.reject_on_rate_limit {
        return on;
    }
    match &rest.config().reject_on_rate_limit {
        RejectOnRateLimit::Always => true,
        RejectOnRateLimit::Routes(prefixes) => {
            // The key is `METHOD:/route`, and the prefixes are paths.
            let path = route.bucket_route.split_once(':').map_or(route.bucket_route.as_str(), |(_, p)| p);
            prefixes.iter().any(|p| path.starts_with(p.as_str()))
        }
        _ => false,
    }
}

/// Runs one request to its end: breaker, limiter, attempts, the failure table of 10.2, mapping a
/// non-2xx to [`crate::error::from_response`]. A 401 on `Auth::Default` invalidates the token.
/// Cancel-safe at every await: dropping the future gives up its place and leaves no permit behind,
/// which the required test `cancel_at_every_await` proves. `retry_class_matrix` covers the table.
pub async fn run(rest: Rest, parts: RequestParts) -> Result<RawResponse, RestError> {
    match parts.options.deadline {
        Some(limit) => timeout(limit, attempts(&rest, &parts, Some(Instant::now() + limit)))
            .await
            .unwrap_or(Err(RestError::Timeout)),
        None => attempts(&rest, &parts, None).await,
    }
}

fn summary(parts: &RequestParts, status: Option<u16>, attempts: u32) -> RequestSummary {
    RequestSummary::new(parts.method, mask_route(&parts.path), status, attempts)
}

async fn attempts(rest: &Rest, parts: &RequestParts, deadline: Option<Instant>) -> Result<RawResponse, RestError> {
    let class = parts.options.retry_class.unwrap_or(RetryClass::for_method(parts.method));
    let route = route_info(parts.method, &parts.path);
    let reject = should_reject(rest, parts, &route);
    let sublimited = crate::limit::has_sublimit(&route, parts.body.json.as_deref());
    let uses_token = matches!(parts.options.auth, Auth::Default);
    let max_attempts = rest.config().retries.saturating_add(1);

    let mut used: u32 = 0;
    let mut bare_429: u32 = 0;
    let mut ambiguous_before = false;

    loop {
        if uses_token && rest.limiter().token_invalidated() {
            return Err(RestError::TokenInvalidated);
        }
        if invalid::is_open(rest.config().invalid_request_breaker) {
            return Err(RestError::CircuitOpen);
        }
        let permit = rest.limiter().acquire(&route, reject, sublimited).await?;
        let prepared = encode::prepare(rest.config(), parts)?;
        let send_limit = prepared.timeout;
        used = used.saturating_add(1);

        let sent = match timeout(send_limit, rest.transport().send(prepared)).await {
            Ok(result) => result,
            Err(_) => Err(crate::TransportError {
                kind: TransportErrorKind::Ambiguous,
                message: String::from("no response before the attempt timeout"),
            }),
        };
        let response = match sent {
            Ok(r) => r,
            Err(e) => {
                drop(permit);
                ambiguous_before |= e.kind == TransportErrorKind::Ambiguous;
                if used < max_attempts && class.retries_transport(e.kind) {
                    tracing::debug!(route = %mask_route(&parts.path), attempt = used, kind = ?e.kind,"retrying after a send failure");
                    continue;
                }
                return Err(e.into());
            }
        };

        let status = response.status;
        invalid::record(status);
        let wait = rest.limiter().observe(permit, status, &response.headers);

        if (200..300).contains(&status) {
            return Ok(response);
        }
        if status == 429 {
            // A 429 consumes no attempt, since Discord refused the request.
            used = used.saturating_sub(1);
            let mut d = wait.unwrap_or(Duration::ZERO);
            if d.is_zero() {
                // A bare 429, such as from Cloudflare, stays local to this request.
                bare_429 += 1;
                if bare_429 >= MAX_BARE_429 {
                    return Err(rate_limited(BARE_429_FLOOR, &response, parts));
                }
                d = BARE_429_FLOOR.saturating_mul(1 << (bare_429 - 1).min(16));
            } else {
                bare_429 = 0;
            }
            if reject {
                return Err(rate_limited(d, &response, parts));
            }
            if deadline.is_some_and(|end| Instant::now() + d >= end) {
                return Err(rate_limited(d, &response, parts));
            }
            sleep(d).await;
            continue;
        }
        if status >= 500 && used < max_attempts && class.retries_status(status) {
            continue;
        }

        let error = from_response(status, &response.headers, &response.body, summary(parts, Some(status), used));
        if class == RetryClass::Callback && ambiguous_before && is_already_acknowledged(&error) {
            // The earlier attempt reached Discord, so the callback did what it was asked to.
            return Ok(RawResponse { status: 204, headers: response.headers, body: Bytes::new() });
        }
        if status == 401 && uses_token {
            rest.limiter().invalidate_token();
        }
        return Err(error);
    }
}

fn is_already_acknowledged(error: &RestError) -> bool {
    error.code() == Some(JsonErrorCode::INTERACTION_HAS_ALREADY_BEEN_ACKNOWLEDGED)
}

fn rate_limited(wait: Duration, response: &RawResponse, parts: &RequestParts) -> RestError {
    let global = crate::limit::header(&response.headers, "x-ratelimit-global").is_some();
    let mut info = RateLimitInfo::new(wait, global, mask_route(&parts.path));
    info.bucket = crate::limit::header(&response.headers, "x-ratelimit-bucket").map(str::to_owned);
    info.scope = crate::limit::header(&response.headers, "x-ratelimit-scope").map(str::to_owned);
    RestError::RateLimited(info)
}
