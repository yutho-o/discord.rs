//! Discord gateway: one shard and a manager over it. No dependency on REST or the cache. Decoded
//! frames leave through [`FrameSink`], and the URL arrives through [`GatewayInfoProvider`], which
//! the facade implements over `Rest`.
//!
//! The socket task owns the WebSocket and the timers. It never decodes a payload body and never
//! waits on anything downstream, so a slow handler cannot delay a heartbeat.
//!
//! The proof of concept runs one shard, `[0, 1]`.

pub mod compress;
pub mod connector;
pub mod envelope;
pub mod error;
pub mod events;
pub mod info;
pub mod manager;
pub mod session;
pub mod shard;

pub use compress::{Compression, Decompressor};
pub use error::GatewayError;
pub use events::{Flow, Frame, FrameSink, GatewayCommand, ShardNotice, ShardStatus, ShutdownMode};
pub use info::{GatewayInfo, GatewayInfoProvider};
pub use manager::{ShardManager, ShardManagerBuilder, ShardsHandle};
pub use session::{MemorySessionStore, SessionInfo, SessionStore};
pub use shard::{ShardHandle, ShardOptions};

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ShardsHandle>();
    assert_send_sync::<GatewayError>();
    assert_send_sync::<Frame>();
};
