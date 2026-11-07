//! `Request<T>`, the lazy request, generic only in its decode tail.

use crate::encode::encode_query;
use crate::retry::RetryClass;
use crate::transport::RawResponse;
use crate::{Rest, RestError};
use dr_core::{BodyError, RequestBody, Secret};
use futures::future::BoxFuture;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::IntoFuture;
use std::marker::PhantomData;
use std::time::Duration;

/// An HTTP method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    /// `GET`
    Get,
    /// `POST`
    Post,
    /// `PUT`
    Put,
    /// `PATCH`
    Patch,
    /// `DELETE`
    Delete,
}

impl Method {
    /// The upper case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Which `Authorization` header a request carries.
#[derive(Clone, Debug, Default)]
pub enum Auth {
    /// `Bot <token>` from the client. A client with no token fails with `MissingToken`.
    #[default]
    Default,
    /// No header. Interaction callbacks and interaction webhooks use this, so a 401 on them does
    /// not invalidate the bot token.
    None,
    /// The whole header value, as given.
    Custom(Secret),
}

/// Settings of one request.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct RequestOptions {
    /// Which `Authorization` header to send.
    pub auth: Auth,
    /// Per-request override of the client's `RejectOnRateLimit`.
    pub reject_on_rate_limit: Option<bool>,
    /// The retry class. `None` means the default for the method, [`RetryClass::for_method`].
    pub retry_class: Option<RetryClass>,
    /// Total time this request may take across attempts. A `Callback` request uses this for the
    /// 3 s interaction deadline.
    pub deadline: Option<Duration>,
}

/// The non-generic form of a request. Only this goes through the limiter and the retry loop.
///
/// `Debug` shows the masked route alone: an interaction path holds a token, and a body can hold
/// anything.
#[non_exhaustive]
pub struct RequestParts {
    /// The method.
    pub method: Method,
    /// The encoded path, starting with `/`, without the API base.
    pub path: String,
    /// The encoded query string, without the `?`.
    pub query: Option<String>,
    /// The body.
    pub body: RequestBody,
    /// The `X-Audit-Log-Reason`, not yet encoded.
    pub reason: Option<String>,
    /// The other settings.
    pub options: RequestOptions,
}

impl std::fmt::Debug for RequestParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestParts")
            .field("method", &self.method)
            .field("route", &crate::error::mask_route(&self.path))
            .finish_non_exhaustive()
    }
}

impl RequestParts {
    /// Parts with the given method, path and body, and default settings.
    pub fn new(method: Method, path: impl Into<String>, body: RequestBody) -> Self {
        Self { method, path: path.into(), query: None, body, reason: None, options: RequestOptions::default() }
    }
}

/// A lazy request: nothing is sent until it is awaited. Generic only in the decode tail.
#[must_use = "a request does nothing until it is awaited"]
pub struct Request<T> {
    rest: Rest,
    parts: Result<RequestParts, RestError>,
    output: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request").finish_non_exhaustive()
    }
}

impl<T> Request<T> {
    /// A request. A failed `body` surfaces when it is awaited, with nothing sent.
    pub fn new(rest: Rest, method: Method, path: String, body: Result<RequestBody, BodyError>) -> Self {
        let parts = body.map(|b| RequestParts::new(method, path, b)).map_err(RestError::from);
        Self { rest, parts, output: PhantomData }
    }

    /// A request that fails with `error` when awaited. API stubs use it.
    pub fn from_error(rest: Rest, error: RestError) -> Self {
        Self { rest, parts: Err(error), output: PhantomData }
    }

    fn with_parts(mut self, f: impl FnOnce(&mut RequestParts)) -> Self {
        if let Ok(p) = &mut self.parts {
            f(p);
        }
        self
    }

    /// Sets the `X-Audit-Log-Reason`. It is encoded at send time.
    pub fn reason(self, r: impl Into<String>) -> Self {
        let r = r.into();
        self.with_parts(|p| p.reason = Some(r))
    }

    /// Sets the query string from a serializable value. A failure surfaces when it is awaited.
    pub fn query(mut self, q: impl Serialize) -> Self {
        match encode_query(&q) {
            Ok(s) => self.with_parts(|p| p.query = (!s.is_empty()).then_some(s)),
            Err(e) => {
                self.parts = Err(e.into());
                self
            }
        }
    }

    /// Overrides the client's `RejectOnRateLimit` for this request.
    pub fn reject_on_rate_limit(self, on: bool) -> Self {
        self.with_parts(|p| p.options.reject_on_rate_limit = Some(on))
    }

    /// Sets which `Authorization` header to send.
    pub fn auth(self, a: Auth) -> Self {
        self.with_parts(|p| p.options.auth = a)
    }

    /// Sets the retry class. Endpoint files set this, `ARCHITECTURE.md` 10.2.
    pub fn retry_class(self, c: RetryClass) -> Self {
        self.with_parts(|p| p.options.retry_class = Some(c))
    }

    /// Sets the total time this request may take across attempts.
    pub fn deadline(self, d: Duration) -> Self {
        self.with_parts(|p| p.options.deadline = Some(d))
    }
}

/// Decodes a response body. An empty body reads as `null`, so `Empty` and `Option` accept a 204.
fn decode_tail<T: DeserializeOwned>(resp: &RawResponse) -> Result<T, RestError> {
    let body: &[u8] = if resp.body.is_empty() { b"null" } else { &resp.body };
    let preview = || String::from_utf8_lossy(&body[..body.len().min(512)]).into_owned();
    let text = std::str::from_utf8(body).map_err(|e| RestError::Decode {
        status: resp.status,
        path: String::new(),
        source: <serde_json::Error as serde::de::Error>::custom(e),
        body_preview: preview(),
    })?;
    dr_core::decode(text).map_err(|e| RestError::Decode {
        status: resp.status,
        path: e.path,
        source: e.source,
        body_preview: preview(),
    })
}

impl<T: DeserializeOwned + Send + 'static> IntoFuture for Request<T> {
    type Output = Result<T, RestError>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let resp = self.rest.execute(self.parts?).await?;
            decode_tail(&resp)
        })
    }
}

/// The body of a void endpoint: a 204 or any other body is accepted and dropped.
#[derive(Clone, Copy, Debug, Default)]
pub struct Empty;

impl<'de> serde::Deserialize<'de> for Empty {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(d).map(|_| Self)
    }
}
