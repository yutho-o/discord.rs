//! `CreateStageInstance` and `EditStageInstance`.

use crate::check_len;
use dr_core::{BodyError, IntoBody, RequestBody, ValidationError};
use dr_model::{ChannelId, ScheduledEventId, StageInstancePrivacyLevel};
use serde::Serialize;

/// Makes a stage channel live. The bot must be a stage moderator: `MANAGE_CHANNELS`,
/// `MUTE_MEMBERS` and `MOVE_MEMBERS` in the channel.
#[derive(Clone, Debug, Serialize)]
pub struct CreateStageInstance {
    channel_id: ChannelId,
    topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<StageInstancePrivacyLevel>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    send_start_notification: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_scheduled_event_id: Option<ScheduledEventId>,
}

impl CreateStageInstance {
    /// A live stage in `channel` about `topic`, 1 to 120 characters.
    pub fn new(channel: ChannelId, topic: impl Into<String>) -> Self {
        Self {
            channel_id: channel,
            topic: topic.into(),
            privacy_level: None,
            send_start_notification: false,
            guild_scheduled_event_id: None,
        }
    }

    /// Notifies the guild's members that the stage started. Needs `MENTION_EVERYONE`.
    pub fn notify(mut self) -> Self {
        self.send_start_notification = true;
        self
    }

    /// Ties it to the scheduled event it starts.
    pub fn scheduled_event(mut self, event: ScheduledEventId) -> Self {
        self.guild_scheduled_event_id = Some(event);
        self
    }

    /// Sets who can find it.
    pub fn privacy_level(mut self, level: StageInstancePrivacyLevel) -> Self {
        self.privacy_level = Some(level);
        self
    }

    /// Checks the topic.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("topic", &self.topic, 1, 120)
    }
}

impl IntoBody for CreateStageInstance {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// An edit of a live stage.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditStageInstance {
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<StageInstancePrivacyLevel>,
}

impl EditStageInstance {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Changes the topic, 1 to 120 characters.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Changes who can find it.
    pub fn privacy_level(mut self, level: StageInstancePrivacyLevel) -> Self {
        self.privacy_level = Some(level);
        self
    }

    /// Checks the topic, when set.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.topic.as_deref().map_or(Ok(()), |t| check_len("topic", t, 1, 120))
    }
}

impl IntoBody for EditStageInstance {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
