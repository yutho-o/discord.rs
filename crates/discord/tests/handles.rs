//! Handles and managers: get reads the cache, fetch asks Discord, resolve does both.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use discord::model::DispatchKind as K;
use discord::prelude::*;
use discord::{Bound, Error};
use dr_mock::{Method, MockServer};
use serde_json::json;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

#[tokio::test]
async fn resolve_fetches_once_then_serves_from_the_cache() {
    let (server, client) = fixture().await;
    let alice = server.world().add_user("alice");
    assert!(client.users().get(alice).is_none());
    assert_eq!(client.users().resolve(alice).await.unwrap().username, "alice");
    assert_eq!(client.users().resolve(alice).await.unwrap().username, "alice");
    let fetches = server.requests().into_iter().filter(|r| r.path == format!("/users/{alice}")).count();
    assert_eq!(fetches, 1);
    assert!(client.user(alice).cached().is_some());
}

#[tokio::test]
async fn a_fetched_member_knows_its_guild() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let alice = server.world().add_user("alice");
    server.world().add_member(guild, alice);
    let member = client.guild(guild).members().fetch(alice).await.unwrap();
    assert_eq!(member.guild_id, Some(guild));
    assert!(client.guild(guild).members().get(alice).is_none(), "fetch does not enter the cache");
    client.guild(guild).members().resolve(alice).await.unwrap();
    assert!(client.guild(guild).members().get(alice).is_some(), "resolve does");
    member.kick().reason("test").await.unwrap();
    let kick = server.requests().into_iter().find(|r| r.method == Method::DELETE).unwrap();
    assert_eq!(kick.path, format!("/guilds/{guild}/members/{alice}"));
    assert_eq!(kick.header("x-audit-log-reason"), Some("test"));
    assert!(client.guild(guild).members().fetch(alice).await.is_err());
}

#[tokio::test]
async fn messages_go_through_their_whole_life() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let handle = client.channel(channel);
    let sent: Bound<Message> = handle.send("hello").await.unwrap();
    assert_eq!(sent.content, "hello");
    assert_eq!(handle.messages().fetch(sent.id).await.unwrap().content, "hello");
    let edited = sent.edit(EditMessage::new().content("bye")).await.unwrap();
    assert_eq!(edited.content, "bye");
    edited.react("👍").await.unwrap();
    let reaction = server.requests().into_iter().find(|r| r.path.contains("/reactions/")).unwrap();
    assert!(reaction.path.ends_with("/reactions/%F0%9F%91%8D/@me"), "{}", reaction.path);
    handle.typing().await.unwrap();
    edited.delete().await.unwrap();
    assert!(handle.messages().fetch(sent.id).await.is_err());
    assert_eq!(sent.guild().unwrap().id(), guild);
    assert_eq!(sent.channel().id(), channel);
}

#[tokio::test]
async fn an_invalid_message_is_rejected_before_any_request() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let failed = client.channel(channel).send("x".repeat(2001)).await.unwrap_err();
    assert!(matches!(failed, Error::Validation(_)), "{failed:?}");
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn commands_need_ready_first() {
    let (server, client) = fixture().await;
    let cmds = commands![SlashCommandBuilder::new("a", "b")];
    let early = client.commands().set_global(cmds.clone()).await;
    assert!(matches!(early, Err(Error::NotReady)), "{early:?}");

    client.set_application_id(server.world().application_id());
    let set = client.commands().set_global(cmds.clone()).await.unwrap();
    assert_eq!(set[0].name, "a");
    assert_eq!(client.commands().list_global().await.unwrap().len(), 1);
    let guild = server.world().add_guild("g");
    client.commands().set_guild(guild, cmds).await.unwrap();
    assert_eq!(client.commands().list_guild(guild).await.unwrap().len(), 1);
}

#[tokio::test]
async fn latest_prefers_the_newest_cached_version() {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t")
        .endpoints(server.endpoints())
        .cache(CacheConfig::standard().messages_per_channel(5))
        .build()
        .unwrap()
        .client();
    let body = json!({"id": "3", "channel_id": "2", "author": {"id": "5", "username": "u"}, "content": "old"});
    let stale: Message = serde_json::from_value(body.clone()).unwrap();
    let stale = client.bind(stale);
    assert_eq!(stale.latest().content, "old", "not cached yet");
    discord::events::deliver(&client, 0, K::MessageCreate, "MESSAGE_CREATE", &body.to_string()).await;
    let edit = json!({"id": "3", "channel_id": "2", "content": "new"});
    discord::events::deliver(&client, 0, K::MessageUpdate, "MESSAGE_UPDATE", &edit.to_string()).await;
    assert_eq!(stale.latest().content, "new");
    assert_eq!(client.channel(2.into()).messages().get(3.into()).unwrap().content, "new");
}
