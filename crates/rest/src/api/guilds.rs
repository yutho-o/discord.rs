//! Guild and member endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::RequestBody;
use dr_model::{Channel, Guild, GuildId, Member, UserId, routes};

/// Guild and member endpoints. Get one from [`Rest::guilds`].
#[derive(Clone, Copy, Debug)]
pub struct GuildsApi<'a>(pub(crate) &'a Rest);

impl GuildsApi<'_> {
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

    /// `DELETE /guilds/{guild}/members/{user}`: a kick. Use `.reason(...)` for the audit log.
    pub fn remove_member(&self, guild: GuildId, user: UserId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_member(guild, user), Ok(RequestBody::empty()))
    }
}
