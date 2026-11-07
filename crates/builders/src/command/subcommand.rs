//! `Subcommand`.

use super::options::{
    BooleanOption, ChannelOption, CommandOption, IntegerOption, RoleOption, StringOption, UserOption,
};
use dr_model::ApplicationCommandOptionType;

/// A subcommand of a slash command. It holds options only.
#[derive(Clone, Debug)]
pub struct Subcommand {
    name: String,
    description: String,
    options: Vec<CommandOption>,
}

macro_rules! subcommand_option {
    ($method:ident, $ty:ident) => {
        #[doc = concat!("Adds an option, configured through a [`", stringify!($ty), "`] closure.")]
        pub fn $method(
            mut self,
            name: impl Into<String>,
            description: impl Into<String>,
            f: impl FnOnce($ty) -> $ty,
        ) -> Self {
            self.options.push(f($ty::start(name.into(), description.into())).finish());
            self
        }
    };
}

impl Subcommand {
    /// A subcommand with the given name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self { name: name.into(), description: description.into(), options: Vec::new() }
    }
    subcommand_option!(string_option, StringOption);
    subcommand_option!(integer_option, IntegerOption);
    subcommand_option!(boolean_option, BooleanOption);
    subcommand_option!(user_option, UserOption);
    subcommand_option!(channel_option, ChannelOption);
    subcommand_option!(role_option, RoleOption);

    pub(crate) fn finish(self) -> CommandOption {
        CommandOption::new(ApplicationCommandOptionType::SUB_COMMAND, self.name, self.description)
            .with_options(self.options)
    }
}
