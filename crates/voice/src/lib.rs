//! Discord voice for discord.rs, a port of `@discordjs/voice`: the voice gateway (version 8), UDP
//! with IP discovery, the `_rtpsize` AEAD transport encryption, DAVE end-to-end encryption, Opus
//! playback and receiving.
//!
//! This crate does not encode audio: sources hand over Opus frames, such as those of an Ogg Opus
//! file read with [`OggOpus`]. The facade's `voice` feature joins channels on the main gateway and
//! hands this crate the [`ConnectionInfo`].
//!
//! ```no_run
//! # async fn f(info: dr_voice::ConnectionInfo) -> Result<(), dr_voice::VoiceError> {
//! use dr_voice::{OggOpus, VoiceConnection, VoiceOptions};
//! let connection = VoiceConnection::connect(info, VoiceOptions::default()).await?;
//! let playback = connection.play(OggOpus::open("song.opus").await?);
//! playback.finished().await;
//! connection.destroy();
//! # Ok(()) }
//! ```

pub mod connection;
pub mod crypto;
pub mod dave;
pub mod error;
pub mod player;
pub mod protocol;
pub mod source;
pub mod udp;

pub use connection::{ConnectionInfo, VoiceConnection, VoiceOptions, VoicePacket, VoiceStatus};
pub use crypto::EncryptionMode;
pub use error::VoiceError;
pub use player::{Playback, PlaybackState};
pub use source::{AudioSource, OggOpus, OpusFrames, opus_duration};

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VoiceConnection>();
    assert_send_sync::<Playback>();
};
