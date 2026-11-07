//! `GuildHandle` and `Bound<Guild>`.

use crate::managers::{
    AutoModManager, BansManager, EmojisManager, MembersManager, RolesManager, ScheduledEventsManager, StickersManager,
};
use crate::{Bound, Call, Client};
use dr_builders::{ChannelPosition, CreateChannel};
use dr_core::RequestBody;
use dr_model::{Channel, Guild, GuildId, Permissions, ThreadList, Webhook};

/// A guild by id: a client and an id, no I/O to create.
#[derive(Clone, Debug)]
pub struct GuildHandle {
    pub(crate) client: Client,
    pub(crate) id: GuildId,
}

impl Client {
    /// A handle to a guild. No I/O.
    pub fn guild(&self, id: GuildId) -> GuildHandle {
        GuildHandle { client: self.clone(), id }
    }
}

impl GuildHandle {
    /// The id.
    pub fn id(&self) -> GuildId {
        self.id
    }

    /// The guild from the cache. No I/O.
    pub fn cached(&self) -> Option<Bound<Guild>> {
        self.client.cache().guild(self.id).map(|g| self.client.bind_arc(g))
    }

    /// Fetches the guild from Discord.
    pub fn fetch(&self) -> Call<Bound<Guild>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get(self.id))
    }

    /// The members of this guild.
    pub fn members(&self) -> MembersManager {
        MembersManager::new(self.client.clone(), self.id)
    }

    /// The bans of this guild.
    pub fn bans(&self) -> BansManager {
        BansManager::new(self.client.clone(), self.id)
    }

    /// The roles of this guild.
    pub fn roles(&self) -> RolesManager {
        RolesManager::new(self.client.clone(), self.id)
    }

    /// One page of this guild's audit log. Needs `VIEW_AUDIT_LOG`.
    pub fn audit_log(&self, query: dr_rest::api::AuditLogQuery) -> Call<Bound<dr_model::AuditLog>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_audit_log(self.id, query))
    }

    /// The auto moderation rules of this guild.
    pub fn automod(&self) -> AutoModManager {
        AutoModManager::new(self.client.clone(), self.id)
    }

    /// The custom emojis of this guild.
    pub fn emojis(&self) -> EmojisManager {
        EmojisManager::new(self.client.clone(), self.id)
    }

    /// The scheduled events of this guild.
    pub fn scheduled_events(&self) -> ScheduledEventsManager {
        ScheduledEventsManager::new(self.client.clone(), self.id)
    }

    /// The stickers of this guild.
    pub fn stickers(&self) -> StickersManager {
        StickersManager::new(self.client.clone(), self.id)
    }

    /// Fetches the channels of this guild from Discord.
    pub fn fetch_channels(&self) -> Call<Vec<Bound<Channel>>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_channels(self.id))
    }

    /// Every webhook of this guild, from Discord. Needs `MANAGE_WEBHOOKS`.
    pub fn webhooks(&self) -> Call<Vec<Bound<Webhook>>> {
        Call::new(self.client.clone(), self.client.rest().webhooks().get_guild_webhooks(self.id))
    }

    /// Every active thread of this guild that the bot can see, from Discord.
    pub fn active_threads(&self) -> Call<Bound<ThreadList>> {
        Call::new(self.client.clone(), self.client.rest().guilds().active_threads(self.id))
    }

    /// The cached channels of this guild, in no particular order. No I/O.
    pub fn cached_channels(&self) -> Vec<Bound<Channel>> {
        self.client.cache().guild_channels(self.id).into_iter().map(|c| self.client.bind_arc(c)).collect()
    }

    /// Creates a channel. Use `.reason(...)` for the audit log.
    pub fn create_channel(&self, body: CreateChannel) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().guilds().create_channel(self.id, body))
    }

    /// Moves several channels at once. Use `.reason(...)` for the audit log.
    pub fn set_channel_positions(&self, positions: Vec<ChannelPosition>) -> Call<()> {
        let body = match RequestBody::json(&positions) {
            Ok(body) => body,
            Err(e) => return Call::failed(self.client.clone(), dr_rest::RestError::from(e).into()),
        };
        Call::new(self.client.clone(), self.client.rest().guilds().modify_channel_positions(self.id, body))
    }

    /// What the bot may do in this guild, outside of any channel, computed from the cache. `None`
    /// before `READY` or when the guild or the bot's member is not cached.
    pub fn my_permissions(&self) -> Option<Permissions> {
        let me = self.client.cache().me()?;
        self.client.cache().member_permissions(self.id, me.id)
    }
}

impl Bound<Guild> {
    /// This guild, as a handle.
    pub fn handle(&self) -> GuildHandle {
        self.client().guild(self.id)
    }
}
