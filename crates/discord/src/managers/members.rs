//! The members of one guild.

use crate::{Bound, Call, Client, Error};
use dr_model::{GuildId, Member, UserId};

/// The members of one guild. Get one from `GuildHandle::members`.
#[derive(Clone, Debug)]
pub struct MembersManager {
    client: Client,
    guild: GuildId,
}

impl MembersManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// The member from the cache. No I/O.
    pub fn get(&self, user: UserId) -> Option<Bound<Member>> {
        self.client.cache().member(self.guild, user).map(|m| self.client.bind_arc(m))
    }

    /// The member from Discord. The response has no `guild_id`, so this fills it in with
    /// [`Call::fix_wire`].
    pub fn fetch(&self, user: UserId) -> Call<Bound<Member>> {
        let guild = self.guild;
        Call::new(self.client.clone(), self.client.rest().guilds().get_member(guild, user))
            .fix_wire(move |member: &mut Member| member.guild_id = Some(guild))
    }

    /// The member from the cache, else from Discord.
    pub async fn resolve(&self, user: UserId) -> Result<Bound<Member>, Error> {
        if let Some(member) = self.get(user) {
            return Ok(member);
        }
        let fetched = self.fetch(user).await?;
        self.client.cache().insert_member_if_absent(self.guild, fetched.data().clone());
        Ok(fetched)
    }

    /// Kicks a member. Use `.reason(...)` for the audit log.
    pub fn kick(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().guilds().remove_member(self.guild, user))
    }
}
