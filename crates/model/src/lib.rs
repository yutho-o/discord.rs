//! Discord API v10 wire types, hand-written for now in the layout the generator
//! will later fill. Every name is reachable from the crate root: `dr_model::Message`,
//! `dr_model::rest::GetGatewayBotResult`, `dr_model::routes::channel(id)`.
//!
//! Patterns are in `ARCHITECTURE.md` 7.4. No `serde(flatten)` on the receive path, no
//! `serde(tag = ...)`, no `deny_unknown_fields`. The `serde-both` feature is cut from the proof of
//! concept, so wire structs derive both directions.

pub mod audit_log;
pub mod automod;
pub mod channel;
pub mod component;
pub mod event;
pub mod gateway_send;
mod r#gen;
pub mod ids;
pub mod interaction;
pub mod interaction_options;
pub mod message_ext;
pub mod permissions;
pub mod poll;
pub mod rest_error;
pub mod scheduled_event;
pub mod stage;
pub mod sticker;
pub mod voice;
pub mod webhook;

pub use audit_log::*;
pub use automod::*;
pub use channel::*;
pub use component::*;
pub use event::*;
pub use gateway_send::*;
pub use r#gen::*;
pub use ids::*;
pub use interaction::*;
pub use interaction_options::*;
pub use poll::*;
pub use rest_error::*;
pub use scheduled_event::*;
pub use stage::*;
pub use sticker::*;
pub use voice::*;
pub use webhook::*;
