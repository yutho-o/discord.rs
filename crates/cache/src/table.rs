//! `Table`, the one locked map. No guard or closure escapes it.

use foldhash::fast::RandomState;
use hashbrown::HashMap;
use parking_lot::RwLock;
use std::hash::Hash;
use std::sync::Arc;

/// A map of `Arc` values behind a lock that is held only inside each method.
#[derive(Debug)]
pub struct Table<K, V> {
    inner: RwLock<HashMap<K, Arc<V>, RandomState>>,
}

impl<K: Eq + Hash, V> Default for Table<K, V> {
    fn default() -> Self {
        Self { inner: RwLock::new(HashMap::default()) }
    }
}

impl<K: Eq + Hash, V> Table<K, V> {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }
    /// The value at `k`.
    pub fn get(&self, k: &K) -> Option<Arc<V>> {
        self.inner.read().get(k).cloned()
    }
    /// Sets the value at `k`, and returns the old one: that is the `old` of an update.
    pub fn insert(&self, k: K, v: Arc<V>) -> Option<Arc<V>> {
        self.inner.write().insert(k, v)
    }
    /// Sets the value at `k` unless there is one. A REST result never replaces gateway state.
    pub fn insert_if_absent(&self, k: K, v: Arc<V>) {
        self.inner.write().entry(k).or_insert(v);
    }
    /// Removes the value at `k`, and returns it.
    pub fn remove(&self, k: &K) -> Option<Arc<V>> {
        self.inner.write().remove(k)
    }
    /// A copy of the values, in no particular order.
    pub fn snapshot(&self) -> Vec<Arc<V>> {
        self.inner.read().values().cloned().collect()
    }
    /// Makes room for `additional` more entries.
    pub fn reserve(&self, additional: usize) {
        self.inner.write().reserve(additional);
    }
    /// How many entries there are.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }
    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
