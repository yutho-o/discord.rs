//! `CreateScheduledEvent` and `EditScheduledEvent`. Rules from Discord's guild scheduled event
//! documentation and discord.js's `GuildScheduledEventManager` (Apache-2.0).

use crate::{ImageData, check_len};
use dr_core::{BodyError, IntoBody, RequestBody, Timestamp, ValidationError};
use dr_model::{
    ChannelId, RecurrenceRule, ScheduledEventEntityMetadata, ScheduledEventEntityType, ScheduledEventPrivacyLevel,
    ScheduledEventStatus,
};
use serde::Serialize;

/// A new scheduled event. Pick where it happens with one of the three constructors. Needs
/// `CREATE_EVENTS`, and in a channel, the permissions to join it.
#[derive(Clone, Debug, Serialize)]
pub struct CreateScheduledEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
    name: String,
    privacy_level: ScheduledEventPrivacyLevel,
    scheduled_start_time: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_end_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    entity_type: ScheduledEventEntityType,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_metadata: Option<ScheduledEventEntityMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence_rule: Option<RecurrenceRule>,
}

impl CreateScheduledEvent {
    fn base(name: String, start: Timestamp, entity_type: ScheduledEventEntityType) -> Self {
        Self {
            channel_id: None,
            name,
            privacy_level: ScheduledEventPrivacyLevel::GUILD_ONLY,
            scheduled_start_time: start,
            scheduled_end_time: None,
            description: None,
            entity_type,
            entity_metadata: None,
            image: None,
            recurrence_rule: None,
        }
    }

    /// An event in a voice channel.
    pub fn voice(name: impl Into<String>, channel: ChannelId, start: Timestamp) -> Self {
        let mut event = Self::base(name.into(), start, ScheduledEventEntityType::VOICE);
        event.channel_id = Some(channel);
        event
    }

    /// An event in a stage channel.
    pub fn stage(name: impl Into<String>, channel: ChannelId, start: Timestamp) -> Self {
        let mut event = Self::base(name.into(), start, ScheduledEventEntityType::STAGE_INSTANCE);
        event.channel_id = Some(channel);
        event
    }

    /// An event somewhere else, such as a web page or a place. It needs an end.
    pub fn external(name: impl Into<String>, location: impl Into<String>, start: Timestamp, end: Timestamp) -> Self {
        let mut event = Self::base(name.into(), start, ScheduledEventEntityType::EXTERNAL);
        event.entity_metadata = Some(ScheduledEventEntityMetadata { location: Some(location.into()) });
        event.scheduled_end_time = Some(end);
        event
    }

    /// When it ends.
    pub fn end(mut self, end: Timestamp) -> Self {
        self.scheduled_end_time = Some(end);
        self
    }

    /// Sets the description, up to 1000 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the cover image.
    pub fn image(mut self, image: ImageData) -> Self {
        self.image = Some(image);
        self
    }

    /// Makes it repeat.
    pub fn recurrence(mut self, rule: RecurrenceRule) -> Self {
        self.recurrence_rule = Some(rule);
        self
    }

    /// Checks a name of 1 to 100 characters, a description of at most 1000, a location of 1 to 100,
    /// and an end after the start.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 1, 100)?;
        if let Some(description) = &self.description {
            check_len("description", description, 0, 1000)?;
        }
        if let Some(location) = self.entity_metadata.as_ref().and_then(|m| m.location.as_deref()) {
            check_len("entity_metadata.location", location, 1, 100)?;
        }
        check_order(Some(&self.scheduled_start_time), self.scheduled_end_time.as_ref())
    }
}

/// Fails when the end is not after the start.
fn check_order(start: Option<&Timestamp>, end: Option<&Timestamp>) -> Result<(), ValidationError> {
    if let (Some(start), Some(end)) = (start, end)
        && end <= start
    {
        return Err(ValidationError::new("scheduled_end_time", "must be after the start"));
    }
    Ok(())
}

impl IntoBody for CreateScheduledEvent {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// An edit of a scheduled event. What is not set stays as it is. Moving an event to an external
/// place needs a location and an end in the same edit.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditScheduledEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<Option<ChannelId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_metadata: Option<Option<ScheduledEventEntityMetadata>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_start_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_end_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_type: Option<ScheduledEventEntityType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<ScheduledEventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence_rule: Option<Option<RecurrenceRule>>,
}

impl EditScheduledEvent {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(Some(description.into()));
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.description = Some(None);
        self
    }

    /// Moves it.
    pub fn start(mut self, start: Timestamp) -> Self {
        self.scheduled_start_time = Some(start);
        self
    }

    /// Sets its end.
    pub fn end(mut self, end: Timestamp) -> Self {
        self.scheduled_end_time = Some(end);
        self
    }

    /// Moves it to a voice channel.
    pub fn voice(mut self, channel: ChannelId) -> Self {
        self.entity_type = Some(ScheduledEventEntityType::VOICE);
        self.channel_id = Some(Some(channel));
        self.entity_metadata = Some(None);
        self
    }

    /// Moves it to a stage channel.
    pub fn stage(mut self, channel: ChannelId) -> Self {
        self.entity_type = Some(ScheduledEventEntityType::STAGE_INSTANCE);
        self.channel_id = Some(Some(channel));
        self.entity_metadata = Some(None);
        self
    }

    /// Moves it out of the guild's channels, to `location`, ending at `end`.
    pub fn external(mut self, location: impl Into<String>, end: Timestamp) -> Self {
        self.entity_type = Some(ScheduledEventEntityType::EXTERNAL);
        self.channel_id = Some(None);
        self.entity_metadata = Some(Some(ScheduledEventEntityMetadata { location: Some(location.into()) }));
        self.scheduled_end_time = Some(end);
        self
    }

    /// Starts, ends or cancels it. Discord allows scheduled to active or canceled, and active to
    /// completed.
    pub fn status(mut self, status: ScheduledEventStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets the cover image.
    pub fn image(mut self, image: ImageData) -> Self {
        self.image = Some(image);
        self
    }

    /// Makes it repeat by this rule.
    pub fn recurrence(mut self, rule: RecurrenceRule) -> Self {
        self.recurrence_rule = Some(Some(rule));
        self
    }

    /// Makes it a one-time event.
    pub fn clear_recurrence(mut self) -> Self {
        self.recurrence_rule = Some(None);
        self
    }

    /// Checks the fields that are set, as [`CreateScheduledEvent::validate`] does.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(name) = &self.name {
            check_len("name", name, 1, 100)?;
        }
        if let Some(Some(description)) = &self.description {
            check_len("description", description, 0, 1000)?;
        }
        if let Some(Some(location)) =
            self.entity_metadata.as_ref().map(|m| m.as_ref().and_then(|m| m.location.as_deref()))
        {
            check_len("entity_metadata.location", location, 1, 100)?;
        }
        check_order(self.scheduled_start_time.as_ref(), self.scheduled_end_time.as_ref())
    }
}

impl IntoBody for EditScheduledEvent {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
