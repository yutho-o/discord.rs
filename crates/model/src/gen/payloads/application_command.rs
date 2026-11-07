//! Application commands as Discord returns them. The send side is `dr_builders`. Upstream:
//! `payloads/v10/_interactions/applicationCommands.ts` and `_applicationCommands/_chatInput/*.ts`.
//!
//! The upstream option union is keyed on `type` and has one arm per option kind. It is one flat
//! struct here, so an unknown option kind decodes like a known one and fields a kind does not have
//! are `None` or empty. Command permissions and interaction data options are not here: the first
//! has no endpoint in the proof of concept, and the second is `interaction_options`.

use crate::{
    ApplicationCommandOptionType, ApplicationCommandType, ApplicationId, ChannelType, CommandId, GenericId, GuildId,
    Permissions,
};
use dr_core::{null_default, open_int};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

open_int! {
    /// Where an application can be installed.
    ApplicationIntegrationType: u16 {
        /// Installable to servers.
        GUILD_INSTALL = 0,
        /// Installable to users.
        USER_INSTALL = 1,
    }
}

open_int! {
    /// Where an interaction can be used.
    InteractionContextType: u16 {
        /// Within servers.
        GUILD = 0,
        /// Within DMs with the app's bot user.
        BOT_DM = 1,
        /// Within group DMs and DMs other than the app's bot user.
        PRIVATE_CHANNEL = 2,
    }
}

open_int! {
    /// Who handles the interaction of a primary entry point command.
    EntryPointCommandHandlerType: u16 {
        /// The app, using an interaction token.
        APP_HANDLER = 1,
        /// Discord, by launching an Activity and sending a follow-up message.
        DISCORD_LAUNCH_ACTIVITY = 2,
    }
}

/// Translations by locale. Keys are locale codes such as `fr`, kept as text so that a new locale
/// decodes, and a value may be `null`.
pub type LocalizationMap = BTreeMap<String, Option<String>>;

/// A registered application command.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ApplicationCommand {
    /// Command id.
    pub id: CommandId,
    /// Owning application.
    pub application_id: ApplicationId,
    /// Guild, for a guild command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Command kind. Absent means chat input.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "type")]
    pub kind: Option<ApplicationCommandType>,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Translations of the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<LocalizationMap>,
    /// The name in the requested locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localized: Option<String>,
    /// Description. Empty on context menu commands.
    #[serde(default)]
    pub description: String,
    /// Translations of the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<LocalizationMap>,
    /// The description in the requested locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localized: Option<String>,
    /// Options.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ApplicationCommandOption>,
    /// Null means everyone by default, zero means admins only. They must not collapse, so this is
    /// always written, and `None` is never read as an empty set.
    #[serde(default)]
    pub default_member_permissions: Option<Permissions>,
    /// Whether the command is available in DMs. Absent means yes. Deprecated by Discord for
    /// [`contexts`](Self::contexts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_permission: Option<bool>,
    /// Whether the command is on when the app joins a guild. Absent means yes. Deprecated by Discord
    /// for `dm_permission` and `default_member_permissions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_permission: Option<bool>,
    /// Whether the command is age restricted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nsfw: bool,
    /// Where the command can be installed, global commands only. Absent means guild install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_types: Option<Vec<ApplicationIntegrationType>>,
    /// Where the command can be used, global commands only. Absent or null means everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexts: Option<Vec<InteractionContextType>>,
    /// Version, changing on each edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<GenericId>,
    /// Who handles the interaction. Primary entry point commands only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<EntryPointCommandHandlerType>,
}

impl ApplicationCommand {
    /// The kind, reading an absent one as chat input, as Discord documents.
    pub fn kind_or_default(&self) -> ApplicationCommandType {
        self.kind.unwrap_or(ApplicationCommandType::CHAT_INPUT)
    }

    /// Creation time from the snowflake, in milliseconds since the Unix epoch.
    #[doc(alias = "createdTimestamp")]
    pub const fn created_at_ms(&self) -> i64 {
        self.id.timestamp_ms()
    }
}

/// One option of a registered command.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ApplicationCommandOption {
    /// Option kind. An unknown one is kept.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ApplicationCommandOptionType,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Translations of the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<LocalizationMap>,
    /// The name in the requested locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localized: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Translations of the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<LocalizationMap>,
    /// The description in the requested locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localized: Option<String>,
    /// Whether it must be given.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    /// Nested options of a subcommand or group.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ApplicationCommandOption>,
    /// Channel kinds a channel option accepts.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub channel_types: Vec<ChannelType>,
    /// Fixed choices of a string, integer or number option.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ApplicationCommandOptionChoice>,
    /// Whether a string, integer or number option asks the app for suggestions.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub autocomplete: bool,
    /// Smallest value of an integer or number option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_value: Option<ApplicationCommandOptionBound>,
    /// Largest value of an integer or number option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_value: Option<ApplicationCommandOptionBound>,
    /// Shortest value of a string option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<i64>,
    /// Longest value of a string option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<i64>,
    /// File kinds an attachment option accepts: `audio`, `image`, `video`, or `.ext`.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub file_types: Vec<String>,
}

/// A fixed choice of a command option.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ApplicationCommandOptionChoice {
    /// Name shown to the user.
    #[serde(default)]
    pub name: String,
    /// Translations of the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<LocalizationMap>,
    /// The name in the requested locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localized: Option<String>,
    /// Value sent to the app.
    pub value: ApplicationCommandOptionChoiceValue,
}

/// The value of a choice, whose form follows the option kind. Upstream types it `number | string`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ApplicationCommandOptionChoiceValue {
    /// Whole number, on an integer option and on a number option with a whole value.
    Integer(i64),
    /// Fractional number.
    Number(f64),
    /// Text.
    String(String),
}

/// A `min_value` or `max_value`. TypeScript has only `number`, and a number option may carry a
/// fraction, so a plain `i64` would fail the whole command. The form received is kept.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ApplicationCommandOptionBound {
    /// Whole number.
    Integer(i64),
    /// Fractional number.
    Number(f64),
}

impl ApplicationCommandOptionBound {
    /// The bound as a float. Discord limits both kinds to 2^53 in magnitude, which this keeps exact.
    #[allow(clippy::cast_precision_loss)]
    pub const fn as_f64(self) -> f64 {
        match self {
            Self::Integer(v) => v as f64,
            Self::Number(v) => v,
        }
    }
}
