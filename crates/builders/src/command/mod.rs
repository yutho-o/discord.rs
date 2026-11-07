//! Application command builders. Options and subcommands cannot mix, enforced by a type-state on
//! `SlashCommandBuilder`, and mixed arrays go through the erased [`CommandData`].

pub mod context_menu;
pub mod options;
pub mod slash;
pub mod subcommand;

pub use context_menu::{MessageContextCommand, UserContextCommand};
pub use options::{BooleanOption, ChannelOption, CommandOption, IntegerOption, RoleOption, StringOption, UserOption};
pub use slash::{CommandData, CommandList, Leaf, Parent, SlashCommandBuilder, Unset};
pub use subcommand::Subcommand;

/// An array of [`CommandData`] from builders of different shapes. The usual argument of
/// `ctx.commands().set_global(...)`.
#[macro_export]
macro_rules! commands {
    ($($c:expr),* $(,)?) => { [$($crate::CommandData::from($c)),*] };
}
