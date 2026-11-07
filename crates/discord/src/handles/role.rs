//! `RoleHandle`.

use crate::{Bound, Client};
use dr_model::{GuildId, Role, RoleId};

/// A role by id: a client and two ids, no I/O to create.
#[derive(Clone, Debug)]
pub struct RoleHandle {
    pub(crate) client: Client,
    pub(crate) guild: GuildId,
    pub(crate) id: RoleId,
}

impl RoleHandle {
    /// A handle to a role in a guild.
    pub fn new(client: Client, guild: GuildId, id: RoleId) -> Self {
        Self { client, guild, id }
    }

    /// The id.
    pub fn id(&self) -> RoleId {
        self.id
    }

    /// The role from the cache. No I/O.
    pub fn cached(&self) -> Option<Bound<Role>> {
        self.client.cache().role(self.guild, self.id).map(|r| self.client.bind_arc(r))
    }
}
