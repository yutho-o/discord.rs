//! Webhooks: creation, posting with the webhook's token alone, edits and messages, on the mock.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use discord::prelude::*;
use dr_mock::{Method, MockServer, RecordedRequest};
use serde_json::json;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

fn last(server: &MockServer) -> RecordedRequest {
    server.requests().pop().unwrap()
}

#[tokio::test]
async fn a_webhook_is_created_and_posts_without_the_bot_token() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "news");

    let avatar = ImageData::png(b"\x89PNG");
    let webhook = client.channel(channel).webhooks().create(CreateWebhook::new("Herald").avatar(avatar)).await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/channels/{channel}/webhooks"));
    assert_eq!(request.json(), json!({"name": "Herald", "avatar": "data:image/png;base64,iVBORw=="}));
    assert!(webhook.is_incoming());
    assert!(!format!("{:?}", webhook.data()).contains("token-"), "Debug hides the token");

    let hook = webhook.handle();
    let body = ExecuteWebhook::new("Extra! Extra!").username("Paperboy");
    let message = hook.execute(body).await.unwrap();
    let request = last(&server);
    assert!(request.header("authorization").is_none(), "the webhook's token is in the path");
    assert_eq!(request.query.as_deref(), Some("wait=true"));
    assert_eq!(request.json(), json!({"content": "Extra! Extra!", "username": "Paperboy"}));
    assert_eq!(message.content, "Extra! Extra!");
    assert_eq!(message.webhook_id, Some(webhook.id));
    assert_eq!(message.author.username, "Paperboy");
    assert_eq!(message.channel_id, channel);

    let edited = hook.edit_message(message.id, EditMessage::new().content("Correction")).await.unwrap();
    assert_eq!(edited.content, "Correction");
    assert_eq!(hook.fetch_message(message.id).await.unwrap().content, "Correction");
    hook.delete_message(message.id).await.unwrap();
    assert!(hook.fetch_message(message.id).await.is_err());
}

#[tokio::test]
async fn a_webhook_url_is_enough() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "news");
    let webhook = client.channel(channel).webhooks().create(CreateWebhook::new("Herald")).await.unwrap();
    let token = webhook.token.as_ref().unwrap().expose().to_owned();
    let url = format!("https://discord.com/api/webhooks/{}/{token}", webhook.id);

    let hook = client.webhook_from_url(&url).unwrap();
    hook.execute(CreateMessage::new().content("file").attach(CreateAttachment::bytes("x", "x.txt"))).await.unwrap();
    assert_eq!(last(&server).files()[0].data, b"x");
    assert_eq!(hook.fetch().await.unwrap().name.as_deref(), Some("Herald"));
    assert!(client.webhook_from_url("https://example.com/nothing").is_none());

    let wrong = client.webhook(webhook.id, "not-the-token");
    assert!(wrong.execute("x").await.is_err(), "a wrong token is refused");
}

#[tokio::test]
async fn edits_use_the_right_token() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let news = server.world().add_channel(guild, "news");
    let other = server.world().add_channel(guild, "other");
    let webhook = client.channel(news).webhooks().create(CreateWebhook::new("Herald")).await.unwrap();
    let hook = webhook.handle();

    let renamed = hook.edit(EditWebhook::new().name("Crier").clear_avatar()).await.unwrap();
    let request = last(&server);
    assert!(request.header("authorization").is_none(), "a rename goes with the webhook's token");
    assert_eq!(request.json(), json!({"name": "Crier", "avatar": null}));
    assert_eq!(renamed.name.as_deref(), Some("Crier"));

    let moved = hook.edit(EditWebhook::new().channel(other)).await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/webhooks/{}", webhook.id), "a move needs the bot's token");
    assert!(request.header("authorization").is_some());
    assert_eq!(moved.channel_id, Some(other));

    assert_eq!(client.guild(guild).webhooks().await.unwrap().len(), 1);
    assert_eq!(client.channel(other).webhooks().fetch_all().await.unwrap().len(), 1);
    client.webhook_by_id(webhook.id).delete().await.unwrap();
    assert_eq!(last(&server).method, Method::DELETE);
    assert!(client.guild(guild).webhooks().await.unwrap().is_empty());
}

#[tokio::test]
async fn bad_webhook_bodies_and_missing_tokens_send_nothing() {
    let (server, client) = fixture().await;
    let webhooks = client.channel(1.into()).webhooks();
    assert!(webhooks.create(CreateWebhook::new("")).await.is_err());
    assert!(webhooks.create(CreateWebhook::new("Discord bot")).await.is_err(), "a reserved word");
    let hook = client.webhook(2.into(), "tok");
    assert!(hook.execute(ExecuteWebhook::new("x").username("clyde")).await.is_err());
    assert!(hook.execute(CreateMessage::new()).await.is_err(), "an empty message");
    let tokenless = client.webhook_by_id(2.into());
    assert!(tokenless.execute("x").await.is_err());
    assert!(tokenless.fetch_message(3.into()).await.is_err());
    assert!(server.requests().is_empty(), "nothing was sent");
}
