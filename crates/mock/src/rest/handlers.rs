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

/// A value of the query string, parsed.
fn query_value<T: std::str::FromStr>(request: &RecordedRequest, key: &str) -> Option<T> {
    request.query.as_deref()?.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == key { value.parse().ok() } else { None }
    })
}

/// A 204 when `done`, else the given not-found error.
fn done_or(done: bool, what: &str, code: u32) -> Response {
    if done { no_content() } else { unknown(what, code) }
}

fn guild_roles(state: &RestState, request: &RecordedRequest, guild: u64) -> Response {
    match request.method.as_str() {
        "GET" => ok(Value::Array(state.world.roles(guild))),
        "POST" => ok(state.world.create_role(guild, &request.json())),
        _ => not_found(),
    }
}

fn guild_role(state: &RestState, request: &RecordedRequest, guild: u64, role: u64) -> Response {
    match request.method.as_str() {
        "PATCH" => state.world.edit_role(guild, role, &request.json()).map_or_else(|| unknown("role", 10011), ok),
        "DELETE" => done_or(state.world.delete_role(guild, role), "role", 10011),
        _ => not_found(),
    }
}

fn guild_ban(state: &RestState, request: &RecordedRequest, guild: u64, user: u64) -> Response {
    match request.method.as_str() {
        "GET" => state.world.ban(guild, user).map_or_else(|| unknown("ban", 10026), ok),
        "PUT" => {
            let reason = request.header("x-audit-log-reason");
            done_or(state.world.create_ban(guild, user, reason), "user", 10013)
        }
        "DELETE" => done_or(state.world.remove_ban(guild, user), "ban", 10026),
        _ => not_found(),
    }
}

/// `GET` lists, `POST` creates, through `make`, which builds the new item.
fn expressions(
    state: &RestState,
    request: &RecordedRequest,
    guild: u64,
    key: &str,
    make: impl FnOnce(&RecordedRequest, u64) -> Option<Value>,
) -> Response {
    let world = &state.world;
    match request.method.as_str() {
        "GET" => world.expressions(guild, key).map_or_else(|| unknown("guild", 10004), |l| ok(Value::Array(l))),
        "POST" => {
            let id = world.next_id();
            let Some(item) = make(request, id) else {
                return error(StatusCode::BAD_REQUEST, 50035, "Invalid Form Body");
            };
            world
                .edit_expressions(guild, key, |list| {
                    list.push(item.clone());
                    Some(item)
                })
                .map_or_else(|| unknown("guild", 10004), ok)
        }
        _ => not_found(),
    }
}

/// `GET`, `PATCH` and `DELETE` of one emoji or sticker.
fn expression(state: &RestState, request: &RecordedRequest, guild: u64, key: &str, id: &str, code: u32) -> Response {
    let world = &state.world;
    let what = if key == "emojis" { "emoji" } else { "sticker" };
    let matches = |item: &Value| item["id"] == json!(id);
    match request.method.as_str() {
        "GET" => world
            .expressions(guild, key)
            .and_then(|list| list.into_iter().find(|i| matches(i)))
            .map_or_else(|| unknown(what, code), ok),
        "PATCH" => {
            let body = request.json();
            let edited = world.edit_expressions(guild, key, |list| {
                let item = list.iter_mut().find(|i| matches(i))?;
                merge_into(item, &body);
                Some(item.clone())
            });
            edited.map_or_else(|| unknown(what, code), ok)
        }
        "DELETE" => {
            let removed = world.edit_expressions(guild, key, |list| {
                let at = list.iter().position(matches)?;
                Some(list.remove(at))
            });
            done_or(removed.is_some(), what, code)
        }
        _ => not_found(),
    }
}

fn merge_into(target: &mut Value, body: &Value) {
    if let (Some(target), Some(body)) = (target.as_object_mut(), body.as_object()) {
        for (key, value) in body {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn new_emoji(state: &RestState, request: &RecordedRequest, id: u64) -> Option<Value> {
    let body = request.json();
    body["image"].as_str().filter(|i| i.starts_with("data:image/"))?;
    Some(json!({
        "id": id.to_string(),
        "name": body["name"],
        "roles": body.get("roles").cloned().unwrap_or(json!([])),
        "user": state.world.bot_user(),
        "require_colons": true,
        "managed": false,
        "animated": body["image"].as_str().is_some_and(|i| i.starts_with("data:image/gif")),
        "available": true,
    }))
}

fn new_sticker(state: &RestState, request: &RecordedRequest, guild: u64, id: u64) -> Option<Value> {
    let parts = request.parts()?;
    let text =
        |name: &str| parts.iter().find(|p| p.name == name).map(|p| String::from_utf8_lossy(&p.data).into_owned());
    let file = parts.iter().find(|p| p.name == "file")?;
    let format = match file.content_type.as_deref() {
        Some("image/gif") => 4,
        Some("application/json") => 3,
        _ => 1,
    };
    let description = text("description").filter(|d| !d.is_empty());
    Some(json!({
        "id": id.to_string(),
        "name": text("name")?,
        "description": description,
        "tags": text("tags")?,
        "type": 2,
        "format_type": format,
        "available": true,
        "guild_id": guild.to_string(),
        "user": state.world.bot_user(),
    }))
}

/// Answers one request. What takes a bot token needs an `Authorization` header.
pub(crate) async fn route(state: &RestState, request: &RecordedRequest) -> Response {
    let path = request.path.replace("%40", "@");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // A webhook by id alone takes the bot's token. With its own token in the path, it takes none.
    let needs_auth = matches!(
        segments.first(),
        Some(&"users" | &"channels" | &"guilds" | &"applications" | &"gateway" | &"stickers" | &"stage-instances")
    ) || matches!(segments.as_slice(), ["webhooks", _]);
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
        ["channels", channel] => {
            let found = id(channel).and_then(|c| match method.as_str() {
                "GET" => world.channel(c),
                "PATCH" => world.edit_channel(c, &request.json()),
                "DELETE" => world.delete_channel(c),
                _ => None,
            });
            found.map_or_else(|| unknown("channel", 10003), ok)
        }
        ["channels", channel, "permissions", target] => match (id(channel), id(target), method.as_str()) {
            (Some(c), Some(t), "PUT") if world.set_overwrite(c, t, Some(&request.json())) => no_content(),
            (Some(c), Some(t), "DELETE") if world.set_overwrite(c, t, None) => no_content(),
            _ => unknown("channel", 10003),
        },
        ["channels", _, "typing"] if is(Method::POST) => no_content(),
        ["channels", channel, "threads"] if is(Method::POST) => id(channel)
            .and_then(|c| world.start_thread(c, None, &request.json()))
            .map_or_else(|| unknown("channel", 10003), ok),
        ["channels", channel, "messages", message, "threads"] if is(Method::POST) => match (id(channel), id(message)) {
            (Some(c), Some(m)) if world.message(c, m).is_some() => {
                world.start_thread(c, Some(m), &request.json()).map_or_else(|| unknown("channel", 10003), ok)
            }
            _ => unknown("message", 10008),
        },
        ["channels", channel, "threads", "archived", visibility] if is(Method::GET) => {
            let (parent, private) = (channel.to_string(), *visibility == "private");
            ok(world.thread_list(|t| {
                t["parent_id"] == json!(parent)
                    && t["thread_metadata"]["archived"] == json!(true)
                    && (t["type"] == json!(12)) == private
            }))
        }
        ["channels", channel, "users", "@me" | "%40me", "threads", "archived", "private"] if is(Method::GET) => {
            let parent = channel.to_string();
            let bot = world.bot_id().get();
            ok(world.thread_list(|t| {
                let joined = t["id"].as_str().and_then(|s| s.parse().ok()).and_then(|id| world.thread_member(id, bot));
                t["parent_id"] == json!(parent)
                    && t["type"] == json!(12)
                    && t["thread_metadata"]["archived"] == json!(true)
                    && joined.is_some()
            }))
        }
        ["channels", thread, "thread-members"] if is(Method::GET) => {
            id(thread).map_or_else(not_found, |t| ok(Value::Array(world.thread_members(t))))
        }
        ["channels", thread, "thread-members", user] => {
            let user = if matches!(*user, "@me" | "%40me") { Some(world.bot_id().get()) } else { id(user) };
            match (id(thread), user, method.as_str()) {
                (Some(t), Some(u), "PUT") if world.set_thread_member(t, u, true) => no_content(),
                (Some(t), Some(u), "DELETE") if world.set_thread_member(t, u, false) => no_content(),
                (Some(t), Some(u), "GET") => world.thread_member(t, u).map_or_else(|| unknown("member", 10007), ok),
                _ => unknown("channel", 10003),
            }
        }
        ["channels", channel, "messages"] if is(Method::POST) => match id(channel) {
            Some(c) if world.channel(c).is_some() => ok(world.create_message(c, &request.json())),
            _ => unknown("channel", 10003),
        },
        ["channels", channel, "messages"] if is(Method::GET) => {
            let limit = query_value(request, "limit").unwrap_or(50);
            id(channel)
                .map_or_else(not_found, |c| ok(Value::Array(world.messages(c, limit, query_value(request, "before")))))
        }
        ["channels", channel, "messages", "bulk-delete"] if is(Method::POST) => {
            let body = request.json();
            let ids: Vec<u64> = body["messages"]
                .as_array()
                .map_or_else(Vec::new, |list| list.iter().filter_map(|i| i.as_str().and_then(id)).collect());
            id(channel).map_or_else(not_found, |c| {
                world.bulk_delete_messages(c, &ids);
                no_content()
            })
        }
        ["channels", channel, "messages", "pins", message] => match (id(channel), id(message), method.as_str()) {
            (Some(c), Some(m), "PUT") => done_or(world.set_pinned(c, m, true), "message", 10008),
            (Some(c), Some(m), "DELETE") => done_or(world.set_pinned(c, m, false), "message", 10008),
            _ => not_found(),
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
        ["stage-instances"] if is(Method::POST) => match world.start_stage(&request.json()) {
            Ok(stage) => ok(stage),
            Err(10003) => unknown("channel", 10003),
            Err(code) => error(StatusCode::BAD_REQUEST, code, "Cannot start a stage here"),
        },
        ["stage-instances", channel] => match (id(channel), method.as_str()) {
            (Some(c), "GET") => world.stage(c).map_or_else(|| unknown("stage instance", 10067), ok),
            (Some(c), "PATCH") => {
                world.edit_stage(c, &request.json()).map_or_else(|| unknown("stage instance", 10067), ok)
            }
            (Some(c), "DELETE") => done_or(world.end_stage(c), "stage instance", 10067),
            _ => not_found(),
        },
        ["guilds", guild, "audit-logs"] if is(Method::GET) => id(guild).map_or_else(not_found, |g| {
            let limit = query_value(request, "limit").unwrap_or(50);
            let user = query_value(request, "user_id");
            let action = query_value(request, "action_type");
            ok(world.audit_log(g, user, action, query_value(request, "before"), limit))
        }),
        ["guilds", guild, "auto-moderation", "rules"] => match (id(guild), method.as_str()) {
            (Some(g), "GET") => ok(Value::Array(world.automod_rules(g))),
            (Some(g), "POST") => match world.create_automod_rule(g, &request.json()) {
                Some(Ok(rule)) => ok(rule),
                Some(Err(message)) => error(StatusCode::BAD_REQUEST, 30035, message),
                None => unknown("guild", 10004),
            },
            _ => not_found(),
        },
        ["guilds", guild, "auto-moderation", "rules", rule] => match (id(guild), id(rule), method.as_str()) {
            (Some(g), Some(r), "GET") => {
                world.automod_rule(g, r).map_or_else(|| unknown("auto moderation rule", 10066), ok)
            }
            (Some(g), Some(r), "PATCH") => world
                .edit_automod_rule(g, r, &request.json())
                .map_or_else(|| unknown("auto moderation rule", 10066), ok),
            (Some(g), Some(r), "DELETE") => done_or(world.delete_automod_rule(g, r), "auto moderation rule", 10066),
            _ => not_found(),
        },
        ["guilds", guild, "scheduled-events"] => match (id(guild), method.as_str()) {
            (Some(g), "GET") => {
                ok(Value::Array(world.scheduled_events(g, query_value(request, "with_user_count") == Some(true))))
            }
            (Some(g), "POST") => {
                world.create_scheduled_event(g, &request.json()).map_or_else(|| unknown("guild", 10004), ok)
            }
            _ => not_found(),
        },
        ["guilds", guild, "scheduled-events", event] => match (id(guild), id(event), method.as_str()) {
            (Some(g), Some(e), "GET") => {
                let with_count = query_value(request, "with_user_count") == Some(true);
                world.scheduled_event(g, e, with_count).map_or_else(|| unknown("guild scheduled event", 10070), ok)
            }
            (Some(g), Some(e), "PATCH") => match world.edit_scheduled_event(g, e, &request.json()) {
                Some(Ok(event)) => ok(event),
                Some(Err(message)) => error(StatusCode::BAD_REQUEST, 180002, message),
                None => unknown("guild scheduled event", 10070),
            },
            (Some(g), Some(e), "DELETE") => done_or(world.delete_scheduled_event(g, e), "guild scheduled event", 10070),
            _ => not_found(),
        },
        ["guilds", guild, "scheduled-events", event, "users"] if is(Method::GET) => match (id(guild), id(event)) {
            (Some(g), Some(e)) => {
                let after = query_value(request, "after").unwrap_or(0);
                let limit = query_value(request, "limit").unwrap_or(100);
                let with_member = query_value(request, "with_member") == Some(true);
                ok(Value::Array(world.event_users(g, e, after, limit, with_member)))
            }
            _ => not_found(),
        },
        ["guilds", guild, "emojis"] => match id(guild) {
            Some(g) => expressions(state, request, g, "emojis", |r, i| new_emoji(state, r, i)),
            None => not_found(),
        },
        ["guilds", guild, "emojis", emoji] => match id(guild) {
            Some(g) => expression(state, request, g, "emojis", emoji, 10014),
            None => not_found(),
        },
        ["guilds", guild, "stickers"] => match id(guild) {
            Some(g) => expressions(state, request, g, "stickers", |r, i| new_sticker(state, r, g, i)),
            None => not_found(),
        },
        ["guilds", guild, "stickers", sticker] => match id(guild) {
            Some(g) => expression(state, request, g, "stickers", sticker, 10060),
            None => not_found(),
        },
        ["stickers", sticker] if is(Method::GET) => {
            let found = world
                .guild_ids()
                .into_iter()
                .find_map(|g| world.expressions(g.get(), "stickers")?.into_iter().find(|s| s["id"] == json!(sticker)));
            found.map_or_else(|| unknown("sticker", 10060), ok)
        }
        ["sticker-packs"] if is(Method::GET) => ok(json!({"sticker_packs": []})),
        ["channels", channel, "polls", message, "answers", answer] if is(Method::GET) => {
            let after = query_value(request, "after").unwrap_or(0);
            let limit = query_value(request, "limit").unwrap_or(25);
            let found = match (id(channel), id(message), answer.parse()) {
                (Some(c), Some(m), Ok(a)) => world.poll_voters(c, m, a, after, limit),
                _ => None,
            };
            found.map_or_else(|| unknown("message", 10008), ok)
        }
        ["channels", channel, "polls", message, "expire"] if is(Method::POST) => match (id(channel), id(message)) {
            (Some(c), Some(m)) => world.expire_poll(c, m).map_or_else(|| unknown("message", 10008), ok),
            _ => not_found(),
        },
        ["channels", _, "messages", _, "reactions", _, "@me"] => no_content(),
        ["channels", _, "messages", _, "reactions", _, _] if is(Method::DELETE) => no_content(),
        ["channels", _, "messages", _, "reactions"] if is(Method::DELETE) => no_content(),
        ["guilds", guild] if is(Method::GET) => {
            id(guild).and_then(|g| world.guild(g)).map_or_else(|| unknown("guild", 10004), ok)
        }
        ["guilds", guild, "webhooks"] if is(Method::GET) => {
            let guild = json!(guild.to_string());
            ok(Value::Array(world.webhooks(|w| w["guild_id"] == guild)))
        }
        ["channels", channel, "webhooks"] => match (id(channel), method.as_str()) {
            (Some(c), "POST") => world.create_webhook(c, &request.json()).map_or_else(|| unknown("channel", 10003), ok),
            (Some(c), "GET") => {
                let channel = json!(c.to_string());
                ok(Value::Array(world.webhooks(|w| w["channel_id"] == channel)))
            }
            _ => not_found(),
        },
        ["guilds", guild, "threads", "active"] if is(Method::GET) => {
            let guild = guild.to_string();
            ok(world.thread_list(|t| t["guild_id"] == json!(guild) && t["thread_metadata"]["archived"] != json!(true)))
        }
        ["guilds", guild, "channels"] if is(Method::POST) => match id(guild) {
            Some(g) => ok(world.create_channel(g, &request.json())),
            None => not_found(),
        },
        ["guilds", _, "channels"] if is(Method::PATCH) => {
            for entry in request.json().as_array().cloned().unwrap_or_default() {
                if let Some(c) = entry["id"].as_str().and_then(|s| s.parse().ok()) {
                    let mut change = entry.clone();
                    change.as_object_mut().map(|o| o.remove("id"));
                    world.edit_channel(c, &change);
                }
            }
            no_content()
        }
        ["guilds", guild, "channels"] if is(Method::GET) => {
            id(guild).map_or_else(not_found, |g| ok(Value::Array(world.guild_channels(g))))
        }
        ["guilds", guild, "members"] if is(Method::GET) => {
            let (limit, after) =
                (query_value(request, "limit").unwrap_or(1), query_value(request, "after").unwrap_or(0));
            id(guild).map_or_else(not_found, |g| ok(Value::Array(world.members(g, limit, after))))
        }
        ["guilds", guild, "members", user, "roles", role] => match (id(guild), id(user), id(role), method.as_str()) {
            (Some(g), Some(u), Some(r), "PUT") => done_or(world.add_member_role(g, u, r), "role", 10011),
            (Some(g), Some(u), Some(r), "DELETE") => done_or(world.remove_member_role(g, u, r), "member", 10007),
            _ => not_found(),
        },
        ["guilds", guild, "bans"] if is(Method::GET) => {
            let (limit, after) =
                (query_value(request, "limit").unwrap_or(1000), query_value(request, "after").unwrap_or(0));
            id(guild).map_or_else(not_found, |g| ok(Value::Array(world.bans(g, limit, after))))
        }
        ["guilds", guild, "bans", user] => match (id(guild), id(user)) {
            (Some(g), Some(u)) => guild_ban(state, request, g, u),
            _ => not_found(),
        },
        ["guilds", guild, "roles"] => id(guild).map_or_else(not_found, |g| guild_roles(state, request, g)),
        ["guilds", guild, "roles", role] => match (id(guild), id(role)) {
            (Some(g), Some(r)) => guild_role(state, request, g, r),
            _ => not_found(),
        },
        ["guilds", guild, "members", user] => match (id(guild), id(user), method.as_str()) {
            (Some(g), Some(u), "PATCH") => {
                world.edit_member(g, u, &request.json()).map_or_else(|| unknown("member", 10007), ok)
            }
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
        ["webhooks", hook] => match (id(hook), method.as_str()) {
            (Some(h), "GET") => world.webhook(h).map_or_else(|| unknown("webhook", 10015), ok),
            (Some(h), "PATCH") => world.edit_webhook(h, &request.json()).map_or_else(|| unknown("webhook", 10015), ok),
            (Some(h), "DELETE") if world.delete_webhook(h) => no_content(),
            _ => unknown("webhook", 10015),
        },
        ["webhooks", hook, token, rest @ ..] if id(hook).is_some_and(|h| world.webhook(h).is_some()) => {
            webhook_with_token(state, request, id(hook).unwrap_or_default(), token, rest)
        }
        ["webhooks", _, token] if is(Method::POST) => ok(world.create_followup(token, &request.json())),
        ["webhooks", _, token, "messages", message] => reply(state, request, token, message),
        _ => not_found(),
    }
}

/// `/webhooks/{id}/{token}/...`, for a webhook the world knows. A wrong token is a 401, as on Discord.
fn webhook_with_token(state: &RestState, request: &RecordedRequest, hook: u64, token: &str, rest: &[&str]) -> Response {
    let world = &state.world;
    let Some(webhook) = world.webhook(hook) else { return unknown("webhook", 10015) };
    if webhook["token"].as_str() != Some(token) {
        return error(StatusCode::UNAUTHORIZED, 50027, "Invalid Webhook Token");
    }
    let method = request.method.as_str();
    match (rest, method) {
        ([], "GET") => {
            let mut seen = webhook;
            seen.as_object_mut().map(|o| o.remove("user"));
            ok(seen)
        }
        ([], "PATCH") => world.edit_webhook(hook, &request.json()).map_or_else(|| unknown("webhook", 10015), ok),
        ([], "DELETE") if world.delete_webhook(hook) => no_content(),
        ([], "POST") => {
            let thread = query_value(request, "thread_id");
            world.execute_webhook(hook, thread, &request.json()).map_or_else(|| unknown("channel", 10003), ok)
        }
        (["messages", message], _) => {
            let Some(message) = id(message) else { return unknown("message", 10008) };
            let Some(channel) = world.message_channel(message) else { return unknown("message", 10008) };
            match method {
                "GET" => world.message(channel, message).map_or_else(|| unknown("message", 10008), ok),
                "PATCH" => {
                    world.edit_message(channel, message, &request.json()).map_or_else(|| unknown("message", 10008), ok)
                }
                "DELETE" if world.delete_message(channel, message) => no_content(),
                _ => unknown("message", 10008),
            }
        }
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
