//! Contract crate of discord.rs: ids, open enums, tagged unions, `Patch`, wire helpers and the
//! request body types. Everything above it depends on it, and it has no tokio.

pub mod endpoints;
pub mod id;
pub mod macros;
pub mod patch;
pub mod secret;
pub mod time;
pub mod wire;

pub use endpoints::Endpoints;
pub use id::{DISCORD_EPOCH_MS, Id, marker};
pub use patch::Patch;
pub use secret::Secret;
pub use time::Timestamp;
pub use wire::{
    BodyError, BoxError, DecodeError, IntoBody, Lenient, RequestBody, ValidationError, decode, lenient_i64,
    null_default,
};

#[doc(hidden)]
pub mod __private {
    pub use bitflags;
    pub use compact_str;
    pub use serde;
    pub use serde_json;
}
