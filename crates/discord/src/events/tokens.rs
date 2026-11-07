//! The event tokens you pass to `ClientBuilder::on`.

use super::GatewayEvent;
use crate::interactions::Interaction;
use crate::{Bound, Deleted, Updated};
use dr_model::{
    Channel, Guild, GuildMemberRemoveEvent, Intents, Member, Message, MessageDeleteEvent, ReadyEvent, UnavailableGuild,
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
