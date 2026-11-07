//! Id aliases and the hand-written id-like types of `ARCHITECTURE.md` 7.5.

use dr_core::Id;
use dr_core::marker::{
    ApplicationMarker, AttachmentMarker, ChannelMarker, CommandMarker, EmojiMarker, GenericMarker, GuildMarker,
    IntegrationMarker, InteractionMarker, MessageMarker, RoleMarker, UserMarker, WebhookMarker,
};
use dr_core::marker::{
    AuditLogEntryMarker, AutoModRuleMarker, EntitlementMarker, ScheduledEventMarker, SkuMarker, StageInstanceMarker,
    StickerMarker, StickerPackMarker, SubscriptionMarker,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Id of a user.
pub type UserId = Id<UserMarker>;
/// Id of a guild.
pub type GuildId = Id<GuildMarker>;
/// Id of a channel.
pub type ChannelId = Id<ChannelMarker>;
/// Id of a message.
pub type MessageId = Id<MessageMarker>;
/// Id of a role.
pub type RoleId = Id<RoleMarker>;
/// Id of a custom emoji.
pub type EmojiId = Id<EmojiMarker>;
/// Id of a message attachment.
pub type AttachmentId = Id<AttachmentMarker>;
/// Id of a webhook.
pub type WebhookId = Id<WebhookMarker>;
/// Id of an application.
pub type ApplicationId = Id<ApplicationMarker>;
/// Id of an application command.
pub type CommandId = Id<CommandMarker>;
/// Id of an interaction.
pub type InteractionId = Id<InteractionMarker>;
/// Id of an integration.
pub type IntegrationId = Id<IntegrationMarker>;
/// Id of a sticker.
pub type StickerId = Id<StickerMarker>;
/// Id of a pack of standard stickers.
pub type StickerPackId = Id<StickerPackMarker>;
/// Id of a guild scheduled event.
pub type ScheduledEventId = Id<ScheduledEventMarker>;
/// Id of an auto moderation rule.
pub type AutoModRuleId = Id<AutoModRuleMarker>;
/// Id of an audit log entry.
pub type AuditLogEntryId = Id<AuditLogEntryMarker>;
/// Id of a stage instance.
pub type StageInstanceId = Id<StageInstanceMarker>;
/// Id of a SKU, something an application sells.
pub type SkuId = Id<SkuMarker>;
/// Id of an entitlement, a user's or guild's access to a SKU.
pub type EntitlementId = Id<EntitlementMarker>;
/// Id of a subscription.
pub type SubscriptionId = Id<SubscriptionMarker>;
/// A snowflake with no specific resource, such as a command version or an overwrite target.
pub type GenericId = Id<GenericMarker>;

/// A message nonce, which Discord accepts and echoes as an integer or a string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Nonce {
    /// Integer form.
    Int(i64),
    /// String form, at most 25 characters.
    Str(String),
}

impl From<i64> for Nonce {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<&str> for Nonce {
    fn from(v: &str) -> Self {
        Self::Str(v.to_owned())
    }
}

impl From<String> for Nonce {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}

impl fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => fmt::Display::fmt(v, f),
            Self::Str(v) => f.write_str(v),
        }
    }
}
