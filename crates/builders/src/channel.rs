//! `CreateChannel`, `EditChannel` and `ChannelPosition`: the bodies of channel creation, edits and
//! reordering.

use crate::check_len;
use dr_core::{BodyError, IntoBody, Patch, RequestBody, ValidationError};
use dr_model::{ChannelId, ChannelType, GenericId, PermissionOverwrite, ThreadAutoArchiveDuration, VideoQualityMode};
use serde::Serialize;

/// The longest slow mode: 6 hours, in seconds.
pub const MAX_SLOW_MODE: u16 = 21_600;

/// The lowest bitrate of a voice channel, in bits per second.
pub const MIN_BITRATE: u32 = 8_000;

/// A topic is at most 1024 characters, or 4096 in a forum or media channel. `None` is a channel of
/// unknown type, as on an edit, which gets the higher limit and leaves the check to Discord.
fn check_topic(topic: &str, kind: Option<ChannelType>) -> Result<(), ValidationError> {
    let long = matches!(kind, None | Some(ChannelType::GUILD_FORUM | ChannelType::GUILD_MEDIA));
    check_len("topic", topic, 0, if long { 4096 } else { 1024 })
}

fn check_common(
    rate_limit: Option<u16>,
    bitrate: Option<u32>,
    user_limit: Option<u16>,
    overwrites: Option<&[PermissionOverwrite]>,
) -> Result<(), ValidationError> {
    if rate_limit.is_some_and(|r| r > MAX_SLOW_MODE) {
        return Err(ValidationError::new("rate_limit_per_user", format!("must be at most {MAX_SLOW_MODE}")));
    }
    if bitrate.is_some_and(|b| b < MIN_BITRATE) {
        return Err(ValidationError::new("bitrate", format!("must be at least {MIN_BITRATE}")));
    }
    if user_limit.is_some_and(|u| u > 10_000) {
        return Err(ValidationError::new("user_limit", "must be at most 99, or 10000 in a stage channel"));
    }
    if overwrites.is_some_and(|o| o.len() > 100) {
        return Err(ValidationError::new("permission_overwrites", "must hold at most 100 overwrites"));
    }
    Ok(())
}

/// The body of a new guild channel, the upstream `GuildChannelCreateOptions`. Fields left unset get
/// Discord's defaults: a text channel at the bottom, with the permissions of its category if any.
///
/// ```
/// # use dr_builders::CreateChannel;
/// # use dr_model::{ChannelType, PermissionOverwrite, Permissions, RoleId};
/// let staff = RoleId::new(2);
/// let body = CreateChannel::new("staff-room")
///     .topic("Moderators only")
///     .permission_overwrite(PermissionOverwrite::role(RoleId::new(1), Permissions::empty(), Permissions::VIEW_CHANNEL))
///     .permission_overwrite(PermissionOverwrite::role(staff, Permissions::VIEW_CHANNEL, Permissions::empty()));
/// ```
///
/// The `@everyone` role has the guild's id.
#[derive(Clone, Debug, Serialize)]
pub struct CreateChannel {
    name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtc_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_quality_mode: Option<VideoQualityMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    permission_overwrites: Vec<PermissionOverwrite>,
}

impl CreateChannel {
    /// A text channel named `name`, 1 to 100 characters.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: None,
            topic: None,
            nsfw: None,
            parent_id: None,
            position: None,
            rate_limit_per_user: None,
            bitrate: None,
            user_limit: None,
            rtc_region: None,
            video_quality_mode: None,
            default_auto_archive_duration: None,
            permission_overwrites: Vec::new(),
        }
    }

    /// Sets the type, such as `ChannelType::GUILD_VOICE` or `ChannelType::GUILD_CATEGORY`.
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Sets the topic.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Marks the channel age-restricted.
    pub fn nsfw(mut self, on: bool) -> Self {
        self.nsfw = Some(on);
        self
    }

    /// Puts the channel in a category.
    pub fn parent(mut self, category: ChannelId) -> Self {
        self.parent_id = Some(category);
        self
    }

    /// Sets the position in the list.
    pub fn position(mut self, position: i64) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets the slow mode, in seconds, at most 6 hours.
    pub fn rate_limit_per_user(mut self, seconds: u16) -> Self {
        self.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the bitrate of a voice channel, in bits per second, at least 8000.
    pub fn bitrate(mut self, bits: u32) -> Self {
        self.bitrate = Some(bits);
        self
    }

    /// Sets how many people a voice channel holds, 0 for no limit.
    pub fn user_limit(mut self, limit: u16) -> Self {
        self.user_limit = Some(limit);
        self
    }

    /// Sets the voice region, such as `rotterdam`.
    pub fn rtc_region(mut self, region: impl Into<String>) -> Self {
        self.rtc_region = Some(region.into());
        self
    }

    /// Sets the camera quality of a voice channel.
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.video_quality_mode = Some(mode);
        self
    }

    /// Sets after how long new threads archive themselves.
    pub fn default_auto_archive_duration(mut self, duration: ThreadAutoArchiveDuration) -> Self {
        self.default_auto_archive_duration = Some(duration);
        self
    }

    /// Adds a permission overwrite.
    pub fn permission_overwrite(mut self, overwrite: PermissionOverwrite) -> Self {
        self.permission_overwrites.push(overwrite);
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 1, 100)?;
        if let Some(topic) = &self.topic {
            check_topic(topic, Some(self.kind.unwrap_or(ChannelType::GUILD_TEXT)))?;
        }
        check_common(self.rate_limit_per_user, self.bitrate, self.user_limit, Some(&self.permission_overwrites))
    }
}

impl IntoBody for CreateChannel {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// The body of a channel or thread edit, the upstream `GuildChannelEditOptions` and
/// `ThreadEditOptions`. A field that was not set stays out of the JSON and keeps its value.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditChannel {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    topic: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    parent_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    position: Patch<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u16>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    rtc_region: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_quality_mode: Option<VideoQualityMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Vec<GenericId>>,
}

impl EditChannel {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames the channel, 1 to 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Converts between a text and an announcement channel, the only change Discord allows.
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Sets the topic.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Patch::Value(topic.into());
        self
    }

    /// Removes the topic: writes `"topic": null`.
    pub fn clear_topic(mut self) -> Self {
        self.topic = Patch::Null;
        self
    }

    /// Marks the channel age-restricted, or not.
    pub fn nsfw(mut self, on: bool) -> Self {
        self.nsfw = Some(on);
        self
    }

    /// Moves the channel into a category.
    pub fn parent(mut self, category: ChannelId) -> Self {
        self.parent_id = Patch::Value(category);
        self
    }

    /// Takes the channel out of its category: writes `"parent_id": null`.
    pub fn clear_parent(mut self) -> Self {
        self.parent_id = Patch::Null;
        self
    }

    /// Sets the position in the list.
    pub fn position(mut self, position: i64) -> Self {
        self.position = Patch::Value(position);
        self
    }

    /// Sets the slow mode, in seconds, at most 6 hours. 0 turns it off.
    pub fn rate_limit_per_user(mut self, seconds: u16) -> Self {
        self.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the bitrate of a voice channel, in bits per second, at least 8000.
    pub fn bitrate(mut self, bits: u32) -> Self {
        self.bitrate = Some(bits);
        self
    }

    /// Sets how many people a voice channel holds, 0 for no limit.
    pub fn user_limit(mut self, limit: u16) -> Self {
        self.user_limit = Some(limit);
        self
    }

    /// Sets the voice region.
    pub fn rtc_region(mut self, region: impl Into<String>) -> Self {
        self.rtc_region = Patch::Value(region.into());
        self
    }

    /// Lets Discord pick the voice region: writes `"rtc_region": null`.
    pub fn automatic_rtc_region(mut self) -> Self {
        self.rtc_region = Patch::Null;
        self
    }

    /// Sets the camera quality of a voice channel.
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.video_quality_mode = Some(mode);
        self
    }

    /// Sets after how long new threads archive themselves.
    pub fn default_auto_archive_duration(mut self, duration: ThreadAutoArchiveDuration) -> Self {
        self.default_auto_archive_duration = Some(duration);
        self
    }

    /// Replaces every permission overwrite with `overwrites`. To change one, use the channel's
    /// `set_permission` instead.
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.permission_overwrites = Some(overwrites);
        self
    }

    /// Archives a thread, or brings it back.
    pub fn archived(mut self, on: bool) -> Self {
        self.archived = Some(on);
        self
    }

    /// Locks a thread: only moderators can unarchive it.
    pub fn locked(mut self, on: bool) -> Self {
        self.locked = Some(on);
        self
    }

    /// Lets the members of a private thread invite others.
    pub fn invitable(mut self, on: bool) -> Self {
        self.invitable = Some(on);
        self
    }

    /// Sets after how long without activity a thread archives itself.
    pub fn auto_archive_duration(mut self, duration: ThreadAutoArchiveDuration) -> Self {
        self.auto_archive_duration = Some(duration);
        self
    }

    /// Replaces the tags of a forum post, at most 5.
    pub fn applied_tags(mut self, tags: Vec<GenericId>) -> Self {
        self.applied_tags = Some(tags);
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.applied_tags.as_ref().is_some_and(|t| t.len() > 5) {
            return Err(ValidationError::new("applied_tags", "must hold at most 5 tags"));
        }
        if let Some(name) = &self.name {
            check_len("name", name, 1, 100)?;
        }
        if let Patch::Value(topic) = &self.topic {
            check_topic(topic, None)?;
        }
        check_common(self.rate_limit_per_user, self.bitrate, self.user_limit, self.permission_overwrites.as_deref())
    }
}

impl IntoBody for EditChannel {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// One entry of a channel reordering, `PATCH /guilds/{guild}/channels`.
#[derive(Clone, Debug, Serialize)]
pub struct ChannelPosition {
    id: ChannelId,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock_permissions: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    parent_id: Patch<ChannelId>,
}

impl ChannelPosition {
    /// Moves `channel` to `position`.
    pub fn new(channel: ChannelId, position: i64) -> Self {
        Self { id: channel, position: Some(position), lock_permissions: None, parent_id: Patch::Absent }
    }

    /// Moves it into a category too.
    pub fn parent(mut self, category: ChannelId) -> Self {
        self.parent_id = Patch::Value(category);
        self
    }

    /// Takes it out of its category.
    pub fn clear_parent(mut self) -> Self {
        self.parent_id = Patch::Null;
        self
    }

    /// Takes the permissions of the new category.
    pub fn lock_permissions(mut self, on: bool) -> Self {
        self.lock_permissions = Some(on);
        self
    }
}
