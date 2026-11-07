//! `CreateThread` and `CreateForumPost`: the bodies of new threads.

use crate::{CreateMessage, check_len, nested};
use dr_core::{BodyError, IntoBody, RequestBody, ValidationError};
use dr_model::{ChannelType, GenericId, ThreadAutoArchiveDuration};
use serde::Serialize;

/// The body of a new thread, the upstream `GuildTextThreadCreateOptions`. From a message, the thread
/// takes the message's channel type and `kind` and `invitable` are ignored. Without one, it is a
/// public thread unless [`Self::private`] is called.
#[derive(Clone, Debug, Serialize)]
pub struct CreateThread {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u16>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
}

impl CreateThread {
    /// A thread named `name`, 1 to 100 characters.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), auto_archive_duration: None, rate_limit_per_user: None, kind: None, invitable: None }
    }

    /// Sets after how long without activity the thread archives itself.
    pub fn auto_archive_duration(mut self, duration: ThreadAutoArchiveDuration) -> Self {
        self.auto_archive_duration = Some(duration);
        self
    }

    /// Sets the slow mode, in seconds, at most 6 hours.
    pub fn rate_limit_per_user(mut self, seconds: u16) -> Self {
        self.rate_limit_per_user = Some(seconds);
        self
    }

    /// Makes a private thread, which only invited members and moderators see. Not for a thread
    /// started from a message.
    pub fn private(mut self) -> Self {
        self.kind = Some(ChannelType::PRIVATE_THREAD);
        self
    }

    /// Lets the members of a private thread invite others.
    pub fn invitable(mut self, on: bool) -> Self {
        self.invitable = Some(on);
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 1, 100)?;
        if self.rate_limit_per_user.is_some_and(|r| r > crate::MAX_SLOW_MODE) {
            return Err(ValidationError::new("rate_limit_per_user", "must be at most 21600"));
        }
        Ok(())
    }
}

impl IntoBody for CreateThread {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// A new post in a forum or media channel: a thread and its first message, the upstream
/// `GuildForumThreadCreateOptions`. Files of the message are uploaded with it.
#[derive(Clone, Debug, Serialize)]
pub struct CreateForumPost {
    name: String,
    message: CreateMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<ThreadAutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u16>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applied_tags: Vec<GenericId>,
}

impl CreateForumPost {
    /// A post named `name`, 1 to 100 characters, starting with `message`.
    pub fn new(name: impl Into<String>, message: impl Into<CreateMessage>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            auto_archive_duration: None,
            rate_limit_per_user: None,
            applied_tags: Vec::new(),
        }
    }

    /// Adds a tag of the forum, at most 5.
    pub fn tag(mut self, tag: GenericId) -> Self {
        self.applied_tags.push(tag);
        self
    }

    /// Sets after how long without activity the post archives itself.
    pub fn auto_archive_duration(mut self, duration: ThreadAutoArchiveDuration) -> Self {
        self.auto_archive_duration = Some(duration);
        self
    }

    /// Sets the slow mode, in seconds, at most 6 hours.
    pub fn rate_limit_per_user(mut self, seconds: u16) -> Self {
        self.rate_limit_per_user = Some(seconds);
        self
    }

    /// Checks the name, the tags and the message.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 1, 100)?;
        if self.applied_tags.len() > 5 {
            return Err(ValidationError::new("applied_tags", "must hold at most 5 tags"));
        }
        if self.rate_limit_per_user.is_some_and(|r| r > crate::MAX_SLOW_MODE) {
            return Err(ValidationError::new("rate_limit_per_user", "must be at most 21600"));
        }
        self.message.validate().map_err(|e| nested(String::from("message"), e))
    }
}

impl IntoBody for CreateForumPost {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        let files = self.message.uploads();
        Ok(RequestBody::json(&self)?.with_files(files))
    }
}
