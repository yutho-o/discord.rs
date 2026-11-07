//! Guild events.

use crate::store::GuildSlot;
use crate::{Cache, Change};
use dr_model::{Guild, UnavailableGuild};
use std::sync::Arc;

/// A copy of a guild with its three lists left out, without cloning them.
fn stored_form(guild: &mut Guild) -> Guild {
    let channels = std::mem::take(&mut guild.channels);
    let members = std::mem::take(&mut guild.members);
    let roles = std::mem::take(&mut guild.roles);
    let stored = guild.clone();
    guild.channels = channels;
    guild.members = members;
    guild.roles = roles;
    stored
}

impl Cache {
    /// `GUILD_CREATE`. The returned payload is as received, with its lists. The cached copy has its
    /// channels, roles and members moved to their tables.
    pub fn apply_guild_create(&self, shard: u32, mut guild: Guild) -> Arc<Guild> {
        let _ = shard;
        if !self.config.is_enabled() {
            return Arc::new(guild);
        }
        let id = guild.id;
        let slot = Arc::new(GuildSlot::default());
        *slot.meta.write() = Some(Arc::new(stored_form(&mut guild)));
        // A guild that comes back after an outage replaces what was known of it. A channel that still
        // exists keeps its messages.
        if let Some(previous) = self.guilds.insert(id, slot.clone()) {
            for channel in previous.channels.snapshot() {
                if !guild.channels.iter().any(|c| c.id == channel.id) {
                    self.channels.remove(&channel.id);
                    self.messages.remove(&channel.id);
                }
            }
        }
        for channel in &guild.channels {
            let mut channel = channel.clone();
            // Discord leaves this out of the channels nested in a guild.
            channel.guild_id = Some(id);
            self.store_channel(Arc::new(channel));
        }
        for role in &guild.roles {
            slot.roles.insert(role.id, Arc::new(role.clone()));
        }
        for member in &guild.members {
            self.store_member(&slot, id, member.clone());
        }
        Arc::new(guild)
    }

    /// `GUILD_UPDATE`. Keeps the tables of the old guild. The event leaves out what only
    /// `GUILD_CREATE` sends, so those fields carry over from the old value.
    pub fn apply_guild_update(&self, shard: u32, mut guild: Guild) -> Change<Guild> {
        let _ = shard;
        if !self.config.is_enabled() {
            return Change { old: None, new: Arc::new(guild) };
        }
        let slot = self.slot(guild.id);
        let old = slot.meta.read().clone();
        if let Some(old) = &old {
            guild.joined_at = guild.joined_at.or(old.joined_at);
            guild.member_count = guild.member_count.or(old.member_count);
            guild.large = old.large;
        }
        for role in &guild.roles {
            slot.roles.insert(role.id, Arc::new(role.clone()));
        }
        *slot.meta.write() = Some(Arc::new(stored_form(&mut guild)));
        Change { old, new: Arc::new(guild) }
    }

    /// `GUILD_DELETE`. With `unavailable` true it is an outage, and the guild stays cached. With it
    /// false it is a removal, and everything under the guild goes. Returns the guild as it was.
    pub fn apply_guild_delete(&self, shard: u32, event: UnavailableGuild) -> Option<Arc<Guild>> {
        let _ = shard;
        if !self.config.is_enabled() {
            return None;
        }
        if event.unavailable {
            let slot = self.guilds.get(&event.id)?;
            let mut meta = slot.meta.write();
            let old = meta.clone()?;
            let mut outage = (*old).clone();
            outage.unavailable = true;
            *meta = Some(Arc::new(outage));
            return Some(old);
        }
        let slot = self.guilds.remove(&event.id)?;
        for channel in slot.channels.snapshot() {
            self.channels.remove(&channel.id);
            self.messages.remove(&channel.id);
        }

        slot.meta.read().clone()
    }
}
