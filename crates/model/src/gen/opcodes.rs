//! Gateway opcodes and close codes. Tags and opcodes peek as `u32`, `ARCHITECTURE.md` 7.4.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu).
//! Changed: translated to `open_int!`. The reconnect policy per close code is in `dr_gateway`.

use dr_core::open_int;

/// The gateway version this crate describes, sent as `v` when connecting.
pub const GATEWAY_VERSION: &str = "10";

open_int! {
    /// Gateway opcodes.
    GatewayOpcode: u32 {
        /// An event was dispatched
        DISPATCH = 0,
        /// A bidirectional opcode to maintain an active gateway connection. Fired periodically by the client,
        /// or fired by the gateway to request an immediate heartbeat from the client.
        HEARTBEAT = 1,
        /// Starts a new session during the initial handshake
        IDENTIFY = 2,
        /// Update the client's presence
        PRESENCE_UPDATE = 3,
        /// Used to join/leave or move between voice channels
        VOICE_STATE_UPDATE = 4,
        /// Resume a previous session that was disconnected
        RESUME = 6,
        /// You should attempt to reconnect and resume immediately
        RECONNECT = 7,
        /// Request information about offline guild members in a large guild
        REQUEST_GUILD_MEMBERS = 8,
        /// The session has been invalidated. You should reconnect and identify/resume accordingly
        INVALID_SESSION = 9,
        /// Sent immediately after connecting, contains the `heartbeat_interval` to use
        HELLO = 10,
        /// Sent in response to receiving a heartbeat to acknowledge that it has been received
        HEARTBEAT_ACK = 11,
        /// Request information about soundboard sounds in a set of guilds
        REQUEST_SOUNDBOARD_SOUNDS = 31,
        /// Request ephemeral channel data for channels in a guild.
        REQUEST_CHANNEL_INFO = 43,
    }
}

open_int! {
    /// Gateway close codes. There is no 4006.
    GatewayCloseCode: u16 {
        /// We're not sure what went wrong. Try reconnecting?
        UNKNOWN_ERROR = 4000,
        /// You sent an invalid Gateway opcode or an invalid payload for an opcode. Don't do that!
        UNKNOWN_OPCODE = 4001,
        /// You sent an invalid payload to us. Don't do that!
        DECODE_ERROR = 4002,
        /// You sent us a payload prior to identifying
        NOT_AUTHENTICATED = 4003,
        /// The account token sent with your identify payload is incorrect
        AUTHENTICATION_FAILED = 4004,
        /// You sent more than one identify payload. Don't do that!
        ALREADY_AUTHENTICATED = 4005,
        /// The sequence sent when resuming the session was invalid. Reconnect and start a new session
        INVALID_SEQ = 4007,
        /// Woah nelly! You're sending payloads to us too quickly. Slow it down! You will be disconnected on
        /// receiving this
        RATE_LIMITED = 4008,
        /// Your session timed out. Reconnect and start a new one
        SESSION_TIMED_OUT = 4009,
        /// You sent us an invalid shard when identifying
        INVALID_SHARD = 4010,
        /// The session would have handled too many guilds - you are required to shard your connection in
        /// order to connect
        SHARDING_REQUIRED = 4011,
        /// You sent an invalid version for the gateway
        INVALID_API_VERSION = 4012,
        /// You sent an invalid intent for a Gateway Intent. You may have incorrectly calculated the bitwise
        /// value
        INVALID_INTENTS = 4013,
        /// You sent a disallowed intent for a Gateway Intent. You may have tried to specify an intent that
        /// you have not enabled or are not whitelisted for
        DISALLOWED_INTENTS = 4014,
    }
}
