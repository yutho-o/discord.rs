//! `CreateAutoModRule` and `EditAutoModRule`. Limits from Discord's auto moderation documentation.

use crate::check_len;
use dr_core::{BodyError, IntoBody, RequestBody, ValidationError};
use dr_model::{
    AutoModAction, AutoModActionType, AutoModEventType, AutoModKeywordPreset, AutoModTriggerMetadata,
    AutoModTriggerType, ChannelId, RoleId,
};
use serde::Serialize;

/// The longest timeout an action can give, 4 weeks in seconds.
pub const MAX_AUTOMOD_TIMEOUT: u32 = 2_419_200;

/// A new auto moderation rule. Pick its trigger with one of the constructors, then add at least one
/// action. Needs `MANAGE_GUILD`.
#[derive(Clone, Debug, Serialize)]
pub struct CreateAutoModRule {
    name: String,
    event_type: AutoModEventType,
    trigger_type: AutoModTriggerType,
    trigger_metadata: AutoModTriggerMetadata,
    actions: Vec<AutoModAction>,
    enabled: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exempt_roles: Vec<RoleId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exempt_channels: Vec<ChannelId>,
}

impl CreateAutoModRule {
    fn base(
        name: String,
        event: AutoModEventType,
        trigger: AutoModTriggerType,
        metadata: AutoModTriggerMetadata,
    ) -> Self {
        Self {
            name,
            event_type: event,
            trigger_type: trigger,
            trigger_metadata: metadata,
            actions: Vec::new(),
            enabled: true,
            exempt_roles: Vec::new(),
            exempt_channels: Vec::new(),
        }
    }

    /// A rule on messages that contain one of `keywords`, up to 1000 of 60 characters each.
    pub fn keyword(name: impl Into<String>, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let metadata = AutoModTriggerMetadata {
            keyword_filter: keywords.into_iter().map(Into::into).collect(),
            ..Default::default()
        };
        Self::base(name.into(), AutoModEventType::MESSAGE_SEND, AutoModTriggerType::KEYWORD, metadata)
    }

    /// A rule on messages that look like spam. A guild has at most one.
    pub fn spam(name: impl Into<String>) -> Self {
        Self::base(
            name.into(),
            AutoModEventType::MESSAGE_SEND,
            AutoModTriggerType::SPAM,
            AutoModTriggerMetadata::default(),
        )
    }

    /// A rule on Discord's word lists. A guild has at most one.
    pub fn keyword_preset(name: impl Into<String>, presets: impl IntoIterator<Item = AutoModKeywordPreset>) -> Self {
        let metadata = AutoModTriggerMetadata { presets: presets.into_iter().collect(), ..Default::default() };
        Self::base(name.into(), AutoModEventType::MESSAGE_SEND, AutoModTriggerType::KEYWORD_PRESET, metadata)
    }

    /// A rule on messages with more than `limit` unique mentions, up to 50. A guild has at most
    /// one.
    pub fn mention_spam(name: impl Into<String>, limit: u8) -> Self {
        let metadata = AutoModTriggerMetadata { mention_total_limit: Some(limit), ..Default::default() };
        Self::base(name.into(), AutoModEventType::MESSAGE_SEND, AutoModTriggerType::MENTION_SPAM, metadata)
    }

    /// A rule on member profiles that contain one of `keywords`.
    pub fn member_profile(name: impl Into<String>, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let metadata = AutoModTriggerMetadata {
            keyword_filter: keywords.into_iter().map(Into::into).collect(),
            ..Default::default()
        };
        Self::base(name.into(), AutoModEventType::MEMBER_UPDATE, AutoModTriggerType::MEMBER_PROFILE, metadata)
    }

    /// Adds regular expressions to match, for keyword and member profile rules: up to 10 of 260
    /// characters each.
    pub fn regex(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.trigger_metadata.regex_patterns.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Words that never trigger the rule.
    pub fn allow(mut self, words: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.trigger_metadata.allow_list.extend(words.into_iter().map(Into::into));
        self
    }

    /// Turns on mention raid protection, for a mention spam rule.
    pub fn raid_protection(mut self, on: bool) -> Self {
        self.trigger_metadata.mention_raid_protection_enabled = Some(on);
        self
    }

    /// Adds an action. A rule needs at least one.
    pub fn action(mut self, action: AutoModAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Creates it turned off.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Roles it does not apply to, up to 20.
    pub fn exempt_roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.exempt_roles = roles.into_iter().collect();
        self
    }

    /// Channels it does not apply to, up to 50.
    pub fn exempt_channels(mut self, channels: impl IntoIterator<Item = ChannelId>) -> Self {
        self.exempt_channels = channels.into_iter().collect();
        self
    }

    /// Checks the name, the metadata of the trigger, and the actions.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 1, 100)?;
        if self.actions.is_empty() {
            return Err(ValidationError::new("actions", "a rule needs at least one action"));
        }
        check_metadata(self.trigger_type, &self.trigger_metadata)?;
        check_actions(&self.actions)?;
        check_exempt(&self.exempt_roles, &self.exempt_channels)
    }
}

impl IntoBody for CreateAutoModRule {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

fn check_list(path: &str, list: &[String], max_items: usize, max_len: usize) -> Result<(), ValidationError> {
    if list.len() > max_items {
        return Err(ValidationError::new(path, format!("must hold at most {max_items} entries")));
    }
    for (i, entry) in list.iter().enumerate() {
        check_len(&format!("{path}[{i}]"), entry, 1, max_len)?;
    }
    Ok(())
}

fn check_metadata(trigger: AutoModTriggerType, m: &AutoModTriggerMetadata) -> Result<(), ValidationError> {
    check_list("trigger_metadata.keyword_filter", &m.keyword_filter, 1000, 60)?;
    check_list("trigger_metadata.regex_patterns", &m.regex_patterns, 10, 260)?;
    let allow_max = if trigger == AutoModTriggerType::KEYWORD_PRESET { 1000 } else { 100 };
    check_list("trigger_metadata.allow_list", &m.allow_list, allow_max, 60)?;
    if m.mention_total_limit.is_some_and(|l| l > 50) {
        return Err(ValidationError::new("trigger_metadata.mention_total_limit", "must be at most 50"));
    }
    let wants_words = matches!(trigger, AutoModTriggerType::KEYWORD | AutoModTriggerType::MEMBER_PROFILE);
    if wants_words && m.keyword_filter.is_empty() && m.regex_patterns.is_empty() {
        return Err(ValidationError::new("trigger_metadata", "needs keywords or regex patterns"));
    }
    if trigger == AutoModTriggerType::KEYWORD_PRESET && m.presets.is_empty() {
        return Err(ValidationError::new("trigger_metadata.presets", "needs at least one preset"));
    }
    Ok(())
}

fn check_actions(actions: &[AutoModAction]) -> Result<(), ValidationError> {
    for (i, action) in actions.iter().enumerate() {
        let metadata = action.metadata.as_ref();
        if let Some(message) = metadata.and_then(|m| m.custom_message.as_deref()) {
            check_len(&format!("actions[{i}].metadata.custom_message"), message, 0, 150)?;
        }
        if action.kind == AutoModActionType::TIMEOUT
            && metadata.and_then(|m| m.duration_seconds).is_none_or(|s| s > MAX_AUTOMOD_TIMEOUT)
        {
            let path = format!("actions[{i}].metadata.duration_seconds");
            return Err(ValidationError::new(path, format!("must be at most {MAX_AUTOMOD_TIMEOUT} seconds")));
        }
    }
    Ok(())
}

fn check_exempt(roles: &[RoleId], channels: &[ChannelId]) -> Result<(), ValidationError> {
    if roles.len() > 20 {
        return Err(ValidationError::new("exempt_roles", "must hold at most 20 roles"));
    }
    if channels.len() > 50 {
        return Err(ValidationError::new("exempt_channels", "must hold at most 50 channels"));
    }
    Ok(())
}

/// An edit of a rule. Its trigger type cannot change. What is not set stays as it is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditAutoModRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_type: Option<AutoModEventType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger_metadata: Option<AutoModTriggerMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<Vec<AutoModAction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_roles: Option<Vec<RoleId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_channels: Option<Vec<ChannelId>>,
}

impl EditAutoModRule {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Replaces what it looks for, for its trigger type.
    pub fn trigger_metadata(mut self, metadata: AutoModTriggerMetadata) -> Self {
        self.trigger_metadata = Some(metadata);
        self
    }

    /// Replaces its actions.
    pub fn actions(mut self, actions: impl IntoIterator<Item = AutoModAction>) -> Self {
        self.actions = Some(actions.into_iter().collect());
        self
    }

    /// Turns it on or off.
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = Some(on);
        self
    }

    /// Replaces the exempt roles.
    pub fn exempt_roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.exempt_roles = Some(roles.into_iter().collect());
        self
    }

    /// Replaces the exempt channels.
    pub fn exempt_channels(mut self, channels: impl IntoIterator<Item = ChannelId>) -> Self {
        self.exempt_channels = Some(channels.into_iter().collect());
        self
    }

    /// Checks the fields that are set. The metadata is checked as for a keyword rule, which has the
    /// lower allow list limit.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(name) = &self.name {
            check_len("name", name, 1, 100)?;
        }
        if let Some(m) = &self.trigger_metadata {
            check_list("trigger_metadata.keyword_filter", &m.keyword_filter, 1000, 60)?;
            check_list("trigger_metadata.regex_patterns", &m.regex_patterns, 10, 260)?;
            check_list("trigger_metadata.allow_list", &m.allow_list, 1000, 60)?;
        }
        if let Some(actions) = &self.actions {
            if actions.is_empty() {
                return Err(ValidationError::new("actions", "a rule needs at least one action"));
            }
            check_actions(actions)?;
        }
        check_exempt(
            self.exempt_roles.as_deref().unwrap_or_default(),
            self.exempt_channels.as_deref().unwrap_or_default(),
        )
    }
}

impl IntoBody for EditAutoModRule {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
