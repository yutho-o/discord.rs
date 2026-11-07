//! Message events.

use crate::{Cache, Change};
use dr_core::Patch;
use dr_model::{Message, MessageDeleteEvent, MessageUpdateEvent};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

/// Sets each listed field from the event when the event has it.
macro_rules! overlay {
    ($m:ident, $e:ident, value: $($f:ident),* ; option: $($o:ident),*) => {
        $(if let Some(v) = $e.$f { $m.$f = v; })*
        $(if $e.$o.is_some() { $m.$o = $e.$o; })*
    };
}

fn merge(mut m: Message, e: MessageUpdateEvent) -> Message {
    overlay!(m, e,
        value: author, content, kind, tts, mention_everyone, mentions, mention_roles, embeds,
            attachments, reactions, components, pinned, mention_channels, message_snapshots;
        option: guild_id, member, timestamp, channel_type, flags, nonce, webhook_id, application_id,
            message_reference, activity, interaction_metadata, interaction, position, call,
            shared_client_theme
    );
    match e.edited_timestamp {
        Patch::Absent => {}
        Patch::Null => m.edited_timestamp = None,
        Patch::Value(t) => m.edited_timestamp = Some(t),
    }
    if !matches!(e.referenced_message, Patch::Absent) {
        m.referenced_message = e.referenced_message;
    }
    m
}

impl Cache {
    /// `MESSAGE_CREATE`. Also stores the author. Evicts the oldest over the per-channel limit.
    pub fn apply_message_create(&self, shard: u32, message: Message) -> Arc<Message> {
        let _ = shard;
        if !self.config.is_enabled() {
            return Arc::new(message);
        }
        self.users.insert(message.author.id, Arc::new(message.author.clone()));
        if let (Some(guild), Some(member)) = (message.guild_id, &message.member) {
            // The nested member has no `user`: the author of the message is that user.
            let mut member = member.clone();
            member.user = Some(message.author.clone());
            let slot = self.slot(guild);
            self.store_member(&slot, guild, member);
        }
        let message = Arc::new(message);
        let limit = usize::try_from(self.config.messages_limit()).unwrap_or(usize::MAX);
        if limit == 0 {
            return message;
        }
        let list = self.messages.get(&message.channel_id).unwrap_or_else(|| {
            let list = Arc::new(Mutex::new(VecDeque::new()));
            self.messages.insert(message.channel_id, list.clone());
            list
        });
        let mut list = list.lock();
        list.push_back(message.clone());
        while list.len() > limit {
            list.pop_front();
        }
        drop(list);
        message
    }

    /// `MESSAGE_UPDATE`. The event is partial, so `new` is the old message with the event's fields
    /// on top, or `Message::partial` with them when none is cached. It always returns a value.
    pub fn apply_message_update(&self, shard: u32, event: MessageUpdateEvent) -> Change<Message> {
        let _ = shard;
        if let Some(list) = self.messages.get(&event.channel_id) {
            let mut list = list.lock();
            if let Some(slot) = list.iter_mut().find(|m| m.id == event.id) {
                let old = slot.clone();
                let new = Arc::new(merge((*old).clone(), event));
                *slot = new.clone();
                return Change { old: Some(old), new };
            }
        }
        let base = Message::partial(event.id, event.channel_id);
        Change { old: None, new: Arc::new(merge(base, event)) }
    }

    /// `MESSAGE_DELETE`. Returns the message as it was cached.
    pub fn apply_message_delete(&self, shard: u32, event: MessageDeleteEvent) -> Option<Arc<Message>> {
        let _ = shard;
        let list = self.messages.get(&event.channel_id)?;
        let mut list = list.lock();
        let index = list.iter().position(|m| m.id == event.id)?;
        list.remove(index)
    }
}
