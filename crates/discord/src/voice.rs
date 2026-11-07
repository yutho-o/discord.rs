//! Voice channels, with the `voice` feature: join, play, receive, leave. The connection itself is
//! `dr_voice`, re-exported as `discord::voice`; this module joins on the main gateway and keeps each
//! guild's connection up to date as Discord moves it.
//!
//! ```no_run
//! # use discord::prelude::*;
//! # async fn f(ctx: Context<()>, guild: GuildId, user: UserId) -> Result<(), BoxError> {
//! use discord::voice::OggOpus;
//! // The channel the user is in, from the cache (needs the GUILD_VOICE_STATES intent).
//! let Some(channel) = ctx.cache().voice_state(guild, user).and_then(|s| s.channel_id) else {
//!     return Ok(());
//! };
//! let connection = ctx.voice().join(guild, channel).await?;
//! let playback = connection.play(OggOpus::open("song.opus").await?);
//! playback.finished().await;
//! ctx.voice().leave(guild).await?;
//! # Ok(()) }
//! ```

use crate::events::{VoiceServerUpdate, VoiceStateUpdate};
use crate::{Client, Error};
use dr_core::Secret;
use dr_gateway::GatewayCommand;
use dr_model::{ChannelId, GuildId, UpdateVoiceState, VoiceServerUpdateEvent, VoiceState};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub use dr_voice::*;

/// How long `join` waits for each of Discord's two answers.
const ANSWER_WAIT: Duration = Duration::from_secs(15);

/// The live voice connections of a client, by guild.
#[derive(Debug, Default)]
pub(crate) struct VoiceRegistry {
    connections: Mutex<HashMap<GuildId, VoiceConnection>>,
    /// The bot's voice session in each guild, from its last `VOICE_STATE_UPDATE`.
    sessions: Mutex<HashMap<GuildId, (ChannelId, Secret)>>,
    /// Guilds with a `join` in progress, whose events `join` reads itself.
    joining: Mutex<HashSet<GuildId>>,
}

/// How to join.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct JoinOptions {
    /// Join muted.
    pub self_mute: bool,
    /// Join deafened. Discord then sends this bot no audio.
    pub self_deaf: bool,
    /// The connection's own options.
    pub connection: VoiceOptions,
}

impl JoinOptions {
    /// Join deafened, as a music bot usually does.
    pub fn deafened(mut self) -> Self {
        self.self_deaf = true;
        self
    }

    /// Join muted.
    pub fn muted(mut self) -> Self {
        self.self_mute = true;
        self
    }
}

/// The client's voice connections. Get it from `Client::voice` or `ctx.voice()`.
#[derive(Clone, Debug)]
pub struct VoiceManager {
    client: Client,
}

impl Client {
    /// Voice channels: join, leave, and the connection of each guild.
    pub fn voice(&self) -> VoiceManager {
        VoiceManager { client: self.clone() }
    }
}

impl VoiceManager {
    /// The connection in `guild`, if the bot is connected there.
    pub fn get(&self, guild: GuildId) -> Option<VoiceConnection> {
        self.client.0.voice.connections.lock().get(&guild).cloned()
    }

    /// Joins a voice channel and waits until audio can flow. Needs the `GUILDS` intent and, in the
    /// channel, `CONNECT` (and `SPEAK` to play). A connection already in `guild` is replaced.
    pub async fn join(&self, guild: GuildId, channel: ChannelId) -> Result<VoiceConnection, Error> {
        self.join_with(guild, channel, JoinOptions::default()).await
    }

    /// [`Self::join`] with options.
    pub async fn join_with(
        &self,
        guild: GuildId,
        channel: ChannelId,
        options: JoinOptions,
    ) -> Result<VoiceConnection, Error> {
        let registry = &self.client.0.voice;
        if !registry.joining.lock().insert(guild) {
            return Err(Error::Other("a join is already in progress in this guild".into()));
        }
        let result = self.join_inner(guild, channel, options).await;
        registry.joining.lock().remove(&guild);
        result
    }

    async fn join_inner(
        &self,
        guild: GuildId,
        channel: ChannelId,
        options: JoinOptions,
    ) -> Result<VoiceConnection, Error> {
        let client = &self.client;
        let gateway = client.0.gateway.get().ok_or(Error::NotReady)?;
        let me = client.cache().me().ok_or(Error::NotReady)?.id;
        if let Some(old) = client.0.voice.connections.lock().remove(&guild) {
            old.destroy();
        }
        // Listen before asking, so that neither answer can come first.
        let mut states =
            client.collector::<VoiceStateUpdate>(move |u| u.new.user_id == me && u.new.guild_id == Some(guild));
        let mut servers = client.collector::<VoiceServerUpdate>(move |s| s.guild_id == guild && s.endpoint.is_some());
        let request = UpdateVoiceState {
            guild_id: guild,
            channel_id: Some(channel),
            self_mute: options.self_mute,
            self_deaf: options.self_deaf,
        };
        gateway.send(gateway.shard_for(guild), GatewayCommand::UpdateVoiceState(request))?;
        let state = states.next_within(ANSWER_WAIT).await.ok_or(Error::Timeout { what: "the bot's voice state" })?;
        let server = servers.next_within(ANSWER_WAIT).await.ok_or(Error::Timeout { what: "a voice server" })?;
        let session = state.new.session_id.clone();
        client.0.voice.sessions.lock().insert(guild, (channel, session.clone()));
        let info = connection_info(&server, channel, me, session)?;
        let connection = VoiceConnection::connect(info, options.connection).await?;
        client.0.voice.connections.lock().insert(guild, connection.clone());
        Ok(connection)
    }

    /// Leaves the voice channel of `guild` and closes its connection. Nothing to leave is not an
    /// error.
    pub async fn leave(&self, guild: GuildId) -> Result<(), Error> {
        let connection = self.client.0.voice.connections.lock().remove(&guild);
        self.client.0.voice.sessions.lock().remove(&guild);
        if let Some(connection) = &connection {
            connection.destroy();
        }
        let gateway = self.client.0.gateway.get().ok_or(Error::NotReady)?;
        let request = UpdateVoiceState { guild_id: guild, channel_id: None, self_mute: false, self_deaf: false };
        gateway.send(gateway.shard_for(guild), GatewayCommand::UpdateVoiceState(request))?;
        Ok(())
    }
}

fn connection_info(
    server: &VoiceServerUpdateEvent,
    channel: ChannelId,
    user: dr_model::UserId,
    session: Secret,
) -> Result<ConnectionInfo, Error> {
    let endpoint = server.endpoint.clone().ok_or(Error::Other("the voice server has no endpoint yet".into()))?;
    Ok(ConnectionInfo {
        endpoint,
        guild_id: server.guild_id,
        channel_id: channel,
        user_id: user,
        session_id: session,
        token: server.token.clone(),
    })
}

/// Keeps connections in step with the main gateway: the bot moved, was disconnected, or its voice
/// server changed. A guild being joined is left to `join`.
pub(crate) fn on_voice_state(client: &Client, state: &VoiceState) {
    let Some(guild) = state.guild_id else { return };
    if client.cache().me().is_none_or(|me| me.id != state.user_id) {
        return;
    }
    let registry = &client.0.voice;
    if registry.joining.lock().contains(&guild) {
        return;
    }
    match state.channel_id {
        None => {
            registry.sessions.lock().remove(&guild);
            if let Some(connection) = registry.connections.lock().remove(&guild) {
                connection.destroy();
            }
        }
        Some(channel) => {
            registry.sessions.lock().insert(guild, (channel, state.session_id.clone()));
        }
    }
}

pub(crate) fn on_voice_server(client: &Client, server: &VoiceServerUpdateEvent) {
    let registry = &client.0.voice;
    if registry.joining.lock().contains(&server.guild_id) || server.endpoint.is_none() {
        return;
    }
    let Some(connection) = registry.connections.lock().get(&server.guild_id).cloned() else { return };
    let Some((channel, session)) = registry.sessions.lock().get(&server.guild_id).cloned() else { return };
    let Some(me) = client.cache().me() else { return };
    if let Ok(info) = connection_info(server, channel, me.id, session) {
        connection.update_server(info);
    }
}
