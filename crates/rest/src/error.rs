//! `RestError` and what is read from an error response.
//!
//! Tokens never reach an error: paths go through [`mask_route`] first.

use crate::request::Method;
use crate::transport::TransportError;
use dr_core::{BodyError, ValidationError};
use dr_model::{JsonErrorCode, RestErrorBody};
use std::time::Duration;

/// One failed field of a form body error: `{"errors":{"embeds":{"0":{"title":{"_errors":[...]}}}}}`
/// flattens to `embeds[0].title`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldError {
    /// Path of the field, such as `embeds[0].title`.
    pub path: String,
    /// Discord's code for the failure, such as `BASE_TYPE_REQUIRED`.
    pub code: String,
    /// Discord's message.
    pub message: String,
}

impl FieldError {
    /// A field error.
    pub fn new(path: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { path: path.into(), code: code.into(), message: message.into() }
    }
}

/// What is known about a rate limit a request gave up on.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RateLimitInfo {
    /// How long Discord asked to wait.
    pub retry_after: Duration,
    /// Whether the limit is the global one.
    pub global: bool,
    /// The bucket hash, when sent.
    pub bucket: Option<String>,
    /// The masked route.
    pub route: String,
    /// The `X-RateLimit-Scope`, when sent.
    pub scope: Option<String>,
}

impl RateLimitInfo {
    /// Rate limit details for `route`.
    pub fn new(retry_after: Duration, global: bool, route: impl Into<String>) -> Self {
        Self { retry_after, global, bucket: None, route: route.into(), scope: None }
    }
}

/// Which request failed, safe to log: the route has its ids and tokens masked. Boxed inside
/// [`RestError`], which keeps every `Result` under the default `clippy::result_large_err` limit.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RequestSummary {
    /// The method.
    pub method: Method,
    /// The route, masked by [`mask_route`].
    pub route: String,
    /// The last status seen.
    pub status: Option<u16>,
    /// How many attempts were made.
    pub attempts: u32,
}

impl RequestSummary {
    /// A summary of a request.
    pub fn new(method: Method, route: impl Into<String>, status: Option<u16>, attempts: u32) -> Self {
        Self { method, route: route.into(), status, attempts }
    }
}

/// Why a REST call failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RestError {
    /// Discord answered with a JSON error.
    #[error("{status} {code:?}: {message}")]
    Api {
        /// HTTP status.
        status: u16,
        /// Discord's error code, when sent.
        code: Option<JsonErrorCode>,
        /// The OAuth2 `error`, when sent.
        oauth_error: Option<String>,
        /// Discord's message.
        message: String,
        /// The flattened form errors.
        errors: Vec<FieldError>,
        /// Which request.
        request: Box<RequestSummary>,
    },
    /// An error status with a body that is not JSON, such as a Cloudflare page.
    #[error("{status}: unparsable error body")]
    Unparsed {
        /// HTTP status.
        status: u16,
        /// The start of the body.
        body_preview: String,
        /// Which request.
        request: Box<RequestSummary>,
    },
    /// An error status with no body.
    #[error("{status}: empty error body")]
    Http {
        /// HTTP status.
        status: u16,
        /// Which request.
        request: Box<RequestSummary>,
    },
    /// A 429 this request does not wait out, or too many with no progress.
    #[error("rate limited, retry after {:?}", .0.retry_after)]
    RateLimited(RateLimitInfo),
    /// The limiter queue is full.
    #[error("rate limit queue is full")]
    QueueFull,
    /// Too many invalid requests in the window: more would risk an IP ban.
    #[error("invalid request breaker is open")]
    CircuitOpen,
    /// The attempt or the deadline ran out.
    #[error("request timed out")]
    Timeout,
    /// The client shut down while this waited.
    #[error("request cancelled")]
    Cancelled,
    /// The transport failed.
    #[error(transparent)]
    Network(#[from] TransportError),
    /// A success or an error body did not decode.
    #[error("{status}: could not decode body at `{path}`: {source}")]
    Decode {
        /// HTTP status.
        status: u16,
        /// Path of the failing field.
        path: String,
        /// The serde_json error.
        #[source]
        source: serde_json::Error,
        /// The start of the body.
        body_preview: String,
    },
    /// A builder rejected its content before anything was sent.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// A body could not be encoded.
    #[error("body encode failed: {0}")]
    Encode(#[source] serde_json::Error),
    /// The request needs a token and the client has none.
    #[error("no token set")]
    MissingToken,
    /// An earlier 401 invalidated the token, so nothing more is sent with it.
    #[error("the token was invalidated by a 401")]
    TokenInvalidated,
}

impl RestError {
    /// Discord's error code, for `Api` errors that have one.
    pub fn code(&self) -> Option<JsonErrorCode> {
        match self {
            Self::Api { code, .. } => *code,
            _ => None,
        }
    }

    /// True when the request reached Discord and was applied although this call returned an error:
    /// a 2xx with a bad body. Do not repeat the send.
    pub fn request_succeeded(&self) -> bool {
        matches!(self, Self::Decode { status, .. } if (200..300).contains(status))
    }

    /// True when trying again later may work.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Api { status, .. } | Self::Unparsed { status, .. } | Self::Http { status, .. } => {
                *status >= 500 || *status == 429
            }
            Self::RateLimited(_) | Self::QueueFull | Self::Timeout | Self::Network(_) => true,
            _ => false,
        }
    }

    /// True when the failure is a rate limit.
    pub fn is_rate_limit(&self) -> bool {
        match self {
            Self::RateLimited(_) => true,
            Self::Api { status, .. } | Self::Unparsed { status, .. } | Self::Http { status, .. } => *status == 429,
            _ => false,
        }
    }
}

impl From<BodyError> for RestError {
    fn from(e: BodyError) -> Self {
        match e {
            BodyError::Validation(v) => Self::Validation(v),
            BodyError::Encode(e) => Self::Encode(e),
            other => Self::Encode(<serde_json::Error as serde::de::Error>::custom(other)),
        }
    }
}

/// Maps a non-2xx response to an error: JSON with `code` or `message` to `Api`, an empty body to
/// `Http`, anything else to `Unparsed`. Never a `Decode`: a bad error body must not hide the status.
pub fn from_response(status: u16, headers: &[(String, String)], body: &[u8], request: RequestSummary) -> RestError {
    let _ = headers;
    if body.is_empty() {
        return RestError::Http { status, request: Box::new(request) };
    }
    let unparsed = |request| RestError::Unparsed {
        status,
        body_preview: String::from_utf8_lossy(&body[..body.len().min(512)]).into_owned(),
        request: Box::new(request),
    };
    let Ok(wire) = serde_json::from_slice::<RestErrorBody>(body) else {
        return unparsed(request);
    };
    if wire.code.is_none() && wire.message.is_empty() && wire.error.is_none() {
        return unparsed(request);
    }
    let errors = wire
        .errors
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).ok())
        .map_or_else(Vec::new, |tree| flatten_errors(&tree));
    let message = if wire.message.is_empty() {
        wire.error_description.clone().unwrap_or_else(|| String::from("No Description"))
    } else {
        wire.message
    };
    RestError::Api { status, code: wire.code, oauth_error: wire.error, message, errors, request: Box::new(request) }
}

/// Flattens the `errors` tree of a form body error into paths. See `[rest 10]` and upstream's
/// `flattenDiscordError`. Bad shapes give an empty list, never a failure.
pub fn flatten_errors(errors: &serde_json::Value) -> Vec<FieldError> {
    let mut out = Vec::new();
    flatten_into(errors, "", &mut out);
    out
}

fn flatten_into(node: &serde_json::Value, key: &str, out: &mut Vec<FieldError>) {
    let serde_json::Value::Object(fields) = node else { return };
    if let Some(message) = fields.get("message").and_then(serde_json::Value::as_str) {
        let code = fields.get("code").and_then(serde_json::Value::as_str).unwrap_or_default();
        out.push(FieldError::new(key, code, message));
        return;
    }
    for (name, value) in fields {
        let next = if name.starts_with('_') {
            key.to_owned()
        } else if key.is_empty() {
            name.clone()
        } else if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
            format!("{key}[{name}]")
        } else {
            format!("{key}.{name}")
        };
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    flatten_into(item, &next, out);
                }
            }
            other => flatten_into(other, &next, out),
        }
    }
}

/// Masks a path for logs and errors: an interaction or webhook token becomes `:token`, and anything
/// after a query mark is dropped. Ids are not secret and stay. Also accepts a `METHOD:/path` key.
pub fn mask_route(path: &str) -> String {
    let path = path.split('?').next().unwrap_or_default();
    let (prefix, rest) = match path.split_once(':') {
        Some((verb, tail)) if !verb.is_empty() && verb.bytes().all(|b| b.is_ascii_uppercase()) => (Some(verb), tail),
        _ => (None, path),
    };
    let mut segments: Vec<&str> = rest.split('/').collect();
    if matches!(segments.get(1), Some(&"interactions" | &"webhooks"))
        && let Some(slot) = segments.get_mut(3)
    {
        *slot = ":token";
    }
    let joined = segments.join("/");
    prefix.map_or_else(|| joined.clone(), |verb| format!("{verb}:{joined}"))
}
