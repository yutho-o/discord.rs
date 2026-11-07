//! The in-memory Discord state. `World` never depends on the gateway: writes publish a
//! [`WorldEvent`], and the gateway half subscribes.

use dr_model::{ApplicationId, ChannelId, GuildId, UserId};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
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
    messages: BTreeMap<(u64, u64), Value>,
    commands: BTreeMap<Option<u64>, Vec<Value>>,
    /// What each interaction token answered, and its messages by id. `@original` is the first answer.
    answered: HashMap<String, BTreeMap<String, Value>>,
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
        let id = self.next_id();
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

    fn message_body(&self, channel: Value, body: &Value) -> Value {
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
            "attachments": [],
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
        let edited = {
            let mut state = self.state.lock();
            let message = state.messages.get_mut(&(channel, id))?;
            overlay(message, body);
            message.clone()
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
