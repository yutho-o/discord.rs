//! The voice gateway, version 8: opcodes, JSON bodies and binary frames.
//!
//! Shapes follow discord-api-types `voice/v8.ts` (MIT, (c) 2020 vladfrangu) and the DAVE protocol
//! whitepaper. JSON frames are `{ "op", "d", "seq"? }`. Binary frames carry DAVE's MLS messages: from
//! the server `[seq: u16 BE][op: u8][payload]`, from the client `[op: u8][payload]`.

use dr_core::Secret;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Voice gateway opcodes.
#[allow(missing_docs)] // each is named as in the documentation
pub mod op {
    pub const IDENTIFY: u8 = 0;
    pub const SELECT_PROTOCOL: u8 = 1;
    pub const READY: u8 = 2;
    pub const HEARTBEAT: u8 = 3;
    pub const SESSION_DESCRIPTION: u8 = 4;
    pub const SPEAKING: u8 = 5;
    pub const HEARTBEAT_ACK: u8 = 6;
    pub const RESUME: u8 = 7;
    pub const HELLO: u8 = 8;
    pub const RESUMED: u8 = 9;
    pub const CLIENTS_CONNECT: u8 = 11;
    pub const CLIENT_DISCONNECT: u8 = 13;
    pub const DAVE_PREPARE_TRANSITION: u8 = 21;
    pub const DAVE_EXECUTE_TRANSITION: u8 = 22;
    pub const DAVE_TRANSITION_READY: u8 = 23;
    pub const DAVE_PREPARE_EPOCH: u8 = 24;
    pub const DAVE_MLS_EXTERNAL_SENDER: u8 = 25;
    pub const DAVE_MLS_KEY_PACKAGE: u8 = 26;
    pub const DAVE_MLS_PROPOSALS: u8 = 27;
    pub const DAVE_MLS_COMMIT_WELCOME: u8 = 28;
    pub const DAVE_MLS_ANNOUNCE_COMMIT_TRANSITION: u8 = 29;
    pub const DAVE_MLS_WELCOME: u8 = 30;
    pub const DAVE_MLS_INVALID_COMMIT_WELCOME: u8 = 31;
}

/// A JSON frame, either way.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceFrame<T> {
    /// Opcode.
    pub op: u8,
    /// Body.
    pub d: T,
    /// The server's sequence, on frames that have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
}

impl<T: Serialize> VoiceFrame<T> {
    /// A frame to send.
    pub fn new(op: u8, d: T) -> Self {
        Self { op, d, seq: None }
    }

    /// The frame as JSON text.
    pub fn to_text(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Op 0 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identify {
    /// The guild id, as a string.
    pub server_id: String,
    /// The bot's user id.
    pub user_id: String,
    /// From the bot's `VOICE_STATE_UPDATE`.
    pub session_id: Secret,
    /// From `VOICE_SERVER_UPDATE`.
    pub token: Secret,
    /// The highest DAVE version this client speaks, 0 for none.
    #[serde(default)]
    pub max_dave_protocol_version: u16,
}

/// Op 7 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Resume {
    /// The guild id.
    pub server_id: String,
    /// The voice session.
    pub session_id: Secret,
    /// The voice token.
    pub token: Secret,
    /// The last sequence received.
    pub seq_ack: i64,
}

/// Op 3 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Heartbeat {
    /// A nonce, echoed in the ack.
    pub t: u64,
    /// The last sequence received.
    pub seq_ack: i64,
}

/// Op 8 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Hello {
    /// Milliseconds between heartbeats. A float on the wire.
    pub heartbeat_interval: f64,
}

/// Op 2 body: where to send audio.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ready {
    /// This client's synchronization source.
    pub ssrc: u32,
    /// The voice server's UDP address.
    pub ip: String,
    /// Its UDP port.
    pub port: u16,
    /// The encryption modes it supports.
    #[serde(default)]
    pub modes: Vec<String>,
}

/// Op 1 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectProtocol {
    /// Always `udp`.
    pub protocol: String,
    /// The address IP discovery found, and the mode chosen.
    pub data: SelectProtocolData,
}

/// The `data` of op 1.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectProtocolData {
    /// This client's public address.
    pub address: String,
    /// Its public port.
    pub port: u16,
    /// The encryption mode.
    pub mode: String,
}

/// Op 4 body: the key to encrypt with.
#[derive(Clone, Deserialize, Serialize)]
pub struct SessionDescription {
    /// The mode chosen.
    pub mode: String,
    /// The 32 byte key.
    pub secret_key: Vec<u8>,
    /// The DAVE version in use, 0 when the call is not end-to-end encrypted.
    #[serde(default)]
    pub dave_protocol_version: u16,
}

impl std::fmt::Debug for SessionDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionDescription")
            .field("mode", &self.mode)
            .field("dave_protocol_version", &self.dave_protocol_version)
            .finish_non_exhaustive()
    }
}

/// Op 5 body. Sent before audio. From the server, it also maps a user to their SSRC.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Speaking {
    /// Bit flags: 1 microphone, 2 soundshare, 4 priority.
    pub speaking: u8,
    /// Always 0 from a bot.
    #[serde(default)]
    pub delay: u32,
    /// The SSRC that speaks.
    pub ssrc: u32,
    /// Who, on a frame from the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Op 11 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientsConnect {
    /// Users now in the call.
    pub user_ids: Vec<String>,
}

/// Op 13 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientDisconnect {
    /// The user who left.
    pub user_id: String,
}

/// Op 21 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrepareTransition {
    /// The transition, 0 for a (re)initialization.
    pub transition_id: u16,
    /// The DAVE version after it.
    pub protocol_version: u16,
}

/// Op 22 and op 23 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransitionId {
    /// The transition.
    pub transition_id: u16,
}

/// Op 24 body.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrepareEpoch {
    /// The MLS epoch. 1 means a new group.
    pub epoch: u64,
    /// The DAVE version.
    pub protocol_version: u16,
}

/// A JSON frame received, with its body still raw.
#[derive(Clone, Debug, Deserialize)]
pub struct Incoming {
    /// Opcode.
    pub op: u8,
    /// Body.
    #[serde(default)]
    pub d: Value,
    /// Sequence, if sent.
    #[serde(default)]
    pub seq: Option<i64>,
}

/// A binary frame from the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryFrame<'a> {
    /// Its sequence.
    pub seq: u16,
    /// Its opcode.
    pub op: u8,
    /// The rest.
    pub payload: &'a [u8],
}

impl<'a> BinaryFrame<'a> {
    /// Reads `[seq: u16 BE][op: u8][payload]`. `None` when shorter than 3 bytes.
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        let (head, payload) = bytes.split_at_checked(3)?;
        Some(Self { seq: u16::from_be_bytes([head[0], head[1]]), op: head[2], payload })
    }
}

/// A binary frame to send: `[op][payload]`.
pub fn binary(op: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(op);
    out.extend_from_slice(payload);
    out
}

/// Close codes after which the session can resume. Upstream: below 4000, or 4015.
pub fn can_resume(code: u16) -> bool {
    code < 4000 || code == 4015
}

/// What a close code means, for the error.
pub fn close_reason(code: u16) -> &'static str {
    match code {
        4001 => "unknown opcode",
        4002 => "a payload failed to decode",
        4003 => "sent before identifying",
        4004 => "authentication failed",
        4005 => "already authenticated",
        4006 => "the session is no longer valid",
        4009 => "the session timed out",
        4011 => "server not found",
        4012 => "unknown protocol",
        4014 => "disconnected: the channel was deleted, or the bot was kicked or moved",
        4015 => "the voice server crashed",
        4016 => "unknown encryption mode",
        4017 => "end-to-end encryption (DAVE) is required",
        4020 => "bad request",
        4021 => "disconnected: rate limited",
        4022 => "disconnected: the call was terminated",
        _ => "closed",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn binary_frames_read_and_write() {
        let frame = BinaryFrame::parse(&[0x01, 0x02, 25, 9, 8]).unwrap();
        assert_eq!(frame, BinaryFrame { seq: 258, op: 25, payload: &[9, 8] });
        assert!(BinaryFrame::parse(&[1, 2]).is_none());
        assert_eq!(binary(26, &[7]), [26, 7]);
    }

    #[test]
    fn frames_match_the_documentation() {
        let hb = VoiceFrame::new(op::HEARTBEAT, Heartbeat { t: 5, seq_ack: 10 }).to_text().unwrap();
        assert_eq!(hb, r#"{"op":3,"d":{"t":5,"seq_ack":10}}"#);
        let sd: SessionDescription =
            serde_json::from_str(r#"{"mode":"aead_aes256_gcm_rtpsize","secret_key":[1,2],"dave_protocol_version":1}"#)
                .unwrap();
        assert_eq!(sd.secret_key, [1, 2]);
        assert!(!format!("{sd:?}").contains("secret_key"), "the key stays out of logs");
        let hello: Hello = serde_json::from_str(r#"{"heartbeat_interval":13750.5}"#).unwrap();
        assert!((hello.heartbeat_interval - 13750.5).abs() < f64::EPSILON);
        assert!(can_resume(1006) && can_resume(4015) && !can_resume(4014) && !can_resume(4006));
    }
}
