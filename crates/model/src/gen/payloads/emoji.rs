//! Emoji. Upstream: `payloads/v10/emoji.ts` of discord-api-types (MIT, (c) 2020 vladfrangu).
//!
//! `APIPartialEmoji`, `APIEmoji` and `APIApplicationEmoji` are one struct here: their property sets
//! nest, and every field beyond `id` and `name` is optional on the wire.

use crate::{EmojiId, RoleId, User};
use dr_core::null_default;
use serde::{Deserialize, Serialize};

/// A custom or unicode emoji. Unicode ones have no `id`.
///
/// Writes only what is set: a default field is left out instead of written as `false`, `[]` or
/// `null`. Reads treat a left-out field as its default, so nothing changes on a round trip. A button
/// or select option has the smaller `MessageComponentEmoji`, which converts from this one.
///
/// Helpers such as [`Emoji::identifier`] and the [`Emoji::parse`] constructor are in
/// `message_ext.rs`, apart from the plain data the generator will emit.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Emoji {
    /// Id of a custom emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EmojiId>,
    /// Name, or the unicode character. May be absent on deleted custom emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Roles allowed to use it.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleId>,
    /// Uploader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Whether it needs colons.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_colons: bool,
    /// Whether it is managed by an integration.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub managed: bool,
    /// Whether it is animated.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub animated: bool,
    /// Whether it can be used now, which can stop being true when a guild loses Server Boosts.
    /// Only guild and application emoji send this, so a reaction or button emoji, which leaves it
    /// out, reads as `true`.
    #[serde(default = "default_available", skip_serializing_if = "is_true")]
    pub available: bool,
}

const fn default_available() -> bool {
    true
}

// serde's `skip_serializing_if` passes a reference.
const fn is_true(v: &bool) -> bool {
    *v
}
