//! The four binding macros, `ARCHITECTURE.md` 7.4 and 7.6. `$crate::__private` keeps user crates
//! free of direct dependencies, except `tagged_union!`, whose private tag wrapper uses the serde
//! derive of the calling crate.
//!
//! Each macro emits a doc line for every generated const, variant and method, so an entry needs a
//! doc comment only when it has something to add. The type itself must carry one.

/// An integer enum that keeps unknown values: a struct over the integer with associated consts.
///
/// Consts work as patterns, including aliases. `Debug` prints `ChannelType::GUILD_TEXT` or
/// `ChannelType(99)`. Duplicates must be written as `alias`, or `name()` has an unreachable arm.
#[macro_export]
macro_rules! open_int {
    (
        $(#[$meta:meta])*
        $name:ident : $repr:ty {
            $( $(#[$cm:meta])* $k:ident = $v:expr ),* $(,)?
            $( ; $( $(#[$am:meta])* alias $a:ident = $t:ident ; )* )?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct $name(pub $repr);
        impl $name {
            $(
                $(#[$cm])*
                #[doc = concat!("Wire value `", stringify!($v), "`.")]
                pub const $k: Self = Self($v);
            )*
            $( $(
                $(#[$am])*
                #[doc = concat!("Alias of [`Self::", stringify!($t), "`].")]
                pub const $a: Self = Self::$t;
            )* )?
            /// Canonical constant name, or `None` for a value this build does not know.
            pub const fn name(self) -> ::core::option::Option<&'static str> {
                match self { $( Self::$k => ::core::option::Option::Some(stringify!($k)), )* _ => ::core::option::Option::None }
            }
            /// The raw wire value.
            pub const fn get(self) -> $repr { self.0 }
        }
        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.name() {
                    ::core::option::Option::Some(n) => write!(f, concat!(stringify!($name), "::{}"), n),
                    ::core::option::Option::None => write!(f, concat!(stringify!($name), "({})"), self.0),
                }
            }
        }
        impl From<$repr> for $name { fn from(v: $repr) -> Self { Self(v) } }
        impl From<$name> for $repr { fn from(v: $name) -> $repr { v.0 } }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                $crate::__private::serde::Serialize::serialize(&self.0, s)
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                <$repr as $crate::__private::serde::Deserialize>::deserialize(d).map(Self)
            }
        }
    };
}

/// A value the build does not recognise. Only serde and `From<&str>` can construct one, so
/// `Unknown` can never hold a known string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Unrecognized(compact_str::CompactString);
impl Unrecognized {
    #[doc(hidden)]
    pub fn __new(s: compact_str::CompactString) -> Self {
        Self(s)
    }
    /// The text as received.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A string enum that keeps unknown values in an `Unknown` variant. Reads go through
/// `CompactString`, so an escaped `"NEWS"` still becomes the known variant.
#[macro_export]
macro_rules! open_str {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vm:meta])* $v:ident = $s:literal ),* $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $(
                $(#[$vm])*
                #[doc = concat!("Wire value `", $s, "`.")]
                $v,
            )*
            /// A value this build does not know.
            Unknown($crate::macros::Unrecognized),
        }
        impl $name {
            /// The wire text.
            pub fn as_str(&self) -> &str {
                match self { $( Self::$v => $s, )* Self::Unknown(u) => u.as_str() }
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                match s { $( $s => Self::$v, )* other => Self::Unknown($crate::macros::Unrecognized::__new(other.into())) }
            }
        }
        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { f.write_str(self.as_str()) }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                let s = <$crate::__private::compact_str::CompactString as $crate::__private::serde::Deserialize>::deserialize(d)?;
                ::core::result::Result::Ok(match s.as_str() {
                    $( $s => Self::$v, )*
                    _ => Self::Unknown($crate::macros::Unrecognized::__new(s)),
                })
            }
        }
    };
}

/// A `u64` flag set that keeps every unknown bit. `number` or `string` sets the write form. Reads
/// always accept both, because the same field has been seen in both forms.
#[macro_export]
macro_rules! open_flags {
    (
        $(#[$meta:meta])*
        $name:ident : $form:ident { $( $(#[$($fm:tt)*])* $f:ident = $v:expr ),* $(,)? }
    ) => {
        $crate::__private::bitflags::bitflags! {
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct $name: u64 {
                $(
                    $(#[$($fm)*])*
                    #[doc = concat!("Bit `", stringify!($v), "`.")]
                    const $f = $v;
                )*
                const _ = !0;
            }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                $crate::open_flags!(@write $form, self, s)
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                $crate::wire::u64_string_or_int(d).map(Self::from_bits_retain)
            }
        }
    };
    (@write number, $self:ident, $s:ident) => { $s.serialize_u64($self.bits()) };
    (@write string, $self:ident, $s:ident) => { $s.collect_str(&$self.bits()) };
}

/// An integer-tagged union. Captures the raw text, peeks the tag as `u32`, then parses the arm. A
/// tag may appear anywhere in the object. An unknown tag becomes `Unknown { kind, raw }`, which
/// re-serializes byte-exact. A missing tag or a bad arm is an error the enclosing decode reports.
///
/// Never place one under `serde(flatten)` on the receive path: it fails at runtime.
#[macro_export]
macro_rules! tagged_union {
    (
        #[tag = $tag:literal]
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$am:meta])* $n:literal => $v:ident($t:ty), )*
            _ => Unknown,
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        pub enum $name {
            $(
                $(#[$am])*
                #[doc = concat!("Tag `", stringify!($n), "`.")]
                $v($t),
            )*
            /// A tag this build does not know. Re-serializes byte-exact.
            Unknown {
                /// The tag as received.
                kind: u32,
                /// The whole object as received.
                raw: ::std::boxed::Box<$crate::__private::serde_json::value::RawValue>,
            },
        }
        impl $name {
            /// The wire tag of this value.
            pub fn kind(&self) -> u32 { match self { $( Self::$v(_) => $n, )* Self::Unknown { kind, .. } => *kind } }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                use $crate::__private::serde::de::Error as _;
                let raw = <::std::boxed::Box<$crate::__private::serde_json::value::RawValue> as $crate::__private::serde::Deserialize>::deserialize(d)?;
                let kind = $crate::wire::peek_tag(raw.get(), $tag).map_err(D::Error::custom)?;
                ::core::result::Result::Ok(match kind {
                    $( $n => Self::$v($crate::wire::parse_arm(raw.get()).map_err(D::Error::custom)?), )*
                    _ => Self::Unknown { kind, raw },
                })
            }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                #[derive(::serde::Serialize)]
                struct Tagged<'a, T> { #[serde(rename = $tag)] tag: u32, #[serde(flatten)] inner: &'a T }
                match self {
                    $( Self::$v(inner) => $crate::__private::serde::Serialize::serialize(&Tagged { tag: $n, inner }, s), )*
                    Self::Unknown { raw, .. } => $crate::__private::serde::Serialize::serialize(raw, s),
                }
            }
        }
    };
}
