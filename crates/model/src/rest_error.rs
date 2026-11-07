//! The JSON body of a REST error, `[rest 10]`. `dr_rest` flattens `errors` into `FieldError`s.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu), `RESTError` and `RESTRateLimit`
//! in `payloads/common.ts`, plus the OAuth2 shape discord.js reads in
//! `packages/rest/src/lib/errors/DiscordAPIError.ts` (Apache-2.0).
//! Changed: one tolerant struct for all the shapes, rather than a union.

use crate::JsonErrorCode;
use dr_core::null_default;
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;

/// Reads `code` so that a shape this build does not expect costs the code and not the whole body:
/// an error body exists to explain a failure, and must not hide the rest of what it says.
fn lenient_code<'de, D: Deserializer<'de>>(d: D) -> Result<Option<JsonErrorCode>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Code(JsonErrorCode),
        Other(IgnoredAny),
    }
    Ok(match Wire::deserialize(d)? {
        Wire::Code(code) => Some(code),
        Wire::Other(_) => None,
    })
}

/// Body of a non-2xx JSON response. Every field defaults, because Cloudflare and OAuth errors
/// carry only part of this. Upstream `RESTError`, with the OAuth2 and 429 fields added.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RestErrorBody {
    /// JSON error code. Code 0 is real, so absence is `None`. A value that is not a `u32` also reads
    /// as `None`.
    #[serde(default, deserialize_with = "lenient_code", skip_serializing_if = "Option::is_none")]
    pub code: Option<JsonErrorCode>,
    /// Human readable message.
    #[serde(default, deserialize_with = "null_default")]
    pub message: String,
    /// The nested per-field errors, kept raw for flattening.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<Box<RawValue>>,
    /// OAuth2 error string, instead of `code` on OAuth errors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// OAuth2 error description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    /// Seconds to wait, on a 429 body. May be fractional. The limiter reads the header instead,
    /// `ARCHITECTURE.md` 10.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<f64>,
    /// True when a 429 is global.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "std::ops::Not::not")]
    pub global: bool,
}

/// Body of a 429 response. Unlike [`RestErrorBody`], every field Discord always sends is always
/// written, so this is the form to send when imitating one. Upstream `RESTRateLimit`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct RestRateLimit {
    /// An error code, present on some limits only.
    #[serde(default, deserialize_with = "lenient_code", skip_serializing_if = "Option::is_none")]
    pub code: Option<JsonErrorCode>,
    /// True when the limit is the global one.
    #[serde(default, deserialize_with = "null_default")]
    pub global: bool,
    /// Human readable message.
    #[serde(default, deserialize_with = "null_default")]
    pub message: String,
    /// Seconds to wait. May be fractional.
    #[serde(default, deserialize_with = "null_default")]
    pub retry_after: f64,
}
