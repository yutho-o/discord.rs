//! REST paths, relative to the versioned API base.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu), `Routes` in `rest/v10/index.ts`.
//! Changed: free functions over typed ids. The escaping wrapper of upstream lives in `dr_rest::encode`.
//!
//! Ids need no encoding. Tokens and emoji arrive already encoded: `dr_rest::encode` does that, and
//! its rules are `ARCHITECTURE.md` 10.2, Request shape. `@me` and `@original` are written the way
//! upstream core sends them in real traffic, `%40me` and `%40original`, with one exception:
//! `.../reactions/{emoji}/@me` is a literal in upstream's route table. Bucket keys differ between
//! the two spellings, so do not normalize them.

use crate::{
    ApplicationId, ChannelId, CommandId, GenericId, GuildId, InteractionId, MessageId, RoleId, UserId, WebhookId,
};

/// The API version these paths belong to, the `10` of `/api/v10`.
pub const API_VERSION: &str = "10";

/// `/channels/{id}`
pub fn channel(channel: ChannelId) -> String {
    format!("/channels/{channel}")
}

/// `/channels/{id}/threads`
pub fn channel_threads(channel: ChannelId) -> String {
    format!("/channels/{channel}/threads")
}

/// `/channels/{id}/messages/{message}/threads`
pub fn channel_message_threads(channel: ChannelId, message: MessageId) -> String {
    format!("/channels/{channel}/messages/{message}/threads")
}

/// `/channels/{id}/threads/archived/{public|private}`
pub fn channel_archived_threads(channel: ChannelId, private: bool) -> String {
    format!("/channels/{channel}/threads/archived/{}", if private { "private" } else { "public" })
}

/// `/channels/{id}/users/@me/threads/archived/private`
pub fn channel_joined_archived_threads(channel: ChannelId) -> String {
    format!("/channels/{channel}/users/%40me/threads/archived/private")
}

/// `/channels/{thread}/thread-members`
pub fn thread_members(thread: ChannelId) -> String {
    format!("/channels/{thread}/thread-members")
}

/// `/channels/{thread}/thread-members/{user}`
pub fn thread_member(thread: ChannelId, user: UserId) -> String {
    format!("/channels/{thread}/thread-members/{user}")
}

/// `/channels/{thread}/thread-members/@me`
pub fn thread_member_me(thread: ChannelId) -> String {
    format!("/channels/{thread}/thread-members/%40me")
}

/// `/guilds/{id}/threads/active`
pub fn guild_active_threads(guild: GuildId) -> String {
    format!("/guilds/{guild}/threads/active")
}

/// `/channels/{id}/webhooks`
pub fn channel_webhooks(channel: ChannelId) -> String {
    format!("/channels/{channel}/webhooks")
}

/// `/guilds/{id}/webhooks`
pub fn guild_webhooks(guild: GuildId) -> String {
    format!("/guilds/{guild}/webhooks")
}

/// `/webhooks/{id}`
pub fn webhook_by_id(webhook: WebhookId) -> String {
    format!("/webhooks/{webhook}")
}

/// `/webhooks/{id}/{token}`. The token arrives encoded.
pub fn webhook_with_token(webhook: WebhookId, token: &str) -> String {
    format!("/webhooks/{webhook}/{token}")
}

/// `/webhooks/{id}/{token}/messages/{message}`. The token arrives encoded.
pub fn webhook_token_message(webhook: WebhookId, token: &str, message: MessageId) -> String {
    format!("/webhooks/{webhook}/{token}/messages/{message}")
}

/// `/channels/{id}/permissions/{overwrite}`, where `overwrite` is a role or a user.
pub fn channel_permission(channel: ChannelId, overwrite: GenericId) -> String {
    format!("/channels/{channel}/permissions/{overwrite}")
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
/// `/channels/{id}/messages/{id}/reactions/{emoji}/{id}`. `emoji` is already percent-encoded.
pub fn channel_message_user_reaction(channel: ChannelId, message: MessageId, emoji: &str, user: UserId) -> String {
    format!("/channels/{channel}/messages/{message}/reactions/{emoji}/{user}")
}
/// `/channels/{id}/messages/{id}/reactions`
pub fn channel_message_all_reactions(channel: ChannelId, message: MessageId) -> String {
    format!("/channels/{channel}/messages/{message}/reactions")
}
/// `/channels/{id}/messages/bulk-delete`
pub fn channel_bulk_delete(channel: ChannelId) -> String {
    format!("/channels/{channel}/messages/bulk-delete")
}
/// `/channels/{id}/messages/pins/{id}`
pub fn channel_message_pin(channel: ChannelId, message: MessageId) -> String {
    format!("/channels/{channel}/messages/pins/{message}")
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
/// `/guilds/{id}/members`
pub fn guild_members(guild: GuildId) -> String {
    format!("/guilds/{guild}/members")
}
/// `/guilds/{id}/members/{id}/roles/{id}`
pub fn guild_member_role(guild: GuildId, user: UserId, role: RoleId) -> String {
    format!("/guilds/{guild}/members/{user}/roles/{role}")
}
/// `/guilds/{id}/bans`
pub fn guild_bans(guild: GuildId) -> String {
    format!("/guilds/{guild}/bans")
}
/// `/guilds/{id}/bans/{id}`
pub fn guild_ban(guild: GuildId, user: UserId) -> String {
    format!("/guilds/{guild}/bans/{user}")
}
/// `/guilds/{id}/roles`
pub fn guild_roles(guild: GuildId) -> String {
    format!("/guilds/{guild}/roles")
}
/// `/guilds/{id}/roles/{id}`
pub fn guild_role(guild: GuildId, role: RoleId) -> String {
    format!("/guilds/{guild}/roles/{role}")
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
