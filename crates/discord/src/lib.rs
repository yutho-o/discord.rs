//! Write complete Discord bots in Rust. An unofficial port of discord.js, not affiliated with Discord.
//!
//! ```no_run
//! use discord::prelude::*;
//!
//! # async fn example() -> Result<(), BoxError> {
//! Client::builder(std::env::var("DISCORD_TOKEN")?)
//!     .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT)
//!     .on(MessageCreate, |_ctx, msg| async move {
//!         if msg.content == "!ping" {
//!             msg.reply("pong").await?;
//!         }
//!         Ok(())
//!     })
//!     .build()?
//!     .run()
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub use dr_builders as builders;
pub use dr_cache as cache;
pub use dr_gateway as gateway;
pub use dr_model as model;
pub use dr_rest as rest;

pub mod bound;
pub mod call;
pub mod client;
pub mod collector;
pub mod context;
pub mod error;
pub mod events;
pub mod handles;
pub mod interactions;
pub mod managers;
#[doc(hidden)]
pub mod pipeline;
pub mod prelude;

#[cfg(doctest)]
mod misuse;

pub use bound::{Bound, Deleted, Updated};
pub use call::{Bindable, Call};
pub use client::{Client, ClientBuilder, ClientStats, DispatchMode, QueueLimits, Runner};
pub use collector::{Collector, ComponentCollector};
pub use context::Context;
pub use dr_core::{Secret, Timestamp};
pub use dr_gateway::ShutdownMode;
pub use error::{BoxError, ConfigError, Error, FailureKind, HandlerFailure, HandlerResult, InteractionError};
pub use events::{GatewayEvent, RawEvent};

// The Send story, proved statically: all of these cross task boundaries.
const _: fn() = || {
    fn send_sync<T: Send + Sync>() {}
    fn send<T: Send>() {}
    send_sync::<Client>();
    send_sync::<Bound<model::Message>>();
    send_sync::<Error>();
    send::<Call<Bound<model::Message>>>();
    send::<Context<()>>();
    send::<handles::SendMessageRequest>();
    send::<interactions::ReplyRequest>();
    send::<interactions::FollowUpRequest>();
    send::<interactions::Interaction>();
};
