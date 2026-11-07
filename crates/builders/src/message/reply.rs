//! Reply references.

use dr_model::{MessageId, MessageReference};

/// The `message_reference` of a reply to `message`. A deleted target does not fail the send
/// (`fail_if_not_exists: false`), matching upstream's default for replies.
pub fn reply_reference(message: MessageId) -> MessageReference {
    MessageReference { message_id: Some(message), fail_if_not_exists: Some(false), ..MessageReference::default() }
}
