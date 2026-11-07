//! `GatewayError`.

use dr_core::BoxError;

/// Why a gateway operation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GatewayError {
    /// A close code that ends the shard: 4004, 4010 to 4014.
    #[error("terminal close code {code}")]
    Closed {
        /// The close code.
        code: u16,
    },
    /// The connection could not be set up.
    #[error("connect failed: {0}")]
    Connect(String),
    /// The gateway URL could not be found.
    #[error("gateway info failed: {0}")]
    Info(#[source] BoxError),
    /// A payload we built could not be encoded.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A compressed stream was corrupt. The shard reconnects with a new decompressor.
    #[error("inflate failed: {0}")]
    Inflate(String),
    /// A command is larger than the 4096 byte limit. Nothing was sent.
    #[error("payload of {size} bytes is over the limit of {limit}")]
    PayloadTooLarge {
        /// Encoded size.
        size: usize,
        /// The limit.
        limit: usize,
    },
    /// No such shard in this manager.
    #[error("unknown shard {shard}")]
    UnknownShard {
        /// The shard asked for.
        shard: u32,
    },
    /// The shard has stopped.
    #[error("the shard is not running")]
    NotRunning,
}
