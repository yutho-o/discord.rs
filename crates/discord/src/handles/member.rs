//! `Bound<Member>`.

use crate::handles::{GuildHandle, UserHandle};
use crate::managers::MembersManager;
use crate::{Bound, Call, Error};
use dr_builders::EditMember;
use dr_model::{ChannelId, Member, Permissions, RoleId, UserId};
use std::time::Duration;

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

    /// What this member may do in the guild, outside of any channel, computed from the cache's guild
    /// and roles. `None` when the guild is not cached, or for a partial member.
    pub fn permissions(&self) -> Option<Permissions> {
        self.client().cache().permissions_for(self.guild_id?, self, None)
    }

    /// What this member may do in `channel`, computed from the cache. `None` when the guild or the
    /// channel is not cached, or for a partial member.
    pub fn permissions_in(&self, channel: ChannelId) -> Option<Permissions> {
        self.client().cache().permissions_for(self.guild_id?, self, Some(channel))
    }

    /// Runs an action on this member through the guild's manager. A partial member, which has no guild
    /// or no user to name, gives a call that fails with `Error::Other`.
    fn act<T: crate::call::Bindable>(&self, action: impl FnOnce(MembersManager, UserId) -> Call<T>) -> Call<T> {
        match (self.guild(), self.user.as_ref()) {
            (Some(guild), Some(user)) => action(guild.members(), user.id),
            _ => Call::failed(self.client().clone(), Error::Other("this member has no guild or no user".to_owned())),
        }
    }

    /// Kicks this member. Use `.reason(...)` for the audit log.
    pub fn kick(&self) -> Call<()> {
        self.act(|members, user| members.kick(user))
    }

    /// Bans this member, deleting none of their messages. Use `.reason(...)` for the audit log.
    pub fn ban(&self) -> Call<()> {
        self.act(|members, user| members.ban(user))
    }

    /// Edits this member. Use `.reason(...)` for the audit log.
    pub fn edit(&self, body: EditMember) -> Call<Bound<Member>> {
        self.act(|members, user| members.edit(user, body))
    }

    /// Times this member out for this long, up to 28 days. Use `.reason(...)` for the audit log.
    pub fn timeout(&self, duration: Duration) -> Call<Bound<Member>> {
        self.act(|members, user| members.timeout(user, duration))
    }

    /// Ends this member's timeout. Use `.reason(...)` for the audit log.
    pub fn clear_timeout(&self) -> Call<Bound<Member>> {
        self.act(|members, user| members.clear_timeout(user))
    }

    /// Gives this member a role. Use `.reason(...)` for the audit log.
    pub fn add_role(&self, role: RoleId) -> Call<()> {
        self.act(|members, user| members.add_role(user, role))
    }

    /// Takes a role from this member. Use `.reason(...)` for the audit log.
    pub fn remove_role(&self, role: RoleId) -> Call<()> {
        self.act(|members, user| members.remove_role(user, role))
    }
}
