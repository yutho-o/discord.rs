//! `StringSelectBuilder`.

use crate::check_len;
use dr_core::ValidationError;
use dr_model::{SelectOption, StringSelect};

/// Builds a string select menu.
#[derive(Clone, Debug)]
pub struct StringSelectBuilder {
    select: StringSelect,
}

/// One choice in a string select.
#[derive(Clone, Debug)]
pub struct StringSelectOption {
    option: SelectOption,
}

impl StringSelectBuilder {
    /// A select with the given id and no options yet.
    pub fn new(custom_id: impl Into<String>) -> Self {
        Self { select: StringSelect::new(custom_id) }
    }
    /// Sets the text shown when nothing is selected.
    pub fn placeholder(mut self, s: impl Into<String>) -> Self {
        self.select.placeholder = Some(s.into());
        self
    }
    /// Sets the minimum number of choices.
    pub fn min_values(mut self, n: u8) -> Self {
        self.select.min_values = Some(n);
        self
    }
    /// Sets the maximum number of choices.
    pub fn max_values(mut self, n: u8) -> Self {
        self.select.max_values = Some(n);
        self
    }
    /// Adds a choice.
    pub fn option(mut self, o: StringSelectOption) -> Self {
        self.select.options.push(o.option);
        self
    }
    /// Sets whether it is greyed out.
    pub fn disabled(mut self, on: bool) -> Self {
        self.select.disabled = on;
        self
    }
    /// Checks `[types 4.3]`: 1 to 25 options, `min_values` and `max_values` ranges, lengths.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_select(&self.select)
    }
    /// The wire form.
    pub fn build(self) -> StringSelect {
        self.select
    }
}

impl StringSelectOption {
    /// A choice with the given label and value.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { option: SelectOption::new(label, value) }
    }
    /// Sets the description.
    pub fn description(mut self, s: impl Into<String>) -> Self {
        self.option.description = Some(s.into());
        self
    }
    /// Sets whether it starts selected.
    pub fn default_selected(mut self, on: bool) -> Self {
        self.option.default = on;
        self
    }
}

pub(super) fn check_select(select: &StringSelect) -> Result<(), ValidationError> {
    check_len("custom_id", &select.custom_id, 1, 100)?;
    let n = select.options.len();
    if n == 0 || n > 25 {
        return Err(ValidationError::new("options", format!("a select holds 1 to 25 options, got {n}")));
    }
    if select.min_values.is_some_and(|v| v > 25) {
        return Err(ValidationError::new("min_values", "must be at most 25"));
    }
    if select.max_values.is_some_and(|v| v == 0 || v > 25) {
        return Err(ValidationError::new("max_values", "must be 1 to 25"));
    }
    if let Some(placeholder) = &select.placeholder {
        check_len("placeholder", placeholder, 0, 150)?;
    }
    for (i, o) in select.options.iter().enumerate() {
        check_len(&format!("options[{i}].label"), &o.label, 1, 100)?;
        check_len(&format!("options[{i}].value"), &o.value, 1, 100)?;
        if let Some(d) = &o.description {
            check_len(&format!("options[{i}].description"), d, 1, 100)?;
        }
    }
    Ok(())
}
