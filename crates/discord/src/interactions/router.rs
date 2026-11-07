//! From a wire interaction to its handlers.

use super::{AutocompleteInteraction, CommandInteraction, ComponentInteraction, Interaction, ModalInteraction};
use crate::Client;
use crate::events::{InteractionCreate, fire};
use crate::pipeline::budget::Lane;
use crate::pipeline::dispatch::spawn_handler;
use dr_model::{CommandData, InteractionData, ModalSubmitData};
use std::sync::Arc;

const EVENT: &str = "INTERACTION_CREATE";

fn command(wire: &dr_model::Interaction, data: CommandData, user: dr_model::User) -> CommandInteraction {
    CommandInteraction {
        id: wire.id,
        application_id: wire.application_id,
        token: wire.token.clone(),
        command_id: data.id,
        name: data.name,
        kind: data.kind,
        guild_id: wire.guild_id,
        channel_id: wire.channel_id,
        user,
        member: wire.member.clone(),
        options: data.options,
        resolved: data.resolved,
        reply_state: Arc::default(),
    }
}

/// Each text input's `custom_id` and value.
fn modal_fields(data: &ModalSubmitData) -> Vec<(String, String)> {
    let inputs = data.hoisted();
    let ids = inputs.iter().filter_map(|c| c.custom_id());
    ids.filter_map(|id| Some((id.to_owned(), data.text_input_value(id)?.to_owned()))).collect()
}

/// Builds the facade value for a wire interaction, then runs the routed handler (`on_command` by
/// name, `on_component` and `on_modal` by first matching prefix, `on_autocomplete` by name), then any
/// `.on(InteractionCreate)` handlers. Both go through `pipeline::dispatch::spawn_handler` in
/// `Lane::Interactions`. An interaction with data that does not decode goes to
/// `Interaction::Unknown`, and is counted. It never panics and never fails the shard.
#[doc(hidden)]
pub async fn route(client: &Client, shard: u32, mut wire: dr_model::Interaction) {
    let table = &client.0.handlers;
    let user = wire.invoker().cloned();
    let data = wire.data.take();
    let message = wire.message.take();
    let payload = match (data, user, message) {
        (Some(InteractionData::ApplicationCommand(data)), Some(user), message) => {
            drop(message);
            let bound = client.bind(command(&wire, data, user));
            if let Some(handler) = table.commands.get(bound.name.as_str()) {
                spawn_handler(client, Lane::Interactions, EVENT, shard, handler.clone(), bound.clone()).await;
            }
            Interaction::Command(bound)
        }
        (Some(InteractionData::Autocomplete(data)), Some(user), message) => {
            drop(message);
            let inner = AutocompleteInteraction { command: command(&wire, data, user) };
            let bound = client.bind(inner);
            if let Some(handler) = table.autocomplete.get(bound.command.name.as_str()) {
                spawn_handler(client, Lane::Interactions, EVENT, shard, handler.clone(), bound.clone()).await;
            }
            Interaction::Autocomplete(bound)
        }
        (Some(InteractionData::MessageComponent(data)), Some(user), Some(message)) => {
            let bound = client.bind(ComponentInteraction {
                id: wire.id,
                application_id: wire.application_id,
                token: wire.token.clone(),
                custom_id: data.custom_id,
                component_type: data.component_type,
                values: data.values,
                message: *message,
                guild_id: wire.guild_id,
                channel_id: wire.channel_id,
                user,
                member: wire.member.clone(),
                reply_state: Arc::default(),
            });
            let found = table.components.iter().find(|(prefix, _)| bound.custom_id.starts_with(prefix));
            if let Some((_, handler)) = found {
                spawn_handler(client, Lane::Interactions, EVENT, shard, handler.clone(), bound.clone()).await;
            }
            Interaction::Component(bound)
        }
        (Some(InteractionData::ModalSubmit(data)), Some(user), message) => {
            drop(message);
            let fields = modal_fields(&data);
            let bound = client.bind(ModalInteraction {
                id: wire.id,
                application_id: wire.application_id,
                token: wire.token.clone(),
                custom_id: data.custom_id,
                fields,
                guild_id: wire.guild_id,
                channel_id: wire.channel_id,
                user,
                member: wire.member.clone(),
                reply_state: Arc::default(),
            });
            let found = table.modals.iter().find(|(prefix, _)| bound.custom_id.starts_with(prefix));
            if let Some((_, handler)) = found {
                spawn_handler(client, Lane::Interactions, EVENT, shard, handler.clone(), bound.clone()).await;
            }
            Interaction::ModalSubmit(bound)
        }
        (data, _, message) => {
            if matches!(data, Some(InteractionData::Malformed { .. })) {
                client.0.stats.record_decode_failure();
            }
            wire.data = data;
            wire.message = message;
            Interaction::Unknown(client.bind(wire))
        }
    };
    fire::<InteractionCreate>(client, shard, payload).await;
}
