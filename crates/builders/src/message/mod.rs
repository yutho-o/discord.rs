//! Message bodies.

pub mod allowed_mentions;
pub mod create;
pub mod edit;
pub mod reply;

pub use allowed_mentions::AllowedMentions;
pub use create::CreateMessage;
pub use edit::EditMessage;
pub use reply::reply_reference;
