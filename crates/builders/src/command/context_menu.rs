//! Context menu commands. They have a name and no description or options.

use super::slash::CommandData;
use dr_model::ApplicationCommandType;

/// A command on the right-click menu of a user.
#[derive(Clone, Debug)]
pub struct UserContextCommand(CommandData);

impl UserContextCommand {
    /// A user command with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(CommandData::new(name.into(), String::new(), ApplicationCommandType::USER))
    }
}

impl From<UserContextCommand> for CommandData {
    fn from(c: UserContextCommand) -> Self {
        c.0
    }
}

/// A command on the right-click menu of a message.
#[derive(Clone, Debug)]
pub struct MessageContextCommand(CommandData);

impl MessageContextCommand {
    /// A message command with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(CommandData::new(name.into(), String::new(), ApplicationCommandType::MESSAGE))
    }
}

impl From<MessageContextCommand> for CommandData {
    fn from(c: MessageContextCommand) -> Self {
        c.0
    }
}
