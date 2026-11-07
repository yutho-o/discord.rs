//! `Bound<T>`: wire data that knows its client, so that `msg.reply("hi")` needs no extra argument.

use crate::Client;
use std::ops::Deref;
use std::sync::Arc;

/// A value from Discord along with the client it came from. Two `Arc`s, 16 bytes. It derefs to the
/// data, and the verbs on it are inherent impls in `handles/` and `interactions/`.
pub struct Bound<T> {
    client: Client,
    data: Arc<T>,
}

impl<T> Bound<T> {
    /// Binds data to a client.
    pub fn new(client: Client, data: Arc<T>) -> Self {
        Self { client, data }
    }
    /// The client.
    pub fn client(&self) -> &Client {
        &self.client
    }
    /// The data, which may be the same allocation the cache holds.
    pub fn data(&self) -> &Arc<T> {
        &self.data
    }
}

impl<T> Clone for Bound<T> {
    fn clone(&self) -> Self {
        Self { client: self.client.clone(), data: self.data.clone() }
    }
}

impl<T> Deref for Bound<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Bound<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}

/// The payload of an update event. `new` is always there: every event always fires.
#[derive(Debug)]
pub struct Updated<T> {
    /// The value before this event, if it was cached.
    pub old: Option<Arc<T>>,
    /// The value after it.
    pub new: Bound<T>,
}

impl<T> Clone for Updated<T> {
    fn clone(&self) -> Self {
        Self { old: self.old.clone(), new: self.new.clone() }
    }
}

/// The payload of a delete event. `event` is always there: every event always fires.
#[derive(Debug)]
pub struct Deleted<P, T> {
    /// The event as received.
    pub event: Bound<P>,
    /// The last cached value, if it was cached.
    pub cached: Option<Arc<T>>,
}

impl<P, T> Clone for Deleted<P, T> {
    fn clone(&self) -> Self {
        Self { event: self.event.clone(), cached: self.cached.clone() }
    }
}
