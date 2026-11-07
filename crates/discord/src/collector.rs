//! Collectors: wait in code for the next events that match, the upstream `Collector`,
//! `awaitMessages`, `awaitMessageComponent` and `awaitReactions`.
//!
//! A collector sees every event of its kind that reaches the handlers, after the cache saw it, whether
//! or not a handler exists for it. It does not add intents: a collector of `MessageCreate` needs the
//! client built with `GUILD_MESSAGES`, as a handler would.

use crate::events::{GatewayEvent, InteractionCreate, MessageCreate, MessageReactionAdd};
use crate::handles::ChannelHandle;
use crate::interactions::{ComponentInteraction, Interaction};
use crate::{Bound, Client};
use dr_model::{Message, MessageReactionAddEvent};
use futures::Stream;
use parking_lot::Mutex;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;

/// How many matched events wait in a collector before newer ones are dropped. A collector that is
/// polled keeps up easily; one that is never polled must not grow without bound.
const BACKLOG: usize = 256;

type Filter<P> = Box<dyn Fn(&P) -> bool + Send + Sync>;

struct Listener<P> {
    filter: Filter<P>,
    tx: mpsc::Sender<P>,
}

/// The live collectors of a client, by event.
#[derive(Default)]
pub(crate) struct Collectors {
    by_event: Mutex<HashMap<TypeId, Box<dyn Any + Send>>>,
}

impl std::fmt::Debug for Collectors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collectors").finish_non_exhaustive()
    }
}

impl Collectors {
    fn add<E: GatewayEvent>(&self, listener: Listener<E::Payload>) {
        let mut map = self.by_event.lock();
        let slot = map.entry(TypeId::of::<E>()).or_insert_with(|| Box::new(Vec::<Listener<E::Payload>>::new()));
        if let Some(list) = slot.downcast_mut::<Vec<Listener<E::Payload>>>() {
            list.push(listener);
        }
    }

    /// Hands `payload` to each collector of `E` whose filter accepts it, and forgets the collectors
    /// that were dropped. The filters run under a lock, so they must be quick and must not wait.
    pub(crate) fn offer<E: GatewayEvent>(&self, payload: &E::Payload) {
        let mut map = self.by_event.lock();
        let Some(list) = map.get_mut(&TypeId::of::<E>()).and_then(|b| b.downcast_mut::<Vec<Listener<E::Payload>>>())
        else {
            return;
        };
        list.retain(|listener| {
            if listener.tx.is_closed() {
                return false;
            }
            if (listener.filter)(payload) && listener.tx.try_send(payload.clone()).is_err() {
                tracing::warn!(event = E::NAME, "a collector is full, an event was dropped");
            }
            true
        });
    }
}

/// The events of one kind that pass a filter, as they arrive, until it is dropped. Read it with
/// [`Collector::next`], or as a [`Stream`].
///
/// ```no_run
/// # use discord::prelude::*;
/// # use std::time::Duration;
/// # async fn f(client: Client, channel: ChannelId, user: UserId) {
/// let mut answers = client.collector::<MessageCreate>(move |m| m.channel_id == channel && m.author.id == user);
/// while let Some(message) = answers.next_within(Duration::from_secs(30)).await {
///     if message.content == "stop" {
///         break;
///     }
/// }
/// # }
/// ```
pub struct Collector<E: GatewayEvent> {
    rx: mpsc::Receiver<E::Payload>,
}

impl<E: GatewayEvent> std::fmt::Debug for Collector<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collector").field("event", &E::NAME).finish_non_exhaustive()
    }
}

impl<E: GatewayEvent> Collector<E> {
    /// The next matching event. It waits as long as it takes: see [`Self::next_within`].
    pub async fn next(&mut self) -> Option<E::Payload> {
        self.rx.recv().await
    }

    /// The next matching event, or `None` if none came within `limit`.
    pub async fn next_within(&mut self, limit: Duration) -> Option<E::Payload> {
        tokio::time::timeout(limit, self.rx.recv()).await.ok().flatten()
    }

    /// Up to `max` matching events, gathering for at most `limit` in total.
    pub async fn take(mut self, max: usize, limit: Duration) -> Vec<E::Payload> {
        let mut out = Vec::new();
        let deadline = tokio::time::Instant::now() + limit;
        while out.len() < max {
            match tokio::time::timeout_at(deadline, self.rx.recv()).await {
                Ok(Some(event)) => out.push(event),
                _ => break,
            }
        }
        out
    }
}

impl<E: GatewayEvent> Stream for Collector<E> {
    type Item = E::Payload;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl Client {
    /// Starts collecting the events of kind `E` that `filter` accepts. Events that arrived before this
    /// call are not seen. Dropping the collector stops it.
    pub fn collector<E: GatewayEvent>(
        &self,
        filter: impl Fn(&E::Payload) -> bool + Send + Sync + 'static,
    ) -> Collector<E> {
        let (tx, rx) = mpsc::channel(BACKLOG);
        self.0.collectors.add::<E>(Listener { filter: Box::new(filter), tx });
        Collector { rx }
    }

    /// The next event of kind `E` that `filter` accepts, or `None` after `limit`.
    pub async fn wait_for<E: GatewayEvent>(
        &self,
        filter: impl Fn(&E::Payload) -> bool + Send + Sync + 'static,
        limit: Duration,
    ) -> Option<E::Payload> {
        self.collector::<E>(filter).next_within(limit).await
    }
}

/// The component interaction in an `INTERACTION_CREATE` payload, if it is one on `message`.
fn component_on(interaction: &Interaction, message: dr_model::MessageId) -> Option<&Bound<ComponentInteraction>> {
    match interaction {
        Interaction::Component(c) if c.message.id == message => Some(c),
        _ => None,
    }
}

impl Bound<Message> {
    /// Starts collecting the clicks and selections on this message's components that `filter`
    /// accepts. Each one still needs an answer, such as `update` or `defer_update`.
    pub fn component_collector(
        &self,
        filter: impl Fn(&Bound<ComponentInteraction>) -> bool + Send + Sync + 'static,
    ) -> ComponentCollector {
        let id = self.id;
        let inner = self.client().collector::<InteractionCreate>(move |i| component_on(i, id).is_some_and(&filter));
        ComponentCollector { inner, message: id }
    }

    /// The next click or selection on this message's components, or `None` after `limit`. Answer it,
    /// or the user sees "This interaction failed".
    pub async fn await_component(&self, limit: Duration) -> Option<Bound<ComponentInteraction>> {
        self.component_collector(|_| true).next_within(limit).await
    }

    /// The next reaction added to this message that `filter` accepts, or `None` after `limit`. Needs
    /// the `GUILD_MESSAGE_REACTIONS` or `DIRECT_MESSAGE_REACTIONS` intent.
    pub async fn await_reaction(
        &self,
        filter: impl Fn(&Bound<MessageReactionAddEvent>) -> bool + Send + Sync + 'static,
        limit: Duration,
    ) -> Option<Bound<MessageReactionAddEvent>> {
        let id = self.id;
        self.client().wait_for::<MessageReactionAdd>(move |r| r.message_id == id && filter(r), limit).await
    }
}

/// The component interactions of one message. Get one from [`Bound::component_collector`].
#[derive(Debug)]
pub struct ComponentCollector {
    inner: Collector<InteractionCreate>,
    message: dr_model::MessageId,
}

impl ComponentCollector {
    /// The next one. It waits as long as it takes.
    pub async fn next(&mut self) -> Option<Bound<ComponentInteraction>> {
        let interaction = self.inner.next().await?;
        component_on(&interaction, self.message).cloned()
    }

    /// The next one, or `None` if none came within `limit`.
    pub async fn next_within(&mut self, limit: Duration) -> Option<Bound<ComponentInteraction>> {
        let interaction = self.inner.next_within(limit).await?;
        component_on(&interaction, self.message).cloned()
    }
}

impl ChannelHandle {
    /// The next message in this channel that `filter` accepts, or `None` after `limit`. Needs the
    /// message intents, and `MESSAGE_CONTENT` to read the text.
    pub async fn await_message(
        &self,
        filter: impl Fn(&Bound<Message>) -> bool + Send + Sync + 'static,
        limit: Duration,
    ) -> Option<Bound<Message>> {
        let id = self.id;
        self.client.wait_for::<MessageCreate>(move |m| m.channel_id == id && filter(m), limit).await
    }

    /// Up to `max` messages in this channel that `filter` accepts, gathering for at most `limit`.
    pub async fn await_messages(
        &self,
        filter: impl Fn(&Bound<Message>) -> bool + Send + Sync + 'static,
        max: usize,
        limit: Duration,
    ) -> Vec<Bound<Message>> {
        let id = self.id;
        self.client.collector::<MessageCreate>(move |m| m.channel_id == id && filter(m)).take(max, limit).await
    }
}
