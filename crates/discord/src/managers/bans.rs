//! The bans of one guild.

use crate::{Bound, Call, Client};
use dr_builders::CreateBan;
use dr_model::{GuildBan, GuildId, UserId};
use dr_rest::api::UserPage;

/// The bans of one guild. Get one from `GuildHandle::bans`. Each call needs `BAN_MEMBERS`.
#[derive(Clone, Debug)]
pub struct BansManager {
    client: Client,
    guild: GuildId,
}

impl BansManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// Bans a user, who need not be in the guild. Use `.reason(...)` for the audit log.
    pub fn create(&self, user: UserId, body: CreateBan) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().create_ban(self.guild, user, body))
    }

    /// Lifts a ban. Use `.reason(...)` for the audit log.
    pub fn remove(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().remove_ban(self.guild, user))
    }

    /// The ban of one user. Fails with an API error when the user is not banned.
    pub fn fetch(&self, user: UserId) -> Call<Bound<GuildBan>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_ban(self.guild, user))
    }

    /// One page of bans, up to 1000.
    pub fn list(&self, page: UserPage) -> Call<Vec<Bound<GuildBan>>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get_bans(self.guild, page))
    }
}
