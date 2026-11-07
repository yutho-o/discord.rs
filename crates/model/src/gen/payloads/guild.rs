//! Guilds, members and roles. Upstream: `payloads/v10/guild.ts`, `permissions.ts`, the extra fields
//! of `GatewayGuildCreateDispatchData` in `gateway/v10.ts`, and field behaviour from discord.js
//! `packages/discord.js/src/structures/{Guild,GuildMember,Role}.js` (Apache-2.0).
//! Changed: translated to Rust; role tags read as `bool`, `ARCHITECTURE.md` 10.3.
//!
//! Left out on purpose, because the proof of concept cuts the features they belong to: `stickers`,
//! `threads`, `presences`, `voice_states`, `stage_instances`, `guild_scheduled_events` and
//! `soundboard_sounds`. Such keys are ignored on read. So are integrations, previews, widgets,
//! bans, onboarding and templates, which no route of the proof of concept returns.

use crate::{
    ApplicationId, AvatarDecorationData, Channel, ChannelId, Collectibles, Emoji, EmojiId, GenericId, GuildFeature,
    GuildId, IntegrationId, Permissions, RoleId, User, UserId,
};
use dr_core::{Timestamp, null_default};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

dr_core::open_int! {
    /// Which messages notify members by default. Unknown values are preserved.
    GuildDefaultMessageNotifications: u16 {
        ALL_MESSAGES = 0,
        ONLY_MENTIONS = 1,
    }
}

dr_core::open_int! {
    /// Whose media the explicit content filter scans. Unknown values are preserved.
    GuildExplicitContentFilter: u16 {
        DISABLED = 0,
        MEMBERS_WITHOUT_ROLES = 1,
        ALL_MEMBERS = 2,
    }
}

dr_core::open_int! {
    /// Multi-factor requirement for moderation. Unknown values are preserved.
    GuildMFALevel: u16 {
        NONE = 0,
        ELEVATED = 1,
    }
}

dr_core::open_int! {
    /// Age rating of a guild. Unknown values are preserved.
    GuildNSFWLevel: u16 {
        DEFAULT = 0,
        EXPLICIT = 1,
        SAFE = 2,
        AGE_RESTRICTED = 3,
    }
}

dr_core::open_int! {
    /// Verification required before a member can talk. Unknown values are preserved.
    GuildVerificationLevel: u16 {
        /// Unrestricted.
        NONE = 0,
        /// Must have a verified email on the account.
        LOW = 1,
        /// Must be registered on Discord for longer than 5 minutes.
        MEDIUM = 2,
        /// Must be a member of the guild for longer than 10 minutes.
        HIGH = 3,
        /// Must have a verified phone number.
        VERY_HIGH = 4,
    }
}

dr_core::open_int! {
    /// Server boost level. Unknown values are preserved.
    GuildPremiumTier: u16 {
        NONE = 0,
        TIER1 = 1,
        TIER2 = 2,
        TIER3 = 3,
    }
}

dr_core::open_int! {
    /// Kind of Student Hub. Unknown values are preserved.
    GuildHubType: u16 {
        DEFAULT = 0,
        HIGH_SCHOOL = 1,
        COLLEGE = 2,
    }
}

dr_core::open_flags! {
    /// What a guild's system channel leaves out. Unknown bits are kept.
    GuildSystemChannelFlags: number {
        /// Suppress member join notifications.
        SUPPRESS_JOIN_NOTIFICATIONS = 1 << 0,
        /// Suppress server boost notifications.
        SUPPRESS_PREMIUM_SUBSCRIPTIONS = 1 << 1,
        /// Suppress server setup tips.
        SUPPRESS_GUILD_REMINDER_NOTIFICATIONS = 1 << 2,
        /// Hide member join sticker reply buttons.
        SUPPRESS_JOIN_NOTIFICATION_REPLIES = 1 << 3,
        /// Suppress role subscription purchase and renewal notifications.
        SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATIONS = 1 << 4,
        /// Hide role subscription sticker reply buttons.
        SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATION_REPLIES = 1 << 5,
    }
}

dr_core::open_flags! {
    /// Flags on a guild member. Unknown bits are kept.
    GuildMemberFlags: number {
        /// Left and rejoined the guild.
        DID_REJOIN = 1 << 0,
        /// Completed onboarding.
        COMPLETED_ONBOARDING = 1 << 1,
        /// Exempt from guild verification requirements.
        BYPASSES_VERIFICATION = 1 << 2,
        /// Started onboarding.
        STARTED_ONBOARDING = 1 << 3,
        /// A guest, who can only access the voice channel they were invited to.
        IS_GUEST = 1 << 4,
        /// Started Server Guide new member actions.
        STARTED_HOME_ACTIONS = 1 << 5,
        /// Completed Server Guide new member actions.
        COMPLETED_HOME_ACTIONS = 1 << 6,
        /// Username, display name or nickname is blocked by AutoMod.
        AUTOMOD_QUARANTINED_USERNAME_OR_GUILD_NICKNAME = 1 << 7,
        /// Deprecated upstream.
        AUTOMOD_QUARANTINED_BIO = 1 << 8,
        /// Dismissed the DM settings upsell.
        DM_SETTINGS_UPSELL_ACKNOWLEDGED = 1 << 9,
        /// Guild tag is blocked by AutoMod.
        AUTO_MOD_QUARANTINED_GUILD_TAG = 1 << 10,
    }
}

dr_core::open_flags! {
    /// Flags on a role. Unknown bits are kept.
    RoleFlags: number {
        /// Can be selected by members in an onboarding prompt.
        IN_PROMPT = 1 << 0,
    }
}

/// A guild, from `GUILD_CREATE`, `GUILD_UPDATE` or REST.
///
/// `channels`, `roles`, `members` and `emojis` are full on `GUILD_CREATE` and may be empty
/// elsewhere. A guild read from the cache has `channels` and `members` empty, since those live in
/// their own tables.
///
/// During an outage `GUILD_CREATE` carries only `id` and `unavailable`, so everything but `id`
/// defaults, and a field upstream declares as always present is `None` or empty here.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Guild {
    /// Guild id.
    pub id: GuildId,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Icon hash.
    #[serde(default)]
    pub icon: Option<String>,
    /// Icon hash, returned in the template object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_hash: Option<String>,
    /// Invite splash hash.
    #[serde(default)]
    pub splash: Option<String>,
    /// Discovery splash hash, for guilds with the `DISCOVERABLE` feature.
    #[serde(default)]
    pub discovery_splash: Option<String>,
    /// Whether the current user owns the guild. Only from `GET /users/@me/guilds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
    /// Owner.
    #[serde(default)]
    pub owner_id: Option<UserId>,
    /// Permissions of the current user, without overwrites. Only from `GET /users/@me/guilds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// Voice region id. Deprecated upstream in favour of `rtc_region` on the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// AFK channel.
    #[serde(default)]
    pub afk_channel_id: Option<ChannelId>,
    /// AFK timeout in seconds: `60`, `300`, `900`, `1800` or `3600`.
    #[serde(default)]
    pub afk_timeout: u32,
    /// Whether the widget is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_enabled: Option<bool>,
    /// Channel the widget invites to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_channel_id: Option<ChannelId>,
    /// Verification level.
    #[serde(default)]
    pub verification_level: Option<crate::GuildVerificationLevel>,
    /// Default notification level.
    #[serde(default)]
    pub default_message_notifications: Option<crate::GuildDefaultMessageNotifications>,
    /// Explicit content filter level.
    #[serde(default)]
    pub explicit_content_filter: Option<crate::GuildExplicitContentFilter>,
    /// Roles.
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<Role>,
    /// Custom emoji.
    #[serde(default, deserialize_with = "null_default")]
    pub emojis: Vec<Emoji>,
    /// Enabled features. Unknown ones are kept.
    #[serde(default, deserialize_with = "null_default")]
    pub features: Vec<GuildFeature>,
    /// Required multi-factor level.
    #[serde(default)]
    pub mfa_level: Option<crate::GuildMFALevel>,
    /// Application that created the guild, if a bot did.
    #[serde(default)]
    pub application_id: Option<ApplicationId>,
    /// Channel for system messages.
    #[serde(default)]
    pub system_channel_id: Option<ChannelId>,
    /// What the system channel leaves out. Unknown bits are kept.
    #[serde(default)]
    pub system_channel_flags: Option<crate::GuildSystemChannelFlags>,
    /// Channel for rules, on community guilds.
    #[serde(default)]
    pub rules_channel_id: Option<ChannelId>,
    /// Maximum presences. Null except for the largest guilds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_presences: Option<u64>,
    /// Maximum members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_members: Option<u64>,
    /// Vanity invite code.
    #[serde(default)]
    pub vanity_url_code: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Banner hash.
    #[serde(default)]
    pub banner: Option<String>,
    /// Server boost level.
    #[serde(default)]
    pub premium_tier: Option<crate::GuildPremiumTier>,
    /// Number of boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_subscription_count: Option<u64>,
    /// Preferred locale of a community guild, by default `en-US`. Empty when not sent.
    #[serde(default)]
    pub preferred_locale: String,
    /// Channel for notices from Discord, on community guilds.
    #[serde(default)]
    pub public_updates_channel_id: Option<ChannelId>,
    /// Maximum users in a video channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_video_channel_users: Option<u64>,
    /// Maximum users in a stage video channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stage_video_channel_users: Option<u64>,
    /// Approximate member count, with counts requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_member_count: Option<u64>,
    /// Approximate non-offline member count, with counts requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_presence_count: Option<u64>,
    /// Age rating.
    #[serde(default)]
    pub nsfw_level: Option<crate::GuildNSFWLevel>,
    /// Whether the boost progress bar is shown.
    #[serde(default)]
    pub premium_progress_bar_enabled: bool,
    /// Kind of Student Hub.
    #[serde(default)]
    pub hub_type: Option<crate::GuildHubType>,
    /// Channel for safety alerts, on community guilds.
    #[serde(default)]
    pub safety_alerts_channel_id: Option<ChannelId>,
    /// Current incidents.
    #[serde(default)]
    pub incidents_data: Option<IncidentsData>,
    /// Welcome screen, returned in the invite object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_screen: Option<GuildWelcomeScreen>,
    /// True on an outage, when only `id` is meaningful.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unavailable: bool,
    /// Whether the guild is over the large threshold, `GUILD_CREATE` only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub large: bool,
    /// Member count, on `GUILD_CREATE` or with counts requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    /// When the bot joined, `GUILD_CREATE` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined_at: Option<Timestamp>,
    /// Channels, `GUILD_CREATE` only.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,
    /// Members, `GUILD_CREATE` only.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Member>,
}

impl Guild {
    /// A guild with only its id known.
    pub fn partial(id: GuildId) -> Self {
        Self {
            id,
            name: String::new(),
            icon: None,
            icon_hash: None,
            splash: None,
            discovery_splash: None,
            owner: None,
            owner_id: None,
            permissions: None,
            region: None,
            afk_channel_id: None,
            afk_timeout: 0,
            widget_enabled: None,
            widget_channel_id: None,
            verification_level: None,
            default_message_notifications: None,
            explicit_content_filter: None,
            roles: Vec::new(),
            emojis: Vec::new(),
            features: Vec::new(),
            mfa_level: None,
            application_id: None,
            system_channel_id: None,
            system_channel_flags: None,
            rules_channel_id: None,
            max_presences: None,
            max_members: None,
            vanity_url_code: None,
            description: None,
            banner: None,
            premium_tier: None,
            premium_subscription_count: None,
            preferred_locale: String::new(),
            public_updates_channel_id: None,
            max_video_channel_users: None,
            max_stage_video_channel_users: None,
            approximate_member_count: None,
            approximate_presence_count: None,
            nsfw_level: None,
            premium_progress_bar_enabled: false,
            hub_type: None,
            safety_alerts_channel_id: None,
            incidents_data: None,
            welcome_screen: None,
            unavailable: false,
            large: false,
            member_count: None,
            joined_at: None,
            channels: Vec::new(),
            members: Vec::new(),
        }
    }

    /// Id of the `@everyone` role, which is the id of the guild.
    pub const fn everyone_role_id(&self) -> RoleId {
        self.id.cast()
    }
}

impl From<UnavailableGuild> for Guild {
    /// The stub a `READY` entry stands for, as upstream's `guilds._add({ id, unavailable })`.
    fn from(g: UnavailableGuild) -> Self {
        Self { unavailable: g.unavailable, ..Self::partial(g.id) }
    }
}

/// A guild known only by id: the `READY` list, and `GUILD_DELETE`. `unavailable` true is an outage.
/// Absent is a removal, which cascades in the cache.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UnavailableGuild {
    /// Guild id.
    pub id: GuildId,
    /// True on an outage, absent when the bot was removed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unavailable: bool,
}

/// The welcome screen of a community guild.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuildWelcomeScreen {
    /// Short message.
    #[serde(default)]
    pub description: Option<String>,
    /// Suggested channels.
    #[serde(default, deserialize_with = "null_default")]
    pub welcome_channels: Vec<GuildWelcomeScreenChannel>,
}

/// One suggested channel on a welcome screen.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuildWelcomeScreenChannel {
    /// The channel.
    pub channel_id: ChannelId,
    /// What is shown for the channel.
    #[serde(default)]
    pub description: String,
    /// Id of the emoji shown beside it, if custom.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// Name of the emoji shown beside it.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

/// Current incident restrictions on a guild.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct IncidentsData {
    /// When invites are enabled again.
    #[serde(default)]
    pub invites_disabled_until: Option<Timestamp>,
    /// When direct messages are enabled again.
    #[serde(default)]
    pub dms_disabled_until: Option<Timestamp>,
    /// When DM spam was detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_spam_detected_at: Option<Timestamp>,
    /// When a raid was detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_detected_at: Option<Timestamp>,
}

/// A guild member. Every field defaults, because `member` inside a message has no `user`, and a
/// partial `GUILD_MEMBER_UPDATE` must still produce one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Member {
    /// The user. Absent on a member nested in a message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Guild, sent on gateway member events and set by the facade on REST results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Nickname.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// Per-guild avatar hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// Per-guild banner hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
    /// Roles.
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<RoleId>,
    /// Join time. Null for a guest.
    #[serde(default)]
    pub joined_at: Option<Timestamp>,
    /// When the member started boosting the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_since: Option<Timestamp>,
    /// Whether the member is deafened in voice channels.
    #[serde(default)]
    pub deaf: bool,
    /// Whether the member is muted in voice channels.
    #[serde(default)]
    pub mute: bool,
    /// Flags, `0` when absent. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<crate::GuildMemberFlags>,
    /// Whether the member has not passed membership screening. Absent means false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
    /// Permissions in the channel, on interaction payloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// When the member's timeout ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub communication_disabled_until: Option<Timestamp>,
    /// Per-guild avatar decoration. Absent resets to none, as upstream does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// Per-guild collectibles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
}

impl Member {
    /// The nickname, or else the display name of the user. `None` with neither, as on a member
    /// nested in a message.
    #[doc(alias = "displayName")]
    pub fn display_name(&self) -> Option<&str> {
        self.nick.as_deref().or_else(|| self.user.as_ref().map(User::display_name))
    }

    /// Whether only part of this member is known, because `joined_at` is missing or null.
    #[doc(alias = "partial")]
    pub const fn is_partial(&self) -> bool {
        self.joined_at.is_none()
    }
}

/// A guild role. The `@everyone` role has the id of its guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Role {
    /// Role id.
    pub id: RoleId,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Colour as `0xRRGGBB`. Deprecated upstream in favour of `colors`, which requests should use.
    #[serde(default)]
    pub color: u32,
    /// Colours. All zero when the payload has none.
    #[serde(default)]
    pub colors: RoleColors,
    /// Whether it is pinned in the member list.
    #[serde(default)]
    pub hoist: bool,
    /// Icon hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Unicode emoji shown as its icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_emoji: Option<String>,
    /// Sorting position.
    #[serde(default)]
    pub position: i64,
    /// Granted permissions.
    pub permissions: Permissions,
    /// Whether it is managed by an integration.
    #[serde(default)]
    pub managed: bool,
    /// Whether it can be mentioned.
    #[serde(default)]
    pub mentionable: bool,
    /// Tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<RoleTags>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<crate::RoleFlags>,
}

impl Role {
    /// Whether this is the `@everyone` role of `guild`.
    pub fn is_everyone(&self, guild: GuildId) -> bool {
        self.id == guild.cast()
    }
}

/// The colours of a role.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RoleColors {
    /// Primary colour.
    #[serde(default)]
    pub primary_color: u32,
    /// Secondary colour, which makes the role a gradient.
    #[serde(default)]
    pub secondary_color: Option<u32>,
    /// Tertiary colour, which makes the gradient holographic.
    #[serde(default)]
    pub tertiary_color: Option<u32>,
}

/// What a role belongs to. On the wire three of these are `key: null`, where presence means true
/// and absence means false, so a plain `Option` would lose them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RoleTags {
    /// The bot the role belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<UserId>,
    /// The integration the role belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<IntegrationId>,
    /// The role's subscription SKU and listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_listing_id: Option<GenericId>,
    /// Whether this is the guild's booster role.
    #[serde(
        default,
        deserialize_with = "key_present",
        serialize_with = "write_null",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub premium_subscriber: bool,
    /// Whether the role can be purchased.
    #[serde(
        default,
        deserialize_with = "key_present",
        serialize_with = "write_null",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub available_for_purchase: bool,
    /// Whether this is a linked role of the guild.
    #[serde(
        default,
        deserialize_with = "key_present",
        serialize_with = "write_null",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub guild_connections: bool,
}

/// The key is there, whatever its value is. Upstream tests `'key' in data.tags`.
fn key_present<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    serde::de::IgnoredAny::deserialize(d).map(|_| true)
}

fn write_null<S: Serializer>(_: &bool, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_none()
}
