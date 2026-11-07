//! Option builders. Each is handed to a closure: `.string_option("x", "y", |o| o.required(true))`.

use crate::{check_len, nested};
use dr_core::ValidationError;
use dr_model::{ApplicationCommandOptionType, ChannelType};
use serde::Serialize;

/// The wire form of one option.
#[derive(Clone, Debug, Serialize)]
pub struct CommandOption {
    #[serde(rename = "type")]
    kind: ApplicationCommandOptionType,
    name: String,
    description: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_length: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_value: Option<i64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    autocomplete: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    channel_types: Vec<ChannelType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<CommandOption>,
}

impl CommandOption {
    pub(crate) fn new(kind: ApplicationCommandOptionType, name: String, description: String) -> Self {
        Self {
            kind,
            name,
            description,
            required: false,
            min_length: None,
            max_length: None,
            min_value: None,
            max_value: None,
            autocomplete: false,
            channel_types: Vec::new(),
            options: Vec::new(),
        }
    }
    pub(crate) fn with_options(mut self, options: Vec<CommandOption>) -> Self {
        self.options = options;
        self
    }
    /// Checks `[types 4.3]`: lowercase name of 1 to 32 code points, description of 1 to 100, ranges,
    /// required options before optional ones among `options`.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_command_name("name", &self.name)?;
        check_len("description", &self.description, 1, 100)?;
        if let (Some(a), Some(b)) = (self.min_length, self.max_length)
            && a > b
        {
            return Err(ValidationError::new("min_length", "must not be over max_length"));
        }
        if self.min_length.is_some_and(|n| n > 6000) {
            return Err(ValidationError::new("min_length", "must be at most 6000"));
        }
        if self.max_length.is_some_and(|n| n == 0 || n > 6000) {
            return Err(ValidationError::new("max_length", "must be 1 to 6000"));
        }
        if let (Some(a), Some(b)) = (self.min_value, self.max_value)
            && a > b
        {
            return Err(ValidationError::new("min_value", "must not be over max_value"));
        }
        check_option_list(&self.options)
    }
}

macro_rules! option_builder {
    ($(#[$doc:meta])* $name:ident, $kind:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug)]
        pub struct $name(CommandOption);
        impl $name {
            pub(crate) fn start(name: String, description: String) -> Self {
                Self(CommandOption::new(ApplicationCommandOptionType::$kind, name, description))
            }
            pub(crate) fn finish(self) -> CommandOption {
                self.0
            }
            /// Sets whether the option must be given.
            pub fn required(mut self, on: bool) -> Self {
                self.0.required = on;
                self
            }
        }
    };
}

option_builder!(
    /// A string option.
    StringOption, STRING
);
option_builder!(
    /// An integer option.
    IntegerOption, INTEGER
);
option_builder!(
    /// A boolean option.
    BooleanOption, BOOLEAN
);
option_builder!(
    /// A user option.
    UserOption, USER
);
option_builder!(
    /// A channel option.
    ChannelOption, CHANNEL
);
option_builder!(
    /// A role option.
    RoleOption, ROLE
);

impl StringOption {
    /// Sets the minimum length.
    pub fn min_length(mut self, n: u16) -> Self {
        self.0.min_length = Some(n);
        self
    }
    /// Sets the maximum length.
    pub fn max_length(mut self, n: u16) -> Self {
        self.0.max_length = Some(n);
        self
    }
    /// Sets whether the option uses autocomplete.
    pub fn autocomplete(mut self, on: bool) -> Self {
        self.0.autocomplete = on;
        self
    }
}

impl IntegerOption {
    /// Sets the minimum value.
    pub fn min_value(mut self, n: i64) -> Self {
        self.0.min_value = Some(n);
        self
    }
    /// Sets the maximum value.
    pub fn max_value(mut self, n: i64) -> Self {
        self.0.max_value = Some(n);
        self
    }
    /// Sets whether the option uses autocomplete.
    pub fn autocomplete(mut self, on: bool) -> Self {
        self.0.autocomplete = on;
        self
    }
}

impl ChannelOption {
    /// Limits the channel kinds accepted.
    pub fn channel_types(mut self, kinds: impl IntoIterator<Item = ChannelType>) -> Self {
        self.0.channel_types = kinds.into_iter().collect();
        self
    }
}

/// A command or option name: 1 to 32 code points, lower case, with no space or punctuation other
/// than `-` and `_`. Discord allows letters of any script, which is what `is_alphanumeric` covers.
pub(crate) fn check_command_name(path: &str, name: &str) -> Result<(), ValidationError> {
    check_len(path, name, 1, 32)?;
    let bad = |c: char| !(c.is_alphanumeric() || c == '-' || c == '_') || c.is_uppercase();
    if name.chars().any(bad) {
        return Err(ValidationError::new(path, "must be lower case letters, digits, - or _"));
    }
    Ok(())
}

/// At most 25 options, required ones first, and each option itself.
pub(crate) fn check_option_list(options: &[CommandOption]) -> Result<(), ValidationError> {
    if options.len() > 25 {
        return Err(ValidationError::new("options", "must hold at most 25 options"));
    }
    let mut seen_optional = false;
    for (i, o) in options.iter().enumerate() {
        o.validate().map_err(|e| nested(format!("options[{i}]"), e))?;
        if o.required && seen_optional {
            return Err(ValidationError::new(format!("options[{i}]"), "required options must come first"));
        }
        seen_optional |= !o.required;
    }
    Ok(())
}
