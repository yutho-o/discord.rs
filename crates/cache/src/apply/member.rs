//! Member events.

use crate::store::GuildSlot;
use crate::{Cache, Change};
use dr_core::Patch;
use dr_model::{GuildId, GuildMemberRemoveEvent, GuildMemberUpdateEvent, Member};
use std::sync::Arc;

/// Applies a three-state field: absent keeps, null clears, a value replaces.
fn patch<T>(slot: &mut Option<T>, value: Patch<T>) {
    match value {
        Patch::Absent => {}
        Patch::Null => *slot = None,
        Patch::Value(v) => *slot = Some(v),
    }
}

impl Cache {
    /// Stores a member seen in an event, and its user, under the policy. Returns the value with
    /// `guild_id` set. A member with no user is not stored.
    pub(crate) fn store_member(&self, slot: &GuildSlot, guild: GuildId, mut member: Member) -> Arc<Member> {
        member.guild_id = Some(guild);
        let member = Arc::new(member);
        if let Some(user) = &member.user {
            self.users.insert(user.id, Arc::new(user.clone()));
            if self.keeps_member(slot, user.id) {
                slot.members.insert(user.id, member.clone());
            }
        }
        member
    }

    /// `GUILD_MEMBER_ADD`. The event body has `guild_id`, and the returned member keeps it.
    pub fn apply_member_add(&self, shard: u32, member: Member) -> Arc<Member> {
        let _ = shard;
        match member.guild_id {
            Some(guild) if self.config.is_enabled() => {
                let slot = self.slot(guild);
                self.store_member(&slot, guild, member)
            }
            _ => Arc::new(member),
        }
    }

    /// `GUILD_MEMBER_UPDATE`. `new` starts from the old member, or from the event when none is
    /// cached. The event's `nick` is absent or null, which the two cases of `Patch` keep apart.
    pub fn apply_member_update(&self, shard: u32, event: GuildMemberUpdateEvent) -> Change<Member> {
        let _ = shard;
        let guild = event.guild_id;
        let slot = self.config.is_enabled().then(|| self.slot(guild));
        let old = slot.as_ref().and_then(|s| s.members.get(&event.user.id));
        let mut new = old.as_deref().cloned().unwrap_or_default();
        new.user = Some(event.user);
        new.guild_id = Some(guild);
        new.roles = event.roles;
        patch(&mut new.nick, event.nick);
        patch(&mut new.avatar, event.avatar);
        patch(&mut new.banner, event.banner);
        patch(&mut new.premium_since, event.premium_since);
        patch(&mut new.communication_disabled_until, event.communication_disabled_until);
        if event.joined_at.is_some() {
            new.joined_at = event.joined_at;
        }
        if let Some(v) = event.deaf {
            new.deaf = v;
        }
        if let Some(v) = event.mute {
            new.mute = v;
        }
        if event.flags.is_some() {
            new.flags = event.flags;
        }
        if let Some(v) = event.pending {
            new.pending = v;
        }
        match slot {
            Some(slot) => Change { old, new: self.store_member(&slot, guild, new) },
            None => Change { old: None, new: Arc::new(new) },
        }
    }

    /// `GUILD_MEMBER_REMOVE`. Returns the member as it was cached.
    pub fn apply_member_remove(&self, shard: u32, event: GuildMemberRemoveEvent) -> Option<Arc<Member>> {
        let _ = shard;
        if !self.config.is_enabled() {
            return None;
        }
        self.guilds.get(&event.guild_id)?.members.remove(&event.user.id)
    }
}
