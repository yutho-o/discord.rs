//! Component interactions.

use super::reply::{FollowUpRequest, ReplyBody, ReplyRequest, ReplyState, ReplyTarget};
use crate::handles::GuildHandle;
use crate::{Bound, Call};
use dr_builders::{CreateMessage, EditMessage, ModalBuilder};
use dr_core::Secret;
use dr_model::{ApplicationId, ChannelId, ComponentType, GuildId, InteractionId, Member, Message, User};
use std::sync::Arc;

/// A button was clicked, or a select was used.
#[derive(Debug)]
#[non_exhaustive]
pub struct ComponentInteraction {
    /// The interaction id.
    pub id: InteractionId,
    /// The application id.
    pub application_id: ApplicationId,
    /// The interaction token. Redacted in `Debug`.
    pub token: Secret,
    /// The component's `custom_id`.
    pub custom_id: String,
    /// Which kind of component.
    pub component_type: ComponentType,
    /// The values chosen in a select. Empty for a button.
    pub values: Vec<String>,
    /// The message the component is on.
    pub message: Message,
    /// The guild, `None` in a DM.
    pub guild_id: Option<GuildId>,
    /// The channel.
    pub channel_id: Option<ChannelId>,
    /// Who used it, whether in a guild or a DM.
    pub user: User,
    /// Their member data, in a guild.
    pub member: Option<Member>,
    /// Whether it was answered.
    pub reply_state: Arc<ReplyState>,
}

impl Bound<ComponentInteraction> {
    /// The first answer: a new message. `AlreadyReplied` on a second one.
    pub fn reply(&self, body: impl Into<CreateMessage>) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Message(body.into()))
    }

    /// The first answer: nothing visible now. Follow with `edit_reply` to change the message.
    pub fn defer(&self) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::DeferUpdate)
    }

    /// The first answer: edits the message the component is on.
    pub fn update(&self, body: EditMessage) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Update(body))
    }

    /// The first answer: a modal.
    pub fn show_modal(&self, modal: ModalBuilder) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Modal(modal))
    }

    /// Edits the answer. `NotReplied` before one.
    pub fn edit_reply(&self, body: EditMessage) -> Call<Bound<Message>> {
        self.target().edit_reply(body)
    }

    /// Fetches the answer. `NotReplied` before one.
    pub fn fetch_reply(&self) -> Call<Bound<Message>> {
        self.target().fetch_reply()
    }

    /// Deletes the answer. `NotReplied` before one.
    pub fn delete_reply(&self) -> Call<()> {
        self.target().delete_reply()
    }

    /// Sends a further message. `NotReplied` before the first answer.
    pub fn follow_up(&self, body: impl Into<CreateMessage>) -> FollowUpRequest {
        FollowUpRequest::new(self.target(), body.into())
    }

    /// The guild, as a handle. `None` in a DM.
    pub fn guild(&self) -> Option<GuildHandle> {
        self.guild_id.map(|g| self.client().guild(g))
    }
}

impl Bound<ComponentInteraction> {
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
