//! `ModalBuilder`.

use crate::{ActionRowBuilder, TextInputBuilder, check_len, nested};
use dr_core::ValidationError;
use serde::Serialize;

/// Builds a modal. `Serialize` writes the wire form, which `show_modal` uses as the `data` of an
/// `InteractionResponse`.
#[derive(Clone, Debug, Serialize)]
pub struct ModalBuilder {
    custom_id: String,
    title: String,
    components: Vec<ActionRowBuilder>,
    #[serde(skip)]
    skip_validation: bool,
}

impl ModalBuilder {
    /// A modal with the given id and title, and no inputs yet.
    pub fn new(custom_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { custom_id: custom_id.into(), title: title.into(), components: Vec::new(), skip_validation: false }
    }
    /// Adds a text input, on a row of its own.
    pub fn text_input(mut self, input: TextInputBuilder) -> Self {
        self.components.push(ActionRowBuilder::new().text_input(input));
        self
    }
    /// Turns off validation for this request only.
    pub fn skip_validation(mut self) -> Self {
        self.skip_validation = true;
        self
    }
    /// Checks `[types 4.3]`: `custom_id` and title lengths, 1 to 5 inputs, each input. It does
    /// nothing after [`Self::skip_validation`].
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.skip_validation {
            return Ok(());
        }
        check_len("custom_id", &self.custom_id, 1, 100)?;
        check_len("title", &self.title, 1, 45)?;
        let n = self.components.len();
        if n == 0 || n > 5 {
            return Err(ValidationError::new("components", format!("a modal holds 1 to 5 inputs, got {n}")));
        }
        for (i, row) in self.components.iter().enumerate() {
            row.validate().map_err(|e| nested(format!("components[{i}]"), e))?;
        }
        Ok(())
    }
}
