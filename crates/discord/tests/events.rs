//! Every event token fires, whatever the cache knows.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::model::DispatchKind as K;
use discord::prelude::*;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

macro_rules! record {
    ($builder:expr, $fired:expr, $($token:ident),* $(,)?) => {{
        let builder = $builder;
        $(
            let fired = $fired.clone();
            let builder = builder.on($token, move |_ctx, _payload| {
                let fired = fired.clone();
                async move {
                    fired.lock().unwrap().insert(stringify!($token));
                    Ok(())
                }
            });
        )*
        builder
    }};
}

const EVENTS: [&str; 14] = [
    "ClientReady",
    "MessageCreate",
    "MessageUpdate",
    "MessageDelete",
    "GuildCreate",
    "GuildUpdate",
    "GuildDelete",
    "ChannelCreate",
    "ChannelUpdate",
    "ChannelDelete",
    "GuildMemberAdd",
    "GuildMemberUpdate",
    "GuildMemberRemove",
    "InteractionCreate",
];

async fn every_event_fires(config: CacheConfig) {
    let fired = Arc::new(Mutex::new(BTreeSet::new()));
    let builder = Client::builder("t").allow_missing_intents().cache(config);
    let runner = record!(
        builder,
        fired,
        ClientReady,
        MessageCreate,
        MessageUpdate,
        MessageDelete,
        GuildCreate,
        GuildUpdate,
        GuildDelete,
        ChannelCreate,
        ChannelUpdate,
        ChannelDelete,
        GuildMemberAdd,
        GuildMemberUpdate,
        GuildMemberRemove,
        InteractionCreate,
    )
    .build()
    .unwrap();
    let client = runner.client();
    let user = json!({"id": "5", "username": "u"});
    let guild = json!({"id": "1", "name": "g", "owner_id": "5"});
    let channel = json!({"id": "2", "type": 0, "guild_id": "1"});
    let member = json!({"guild_id": "1", "user": user, "roles": []});
    let message = json!({"id": "3", "channel_id": "2", "author": user, "content": "x"});
    let partial = json!({"id": "3", "channel_id": "2", "content": "y"});
    let ready = json!({
        "v": 10, "user": user, "guilds": [], "session_id": "s", "resume_gateway_url": "ws://x",
        "application": {"id": "9", "flags": 0},
    });
    let interaction = json!({
        "id": "4", "application_id": "9", "type": 2, "token": "tok", "version": 1, "channel_id": "2",
        "user": user, "data": {"id": "6", "name": "ping", "type": 1},
    });
    let events = [
        (K::Ready, "READY", ready),
        (K::GuildCreate, "GUILD_CREATE", guild.clone()),
        (K::GuildUpdate, "GUILD_UPDATE", guild),
        (K::ChannelCreate, "CHANNEL_CREATE", channel.clone()),
        (K::ChannelUpdate, "CHANNEL_UPDATE", channel.clone()),
        (K::GuildMemberAdd, "GUILD_MEMBER_ADD", member.clone()),
        (K::GuildMemberUpdate, "GUILD_MEMBER_UPDATE", member),
        (K::MessageCreate, "MESSAGE_CREATE", message),
        (K::MessageUpdate, "MESSAGE_UPDATE", partial),
        (K::MessageDelete, "MESSAGE_DELETE", json!({"id": "3", "channel_id": "2"})),
        (K::GuildMemberRemove, "GUILD_MEMBER_REMOVE", json!({"guild_id": "1", "user": user})),
        (K::ChannelDelete, "CHANNEL_DELETE", channel),
        (K::GuildDelete, "GUILD_DELETE", json!({"id": "1"})),
        (K::InteractionCreate, "INTERACTION_CREATE", interaction),
    ];
    for (kind, name, body) in events {
        discord::events::deliver(&client, 0, kind, name, &body.to_string()).await;
    }
    assert_eq!(client.stats().decode_failures(), 0, "every body above is valid");
    wait_until("every event", || fired.lock().unwrap().len() == EVENTS.len()).await;
    let fired = fired.lock().unwrap();
    for name in EVENTS {
        assert!(fired.contains(name), "{name} did not fire");
    }
}

#[tokio::test]
async fn every_event_fires_with_empty_cache() {
    every_event_fires(CacheConfig::none()).await;
}

#[tokio::test]
async fn every_event_fires_with_the_standard_cache() {
    every_event_fires(CacheConfig::standard()).await;
}

#[tokio::test]
async fn update_events_carry_the_old_value_when_cached() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let runner = Client::builder("t")
        .allow_missing_intents()
        .cache(CacheConfig::standard().messages_per_channel(10))
        .on(MessageUpdate, move |_ctx, update| {
            let tx = tx.clone();
            async move {
                tx.send(update).await?;
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let created = json!({"id": "3", "channel_id": "2", "author": {"id": "5", "username": "u"}, "content": "old"});
    discord::events::deliver(&client, 0, K::MessageCreate, "MESSAGE_CREATE", &created.to_string()).await;
    let edit = json!({"id": "3", "channel_id": "2", "content": "new"});
    discord::events::deliver(&client, 0, K::MessageUpdate, "MESSAGE_UPDATE", &edit.to_string()).await;
    let update = rx.recv().await.unwrap();
    assert_eq!(update.old.unwrap().content, "old");
    assert_eq!(update.new.content, "new");
    assert_eq!(update.new.author.username, "u", "the rest comes from the cached message");
    assert_eq!(client.cache().channel_messages(2.into())[0].content, "new");
}
