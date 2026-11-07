//! Endpoint handlers over the [`crate::World`], covering what `dr_rest::api` calls.

use super::{RecordedRequest, RestState};
use axum::Json;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

fn ok(body: Value) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

fn error(status: StatusCode, code: u32, message: &str) -> Response {
    (status, Json(json!({ "code": code, "message": message }))).into_response()
}

fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, 10000, "404: Not Found")
}

fn id(text: &str) -> Option<u64> {
    text.parse().ok()
}

/// Answers one request. What takes a bot token needs an `Authorization` header.
pub(crate) async fn route(state: &RestState, request: &RecordedRequest) -> Response {
    let path = request.path.replace("%40", "@");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let needs_auth =
        matches!(segments.first(), Some(&"users" | &"channels" | &"guilds" | &"applications" | &"gateway"));
    if needs_auth && !request.header("authorization").is_some_and(|a| a.starts_with("Bot ")) {
        return error(StatusCode::UNAUTHORIZED, 0, "401: Unauthorized");
    }
    let world = &state.world;
    let method = &request.method;
    let is = |m: Method| *method == m;
    match segments.as_slice() {
        ["gateway", "bot"] if is(Method::GET) => ok(json!({
            "url": state.gateway_url,
            "shards": 1,
            "session_start_limit": {"total": 1000, "remaining": 999, "reset_after": 0, "max_concurrency": 1},
        })),
        ["users", "@me"] if is(Method::GET) => ok(world.bot_user()),
        ["users", user] if is(Method::GET) => {
            id(user).and_then(|u| world.user(u)).map_or_else(|| unknown("user", 10013), ok)
        }
        ["channels", channel] if is(Method::GET) => {
            id(channel).and_then(|c| world.channel(c)).map_or_else(|| unknown("channel", 10003), ok)
        }
        ["channels", _, "typing"] if is(Method::POST) => no_content(),
        ["channels", channel, "messages"] if is(Method::POST) => match id(channel) {
            Some(c) if world.channel(c).is_some() => ok(world.create_message(c, &request.json())),
            _ => unknown("channel", 10003),
        },
        ["channels", channel, "messages", message] => match (id(channel), id(message)) {
            (Some(c), Some(m)) => match method.as_str() {
                "GET" => world.message(c, m).map_or_else(|| unknown("message", 10008), ok),
                "PATCH" => world.edit_message(c, m, &request.json()).map_or_else(|| unknown("message", 10008), ok),
                "DELETE" => {
                    if world.delete_message(c, m) {
                        no_content()
                    } else {
                        unknown("message", 10008)
                    }
                }
                _ => not_found(),
            },
            _ => not_found(),
        },
        ["channels", _, "messages", _, "reactions", _, "@me"] => no_content(),
        ["guilds", guild] if is(Method::GET) => {
            id(guild).and_then(|g| world.guild(g)).map_or_else(|| unknown("guild", 10004), ok)
        }
        ["guilds", guild, "channels"] if is(Method::GET) => {
            id(guild).map_or_else(not_found, |g| ok(Value::Array(world.guild_channels(g))))
        }
        ["guilds", guild, "members", user] => match (id(guild), id(user), method.as_str()) {
            (Some(g), Some(u), "GET") => world.member(g, u).map_or_else(|| unknown("member", 10007), ok),
            (Some(g), Some(u), "DELETE") => {
                if world.remove_member(g, u) {
                    no_content()
                } else {
                    unknown("member", 10007)
                }
            }
            _ => not_found(),
        },
        ["applications", _, "commands"] => commands(state, request, None),
        ["applications", _, "guilds", guild, "commands"] => commands(state, request, id(guild)),
        ["applications", _, "commands", command] if is(Method::DELETE) => delete_command(state, None, command),
        ["applications", _, "guilds", guild, "commands", command] if is(Method::DELETE) => {
            delete_command(state, id(guild), command)
        }
        ["interactions", _, token, "callback"] if is(Method::POST) => callback(state, token, &request.json()),
        ["webhooks", _, token] if is(Method::POST) => ok(world.create_followup(token, &request.json())),
        ["webhooks", _, token, "messages", message] => reply(state, request, token, message),
        _ => not_found(),
    }
}

fn unknown(what: &str, code: u32) -> Response {
    error(StatusCode::NOT_FOUND, code, &format!("Unknown {what}"))
}

fn commands(state: &RestState, request: &RecordedRequest, guild: Option<u64>) -> Response {
    let world = &state.world;
    match request.method.as_str() {
        "GET" => ok(Value::Array(world.commands(guild))),
        "PUT" => {
            let list = request.json();
            let list = list.as_array().cloned().unwrap_or_default();
            ok(Value::Array(world.set_commands(guild, &list)))
        }
        "POST" => ok(world.add_command(guild, &request.json())),
        _ => not_found(),
    }
}

fn delete_command(state: &RestState, guild: Option<u64>, command: &str) -> Response {
    if state.world.delete_command(guild, command) { no_content() } else { unknown("application command", 10063) }
}

fn callback(state: &RestState, token: &str, body: &Value) -> Response {
    if !state.world.answer_interaction(token, body) {
        return error(StatusCode::BAD_REQUEST, 40060, "Interaction has already been acknowledged.");
    }
    no_content()
}

fn reply(state: &RestState, request: &RecordedRequest, token: &str, message: &str) -> Response {
    let world = &state.world;
    match request.method.as_str() {
        "GET" => world.webhook_message(token, message).map_or_else(|| unknown("webhook message", 10008), ok),
        "PATCH" => world
            .edit_webhook_message(token, message, &request.json())
            .map_or_else(|| unknown("webhook message", 10008), ok),
        "DELETE" => {
            if world.delete_webhook_message(token, message) {
                no_content()
            } else {
                unknown("webhook message", 10008)
            }
        }
        _ => not_found(),
    }
}
