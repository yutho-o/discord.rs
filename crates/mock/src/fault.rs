//! Faults a REST script can inject.

use std::time::Duration;

/// What can go wrong with a response, besides its status and body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fault {
    /// Wait this long before answering.
    Latency(Duration),
    /// Drop the connection before answering: an ambiguous failure for the client.
    ResetBeforeResponse,
    /// Do what the route does, then drop the connection instead of answering.
    ResetAfterResponse,
    /// Send fewer bytes than `Content-Length` says.
    TruncatedBody,
    /// A 5xx with an HTML body.
    NonJson5xx,
    /// A 429 with no rate limit headers.
    HeaderlessRateLimit,
    /// A Cloudflare style 429: HTML, and `Retry-After` only.
    CloudflareRateLimit,
}
