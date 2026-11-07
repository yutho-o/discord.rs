//! Reading a command's options.

use super::CommandInteraction;
use crate::error::InteractionError;
use dr_model::{ChannelId, HoistedOptions, RoleId, UserId};

/// Typed reads of a command's options. `foo` gives an `Option`, and `require_foo` an error naming the
/// option. Inside a subcommand, these read that subcommand's options.
#[derive(Clone, Copy, Debug)]
pub struct CommandOptions<'a>(pub(crate) &'a CommandInteraction);

impl<'a> CommandOptions<'a> {
    fn hoisted(&self) -> HoistedOptions<'a> {
        HoistedOptions::new(&self.0.options, self.0.resolved.as_ref())
    }

    /// A string option.
    pub fn string(&self, name: &str) -> Option<&'a str> {
        self.hoisted().string(name)
    }

    /// An integer option.
    pub fn integer(&self, name: &str) -> Option<i64> {
        self.hoisted().integer(name)
    }

    /// A boolean option.
    pub fn boolean(&self, name: &str) -> Option<bool> {
        self.hoisted().boolean(name)
    }

    /// A user option, as its id.
    pub fn user(&self, name: &str) -> Option<UserId> {
        self.hoisted().user(name)
    }

    /// A channel option, as its id.
    pub fn channel(&self, name: &str) -> Option<ChannelId> {
        self.hoisted().channel(name)
    }

    /// A role option, as its id.
    pub fn role(&self, name: &str) -> Option<RoleId> {
        self.hoisted().role(name)
    }

    /// The name of the subcommand that was used.
    pub fn subcommand(&self) -> Option<&'a str> {
        self.hoisted().subcommand
    }

    /// The name of the option being typed, for autocomplete.
    pub(crate) fn focused(&self) -> Option<&'a str> {
        self.hoisted().focused().map(|o| o.name.as_str())
    }

    /// A required string option.
    pub fn require_string(&self, name: &str) -> Result<&'a str, InteractionError> {
        self.string(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }

    /// A required integer option.
    pub fn require_integer(&self, name: &str) -> Result<i64, InteractionError> {
        self.integer(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }

    /// A required boolean option.
    pub fn require_boolean(&self, name: &str) -> Result<bool, InteractionError> {
        self.boolean(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }

    /// A required user option.
    pub fn require_user(&self, name: &str) -> Result<UserId, InteractionError> {
        self.user(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }

    /// A required channel option.
    pub fn require_channel(&self, name: &str) -> Result<ChannelId, InteractionError> {
        self.channel(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }

    /// A required role option.
    pub fn require_role(&self, name: &str) -> Result<RoleId, InteractionError> {
        self.role(name).ok_or_else(|| InteractionError::Option(name.to_owned()))
    }
}
