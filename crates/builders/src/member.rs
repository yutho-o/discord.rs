//! `EditMember` and `CreateBan`.

use crate::check_len;
use dr_core::{BodyError, IntoBody, Patch, RequestBody, Timestamp, ValidationError};
use dr_model::{ChannelId, RoleId};
use serde::Serialize;
use std::time::Duration;

/// The longest timeout Discord accepts.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(28 * 24 * 60 * 60);

/// The most history a ban can delete: 7 days.
pub const MAX_BAN_DELETE_SECONDS: u32 = 7 * 24 * 60 * 60;

fn now_micros() -> i64 {
    dr_core::Timestamp::now().0
}

/// The body of a member edit. A field that was not set stays out of the JSON and keeps its value on
/// Discord's side.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditMember {
    #[serde(skip_serializing_if = "Patch::is_absent")]
    nick: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<RoleId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deaf: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    communication_disabled_until: Patch<Timestamp>,
}

impl EditMember {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the nickname, at most 32 characters.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.nick = Patch::Value(nick.into());
        self
    }

    /// Removes the nickname.
    pub fn clear_nick(mut self) -> Self {
        self.nick = Patch::Null;
        self
    }

    /// Replaces the member's whole role list.
    pub fn roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.roles = Some(roles.into_iter().collect());
        self
    }

    /// Mutes or unmutes in voice channels. It fails if the member is not in one.
    pub fn mute(mut self, on: bool) -> Self {
        self.mute = Some(on);
        self
    }

    /// Deafens or undeafens in voice channels. It fails if the member is not in one.
    pub fn deaf(mut self, on: bool) -> Self {
        self.deaf = Some(on);
        self
    }

    /// Moves the member to another voice channel.
    pub fn move_to(mut self, channel: ChannelId) -> Self {
        self.channel_id = Patch::Value(channel);
        self
    }

    /// Disconnects the member from voice.
    pub fn disconnect(mut self) -> Self {
        self.channel_id = Patch::Null;
        self
    }

    /// Times the member out for this long from now, at most [`MAX_TIMEOUT`].
    pub fn timeout_for(self, duration: Duration) -> Self {
        let micros = i64::try_from(duration.as_micros()).unwrap_or(i64::MAX);
        self.timeout_until(Timestamp(now_micros().saturating_add(micros)))
    }

    /// Times the member out until this moment, at most [`MAX_TIMEOUT`] from now.
    pub fn timeout_until(mut self, until: Timestamp) -> Self {
        self.communication_disabled_until = Patch::Value(until);
        self
    }

    /// Ends a timeout.
    pub fn clear_timeout(mut self) -> Self {
        self.communication_disabled_until = Patch::Null;
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Patch::Value(nick) = &self.nick {
            check_len("nick", nick, 0, 32)?;
        }
        if let Patch::Value(until) = &self.communication_disabled_until {
            let max = i64::try_from(MAX_TIMEOUT.as_micros()).unwrap_or(i64::MAX);
            if until.0 > now_micros().saturating_add(max) {
                return Err(ValidationError::new("communication_disabled_until", "must be at most 28 days from now"));
            }
        }
        Ok(())
    }
}

impl IntoBody for EditMember {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// The body of a ban. Use `.reason(...)` on the call for the audit log.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct CreateBan {
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_message_seconds: Option<u32>,
}

impl CreateBan {
    /// A ban that deletes no messages.
    pub fn new() -> Self {
        Self::default()
    }

    /// Also deletes the user's messages from the last `seconds`, at most
    /// [`MAX_BAN_DELETE_SECONDS`].
    pub fn delete_message_seconds(mut self, seconds: u32) -> Self {
        self.delete_message_seconds = Some(seconds);
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self.delete_message_seconds {
            Some(seconds) if seconds > MAX_BAN_DELETE_SECONDS => {
                Err(ValidationError::new("delete_message_seconds", "must be at most 604800, which is 7 days"))
            }
            _ => Ok(()),
        }
    }
}

impl IntoBody for CreateBan {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
