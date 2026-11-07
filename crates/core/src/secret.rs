//! A string that stays out of logs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A token, session id or other secret. `Debug` prints `Secret(***)` and `Display` prints `***`.
///
/// `Serialize` writes the real value, since this type is what goes into an identify payload. Only
/// put it in a struct that is sent to Discord, never in one that is logged as JSON.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(Box<str>);

impl Secret {
    /// Wraps a value.
    pub fn new(s: impl Into<Box<str>>) -> Self {
        Self(s.into())
    }

    /// The real value. Call this only where it goes on the wire.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}
impl From<String> for Secret {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}
impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}
impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Box::<str>::deserialize(d).map(Self)
    }
}
