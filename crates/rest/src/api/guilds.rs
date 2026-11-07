//! Guild, member, ban and role endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{Channel, Guild, GuildBan, GuildId, Member, Role, RoleId, ThreadList, UserId, routes};
use serde::Serialize;

/// One page of a list ordered by user id, such as members or bans.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct UserPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<UserId>,
}

impl UserPage {
    /// The first page, at Discord's default size for the list.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many entries, 1 to 1000.
    #[must_use]
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit);
        self
    }

    /// The entries after this user.
    #[must_use]
    pub fn after(mut self, user: UserId) -> Self {
        self.after = Some(user);
        self
    }
}

/// Guild, member, ban and role endpoints. Get one from [`Rest::guilds`].
#[derive(Clone, Copy, Debug)]
pub struct GuildsApi<'a>(pub(crate) &'a Rest);

/// Which audit log entries to list. With no filter, the 50 newest.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct AuditLogQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_type: Option<dr_model::AuditLogEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<dr_model::AuditLogEntryId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<dr_model::AuditLogEntryId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}

impl AuditLogQuery {
    /// The newest entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only what this user did.
    #[must_use]
    pub fn user(mut self, user: UserId) -> Self {
        self.user_id = Some(user);
        self
    }

    /// Only this kind of action.
    #[must_use]
    pub fn action(mut self, action: dr_model::AuditLogEvent) -> Self {
        self.action_type = Some(action);
        self
    }

    /// Entries older than this one.
    #[must_use]
    pub fn before(mut self, entry: dr_model::AuditLogEntryId) -> Self {
        self.before = Some(entry);
        self
    }

    /// Entries newer than this one.
    #[must_use]
    pub fn after(mut self, entry: dr_model::AuditLogEntryId) -> Self {
        self.after = Some(entry);
        self
    }

    /// How many, 1 to 100.
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl GuildsApi<'_> {
    /// `GET /guilds/{guild}/audit-logs`. Needs `VIEW_AUDIT_LOG`.
    pub fn get_audit_log(&self, guild: GuildId, query: AuditLogQuery) -> Request<dr_model::AuditLog> {
        Request::new(self.0.clone(), Method::Get, routes::guild_audit_log(guild), Ok(RequestBody::empty())).query(query)
    }

    /// `GET /guilds/{guild}`.
    pub fn get(&self, guild: GuildId) -> Request<Guild> {
        Request::new(self.0.clone(), Method::Get, routes::guild(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/channels`.
    pub fn get_channels(&self, guild: GuildId) -> Request<Vec<Channel>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_channels(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/members/{user}`. The response has no `guild_id`, so the caller sets it.
    pub fn get_member(&self, guild: GuildId, user: UserId) -> Request<Member> {
        Request::new(self.0.clone(), Method::Get, routes::guild_member(guild, user), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/members`. Needs the `GUILD_MEMBERS` intent. The responses have no
    /// `guild_id`, so the caller sets it.
    pub fn get_members(&self, guild: GuildId, page: UserPage) -> Request<Vec<Member>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_members(guild), Ok(RequestBody::empty())).query(page)
    }

    /// `PATCH /guilds/{guild}/members/{user}`. The response has no `guild_id`, so the caller sets it.
    pub fn modify_member(&self, guild: GuildId, user: UserId, body: impl IntoBody) -> Request<Member> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_member(guild, user), body.into_body())
    }

    /// `PUT /guilds/{guild}/members/{user}/roles/{role}`.
    pub fn add_member_role(&self, guild: GuildId, user: UserId, role: RoleId) -> Request<Empty> {
        let path = routes::guild_member_role(guild, user, role);
        Request::new(self.0.clone(), Method::Put, path, Ok(RequestBody::empty()))
    }

    /// `DELETE /guilds/{guild}/members/{user}/roles/{role}`.
    pub fn remove_member_role(&self, guild: GuildId, user: UserId, role: RoleId) -> Request<Empty> {
        let path = routes::guild_member_role(guild, user, role);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/bans`.
    pub fn get_bans(&self, guild: GuildId, page: UserPage) -> Request<Vec<GuildBan>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_bans(guild), Ok(RequestBody::empty())).query(page)
    }

    /// `GET /guilds/{guild}/bans/{user}`.
    pub fn get_ban(&self, guild: GuildId, user: UserId) -> Request<GuildBan> {
        Request::new(self.0.clone(), Method::Get, routes::guild_ban(guild, user), Ok(RequestBody::empty()))
    }

    /// `PUT /guilds/{guild}/bans/{user}`. Use `.reason(...)` for the audit log.
    pub fn create_ban(&self, guild: GuildId, user: UserId, body: impl IntoBody) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Put, routes::guild_ban(guild, user), body.into_body())
    }

    /// `DELETE /guilds/{guild}/bans/{user}`. Use `.reason(...)` for the audit log.
    pub fn remove_ban(&self, guild: GuildId, user: UserId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_ban(guild, user), Ok(RequestBody::empty()))
    }

    /// `POST /guilds/{guild}/channels`. Use `.reason(...)` for the audit log.
    pub fn create_channel(&self, guild: GuildId, body: impl IntoBody) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Post, routes::guild_channels(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/channels`: moves several channels at once. `body` is a list of
    /// positions.
    pub fn modify_channel_positions(&self, guild: GuildId, body: impl IntoBody) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_channels(guild), body.into_body())
    }

    /// `GET /guilds/{guild}/threads/active`: every active thread the bot can see.
    pub fn active_threads(&self, guild: GuildId) -> Request<ThreadList> {
        Request::new(self.0.clone(), Method::Get, routes::guild_active_threads(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/roles`.
    pub fn get_roles(&self, guild: GuildId) -> Request<Vec<Role>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_roles(guild), Ok(RequestBody::empty()))
    }

    /// `POST /guilds/{guild}/roles`.
    pub fn create_role(&self, guild: GuildId, body: impl IntoBody) -> Request<Role> {
        Request::new(self.0.clone(), Method::Post, routes::guild_roles(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/roles/{role}`.
    pub fn modify_role(&self, guild: GuildId, role: RoleId, body: impl IntoBody) -> Request<Role> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_role(guild, role), body.into_body())
    }

    /// `DELETE /guilds/{guild}/roles/{role}`.
    pub fn delete_role(&self, guild: GuildId, role: RoleId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_role(guild, role), Ok(RequestBody::empty()))
    }

    /// `DELETE /guilds/{guild}/members/{user}`: a kick. Use `.reason(...)` for the audit log.
    pub fn remove_member(&self, guild: GuildId, user: UserId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_member(guild, user), Ok(RequestBody::empty()))
    }
}
