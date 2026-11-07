//! `SlashCommandBuilder` with its type-state, and the erased `CommandData`.

use super::options::{
    BooleanOption, ChannelOption, CommandOption, IntegerOption, RoleOption, StringOption, UserOption,
};
use super::options::{check_command_name, check_option_list};
use super::subcommand::Subcommand;
use crate::check_len;
use dr_core::{BodyError, IntoBody, Patch, RequestBody, ValidationError};
use dr_model::{ApplicationCommandType, Permissions};
use serde::Serialize;
use std::marker::PhantomData;

/// Type-state: neither options nor subcommands were added yet.
#[derive(Debug)]
pub struct Unset;
/// Type-state: the command has options, so it cannot take subcommands.
#[derive(Debug)]
pub struct Leaf;
/// Type-state: the command has subcommands, so it cannot take options.
#[derive(Debug)]
pub struct Parent;

/// The erased wire form of any application command. Every builder converts into it, which is what
/// lets `commands![]` mix shapes.
#[derive(Clone, Debug, Serialize)]
pub struct CommandData {
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(rename = "type")]
    kind: ApplicationCommandType,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<CommandOption>,
    /// Null means everyone by default and zero means admins only, so this must not collapse.
    #[serde(skip_serializing_if = "Patch::is_absent")]
    default_member_permissions: Patch<Permissions>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    nsfw: bool,
}

impl CommandData {
    pub(crate) fn new(name: String, description: String, kind: ApplicationCommandType) -> Self {
        Self { name, description, kind, options: Vec::new(), default_member_permissions: Patch::Absent, nsfw: false }
    }
    /// Checks `[types 4.3]`: lowercase chat input names, lengths, at most 25 options, required
    /// options before optional ones, and each option.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.kind == ApplicationCommandType::CHAT_INPUT {
            check_command_name("name", &self.name)?;
            check_len("description", &self.description, 1, 100)?;
        } else {
            check_len("name", &self.name, 1, 32)?;
        }
        check_option_list(&self.options)
    }
}

/// A list of commands as one request body, for the bulk overwrite routes. `IntoBody` validates each.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(transparent)]
pub struct CommandList(Vec<CommandData>);

impl FromIterator<CommandData> for CommandList {
    fn from_iter<I: IntoIterator<Item = CommandData>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoBody for CommandData {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

impl IntoBody for CommandList {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        for c in &self.0 {
            c.validate()?;
        }
        RequestBody::json(&self)
    }
}

/// Builds a chat input command. `K` is the type-state, so that adding an option after a subcommand,
/// or a subcommand after an option, does not compile.
#[derive(Clone, Debug)]
pub struct SlashCommandBuilder<K = Unset> {
    data: CommandData,
    state: PhantomData<fn() -> K>,
}

impl SlashCommandBuilder<Unset> {
    /// A command with the given name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            data: CommandData::new(name.into(), description.into(), ApplicationCommandType::CHAT_INPUT),
            state: PhantomData,
        }
    }
}

impl<K> SlashCommandBuilder<K> {
    fn retag<N>(self) -> SlashCommandBuilder<N> {
        SlashCommandBuilder { data: self.data, state: PhantomData }
    }
    /// Sets the permissions a member needs by default. `Permissions::empty()` means admins only.
    pub fn default_member_permissions(mut self, p: Permissions) -> Self {
        self.data.default_member_permissions = Patch::Value(p);
        self
    }
    /// Sets whether the command is age restricted.
    pub fn nsfw(mut self, on: bool) -> Self {
        self.data.nsfw = on;
        self
    }
}

macro_rules! leaf_option {
    ($method:ident, $ty:ident) => {
        #[doc = concat!("Adds an option, configured through a [`", stringify!($ty), "`] closure.")]
        pub fn $method(
            mut self,
            name: impl Into<String>,
            description: impl Into<String>,
            f: impl FnOnce($ty) -> $ty,
        ) -> SlashCommandBuilder<Leaf> {
            self.data.options.push(f($ty::start(name.into(), description.into())).finish());
            self.retag()
        }
    };
}

macro_rules! leaf_methods {
    ($k:ty) => {
        impl SlashCommandBuilder<$k> {
            leaf_option!(string_option, StringOption);
            leaf_option!(integer_option, IntegerOption);
            leaf_option!(boolean_option, BooleanOption);
            leaf_option!(user_option, UserOption);
            leaf_option!(channel_option, ChannelOption);
            leaf_option!(role_option, RoleOption);
        }
    };
}
leaf_methods!(Unset);
leaf_methods!(Leaf);

macro_rules! parent_methods {
    ($k:ty) => {
        impl SlashCommandBuilder<$k> {
            /// Adds a subcommand.
            pub fn subcommand(mut self, s: Subcommand) -> SlashCommandBuilder<Parent> {
                self.data.options.push(s.finish());
                self.retag()
            }
        }
    };
}
parent_methods!(Unset);
parent_methods!(Parent);

impl<K> From<SlashCommandBuilder<K>> for CommandData {
    fn from(b: SlashCommandBuilder<K>) -> Self {
        b.data
    }
}
