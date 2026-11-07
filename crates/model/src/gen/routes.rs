//! REST paths used by the proof of concept, relative to the versioned API base.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu), `Routes` in `rest/v10/index.ts`.
//! Changed: free functions over typed ids. The escaping wrapper of upstream lives in `dr_rest::encode`.
//!
//! Ids need no encoding. Tokens and emoji arrive already encoded: `dr_rest::encode` does that, and
//! its rules are `ARCHITECTURE.md` 10.2, Request shape. `@me` and `@original` are written the way
//! upstream core sends them in real traffic, `%40me` and `%40original`, with one exception:
//! `.../reactions/{emoji}/@me` is a literal in upstream's route table. Bucket keys differ between
//! the two spellings, so do not normalize them.

use crate::{ApplicationId, ChannelId, CommandId, GuildId, InteractionId, MessageId, UserId};

/// The API version these paths belong to, the `10` of `/api/v10`.
pub const API_VERSION: &str = "10";

/// `/channels/{id}`
pub fn channel(channel: ChannelId) -> String {
    format!("/channels/{channel}")
}
/// `/channels/{id}/messages`
pub fn channel_messages(channel: ChannelId) -> String {
    format!("/channels/{channel}/messages")
}
/// `/channels/{id}/messages/{id}`
pub fn channel_message(channel: ChannelId, message: MessageId) -> String {
    format!("/channels/{channel}/messages/{message}")
}
/// `/channels/{id}/messages/{id}/reactions/{emoji}/@me`. `emoji` is already percent-encoded.
pub fn channel_message_own_reaction(channel: ChannelId, message: MessageId, emoji: &str) -> String {
    format!("/channels/{channel}/messages/{message}/reactions/{emoji}/@me")
}
/// `/channels/{id}/typing`
pub fn channel_typing(channel: ChannelId) -> String {
    format!("/channels/{channel}/typing")
}
/// `/users/%40me`
pub fn current_user() -> String {
    "/users/%40me".to_owned()
}
/// `/users/{id}`
pub fn user(user: UserId) -> String {
    format!("/users/{user}")
}
/// `/gateway/bot`
pub fn gateway_bot() -> String {
    "/gateway/bot".to_owned()
}
/// `/guilds/{id}`
pub fn guild(guild: GuildId) -> String {
    format!("/guilds/{guild}")
}
/// `/guilds/{id}/channels`
pub fn guild_channels(guild: GuildId) -> String {
    format!("/guilds/{guild}/channels")
}
/// `/guilds/{id}/members/{id}`
pub fn guild_member(guild: GuildId, user: UserId) -> String {
    format!("/guilds/{guild}/members/{user}")
}
/// `/interactions/{id}/{token}/callback`. `token` is already encoded.
pub fn interaction_callback(interaction: InteractionId, token: &str) -> String {
    format!("/interactions/{interaction}/{token}/callback")
}
/// `/webhooks/{application}/{token}`, the follow-up route. `token` is already encoded.
pub fn webhook(application: ApplicationId, token: &str) -> String {
    format!("/webhooks/{application}/{token}")
}
/// `/webhooks/{application}/{token}/messages/%40original`. `token` is already encoded.
pub fn webhook_original_message(application: ApplicationId, token: &str) -> String {
    format!("/webhooks/{application}/{token}/messages/%40original")
}
/// `/webhooks/{application}/{token}/messages/{id}`. `token` is already encoded.
pub fn webhook_message(application: ApplicationId, token: &str, message: MessageId) -> String {
    format!("/webhooks/{application}/{token}/messages/{message}")
}
/// `/applications/{id}/commands`
pub fn application_commands(application: ApplicationId) -> String {
    format!("/applications/{application}/commands")
}
/// `/applications/{id}/commands/{id}`
pub fn application_command(application: ApplicationId, command: CommandId) -> String {
    format!("/applications/{application}/commands/{command}")
}
/// `/applications/{id}/guilds/{id}/commands`
pub fn application_guild_commands(application: ApplicationId, guild: GuildId) -> String {
    format!("/applications/{application}/guilds/{guild}/commands")
}
/// `/applications/{id}/guilds/{id}/commands/{id}`
pub fn application_guild_command(application: ApplicationId, guild: GuildId, command: CommandId) -> String {
    format!("/applications/{application}/guilds/{guild}/commands/{command}")
}
