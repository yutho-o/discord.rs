//! Event tokens, the handler table and delivery.

pub mod table;
pub mod tokens;

pub use table::{Erased, Failure, HandlerTable, fire};
pub use tokens::*;

use crate::pipeline::budget::Lane;
use crate::pipeline::dispatch::spawn_handler;
use crate::{Client, Deleted, Updated};
use dr_cache::Change;
use dr_core::DecodeError;
use dr_model::{Dispatch, DispatchKind, Intents};
use std::sync::Arc;
use std::time::Duration;

/// How long `ClientReady` waits for the guild list after `READY`.
const WAIT_GUILD_TIMEOUT: Duration = Duration::from_secs(15);

/// An event you can pass to `ClientBuilder::on`.
pub trait GatewayEvent: Copy + Send + Sync + 'static {
    /// The event's name, used in errors and logs.
    const NAME: &'static str;
    /// Any one of these is enough for the event to arrive. Empty means none is needed.
    const INTENTS: Intents;
    /// What a handler receives. `Clone`, because one event can have several handlers.
    type Payload: Clone + Send + 'static;
}

/// A dispatch as it arrived, for `ClientBuilder::on_raw`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RawEvent {
    /// The event name.
    pub t: String,
    /// The event body, as JSON text.
    pub d: Arc<str>,
    /// Why the typed decode failed, if it did. The typed handlers then do not run.
    pub decode_error: Option<Arc<DecodeError>>,
}

fn updated<T>(client: &Client, change: Change<T>) -> Updated<T> {
    Updated { old: change.old, new: client.bind_arc(change.new) }
}

/// Delivers one dispatch: `on_raw`, typed decode, cache apply, then the handlers. A decode failure is
/// counted and reaches `on_raw`, and nothing else happens: the shard continues.
///
/// The ingest task calls this one frame at a time. Tests call it directly, with no gateway running.
#[doc(hidden)]
pub async fn deliver(client: &Client, shard: u32, kind: DispatchKind, t: &str, d: &str) {
    client.0.stats.record_event();
    let (dispatch, decode_error) = match Dispatch::decode(kind, t, d) {
        Ok(dispatch) => (Some(dispatch), None),
        Err(e) => {
            client.0.stats.record_decode_failure();
            tracing::warn!(event = t, error = %e, "dispatch failed to decode");
            (None, Some(Arc::new(e)))
        }
    };
    if !client.0.handlers.raw.is_empty() {
        let raw = RawEvent { t: t.to_owned(), d: Arc::from(d), decode_error };
        for handler in &client.0.handlers.raw {
            spawn_handler(client, Lane::Events, "RAW", shard, handler.clone(), raw.clone()).await;
        }
    }
    if let Some(dispatch) = dispatch {
        apply_and_fire(client, shard, dispatch).await;
    }
}

async fn apply_and_fire(client: &Client, shard: u32, dispatch: Dispatch) {
    let cache = client.cache();
    match dispatch {
        Dispatch::Ready(event) => {
            client.set_application_id(event.application.id);
            let ready = cache.apply_ready(shard, *event);
            if let Some(ready) = client.0.ready.on_ready(shard, ready) {
                fire::<ClientReady>(client, shard, client.bind_arc(ready)).await;
            } else {
                // Some guilds may never arrive: an outage must not hold `ClientReady` back forever.
                // Not tracked, so that a shutdown does not wait for the timer.
                let client = client.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(WAIT_GUILD_TIMEOUT).await;
                    if client.0.lifecycle.is_stopping() {
                        return;
                    }
                    if let Some(ready) = client.0.ready.on_timeout() {
                        fire::<ClientReady>(&client, shard, client.bind_arc(ready)).await;
                    }
                });
            }
        }
        Dispatch::GuildCreate(guild) => {
            let id = guild.id;
            let guild = cache.apply_guild_create(shard, *guild);
            fire::<GuildCreate>(client, shard, client.bind_arc(guild)).await;
            if let Some(ready) = client.0.ready.on_guild_create(shard, id) {
                fire::<ClientReady>(client, shard, client.bind_arc(ready)).await;
            }
        }
        Dispatch::GuildUpdate(guild) => {
            let change = cache.apply_guild_update(shard, *guild);
            fire::<GuildUpdate>(client, shard, updated(client, change)).await;
        }
        Dispatch::GuildDelete(event) => {
            let cached = cache.apply_guild_delete(shard, event.clone());
            fire::<GuildDelete>(client, shard, Deleted { event: client.bind(event), cached }).await;
        }
        Dispatch::ChannelCreate(channel) => {
            let channel = cache.apply_channel_create(shard, *channel);
            fire::<ChannelCreate>(client, shard, client.bind_arc(channel)).await;
        }
        Dispatch::ChannelUpdate(channel) => {
            let change = cache.apply_channel_update(shard, *channel);
            fire::<ChannelUpdate>(client, shard, updated(client, change)).await;
        }
        Dispatch::ChannelDelete(channel) => {
            let cached = cache.apply_channel_delete(shard, (*channel).clone());
            fire::<ChannelDelete>(client, shard, Deleted { event: client.bind(*channel), cached }).await;
        }
        Dispatch::GuildMemberAdd(member) => {
            let member = cache.apply_member_add(shard, *member);
            fire::<GuildMemberAdd>(client, shard, client.bind_arc(member)).await;
        }
        Dispatch::GuildMemberUpdate(event) => {
            let change = cache.apply_member_update(shard, *event);
            fire::<GuildMemberUpdate>(client, shard, updated(client, change)).await;
        }
        Dispatch::GuildMemberRemove(event) => {
            let cached = cache.apply_member_remove(shard, (*event).clone());
            fire::<GuildMemberRemove>(client, shard, Deleted { event: client.bind(*event), cached }).await;
        }
        Dispatch::MessageCreate(message) => {
            let message = cache.apply_message_create(shard, *message);
            fire::<MessageCreate>(client, shard, client.bind_arc(message)).await;
        }
        Dispatch::MessageUpdate(event) => {
            let change = cache.apply_message_update(shard, *event);
            fire::<MessageUpdate>(client, shard, updated(client, change)).await;
        }
        Dispatch::MessageDelete(event) => {
            let cached = cache.apply_message_delete(shard, (*event).clone());
            fire::<MessageDelete>(client, shard, Deleted { event: client.bind(*event), cached }).await;
        }
        Dispatch::InteractionCreate(interaction) => {
            crate::interactions::router::route(client, shard, *interaction).await;
        }
        // `Resumed`, `Unknown` and any later arm are reached by `on_raw` only.
        _ => {}
    }
}
