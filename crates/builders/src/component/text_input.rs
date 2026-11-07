//! `TextInputBuilder`, for modals.

use crate::check_len;
use dr_core::ValidationError;
use dr_model::{TextInput, TextInputStyle};

/// Builds a text input.
#[derive(Clone, Debug)]
pub struct TextInputBuilder {
    input: TextInput,
}

impl TextInputBuilder {
    /// A text input of the given style.
    pub fn new(custom_id: impl Into<String>, label: impl Into<String>, style: TextInputStyle) -> Self {
        Self { input: TextInput::new(custom_id, style, label) }
    }
    /// Sets whether it must be filled in.
    pub fn required(mut self, on: bool) -> Self {
        self.input.required = Some(on);
        self
    }
    /// Sets the minimum length.
    pub fn min_length(mut self, n: u16) -> Self {
        self.input.min_length = Some(n);
        self
    }
    /// Sets the maximum length.
    pub fn max_length(mut self, n: u16) -> Self {
        self.input.max_length = Some(n);
        self
    }
    /// Sets the placeholder text.
    pub fn placeholder(mut self, s: impl Into<String>) -> Self {
        self.input.placeholder = Some(s.into());
        self
    }
    /// Sets the prefilled text.
    pub fn value(mut self, s: impl Into<String>) -> Self {
        self.input.value = Some(s.into());
        self
    }
    /// Checks `[types 4.3]`: `custom_id` and label lengths, length ranges.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_input(&self.input)
    }
    /// The wire form.
    pub fn build(self) -> TextInput {
        self.input
    }
}

pub(super) fn check_input(input: &TextInput) -> Result<(), ValidationError> {
    check_len("custom_id", &input.custom_id, 1, 100)?;
    if let Some(label) = &input.label {
        check_len("label", label, 1, 45)?;
    }
    if input.min_length.is_some_and(|n| n > 4000) {
        return Err(ValidationError::new("min_length", "must be at most 4000"));
    }
    if input.max_length.is_some_and(|n| n == 0 || n > 4000) {
        return Err(ValidationError::new("max_length", "must be 1 to 4000"));
    }
    if let Some(placeholder) = &input.placeholder {
        check_len("placeholder", placeholder, 0, 100)?;
    }
    if let Some(value) = &input.value {
        check_len("value", value, 0, 4000)?;
    }
    Ok(())
}
