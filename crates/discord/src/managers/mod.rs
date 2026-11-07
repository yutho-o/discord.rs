//! Managers, with one shape throughout.:
//!
//! - `get(id) -> Option<Bound<T>>` reads the cache. Sync, no I/O.
//! - `fetch(id) -> Call<Bound<T>>` asks Discord. The result does not enter the cache.
//! - `resolve(id).await -> Result<Bound<T>, Error>` reads the cache and falls back to a fetch, whose
//!   result enters the cache through `insert_*_if_absent`.

pub mod channels;
pub mod commands;
pub mod guilds;
pub mod members;
pub mod messages;
pub mod roles;
pub mod users;

pub use channels::ChannelsManager;
pub use commands::CommandsManager;
pub use guilds::GuildsManager;
pub use members::MembersManager;
pub use messages::MessagesManager;
pub use roles::RolesManager;
pub use users::UsersManager;
