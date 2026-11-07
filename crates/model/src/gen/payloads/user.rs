//! Users. Upstream: `payloads/v10/user.ts`, field behaviour from discord.js
//! `packages/discord.js/src/structures/User.js` (Apache-2.0).
//! Changed: translated to Rust; absent versus null kept through `Patch`, `ARCHITECTURE.md` 7.4.
//!
//! Connections and application role connections from the same upstream file belong to OAuth2, which
//! are not ported yet.

use crate::{GenericId, GuildId, UserFlags, UserId};
use dr_core::{Patch, open_str};
use serde::{Deserialize, Serialize};

dr_core::open_int! {
    /// Kinds of Nitro subscription. Unknown values are preserved.
    UserPremiumType: u16 {
        NONE = 0,
        NITRO_CLASSIC = 1,
        NITRO = 2,
        NITRO_BASIC = 3,
    }
}

open_str! {
    /// Background colour of a nameplate. Unknown values are preserved.
    NameplatePalette {
        Berry = "berry",
        BubbleGum = "bubble_gum",
        Clover = "clover",
        Cobalt = "cobalt",
        Crimson = "crimson",
        Forest = "forest",
        Lemon = "lemon",
        Sky = "sky",
        Teal = "teal",
        Violet = "violet",
        White = "white",
    }
}

/// A Discord user.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct User {
    /// User id.
    pub id: UserId,
    /// Username, unique among users. Empty on a partial user, see [`User::is_partial`].
    #[serde(default)]
    pub username: String,
    /// Legacy discriminator, `0` on most users today.
    #[serde(default)]
    pub discriminator: String,
    /// Display name.
    #[serde(default)]
    pub global_name: Option<String>,
    /// Avatar hash.
    #[serde(default)]
    pub avatar: Option<String>,
    /// Whether this is a bot account.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bot: bool,
    /// Whether this is an official system account.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub system: bool,
    /// Whether two factor authentication is on. Needs the OAuth2 `identify` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mfa_enabled: Option<bool>,
    /// Banner hash. Absent means not fetched, null means none: only a forced fetch returns it.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub banner: Patch<String>,
    /// Banner colour as `0xRRGGBB`. Absent means not fetched, null means none.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub accent_color: Patch<u32>,
    /// Chosen language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Whether the email is verified. Needs the OAuth2 `email` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    /// Email. Needs the OAuth2 `email` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Flags on the account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UserFlags>,
    /// Nitro subscription. `0` for an application not approved for the premium scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<crate::UserPremiumType>,
    /// Public flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_flags: Option<UserFlags>,
    /// Avatar decoration hash. Deprecated upstream in favour of `avatar_decoration_data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration: Option<String>,
    /// Avatar decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// Collectibles, excluding avatar decorations and profile effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
    /// Primary guild, the source of the server tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_guild: Option<UserPrimaryGuild>,
}

/// The account of the running bot, from `READY` and `GET /users/@me`. The same shape as [`User`].
pub type CurrentUser = User;

impl User {
    /// A user with only its id known.
    pub fn partial(id: UserId) -> Self {
        Self {
            id,
            username: String::new(),
            discriminator: String::new(),
            global_name: None,
            avatar: None,
            bot: false,
            system: false,
            mfa_enabled: None,
            banner: Patch::Absent,
            accent_color: Patch::Absent,
            locale: None,
            verified: None,
            email: None,
            flags: None,
            premium_type: None,
            public_flags: None,
            avatar_decoration: None,
            avatar_decoration_data: None,
            collectibles: None,
            primary_guild: None,
        }
    }

    /// Whether only part of this user is known, because the payload had no `username`.
    #[doc(alias = "partial")]
    pub fn is_partial(&self) -> bool {
        self.username.is_empty()
    }

    /// The display name, or the username when there is none. An empty display name is returned as
    /// is, as upstream's `??` does.
    #[doc(alias = "displayName")]
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }

    /// The username, or `name#1234` for a legacy discriminator. `None` on a partial user.
    pub fn tag(&self) -> Option<String> {
        if self.is_partial() {
            None
        } else if matches!(self.discriminator.as_str(), "0" | "0000") {
            Some(self.username.clone())
        } else {
            Some(format!("{}#{}", self.username, self.discriminator))
        }
    }
}

/// An avatar decoration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AvatarDecorationData {
    /// Decoration hash.
    #[serde(default)]
    pub asset: String,
    /// Id of the decoration's SKU.
    pub sku_id: GenericId,
}

/// The collectibles a user has, excluding avatar decorations and profile effects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Collectibles {
    /// Nameplate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameplate: Option<NameplateData>,
}

/// A nameplate.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct NameplateData {
    /// Id of the nameplate's SKU.
    pub sku_id: GenericId,
    /// Path to the asset, such as `nameplates/nameplates/twilight/`.
    #[serde(default)]
    pub asset: String,
    /// Label. Unused by Discord at present.
    #[serde(default)]
    pub label: String,
    /// Background colour.
    #[serde(default)]
    pub palette: Option<NameplatePalette>,
}

/// A user's primary guild. Every field is nullable, because the system can clear the identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UserPrimaryGuild {
    /// Id of the guild.
    #[serde(default)]
    pub identity_guild_id: Option<GuildId>,
    /// Whether the user displays the guild's tag.
    #[serde(default)]
    pub identity_enabled: Option<bool>,
    /// Text of the tag, at most 4 characters.
    #[serde(default)]
    pub tag: Option<String>,
    /// Badge hash.
    #[serde(default)]
    pub badge: Option<String>,
}
