//! `GuildHandle` and `Bound<Guild>`.

use crate::managers::{MembersManager, RolesManager};
use crate::{Bound, Call, Client};
use dr_model::{Channel, Guild, GuildId};

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

    /// The roles of this guild.
    pub fn roles(&self) -> RolesManager {
        RolesManager::new(self.client.clone(), self.id)
    }

    /// Fetches the channels of this guild from Discord.
    pub fn fetch_channels(&self) -> Call<Vec<Bound<Channel>>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_channels(self.id))
    }
}

impl Bound<Guild> {
    /// This guild, as a handle.
    pub fn handle(&self) -> GuildHandle {
        self.client().guild(self.id)
    }
}
