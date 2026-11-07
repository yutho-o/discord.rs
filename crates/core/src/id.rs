//! Typed snowflakes, `ARCHITECTURE.md` 7.5.

use serde::de::{Deserializer, Error as DeError, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

/// Discord's epoch, the first millisecond of 2015, in milliseconds since the Unix epoch.
pub const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

/// A snowflake typed by the resource it names, so that passing a guild id where a channel id
/// belongs does not compile.
///
/// Size 8. Reads a string or an integer, including as a JSON map key. Writes a string. `"0"` is
/// valid, so that a zero from Discord cannot fail a payload. The trait impls are by hand, because
/// a `derive` would add an `M: Trait` bound and markers are uninhabited.
#[repr(transparent)]
pub struct Id<M>(u64, PhantomData<fn() -> M>);

/// Marker types for [`Id`], one per Discord resource.
pub mod marker {
    macro_rules! markers {
        ($($m:ident),* $(,)?) => {
            $(
                #[doc = concat!("Marks an [`Id`](super::Id) as naming a resource of kind `", stringify!($m), "`.")]
                #[derive(Debug)]
                pub enum $m {}
            )*
        };
    }
    markers!(
        UserMarker,
        GuildMarker,
        ChannelMarker,
        MessageMarker,
        RoleMarker,
        EmojiMarker,
        AttachmentMarker,
        WebhookMarker,
        ApplicationMarker,
        CommandMarker,
        InteractionMarker,
        IntegrationMarker,
        StickerMarker,
        StickerPackMarker,
        ScheduledEventMarker,
        AutoModRuleMarker,
        AuditLogEntryMarker,
        StageInstanceMarker,
        SkuMarker,
        EntitlementMarker,
        SubscriptionMarker,
        GenericMarker,
    );
}

impl<M> Id<M> {
    /// Wraps a raw snowflake.
    pub const fn new(raw: u64) -> Self {
        Self(raw, PhantomData)
    }

    /// The raw snowflake.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Reinterprets the id as another kind. For id-equals-id cases, such as the `@everyone` role
    /// having the id of its guild.
    pub const fn cast<N>(self) -> Id<N> {
        Id(self.0, PhantomData)
    }

    /// Creation time encoded in the snowflake, in milliseconds since the Unix epoch.
    #[allow(clippy::cast_possible_wrap)]
    pub const fn timestamp_ms(self) -> i64 {
        ((self.0 >> 22) as i64) + DISCORD_EPOCH_MS
    }
}

impl<M> Clone for Id<M> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M> Copy for Id<M> {}
impl<M> PartialEq for Id<M> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<M> Eq for Id<M> {}
impl<M> Hash for Id<M> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}
impl<M> PartialOrd for Id<M> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<M> Ord for Id<M> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.cmp(&o.0)
    }
}
impl<M> fmt::Debug for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}
impl<M> fmt::Display for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl<M> From<u64> for Id<M> {
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}
impl<M> From<Id<M>> for u64 {
    fn from(v: Id<M>) -> u64 {
        v.0
    }
}
impl<M> FromStr for Id<M> {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self::new)
    }
}
impl<M> Serialize for Id<M> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}
impl<'de, M> Deserialize<'de> for Id<M> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<M>(PhantomData<fn() -> M>);
        impl<M> Visitor<'_> for V<M> {
            type Value = Id<M>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a snowflake as a string or integer")
            }
            fn visit_str<E: DeError>(self, v: &str) -> Result<Id<M>, E> {
                v.parse().map(Id::new).map_err(E::custom)
            }
            fn visit_u64<E: DeError>(self, v: u64) -> Result<Id<M>, E> {
                Ok(Id::new(v))
            }
            fn visit_i64<E: DeError>(self, v: i64) -> Result<Id<M>, E> {
                u64::try_from(v).map(Id::new).map_err(E::custom)
            }
        }
        d.deserialize_any(V(PhantomData))
    }
}
