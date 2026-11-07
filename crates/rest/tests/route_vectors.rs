//! The vectors of `docs/design/rest.md` section 3.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_rest::Method::{self, Delete, Get, Patch, Post, Put};
use dr_rest::route::route_info;
use std::time::{SystemTime, UNIX_EPOCH};

const CH: &str = "339942739275677727";

#[test]
fn route_vectors() {
    let vectors: &[(Method, &str, &str, &str)] = &[
        (Get, "/channels/339942739275677727", "/channels/:id", CH),
        (Post, "/channels/339942739275677727/messages", "/channels/:id/messages", CH),
        (Get, "/channels/339942739275677727/messages/392063687801700356", "/channels/:id/messages/:id", CH),
        (
            Delete,
            "/channels/339942739275677727/messages/392063687801700356",
            "/channels/:id/messages/:id/Delete Old Message",
            CH,
        ),
        (
            Put,
            "/channels/339942739275677727/messages/392063687801700356/reactions/%F0%9F%91%8D/@me",
            "/channels/:id/messages/:id/reactions/:reaction",
            CH,
        ),
        (
            Delete,
            "/channels/339942739275677727/messages/392063687801700356/reactions/name%3A180162837826289664/180162837826289664",
            "/channels/:id/messages/:id/reactions/:reaction",
            CH,
        ),
        (
            Get,
            "/channels/339942739275677727/messages/392063687801700356/reactions/%F0%9F%91%8D",
            "/channels/:id/messages/:id/reactions/:reaction",
            CH,
        ),
        (
            Delete,
            "/channels/339942739275677727/messages/392063687801700356/reactions",
            "/channels/:id/messages/:id/reactions",
            CH,
        ),
        (Get, "/guilds/222078108977594368/members/180162837826289664", "/guilds/:id/members/:id", "222078108977594368"),
        (Patch, "/guilds/222078108977594368/members/%40me", "/guilds/:id/members/%40me", "222078108977594368"),
        (Get, "/guilds/templates/abcDEF", "/guilds/templates/abcDEF", "global"),
        (Patch, "/guilds/222078108977594368/templates/abcDEF", "/guilds/:id/templates/abcDEF", "222078108977594368"),
        (Get, "/webhooks/1234567890123456789", "/webhooks/:id", "1234567890123456789"),
        (Post, "/webhooks/1234567890123456789/tok-en_1", "/webhooks/:id/:token", "1234567890123456789/tok-en_1"),
        (
            Patch,
            "/webhooks/1234567890123456789/tok-en_1/messages/%40original",
            "/webhooks/:id/:token/messages/%40original",
            "1234567890123456789/tok-en_1",
        ),
        (
            Delete,
            "/webhooks/1234567890123456789/tok-en_1/messages/392063687801700356",
            "/webhooks/:id/:token/messages/:id",
            "1234567890123456789/tok-en_1",
        ),
        (
            Post,
            "/webhooks/1234567890123456789/tok-en_1/slack",
            "/webhooks/:id/:token/slack",
            "1234567890123456789/tok-en_1",
        ),
        (Post, "/interactions/1234567890123456789/tok-en_1/callback", "/interactions/:id/:token/callback", "burst"),
        (Get, "/gateway/bot", "/gateway/bot", "global"),
        (Get, "/users/%40me", "/users/%40me", "global"),
        (Get, "/users/180162837826289664", "/users/:id", "global"),
        (Get, "/invites/abcDEF", "/invites/abcDEF", "global"),
        (
            Put,
            "/applications/1234567890123456789/guilds/222078108977594368/commands",
            "/applications/:id/guilds/:id/commands",
            "global",
        ),
        (Get, "/channels/not-a-snowflake", "/channels/not-a-snowflake", "global"),
    ];
    for (method, path, bucket, major) in vectors {
        let info = route_info(*method, path);
        let key = format!("{}:{bucket}", method.as_str());
        assert_eq!(info.bucket_route, key, "{path}");
        assert_eq!(info.major, *major, "{path}");
    }
}

#[test]
fn a_recent_message_delete_has_the_ordinary_bucket() {
    const DISCORD_EPOCH_MS: u128 = 1_420_070_400_000;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let snowflake = (now - DISCORD_EPOCH_MS) << 22;
    let info = route_info(Delete, &format!("/channels/{CH}/messages/{snowflake}"));
    assert_eq!(info.bucket_route, "DELETE:/channels/:id/messages/:id");
}

#[test]
fn only_callbacks_are_burst() {
    assert!(route_info(Post, "/interactions/1234567890123456789/tok/callback").is_burst());
    assert!(!route_info(Post, "/channels/339942739275677727/messages").is_burst());
}
