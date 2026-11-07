//! Plain wire data, in the layout the generator will fill. Hand-written for the proof of concept
//! and checked field by field against `payloads/v10` of discord-api-types.

pub mod enums;
pub mod errors;
pub mod flags;
pub mod opcodes;
pub mod payloads;
pub mod rest;
pub mod routes;

pub use enums::*;
pub use errors::*;
pub use flags::*;
pub use opcodes::*;
pub use payloads::*;
