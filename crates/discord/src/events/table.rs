//! `HandlerTable`: every handler, erased, frozen once the client is built.
//!

use super::{GatewayEvent, RawEvent};
use crate::error::{HandlerFailure, HandlerResult};
use crate::interactions::{AutocompleteInteraction, CommandInteraction, ComponentInteraction, ModalInteraction};
use crate::pipeline::budget::Lane;
use crate::pipeline::dispatch::spawn_handler;
use crate::{Bound, Client};
use dr_model::Intents;
use futures::future::BoxFuture;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// A handler with its state captured and its future boxed: all the table needs to know.
pub type Erased<P> = Arc<dyn Fn(Client, u32, P) -> BoxFuture<'static, HandlerResult> + Send + Sync>;

/// The `on_handler_error` callback.
pub type Failure = Arc<dyn Fn(HandlerFailure) + Send + Sync>;

/// Every handler of a client, by event, then by name or prefix for interactions.
#[derive(Default)]
pub struct HandlerTable {
    by_event: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    needs: Vec<(&'static str, Intents)>,
    /// By command name.
    pub commands: HashMap<&'static str, Erased<Bound<CommandInteraction>>>,
    /// By `custom_id` prefix, in registration order. The first match wins.
    pub components: Vec<(&'static str, Erased<Bound<ComponentInteraction>>)>,
    /// By `custom_id` prefix, in registration order. The first match wins.
    pub modals: Vec<(&'static str, Erased<Bound<ModalInteraction>>)>,
    /// By command name.
    pub autocomplete: HashMap<&'static str, Erased<Bound<AutocompleteInteraction>>>,
    /// Every `on_raw` handler.
    pub raw: Vec<Erased<RawEvent>>,
    /// The `on_handler_error` callback.
    pub on_failure: Option<Failure>,
}

impl std::fmt::Debug for HandlerTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerTable").field("events", &self.by_event.len()).finish_non_exhaustive()
    }
}

impl HandlerTable {
    /// Adds a handler for an event.
    pub fn add<E: GatewayEvent>(&mut self, handler: Erased<E::Payload>) {
        self.needs.push((E::NAME, E::INTENTS));
        let slot = self.by_event.entry(TypeId::of::<E>()).or_insert_with(|| Box::new(Vec::<Erased<E::Payload>>::new()));
        if let Some(list) = slot.downcast_mut::<Vec<Erased<E::Payload>>>() {
            list.push(handler);
        }
    }

    /// The handlers of an event, in registration order.
    pub fn handlers<E: GatewayEvent>(&self) -> &[Erased<E::Payload>] {
        self.by_event
            .get(&TypeId::of::<E>())
            .and_then(|b| b.downcast_ref::<Vec<Erased<E::Payload>>>())
            .map_or(&[], Vec::as_slice)
    }

    /// What each registered event needs, for the check in `build`.
    pub fn needs(&self) -> &[(&'static str, Intents)] {
        &self.needs
    }
}

/// Starts every handler of an event, each in a task of its own. Interactions count against their own
/// budget. Waits only for room in the budget.
pub async fn fire<E: GatewayEvent>(client: &Client, shard: u32, payload: E::Payload) {
    let lane =
        if TypeId::of::<E>() == TypeId::of::<super::InteractionCreate>() { Lane::Interactions } else { Lane::Events };
    for handler in client.0.handlers.handlers::<E>() {
        spawn_handler(client, lane, E::NAME, shard, handler.clone(), payload.clone()).await;
    }
}
