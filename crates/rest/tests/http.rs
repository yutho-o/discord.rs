//! REST over real HTTP against the mock, with faults the scripted transport cannot produce.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::Secret;
use dr_mock::{Fault, Method as M, MockServer};
use dr_model::{ChannelId, UserId};
use dr_rest::{Method, Rest, RestError};
use serde_json::{Value, json};

async fn fixture() -> (MockServer, Rest) {
    let server = MockServer::start().await.unwrap();
    let rest = Rest::builder().token(Secret::new("Bot abc".to_owned())).endpoints(server.endpoints()).build().unwrap();
    (server, rest)
}

#[tokio::test]
async fn the_path_and_headers_are_what_discord_expects() {
    let (server, rest) = fixture().await;
    let me = rest.users().get_current().await.unwrap();
    assert_eq!(me.username, "mockbot");
    let sent = &server.requests()[0];
    assert_eq!(sent.path, "/users/%40me", "the @ is encoded");
    assert_eq!(sent.header("authorization"), Some("Bot abc"), "one Bot prefix");
    assert!(sent.header("user-agent").unwrap().starts_with("DiscordBot"), "{:?}", sent.header("user-agent"));
}

#[tokio::test]
async fn the_gateway_url_comes_from_rest() {
    let (server, rest) = fixture().await;
    let bot = rest.gateway().get_bot().await.unwrap();
    assert_eq!(Some(bot.url), server.endpoints().gateway_override);
    assert_eq!(bot.session_start_limit.max_concurrency, 1);
}

#[tokio::test]
async fn a_reset_before_the_answer_resends_a_get_and_not_a_post() {
    let (server, rest) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let path = format!("/channels/{channel}");
    server.rest().on(M::GET, path.clone()).times(1).fault(Fault::ResetBeforeResponse);
    rest.channels().get(channel).await.unwrap();
    assert_eq!(server.requests().iter().filter(|r| r.path == path).count(), 2, "a GET is retried");

    let path = format!("/channels/{channel}/messages");
    server.rest().on(M::POST, path.clone()).times(1).fault(Fault::ResetBeforeResponse);
    let failed = rest.channels().create_message(channel, json!({"content": "x"})).await;
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(server.requests().iter().filter(|r| r.path == path).count(), 1, "a plain POST is not");
}

#[tokio::test]
async fn a_non_json_5xx_is_an_error_with_the_status() {
    let (server, rest) = fixture().await;
    let id = UserId::new(5);
    let path = format!("/users/{id}");
    server.rest().on(M::GET, path).fault(Fault::NonJson5xx);
    let failed = rest.users().get(id).await.unwrap_err();
    assert!(failed.is_retryable(), "{failed:?}");
}

#[tokio::test]
async fn a_truncated_body_is_an_error() {
    let (server, rest) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel: ChannelId = server.world().add_channel(guild, "c");
    let path = format!("/channels/{channel}/messages");
    server.rest().on(M::POST, path).times(1).fault(Fault::TruncatedBody);
    let failed = rest.channels().create_message(channel, json!({"content": "x"})).await;
    assert!(failed.is_err(), "{failed:?}");
}

#[tokio::test]
async fn a_headerless_429_is_retried_at_its_body_time() {
    let (server, rest) = fixture().await;
    let id = server.world().add_user("alice");
    let path = format!("/users/{id}");
    server.rest().on(M::GET, path.clone()).times(1).fault(Fault::HeaderlessRateLimit);
    assert_eq!(rest.users().get(id).await.unwrap().username, "alice");
    assert_eq!(server.requests().iter().filter(|r| r.path == path).count(), 2);
}

#[tokio::test]
async fn a_cloudflare_429_surfaces_as_a_rate_limit_or_retries() {
    let (server, rest) = fixture().await;
    let id = server.world().add_user("alice");
    server.rest().on(M::GET, format!("/users/{id}")).times(1).fault(Fault::CloudflareRateLimit);
    assert_eq!(rest.users().get(id).await.unwrap().username, "alice");
}

#[tokio::test]
async fn unknown_things_are_api_errors_with_their_code() {
    let (_server, rest) = fixture().await;
    let failed = rest.channels().get(ChannelId::new(1)).await.unwrap_err();
    let RestError::Api { .. } = &failed else { panic!("{failed:?}") };
    assert_eq!(failed.code().map(|c| c.0), Some(10003));
    let raw = rest.raw::<Value>(Method::Get, "/nope").await;
    assert!(raw.is_err());
}
