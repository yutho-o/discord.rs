//! What the client sends on the gateway.
//!
//! Shapes follow discord-api-types `gateway/v10.ts` (MIT, (c) 2020 vladfrangu) and the write rules
//! of `ARCHITECTURE.md` 10.1. A send struct is serialized with [`GatewayFrame`] around it. Each also
//! reads back, because wire structs derive both directions and the mock parses what a client sends.

use crate::{GatewayOpcode, Intents};
use dr_core::{Secret, open_str};
use serde::{Deserialize, Serialize};

/// `{ "op": op, "d": d }`, the shape of every message sent to the gateway.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GatewayFrame<T> {
    /// Opcode.
    pub op: u32,
    /// Body. A `None` here writes `"d": null`, which op 1 needs.
    pub d: T,
}

impl GatewayFrame<Identify> {
    /// An op 2 frame.
    pub fn identify(d: Identify) -> Self {
        Self { op: GatewayOpcode::IDENTIFY.get(), d }
    }
}

impl GatewayFrame<Resume> {
    /// An op 6 frame.
    pub fn resume(d: Resume) -> Self {
        Self { op: GatewayOpcode::RESUME.get(), d }
    }
}

impl GatewayFrame<Heartbeat> {
    /// An op 1 frame. `None` before the first sequence, which writes `"d": null`.
    pub fn heartbeat(d: Heartbeat) -> Self {
        Self { op: GatewayOpcode::HEARTBEAT.get(), d }
    }
}

impl GatewayFrame<UpdatePresence> {
    /// An op 3 frame.
    pub fn presence(d: UpdatePresence) -> Self {
        Self { op: GatewayOpcode::PRESENCE_UPDATE.get(), d }
    }
}

/// Op 2 body. `shard`, `capabilities` and `compress` are always written, `large_threshold` and
/// `presence` only when set, as `@discordjs/ws` does.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identify {
    /// Bot token with no `Bot ` prefix. Redacted in `Debug`.
    pub token: Secret,
    /// Client description.
    pub properties: IdentifyProperties,
    /// Whether `compress: true` per-payload compression is on. Always false for now.
    #[serde(default)]
    pub compress: bool,
    /// Offline member threshold, from 50 to 250. Written only when set. Discord defaults to 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_threshold: Option<u32>,
    /// `[shard_id, shard_count]`. Reads as `[0, 1]` when absent.
    #[serde(default = "single_shard")]
    pub shard: [u32; 2],
    /// Initial presence, sent only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<UpdatePresence>,
    /// Intents.
    pub intents: Intents,
    /// Client capabilities, `[gateway 11.4]`. Written even when 0.
    #[serde(default)]
    pub capabilities: u64,
}

const fn single_shard() -> [u32; 2] {
    [0, 1]
}

/// `properties` of an identify.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IdentifyProperties {
    /// Operating system.
    #[serde(default)]
    pub os: String,
    /// Library name.
    #[serde(default)]
    pub browser: String,
    /// Library name.
    #[serde(default)]
    pub device: String,
}

/// Op 6 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Resume {
    /// Bot token with no `Bot ` prefix. Redacted in `Debug`.
    pub token: Secret,
    /// Session id. Redacted in `Debug`.
    pub session_id: Secret,
    /// Last sequence received.
    pub seq: u64,
}

/// Op 1 body: the last sequence, or `null` before the first one.
pub type Heartbeat = Option<u64>;

/// Op 3 body, also the `presence` of an identify.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdatePresence {
    /// Unix time in milliseconds of when the client went idle, or null when it is not idle.
    pub since: Option<u64>,
    /// Activities.
    pub activities: Vec<Activity>,
    /// Status.
    pub status: PresenceStatus,
    /// Whether the client is away from keyboard.
    pub afk: bool,
}

impl UpdatePresence {
    /// A presence with this status and no activity.
    pub fn new(status: PresenceStatus) -> Self {
        Self { since: None, activities: Vec::new(), status, afk: false }
    }

    /// Adds an activity.
    #[must_use]
    pub fn activity(mut self, activity: Activity) -> Self {
        self.activities.push(activity);
        self
    }
}

/// One activity in a presence update.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Activity {
    /// Activity name.
    pub name: String,
    /// Activity kind, one of the constants of this type such as [`Activity::PLAYING`].
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: u16,
    /// Custom status text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Stream URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Activity {
    fn of_kind(kind: u16, name: impl Into<String>) -> Self {
        Self { name: name.into(), kind, state: None, url: None }
    }

    /// "Playing {name}".
    pub fn playing(name: impl Into<String>) -> Self {
        Self::of_kind(Self::PLAYING, name)
    }

    /// "Listening to {name}".
    pub fn listening(name: impl Into<String>) -> Self {
        Self::of_kind(Self::LISTENING, name)
    }

    /// "Watching {name}".
    pub fn watching(name: impl Into<String>) -> Self {
        Self::of_kind(Self::WATCHING, name)
    }

    /// "Competing in {name}".
    pub fn competing(name: impl Into<String>) -> Self {
        Self::of_kind(Self::COMPETING, name)
    }

    /// "Streaming {name}", with a link to the stream.
    pub fn streaming(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self { url: Some(url.into()), ..Self::of_kind(Self::STREAMING, name) }
    }

    /// A custom status showing this text.
    pub fn custom(text: impl Into<String>) -> Self {
        Self { state: Some(text.into()), ..Self::of_kind(Self::CUSTOM, "Custom Status") }
    }

    /// Kind "Playing {name}".
    pub const PLAYING: u16 = 0;
    /// Kind "Streaming {name}", the kind that uses `url`.
    pub const STREAMING: u16 = 1;
    /// Kind "Listening to {name}".
    pub const LISTENING: u16 = 2;
    /// Kind "Watching {name}".
    pub const WATCHING: u16 = 3;
    /// Kind "{state}", a custom status.
    pub const CUSTOM: u16 = 4;
    /// Kind "Competing in {name}".
    pub const COMPETING: u16 = 5;
}

open_str! {
    /// A status to show. `invisible` is valid here and never arrives in a received presence.
    #[doc(alias = "PresenceUpdateStatus")]
    PresenceStatus {
        Online = "online",
        DoNotDisturb = "dnd",
        Idle = "idle",
        Invisible = "invisible",
        Offline = "offline",
    }
}

/// Op 8 body: asks for the members of a guild, answered by `GUILD_MEMBERS_CHUNK` events. Set either
/// `query` (with `limit`) or `user_ids`. Listing every member needs the `GUILD_MEMBERS` intent,
/// and presences need `GUILD_PRESENCES`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestGuildMembers {
    /// Guild id.
    pub guild_id: crate::GuildId,
    /// Usernames that start with this. An empty string with `limit` 0 means every member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The most members to send, 0 for no limit when listing every member.
    #[serde(default)]
    pub limit: u32,
    /// Whether to send their presences too.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub presences: bool,
    /// Exactly these users, up to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<crate::UserId>>,
    /// Comes back in each chunk, to match them to this request. At most 32 bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

impl RequestGuildMembers {
    /// Every member of the guild.
    pub fn all(guild_id: crate::GuildId) -> Self {
        Self { guild_id, query: Some(String::new()), limit: 0, presences: false, user_ids: None, nonce: None }
    }

    /// Members whose username starts with `query`, at most `limit`.
    pub fn search(guild_id: crate::GuildId, query: impl Into<String>, limit: u32) -> Self {
        Self { guild_id, query: Some(query.into()), limit, presences: false, user_ids: None, nonce: None }
    }

    /// These users, at most 100.
    pub fn users(guild_id: crate::GuildId, ids: impl IntoIterator<Item = crate::UserId>) -> Self {
        let ids = ids.into_iter().collect();
        Self { guild_id, query: None, limit: 0, presences: false, user_ids: Some(ids), nonce: None }
    }
}

impl GatewayFrame<RequestGuildMembers> {
    /// An op 8 frame.
    pub fn request_guild_members(d: RequestGuildMembers) -> Self {
        Self { op: GatewayOpcode::REQUEST_GUILD_MEMBERS.get(), d }
    }
}
