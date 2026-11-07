//! Managers, with one shape throughout.:
//!
//! - `get(id) -> Option<Bound<T>>` reads the cache. Sync, no I/O.
//! - `fetch(id) -> Call<Bound<T>>` asks Discord. The result does not enter the cache.
//! - `resolve(id).await -> Result<Bound<T>, Error>` reads the cache and falls back to a fetch, whose
//!   result enters the cache through `insert_*_if_absent`.

pub mod automod;
pub mod bans;
mod channels;
pub mod commands;
pub mod expressions;
pub mod guilds;
pub mod members;
pub mod messages;
pub mod roles;
pub mod scheduled_events;
mod threads;
pub mod users;

pub use automod::AutoModManager;
pub use bans::BansManager;
pub use channels::ChannelsManager;
pub use commands::CommandsManager;
pub use expressions::{EmojisManager, StickersManager};
pub use guilds::GuildsManager;
pub use members::MembersManager;
pub use messages::MessagesManager;
pub use roles::RolesManager;
pub use scheduled_events::ScheduledEventsManager;
pub use threads::{ThreadMembersManager, ThreadsManager};
pub use users::UsersManager;
