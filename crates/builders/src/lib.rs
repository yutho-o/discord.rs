//! Request bodies and nested data builders. No tokio. Naming, `ARCHITECTURE.md` 7.1: request bodies
//! are verb-noun (`CreateMessage`, `EditMessage`), nested data keeps the upstream `*Builder` name
//! (`EmbedBuilder`, `ActionRowBuilder`, `SlashCommandBuilder`).
//!
//! Setters take a bare noun. Validation is hand-written and runs at send time, through
//! [`dr_core::IntoBody`], so a failure surfaces at `.await` with nothing sent. `.skip_validation()`
//! is per request. Lengths are code points (`chars().count()`), and the embed total of 6000 is
//! UTF-16 units. Rules are in `[types 4.3]`.

pub mod attachment;
pub mod automod;
pub mod channel;
pub mod command;
pub mod component;
pub mod embed;
pub mod expression;
pub mod member;
pub mod message;
pub mod modal;
pub mod poll;
pub mod role;
pub mod scheduled_event;
pub mod stage;
pub mod thread;
pub mod webhook;

pub use attachment::{CreateAttachment, MAX_ATTACHMENTS};
pub use automod::{CreateAutoModRule, EditAutoModRule, MAX_AUTOMOD_TIMEOUT};
pub use channel::{ChannelPosition, CreateChannel, EditChannel, MAX_SLOW_MODE, MIN_BITRATE};
pub use command::{
    BooleanOption, ChannelOption, CommandData, CommandList, CommandOption, IntegerOption, Leaf, MessageContextCommand,
    Parent, RoleOption, SlashCommandBuilder, StringOption, Subcommand, Unset, UserContextCommand, UserOption,
};
pub use component::{ActionRowBuilder, ButtonBuilder, StringSelectBuilder, StringSelectOption, TextInputBuilder};
pub use embed::EmbedBuilder;
pub use expression::{CreateEmoji, CreateSticker, EditEmoji, EditSticker, MAX_EMOJI_BYTES, MAX_STICKER_BYTES};
pub use member::{CreateBan, EditMember, MAX_BAN_DELETE_SECONDS, MAX_TIMEOUT};
pub use message::{AllowedMentions, BulkDeleteMessages, CreateMessage, EditMessage, reply_reference};
pub use modal::ModalBuilder;
pub use poll::{MAX_POLL_ANSWERS, MAX_POLL_HOURS, PollBuilder};
pub use role::{CreateRole, EditRole};
pub use scheduled_event::{CreateScheduledEvent, EditScheduledEvent};
pub use stage::{CreateStageInstance, EditStageInstance};
pub use thread::{CreateForumPost, CreateThread};
pub use webhook::{CreateWebhook, EditWebhook, ExecuteWebhook, ImageData};

/// Fails unless `value` has between `min` and `max` code points.
pub(crate) fn check_len(path: &str, value: &str, min: usize, max: usize) -> Result<(), dr_core::ValidationError> {
    let n = value.chars().count();
    if n < min || n > max {
        return Err(dr_core::ValidationError::new(path, format!("must be {min} to {max} characters, got {n}")));
    }
    Ok(())
}

/// Adds `prefix` in front of the path of an error from a nested value.
pub(crate) fn nested(prefix: String, e: dr_core::ValidationError) -> dr_core::ValidationError {
    let path = if e.path.is_empty() { prefix } else { format!("{prefix}.{}", e.path) };
    dr_core::ValidationError::new(path, e.message)
}
