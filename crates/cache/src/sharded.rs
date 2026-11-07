//! `Sharded`, a [`Table`] split by key hash so that writers of different guilds do not contend.

use crate::Table;
use foldhash::fast::RandomState;
use std::hash::{BuildHasher, Hash};
use std::sync::Arc;

const TABLES: usize = 256;

/// The same operations as [`Table`], with 256 locks, so a large cache does not serialize.
#[derive(Debug)]
pub struct Sharded<K, V> {
    tables: Vec<Table<K, V>>,
    hasher: RandomState,
}

impl<K: Eq + Hash, V> Default for Sharded<K, V> {
    fn default() -> Self {
        Self { tables: (0..TABLES).map(|_| Table::default()).collect(), hasher: RandomState::default() }
    }
}

impl<K: Eq + Hash, V> Sharded<K, V> {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }
    fn table(&self, k: &K) -> Option<&Table<K, V>> {
        let h = self.hasher.hash_one(k).to_le_bytes();
        self.tables.get(usize::from(h[7]))
    }
    /// The value at `k`.
    pub fn get(&self, k: &K) -> Option<Arc<V>> {
        self.table(k).and_then(|t| t.get(k))
    }
    /// Sets the value at `k`, and returns the old one.
    pub fn insert(&self, k: K, v: Arc<V>) -> Option<Arc<V>> {
        self.table(&k).and_then(|t| t.insert(k, v))
    }
    /// Sets the value at `k` unless there is one.
    pub fn insert_if_absent(&self, k: K, v: Arc<V>) {
        if let Some(t) = self.table(&k) {
            t.insert_if_absent(k, v);
        }
    }
    /// Removes the value at `k`, and returns it.
    pub fn remove(&self, k: &K) -> Option<Arc<V>> {
        self.table(k).and_then(|t| t.remove(k))
    }
    /// A copy of the values, in no particular order. Takes each lock in turn.
    pub fn snapshot(&self) -> Vec<Arc<V>> {
        self.tables.iter().flat_map(Table::snapshot).collect()
    }
    /// How many entries there are.
    pub fn len(&self) -> usize {
        self.tables.iter().map(Table::len).sum()
    }
    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(Table::is_empty)
    }
}
