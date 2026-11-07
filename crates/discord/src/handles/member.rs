//! `Bound<Member>`.

use crate::handles::{GuildHandle, UserHandle};
use crate::{Bound, Call, Error};
use dr_model::Member;

impl Bound<Member> {
    /// The guild, as a handle. `None` only for a member value that came with no `guild_id`, which
    /// the facade's fetches fill in.
    pub fn guild(&self) -> Option<GuildHandle> {
        self.guild_id.map(|g| self.client().guild(g))
    }

    /// The user, as a handle. `None` for a partial member with no user.
    pub fn user_handle(&self) -> Option<UserHandle> {
        self.user.as_ref().map(|u| self.client().user(u.id))
    }

    /// Kicks this member. Use `.reason(...)` for the audit log. Fails with `Error::Other` on a partial
    /// member, which has no guild or no user to name.
    pub fn kick(&self) -> Call<()> {
        match (self.guild_id, self.user.as_ref()) {
            (Some(guild), Some(user)) => {
                Call::new(self.client().clone(), self.client().rest().guilds().remove_member(guild, user.id))
            }
            _ => Call::failed(self.client().clone(), Error::Other("this member has no guild or no user".to_owned())),
        }
    }
}
