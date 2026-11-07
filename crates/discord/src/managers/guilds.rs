//! `ctx.guilds()`.

use crate::{Bound, Call, Client, Error};
use dr_model::{Guild, GuildId};

/// Guilds. Get one from [`Client::guilds`].
#[derive(Clone, Debug)]
pub struct GuildsManager {
    client: Client,
}

impl Client {
    /// Guilds, by id.
    pub fn guilds(&self) -> GuildsManager {
        GuildsManager { client: self.clone() }
    }
}

impl GuildsManager {
    /// The guild from the cache. No I/O.
    pub fn get(&self, id: GuildId) -> Option<Bound<Guild>> {
        self.client.cache().guild(id).map(|g| self.client.bind_arc(g))
    }

    /// The guild from Discord.
    pub fn fetch(&self, id: GuildId) -> Call<Bound<Guild>> {
        Call::new(self.client.clone(), self.client.rest().guilds().get(id))
    }

    /// The guild from the cache, else from Discord.
    pub async fn resolve(&self, id: GuildId) -> Result<Bound<Guild>, Error> {
        if let Some(guild) = self.get(id) {
            return Ok(guild);
        }
        let fetched = self.fetch(id).await?;
        self.client.cache().insert_guild_if_absent(fetched.data().clone());
        Ok(fetched)
    }
}
