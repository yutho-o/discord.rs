//! The audit log. Upstream: `payloads/v10/auditLog.ts`, `rest/v10/auditLog.ts` and
//! `GatewayGuildAuditLogEntryCreateDispatchData` in discord-api-types (MIT, (c) 2020 vladfrangu).
//!
//! A change keeps its old and new values as raw JSON: their type depends on the key, and Discord adds
//! keys without notice. [`AuditLogChange::new_as`] reads one as a type of your choice.

use crate::{
    ApplicationCommand, ApplicationId, AuditLogEntryId, AutoModRule, Channel, ChannelId, GenericId, GuildId,
    GuildScheduledEvent, MessageId, User, UserId, Webhook,
};
use dr_core::{null_default, open_int};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

open_int! {
    /// What an audit log entry records.
    AuditLogEvent: u16 {
        GUILD_UPDATE = 1,
        CHANNEL_CREATE = 10,
        CHANNEL_UPDATE = 11,
        CHANNEL_DELETE = 12,
        CHANNEL_OVERWRITE_CREATE = 13,
        CHANNEL_OVERWRITE_UPDATE = 14,
        CHANNEL_OVERWRITE_DELETE = 15,
        CHANNEL_POSITION_UPDATE = 16,
        MEMBER_KICK = 20,
        MEMBER_PRUNE = 21,
        MEMBER_BAN_ADD = 22,
        MEMBER_BAN_REMOVE = 23,
        MEMBER_UPDATE = 24,
        MEMBER_ROLE_UPDATE = 25,
        MEMBER_MOVE = 26,
        MEMBER_DISCONNECT = 27,
        BOT_ADD = 28,
        ROLE_CREATE = 30,
        ROLE_UPDATE = 31,
        ROLE_DELETE = 32,
        ROLE_POSITION_UPDATE = 33,
        INVITE_CREATE = 40,
        INVITE_UPDATE = 41,
        INVITE_DELETE = 42,
        WEBHOOK_CREATE = 50,
        WEBHOOK_UPDATE = 51,
        WEBHOOK_DELETE = 52,
        EMOJI_CREATE = 60,
        EMOJI_UPDATE = 61,
        EMOJI_DELETE = 62,
        MESSAGE_DELETE = 72,
        MESSAGE_BULK_DELETE = 73,
        MESSAGE_PIN = 74,
        MESSAGE_UNPIN = 75,
        INTEGRATION_CREATE = 80,
        INTEGRATION_UPDATE = 81,
        INTEGRATION_DELETE = 82,
        STAGE_INSTANCE_CREATE = 83,
        STAGE_INSTANCE_UPDATE = 84,
        STAGE_INSTANCE_DELETE = 85,
        STICKER_CREATE = 90,
        STICKER_UPDATE = 91,
        STICKER_DELETE = 92,
        GUILD_SCHEDULED_EVENT_CREATE = 100,
        GUILD_SCHEDULED_EVENT_UPDATE = 101,
        GUILD_SCHEDULED_EVENT_DELETE = 102,
        THREAD_CREATE = 110,
        THREAD_UPDATE = 111,
        THREAD_DELETE = 112,
        APPLICATION_COMMAND_PERMISSION_UPDATE = 121,
        SOUNDBOARD_SOUND_CREATE = 130,
        SOUNDBOARD_SOUND_UPDATE = 131,
        SOUNDBOARD_SOUND_DELETE = 132,
        AUTO_MODERATION_RULE_CREATE = 140,
        AUTO_MODERATION_RULE_UPDATE = 141,
        AUTO_MODERATION_RULE_DELETE = 142,
        AUTO_MODERATION_BLOCK_MESSAGE = 143,
        AUTO_MODERATION_FLAG_TO_CHANNEL = 144,
        AUTO_MODERATION_USER_COMMUNICATION_DISABLED = 145,
        AUTO_MODERATION_QUARANTINE_USER = 146,
        CREATOR_MONETIZATION_REQUEST_CREATED = 150,
        CREATOR_MONETIZATION_TERMS_ACCEPTED = 151,
        ONBOARDING_PROMPT_CREATE = 163,
        ONBOARDING_PROMPT_UPDATE = 164,
        ONBOARDING_PROMPT_DELETE = 165,
        ONBOARDING_CREATE = 166,
        ONBOARDING_UPDATE = 167,
        HOME_SETTINGS_CREATE = 190,
        HOME_SETTINGS_UPDATE = 191,
        VOICE_CHANNEL_STATUS_CREATE = 192,
        VOICE_CHANNEL_STATUS_DELETE = 193,
    }
}

/// One page of a guild's audit log, with the objects its entries name.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditLog {
    /// The entries, newest first.
    #[serde(default, deserialize_with = "null_default")]
    pub audit_log_entries: Vec<AuditLogEntry>,
    /// The users the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub users: Vec<User>,
    /// The webhooks the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub webhooks: Vec<Webhook>,
    /// The threads the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub threads: Vec<Channel>,
    /// The scheduled events the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub guild_scheduled_events: Vec<GuildScheduledEvent>,
    /// The auto moderation rules the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub auto_moderation_rules: Vec<AutoModRule>,
    /// The application commands the entries name.
    #[serde(default, deserialize_with = "null_default")]
    pub application_commands: Vec<ApplicationCommand>,
    /// The integrations the entries name, as raw JSON: integrations are not ported yet.
    #[serde(default, deserialize_with = "null_default")]
    pub integrations: Vec<Value>,
}

impl AuditLog {
    /// The user with this id, from `users`.
    pub fn user(&self, id: UserId) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }
}

/// One administrative action.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditLogEntry {
    /// Its id. Its timestamp is when the action happened.
    pub id: AuditLogEntryId,
    /// What the action was on: a user, role, channel or other id, depending on `action_type`.
    #[serde(default)]
    pub target_id: Option<String>,
    /// What changed.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<AuditLogChange>,
    /// Who did it. `None` for some automatic actions.
    #[serde(default)]
    pub user_id: Option<UserId>,
    /// What was done.
    pub action_type: AuditLogEvent,
    /// More about it, for some action types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AuditLogOptions>,
    /// The reason given, from the `X-Audit-Log-Reason` header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditLogEntry {
    /// `target_id` as a snowflake, when it is one.
    pub fn target(&self) -> Option<GenericId> {
        self.target_id.as_deref()?.parse().ok()
    }

    /// The change of `key`, if the entry has one.
    pub fn change(&self, key: &str) -> Option<&AuditLogChange> {
        self.changes.iter().find(|c| c.key == key)
    }
}

/// One changed field: its name and its values before and after.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditLogChange {
    /// The field, such as `name`, or `$add` and `$remove` for role changes.
    pub key: String,
    /// The value after, absent when the field was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_value: Option<Value>,
    /// The value before, absent when the field was added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
}

impl AuditLogChange {
    /// The value after, read as `T`. `None` when it is absent or of another type.
    pub fn new_as<T: DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_value(self.new_value.clone()?).ok()
    }

    /// The value before, read as `T`. `None` when it is absent or of another type.
    pub fn old_as<T: DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_value(self.old_value.clone()?).ok()
    }
}

/// More about an entry. Which fields are set depends on its action type. Counts come as strings
/// from Discord and are kept so.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AuditLogOptions {
    /// The name of the auto moderation rule that acted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_moderation_rule_name: Option<String>,
    /// That rule's trigger type, as a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_moderation_rule_trigger_type: Option<String>,
    /// How many days of inactivity a prune counted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_member_days: Option<String>,
    /// How many members a prune removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members_removed: Option<String>,
    /// The channel concerned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The message pinned or unpinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// How many entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<String>,
    /// The role or member of a permission overwrite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<GenericId>,
    /// `"0"` for a role overwrite, `"1"` for a member one.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "type")]
    pub kind: Option<String>,
    /// The role name of an overwrite, when it is a role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    /// The kind of integration that did a kick or a role change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_type: Option<String>,
    /// The application whose command permissions changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// The new voice channel status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// `GUILD_AUDIT_LOG_ENTRY_CREATE`: an entry was added. Needs `GUILD_MODERATION` and the
/// `VIEW_AUDIT_LOG` permission.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditLogEntryCreateEvent {
    /// The guild.
    pub guild_id: GuildId,
    /// The entry.
    #[serde(flatten)]
    pub entry: AuditLogEntry,
}
