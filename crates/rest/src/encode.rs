//! Encoding of paths, queries and the audit log reason, and the shape of a request on the wire.

use crate::request::{Auth, Method, RequestParts};
use crate::transport::PreparedRequest;
use crate::{RestConfig, RestError};
use bytes::{BufMut, Bytes, BytesMut};
use dr_core::{BodyError, RequestBody};
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

/// The `Content-Type` and the bytes of a body. JSON alone goes as `application/json`. With files, it
/// goes as `multipart/form-data`: the JSON as the part `payload_json`, then file `i` of
/// [`RequestBody::files`] as `files[i]`.
pub fn encode_body(body: &RequestBody) -> Option<(String, Bytes)> {
    if body.files.is_empty() && body.fields.is_empty() {
        return body.json.clone().map(|json| (String::from("application/json"), json));
    }
    let boundary = pick_boundary(body);
    let mut out = BytesMut::new();
    if let Some(json) = &body.json {
        part_header(&mut out, &boundary, "payload_json", None, "application/json");
        out.put_slice(json);
        out.put_slice(CRLF);
    }
    for (name, value) in &body.fields {
        part_header(&mut out, &boundary, &escape_filename(name), None, "text/plain; charset=utf-8");
        out.put_slice(value.as_bytes());
        out.put_slice(CRLF);
    }
    for (i, file) in body.files.iter().enumerate() {
        let media = file.content_type.as_deref().filter(|t| is_media_type(t)).unwrap_or("application/octet-stream");
        let field = file.field.as_deref().map_or_else(|| format!("files[{i}]"), escape_filename);
        part_header(&mut out, &boundary, &field, Some(&file.name), media);
        out.put_slice(&file.data);
        out.put_slice(CRLF);
    }
    out.put_slice(format!("--{boundary}--").as_bytes());
    out.put_slice(CRLF);
    Some((format!("multipart/form-data; boundary={boundary}"), out.freeze()))
}

const CRLF: &[u8] = b"\r\n";

/// A boundary that appears in no part, so no content can end a part early.
fn pick_boundary(body: &RequestBody) -> String {
    let fields = body.fields.iter().map(|(_, v)| v.as_bytes());
    let parts: Vec<&[u8]> =
        body.json.iter().map(|j| &j[..]).chain(fields).chain(body.files.iter().map(|f| &f.data[..])).collect();
    let mut seed = parts
        .iter()
        .fold(0x9e37_79b9_7f4a_7c15_u64, |h, p| h.rotate_left(5) ^ u64::try_from(p.len()).unwrap_or(u64::MAX));
    loop {
        let boundary = format!("discord-rs-{seed:016x}");
        let needle = boundary.as_bytes();
        if !parts.iter().any(|p| p.windows(needle.len()).any(|w| w == needle)) {
            return boundary;
        }
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    }
}

fn part_header(out: &mut BytesMut, boundary: &str, name: &str, filename: Option<&str>, media: &str) {
    out.put_slice(format!("--{boundary}").as_bytes());
    out.put_slice(CRLF);
    out.put_slice(format!("Content-Disposition: form-data; name=\"{name}\"").as_bytes());
    if let Some(filename) = filename {
        out.put_slice(format!("; filename=\"{}\"", escape_filename(filename)).as_bytes());
    }
    out.put_slice(CRLF);
    out.put_slice(format!("Content-Type: {media}").as_bytes());
    out.put_slice(CRLF);
    out.put_slice(CRLF);
}

/// Escapes a file name the way browsers do in `multipart/form-data`, so that it cannot close the
/// quoted value or start a new header line.
fn escape_filename(name: &str) -> String {
    name.replace('"', "%22").replace('\r', "%0D").replace('\n', "%0A")
}

/// True for `type/subtype` with optional parameters, in printable ASCII with no new line.
fn is_media_type(value: &str) -> bool {
    value.contains('/') && value.bytes().all(|b| (0x20..0x7f).contains(&b))
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
    let body = if parts.method == Method::Get { None } else { encode_body(&parts.body) };
    let mut headers = Vec::with_capacity(4);
    let body = body.map(|(content_type, bytes)| {
        headers.push(("Content-Type", content_type));
        bytes
    });
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
