//! Tri-state field: absent, explicit null, value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A field where absent and `null` mean different things, such as `referenced_message` or a PATCH
/// body. Use it with `#[serde(default, skip_serializing_if = "Patch::is_absent")]`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Patch<T> {
    /// Not on the wire.
    #[default]
    Absent,
    /// Present as `null`.
    Null,
    /// Present with a value.
    Value(T),
}

impl<T> Patch<T> {
    /// True when the field is not on the wire.
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// True when the field is present as `null`.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The value, if there is one.
    pub const fn as_option(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Consumes the patch and returns the value, if there is one.
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    /// Maps the value and keeps the state.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Patch<U> {
        match self {
            Self::Absent => Patch::Absent,
            Self::Null => Patch::Null,
            Self::Value(v) => Patch::Value(f(v)),
        }
    }
}

impl<T> From<T> for Patch<T> {
    fn from(v: T) -> Self {
        Self::Value(v)
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(v) => v.serialize(s),
            Self::Absent | Self::Null => s.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Option::<T>::deserialize(d)?.map_or(Self::Null, Self::Value))
    }
}
