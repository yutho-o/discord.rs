//! The roles of one guild.

use crate::handles::RoleHandle;
use crate::{Bound, Call, Client};
use dr_builders::CreateRole;
use dr_model::{GuildId, Role, RoleId};

/// The roles of one guild. Get one from `GuildHandle::roles`.
#[derive(Clone, Debug)]
pub struct RolesManager {
    client: Client,
    guild: GuildId,
}

impl RolesManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// A handle to a role. No I/O.
    pub fn handle(&self, id: RoleId) -> RoleHandle {
        RoleHandle::new(self.client.clone(), self.guild, id)
    }

    /// Every role of this guild, from Discord.
    pub fn fetch_all(&self) -> Call<Vec<Bound<Role>>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_roles(self.guild))
    }

    /// Creates a role. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateRole) -> Call<Bound<Role>> {
        Call::new(self.client.clone(), self.client.rest().guilds().create_role(self.guild, body))
    }

    /// The role from the cache. No I/O.
    pub fn get(&self, id: RoleId) -> Option<Bound<Role>> {
        self.client.cache().role(self.guild, id).map(|r| self.client.bind_arc(r))
    }

    /// Every cached role of this guild, in no particular order. No I/O.
    pub fn cached(&self) -> Vec<Bound<Role>> {
        self.client.cache().guild_roles(self.guild).into_iter().map(|r| self.client.bind_arc(r)).collect()
    }
}
