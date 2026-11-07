//! Voice on the main gateway: voice states, the voice server handoff and op 4. The voice gateway
//! itself, with its own opcodes, is in the voice crate.

use crate::{ChannelId, GatewayFrame, GatewayOpcode, GuildId, Member, UserId};
use dr_core::{Secret, Timestamp};
use serde::{Deserialize, Serialize};

/// Someone's voice connection state: where they are and whether they are muted. The body of
/// `VOICE_STATE_UPDATE`, and an element of `Guild::voice_states`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceState {
    /// Guild id. Absent inside `GUILD_CREATE`, where the guild is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel they are in, `None` once they left.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// Who.
    pub user_id: UserId,
    /// Their member, on `VOICE_STATE_UPDATE` in a guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// The voice session. Redacted in `Debug`: with the voice server's token, it opens a connection.
    #[serde(default)]
    pub session_id: Secret,
    /// Deafened by the guild.
    #[serde(default)]
    pub deaf: bool,
    /// Muted by the guild.
    #[serde(default)]
    pub mute: bool,
    /// Deafened by themselves.
    #[serde(default)]
    pub self_deaf: bool,
    /// Muted by themselves.
    #[serde(default)]
    pub self_mute: bool,
    /// Streaming with Go Live.
    #[serde(default)]
    pub self_stream: bool,
    /// Their camera is on.
    #[serde(default)]
    pub self_video: bool,
    /// Kept from speaking, as in a stage channel.
    #[serde(default)]
    pub suppress: bool,
    /// When they asked to speak in a stage channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_to_speak_timestamp: Option<Timestamp>,
}

/// Body of `VOICE_SERVER_UPDATE`: which voice server to connect to, and with what token.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceServerUpdateEvent {
    /// The voice connection token. Redacted in `Debug`.
    pub token: Secret,
    /// Guild id.
    pub guild_id: GuildId,
    /// The voice server's host, with no scheme. `None` while Discord finds a new one: wait for the
    /// next update.
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Op 4 body: joins, moves or leaves a voice channel. Discord answers with `VOICE_STATE_UPDATE` and,
/// on a join, `VOICE_SERVER_UPDATE`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateVoiceState {
    /// Guild id.
    pub guild_id: GuildId,
    /// The channel to join, `None` to leave. Always written, as `null` to leave.
    pub channel_id: Option<ChannelId>,
    /// Join muted.
    #[serde(default)]
    pub self_mute: bool,
    /// Join deafened.
    #[serde(default)]
    pub self_deaf: bool,
}

impl GatewayFrame<UpdateVoiceState> {
    /// An op 4 frame.
    pub fn voice_state(d: UpdateVoiceState) -> Self {
        Self { op: GatewayOpcode::VOICE_STATE_UPDATE.get(), d }
    }
}
