//! REST results, queries and bodies that are not a payload of their own, named by method and path
//! per `ARCHITECTURE.md` 7.1. Request bodies mostly come from `dr_builders`.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu), `rest/v10` and
//! `payloads/v10/gateway.ts`.
//! Changed: translated to serde structs.

use serde::{Deserialize, Serialize};

/// Result of `GET /gateway/bot`. Upstream `APIGatewayBotInfo`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetGatewayBotResult {
    /// WebSocket URL to connect to.
    #[serde(default)]
    pub url: String,
    /// Recommended shard count.
    #[serde(default)]
    pub shards: u32,
    /// Identify budget.
    pub session_start_limit: SessionStartLimit,
}

/// The identify budget of a bot. Upstream `APIGatewaySessionStartLimit`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionStartLimit {
    /// Identifies allowed per window.
    #[serde(default)]
    pub total: u32,
    /// Identifies left in this window.
    #[serde(default)]
    pub remaining: u32,
    /// Milliseconds until the window resets.
    #[serde(default)]
    pub reset_after: u64,
    /// Identifies allowed per 5 seconds.
    #[serde(default)]
    pub max_concurrency: u32,
}

/// Query of `GET /guilds/{id}`. Upstream `RESTGetAPIGuildQuery`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetGuildQuery {
    /// Whether to include approximate member and presence counts. Discord defaults to false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_counts: Option<bool>,
}

/// Query of `GET /applications/{id}/commands` and its guild form. Upstream
/// `RESTGetAPIApplicationCommandsQuery`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetApplicationCommandsQuery {
    /// Whether to include all localizations. Discord defaults to false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_localizations: Option<bool>,
}

/// Query of `POST /interactions/{id}/{token}/callback`. Upstream
/// `RESTPostAPIInteractionCallbackQuery`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PostInteractionCallbackQuery {
    /// Whether to get the callback response as the result, instead of a 204.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_response: Option<bool>,
}
