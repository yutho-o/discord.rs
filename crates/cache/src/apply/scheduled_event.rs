//! Scheduled event events.

use crate::{Cache, Change};
use dr_model::{GuildScheduledEvent, ScheduledEventUserEvent};
use std::sync::Arc;

impl Cache {
    /// `GUILD_SCHEDULED_EVENT_CREATE` and `GUILD_SCHEDULED_EVENT_UPDATE`. `old` is the event as it
    /// was. The gateway sends no `user_count`, so the cached count carries over.
    pub fn apply_scheduled_event_upsert(
        &self,
        shard: u32,
        mut event: GuildScheduledEvent,
    ) -> Change<GuildScheduledEvent> {
        let _ = shard;
        if !self.config.is_enabled() {
            return Change { old: None, new: Arc::new(event) };
        }
        let slot = self.slot(event.guild_id);
        let previous = slot.scheduled_events.get(&event.id);
        if let Some(previous) = &previous {
            event.user_count = event.user_count.or(previous.user_count);
        }
        let new = Arc::new(event);
        slot.scheduled_events.insert(new.id, new.clone());
        Change { old: previous, new }
    }

    /// `GUILD_SCHEDULED_EVENT_DELETE`. Returns the event as it was cached.
    pub fn apply_scheduled_event_delete(
        &self,
        shard: u32,
        event: &GuildScheduledEvent,
    ) -> Option<Arc<GuildScheduledEvent>> {
        let _ = shard;
        self.guilds.get(&event.guild_id)?.scheduled_events.remove(&event.id)
    }

    /// `GUILD_SCHEDULED_EVENT_USER_ADD` and `_REMOVE`. Moves a cached `user_count` by one, when the
    /// cache has a count.
    pub fn apply_scheduled_event_user(&self, shard: u32, event: &ScheduledEventUserEvent, added: bool) {
        let _ = shard;
        let Some(slot) = self.guilds.get(&event.guild_id) else { return };
        let Some(cached) = slot.scheduled_events.get(&event.guild_scheduled_event_id) else { return };
        let Some(count) = cached.user_count else { return };
        let mut copy = (*cached).clone();
        copy.user_count = Some(if added { count.saturating_add(1) } else { count.saturating_sub(1) });
        slot.scheduled_events.insert(copy.id, Arc::new(copy));
    }
}
