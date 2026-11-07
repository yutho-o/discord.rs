//! Encoding of paths, queries and the audit log reason, and the shape of a request on the wire.

use crate::request::{Auth, Method, RequestParts};
use crate::transport::PreparedRequest;
use crate::{RestConfig, RestError};
use dr_core::BodyError;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;

/// What `encodeURIComponent` leaves alone: `A-Za-z0-9-_.!~*'()`.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// Percent-encodes an audit log reason for the `X-Audit-Log-Reason` header. The result is safe in a
/// header whatever the input holds, including new lines and non-ASCII text.
pub fn encode_reason(reason: &str) -> String {
    utf8_percent_encode(reason, URI_COMPONENT).to_string()
}

/// Percent-encodes one path segment, for values a caller supplies, such as a reaction emoji or an
/// interaction token. `/`, `?`, `#` and `%` must not survive.
pub fn encode_path_segment(segment: &str) -> Cow<'_, str> {
    utf8_percent_encode(segment, URI_COMPONENT).into()
}

/// Encodes a serializable struct as a query string, without the `?`. `Option::None` fields are left
/// out. Sequences repeat the key. An empty result means no query.
pub fn encode_query<Q: Serialize>(query: &Q) -> Result<String, BodyError> {
    let value = serde_json::to_value(query)?;
    let mut out = form_urlencoded::Serializer::new(String::new());
    if let Value::Object(fields) = value {
        for (key, field) in fields {
            match field {
                Value::Array(items) => {
                    for item in items {
                        if let Some(text) = scalar(item) {
                            out.append_pair(&key, &text);
                        }
                    }
                }
                other => {
                    if let Some(text) = scalar(other) {
                        out.append_pair(&key, &text);
                    }
                }
            }
        }
    }
    Ok(out.finish())
}

/// A query value with no JSON quotes. `null` is left out.
fn scalar(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

/// Builds what the transport sends for one attempt, with the headers in the order of
/// `ARCHITECTURE.md` 10.2: `Content-Type` when there is a body, `User-Agent`, `Authorization` by
/// [`Auth`], then `X-Audit-Log-Reason`.
///
/// A `GET` drops its body, and an empty reason is not sent.
pub fn prepare(config: &RestConfig, parts: &RequestParts) -> Result<PreparedRequest, RestError> {
    let mut url = format!("{}{}", config.api_base, parts.path);
    if let Some(q) = &parts.query {
        url.push('?');
        url.push_str(q);
    }
    let body = if parts.method == Method::Get { None } else { parts.body.json.clone() };
    let mut headers = Vec::with_capacity(4);
    if body.is_some() {
        headers.push(("Content-Type", String::from("application/json")));
    }
    headers.push(("User-Agent", config.user_agent.clone()));
    match &parts.options.auth {
        Auth::Default => {
            let token = config.token.as_ref().ok_or(RestError::MissingToken)?;
            // A token given with its prefix must not end up with two.
            let bare = token.expose().strip_prefix("Bot ").unwrap_or(token.expose());
            headers.push(("Authorization", format!("Bot {bare}")));
        }
        Auth::None => {}
        Auth::Custom(value) => headers.push(("Authorization", value.expose().to_owned())),
    }
    if let Some(r) = parts.reason.as_ref().filter(|r| !r.is_empty()) {
        headers.push(("X-Audit-Log-Reason", encode_reason(r)));
    }
    Ok(PreparedRequest { method: parts.method, url, headers, body: body.unwrap_or_default(), timeout: config.timeout })
}
