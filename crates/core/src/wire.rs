//! Decode entry, serde field helpers and request bodies, `ARCHITECTURE.md` 7.4 and 8.1.

use bytes::Bytes;
use serde::de::{DeserializeOwned, Deserializer, Error as DeError, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fmt;

/// The boxed error type of user handlers and of the seams between crates.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---- union support --------------------------------------------------------------------

/// Peeks an integer tag at the top level of `raw` without parsing the other fields. Always `u32`:
/// a narrower type would reject the very payload it is meant to tolerate.
pub fn peek_tag(raw: &str, key: &'static str) -> Result<u32, serde_json::Error> {
    struct V(&'static str);
    impl<'de> Visitor<'de> for V {
        type Value = u32;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "object with integer `{}`", self.0)
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<u32, A::Error> {
            let mut found = None;
            while let Some(k) = m.next_key::<std::borrow::Cow<'de, str>>()? {
                if k == self.0 {
                    found = Some(m.next_value::<u32>()?);
                } else {
                    m.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            found.ok_or_else(|| A::Error::missing_field(self.0))
        }
    }
    let mut de = serde_json::Deserializer::from_str(raw);
    de.deserialize_map(V(key))
}

/// Parses one arm of a `tagged_union!`. Exists so that the macro needs no serde_json at its site.
pub fn parse_arm<T: DeserializeOwned>(raw: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(raw)
}

// ---- field helpers --------------------------------------------------------------------

/// Reads `null` as the default value, for a list Discord declares as non-nullable and sends as
/// `null`. Use with `#[serde(default, deserialize_with = "null_default")]`.
pub fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Reads an integer that may arrive as `1.0`, which serde_json refuses for a plain `i64`.
pub fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a number")
        }
        fn visit_i64<E: DeError>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }
        fn visit_u64<E: DeError>(self, v: u64) -> Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }
        #[allow(clippy::cast_possible_truncation)]
        fn visit_f64<E: DeError>(self, v: f64) -> Result<i64, E> {
            if v.fract() == 0.0 && v.abs() < 9.0e15 { Ok(v as i64) } else { Err(E::custom("not an integral number")) }
        }
    }
    d.deserialize_any(V)
}

/// Reads a `u64` written as an integer or a decimal string. The read side of `open_flags!`.
pub fn u64_string_or_int<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an integer or decimal string")
        }
        fn visit_str<E: DeError>(self, v: &str) -> Result<u64, E> {
            v.parse().map_err(E::custom)
        }
        fn visit_u64<E: DeError>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_i64<E: DeError>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(E::custom)
        }
    }
    d.deserialize_any(V)
}

/// A list where one bad element does not lose the others.
#[derive(Clone, Debug, Default)]
pub struct Lenient<T> {
    /// The elements that decoded.
    pub items: Vec<T>,
    /// How many elements failed to decode.
    pub dropped: u32,
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raws = Vec::<Box<RawValue>>::deserialize(d)?;
        let mut out = Self { items: Vec::with_capacity(raws.len()), dropped: 0 };
        for r in raws {
            match serde_json::from_str(r.get()) {
                Ok(v) => out.items.push(v),
                Err(_) => out.dropped += 1,
            }
        }
        Ok(out)
    }
}

// ---- decode with a path only on failure -------------------------------------------------

/// A typed decode failed. `path` says where, as far as a path can pass through a union.
#[derive(Debug, thiserror::Error)]
#[error("decode failed at `{path}`: {source}")]
pub struct DecodeError {
    /// Dotted path of the failing field, `.` if none could be found.
    pub path: String,
    /// The serde_json error.
    #[source]
    pub source: serde_json::Error,
}

/// The one decode entry. The hot path has no tracking cost. On failure the same text is decoded
/// again to find the path.
pub fn decode<T: DeserializeOwned>(text: &str) -> Result<T, DecodeError> {
    match serde_json::from_str(text) {
        Ok(v) => Ok(v),
        Err(first) => {
            let mut de = serde_json::Deserializer::from_str(text);
            match serde_path_to_error::deserialize::<_, T>(&mut de) {
                Err(e) => {
                    let path = e.path().to_string();
                    Err(DecodeError { path, source: e.into_inner() })
                }
                Ok(_) => Err(DecodeError { path: String::from("."), source: first }),
            }
        }
    }
}

// ---- request bodies --------------------------------------------------------------------

/// A builder rejected its content before anything was sent.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {message}")]
pub struct ValidationError {
    /// Path of the bad field, such as `embeds[0].title`.
    pub path: String,
    /// What is wrong with it, lowercase with no trailing period.
    pub message: String,
}

impl ValidationError {
    /// A validation error at `path`.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self { path: path.into(), message: message.into() }
    }
}

/// Why a value could not become a request body.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BodyError {
    /// A builder rejected its content.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// JSON encoding failed.
    #[error(transparent)]
    Encode(#[from] serde_json::Error),
}

/// One file of a request. The REST client sends it as the part `files[index]` of a
/// `multipart/form-data` body, where `index` is its place in [`RequestBody::files`].
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileUpload {
    /// The file name Discord shows, and that `attachment://` URLs in embeds refer to.
    pub name: String,
    /// The media type. `None` sends `application/octet-stream`, and Discord guesses from the name.
    pub content_type: Option<String>,
    /// The content.
    pub data: Bytes,
}

impl FileUpload {
    /// A file named `name` holding `data`.
    pub fn new(name: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self { name: name.into(), content_type: None, data: data.into() }
    }

    /// Sets the media type, such as `image/png`.
    pub fn content_type(mut self, value: impl Into<String>) -> Self {
        self.content_type = Some(value.into());
        self
    }
}

/// Shows the name and the size, not the content.
impl std::fmt::Debug for FileUpload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileUpload")
            .field("name", &self.name)
            .field("content_type", &self.content_type)
            .field("len", &self.data.len())
            .finish()
    }
}

/// The encoded body of a request. An empty one sends no body.
///
/// With files, the REST client sends `multipart/form-data`: the JSON as `payload_json`, then each
/// file. Build one with [`RequestBody::json`] or [`RequestBody::empty`], then
/// [`RequestBody::with_files`].
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct RequestBody {
    /// JSON text, if any.
    pub json: Option<Bytes>,
    /// Files, sent in this order.
    pub files: Vec<FileUpload>,
}

impl RequestBody {
    /// A request with no body.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Encodes `v` as JSON.
    pub fn json<T: Serialize + ?Sized>(v: &T) -> Result<Self, BodyError> {
        Ok(Self { json: Some(Bytes::from(serde_json::to_vec(v)?)), files: Vec::new() })
    }

    /// Adds files after those already there.
    pub fn with_files(mut self, files: impl IntoIterator<Item = FileUpload>) -> Self {
        self.files.extend(files);
        self
    }
}

/// How a builder or wire struct becomes bytes. `dr_rest` knows this trait and not `dr_builders`.
/// Validation runs here, so a failure surfaces at `.await` with nothing sent.
pub trait IntoBody {
    /// Validates and encodes the value.
    fn into_body(self) -> Result<RequestBody, BodyError>;
}

/// A body with files added after it is encoded. An interaction callback uses it: the message inside
/// knows its files, and the wrapper around it does not.
#[derive(Debug)]
pub struct WithFiles<B> {
    body: B,
    files: Vec<FileUpload>,
}

impl<B: IntoBody> WithFiles<B> {
    /// `body`, then `files`.
    pub fn new(body: B, files: Vec<FileUpload>) -> Self {
        Self { body, files }
    }
}

impl<B: IntoBody> IntoBody for WithFiles<B> {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        Ok(self.body.into_body()?.with_files(self.files))
    }
}

impl IntoBody for RequestBody {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        Ok(self)
    }
}

impl IntoBody for () {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        Ok(RequestBody::default())
    }
}

impl IntoBody for serde_json::Value {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        RequestBody::json(&self)
    }
}
