//! Handles and the verbs on bound values.
//!
//! A handle is a client and an id: it does no I/O to create. `client.channel(id)`, `client.guild(id)`
//! and `client.user(id)` make them. Verbs on a bound value,
//! such as `msg.reply`, are inherent impls on `Bound<T>` in the same files.

pub mod channel;
pub mod guild;
pub mod member;
pub mod message;
pub mod role;
pub mod user;

pub use channel::ChannelHandle;
pub use guild::GuildHandle;
pub use message::SendMessageRequest;
pub use role::RoleHandle;
pub use user::UserHandle;
