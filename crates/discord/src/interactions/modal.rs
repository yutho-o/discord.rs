//! Modal submissions.

use super::reply::{FollowUpRequest, ReplyBody, ReplyRequest, ReplyState, ReplyTarget};
use crate::{Bound, Call};
use dr_builders::{CreateMessage, EditMessage};
use dr_core::Secret;
use dr_model::{ApplicationId, ChannelId, GuildId, InteractionId, Member, Message, User};
use std::sync::Arc;

/// A modal was submitted.
#[derive(Debug)]
#[non_exhaustive]
pub struct ModalInteraction {
    /// The interaction id.
    pub id: InteractionId,
    /// The application id.
    pub application_id: ApplicationId,
    /// The interaction token. Redacted in `Debug`.
    pub token: Secret,
    /// The modal's `custom_id`.
    pub custom_id: String,
    /// Each text input's `custom_id` and value.
    pub fields: Vec<(String, String)>,
    /// The guild, `None` in a DM.
    pub guild_id: Option<GuildId>,
    /// The channel.
    pub channel_id: Option<ChannelId>,
    /// Who submitted it, whether in a guild or a DM.
    pub user: User,
    /// Their member data, in a guild.
    pub member: Option<Member>,
    /// Whether it was answered.
    pub reply_state: Arc<ReplyState>,
}

impl ModalInteraction {
    /// The value of a text input.
    pub fn value(&self, custom_id: &str) -> Option<&str> {
        self.fields.iter().find(|(id, _)| id == custom_id).map(|(_, v)| v.as_str())
    }
}

impl Bound<ModalInteraction> {
    /// The first answer: a message. `AlreadyReplied` on a second one.
    pub fn reply(&self, body: impl Into<CreateMessage>) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Message(body.into()))
    }

    /// The first answer: "thinking...". Follow with `edit_reply`.
    pub fn defer(&self) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Defer { ephemeral: false })
    }

    /// Edits the answer. `NotReplied` before one.
    pub fn edit_reply(&self, body: EditMessage) -> Call<Bound<Message>> {
        self.target().edit_reply(body)
    }

    /// Sends a further message. `NotReplied` before the first answer.
    pub fn follow_up(&self, body: impl Into<CreateMessage>) -> FollowUpRequest {
        FollowUpRequest::new(self.target(), body.into())
    }
}

impl Bound<ModalInteraction> {
    /// What every answer needs.
    fn target(&self) -> ReplyTarget {
        ReplyTarget {
            client: self.client().clone(),
            id: self.id,
            application_id: self.application_id,
            token: self.token.clone(),
            state: self.reply_state.clone(),
        }
    }
}
