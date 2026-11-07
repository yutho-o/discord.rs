//! The rate limit headers every ordinary answer carries. The numbers are roomy, so that the client's
//! limiter never makes a test wait. A test that wants a limit scripts a 429.

use axum::http::HeaderValue;
use axum::response::Response;

/// Adds the headers, with one bucket per method and first two path segments.
pub(crate) fn add_headers(response: &mut Response, method: &str, path: &str) {
    let route: String = path.split('/').filter(|s| !s.is_empty()).take(2).collect::<Vec<_>>().join("-");
    let bucket = format!("mock-{method}-{route}");
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&bucket) {
        headers.insert("x-ratelimit-bucket", value);
    }
    headers.insert("x-ratelimit-limit", HeaderValue::from_static("1000"));
    headers.insert("x-ratelimit-remaining", HeaderValue::from_static("999"));
    headers.insert("x-ratelimit-reset-after", HeaderValue::from_static("60"));
    headers.insert("x-ratelimit-reset", HeaderValue::from_static("4000000000"));
}
