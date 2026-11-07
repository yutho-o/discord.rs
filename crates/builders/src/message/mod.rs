//! Message bodies.

pub mod allowed_mentions;
pub mod bulk_delete;
pub mod create;
pub mod edit;
pub mod reply;

pub use allowed_mentions::AllowedMentions;
pub use bulk_delete::BulkDeleteMessages;
pub use create::CreateMessage;
pub use edit::EditMessage;
pub use reply::reply_reference;
