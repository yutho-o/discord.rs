//! Guild scheduled event endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{GuildId, GuildScheduledEvent, ScheduledEventId, ScheduledEventUser, UserId, routes};
use serde::Serialize;

#[derive(Serialize)]
struct WithUserCount {
    with_user_count: bool,
}

/// Which interested users of an event to list. With no anchor, the first ones by user id.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct EventUserPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    with_member: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<UserId>,
}

impl EventUserPage {
    /// The first page: Discord's 100 first users.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many users, 1 to 100.
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Also gives each user's member.
    #[must_use]
    pub fn with_member(mut self) -> Self {
        self.with_member = true;
        self
    }

    /// Users before this one.
    #[must_use]
    pub fn before(mut self, user: UserId) -> Self {
        self.before = Some(user);
        self
    }

    /// Users after this one.
    #[must_use]
    pub fn after(mut self, user: UserId) -> Self {
        self.after = Some(user);
        self
    }
}

/// Guild scheduled event endpoints. Get one from [`Rest::scheduled_events`].
#[derive(Clone, Copy, Debug)]
pub struct ScheduledEventsApi<'a>(pub(crate) &'a Rest);

impl ScheduledEventsApi<'_> {
    /// `GET /guilds/{guild}/scheduled-events`. With `with_user_count`, each event has its
    /// `user_count`.
    pub fn list(&self, guild: GuildId, with_user_count: bool) -> Request<Vec<GuildScheduledEvent>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_scheduled_events(guild), Ok(RequestBody::empty()))
            .query(WithUserCount { with_user_count })
    }

    /// `GET /guilds/{guild}/scheduled-events/{event}`.
    pub fn get(&self, guild: GuildId, event: ScheduledEventId, with_user_count: bool) -> Request<GuildScheduledEvent> {
        let path = routes::guild_scheduled_event(guild, event);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty()))
            .query(WithUserCount { with_user_count })
    }

    /// `POST /guilds/{guild}/scheduled-events`. Use `.reason(...)` for the audit log.
    pub fn create(&self, guild: GuildId, body: impl IntoBody) -> Request<GuildScheduledEvent> {
        Request::new(self.0.clone(), Method::Post, routes::guild_scheduled_events(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/scheduled-events/{event}`.
    pub fn modify(&self, guild: GuildId, event: ScheduledEventId, body: impl IntoBody) -> Request<GuildScheduledEvent> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_scheduled_event(guild, event), body.into_body())
    }

    /// `DELETE /guilds/{guild}/scheduled-events/{event}`.
    pub fn delete(&self, guild: GuildId, event: ScheduledEventId) -> Request<Empty> {
        let path = routes::guild_scheduled_event(guild, event);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/scheduled-events/{event}/users`.
    pub fn users(
        &self,
        guild: GuildId,
        event: ScheduledEventId,
        page: EventUserPage,
    ) -> Request<Vec<ScheduledEventUser>> {
        let path = routes::guild_scheduled_event_users(guild, event);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty())).query(page)
    }
}
