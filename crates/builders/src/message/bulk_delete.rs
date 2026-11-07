//! `BulkDeleteMessages`.

use dr_core::{BodyError, IntoBody, RequestBody, ValidationError};
use dr_model::MessageId;
use serde::Serialize;

/// The body of a bulk delete: 2 to 100 messages. Discord refuses messages older than 14 days.
#[derive(Clone, Debug, Serialize)]
pub struct BulkDeleteMessages {
    messages: Vec<MessageId>,
}

impl BulkDeleteMessages {
    /// A bulk delete of these messages.
    pub fn new(messages: impl IntoIterator<Item = MessageId>) -> Self {
        Self { messages: messages.into_iter().collect() }
    }

    /// Checks the number of messages.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let n = self.messages.len();
        if (2..=100).contains(&n) {
            Ok(())
        } else {
            Err(ValidationError::new("messages", format!("must hold 2 to 100 messages, got {n}")))
        }
    }
}

impl IntoBody for BulkDeleteMessages {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
