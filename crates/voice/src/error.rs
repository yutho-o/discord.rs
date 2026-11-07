//! Voice errors.

use crate::crypto::CryptoError;

/// A voice connection or playback failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VoiceError {
    /// The voice gateway or its UDP socket could not be reached.
    #[error("voice connection failed: {0}")]
    Connect(String),
    /// The voice gateway closed for good.
    #[error("voice gateway closed with {code}: {reason}")]
    Closed {
        /// The close code.
        code: u16,
        /// What it means.
        reason: &'static str,
    },
    /// The connection did not become ready in time.
    #[error("timed out: {0}")]
    Timeout(&'static str),
    /// The server offered no encryption mode this crate speaks.
    #[error("no supported encryption mode among {0:?}")]
    NoMode(Vec<String>),
    /// A packet did not encrypt.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// Reading a source or a socket failed.
    #[error(transparent)]
    Io(std::io::Error),
    /// The server sent something unexpected.
    #[error("voice protocol: {0}")]
    Protocol(String),
    /// A source could not be read as Opus.
    #[error("audio source: {0}")]
    Source(String),
    /// The connection was destroyed.
    #[error("the voice connection is closed")]
    Destroyed,
}
