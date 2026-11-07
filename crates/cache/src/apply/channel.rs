//! Channel events.

use crate::{Cache, Change};
use dr_model::{Channel, ThreadListSyncEvent};
use std::sync::Arc;

impl Cache {
    /// `CHANNEL_CREATE`.
    pub fn apply_channel_create(&self, shard: u32, channel: Channel) -> Arc<Channel> {
        let _ = shard;
        let channel = Arc::new(channel);
        if self.config.is_enabled() {
            self.store_channel(channel.clone());
        }
        channel
    }

    /// `CHANNEL_UPDATE`. A channel type change keeps its cached messages.
    pub fn apply_channel_update(&self, shard: u32, channel: Channel) -> Change<Channel> {
        let _ = shard;
        let new = Arc::new(channel);
        let old = if self.config.is_enabled() { self.store_channel(new.clone()) } else { None };
        Change { old, new }
    }

    /// `CHANNEL_DELETE`. Removes the channel and its messages. Returns the channel as it was cached.
    pub fn apply_channel_delete(&self, shard: u32, channel: Channel) -> Option<Arc<Channel>> {
        let _ = shard;
        if !self.config.is_enabled() {
            return None;
        }
        self.messages.remove(&channel.id);
        if let Some(guild) = channel.guild_id {
            if let Some(slot) = self.guilds.get(&guild) {
                slot.channels.remove(&channel.id);
            }
        } else {
            self.dm_order.lock().retain(|id| *id != channel.id);
        }
        self.channels.remove(&channel.id)
    }

    /// `THREAD_LIST_SYNC`. Drops the cached threads of the synced channels, or of the whole guild when
    /// the event names none, then stores the threads it lists.
    pub fn apply_thread_list_sync(&self, shard: u32, event: &ThreadListSyncEvent) {
        if !self.config.is_enabled() {
            return;
        }
        let synced = |parent: Option<dr_model::ChannelId>| match &event.channel_ids {
            None => true,
            Some(ids) => parent.is_some_and(|p| ids.contains(&p)),
        };
        let stale: Vec<Arc<Channel>> =
            self.guild_channels(event.guild_id).into_iter().filter(|c| c.is_thread() && synced(c.parent_id)).collect();
        for thread in stale {
            self.apply_channel_delete(shard, (*thread).clone());
        }
        for thread in &event.threads {
            let mut thread = thread.clone();
            thread.guild_id.get_or_insert(event.guild_id);
            self.store_channel(Arc::new(thread));
        }
    }
}
