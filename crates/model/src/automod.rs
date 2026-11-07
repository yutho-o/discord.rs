//! Auto moderation. Upstream: `payloads/v10/autoModeration.ts`, `rest/v10/autoModeration.ts` and
//! `GatewayAutoModerationActionExecutionDispatchData` in discord-api-types (MIT, (c) 2020 vladfrangu).
//!
//! The rule body is `CreateAutoModRule` in the builders crate. [`AutoModAction`] and
//! [`AutoModTriggerMetadata`] are written as well as read, so builders use them as they are.

use crate::{AutoModRuleId, ChannelId, GuildId, MessageId, RoleId, UserId};
use dr_core::{null_default, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// What a rule looks for.
    AutoModTriggerType: u16 {
        /// Words or patterns of the rule's own.
        KEYWORD = 1,
        /// Content that looks like spam.
        SPAM = 3,
        /// Words of Discord's lists.
        KEYWORD_PRESET = 4,
        /// Too many mentions in one message.
        MENTION_SPAM = 5,
        /// Words in member profiles.
        MEMBER_PROFILE = 6,
    }
}

open_int! {
    /// One of Discord's word lists.
    AutoModKeywordPreset: u16 {
        PROFANITY = 1,
        SEXUAL_CONTENT = 2,
        SLURS = 3,
    }
}

open_int! {
    /// When a rule runs.
    AutoModEventType: u16 {
        /// When a member sends or edits a message.
        MESSAGE_SEND = 1,
        /// When a member edits their profile.
        MEMBER_UPDATE = 2,
    }
}

open_int! {
    /// What a rule does when it triggers.
    AutoModActionType: u16 {
        /// Blocks the message.
        BLOCK_MESSAGE = 1,
        /// Posts an alert in a channel.
        SEND_ALERT_MESSAGE = 2,
        /// Times the member out.
        TIMEOUT = 3,
        /// Keeps the member from interacting in the guild until they change their profile.
        BLOCK_MEMBER_INTERACTION = 4,
    }
}

/// An auto moderation rule of a guild.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AutoModRule {
    /// Its id.
    pub id: AutoModRuleId,
    /// Its guild.
    pub guild_id: GuildId,
    /// Its name.
    pub name: String,
    /// Who created it.
    pub creator_id: UserId,
    /// When it runs.
    pub event_type: AutoModEventType,
    /// What it looks for.
    pub trigger_type: AutoModTriggerType,
    /// The details of what it looks for.
    #[serde(default)]
    pub trigger_metadata: AutoModTriggerMetadata,
    /// What it does.
    #[serde(default, deserialize_with = "null_default")]
    pub actions: Vec<AutoModAction>,
    /// Whether it is on.
    #[serde(default)]
    pub enabled: bool,
    /// Roles it does not apply to.
    #[serde(default, deserialize_with = "null_default")]
    pub exempt_roles: Vec<RoleId>,
    /// Channels it does not apply to.
    #[serde(default, deserialize_with = "null_default")]
    pub exempt_channels: Vec<ChannelId>,
}

/// What a rule looks for, by trigger type. Only the fields of the rule's trigger type are set.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoModTriggerMetadata {
    /// Words to match, for `KEYWORD` and `MEMBER_PROFILE`. `*` at either end matches a prefix or
    /// suffix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyword_filter: Vec<String>,
    /// Rust-flavoured regular expressions to match, for `KEYWORD` and `MEMBER_PROFILE`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regex_patterns: Vec<String>,
    /// Discord's word lists, for `KEYWORD_PRESET`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<AutoModKeywordPreset>,
    /// Words that never trigger the rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_list: Vec<String>,
    /// How many unique role and user mentions a message may have, for `MENTION_SPAM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_total_limit: Option<u8>,
    /// Whether to spot mention raids, for `MENTION_SPAM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_raid_protection_enabled: Option<bool>,
}

/// What a rule does when it triggers.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoModAction {
    /// The kind of action.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: AutoModActionType,
    /// Its details, for alerts, timeouts and custom block messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<AutoModActionMetadata>,
}

impl AutoModAction {
    /// Blocks the message. `message` is shown to the member, up to 150 characters.
    pub fn block(message: Option<String>) -> Self {
        let metadata = message.map(|m| AutoModActionMetadata { custom_message: Some(m), ..Default::default() });
        Self { kind: AutoModActionType::BLOCK_MESSAGE, metadata }
    }

    /// Posts an alert in `channel`.
    pub fn alert(channel: ChannelId) -> Self {
        let metadata = AutoModActionMetadata { channel_id: Some(channel), ..Default::default() };
        Self { kind: AutoModActionType::SEND_ALERT_MESSAGE, metadata: Some(metadata) }
    }

    /// Times the member out for `seconds`, up to 4 weeks. Only on keyword and mention spam rules.
    pub fn timeout(seconds: u32) -> Self {
        let metadata = AutoModActionMetadata { duration_seconds: Some(seconds), ..Default::default() };
        Self { kind: AutoModActionType::TIMEOUT, metadata: Some(metadata) }
    }

    /// Keeps the member from interacting until they change their profile. Only on member profile
    /// rules.
    pub fn block_member_interaction() -> Self {
        Self { kind: AutoModActionType::BLOCK_MEMBER_INTERACTION, metadata: None }
    }
}

/// The details of an action.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoModActionMetadata {
    /// Where an alert goes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// How long a timeout lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    /// What a blocked member is told.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_message: Option<String>,
}

/// `AUTO_MODERATION_ACTION_EXECUTION`: a rule triggered and an action ran. Needs
/// `AUTO_MODERATION_EXECUTION`, and `MESSAGE_CONTENT` for `content` and the matches.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AutoModActionExecutionEvent {
    /// The guild.
    pub guild_id: GuildId,
    /// The action that ran.
    pub action: AutoModAction,
    /// The rule that triggered.
    pub rule_id: AutoModRuleId,
    /// The rule's trigger type.
    pub rule_trigger_type: AutoModTriggerType,
    /// Whose content triggered it.
    pub user_id: UserId,
    /// Where, for a message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The message, unless it was blocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// The alert posted, for an alert action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert_system_message_id: Option<MessageId>,
    /// The content that triggered it. Empty without `MESSAGE_CONTENT`.
    #[serde(default)]
    pub content: String,
    /// The keyword or pattern that matched.
    #[serde(default)]
    pub matched_keyword: Option<String>,
    /// The part of the content that matched. `None` without `MESSAGE_CONTENT`.
    #[serde(default)]
    pub matched_content: Option<String>,
}
