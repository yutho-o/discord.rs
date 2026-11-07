//! The writes: one `apply_<event>` per event, pure and sync, in strict event order per shard.
//!
//! Each returns the `Arc` the event's payload will use. Every apply works with an empty cache and
//! under `CacheConfig::none`: it always returns a value built from the event, which is what makes
//! "every event always fires" true.

pub mod channel;
pub mod guild;
pub mod member;
pub mod message;

use dr_model::ReadyEvent;
use std::sync::Arc;

use crate::Cache;

/// The pair an update produces. `old` is what the swap returned, `None` when it was not cached.
#[derive(Debug)]
pub struct Change<T> {
    /// The value before this event, if it was cached.
    pub old: Option<Arc<T>>,
    /// The value after it.
    pub new: Arc<T>,
}

impl<T> Clone for Change<T> {
    fn clone(&self) -> Self {
        Self { old: self.old.clone(), new: self.new.clone() }
    }
}

impl Cache {
    /// `READY`: stores the bot's user. Always does so, even under `CacheConfig::none`.
    pub fn apply_ready(&self, shard: u32, event: ReadyEvent) -> Arc<ReadyEvent> {
        let _ = shard;
        let me = Arc::new(event.user.clone());
        *self.me.write() = Some(me.clone());
        if self.config.is_enabled() {
            self.users.insert(me.id, me);
        }
        Arc::new(event)
    }
}
