//! `ButtonBuilder`, for buttons the `ActionRowBuilder` shortcuts do not cover.

use crate::check_len;
use dr_core::ValidationError;
use dr_model::{Button, ButtonStyle};

/// Builds one button.
#[derive(Clone, Debug)]
pub struct ButtonBuilder {
    button: Button,
}

impl ButtonBuilder {
    /// A button of the given style.
    pub fn new(style: ButtonStyle) -> Self {
        Self { button: Button::new(style) }
    }
    /// Sets the text.
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.button.label = Some(s.into());
        self
    }
    /// Sets the id sent back on a click.
    pub fn custom_id(mut self, s: impl Into<String>) -> Self {
        self.button.custom_id = Some(s.into());
        self
    }
    /// Sets the target of a link button.
    pub fn url(mut self, s: impl Into<String>) -> Self {
        self.button.url = Some(s.into());
        self
    }
    /// Sets whether it is greyed out.
    pub fn disabled(mut self, on: bool) -> Self {
        self.button.disabled = on;
        self
    }
    /// Checks `[types 4.3]`: `custom_id` xor `url` by style, label length, `custom_id` length.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_button(&self.button)
    }
    /// The wire form.
    pub fn build(self) -> Button {
        self.button
    }
}

pub(super) fn check_button(b: &Button) -> Result<(), ValidationError> {
    if let Some(label) = &b.label {
        check_len("label", label, 1, 80)?;
    }
    let is_link = b.style == ButtonStyle::LINK;
    let is_premium = b.style == ButtonStyle::PREMIUM;
    match (&b.custom_id, &b.url) {
        (Some(id), None) if !is_link && !is_premium => check_len("custom_id", id, 1, 100)?,
        (None, Some(url)) if is_link => check_len("url", url, 1, 512)?,
        (None, None) if is_premium => {}
        _ => return Err(ValidationError::new("", "a button needs a custom_id, or a url for the link style")),
    }
    if !is_premium && b.label.is_none() && b.emoji.is_none() {
        return Err(ValidationError::new("label", "a button needs a label or an emoji"));
    }
    Ok(())
}
