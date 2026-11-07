//! What the facade's tests share: wire payloads and a way to wait.
#![allow(dead_code, clippy::panic, clippy::unwrap_used)]

use dr_mock::World;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Polls until `done`, failing the test after 20 s. `what` names it in the failure.
pub async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

pub const TIME: &str = "2020-01-01T00:00:00.000000+00:00";

pub fn user(id: u64, name: &str) -> Value {
    json!({"id": id.to_string(), "username": name, "discriminator": "0", "global_name": null, "avatar": null})
}

/// A `MESSAGE_CREATE` body from a human.
pub fn message(world: &World, guild: impl ToString, channel: impl ToString, author: u64, content: &str) -> Value {
    json!({
        "id": world.next_id().to_string(),
        "channel_id": channel.to_string(),
        "guild_id": guild.to_string(),
        "author": user(author, "alice"),
        "member": {"roles": [], "joined_at": TIME, "deaf": false, "mute": false},
        "content": content,
        "timestamp": TIME,
        "edited_timestamp": null,
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0,
    })
}

fn interaction(world: &World, kind: u32, guild: impl ToString, channel: impl ToString, who: u64, data: Value) -> Value {
    let id = world.next_id();
    json!({
        "id": id.to_string(),
        "application_id": world.application_id().to_string(),
        "type": kind,
        "token": format!("token-{id}"),
        "version": 1,
        "guild_id": guild.to_string(),
        "channel_id": channel.to_string(),
        "member": {"user": user(who, "alice"), "roles": [], "joined_at": TIME, "deaf": false, "mute": false, "permissions": "0"},
        "data": data,
    })
}

/// An `INTERACTION_CREATE` body for a slash command.
pub fn slash(
    world: &World,
    guild: impl ToString,
    channel: impl ToString,
    who: u64,
    name: &str,
    options: Value,
) -> Value {
    let data = json!({"id": "1", "name": name, "type": 1, "options": options});
    interaction(world, 2, guild, channel, who, data)
}

/// A string option.
pub fn string_option(name: &str, value: &str) -> Value {
    json!({"name": name, "type": 3, "value": value})
}

/// An `INTERACTION_CREATE` body for a button click on `message`.
pub fn click(
    world: &World,
    guild: impl ToString,
    channel: impl ToString,
    who: u64,
    custom_id: &str,
    message: Value,
) -> Value {
    let data = json!({"custom_id": custom_id, "component_type": 2});
    let mut body = interaction(world, 3, guild, channel, who, data);
    body["message"] = message;
    body
}

/// The callback path of an interaction body.
pub fn callback_path(body: &Value) -> String {
    format!("/interactions/{}/{}/callback", body["id"].as_str().unwrap(), body["token"].as_str().unwrap())
}
