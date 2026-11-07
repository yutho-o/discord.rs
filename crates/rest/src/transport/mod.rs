//! The transport seam. The default is reqwest; tests can substitute their own.

mod reqwest;
pub mod tls;

pub use self::reqwest::ReqwestTransport;

use crate::request::Method;
use bytes::Bytes;
use futures::future::BoxFuture;
use std::time::Duration;

/// One attempt, ready to send. `Debug` leaves out the URL and the headers, which hold secrets.
#[derive(Clone)]
pub struct PreparedRequest {
    /// The method.
    pub method: Method,
    /// The full URL. An interaction URL holds a token.
    pub url: String,
    /// The headers, including `Authorization`.
    pub headers: Vec<(&'static str, String)>,
    /// The body. Empty sends none.
    pub body: Bytes,
    /// Timeout for the response headers.
    pub timeout: Duration,
}

impl std::fmt::Debug for PreparedRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedRequest")
            .field("method", &self.method)
            .field("body_len", &self.body.len())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// A response, as read from the socket.
#[derive(Clone, Debug)]
pub struct RawResponse {
    /// HTTP status.
    pub status: u16,
    /// The headers, with lower case names.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Bytes,
}

/// A send failed with no HTTP response.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct TransportError {
    /// Whether it can have reached Discord.
    pub kind: TransportErrorKind,
    /// What happened.
    pub message: String,
}

/// Whether a failed send can have reached Discord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportErrorKind {
    /// The request cannot have reached Discord: connect, DNS, TLS. A retry is safe for any method.
    NotSent,
    /// It may or may not have been processed: a timeout or a reset. A retry depends on `RetryClass`.
    Ambiguous,
}

/// Sends one attempt. Retries and rate limits live above this, so a test transport is simple.
pub trait Transport: Send + Sync + 'static {
    /// Sends the request and reads the whole response.
    fn send(&self, req: PreparedRequest) -> BoxFuture<'static, Result<RawResponse, TransportError>>;
}
