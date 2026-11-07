//! `EditRole`, also used to create a role.

use crate::check_len;
use dr_core::{BodyError, IntoBody, RequestBody, ValidationError};
use dr_model::Permissions;
use serde::Serialize;

/// The largest colour, `0xFFFFFF`.
const MAX_COLOR: u32 = 0x00FF_FFFF;

/// The body of a role creation or edit. A field that was not set stays out of the JSON: on an edit
/// it keeps its value, and on a creation Discord picks the default.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditRole {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hoist: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentionable: Option<bool>,
}

/// A role creation has the same fields as an edit.
pub type CreateRole = EditRole;

impl EditRole {
    /// A body that sets nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the name, at most 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the permissions.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Sets the colour, as `0xRRGGBB`.
    pub fn color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    /// Shows members with this role apart in the member list.
    pub fn hoist(mut self, on: bool) -> Self {
        self.hoist = Some(on);
        self
    }

    /// Lets anyone mention this role.
    pub fn mentionable(mut self, on: bool) -> Self {
        self.mentionable = Some(on);
        self
    }

    /// Checks the fields against Discord's limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(name) = &self.name {
            check_len("name", name, 1, 100)?;
        }
        if self.color.is_some_and(|color| color > MAX_COLOR) {
            return Err(ValidationError::new("color", "must be at most 0xFFFFFF"));
        }
        Ok(())
    }
}

impl IntoBody for EditRole {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
