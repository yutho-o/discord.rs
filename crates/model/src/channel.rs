//! Ported from discord.js packages/discord.js/src/structures/BaseChannel.js and
//! src/util/Constants.js (Apache-2.0), wire shapes from discord-api-types
//! payloads/v10/channel.ts (MIT).
//! Changed: translated to Rust; the twelve channel classes and union arms are one flat struct, and
//! duck-typed checks such as `'messages' in this` are decided by `kind`.
//!
//! `Channel` is one flat struct, `ARCHITECTURE.md` 7.4. Channels change type at runtime and a new
//! type must decode, so this is not an enum. Unknown `kind` values are kept.

use crate::{
    ApplicationId, ChannelId, ChannelType, EmojiId, GenericId, GuildId, Member, MessageId, Permissions, User, UserId,
};
use dr_core::{Timestamp, null_default, open_flags, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// Camera video quality of a voice or stage channel.
    VideoQualityMode: u16 {
        /// Discord chooses the quality for optimal performance.
        AUTO = 1,
        /// 720p.
        FULL = 2,
    }
}

open_int! {
    /// What a permission overwrite targets.
    OverwriteType: u16 {
        /// The overwrite targets a role.
        ROLE = 0,
        /// The overwrite targets a member.
        MEMBER = 1,
    }
}

open_int! {
    /// Minutes of inactivity after which a thread archives itself.
    ThreadAutoArchiveDuration: u16 {
        /// One hour.
        ONE_HOUR = 60,
        /// One day.
        ONE_DAY = 1_440,
        /// Three days.
        THREE_DAYS = 4_320,
        /// One week.
        ONE_WEEK = 10_080,
    }
}

open_int! {
    /// Default order of the posts in a thread-only channel.
    SortOrderType: u16 {
        /// By activity.
        LATEST_ACTIVITY = 0,
        /// By creation time, most recent first.
        CREATION_DATE = 1,
    }
}

open_int! {
    /// Default layout of the posts in a forum channel.
    ForumLayoutType: u16 {
        /// No default has been set by a channel admin.
        NOT_SET = 0,
        /// Posts as a list.
        LIST_VIEW = 1,
        /// Posts as a collection of tiles.
        GALLERY_VIEW = 2,
    }
}

open_flags! {
    /// Flags on a channel. Unknown bits are kept.
    ChannelFlags: number {
        /// Undocumented by Discord.
        GUILD_FEED_REMOVED = 1 << 0,
        /// This thread is pinned to the top of its parent forum channel.
        PINNED = 1 << 1,
        /// Undocumented by Discord.
        ACTIVE_CHANNELS_REMOVED = 1 << 2,
        /// A tag is required when creating a thread in this forum channel.
        REQUIRE_TAG = 1 << 4,
        /// Undocumented by Discord.
        IS_SPAM = 1 << 5,
        /// Undocumented by Discord.
        IS_GUILD_RESOURCE_CHANNEL = 1 << 7,
        /// Undocumented by Discord.
        CLYDE_AI = 1 << 8,
        /// Undocumented by Discord.
        IS_SCHEDULED_FOR_DELETION = 1 << 9,
        /// Media download options are hidden.
        HIDE_MEDIA_DOWNLOAD_OPTIONS = 1 << 15,
        /// The metadata is obfuscated because the current user cannot view the channel. Gateway only.
        CHANNEL_OBFUSCATED = 1 << 17,
        /// Users must opt in to view the contents.
        IS_SPOILER_CHANNEL = 1 << 21,
    }
}

open_flags! {
    /// Flags on a thread member. All undocumented by Discord. Unknown bits are kept.
    ThreadMemberFlags: number {
        /// The member has interacted with the thread.
        HAS_INTERACTED = 1 << 0,
        /// Notified of all messages.
        ALL_MESSAGES = 1 << 1,
        /// Notified of mentions only.
        ONLY_MENTIONS = 1 << 2,
        /// Not notified.
        NO_MESSAGES = 1 << 3,
    }
}

/// Any Discord channel. Fields a kind does not have are `None` or empty, and a partial channel, as
/// found in interaction data, decodes too. Only `id` and `type` are required.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Channel {
    /// Channel id.
    pub id: ChannelId,
    /// Channel kind. A value this build does not know is kept as is.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ChannelType,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<ChannelFlags>,
    /// Guild, absent on DMs and sometimes on gateway payloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Name, absent on DMs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Sorting position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    /// Topic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Whether the channel is age restricted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nsfw: bool,
    /// Parent category or thread parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ChannelId>,
    /// Latest message, or latest thread on a thread-only channel. It may not exist any more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message_id: Option<MessageId>,
    /// Latest pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pin_timestamp: Option<Timestamp>,
    /// Per-user slowmode, in seconds. Absent on a thread means no slowmode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_per_user: Option<i64>,
    /// Owner of a group DM or thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<UserId>,
    /// Permission overwrites.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    /// Recipients of a DM.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub recipients: Vec<User>,
    /// Icon hash of a group DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Application that created a group DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// Whether a group DM is managed by an OAuth2 application.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub managed: bool,
    /// Bitrate of a voice or stage channel, in bits per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<i64>,
    /// User limit of a voice or stage channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_limit: Option<i64>,
    /// Voice region id. `None` means automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtc_region: Option<String>,
    /// Camera video quality. See [`Channel::video_quality`] for the value when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_quality_mode: Option<VideoQualityMode>,
    /// Default archive delay of new threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    /// Slowmode copied to new threads at creation, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_thread_rate_limit_per_user: Option<i64>,
    /// Metadata of a thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_metadata: Option<ThreadMetadata>,
    /// The bot's own membership of a thread, on select endpoints only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Box<ThreadMember>>,
    /// Messages in a thread, without the initial or deleted ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_count: Option<i64>,
    /// Approximate members of a thread. It stops counting at 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<i64>,
    /// Messages ever sent in a thread. It does not go down on a delete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_message_sent: Option<i64>,
    /// Tags applied to a thread in a thread-only channel.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub applied_tags: Vec<GenericId>,
    /// Tags usable in a thread-only channel.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub available_tags: Vec<GuildForumTag>,
    /// Emoji on the add reaction button in a thread-only channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reaction_emoji: Option<GuildForumDefaultReactionEmoji>,
    /// Default post order of a thread-only channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sort_order: Option<SortOrderType>,
    /// Default post layout of a forum channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_forum_layout: Option<ForumLayoutType>,
    /// Permissions of the invoking user, on interaction resolved data only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// Permissions of the application, on interaction resolved data only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_permissions: Option<Permissions>,
}

/// One allow and deny pair on a channel. `allow` and `deny` are required: a guessed bit set could
/// grant what Discord denies.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PermissionOverwrite {
    /// Role or user id.
    pub id: GenericId,
    /// Role or member. Absent reads as a role.
    #[serde(rename = "type", default = "default_overwrite_type")]
    #[doc(alias = "type")]
    pub kind: OverwriteType,
    /// Granted permissions.
    pub allow: Permissions,
    /// Denied permissions.
    pub deny: Permissions,
}

const fn default_overwrite_type() -> OverwriteType {
    OverwriteType::ROLE
}

/// The part of a thread other channels do not have.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ThreadMetadata {
    /// Whether the thread is archived.
    #[serde(default)]
    pub archived: bool,
    /// Inactivity delay before it archives itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    /// When the archive status last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_timestamp: Option<Timestamp>,
    /// Whether only members with `MANAGE_THREADS` can unarchive it.
    #[serde(default)]
    pub locked: bool,
    /// Whether non-moderators can add other non-moderators. Private threads only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitable: Option<bool>,
    /// When the thread was created. Only on threads created after 2022-01-09.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_timestamp: Option<Timestamp>,
}

/// A member of a thread. `id` and `user_id` are omitted on the threads inside `GUILD_CREATE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadMember {
    /// The thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ChannelId>,
    /// The user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    /// When the user last joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_timestamp: Option<Timestamp>,
    /// Flags. Unknown bits are kept.
    #[serde(default = "ThreadMemberFlags::empty")]
    pub flags: ThreadMemberFlags,
    /// Guild member data, only when `with_member` was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

/// A tag usable on posts in a thread-only channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuildForumTag {
    /// Tag id.
    pub id: GenericId,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Whether only members with `MANAGE_THREADS` can add or remove it.
    #[serde(default)]
    pub moderated: bool,
    /// Custom emoji of the guild.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// Unicode emoji.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

/// The emoji on the add reaction button of a thread-only channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuildForumDefaultReactionEmoji {
    /// Custom emoji of the guild.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// Unicode emoji.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

impl Channel {
    /// A channel with only its id and kind known.
    pub fn partial(id: ChannelId, kind: ChannelType) -> Self {
        Self {
            id,
            kind,
            flags: None,
            guild_id: None,
            name: None,
            position: None,
            topic: None,
            nsfw: false,
            parent_id: None,
            last_message_id: None,
            last_pin_timestamp: None,
            rate_limit_per_user: None,
            owner_id: None,
            permission_overwrites: Vec::new(),
            recipients: Vec::new(),
            icon: None,
            application_id: None,
            managed: false,
            bitrate: None,
            user_limit: None,
            rtc_region: None,
            video_quality_mode: None,
            default_auto_archive_duration: None,
            default_thread_rate_limit_per_user: None,
            thread_metadata: None,
            member: None,
            message_count: None,
            member_count: None,
            total_message_sent: None,
            applied_tags: Vec::new(),
            available_tags: Vec::new(),
            default_reaction_emoji: None,
            default_sort_order: None,
            default_forum_layout: None,
            permissions: None,
            app_permissions: None,
        }
    }

    /// True when the channel holds messages. Matches upstream `isTextBased`, so it includes group
    /// DMs, where a bot cannot send: see [`Channel::is_sendable`].
    #[doc(alias = "isTextBased")]
    pub fn is_text_based(&self) -> bool {
        self.is_sendable() || self.kind == ChannelType::GROUP_DM
    }

    /// True when a bot can send messages here, permissions aside. Matches upstream `isSendable`.
    #[doc(alias = "isSendable")]
    pub fn is_sendable(&self) -> bool {
        matches!(
            self.kind,
            ChannelType::GUILD_TEXT
                | ChannelType::DM
                | ChannelType::GUILD_VOICE
                | ChannelType::GUILD_ANNOUNCEMENT
                | ChannelType::ANNOUNCEMENT_THREAD
                | ChannelType::PUBLIC_THREAD
                | ChannelType::PRIVATE_THREAD
                | ChannelType::GUILD_STAGE_VOICE
        )
    }

    /// True for the three thread kinds.
    #[doc(alias = "isThread")]
    pub fn is_thread(&self) -> bool {
        matches!(self.kind, ChannelType::ANNOUNCEMENT_THREAD | ChannelType::PUBLIC_THREAD | ChannelType::PRIVATE_THREAD)
    }

    /// True for voice and stage channels. Upstream tests for a `bitrate` key, which depends on the
    /// payload, so this deviates by deciding on `kind`.
    #[doc(alias = "isVoiceBased")]
    pub fn is_voice_based(&self) -> bool {
        matches!(self.kind, ChannelType::GUILD_VOICE | ChannelType::GUILD_STAGE_VOICE)
    }

    /// True for DMs and group DMs.
    #[doc(alias = "isDMBased")]
    pub fn is_dm_based(&self) -> bool {
        matches!(self.kind, ChannelType::DM | ChannelType::GROUP_DM)
    }

    /// True for channels that hold only threads: forum and media.
    #[doc(alias = "isThreadOnly")]
    pub fn is_thread_only(&self) -> bool {
        matches!(self.kind, ChannelType::GUILD_FORUM | ChannelType::GUILD_MEDIA)
    }

    /// Camera video quality. Discord documents absent as [`VideoQualityMode::AUTO`].
    #[doc(alias = "videoQualityMode")]
    pub fn video_quality(&self) -> VideoQualityMode {
        self.video_quality_mode.unwrap_or(VideoQualityMode::AUTO)
    }

    /// Creation time from the snowflake, in milliseconds since the Unix epoch.
    #[doc(alias = "createdTimestamp")]
    pub const fn created_at_ms(&self) -> i64 {
        self.id.timestamp_ms()
    }
}
