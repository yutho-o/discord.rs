//! Autocomplete requests.

use super::CommandInteraction;
use super::options::CommandOptions;
use super::reply::{ReplyBody, ReplyRequest, ReplyTarget};
use crate::Bound;
use serde::Serialize;

/// One suggestion for an autocomplete answer.
#[derive(Clone, Debug, Serialize)]
pub struct AutocompleteChoice {
    /// What the user sees.
    pub name: String,
    /// What the command receives: a JSON string or number.
    pub value: serde_json::Value,
}

impl AutocompleteChoice {
    /// A suggestion with a string value.
    pub fn string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self { name: name.into(), value: serde_json::Value::String(value.into()) }
    }

    /// A suggestion with an integer value.
    pub fn integer(name: impl Into<String>, value: i64) -> Self {
        Self { name: name.into(), value: serde_json::Value::from(value) }
    }
}

/// A user is typing into an option with autocomplete on.
#[derive(Debug)]
#[non_exhaustive]
pub struct AutocompleteInteraction {
    /// The same fields as a command. The focused option is marked in its options.
    pub command: CommandInteraction,
}

impl Bound<AutocompleteInteraction> {
    /// Typed reads of the options typed so far.
    pub fn options(&self) -> CommandOptions<'_> {
        CommandOptions(&self.command)
    }

    /// The name of the option being typed.
    pub fn focused(&self) -> Option<&str> {
        CommandOptions(&self.command).focused()
    }

    /// The answer: up to 25 suggestions. Any past that are left out.
    pub fn respond(&self, choices: impl IntoIterator<Item = AutocompleteChoice>) -> ReplyRequest {
        let target = ReplyTarget {
            client: self.client().clone(),
            id: self.command.id,
            application_id: self.command.application_id,
            token: self.command.token.clone(),
            state: self.command.reply_state.clone(),
        };
        ReplyRequest::new(target, ReplyBody::Autocomplete(choices.into_iter().collect()))
    }
}
