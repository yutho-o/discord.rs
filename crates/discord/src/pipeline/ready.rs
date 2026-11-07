//! When `ClientReady` fires: once per client start, when every shard this process runs sent `READY`
//! and then finished its guild list or passed `wait_guild_timeout`. Works with no cache at all.

use dr_model::{GuildId, ReadyEvent};
use parking_lot::Mutex;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

#[derive(Debug, Default)]
struct ShardState {
    ready: Option<Arc<ReadyEvent>>,
    /// The guilds of its `READY` that have not had their `GUILD_CREATE` yet.
    awaited: HashSet<GuildId>,
    /// Its guild list is complete, or its wait timed out.
    settled: bool,
}

#[derive(Debug)]
struct State {
    fired: bool,
    expected: BTreeSet<u32>,
    shards: BTreeMap<u32, ShardState>,
}

impl Default for State {
    fn default() -> Self {
        Self { fired: false, expected: BTreeSet::from([0]), shards: BTreeMap::new() }
    }
}

/// Decides when `ClientReady` fires. Each method returns the event when this is the moment: the
/// `READY` of the lowest shard, with the guilds of every shard's `READY`.
#[derive(Debug, Default)]
pub struct ReadyTracker {
    state: Mutex<State>,
}

impl ReadyTracker {
    /// A tracker that has not fired and waits for shard 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the shards to wait for, before any of them connects. An empty set keeps the current one.
    pub fn expect(&self, shards: impl IntoIterator<Item = u32>) {
        let shards: BTreeSet<u32> = shards.into_iter().collect();
        if !shards.is_empty() {
            self.state.lock().expected = shards;
        }
    }

    /// A `READY` arrived. A shard with no guilds is settled at once.
    pub fn on_ready(&self, shard: u32, ready: Arc<ReadyEvent>) -> Option<Arc<ReadyEvent>> {
        let mut state = self.state.lock();
        if state.fired {
            return None;
        }
        let awaited: HashSet<GuildId> = ready.guilds.iter().map(|g| g.id).collect();
        let settled = awaited.is_empty();
        state.shards.insert(shard, ShardState { ready: Some(ready), awaited, settled });
        Self::check(&mut state)
    }

    /// A `GUILD_CREATE` arrived. Settles its shard when it was the last one expected there.
    pub fn on_guild_create(&self, shard: u32, guild: GuildId) -> Option<Arc<ReadyEvent>> {
        let mut state = self.state.lock();
        if state.fired {
            return None;
        }
        let entry = state.shards.get_mut(&shard)?;
        if entry.awaited.remove(&guild) && entry.awaited.is_empty() {
            entry.settled = true;
        }
        Self::check(&mut state)
    }

    /// `wait_guild_timeout` passed for `shard`, 15 s after its `READY`. Settles it if it has not.
    pub fn on_timeout(&self, shard: u32) -> Option<Arc<ReadyEvent>> {
        let mut state = self.state.lock();
        if state.fired {
            return None;
        }
        let entry = state.shards.get_mut(&shard)?;
        if entry.ready.is_some() {
            entry.settled = true;
        }
        Self::check(&mut state)
    }

    /// Fires when every expected shard is settled.
    fn check(state: &mut State) -> Option<Arc<ReadyEvent>> {
        let all = state.expected.iter().all(|id| state.shards.get(id).is_some_and(|s| s.settled && s.ready.is_some()));
        if !all {
            return None;
        }
        state.fired = true;
        let mut readies = state.expected.iter().filter_map(|id| state.shards.get(id)?.ready.clone());
        let first = readies.next()?;
        let rest: Vec<Arc<ReadyEvent>> = readies.collect();
        if rest.is_empty() {
            return Some(first);
        }
        let mut merged = ReadyEvent::clone(&first);
        for other in rest {
            merged.guilds.extend(other.guilds.iter().cloned());
        }
        Some(Arc::new(merged))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use serde_json::json;

    fn ready(shard: u32, guilds: &[u64]) -> Arc<ReadyEvent> {
        let guilds: Vec<_> = guilds.iter().map(|g| json!({"id": g.to_string(), "unavailable": true})).collect();
        Arc::new(
            serde_json::from_value(json!({
                "v": 10, "user": {"id": "1", "username": "bot", "discriminator": "0"},
                "guilds": guilds, "session_id": "s", "shard": [shard, 2],
                "application": {"id": "1", "flags": 0}
            }))
            .unwrap(),
        )
    }

    #[test]
    fn one_shard_fires_after_its_guilds() {
        let tracker = ReadyTracker::new();
        assert!(tracker.on_ready(0, ready(0, &[10])).is_none());
        assert!(tracker.on_guild_create(0, GuildId::new(10)).is_some());
        assert!(tracker.on_timeout(0).is_none(), "once");
    }

    #[test]
    fn every_shard_must_be_ready_and_guilds_merge() {
        let tracker = ReadyTracker::new();
        tracker.expect([0, 1]);
        assert!(tracker.on_ready(1, ready(1, &[])).is_none(), "shard 0 is missing");
        assert!(tracker.on_ready(0, ready(0, &[10, 11])).is_none());
        assert!(tracker.on_guild_create(0, GuildId::new(10)).is_none());
        let fired = tracker.on_timeout(0).unwrap();
        assert_eq!(fired.guilds.len(), 2);
        assert_eq!(fired.shard, Some([0, 2]));
    }

    #[test]
    fn a_timeout_before_ready_does_nothing() {
        let tracker = ReadyTracker::new();
        tracker.expect([0, 1]);
        assert!(tracker.on_timeout(1).is_none());
        assert!(tracker.on_ready(0, ready(0, &[])).is_none());
        assert!(tracker.on_ready(1, ready(1, &[20])).is_none());
        assert!(tracker.on_guild_create(1, GuildId::new(20)).is_some());
    }
}
