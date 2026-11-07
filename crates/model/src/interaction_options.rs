//! The option tree of a command interaction, and reading it the way upstream's resolver does.
//!
//! Wire shapes: `payloads/v10/_interactions/_applicationCommands/_chatInput/*.ts` of
//! `discord-api-types` (MIT). Reading rules: `structures/CommandInteractionOptionResolver.js` of
//! `discord.js` (Apache-2.0), `[client 12.4]`.
//!
//! Upstream's option is a union on `type`. Here it is one struct with an [`OptionValue`] parsed by
//! `type` once the whole object is read, so that the order of the keys does not matter. An option
//! kind this build does not know, or a value of an unexpected shape, is kept raw instead of losing
//! the interaction.

use crate::{ApplicationCommandOptionType, AttachmentId, ChannelId, GenericId, ResolvedData, RoleId, UserId};
use dr_core::{lenient_i64, null_default};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::value::RawValue;

/// One option as received in an application command or autocomplete interaction.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "OptionWire")]
pub struct InteractionOption {
    /// Option name.
    pub name: String,
    /// Option kind. Unknown kinds are kept.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ApplicationCommandOptionType,
    /// The value. Absent on a subcommand and a subcommand group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<OptionValue>,
    /// Nested options of a subcommand or subcommand group.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<InteractionOption>,
    /// True on the option being typed, for autocomplete.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub focused: bool,
}

/// The value of an option, read according to the option's kind.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum OptionValue {
    /// A `STRING` option.
    String(String),
    /// An `INTEGER` option. A value sent as `1.0` is accepted.
    Integer(i64),
    /// A `NUMBER` option.
    Number(f64),
    /// A `BOOLEAN` option.
    Boolean(bool),
    /// A `USER`, `CHANNEL`, `ROLE`, `MENTIONABLE` or `ATTACHMENT` option: the id of the target, which
    /// [`ResolvedData`] describes.
    Snowflake(GenericId),
    /// An `INTEGER` or `NUMBER` option during autocomplete: the raw text typed so far, which may be
    /// empty or not a number at all. Upstream types this as
    /// `Type extends ApplicationCommandAutocomplete ? string : number`.
    Partial(String),
    /// An option kind this build does not know, or a value of an unexpected shape. Re-serializes
    /// byte-exact.
    Unknown(Box<RawValue>),
}

impl Serialize for OptionValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::String(v) | Self::Partial(v) => s.serialize_str(v),
            Self::Integer(v) => s.serialize_i64(*v),
            Self::Number(v) => s.serialize_f64(*v),
            Self::Boolean(v) => s.serialize_bool(*v),
            Self::Snowflake(v) => v.serialize(s),
            Self::Unknown(v) => v.serialize(s),
        }
    }
}

impl OptionValue {
    /// Reads `raw` as the kind says. Never fails: whatever does not fit stays [`Self::Unknown`].
    fn parse(kind: ApplicationCommandOptionType, raw: Box<RawValue>) -> Self {
        use ApplicationCommandOptionType as T;
        let text = raw.get();
        let parsed = match kind {
            T::STRING => serde_json::from_str(text).ok().map(Self::String),
            T::INTEGER => lenient_i64(&mut serde_json::Deserializer::from_str(text))
                .ok()
                .map(Self::Integer)
                .or_else(|| Self::partial(text)),
            T::NUMBER => serde_json::from_str::<f64>(text).ok().map(Self::Number).or_else(|| Self::partial(text)),
            T::BOOLEAN => serde_json::from_str(text).ok().map(Self::Boolean),
            T::USER | T::CHANNEL | T::ROLE | T::MENTIONABLE | T::ATTACHMENT => {
                serde_json::from_str(text).ok().map(Self::Snowflake)
            }
            _ => None,
        };
        parsed.unwrap_or(Self::Unknown(raw))
    }

    fn partial(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok().map(Self::Partial)
    }
}

/// The receive form: `value` stays raw until `type` is known, wherever it sits in the object.
#[derive(Deserialize)]
struct OptionWire {
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    kind: ApplicationCommandOptionType,
    #[serde(default)]
    value: Option<Box<RawValue>>,
    #[serde(default, deserialize_with = "null_default")]
    options: Vec<InteractionOption>,
    #[serde(default)]
    focused: bool,
}

impl From<OptionWire> for InteractionOption {
    fn from(w: OptionWire) -> Self {
        Self {
            name: w.name,
            kind: w.kind,
            value: w.value.map(|raw| OptionValue::parse(w.kind, raw)),
            options: w.options,
            focused: w.focused,
        }
    }
}

impl InteractionOption {
    /// The text of a `STRING` option.
    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Some(OptionValue::String(v)) => Some(v),
            _ => None,
        }
    }

    /// The value of an `INTEGER` option.
    pub fn as_i64(&self) -> Option<i64> {
        match self.value {
            Some(OptionValue::Integer(v)) => Some(v),
            _ => None,
        }
    }

    /// The value of a `NUMBER` option.
    pub fn as_f64(&self) -> Option<f64> {
        match self.value {
            Some(OptionValue::Number(v)) => Some(v),
            _ => None,
        }
    }

    /// The value of a `BOOLEAN` option.
    pub fn as_bool(&self) -> Option<bool> {
        match self.value {
            Some(OptionValue::Boolean(v)) => Some(v),
            _ => None,
        }
    }

    /// The target of a `USER`, `CHANNEL`, `ROLE`, `MENTIONABLE` or `ATTACHMENT` option.
    pub fn as_snowflake(&self) -> Option<GenericId> {
        match self.value {
            Some(OptionValue::Snowflake(v)) => Some(v),
            _ => None,
        }
    }

    /// The text typed so far during autocomplete: a `STRING`, or an `INTEGER` or `NUMBER` that arrived
    /// as text.
    pub fn typed_text(&self) -> Option<&str> {
        match &self.value {
            Some(OptionValue::String(v) | OptionValue::Partial(v)) => Some(v),
            _ => None,
        }
    }
}

/// The options that matter once a leading subcommand group and subcommand are taken off, the way
/// upstream's `CommandInteractionOptionResolver` hoists them.
///
/// A getter gives `None` for an option that is absent or of another kind, where upstream throws on
/// the second. The caller turns that into an error, as `options().require_string(n)` does in the
/// facade.
#[derive(Clone, Copy, Debug)]
#[doc(alias = "CommandInteractionOptionResolver")]
pub struct HoistedOptions<'a> {
    /// Name of the subcommand group that was used.
    pub group: Option<&'a str>,
    /// Name of the subcommand that was used.
    pub subcommand: Option<&'a str>,
    /// The bottom-level options.
    pub options: &'a [InteractionOption],
    /// Resolved data of the interaction, which tells what a mentionable names.
    pub resolved: Option<&'a ResolvedData>,
}

impl<'a> HoistedOptions<'a> {
    /// Hoists a leading subcommand group, then a leading subcommand. Only the first option can be
    /// either, as upstream checks.
    pub fn new(options: &'a [InteractionOption], resolved: Option<&'a ResolvedData>) -> Self {
        let mut hoisted = Self { group: None, subcommand: None, options, resolved };
        if let Some(first) = hoisted.options.first()
            && first.kind == ApplicationCommandOptionType::SUBCOMMAND_GROUP
        {
            hoisted.group = Some(&first.name);
            hoisted.options = &first.options;
        }
        if let Some(first) = hoisted.options.first()
            && first.kind == ApplicationCommandOptionType::SUBCOMMAND
        {
            hoisted.subcommand = Some(&first.name);
            hoisted.options = &first.options;
        }
        hoisted
    }

    /// An option of any kind, by name.
    pub fn get(&self, name: &str) -> Option<&'a InteractionOption> {
        self.options.iter().find(|o| o.name == name)
    }

    /// The option being typed, for autocomplete.
    #[doc(alias = "getFocused")]
    pub fn focused(&self) -> Option<&'a InteractionOption> {
        self.options.iter().find(|o| o.focused)
    }

    fn typed(&self, name: &str, kind: ApplicationCommandOptionType) -> Option<&'a InteractionOption> {
        self.get(name).filter(|o| o.kind == kind)
    }

    /// A `STRING` option.
    #[doc(alias = "getString")]
    pub fn string(&self, name: &str) -> Option<&'a str> {
        self.typed(name, ApplicationCommandOptionType::STRING)?.as_str()
    }

    /// An `INTEGER` option.
    #[doc(alias = "getInteger")]
    pub fn integer(&self, name: &str) -> Option<i64> {
        self.typed(name, ApplicationCommandOptionType::INTEGER)?.as_i64()
    }

    /// A `NUMBER` option.
    #[doc(alias = "getNumber")]
    pub fn number(&self, name: &str) -> Option<f64> {
        self.typed(name, ApplicationCommandOptionType::NUMBER)?.as_f64()
    }

    /// A `BOOLEAN` option.
    #[doc(alias = "getBoolean")]
    pub fn boolean(&self, name: &str) -> Option<bool> {
        self.typed(name, ApplicationCommandOptionType::BOOLEAN)?.as_bool()
    }

    /// The id from a `USER` option, or from a `MENTIONABLE` option that names a user according to the
    /// resolved data. A mentionable naming a role gives `None`, as upstream's `getUser` does.
    #[doc(alias = "getUser")]
    pub fn user(&self, name: &str) -> Option<UserId> {
        let option = self.get(name)?;
        let id = option.as_snowflake()?.cast();
        match option.kind {
            ApplicationCommandOptionType::USER => Some(id),
            ApplicationCommandOptionType::MENTIONABLE => self.resolved?.users.contains_key(&id).then_some(id),
            _ => None,
        }
    }

    /// The id from a `ROLE` option, or from a `MENTIONABLE` option that names a role according to the
    /// resolved data. A mentionable naming a user gives `None`, as upstream's `getRole` does.
    #[doc(alias = "getRole")]
    pub fn role(&self, name: &str) -> Option<RoleId> {
        let option = self.get(name)?;
        let id = option.as_snowflake()?.cast();
        match option.kind {
            ApplicationCommandOptionType::ROLE => Some(id),
            ApplicationCommandOptionType::MENTIONABLE => self.resolved?.roles.contains_key(&id).then_some(id),
            _ => None,
        }
    }

    /// The id from a `CHANNEL` option.
    #[doc(alias = "getChannel")]
    pub fn channel(&self, name: &str) -> Option<ChannelId> {
        Some(self.typed(name, ApplicationCommandOptionType::CHANNEL)?.as_snowflake()?.cast())
    }

    /// The id from an `ATTACHMENT` option.
    #[doc(alias = "getAttachment")]
    pub fn attachment(&self, name: &str) -> Option<AttachmentId> {
        Some(self.typed(name, ApplicationCommandOptionType::ATTACHMENT)?.as_snowflake()?.cast())
    }

    /// The id from a `MENTIONABLE` option, which names a user or a role.
    #[doc(alias = "getMentionable")]
    pub fn mentionable(&self, name: &str) -> Option<GenericId> {
        self.typed(name, ApplicationCommandOptionType::MENTIONABLE)?.as_snowflake()
    }
}
