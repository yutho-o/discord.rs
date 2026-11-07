//! Who a message may ping.

use dr_model::{AllowedMentionType, RoleId, UserId};
use serde::Serialize;

/// The `allowed_mentions` of a message.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AllowedMentions {
    parse: Vec<AllowedMentionType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    users: Vec<UserId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    roles: Vec<RoleId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replied_user: Option<bool>,
}

impl AllowedMentions {
    /// Pings nobody: writes `{"parse":[]}`. The form behind `.no_mentions()`.
    pub fn none() -> Self {
        Self::default()
    }
    /// Sets the mention kinds to parse from the content.
    pub fn parse(mut self, kinds: impl IntoIterator<Item = AllowedMentionType>) -> Self {
        self.parse = kinds.into_iter().collect();
        self
    }
    /// Sets the users that may be pinged.
    pub fn users(mut self, users: impl IntoIterator<Item = UserId>) -> Self {
        self.users = users.into_iter().collect();
        self
    }
    /// Sets the roles that may be pinged.
    pub fn roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }
    /// Sets whether the author of a replied-to message is pinged.
    pub fn replied_user(mut self, on: bool) -> Self {
        self.replied_user = Some(on);
        self
    }
}
