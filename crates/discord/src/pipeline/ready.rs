//! When `ClientReady` fires: once per client start, when the shard finished its guild list or passed
//! `wait_guild_timeout`. Works with no cache at all. One shard.

use dr_model::{GuildId, ReadyEvent};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Default)]
struct State {
    fired: bool,
    pending: Option<Arc<ReadyEvent>>,
    expected: HashSet<GuildId>,
}

/// Decides when `ClientReady` fires. Each method returns the event when this is the moment.
#[derive(Debug, Default)]
pub struct ReadyTracker {
    state: Mutex<State>,
}

impl ReadyTracker {
    /// A tracker that has not fired.
    pub fn new() -> Self {
        Self::default()
    }

    /// A `READY` arrived. Fires at once when its guild list is empty.
    pub fn on_ready(&self, shard: u32, ready: Arc<ReadyEvent>) -> Option<Arc<ReadyEvent>> {
        let _ = shard;
        let mut state = self.state.lock();
        if state.fired {
            return None;
        }
        state.expected = ready.guilds.iter().map(|g| g.id).collect();
        if state.expected.is_empty() {
            state.fired = true;
            return Some(ready);
        }
        state.pending = Some(ready);
        None
    }

    /// A `GUILD_CREATE` arrived. Fires when it was the last one expected.
    pub fn on_guild_create(&self, shard: u32, guild: GuildId) -> Option<Arc<ReadyEvent>> {
        let _ = shard;
        let mut state = self.state.lock();
        state.expected.remove(&guild);
        if state.fired || !state.expected.is_empty() {
            return None;
        }
        let ready = state.pending.take()?;
        state.fired = true;
        Some(ready)
    }

    /// `wait_guild_timeout` passed, 15 s after `READY`. Fires if it has not yet.
    pub fn on_timeout(&self) -> Option<Arc<ReadyEvent>> {
        let mut state = self.state.lock();
        if state.fired {
            return None;
        }
        let ready = state.pending.take()?;
        state.fired = true;
        Some(ready)
    }
}
