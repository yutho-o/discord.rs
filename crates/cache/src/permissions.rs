//! Permissions computed from what the cache holds, with [`dr_model::permissions`].

use crate::Cache;
use dr_model::permissions::{MemberContext, for_member};
use dr_model::{ChannelId, GuildId, Member, Permissions, UserId};

impl Cache {
    /// The guild-wide permissions of a member. `None` unless the guild and the member are cached. A
    /// role missing from the cache adds nothing, so a cold cache can only under-count.
    pub fn member_permissions(&self, guild: GuildId, user: UserId) -> Option<Permissions> {
        let member = self.member(guild, user)?;
        self.permissions_of(guild, user, &member, None)
    }

    /// The permissions of a member in a channel of their guild, after its overwrites. For a thread,
    /// those of its parent channel. `None` unless the channel, its guild and the member are cached.
    pub fn channel_permissions(&self, channel: ChannelId, user: UserId) -> Option<Permissions> {
        let mut target = self.channel(channel)?;
        if target.is_thread() {
            target = self.channel(target.parent_id?)?;
        }
        let guild = target.guild_id?;
        let member = self.member(guild, user)?;
        self.permissions_of(guild, user, &member, Some(&target.permission_overwrites))
    }

    /// The permissions of a member that is not necessarily cached, such as one an interaction or a
    /// fetch returned. `channel` adds that channel's overwrites, and `None` from it means the channel
    /// is not cached.
    pub fn permissions_for(&self, guild: GuildId, member: &Member, channel: Option<ChannelId>) -> Option<Permissions> {
        let user = member.user.as_ref()?.id;
        let overwrites = match channel {
            None => None,
            Some(id) => {
                let mut target = self.channel(id)?;
                if target.is_thread() {
                    target = self.channel(target.parent_id?)?;
                }
                Some(target)
            }
        };
        self.permissions_of(guild, user, member, overwrites.as_ref().map(|c| c.permission_overwrites.as_slice()))
    }

    fn permissions_of(
        &self,
        guild: GuildId,
        user: UserId,
        member: &Member,
        overwrites: Option<&[dr_model::PermissionOverwrite]>,
    ) -> Option<Permissions> {
        let meta = self.guild(guild)?;
        let context = MemberContext {
            guild_id: guild,
            owner_id: meta.owner_id,
            user_id: user,
            roles: &member.roles,
            timed_out: member.is_timed_out(),
        };
        Some(for_member(&context, |role| self.role(guild, role).map(|r| r.permissions), overwrites))
    }
}
