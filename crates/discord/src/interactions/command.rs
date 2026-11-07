//! Command interactions.

use super::options::CommandOptions;
use super::reply::{FollowUpRequest, ReplyBody, ReplyRequest, ReplyState, ReplyTarget};
use crate::handles::GuildHandle;
use crate::{Bound, Call};
use dr_builders::{CreateMessage, EditMessage, ModalBuilder};
use dr_core::Secret;
use dr_model::{
    ApplicationCommandType, ApplicationId, ChannelId, CommandId, GuildId, InteractionId, InteractionOption, Member,
    Message, Permissions, ResolvedData, User,
};
use std::sync::Arc;

/// A slash or context menu command was used.
#[derive(Debug)]
#[non_exhaustive]
pub struct CommandInteraction {
    /// The interaction id.
    pub id: InteractionId,
    /// The application id.
    pub application_id: ApplicationId,
    /// The interaction token. Redacted in `Debug`.
    pub token: Secret,
    /// The command's id.
    pub command_id: CommandId,
    /// The command's name.
    pub name: String,
    /// Slash, user or message command.
    pub kind: ApplicationCommandType,
    /// The guild, `None` in a DM.
    pub guild_id: Option<GuildId>,
    /// The channel.
    pub channel_id: Option<ChannelId>,
    /// Who used it, whether in a guild or a DM.
    pub user: User,
    /// Their member data, in a guild.
    pub member: Option<Member>,
    /// The options as received. Read them through `options()`.
    pub options: Vec<InteractionOption>,
    /// The objects the options name.
    pub resolved: Option<ResolvedData>,
    /// Whether it was answered.
    pub reply_state: Arc<ReplyState>,
}

impl Bound<CommandInteraction> {
    /// Whether the person who used the command holds all of these permissions in this channel. False
    /// in a DM, and whenever Discord sent no permissions, so a check built on this fails closed.
    ///
    /// Handlers that act with the bot's powers, such as a ban, must check this: Discord's
    /// `default_member_permissions` is only a default, which server admins can change.
    pub fn has_permissions(&self, needed: Permissions) -> bool {
        self.member.as_ref().and_then(|m| m.permissions).is_some_and(|held| held.contains(needed))
    }

    /// Typed reads of the options.
    pub fn options(&self) -> CommandOptions<'_> {
        CommandOptions(self.data())
    }

    /// The first answer: a message. `AlreadyReplied` on a second one.
    pub fn reply(&self, body: impl Into<CreateMessage>) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Message(body.into()))
    }

    /// The first answer: "thinking...". Follow with `edit_reply`, within 15 minutes.
    pub fn defer(&self) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Defer { ephemeral: false })
    }

    /// The first answer: a modal.
    pub fn show_modal(&self, modal: ModalBuilder) -> ReplyRequest {
        ReplyRequest::new(self.target(), ReplyBody::Modal(modal))
    }

    /// Edits the first answer. `NotReplied` before one.
    pub fn edit_reply(&self, body: EditMessage) -> Call<Bound<Message>> {
        self.target().edit_reply(body)
    }

    /// Fetches the first answer. `NotReplied` before one.
    pub fn fetch_reply(&self) -> Call<Bound<Message>> {
        self.target().fetch_reply()
    }

    /// Deletes the first answer. `NotReplied` before one.
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

impl Bound<CommandInteraction> {
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
