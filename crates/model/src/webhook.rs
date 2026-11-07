//! Ported from discord.js packages/discord.js/src/structures/Webhook.js (Apache-2.0), wire shapes
//! from discord-api-types payloads/v10/webhook.ts (MIT).
//! Changed: translated to Rust; the token is a `Secret`, so `Debug` does not print it.

use crate::{ApplicationId, ChannelId, GuildId, User, WebhookId};
use dr_core::{Secret, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// What a webhook is for.
    WebhookType: u16 {
        /// Posts messages with its token.
        INCOMING = 1,
        /// Follows an announcement channel into another channel.
        CHANNEL_FOLLOWER = 2,
        /// Used with interactions.
        APPLICATION = 3,
    }
}

/// A webhook.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Webhook {
    /// Webhook id.
    pub id: WebhookId,
    /// What it is for.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: WebhookType,
    /// Guild id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel it posts in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// Who created it. Absent when fetched with its token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Its default name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its default avatar hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// The token that lets anyone post with it. Only incoming webhooks have one, and only for those
    /// who can manage webhooks or who used the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret>,
    /// The application that created it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
}

impl Webhook {
    /// True for a webhook with a token: the bot can post with it.
    pub fn is_incoming(&self) -> bool {
        self.kind == WebhookType::INCOMING && self.token.is_some()
    }
}
