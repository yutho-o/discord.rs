//! Collectors: waiting in code for messages, clicks and reactions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{click, message};
use discord::model::DispatchKind as K;
use discord::prelude::*;
use dr_mock::MockServer;
use futures::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

async fn deliver(client: &Client, kind: K, t: &str, body: &Value) {
    discord::events::deliver(client, 0, kind, t, &body.to_string()).await;
}

const SOON: Duration = Duration::from_secs(5);

#[tokio::test]
async fn a_message_is_awaited_in_its_channel() {
    let (server, client) = fixture().await;
    let world = server.world();
    let waiter = {
        let channel = client.channel(ChannelId::new(2));
        tokio::spawn(async move { channel.await_message(|m| m.content.starts_with("yes"), SOON).await })
    };
    tokio::task::yield_now().await;
    // Give the waiter time to register before anything arrives.
    tokio::time::sleep(Duration::from_millis(50)).await;
    deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 3, 5, "yes, other channel")).await;
    deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 2, 5, "no")).await;
    deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 2, 5, "yes please")).await;
    let got = waiter.await.unwrap().unwrap();
    assert_eq!(got.content, "yes please");
}

#[tokio::test]
async fn nothing_arrives_before_the_limit() {
    let (_server, client) = fixture().await;
    let waited = client.channel(ChannelId::new(2)).await_message(|_| true, Duration::from_millis(30)).await;
    assert!(waited.is_none());
}

#[tokio::test]
async fn a_collector_gathers_several_and_reads_as_a_stream() {
    let (server, client) = fixture().await;
    let world = server.world();
    let mut stream = client.collector::<MessageCreate>(|m| m.author.id == UserId::new(5));
    let batch = client.collector::<MessageCreate>(|_| true);
    for (author, text) in [(5, "a"), (6, "b"), (5, "c")] {
        deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 2, author, text)).await;
    }
    assert_eq!(stream.next().await.unwrap().content, "a");
    assert_eq!(StreamExt::next(&mut stream).await.unwrap().content, "c");
    let all = batch.take(2, SOON).await;
    assert_eq!(all.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), ["a", "b"], "stops at max");
}

#[tokio::test]
async fn a_click_on_a_message_is_awaited_and_answered() {
    let (server, client) = fixture().await;
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "general");
    let sent = client.channel(channel).send("Pick one").await.unwrap();
    let other = client.channel(channel).send("Another").await.unwrap();
    let wire = world.message(channel.get(), sent.id.get()).unwrap();
    let other_wire = world.message(channel.get(), other.id.get()).unwrap();

    let waiter = {
        let sent = sent.clone();
        tokio::spawn(async move { sent.await_component(SOON).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    deliver(&client, K::InteractionCreate, "INTERACTION_CREATE", &click(world, guild, channel, 5, "no", other_wire))
        .await;
    deliver(&client, K::InteractionCreate, "INTERACTION_CREATE", &click(world, guild, channel, 5, "yes", wire)).await;
    let clicked = waiter.await.unwrap().unwrap();
    assert_eq!(clicked.custom_id, "yes", "the click on the other message was not taken");
    clicked.update(EditMessage::new().content("Picked")).await.unwrap();
    assert_eq!(server.requests().last().unwrap().json()["type"], 7);
}

#[tokio::test]
async fn a_component_collector_filters_by_user() {
    let (server, client) = fixture().await;
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "general");
    let sent = client.channel(channel).send("Vote").await.unwrap();
    let wire = world.message(channel.get(), sent.id.get()).unwrap();
    let mut votes = sent.component_collector(|c| c.user.id == UserId::new(6));
    deliver(&client, K::InteractionCreate, "INTERACTION_CREATE", &click(world, guild, channel, 5, "a", wire.clone()))
        .await;
    deliver(&client, K::InteractionCreate, "INTERACTION_CREATE", &click(world, guild, channel, 6, "b", wire)).await;
    assert_eq!(votes.next_within(SOON).await.unwrap().custom_id, "b");
    assert!(votes.next_within(Duration::from_millis(20)).await.is_none());
}

#[tokio::test]
async fn a_reaction_is_awaited_on_its_message() {
    let (server, client) = fixture().await;
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "general");
    let sent = client.channel(channel).send("React").await.unwrap();
    let waiter = {
        let sent = sent.clone();
        tokio::spawn(async move { sent.await_reaction(|r| r.emoji.name.as_deref() == Some("👍"), SOON).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    let reaction = |emoji: &str| {
        json!({"user_id": "5", "channel_id": channel.to_string(), "message_id": sent.id.to_string(),
               "emoji": {"id": null, "name": emoji}, "burst": false, "type": 0})
    };
    deliver(&client, K::MessageReactionAdd, "MESSAGE_REACTION_ADD", &reaction("👎")).await;
    deliver(&client, K::MessageReactionAdd, "MESSAGE_REACTION_ADD", &reaction("👍")).await;
    let got = waiter.await.unwrap().unwrap();
    assert_eq!(got.user_id, UserId::new(5));
}

#[tokio::test]
async fn an_unread_collector_never_blocks_delivery() {
    let (server, client) = fixture().await;
    let world = server.world();
    let unread = client.collector::<MessageCreate>(|_| true);
    for i in 0..300 {
        deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 2, 5, &i.to_string())).await;
    }
    let kept = unread.take(1000, Duration::from_millis(50)).await;
    assert_eq!(kept.len(), 256, "the backlog is bounded");
    assert_eq!(kept[0].content, "0", "the oldest are kept");

    let dropped = client.collector::<MessageCreate>(|_| true);
    drop(dropped);
    deliver(&client, K::MessageCreate, "MESSAGE_CREATE", &message(world, 1, 2, 5, "after")).await;
}
