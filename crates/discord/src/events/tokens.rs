//! The event tokens you pass to `ClientBuilder::on`.

use super::GatewayEvent;
use crate::interactions::Interaction;
use crate::{Bound, Deleted, Updated};
use dr_model::{
    AuditLogEntryCreateEvent, AutoModActionExecutionEvent, AutoModRule, Channel, Guild, GuildBanEvent,
    GuildEmojisUpdateEvent, GuildMemberRemoveEvent, GuildMembersChunkEvent, GuildRoleDeleteEvent, GuildRoleEvent,
    GuildScheduledEvent, GuildStickersUpdateEvent, Intents, Member, Message, MessageDeleteBulkEvent,
    MessageDeleteEvent, MessagePollVoteEvent, MessageReactionAddEvent, MessageReactionRemoveEvent, ReadyEvent, Role,
    ScheduledEventUserEvent, StageInstance, ThreadDeleteEvent, ThreadListSyncEvent, ThreadMemberUpdateEvent,
    ThreadMembersUpdateEvent, UnavailableGuild, VoiceServerUpdateEvent, VoiceState, WebhooksUpdateEvent,
};

macro_rules! event {
    ($(#[$doc:meta])* $name:ident, $wire:literal, $intents:expr, $payload:ty) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        impl GatewayEvent for $name {
            const NAME: &'static str = $wire;
            const INTENTS: Intents = $intents;
            type Payload = $payload;
        }
    };
}

const MESSAGES: Intents = Intents::GUILD_MESSAGES.union(Intents::DIRECT_MESSAGES);
const POLLS: Intents = Intents::GUILD_MESSAGE_POLLS.union(Intents::DIRECT_MESSAGE_POLLS);
const REACTIONS: Intents = Intents::GUILD_MESSAGE_REACTIONS.union(Intents::DIRECT_MESSAGE_REACTIONS);

event!(
    /// Several messages were deleted at once.
    MessageDeleteBulk, "MESSAGE_DELETE_BULK", Intents::GUILD_MESSAGES, Bound<MessageDeleteBulkEvent>
);
event!(
    /// Someone added a reaction. The cached message's reaction list does not follow.
    MessageReactionAdd, "MESSAGE_REACTION_ADD", REACTIONS, Bound<MessageReactionAddEvent>
);
event!(
    /// Someone removed a reaction. The cached message's reaction list does not follow.
    MessageReactionRemove, "MESSAGE_REACTION_REMOVE", REACTIONS, Bound<MessageReactionRemoveEvent>
);
event!(
    /// A user was banned.
    GuildBanAdd, "GUILD_BAN_ADD", Intents::GUILD_MODERATION, Bound<GuildBanEvent>
);
event!(
    /// A ban was lifted.
    GuildBanRemove, "GUILD_BAN_REMOVE", Intents::GUILD_MODERATION, Bound<GuildBanEvent>
);
event!(
    /// A role was created.
    GuildRoleCreate, "GUILD_ROLE_CREATE", Intents::GUILDS, Bound<GuildRoleEvent>
);
event!(
    /// A role was edited.
    GuildRoleUpdate, "GUILD_ROLE_UPDATE", Intents::GUILDS, Bound<GuildRoleEvent>
);
event!(
    /// A role was deleted. `cached` is the role as it was, if the cache had it.
    GuildRoleDelete, "GUILD_ROLE_DELETE", Intents::GUILDS, Deleted<GuildRoleDeleteEvent, Role>
);
event!(
    /// A thread was created, or the bot was added to a private thread.
    ThreadCreate, "THREAD_CREATE", Intents::GUILDS, Bound<Channel>
);
event!(
    /// A thread was edited, archived or unarchived.
    ThreadUpdate, "THREAD_UPDATE", Intents::GUILDS, Updated<Channel>
);
event!(
    /// A thread was deleted. `cached` is the thread as it was, if the cache had it.
    ThreadDelete, "THREAD_DELETE", Intents::GUILDS, Deleted<ThreadDeleteEvent, Channel>
);
event!(
    /// The bot gained access to channels and receives their active threads.
    ThreadListSync, "THREAD_LIST_SYNC", Intents::GUILDS, Bound<ThreadListSyncEvent>
);
event!(
    /// The bot's membership of a thread changed.
    ThreadMemberUpdate, "THREAD_MEMBER_UPDATE", Intents::GUILDS, Bound<ThreadMemberUpdateEvent>
);
event!(
    /// People joined or left a thread. Members other than the bot need `GUILD_MEMBERS`.
    ThreadMembersUpdate, "THREAD_MEMBERS_UPDATE", Intents::GUILDS.union(Intents::GUILD_MEMBERS),
    Bound<ThreadMembersUpdateEvent>
);
event!(
    /// A webhook of a channel was created, edited or deleted.
    WebhooksUpdate, "WEBHOOKS_UPDATE", Intents::GUILD_WEBHOOKS, Bound<WebhooksUpdateEvent>
);

event!(
    /// The client is ready: once per start, after the guild list finished.
    ClientReady, "CLIENT_READY", Intents::empty(), Bound<ReadyEvent>
);
event!(
    /// A message was sent.
    MessageCreate, "MESSAGE_CREATE", MESSAGES, Bound<Message>
);
event!(
    /// A message was edited. `new` is the old message with the partial event on top, or the partial
    /// event alone when it was not cached.
    MessageUpdate, "MESSAGE_UPDATE", MESSAGES, Updated<Message>
);
event!(
    /// A message was deleted.
    MessageDelete, "MESSAGE_DELETE", MESSAGES, Deleted<MessageDeleteEvent, Message>
);
event!(
    /// The bot joined a guild, or a guild became available.
    GuildCreate, "GUILD_CREATE", Intents::GUILDS, Bound<Guild>
);
event!(
    /// A guild changed.
    GuildUpdate, "GUILD_UPDATE", Intents::GUILDS, Updated<Guild>
);
event!(
    /// The bot left a guild, or a guild became unavailable: see `UnavailableGuild::unavailable`.
    GuildDelete, "GUILD_DELETE", Intents::GUILDS, Deleted<UnavailableGuild, Guild>
);
event!(
    /// A channel was created.
    ChannelCreate, "CHANNEL_CREATE", Intents::GUILDS, Bound<Channel>
);
event!(
    /// A channel changed.
    ChannelUpdate, "CHANNEL_UPDATE", Intents::GUILDS, Updated<Channel>
);
event!(
    /// A channel was deleted.
    ChannelDelete, "CHANNEL_DELETE", Intents::GUILDS, Deleted<Channel, Channel>
);
event!(
    /// A member joined a guild.
    GuildMemberAdd, "GUILD_MEMBER_ADD", Intents::GUILD_MEMBERS, Bound<Member>
);
event!(
    /// A member changed.
    GuildMemberUpdate, "GUILD_MEMBER_UPDATE", Intents::GUILD_MEMBERS, Updated<Member>
);
event!(
    /// A member left a guild.
    GuildMemberRemove, "GUILD_MEMBER_REMOVE", Intents::GUILD_MEMBERS, Deleted<GuildMemberRemoveEvent, Member>
);
event!(
    /// An interaction arrived. Runs after any routed handler from `on_command` and the like.
    InteractionCreate, "INTERACTION_CREATE", Intents::empty(), Interaction
);

event!(
    /// A part of the answer to a request for guild members, such as `members().fetch_all()`. The
    /// members are in the cache before handlers run.
    GuildMembersChunk, "GUILD_MEMBERS_CHUNK", Intents::empty(), Bound<GuildMembersChunkEvent>
);

event!(
    /// Someone joined, left or moved between voice channels, or muted or deafened. `old` is where
    /// they were, when the cache knew.
    VoiceStateUpdate, "VOICE_STATE_UPDATE", Intents::GUILD_VOICE_STATES, Updated<VoiceState>
);

event!(
    /// The bot's voice server for a guild, after it joins or when Discord moves it. The `voice`
    /// feature acts on it by itself.
    VoiceServerUpdate, "VOICE_SERVER_UPDATE", Intents::empty(), Bound<VoiceServerUpdateEvent>
);

event!(
    /// Someone voted for an answer of a poll. The cached message's poll results do not follow.
    MessagePollVoteAdd, "MESSAGE_POLL_VOTE_ADD", POLLS, Bound<MessagePollVoteEvent>
);

event!(
    /// Someone took back their vote for an answer of a poll.
    MessagePollVoteRemove, "MESSAGE_POLL_VOTE_REMOVE", POLLS, Bound<MessagePollVoteEvent>
);

event!(
    /// A guild's emojis changed. The event holds the whole new list, and the cache follows.
    GuildEmojisUpdate, "GUILD_EMOJIS_UPDATE", Intents::GUILD_EXPRESSIONS, Updated<GuildEmojisUpdateEvent>
);

event!(
    /// A guild's stickers changed. The event holds the whole new list, and the cache follows.
    GuildStickersUpdate, "GUILD_STICKERS_UPDATE", Intents::GUILD_EXPRESSIONS, Updated<GuildStickersUpdateEvent>
);

event!(
    /// A scheduled event was created.
    GuildScheduledEventCreate, "GUILD_SCHEDULED_EVENT_CREATE", Intents::GUILD_SCHEDULED_EVENTS, Bound<GuildScheduledEvent>
);

event!(
    /// A scheduled event changed, started or ended. `old` is the event as cached.
    GuildScheduledEventUpdate, "GUILD_SCHEDULED_EVENT_UPDATE", Intents::GUILD_SCHEDULED_EVENTS, Updated<GuildScheduledEvent>
);

event!(
    /// A scheduled event was deleted.
    GuildScheduledEventDelete, "GUILD_SCHEDULED_EVENT_DELETE", Intents::GUILD_SCHEDULED_EVENTS,
    Deleted<GuildScheduledEvent, GuildScheduledEvent>
);

event!(
    /// Someone became interested in a scheduled event.
    GuildScheduledEventUserAdd, "GUILD_SCHEDULED_EVENT_USER_ADD", Intents::GUILD_SCHEDULED_EVENTS,
    Bound<ScheduledEventUserEvent>
);

event!(
    /// Someone was no longer interested in a scheduled event.
    GuildScheduledEventUserRemove, "GUILD_SCHEDULED_EVENT_USER_REMOVE", Intents::GUILD_SCHEDULED_EVENTS,
    Bound<ScheduledEventUserEvent>
);

event!(
    /// A stage channel went live.
    StageInstanceCreate, "STAGE_INSTANCE_CREATE", Intents::GUILDS, Bound<StageInstance>
);

event!(
    /// A live stage changed. `old` is the stage as cached.
    StageInstanceUpdate, "STAGE_INSTANCE_UPDATE", Intents::GUILDS, Updated<StageInstance>
);

event!(
    /// A live stage ended.
    StageInstanceDelete, "STAGE_INSTANCE_DELETE", Intents::GUILDS, Deleted<StageInstance, StageInstance>
);

event!(
    /// An auto moderation rule was created.
    AutoModerationRuleCreate, "AUTO_MODERATION_RULE_CREATE", Intents::AUTO_MODERATION_CONFIGURATION, Bound<AutoModRule>
);

event!(
    /// An auto moderation rule changed. The event holds the new rule only: rules are not cached.
    AutoModerationRuleUpdate, "AUTO_MODERATION_RULE_UPDATE", Intents::AUTO_MODERATION_CONFIGURATION, Bound<AutoModRule>
);

event!(
    /// An auto moderation rule was deleted.
    AutoModerationRuleDelete, "AUTO_MODERATION_RULE_DELETE", Intents::AUTO_MODERATION_CONFIGURATION, Bound<AutoModRule>
);

event!(
    /// An auto moderation rule triggered and ran one of its actions. One event per action.
    AutoModerationActionExecution, "AUTO_MODERATION_ACTION_EXECUTION", Intents::AUTO_MODERATION_EXECUTION,
    Bound<AutoModActionExecutionEvent>
);

event!(
    /// An entry was added to a guild's audit log. Needs `VIEW_AUDIT_LOG` as well as the intent.
    GuildAuditLogEntryCreate, "GUILD_AUDIT_LOG_ENTRY_CREATE", Intents::GUILD_MODERATION, Bound<AuditLogEntryCreateEvent>
);
