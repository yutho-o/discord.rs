//! The in-memory Discord state. `World` never depends on the gateway: writes publish a
//! [`WorldEvent`], and the gateway half subscribes.

use dr_model::{ApplicationId, ChannelId, GuildId, UserId};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

/// A gateway dispatch the world wants delivered to connected clients.
#[derive(Clone, Debug)]
pub struct WorldEvent {
    /// The event name, such as `MESSAGE_CREATE`.
    pub t: String,
    /// The event body.
    pub d: Value,
}

#[derive(Debug, Default)]
struct State {
    guilds: BTreeMap<u64, Value>,
    channels: BTreeMap<u64, Value>,
    users: BTreeMap<u64, Value>,
    members: BTreeMap<(u64, u64), Value>,
    bans: BTreeMap<(u64, u64), Value>,
    roles: BTreeMap<(u64, u64), Value>,
    messages: BTreeMap<(u64, u64), Value>,
    commands: BTreeMap<Option<u64>, Vec<Value>>,
    /// Webhooks by id, with their tokens.
    webhooks: BTreeMap<u64, Value>,
    /// Who is in which thread: `(thread, user)`.
    thread_members: BTreeSet<(u64, u64)>,
    /// What each interaction token answered, and its messages by id. `@original` is the first answer.
    answered: HashMap<String, BTreeMap<String, Value>>,
}

/// True for the three thread types.
fn is_thread(channel: &Value) -> bool {
    matches!(channel["type"].as_u64(), Some(10..=12))
}

/// Copies each field of `body` over `target`.
fn merge(target: &mut Value, body: &Value) {
    if let (Some(target), Some(body)) = (target.as_object_mut(), body.as_object()) {
        for (key, value) in body {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// The state both halves of the mock read. Payloads are JSON, in Discord's wire form.
#[derive(Debug)]
pub struct World {
    state: Mutex<State>,
    events: broadcast::Sender<WorldEvent>,
    next: AtomicU64,
    application_id: ApplicationId,
    bot_id: UserId,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// An empty world with one bot user.
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(1024);
        let world = Self {
            state: Mutex::new(State::default()),
            events,
            next: AtomicU64::new(1 << 40),
            application_id: ApplicationId::new(1 << 39),
            bot_id: UserId::new(1 << 39),
        };
        let bot = json!({
            "id": world.bot_id.to_string(),
            "username": "mockbot",
            "discriminator": "0",
            "global_name": null,
            "avatar": null,
            "bot": true,
        });
        world.state.lock().users.insert(world.bot_id.get(), bot);
        world
    }

    /// The application id, which `READY` reports.
    pub fn application_id(&self) -> ApplicationId {
        self.application_id
    }

    /// The bot's user id.
    pub fn bot_id(&self) -> UserId {
        self.bot_id
    }

    /// The bot's user, in wire form.
    pub fn bot_user(&self) -> Value {
        self.state.lock().users.get(&self.bot_id.get()).cloned().unwrap_or(Value::Null)
    }

    /// A new snowflake. Each is larger than the last.
    pub fn next_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// Publishes a dispatch to connected clients. With no client connected it goes nowhere.
    pub fn emit(&self, t: impl Into<String>, d: Value) {
        // An error here only means nobody is subscribed.
        let _ = self.events.send(WorldEvent { t: t.into(), d });
    }

    /// Receives every later [`World::emit`].
    pub fn subscribe(&self) -> broadcast::Receiver<WorldEvent> {
        self.events.subscribe()
    }

    /// Adds a guild owned by the bot, with no channels. Emits nothing: a guild reaches a client
    /// through `READY` and `GUILD_CREATE` when it connects.
    pub fn add_guild(&self, name: &str) -> GuildId {
        self.add_guild_with_id(self.next_id(), name)
    }

    /// [`World::add_guild`] with a chosen id, to place a guild on a shard: `(id >> 22) % shard_count`.
    pub fn add_guild_with_id(&self, id: u64, name: &str) -> GuildId {
        let guild = json!({
            "id": id.to_string(),
            "name": name,
            "icon": null,
            "owner_id": self.bot_id.to_string(),
            "system_channel_id": null,
            "features": [],
            "roles": [],
            "emojis": [],
        });
        self.state.lock().guilds.insert(id, guild);
        GuildId::new(id)
    }

    /// Adds a text channel to a guild. Emits nothing.
    pub fn add_channel(&self, guild: GuildId, name: &str) -> ChannelId {
        let id = self.next_id();
        let channel = json!({
            "id": id.to_string(),
            "type": 0,
            "guild_id": guild.to_string(),
            "name": name,
        });
        self.state.lock().channels.insert(id, channel);
        ChannelId::new(id)
    }

    /// Creates a channel, as `POST /guilds/{id}/channels` does, and emits `CHANNEL_CREATE`.
    pub fn create_channel(&self, guild: u64, body: &Value) -> Value {
        let id = self.next_id();
        let mut channel = json!({"id": id.to_string(), "type": 0, "guild_id": guild.to_string(), "position": 0});
        merge(&mut channel, body);
        self.state.lock().channels.insert(id, channel.clone());
        self.emit("CHANNEL_CREATE", channel.clone());
        channel
    }

    /// Merges an edit into a channel and emits `CHANNEL_UPDATE`, or `THREAD_UPDATE` for a thread,
    /// whose `archived`, `locked`, `invitable` and `auto_archive_duration` go to its metadata.
    /// `None` when there is no such channel.
    pub fn edit_channel(&self, id: u64, body: &Value) -> Option<Value> {
        let channel = {
            let mut state = self.state.lock();
            let channel = state.channels.get_mut(&id)?;
            let mut body = body.clone();
            if is_thread(channel) {
                for key in ["archived", "locked", "invitable", "auto_archive_duration"] {
                    if let Some(value) = body.as_object_mut().and_then(|o| o.remove(key)) {
                        channel["thread_metadata"][key] = value;
                    }
                }
            }
            merge(channel, &body);
            channel.clone()
        };
        self.emit(if is_thread(&channel) { "THREAD_UPDATE" } else { "CHANNEL_UPDATE" }, channel.clone());
        Some(channel)
    }

    /// Deletes a channel and emits `CHANNEL_DELETE`, or `THREAD_DELETE` for a thread. Returns it as
    /// it was.
    pub fn delete_channel(&self, id: u64) -> Option<Value> {
        let channel = self.state.lock().channels.remove(&id)?;
        if is_thread(&channel) {
            let partial = json!({"id": channel["id"], "type": channel["type"], "guild_id": channel["guild_id"],
                                 "parent_id": channel["parent_id"]});
            self.emit("THREAD_DELETE", partial);
        } else {
            self.emit("CHANNEL_DELETE", channel.clone());
        }
        Some(channel)
    }

    /// Starts a thread in `parent`, from `message` if given, as `POST .../threads` does. A forum post's
    /// `message` becomes the first message of the thread. The bot joins it. Emits `THREAD_CREATE`.
    /// `None` when there is no such channel.
    pub fn start_thread(&self, parent: u64, message: Option<u64>, body: &Value) -> Option<Value> {
        let parent_channel = self.channel(parent)?;
        let id = message.unwrap_or_else(|| self.next_id());
        let kind = match (parent_channel["type"].as_u64(), body["type"].as_u64()) {
            (Some(5), _) => 10,
            (_, Some(12)) if message.is_none() => 12,
            _ => 11,
        };
        let mut thread = json!({
            "id": id.to_string(),
            "type": kind,
            "guild_id": parent_channel["guild_id"],
            "parent_id": parent.to_string(),
            "name": body["name"],
            "owner_id": self.bot_id.to_string(),
            "member_count": 1,
            "message_count": 0,
            "thread_metadata": {
                "archived": false,
                "locked": false,
                "auto_archive_duration": body.get("auto_archive_duration").cloned().unwrap_or(json!(4320)),
                "archive_timestamp": "2020-01-01T00:00:00.000000+00:00",
            },
        });
        if let Some(tags) = body.get("applied_tags") {
            thread["applied_tags"] = tags.clone();
        }
        {
            let mut state = self.state.lock();
            state.channels.insert(id, thread.clone());
            state.thread_members.insert((id, self.bot_id.get()));
        }
        self.emit("THREAD_CREATE", thread.clone());
        if let Some(first) = body.get("message") {
            thread["message"] = self.create_message(id, first);
        }
        Some(thread)
    }

    /// Creates an incoming webhook in a channel, with the token `token-{id}`, and emits
    /// `WEBHOOKS_UPDATE`. `None` when there is no such channel.
    pub fn create_webhook(&self, channel: u64, body: &Value) -> Option<Value> {
        let guild = self.channel(channel)?["guild_id"].clone();
        let id = self.next_id();
        let webhook = json!({
            "id": id.to_string(),
            "type": 1,
            "guild_id": guild,
            "channel_id": channel.to_string(),
            "name": body["name"],
            "avatar": if body["avatar"].is_null() { Value::Null } else { json!("hash") },
            "token": format!("token-{id}"),
            "user": self.bot_user(),
            "application_id": null,
        });
        self.state.lock().webhooks.insert(id, webhook.clone());
        self.emit("WEBHOOKS_UPDATE", json!({"guild_id": guild, "channel_id": channel.to_string()}));
        Some(webhook)
    }

    /// A webhook, as the bot's token sees it.
    pub fn webhook(&self, id: u64) -> Option<Value> {
        self.state.lock().webhooks.get(&id).cloned()
    }

    /// The webhooks that `keep` accepts.
    pub fn webhooks(&self, keep: impl Fn(&Value) -> bool) -> Vec<Value> {
        self.state.lock().webhooks.values().filter(|w| keep(w)).cloned().collect()
    }

    /// Edits a webhook and emits `WEBHOOKS_UPDATE`.
    pub fn edit_webhook(&self, id: u64, body: &Value) -> Option<Value> {
        let webhook = {
            let mut state = self.state.lock();
            let webhook = state.webhooks.get_mut(&id)?;
            if let Some(name) = body.get("name") {
                webhook["name"] = name.clone();
            }
            if let Some(avatar) = body.get("avatar") {
                webhook["avatar"] = if avatar.is_null() { Value::Null } else { json!("hash") };
            }
            if let Some(channel) = body.get("channel_id") {
                webhook["channel_id"] = channel.clone();
            }
            webhook.clone()
        };
        self.emit("WEBHOOKS_UPDATE", json!({"guild_id": webhook["guild_id"], "channel_id": webhook["channel_id"]}));
        Some(webhook)
    }

    /// Deletes a webhook and emits `WEBHOOKS_UPDATE`. False when there is none.
    pub fn delete_webhook(&self, id: u64) -> bool {
        let Some(webhook) = self.state.lock().webhooks.remove(&id) else { return false };
        self.emit("WEBHOOKS_UPDATE", json!({"guild_id": webhook["guild_id"], "channel_id": webhook["channel_id"]}));
        true
    }

    /// Posts through a webhook, in `thread` if given, else in its channel. The author is the webhook,
    /// under `username` if the body sets one.
    pub fn execute_webhook(&self, id: u64, thread: Option<u64>, body: &Value) -> Option<Value> {
        let webhook = self.webhook(id)?;
        let channel = match thread {
            Some(t) => t,
            None => webhook["channel_id"].as_str()?.parse().ok()?,
        };
        let mut message = self.create_message(channel, body);
        let name = body.get("username").cloned().unwrap_or_else(|| webhook["name"].clone());
        message["author"] = json!({"id": id.to_string(), "username": name, "bot": true, "discriminator": "0000"});
        message["webhook_id"] = json!(id.to_string());
        let message_id = message["id"].as_str()?.parse().ok()?;
        self.state.lock().messages.insert((channel, message_id), message.clone());
        Some(message)
    }

    /// The channel of a stored message, found by its id alone.
    pub fn message_channel(&self, id: u64) -> Option<u64> {
        self.state.lock().messages.keys().find(|(_, m)| *m == id).map(|(c, _)| *c)
    }

    /// Adds or removes a member of a thread. False when there is no such thread.
    pub fn set_thread_member(&self, thread: u64, user: u64, present: bool) -> bool {
        let mut state = self.state.lock();
        if !state.channels.get(&thread).is_some_and(is_thread) {
            return false;
        }
        if present {
            state.thread_members.insert((thread, user));
        } else {
            state.thread_members.remove(&(thread, user));
        }
        true
    }

    /// A member of a thread, in wire form. `None` when the user is not in it.
    pub fn thread_member(&self, thread: u64, user: u64) -> Option<Value> {
        let state = self.state.lock();
        state.thread_members.contains(&(thread, user)).then(|| {
            json!({"id": thread.to_string(), "user_id": user.to_string(),
                   "join_timestamp": "2020-01-01T00:00:00.000000+00:00", "flags": 0})
        })
    }

    /// The members of a thread, in wire form.
    pub fn thread_members(&self, thread: u64) -> Vec<Value> {
        let users: Vec<u64> =
            self.state.lock().thread_members.range((thread, 0)..=(thread, u64::MAX)).map(|(_, u)| *u).collect();
        users.into_iter().filter_map(|u| self.thread_member(thread, u)).collect()
    }

    /// Threads as a thread listing answers: those `keep` accepts, with the bot's memberships. `keep`
    /// runs with no lock held, so it may read the world.
    pub fn thread_list(&self, keep: impl Fn(&Value) -> bool) -> Value {
        let all: Vec<Value> = self.state.lock().channels.values().filter(|c| is_thread(c)).cloned().collect();
        let threads: Vec<Value> = all.into_iter().filter(|c| keep(c)).collect();
        let members: Vec<Value> = threads
            .iter()
            .filter_map(|t| t["id"].as_str().and_then(|s| s.parse().ok()))
            .filter_map(|t| self.thread_member(t, self.bot_id.get()))
            .collect();
        json!({"threads": threads, "members": members, "has_more": false})
    }

    /// Sets one permission overwrite, as `PUT /channels/{id}/permissions/{target}` does, or removes it
    /// with `body` `None`. Emits `CHANNEL_UPDATE`. False when there is no such channel.
    pub fn set_overwrite(&self, channel: u64, target: u64, body: Option<&Value>) -> bool {
        let updated = {
            let mut state = self.state.lock();
            let Some(stored) = state.channels.get_mut(&channel) else { return false };
            let mut list: Vec<Value> = stored["permission_overwrites"].as_array().cloned().unwrap_or_default();
            list.retain(|o| o["id"] != json!(target.to_string()));
            if let Some(body) = body {
                let mut overwrite = json!({"id": target.to_string()});
                merge(&mut overwrite, body);
                list.push(overwrite);
            }
            stored["permission_overwrites"] = Value::Array(list);
            stored.clone()
        };
        self.emit("CHANNEL_UPDATE", updated);
        true
    }

    /// Adds a user. Emits nothing.
    pub fn add_user(&self, username: &str) -> UserId {
        let id = self.next_id();
        let user = json!({
            "id": id.to_string(),
            "username": username,
            "discriminator": "0",
            "global_name": null,
            "avatar": null,
        });
        self.state.lock().users.insert(id, user);
        UserId::new(id)
    }

    /// Makes a user a member of a guild. Emits nothing. An unknown user does nothing.
    pub fn add_member(&self, guild: GuildId, user: UserId) {
        let mut state = self.state.lock();
        let Some(user_body) = state.users.get(&user.get()).cloned() else { return };
        let member = json!({
            "user": user_body,
            "roles": [],
            "joined_at": "2020-01-01T00:00:00.000000+00:00",
            "deaf": false,
            "mute": false,
        });
        state.members.insert((guild.get(), user.get()), member);
    }

    /// The ids of every guild.
    pub fn guild_ids(&self) -> Vec<GuildId> {
        self.state.lock().guilds.keys().map(|id| GuildId::new(*id)).collect()
    }

    /// A guild as `GUILD_CREATE` sends it: with its channels and members.
    pub fn guild_payload(&self, id: GuildId) -> Option<Value> {
        let state = self.state.lock();
        let mut guild = state.guilds.get(&id.get())?.clone();
        let wanted = id.to_string();
        let channels: Vec<Value> = state
            .channels
            .values()
            .filter(|c| c.get("guild_id").and_then(Value::as_str) == Some(wanted.as_str()))
            .cloned()
            .collect();
        let members: Vec<Value> =
            state.members.range((id.get(), 0)..=(id.get(), u64::MAX)).map(|(_, m)| m.clone()).collect();
        guild["member_count"] = json!(members.len());
        guild["channels"] = Value::Array(channels);
        guild["members"] = Value::Array(members);
        guild["joined_at"] = json!("2020-01-01T00:00:00.000000+00:00");
        guild["unavailable"] = json!(false);
        Some(guild)
    }

    /// A user, in wire form.
    pub fn user(&self, id: u64) -> Option<Value> {
        self.state.lock().users.get(&id).cloned()
    }

    /// A channel, in wire form.
    pub fn channel(&self, id: u64) -> Option<Value> {
        self.state.lock().channels.get(&id).cloned()
    }

    /// A guild as `GET /guilds/{id}` sends it, with no channels and no members.
    pub fn guild(&self, id: u64) -> Option<Value> {
        self.state.lock().guilds.get(&id).cloned()
    }

    /// The channels of a guild, in wire form.
    pub fn guild_channels(&self, id: u64) -> Vec<Value> {
        let wanted = id.to_string();
        let state = self.state.lock();
        state.channels.values().filter(|c| c["guild_id"].as_str() == Some(wanted.as_str())).cloned().collect()
    }

    /// A member, in wire form, as REST sends it: no `guild_id`.
    pub fn member(&self, guild: u64, user: u64) -> Option<Value> {
        self.state.lock().members.get(&(guild, user)).cloned()
    }

    /// Removes a member. False when there is none.
    pub fn remove_member(&self, guild: u64, user: u64) -> bool {
        self.state.lock().members.remove(&(guild, user)).is_some()
    }

    /// Up to `limit` members of a guild, by user id, after `after`.
    pub fn members(&self, guild: u64, limit: usize, after: u64) -> Vec<Value> {
        let state = self.state.lock();
        state
            .members
            .range((guild, after.saturating_add(1))..=(guild, u64::MAX))
            .take(limit)
            .map(|(_, member)| member.clone())
            .collect()
    }

    /// Merges an edit into a member. `None` when there is no such member.
    pub fn edit_member(&self, guild: u64, user: u64, body: &Value) -> Option<Value> {
        let mut state = self.state.lock();
        let member = state.members.get_mut(&(guild, user))?;
        merge(member, body);
        Some(member.clone())
    }

    /// Gives a member a role. False when the member or the role does not exist.
    pub fn add_member_role(&self, guild: u64, user: u64, role: u64) -> bool {
        let mut state = self.state.lock();
        if !state.roles.contains_key(&(guild, role)) {
            return false;
        }
        let Some(roles) = state.members.get_mut(&(guild, user)).and_then(|m| m["roles"].as_array_mut()) else {
            return false;
        };
        let role = json!(role.to_string());
        if !roles.contains(&role) {
            roles.push(role);
        }
        true
    }

    /// Takes a role from a member. False when there is no such member.
    pub fn remove_member_role(&self, guild: u64, user: u64, role: u64) -> bool {
        let mut state = self.state.lock();
        let Some(roles) = state.members.get_mut(&(guild, user)).and_then(|m| m["roles"].as_array_mut()) else {
            return false;
        };
        let role = json!(role.to_string());
        roles.retain(|r| *r != role);
        true
    }

    /// Bans a user, which also removes their member. False when there is no such user.
    pub fn create_ban(&self, guild: u64, user: u64, reason: Option<&str>) -> bool {
        let mut state = self.state.lock();
        let Some(payload) = state.users.get(&user).cloned() else { return false };
        state.members.remove(&(guild, user));
        state.bans.insert((guild, user), json!({"reason": reason, "user": payload}));
        true
    }

    /// Lifts a ban. False when there is none.
    pub fn remove_ban(&self, guild: u64, user: u64) -> bool {
        self.state.lock().bans.remove(&(guild, user)).is_some()
    }

    /// One ban.
    pub fn ban(&self, guild: u64, user: u64) -> Option<Value> {
        self.state.lock().bans.get(&(guild, user)).cloned()
    }

    /// Up to `limit` bans of a guild, by user id, after `after`.
    pub fn bans(&self, guild: u64, limit: usize, after: u64) -> Vec<Value> {
        let state = self.state.lock();
        state
            .bans
            .range((guild, after.saturating_add(1))..=(guild, u64::MAX))
            .take(limit)
            .map(|(_, b)| b.clone())
            .collect()
    }

    /// The roles of a guild.
    pub fn roles(&self, guild: u64) -> Vec<Value> {
        let state = self.state.lock();
        state.roles.range((guild, 0)..=(guild, u64::MAX)).map(|(_, role)| role.clone()).collect()
    }

    /// Creates a role from a request body.
    pub fn create_role(&self, guild: u64, body: &Value) -> Value {
        let id = self.next_id();
        let mut role = json!({
            "id": id.to_string(),
            "name": "new role",
            "color": 0,
            "hoist": false,
            "position": 1,
            "permissions": "0",
            "managed": false,
            "mentionable": false,
        });
        merge(&mut role, body);
        self.state.lock().roles.insert((guild, id), role.clone());
        role
    }

    /// Merges an edit into a role. `None` when there is no such role.
    pub fn edit_role(&self, guild: u64, id: u64, body: &Value) -> Option<Value> {
        let mut state = self.state.lock();
        let role = state.roles.get_mut(&(guild, id))?;
        merge(role, body);
        Some(role.clone())
    }

    /// Deletes a role. False when there is none.
    pub fn delete_role(&self, guild: u64, id: u64) -> bool {
        self.state.lock().roles.remove(&(guild, id)).is_some()
    }

    /// Up to `limit` messages of a channel, newest first, before `before` when given.
    pub fn messages(&self, channel: u64, limit: usize, before: Option<u64>) -> Vec<Value> {
        let state = self.state.lock();
        state
            .messages
            .range((channel, 0)..(channel, before.unwrap_or(u64::MAX)))
            .rev()
            .take(limit)
            .map(|(_, message)| message.clone())
            .collect()
    }

    /// Deletes these messages of a channel. Unknown ones are ignored.
    pub fn bulk_delete_messages(&self, channel: u64, ids: &[u64]) {
        let mut state = self.state.lock();
        for id in ids {
            state.messages.remove(&(channel, *id));
        }
    }

    /// Sets whether a message is pinned. False when there is no such message.
    pub fn set_pinned(&self, channel: u64, id: u64, pinned: bool) -> bool {
        let mut state = self.state.lock();
        let Some(message) = state.messages.get_mut(&(channel, id)) else { return false };
        message["pinned"] = json!(pinned);
        true
    }

    /// The attachments a message has after a body that lists `refs`: a numeric id is a file of the
    /// request, a string id one of `existing` that it keeps.
    fn resolve_attachments(&self, channel: &Value, refs: &[Value], existing: &[Value]) -> Value {
        let resolved = refs.iter().filter_map(|r| match r.get("id") {
            Some(Value::String(id)) => existing.iter().find(|e| e.get("id") == Some(&json!(id))).cloned(),
            Some(Value::Number(_)) => {
                let id = self.next_id();
                let name = r.get("filename").and_then(Value::as_str).unwrap_or("file");
                let url =
                    format!("https://cdn.discordapp.com/attachments/{}/{id}/{name}", channel.as_str().unwrap_or(""));
                Some(json!({
                    "id": id.to_string(),
                    "filename": name,
                    "description": r.get("description").cloned().unwrap_or(Value::Null),
                    "content_type": r.get("content_type").cloned().unwrap_or(Value::Null),
                    "size": r.get("size").cloned().unwrap_or(json!(0)),
                    "url": url,
                    "proxy_url": url,
                }))
            }
            _ => None,
        });
        Value::Array(resolved.collect())
    }

    fn message_body(&self, channel: Value, body: &Value) -> Value {
        let refs = body.get("attachments").and_then(Value::as_array).cloned().unwrap_or_default();
        let attachments = self.resolve_attachments(&channel, &refs, &[]);
        json!({
            "id": self.next_id().to_string(),
            "channel_id": channel,
            "author": self.bot_user(),
            "content": body.get("content").cloned().unwrap_or(json!("")),
            "embeds": body.get("embeds").cloned().unwrap_or(json!([])),
            "components": body.get("components").cloned().unwrap_or(json!([])),
            "flags": body.get("flags").cloned().unwrap_or(json!(0)),
            "nonce": body.get("nonce").cloned().unwrap_or(Value::Null),
            "timestamp": "2020-01-01T00:00:00.000000+00:00",
            "edited_timestamp": null,
            "type": 0,
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": attachments,
            "pinned": false,
        })
    }

    /// A message from the bot, as `POST /channels/{id}/messages` does: stored, and `MESSAGE_CREATE`
    /// emitted. The same nonce twice gives the same message only with `enforce_nonce`, as on Discord.
    pub fn create_message(&self, channel: u64, body: &Value) -> Value {
        if body.get("enforce_nonce").and_then(Value::as_bool) == Some(true) {
            let nonce = body.get("nonce");
            let state = self.state.lock();
            let mut stored = state.messages.range((channel, 0)..=(channel, u64::MAX));
            if let Some((_, existing)) = stored.find(|(_, m)| m.get("nonce") == nonce) {
                return existing.clone();
            }
        }
        let mut message = self.message_body(json!(channel.to_string()), body);
        let id = message["id"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
        if let Some(guild) = self.channel(channel).and_then(|c| c.get("guild_id").cloned()) {
            message["guild_id"] = guild;
        }
        self.state.lock().messages.insert((channel, id), message.clone());
        self.emit("MESSAGE_CREATE", message.clone());
        message
    }

    /// A stored message.
    pub fn message(&self, channel: u64, id: u64) -> Option<Value> {
        self.state.lock().messages.get(&(channel, id)).cloned()
    }

    /// Applies an edit, as `PATCH` does, and emits `MESSAGE_UPDATE`.
    pub fn edit_message(&self, channel: u64, id: u64, body: &Value) -> Option<Value> {
        let (edited, before) = {
            let mut state = self.state.lock();
            let message = state.messages.get_mut(&(channel, id))?;
            let before = message.get("attachments").and_then(Value::as_array).cloned().unwrap_or_default();
            overlay(message, body);
            (message.clone(), before)
        };
        let edited = match body.get("attachments").and_then(Value::as_array) {
            Some(refs) => {
                let attachments = self.resolve_attachments(&json!(channel.to_string()), refs, &before);
                let mut state = self.state.lock();
                let message = state.messages.get_mut(&(channel, id))?;
                message["attachments"] = attachments;
                message.clone()
            }
            None => edited,
        };
        self.emit("MESSAGE_UPDATE", edited.clone());
        Some(edited)
    }

    /// Deletes a message and emits `MESSAGE_DELETE`. False when there is none.
    pub fn delete_message(&self, channel: u64, id: u64) -> bool {
        let removed = self.state.lock().messages.remove(&(channel, id));
        let Some(removed) = removed else { return false };
        let mut event = json!({"id": id.to_string(), "channel_id": channel.to_string()});
        if let Some(guild) = removed.get("guild_id") {
            event["guild_id"] = guild.clone();
        }
        self.emit("MESSAGE_DELETE", event);
        true
    }

    /// The commands of a guild, or the global ones.
    pub fn commands(&self, guild: Option<u64>) -> Vec<Value> {
        self.state.lock().commands.get(&guild).cloned().unwrap_or_default()
    }

    fn command_body(&self, guild: Option<u64>, body: &Value) -> Value {
        let mut command = body.clone();
        command["id"] = json!(self.next_id().to_string());
        command["application_id"] = json!(self.application_id.to_string());
        command["version"] = json!("1");
        if command.get("type").is_none() {
            command["type"] = json!(1);
        }
        if let Some(guild) = guild {
            command["guild_id"] = json!(guild.to_string());
        }
        command
    }

    /// Replaces the commands of a guild, or the global ones.
    pub fn set_commands(&self, guild: Option<u64>, list: &[Value]) -> Vec<Value> {
        let commands: Vec<Value> = list.iter().map(|c| self.command_body(guild, c)).collect();
        self.state.lock().commands.insert(guild, commands.clone());
        commands
    }

    /// Adds a command.
    pub fn add_command(&self, guild: Option<u64>, body: &Value) -> Value {
        let command = self.command_body(guild, body);
        self.state.lock().commands.entry(guild).or_default().push(command.clone());
        command
    }

    /// Deletes a command. False when there is none.
    pub fn delete_command(&self, guild: Option<u64>, id: &str) -> bool {
        let mut state = self.state.lock();
        let Some(list) = state.commands.get_mut(&guild) else { return false };
        let before = list.len();
        list.retain(|c| c["id"].as_str() != Some(id));
        list.len() != before
    }

    /// Records the first answer to an interaction. False if it already has one.
    pub fn answer_interaction(&self, token: &str, body: &Value) -> bool {
        let data = body.get("data").cloned().unwrap_or(json!({}));
        let original = self.message_body(json!("0"), &data);
        let mut state = self.state.lock();
        let messages = state.answered.entry(token.to_owned()).or_default();
        if messages.contains_key("@original") {
            return false;
        }
        messages.insert("@original".to_owned(), original);
        true
    }

    /// A further message on an interaction token.
    pub fn create_followup(&self, token: &str, body: &Value) -> Value {
        let message = self.message_body(json!("0"), body);
        let id = message["id"].as_str().unwrap_or_default().to_owned();
        self.state.lock().answered.entry(token.to_owned()).or_default().insert(id, message.clone());
        message
    }

    /// A message of an interaction token, by id or `@original`.
    pub fn webhook_message(&self, token: &str, id: &str) -> Option<Value> {
        self.state.lock().answered.get(token)?.get(id).cloned()
    }

    /// Edits a message of an interaction token.
    pub fn edit_webhook_message(&self, token: &str, id: &str, body: &Value) -> Option<Value> {
        let mut state = self.state.lock();
        let message = state.answered.get_mut(token)?.get_mut(id)?;
        overlay(message, body);
        Some(message.clone())
    }

    /// Deletes a message of an interaction token. False when there is none.
    pub fn delete_webhook_message(&self, token: &str, id: &str) -> bool {
        self.state.lock().answered.get_mut(token).is_some_and(|m| m.remove(id).is_some())
    }
}

/// Puts the fields of an edit on top of a message.
fn overlay(message: &mut Value, body: &Value) {
    if let Some(fields) = body.as_object() {
        for (key, value) in fields {
            message[key] = value.clone();
        }
    }
    message["edited_timestamp"] = json!("2020-01-01T00:00:01.000000+00:00");
}
