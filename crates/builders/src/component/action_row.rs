//! `ActionRowBuilder`.
//!
//! `Serialize` writes `{"type":1,"components":[...]}`, `validate` checks the
//! row, and the conversion into `Component` stays.

use crate::{ButtonBuilder, StringSelectBuilder, TextInputBuilder};
use dr_core::ValidationError;
use dr_model::{ActionRow, Button, ButtonStyle, Component};
use serde::{Serialize, Serializer};

/// Builds one row of components: up to 5 buttons, or one select, or one text input in a modal.
#[derive(Clone, Debug, Default)]
pub struct ActionRowBuilder {
    components: Vec<Component>,
}

impl ActionRowBuilder {
    /// An empty row.
    pub fn new() -> Self {
        Self::default()
    }
    fn styled(mut self, style: ButtonStyle, id: String, label: String) -> Self {
        let mut b = Button::new(style);
        b.custom_id = Some(id);
        b.label = Some(label);
        self.components.push(Component::Button(b));
        self
    }
    /// Adds a primary button.
    pub fn primary(self, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        self.styled(ButtonStyle::PRIMARY, custom_id.into(), label.into())
    }
    /// Adds a secondary button.
    pub fn secondary(self, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        self.styled(ButtonStyle::SECONDARY, custom_id.into(), label.into())
    }
    /// Adds a success button.
    pub fn success(self, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        self.styled(ButtonStyle::SUCCESS, custom_id.into(), label.into())
    }
    /// Adds a danger button.
    pub fn danger(self, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        self.styled(ButtonStyle::DANGER, custom_id.into(), label.into())
    }
    /// Adds a link button.
    pub fn link(mut self, url: impl Into<String>, label: impl Into<String>) -> Self {
        let mut b = Button::new(ButtonStyle::LINK);
        b.url = Some(url.into());
        b.label = Some(label.into());
        self.components.push(Component::Button(b));
        self
    }
    /// Adds a button built separately, for options the shortcuts do not cover.
    pub fn button(mut self, button: ButtonBuilder) -> Self {
        self.components.push(Component::Button(button.build()));
        self
    }
    /// Adds a string select.
    pub fn string_select(mut self, select: StringSelectBuilder) -> Self {
        self.components.push(Component::StringSelect(select.build()));
        self
    }
    /// Adds a text input. Text inputs belong in modals, see `ModalBuilder::text_input`.
    pub fn text_input(mut self, input: TextInputBuilder) -> Self {
        self.components.push(Component::TextInput(input.build()));
        self
    }
    /// Checks the row against `[types 4.3]`: 1 to 5 components, no mix of a select with buttons,
    /// and each child. The path in an error is relative to this row, and the caller prefixes it.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let n = self.components.len();
        if n == 0 || n > 5 {
            return Err(ValidationError::new("components", format!("a row holds 1 to 5 components, got {n}")));
        }
        let buttons = self.components.iter().filter(|c| matches!(c, Component::Button(_))).count();
        if buttons != n && n > 1 {
            return Err(ValidationError::new("components", "a select or text input needs a row of its own"));
        }
        for (i, c) in self.components.iter().enumerate() {
            let result = match c {
                Component::Button(b) => super::button::check_button(b),
                Component::StringSelect(s) => super::select::check_select(s),
                Component::TextInput(t) => super::text_input::check_input(t),
                _ => Ok(()),
            };
            result.map_err(|e| crate::nested(format!("components[{i}]"), e))?;
        }
        Ok(())
    }
}

impl From<ActionRowBuilder> for Component {
    fn from(b: ActionRowBuilder) -> Self {
        Component::ActionRow(ActionRow::new(b.components))
    }
}

impl Serialize for ActionRowBuilder {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Component::from(self.clone()).serialize(s)
    }
}
