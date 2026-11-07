//! Gateway envelope and the `Dispatch` table, `ARCHITECTURE.md` 8.2.
//!
//! Event bodies follow discord-api-types `gateway/v10.ts` (MIT, (c) 2020 vladfrangu), under the
//! patterns of `ARCHITECTURE.md` 7.4. Only some of the 79 upstream events are typed so far. Every
//! other event reaches [`Dispatch::Unknown`] with its name and raw JSON, and is not lost.

use crate::{
    ApplicationFlags, ApplicationId, Attachment, AuditLogEntryCreateEvent, AutoModActionExecutionEvent, AutoModRule,
    AvatarDecorationData, Channel, ChannelId, ChannelMention, ChannelType, Collectibles, Component, Embed, Emoji,
    Guild, GuildId, GuildMemberFlags, GuildScheduledEvent, GuildStickersUpdateEvent, Interaction, Member, Message,
    MessageActivity, MessageCall, MessageFlags, MessageId, MessageInteraction, MessageInteractionMetadata,
    MessagePollVoteEvent, MessageReference, MessageSharedClientTheme, MessageSnapshot, MessageType, Nonce, Reaction,
    ResolvedData, Role, RoleId, ScheduledEventUserEvent, StageInstance, ThreadMember, UnavailableGuild, User, UserId,
    VoiceServerUpdateEvent, VoiceState, WebhookId,
};
use dr_core::{DecodeError, Patch, Secret, Timestamp, decode, null_default};
use serde::de::{Deserializer, Error as DeError, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::borrow::Cow;
use std::fmt;

/// The outer frame of every gateway message. The socket task reads only this, and advances the
/// sequence from it before any typed decode.
///
/// `Debug` prints the length of `d`, not its text: a raw body can hold a session id or a token.
#[derive(Deserialize)]
pub struct Envelope<'a> {
    /// Opcode. Never narrower than `u32`, or an unknown one would reject the frame.
    pub op: u32,
    /// Sequence number, on dispatches.
    #[serde(default)]
    pub s: Option<u64>,
    /// Event name, on dispatches. Borrowed from the frame unless the JSON escapes it.
    #[serde(default, borrow, deserialize_with = "optional_cow")]
    pub t: Option<Cow<'a, str>>,
    /// Event body, kept raw. `"d": null` reads as `None`.
    #[serde(default, borrow)]
    pub d: Option<&'a RawValue>,
}

impl fmt::Debug for Envelope<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("op", &self.op)
            .field("s", &self.s)
            .field("t", &self.t)
            .field("d_len", &self.d.map(|d| d.get().len()))
            .finish()
    }
}

/// Reads an optional string, borrowing it when the JSON has no escape in it. The blanket `Cow` impl
/// of serde always allocates, and the `borrow` attribute alone does not reach inside an `Option`.
fn optional_cow<'de: 'a, 'a, D: Deserializer<'de>>(d: D) -> Result<Option<Cow<'a, str>>, D::Error> {
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Option<Cow<'de, str>>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string or null")
        }
        fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(self)
        }
        fn visit_borrowed_str<E: DeError>(self, v: &'de str) -> Result<Self::Value, E> {
            Ok(Some(Cow::Borrowed(v)))
        }
        fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(Cow::Owned(v.to_owned())))
        }
    }
    d.deserialize_option(V)
}

/// Op 10 body, the first frame of every connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Hello {
    /// Milliseconds between heartbeats. Required: a defaulted 0 would flood the gateway.
    pub heartbeat_interval: u64,
}

/// Which event a frame holds, known before its body is decoded. Indexes the `needs_decode` table.
///
/// `TypingStart` and `PresenceUpdate` exist so that the queue can shed them under load. Their body
/// is not typed yet, so they decode to [`Dispatch::Unknown`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
#[allow(missing_docs)] // each variant is the PascalCase of the event name
pub enum DispatchKind {
    Ready,
    Resumed,
    GuildCreate,
    GuildUpdate,
    GuildDelete,
    ChannelCreate,
    ChannelUpdate,
    ChannelDelete,
    GuildMemberAdd,
    GuildMemberUpdate,
    GuildMemberRemove,
    MessageCreate,
    MessageUpdate,
    MessageDelete,
    InteractionCreate,
    TypingStart,
    PresenceUpdate,
    MessageDeleteBulk,
    MessageReactionAdd,
    MessageReactionRemove,
    GuildBanAdd,
    GuildBanRemove,
    GuildRoleCreate,
    GuildRoleUpdate,
    GuildRoleDelete,
    ThreadCreate,
    ThreadUpdate,
    ThreadDelete,
    ThreadListSync,
    ThreadMemberUpdate,
    ThreadMembersUpdate,
    WebhooksUpdate,
    GuildMembersChunk,
    VoiceStateUpdate,
    VoiceServerUpdate,
    MessagePollVoteAdd,
    MessagePollVoteRemove,
    GuildEmojisUpdate,
    GuildStickersUpdate,
    GuildScheduledEventCreate,
    GuildScheduledEventUpdate,
    GuildScheduledEventDelete,
    GuildScheduledEventUserAdd,
    GuildScheduledEventUserRemove,
    StageInstanceCreate,
    StageInstanceUpdate,
    StageInstanceDelete,
    AutoModerationRuleCreate,
    AutoModerationRuleUpdate,
    AutoModerationRuleDelete,
    AutoModerationActionExecution,
    GuildAuditLogEntryCreate,
    /// Any event name this build does not know.
    Unknown,
}

impl DispatchKind {
    /// Number of kinds, the size of a table indexed by [`DispatchKind::index`].
    pub const COUNT: usize = 53;

    /// Every kind, in [`DispatchKind::index`] order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Ready,
        Self::Resumed,
        Self::GuildCreate,
        Self::GuildUpdate,
        Self::GuildDelete,
        Self::ChannelCreate,
        Self::ChannelUpdate,
        Self::ChannelDelete,
        Self::GuildMemberAdd,
        Self::GuildMemberUpdate,
        Self::GuildMemberRemove,
        Self::MessageCreate,
        Self::MessageUpdate,
        Self::MessageDelete,
        Self::InteractionCreate,
        Self::TypingStart,
        Self::PresenceUpdate,
        Self::MessageDeleteBulk,
        Self::MessageReactionAdd,
        Self::MessageReactionRemove,
        Self::GuildBanAdd,
        Self::GuildBanRemove,
        Self::GuildRoleCreate,
        Self::GuildRoleUpdate,
        Self::GuildRoleDelete,
        Self::ThreadCreate,
        Self::ThreadUpdate,
        Self::ThreadDelete,
        Self::ThreadListSync,
        Self::ThreadMemberUpdate,
        Self::ThreadMembersUpdate,
        Self::WebhooksUpdate,
        Self::GuildMembersChunk,
        Self::VoiceStateUpdate,
        Self::VoiceServerUpdate,
        Self::MessagePollVoteAdd,
        Self::MessagePollVoteRemove,
        Self::GuildEmojisUpdate,
        Self::GuildStickersUpdate,
        Self::GuildScheduledEventCreate,
        Self::GuildScheduledEventUpdate,
        Self::GuildScheduledEventDelete,
        Self::GuildScheduledEventUserAdd,
        Self::GuildScheduledEventUserRemove,
        Self::StageInstanceCreate,
        Self::StageInstanceUpdate,
        Self::StageInstanceDelete,
        Self::AutoModerationRuleCreate,
        Self::AutoModerationRuleUpdate,
        Self::AutoModerationRuleDelete,
        Self::AutoModerationActionExecution,
        Self::GuildAuditLogEntryCreate,
        Self::Unknown,
    ];

    /// The kind of an event name. An unknown name is [`DispatchKind::Unknown`].
    pub fn from_t(t: &str) -> Self {
        match t {
            "READY" => Self::Ready,
            "RESUMED" => Self::Resumed,
            "GUILD_CREATE" => Self::GuildCreate,
            "GUILD_UPDATE" => Self::GuildUpdate,
            "GUILD_DELETE" => Self::GuildDelete,
            "CHANNEL_CREATE" => Self::ChannelCreate,
            "CHANNEL_UPDATE" => Self::ChannelUpdate,
            "CHANNEL_DELETE" => Self::ChannelDelete,
            "GUILD_MEMBER_ADD" => Self::GuildMemberAdd,
            "GUILD_MEMBER_UPDATE" => Self::GuildMemberUpdate,
            "GUILD_MEMBER_REMOVE" => Self::GuildMemberRemove,
            "MESSAGE_CREATE" => Self::MessageCreate,
            "MESSAGE_UPDATE" => Self::MessageUpdate,
            "MESSAGE_DELETE" => Self::MessageDelete,
            "INTERACTION_CREATE" => Self::InteractionCreate,
            "TYPING_START" => Self::TypingStart,
            "PRESENCE_UPDATE" => Self::PresenceUpdate,
            "MESSAGE_DELETE_BULK" => Self::MessageDeleteBulk,
            "MESSAGE_REACTION_ADD" => Self::MessageReactionAdd,
            "MESSAGE_REACTION_REMOVE" => Self::MessageReactionRemove,
            "GUILD_BAN_ADD" => Self::GuildBanAdd,
            "GUILD_BAN_REMOVE" => Self::GuildBanRemove,
            "GUILD_ROLE_CREATE" => Self::GuildRoleCreate,
            "GUILD_ROLE_UPDATE" => Self::GuildRoleUpdate,
            "GUILD_ROLE_DELETE" => Self::GuildRoleDelete,
            "THREAD_CREATE" => Self::ThreadCreate,
            "THREAD_UPDATE" => Self::ThreadUpdate,
            "THREAD_DELETE" => Self::ThreadDelete,
            "THREAD_LIST_SYNC" => Self::ThreadListSync,
            "THREAD_MEMBER_UPDATE" => Self::ThreadMemberUpdate,
            "THREAD_MEMBERS_UPDATE" => Self::ThreadMembersUpdate,
            "WEBHOOKS_UPDATE" => Self::WebhooksUpdate,
            "GUILD_MEMBERS_CHUNK" => Self::GuildMembersChunk,
            "VOICE_STATE_UPDATE" => Self::VoiceStateUpdate,
            "VOICE_SERVER_UPDATE" => Self::VoiceServerUpdate,
            "MESSAGE_POLL_VOTE_ADD" => Self::MessagePollVoteAdd,
            "MESSAGE_POLL_VOTE_REMOVE" => Self::MessagePollVoteRemove,
            "GUILD_EMOJIS_UPDATE" => Self::GuildEmojisUpdate,
            "GUILD_STICKERS_UPDATE" => Self::GuildStickersUpdate,
            "GUILD_SCHEDULED_EVENT_CREATE" => Self::GuildScheduledEventCreate,
            "GUILD_SCHEDULED_EVENT_UPDATE" => Self::GuildScheduledEventUpdate,
            "GUILD_SCHEDULED_EVENT_DELETE" => Self::GuildScheduledEventDelete,
            "GUILD_SCHEDULED_EVENT_USER_ADD" => Self::GuildScheduledEventUserAdd,
            "GUILD_SCHEDULED_EVENT_USER_REMOVE" => Self::GuildScheduledEventUserRemove,
            "STAGE_INSTANCE_CREATE" => Self::StageInstanceCreate,
            "STAGE_INSTANCE_UPDATE" => Self::StageInstanceUpdate,
            "STAGE_INSTANCE_DELETE" => Self::StageInstanceDelete,
            "AUTO_MODERATION_RULE_CREATE" => Self::AutoModerationRuleCreate,
            "AUTO_MODERATION_RULE_UPDATE" => Self::AutoModerationRuleUpdate,
            "AUTO_MODERATION_RULE_DELETE" => Self::AutoModerationRuleDelete,
            "AUTO_MODERATION_ACTION_EXECUTION" => Self::AutoModerationActionExecution,
            "GUILD_AUDIT_LOG_ENTRY_CREATE" => Self::GuildAuditLogEntryCreate,
            _ => Self::Unknown,
        }
    }

    /// The event name of this kind, or `None` for [`DispatchKind::Unknown`].
    pub const fn name(self) -> Option<&'static str> {
        Some(match self {
            Self::Ready => "READY",
            Self::Resumed => "RESUMED",
            Self::GuildCreate => "GUILD_CREATE",
            Self::GuildUpdate => "GUILD_UPDATE",
            Self::GuildDelete => "GUILD_DELETE",
            Self::ChannelCreate => "CHANNEL_CREATE",
            Self::ChannelUpdate => "CHANNEL_UPDATE",
            Self::ChannelDelete => "CHANNEL_DELETE",
            Self::GuildMemberAdd => "GUILD_MEMBER_ADD",
            Self::GuildMemberUpdate => "GUILD_MEMBER_UPDATE",
            Self::GuildMemberRemove => "GUILD_MEMBER_REMOVE",
            Self::MessageCreate => "MESSAGE_CREATE",
            Self::MessageUpdate => "MESSAGE_UPDATE",
            Self::MessageDelete => "MESSAGE_DELETE",
            Self::InteractionCreate => "INTERACTION_CREATE",
            Self::TypingStart => "TYPING_START",
            Self::PresenceUpdate => "PRESENCE_UPDATE",
            Self::MessageDeleteBulk => "MESSAGE_DELETE_BULK",
            Self::MessageReactionAdd => "MESSAGE_REACTION_ADD",
            Self::MessageReactionRemove => "MESSAGE_REACTION_REMOVE",
            Self::GuildBanAdd => "GUILD_BAN_ADD",
            Self::GuildBanRemove => "GUILD_BAN_REMOVE",
            Self::GuildRoleCreate => "GUILD_ROLE_CREATE",
            Self::GuildRoleUpdate => "GUILD_ROLE_UPDATE",
            Self::GuildRoleDelete => "GUILD_ROLE_DELETE",
            Self::ThreadCreate => "THREAD_CREATE",
            Self::ThreadUpdate => "THREAD_UPDATE",
            Self::ThreadDelete => "THREAD_DELETE",
            Self::ThreadListSync => "THREAD_LIST_SYNC",
            Self::ThreadMemberUpdate => "THREAD_MEMBER_UPDATE",
            Self::ThreadMembersUpdate => "THREAD_MEMBERS_UPDATE",
            Self::WebhooksUpdate => "WEBHOOKS_UPDATE",
            Self::GuildMembersChunk => "GUILD_MEMBERS_CHUNK",
            Self::VoiceStateUpdate => "VOICE_STATE_UPDATE",
            Self::VoiceServerUpdate => "VOICE_SERVER_UPDATE",
            Self::MessagePollVoteAdd => "MESSAGE_POLL_VOTE_ADD",
            Self::MessagePollVoteRemove => "MESSAGE_POLL_VOTE_REMOVE",
            Self::GuildEmojisUpdate => "GUILD_EMOJIS_UPDATE",
            Self::GuildStickersUpdate => "GUILD_STICKERS_UPDATE",
            Self::GuildScheduledEventCreate => "GUILD_SCHEDULED_EVENT_CREATE",
            Self::GuildScheduledEventUpdate => "GUILD_SCHEDULED_EVENT_UPDATE",
            Self::GuildScheduledEventDelete => "GUILD_SCHEDULED_EVENT_DELETE",
            Self::GuildScheduledEventUserAdd => "GUILD_SCHEDULED_EVENT_USER_ADD",
            Self::GuildScheduledEventUserRemove => "GUILD_SCHEDULED_EVENT_USER_REMOVE",
            Self::StageInstanceCreate => "STAGE_INSTANCE_CREATE",
            Self::StageInstanceUpdate => "STAGE_INSTANCE_UPDATE",
            Self::StageInstanceDelete => "STAGE_INSTANCE_DELETE",
            Self::AutoModerationRuleCreate => "AUTO_MODERATION_RULE_CREATE",
            Self::AutoModerationRuleUpdate => "AUTO_MODERATION_RULE_UPDATE",
            Self::AutoModerationRuleDelete => "AUTO_MODERATION_RULE_DELETE",
            Self::AutoModerationActionExecution => "AUTO_MODERATION_ACTION_EXECUTION",
            Self::GuildAuditLogEntryCreate => "GUILD_AUDIT_LOG_ENTRY_CREATE",
            Self::Unknown => return None,
        })
    }

    /// Position in a table of [`DispatchKind::COUNT`] entries.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// True for events that may be dropped under load. The sequence still advances for them.
    pub const fn is_sheddable(self) -> bool {
        matches!(self, Self::TypingStart | Self::PresenceUpdate)
    }
}

/// A decoded gateway dispatch.
///
/// `Debug` prints the length of an `Unknown` body, not its text, since an event with no typed arm
/// can hold a token, as `VOICE_SERVER_UPDATE` does.
#[derive(Clone)]
#[non_exhaustive]
#[allow(missing_docs)] // each arm is the PascalCase of the event name
pub enum Dispatch {
    Ready(Box<ReadyEvent>),
    Resumed,
    GuildCreate(Box<Guild>),
    GuildUpdate(Box<Guild>),
    GuildDelete(UnavailableGuild),
    ChannelCreate(Box<Channel>),
    ChannelUpdate(Box<Channel>),
    ChannelDelete(Box<Channel>),
    GuildMemberAdd(Box<Member>),
    GuildMemberUpdate(Box<GuildMemberUpdateEvent>),
    GuildMemberRemove(Box<GuildMemberRemoveEvent>),
    MessageCreate(Box<Message>),
    MessageUpdate(Box<MessageUpdateEvent>),
    MessageDelete(Box<MessageDeleteEvent>),
    InteractionCreate(Box<Interaction>),
    MessageDeleteBulk(Box<MessageDeleteBulkEvent>),
    MessageReactionAdd(Box<MessageReactionAddEvent>),
    MessageReactionRemove(Box<MessageReactionRemoveEvent>),
    GuildBanAdd(Box<GuildBanEvent>),
    GuildBanRemove(Box<GuildBanEvent>),
    GuildRoleCreate(Box<GuildRoleEvent>),
    GuildRoleUpdate(Box<GuildRoleEvent>),
    GuildRoleDelete(Box<GuildRoleDeleteEvent>),
    ThreadCreate(Box<Channel>),
    ThreadUpdate(Box<Channel>),
    ThreadDelete(Box<ThreadDeleteEvent>),
    ThreadListSync(Box<ThreadListSyncEvent>),
    ThreadMemberUpdate(Box<ThreadMemberUpdateEvent>),
    ThreadMembersUpdate(Box<ThreadMembersUpdateEvent>),
    WebhooksUpdate(Box<WebhooksUpdateEvent>),
    GuildMembersChunk(Box<GuildMembersChunkEvent>),
    VoiceStateUpdate(Box<VoiceState>),
    VoiceServerUpdate(Box<VoiceServerUpdateEvent>),
    MessagePollVoteAdd(Box<MessagePollVoteEvent>),
    MessagePollVoteRemove(Box<MessagePollVoteEvent>),
    GuildEmojisUpdate(Box<GuildEmojisUpdateEvent>),
    GuildStickersUpdate(Box<GuildStickersUpdateEvent>),
    GuildScheduledEventCreate(Box<GuildScheduledEvent>),
    GuildScheduledEventUpdate(Box<GuildScheduledEvent>),
    GuildScheduledEventDelete(Box<GuildScheduledEvent>),
    GuildScheduledEventUserAdd(Box<ScheduledEventUserEvent>),
    GuildScheduledEventUserRemove(Box<ScheduledEventUserEvent>),
    StageInstanceCreate(Box<StageInstance>),
    StageInstanceUpdate(Box<StageInstance>),
    StageInstanceDelete(Box<StageInstance>),
    AutoModerationRuleCreate(Box<AutoModRule>),
    AutoModerationRuleUpdate(Box<AutoModRule>),
    AutoModerationRuleDelete(Box<AutoModRule>),
    AutoModerationActionExecution(Box<AutoModActionExecutionEvent>),
    GuildAuditLogEntryCreate(Box<AuditLogEntryCreateEvent>),
    /// An event with no typed arm. Keeps its name and raw JSON.
    Unknown {
        /// Event name.
        t: String,
        /// Event body as received.
        raw: Box<RawValue>,
    },
}

impl Dispatch {
    /// Decodes an event body. `kind` must come from `DispatchKind::from_t(t)`, and `d` is the raw
    /// body. A failure is `DecodeFailed` for the caller: the shard continues.
    ///
    /// The body of `RESUMED` is not read: upstream declares none, and Discord sends a trace there. An
    /// event with no typed arm and an empty `d` gets a raw `null`, so that it still arrives.
    pub fn decode(kind: DispatchKind, t: &str, d: &str) -> Result<Self, DecodeError> {
        Ok(match kind {
            DispatchKind::Ready => Self::Ready(decode(d)?),
            DispatchKind::Resumed => Self::Resumed,
            DispatchKind::GuildCreate => Self::GuildCreate(decode(d)?),
            DispatchKind::GuildUpdate => Self::GuildUpdate(decode(d)?),
            DispatchKind::GuildDelete => Self::GuildDelete(decode(d)?),
            DispatchKind::ChannelCreate => Self::ChannelCreate(decode(d)?),
            DispatchKind::ChannelUpdate => Self::ChannelUpdate(decode(d)?),
            DispatchKind::ChannelDelete => Self::ChannelDelete(decode(d)?),
            DispatchKind::GuildMemberAdd => Self::GuildMemberAdd(decode_member_add(d)?),
            DispatchKind::GuildMemberUpdate => Self::GuildMemberUpdate(decode(d)?),
            DispatchKind::GuildMemberRemove => Self::GuildMemberRemove(decode(d)?),
            DispatchKind::MessageCreate => Self::MessageCreate(decode(d)?),
            DispatchKind::MessageUpdate => Self::MessageUpdate(decode(d)?),
            DispatchKind::MessageDelete => Self::MessageDelete(decode(d)?),
            DispatchKind::InteractionCreate => Self::InteractionCreate(decode(d)?),
            DispatchKind::MessageDeleteBulk => Self::MessageDeleteBulk(decode(d)?),
            DispatchKind::MessageReactionAdd => Self::MessageReactionAdd(decode(d)?),
            DispatchKind::MessageReactionRemove => Self::MessageReactionRemove(decode(d)?),
            DispatchKind::GuildBanAdd => Self::GuildBanAdd(decode(d)?),
            DispatchKind::GuildBanRemove => Self::GuildBanRemove(decode(d)?),
            DispatchKind::GuildRoleCreate => Self::GuildRoleCreate(decode(d)?),
            DispatchKind::GuildRoleUpdate => Self::GuildRoleUpdate(decode(d)?),
            DispatchKind::GuildRoleDelete => Self::GuildRoleDelete(decode(d)?),
            DispatchKind::ThreadCreate => Self::ThreadCreate(decode(d)?),
            DispatchKind::ThreadUpdate => Self::ThreadUpdate(decode(d)?),
            DispatchKind::ThreadDelete => Self::ThreadDelete(decode(d)?),
            DispatchKind::ThreadListSync => Self::ThreadListSync(decode(d)?),
            DispatchKind::ThreadMemberUpdate => Self::ThreadMemberUpdate(decode(d)?),
            DispatchKind::ThreadMembersUpdate => Self::ThreadMembersUpdate(decode(d)?),
            DispatchKind::WebhooksUpdate => Self::WebhooksUpdate(decode(d)?),
            DispatchKind::GuildMembersChunk => Self::GuildMembersChunk(decode(d)?),
            DispatchKind::VoiceStateUpdate => Self::VoiceStateUpdate(decode(d)?),
            DispatchKind::VoiceServerUpdate => Self::VoiceServerUpdate(decode(d)?),
            DispatchKind::MessagePollVoteAdd => Self::MessagePollVoteAdd(decode(d)?),
            DispatchKind::MessagePollVoteRemove => Self::MessagePollVoteRemove(decode(d)?),
            DispatchKind::GuildEmojisUpdate => Self::GuildEmojisUpdate(decode(d)?),
            DispatchKind::GuildStickersUpdate => Self::GuildStickersUpdate(decode(d)?),
            DispatchKind::GuildScheduledEventCreate => Self::GuildScheduledEventCreate(decode(d)?),
            DispatchKind::GuildScheduledEventUpdate => Self::GuildScheduledEventUpdate(decode(d)?),
            DispatchKind::GuildScheduledEventDelete => Self::GuildScheduledEventDelete(decode(d)?),
            DispatchKind::GuildScheduledEventUserAdd => Self::GuildScheduledEventUserAdd(decode(d)?),
            DispatchKind::GuildScheduledEventUserRemove => Self::GuildScheduledEventUserRemove(decode(d)?),
            DispatchKind::StageInstanceCreate => Self::StageInstanceCreate(decode(d)?),
            DispatchKind::StageInstanceUpdate => Self::StageInstanceUpdate(decode(d)?),
            DispatchKind::StageInstanceDelete => Self::StageInstanceDelete(decode(d)?),
            DispatchKind::AutoModerationRuleCreate => Self::AutoModerationRuleCreate(decode(d)?),
            DispatchKind::AutoModerationRuleUpdate => Self::AutoModerationRuleUpdate(decode(d)?),
            DispatchKind::AutoModerationRuleDelete => Self::AutoModerationRuleDelete(decode(d)?),
            DispatchKind::AutoModerationActionExecution => Self::AutoModerationActionExecution(decode(d)?),
            DispatchKind::GuildAuditLogEntryCreate => Self::GuildAuditLogEntryCreate(decode(d)?),
            DispatchKind::TypingStart | DispatchKind::PresenceUpdate | DispatchKind::Unknown => {
                let raw = if d.is_empty() { RawValue::NULL.to_owned() } else { decode(d)? };
                Self::Unknown { t: t.to_owned(), raw }
            }
        })
    }

    /// The kind of this event. An `Unknown` arm reports the kind of its name, so `TYPING_START`, which
    /// has no typed arm, still answers [`DispatchKind::TypingStart`].
    pub fn kind(&self) -> DispatchKind {
        match self {
            Self::Ready(_) => DispatchKind::Ready,
            Self::Resumed => DispatchKind::Resumed,
            Self::GuildCreate(_) => DispatchKind::GuildCreate,
            Self::GuildUpdate(_) => DispatchKind::GuildUpdate,
            Self::GuildDelete(_) => DispatchKind::GuildDelete,
            Self::ChannelCreate(_) => DispatchKind::ChannelCreate,
            Self::ChannelUpdate(_) => DispatchKind::ChannelUpdate,
            Self::ChannelDelete(_) => DispatchKind::ChannelDelete,
            Self::GuildMemberAdd(_) => DispatchKind::GuildMemberAdd,
            Self::GuildMemberUpdate(_) => DispatchKind::GuildMemberUpdate,
            Self::GuildMemberRemove(_) => DispatchKind::GuildMemberRemove,
            Self::MessageCreate(_) => DispatchKind::MessageCreate,
            Self::MessageUpdate(_) => DispatchKind::MessageUpdate,
            Self::MessageDelete(_) => DispatchKind::MessageDelete,
            Self::InteractionCreate(_) => DispatchKind::InteractionCreate,
            Self::MessageDeleteBulk(_) => DispatchKind::MessageDeleteBulk,
            Self::MessageReactionAdd(_) => DispatchKind::MessageReactionAdd,
            Self::MessageReactionRemove(_) => DispatchKind::MessageReactionRemove,
            Self::GuildBanAdd(_) => DispatchKind::GuildBanAdd,
            Self::GuildBanRemove(_) => DispatchKind::GuildBanRemove,
            Self::GuildRoleCreate(_) => DispatchKind::GuildRoleCreate,
            Self::GuildRoleUpdate(_) => DispatchKind::GuildRoleUpdate,
            Self::GuildRoleDelete(_) => DispatchKind::GuildRoleDelete,
            Self::ThreadCreate(_) => DispatchKind::ThreadCreate,
            Self::ThreadUpdate(_) => DispatchKind::ThreadUpdate,
            Self::ThreadDelete(_) => DispatchKind::ThreadDelete,
            Self::ThreadListSync(_) => DispatchKind::ThreadListSync,
            Self::ThreadMemberUpdate(_) => DispatchKind::ThreadMemberUpdate,
            Self::ThreadMembersUpdate(_) => DispatchKind::ThreadMembersUpdate,
            Self::WebhooksUpdate(_) => DispatchKind::WebhooksUpdate,
            Self::GuildMembersChunk(_) => DispatchKind::GuildMembersChunk,
            Self::VoiceStateUpdate(_) => DispatchKind::VoiceStateUpdate,
            Self::VoiceServerUpdate(_) => DispatchKind::VoiceServerUpdate,
            Self::MessagePollVoteAdd(_) => DispatchKind::MessagePollVoteAdd,
            Self::MessagePollVoteRemove(_) => DispatchKind::MessagePollVoteRemove,
            Self::GuildEmojisUpdate(_) => DispatchKind::GuildEmojisUpdate,
            Self::GuildStickersUpdate(_) => DispatchKind::GuildStickersUpdate,
            Self::GuildScheduledEventCreate(_) => DispatchKind::GuildScheduledEventCreate,
            Self::GuildScheduledEventUpdate(_) => DispatchKind::GuildScheduledEventUpdate,
            Self::GuildScheduledEventDelete(_) => DispatchKind::GuildScheduledEventDelete,
            Self::GuildScheduledEventUserAdd(_) => DispatchKind::GuildScheduledEventUserAdd,
            Self::GuildScheduledEventUserRemove(_) => DispatchKind::GuildScheduledEventUserRemove,
            Self::StageInstanceCreate(_) => DispatchKind::StageInstanceCreate,
            Self::StageInstanceUpdate(_) => DispatchKind::StageInstanceUpdate,
            Self::StageInstanceDelete(_) => DispatchKind::StageInstanceDelete,
            Self::AutoModerationRuleCreate(_) => DispatchKind::AutoModerationRuleCreate,
            Self::AutoModerationRuleUpdate(_) => DispatchKind::AutoModerationRuleUpdate,
            Self::AutoModerationRuleDelete(_) => DispatchKind::AutoModerationRuleDelete,
            Self::AutoModerationActionExecution(_) => DispatchKind::AutoModerationActionExecution,
            Self::GuildAuditLogEntryCreate(_) => DispatchKind::GuildAuditLogEntryCreate,
            Self::Unknown { t, .. } => DispatchKind::from_t(t),
        }
    }
}

/// `Member` defaults every field, because it is also the partial member nested in a message. Upstream
/// requires `guild_id` and `user` on `GUILD_MEMBER_ADD`, so they are checked here: without them
/// nothing could key the member, and a required field that is gone is a decode failure.
fn decode_member_add(d: &str) -> Result<Box<Member>, DecodeError> {
    let member: Box<Member> = decode(d)?;
    let missing = match (&member.guild_id, &member.user) {
        (None, _) => "guild_id",
        (_, None) => "user",
        _ => return Ok(member),
    };
    Err(DecodeError { path: missing.to_owned(), source: DeError::missing_field(missing) })
}

impl fmt::Debug for Dispatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn arm(f: &mut fmt::Formatter<'_>, name: &str, body: &dyn fmt::Debug) -> fmt::Result {
            f.debug_tuple(name).field(body).finish()
        }
        match self {
            Self::Ready(v) => arm(f, "Ready", v),
            Self::Resumed => f.write_str("Resumed"),
            Self::GuildCreate(v) => arm(f, "GuildCreate", v),
            Self::GuildUpdate(v) => arm(f, "GuildUpdate", v),
            Self::GuildDelete(v) => arm(f, "GuildDelete", v),
            Self::ChannelCreate(v) => arm(f, "ChannelCreate", v),
            Self::ChannelUpdate(v) => arm(f, "ChannelUpdate", v),
            Self::ChannelDelete(v) => arm(f, "ChannelDelete", v),
            Self::GuildMemberAdd(v) => arm(f, "GuildMemberAdd", v),
            Self::GuildMemberUpdate(v) => arm(f, "GuildMemberUpdate", v),
            Self::GuildMemberRemove(v) => arm(f, "GuildMemberRemove", v),
            Self::MessageCreate(v) => arm(f, "MessageCreate", v),
            Self::MessageUpdate(v) => arm(f, "MessageUpdate", v),
            Self::MessageDelete(v) => arm(f, "MessageDelete", v),
            Self::InteractionCreate(v) => arm(f, "InteractionCreate", v),
            Self::MessageDeleteBulk(v) => arm(f, "MessageDeleteBulk", v),
            Self::MessageReactionAdd(v) => arm(f, "MessageReactionAdd", v),
            Self::MessageReactionRemove(v) => arm(f, "MessageReactionRemove", v),
            Self::GuildBanAdd(v) => arm(f, "GuildBanAdd", v),
            Self::GuildBanRemove(v) => arm(f, "GuildBanRemove", v),
            Self::GuildRoleCreate(v) => arm(f, "GuildRoleCreate", v),
            Self::GuildRoleUpdate(v) => arm(f, "GuildRoleUpdate", v),
            Self::GuildRoleDelete(v) => arm(f, "GuildRoleDelete", v),
            Self::ThreadCreate(v) => arm(f, "ThreadCreate", v),
            Self::ThreadUpdate(v) => arm(f, "ThreadUpdate", v),
            Self::ThreadDelete(v) => arm(f, "ThreadDelete", v),
            Self::ThreadListSync(v) => arm(f, "ThreadListSync", v),
            Self::ThreadMemberUpdate(v) => arm(f, "ThreadMemberUpdate", v),
            Self::ThreadMembersUpdate(v) => arm(f, "ThreadMembersUpdate", v),
            Self::WebhooksUpdate(v) => arm(f, "WebhooksUpdate", v),
            Self::GuildMembersChunk(v) => arm(f, "GuildMembersChunk", v),
            Self::VoiceStateUpdate(v) => arm(f, "VoiceStateUpdate", v),
            Self::VoiceServerUpdate(v) => arm(f, "VoiceServerUpdate", v),
            Self::MessagePollVoteAdd(v) => arm(f, "MessagePollVoteAdd", v),
            Self::MessagePollVoteRemove(v) => arm(f, "MessagePollVoteRemove", v),
            Self::GuildEmojisUpdate(v) => arm(f, "GuildEmojisUpdate", v),
            Self::GuildStickersUpdate(v) => arm(f, "GuildStickersUpdate", v),
            Self::GuildScheduledEventCreate(v) => arm(f, "GuildScheduledEventCreate", v),
            Self::GuildScheduledEventUpdate(v) => arm(f, "GuildScheduledEventUpdate", v),
            Self::GuildScheduledEventDelete(v) => arm(f, "GuildScheduledEventDelete", v),
            Self::GuildScheduledEventUserAdd(v) => arm(f, "GuildScheduledEventUserAdd", v),
            Self::GuildScheduledEventUserRemove(v) => arm(f, "GuildScheduledEventUserRemove", v),
            Self::StageInstanceCreate(v) => arm(f, "StageInstanceCreate", v),
            Self::StageInstanceUpdate(v) => arm(f, "StageInstanceUpdate", v),
            Self::StageInstanceDelete(v) => arm(f, "StageInstanceDelete", v),
            Self::AutoModerationRuleCreate(v) => arm(f, "AutoModerationRuleCreate", v),
            Self::AutoModerationRuleUpdate(v) => arm(f, "AutoModerationRuleUpdate", v),
            Self::AutoModerationRuleDelete(v) => arm(f, "AutoModerationRuleDelete", v),
            Self::AutoModerationActionExecution(v) => arm(f, "AutoModerationActionExecution", v),
            Self::GuildAuditLogEntryCreate(v) => arm(f, "GuildAuditLogEntryCreate", v),
            Self::Unknown { t, raw } => {
                f.debug_struct("Unknown").field("t", t).field("raw_len", &raw.get().len()).finish()
            }
        }
    }
}

/// Body of `READY`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReadyEvent {
    /// API version.
    #[serde(default)]
    pub v: u32,
    /// The bot's account.
    pub user: User,
    /// Guilds to expect, all unavailable until their `GUILD_CREATE`.
    #[serde(default, deserialize_with = "null_default")]
    pub guilds: Vec<UnavailableGuild>,
    /// Session to resume. Redacted in `Debug`.
    pub session_id: Secret,
    /// Where to resume.
    #[serde(default)]
    pub resume_gateway_url: String,
    /// `[shard_id, shard_count]`, sent when the identify named a shard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard: Option<[u32; 2]>,
    /// The application, source of the id for command registration.
    pub application: ReadyApplication,
}

/// The partial application in `READY`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReadyApplication {
    /// Application id.
    pub id: ApplicationId,
    /// Flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<ApplicationFlags>,
    /// The same concept as `flags`, as a decimal string. Kept as text, so that no value can fail the
    /// event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags_new: Option<String>,
}

/// Body of `MESSAGE_UPDATE`. Only `id` and `channel_id` are guaranteed: a field that is `None` or
/// `Absent` was not in the payload, and the cache keeps the old value for it.
///
/// Upstream types this as a full message, and discord.js patches it field by field with
/// `'x' in data`, because Discord also sends partial bodies, such as embeds only. It reads every
/// field that [`Message`] holds, so a stored message cannot go stale, and like [`Message`] leaves out
/// fields of cut features, such as `poll`, `sticker_items` and `thread`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageUpdateEvent {
    /// Message id.
    pub id: MessageId,
    /// Channel id.
    pub channel_id: ChannelId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Author, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<User>,
    /// Member data of the author, when sent. It has no `user`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// New text, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Send time, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// Edit time. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub edited_timestamp: Patch<Timestamp>,
    /// Message kind, when sent.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "type")]
    pub kind: Option<MessageType>,
    /// Kind of the channel it is in, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<ChannelType>,
    /// Text to speech, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
    /// Whether it mentions everyone, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_everyone: Option<bool>,
    /// Mentioned users, when sent. Upstream nests a `member` in each, which is not read here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentions: Option<Vec<User>>,
    /// Mentioned roles, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_roles: Option<Vec<RoleId>>,
    /// New embeds, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Vec<Embed>>,
    /// New attachments, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    /// Reactions, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<Reaction>>,
    /// New components, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    /// New flags, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// New pin state, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    /// Nonce, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Nonce>,
    /// Webhook that sent it, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<WebhookId>,
    /// Application that sent it, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// What this message replies to or forwards, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
    /// The replied-to message: absent means not sent, null means deleted.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub referenced_message: Patch<Box<Message>>,
    /// Mentioned channels, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_channels: Option<Vec<ChannelMention>>,
    /// Rich Presence activity, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<MessageActivity>,
    /// The interaction this message was sent in response to, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_metadata: Option<Box<MessageInteractionMetadata>>,
    /// The older record of that interaction, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Box<MessageInteraction>>,
    /// Approximate position in a thread, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    /// The forwarded message, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_snapshots: Option<Vec<MessageSnapshot>>,
    /// The call this message is about, when sent. This is how a call's end arrives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<MessageCall>,
    /// A custom client theme shared through the message, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_client_theme: Option<MessageSharedClientTheme>,
    /// The objects the auto-populated selects of this message name, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Box<ResolvedData>>,
    /// The poll, when sent. Discord sends it with the final results once the poll ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<Box<crate::Poll>>,
}

/// `GUILD_EMOJIS_UPDATE`: a guild's emojis changed. It holds the whole new list.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildEmojisUpdateEvent {
    /// The guild.
    pub guild_id: GuildId,
    /// Every emoji the guild has now.
    #[serde(default, deserialize_with = "dr_core::null_default")]
    pub emojis: Vec<Emoji>,
}

/// Body of `MESSAGE_DELETE_BULK`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageDeleteBulkEvent {
    /// The deleted messages.
    pub ids: Vec<MessageId>,
    /// Channel id.
    pub channel_id: ChannelId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

/// Body of `MESSAGE_REACTION_ADD`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageReactionAddEvent {
    /// Who reacted.
    pub user_id: UserId,
    /// Channel id.
    pub channel_id: ChannelId,
    /// The message reacted to.
    pub message_id: MessageId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The member who reacted, in a guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// The emoji.
    pub emoji: Emoji,
    /// Who wrote the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_author_id: Option<UserId>,
    /// Whether it is a super reaction.
    #[serde(default)]
    pub burst: bool,
}

/// Body of `MESSAGE_REACTION_REMOVE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageReactionRemoveEvent {
    /// Who removed their reaction.
    pub user_id: UserId,
    /// Channel id.
    pub channel_id: ChannelId,
    /// The message reacted to.
    pub message_id: MessageId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The emoji.
    pub emoji: Emoji,
    /// Whether it was a super reaction.
    #[serde(default)]
    pub burst: bool,
}

/// Body of `GUILD_BAN_ADD` and `GUILD_BAN_REMOVE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildBanEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The user.
    pub user: User,
}

/// Body of `GUILD_ROLE_CREATE` and `GUILD_ROLE_UPDATE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildRoleEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The role.
    pub role: Role,
}

/// Body of `GUILD_ROLE_DELETE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildRoleDeleteEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The deleted role.
    pub role_id: RoleId,
}

/// Body of `MESSAGE_DELETE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageDeleteEvent {
    /// Message id.
    pub id: MessageId,
    /// Channel id.
    pub channel_id: ChannelId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

/// Body of `GUILD_MEMBER_UPDATE`. It reads every field of [`Member`] but `permissions`, which only
/// interaction payloads have.
///
/// `roles` and `joined_at` keep the shape of [`Member`], so a body that leaves them out reads the
/// same as one that sends them empty or null. Upstream requires both.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildMemberUpdateEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The user.
    pub user: User,
    /// Roles.
    #[serde(default, deserialize_with = "null_default")]
    pub roles: Vec<RoleId>,
    /// Nickname. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub nick: Patch<String>,
    /// Per-guild avatar hash. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub avatar: Patch<String>,
    /// Per-guild banner hash. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub banner: Patch<String>,
    /// Join time, null for a guest.
    #[serde(default)]
    pub joined_at: Option<Timestamp>,
    /// When the member started boosting. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub premium_since: Patch<Timestamp>,
    /// When a timeout ends. Absent versus null matters here.
    #[serde(default, skip_serializing_if = "Patch::is_absent")]
    pub communication_disabled_until: Patch<Timestamp>,
    /// Whether the member is deafened in voice, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deaf: Option<bool>,
    /// Whether the member is muted in voice, when sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    /// Member flags, when sent. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<GuildMemberFlags>,
    /// Whether the member has not passed membership screening.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    /// Per-guild avatar decoration. Absent or null resets it to none, as on [`Member`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// Per-guild collectibles. Absent or null resets them to none, as on [`Member`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
}

/// Body of `GUILD_MEMBER_REMOVE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildMemberRemoveEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The user.
    pub user: User,
}

/// Body of `THREAD_DELETE`: what is left of the thread.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadDeleteEvent {
    /// Thread id.
    pub id: ChannelId,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel the thread was in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ChannelId>,
    /// The thread's type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
}

impl ThreadDeleteEvent {
    /// The deleted thread as a partial channel.
    pub fn to_channel(&self) -> Channel {
        let mut channel = Channel::partial(self.id, self.kind);
        channel.guild_id = self.guild_id;
        channel.parent_id = self.parent_id;
        channel
    }
}

/// Body of `THREAD_LIST_SYNC`: the active threads of a guild, sent when the bot gains access to a
/// channel.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadListSyncEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The parent channels synced. Absent means the whole guild: threads of other channels that the
    /// cache holds are gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_ids: Option<Vec<ChannelId>>,
    /// Every active thread of those channels.
    #[serde(default, deserialize_with = "null_default")]
    pub threads: Vec<Channel>,
    /// The bot's membership of those threads.
    #[serde(default, deserialize_with = "null_default")]
    pub members: Vec<ThreadMember>,
}

/// Body of `THREAD_MEMBER_UPDATE`: the bot's membership of a thread changed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadMemberUpdateEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The membership.
    #[serde(flatten)]
    pub member: ThreadMember,
}

/// Body of `THREAD_MEMBERS_UPDATE`: people joined or left a thread. Members other than the bot need
/// the `GUILD_MEMBERS` intent.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadMembersUpdateEvent {
    /// Thread id.
    pub id: ChannelId,
    /// Guild id.
    pub guild_id: GuildId,
    /// Approximate count, which stops at 50.
    #[serde(default)]
    pub member_count: i64,
    /// Who joined.
    #[serde(default, deserialize_with = "null_default")]
    pub added_members: Vec<ThreadMember>,
    /// Who left.
    #[serde(default, deserialize_with = "null_default")]
    pub removed_member_ids: Vec<UserId>,
}

/// Body of `WEBHOOKS_UPDATE`: a webhook of a channel was created, edited or deleted. It does not say
/// which: fetch the channel's webhooks to know.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebhooksUpdateEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// Channel id.
    pub channel_id: ChannelId,
}

/// Body of `GUILD_MEMBERS_CHUNK`: one part of the answer to a request for guild members (op 8).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildMembersChunkEvent {
    /// Guild id.
    pub guild_id: GuildId,
    /// The members of this chunk.
    #[serde(default, deserialize_with = "null_default")]
    pub members: Vec<Member>,
    /// This chunk's index, from 0.
    #[serde(default)]
    pub chunk_index: u32,
    /// How many chunks answer the request.
    #[serde(default)]
    pub chunk_count: u32,
    /// The requested user ids that are not members.
    #[serde(default, deserialize_with = "null_default")]
    pub not_found: Vec<UserId>,
    /// Their presences, when asked for. Kept as raw JSON: presences are not modeled yet.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub presences: Vec<serde_json::Value>,
    /// The nonce of the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

impl GuildMembersChunkEvent {
    /// Whether this is the last chunk of its request.
    pub fn is_last(&self) -> bool {
        self.chunk_index.saturating_add(1) >= self.chunk_count
    }
}
