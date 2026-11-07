//! Application command endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{ApplicationCommand, ApplicationId, CommandId, GuildId, routes};

/// Application command endpoints. Get one from [`Rest::application_commands`].
#[derive(Clone, Copy, Debug)]
pub struct ApplicationCommandsApi<'a>(pub(crate) &'a Rest);

impl ApplicationCommandsApi<'_> {
    /// `GET /applications/{app}/commands`.
    pub fn get_global_commands(&self, app: ApplicationId) -> Request<Vec<ApplicationCommand>> {
        Request::new(self.0.clone(), Method::Get, routes::application_commands(app), Ok(RequestBody::empty()))
    }

    /// `POST /applications/{app}/commands`.
    pub fn create_global_command(&self, app: ApplicationId, body: impl IntoBody) -> Request<ApplicationCommand> {
        Request::new(self.0.clone(), Method::Post, routes::application_commands(app), body.into_body())
    }

    /// `DELETE /applications/{app}/commands/{command}`.
    pub fn delete_global_command(&self, app: ApplicationId, command: CommandId) -> Request<Empty> {
        Request::new(
            self.0.clone(),
            Method::Delete,
            routes::application_command(app, command),
            Ok(RequestBody::empty()),
        )
    }

    /// `PUT /applications/{app}/commands`. Replaces the whole set.
    pub fn bulk_overwrite_global_commands(
        &self,
        app: ApplicationId,
        body: impl IntoBody,
    ) -> Request<Vec<ApplicationCommand>> {
        Request::new(self.0.clone(), Method::Put, routes::application_commands(app), body.into_body())
    }

    /// `GET /applications/{app}/guilds/{guild}/commands`.
    pub fn get_guild_commands(&self, app: ApplicationId, guild: GuildId) -> Request<Vec<ApplicationCommand>> {
        Request::new(
            self.0.clone(),
            Method::Get,
            routes::application_guild_commands(app, guild),
            Ok(RequestBody::empty()),
        )
    }

    /// `POST /applications/{app}/guilds/{guild}/commands`.
    pub fn create_guild_command(
        &self,
        app: ApplicationId,
        guild: GuildId,
        body: impl IntoBody,
    ) -> Request<ApplicationCommand> {
        Request::new(self.0.clone(), Method::Post, routes::application_guild_commands(app, guild), body.into_body())
    }

    /// `DELETE /applications/{app}/guilds/{guild}/commands/{command}`.
    pub fn delete_guild_command(&self, app: ApplicationId, guild: GuildId, command: CommandId) -> Request<Empty> {
        let path = routes::application_guild_command(app, guild, command);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `PUT /applications/{app}/guilds/{guild}/commands`. Replaces the whole set.
    pub fn bulk_overwrite_guild_commands(
        &self,
        app: ApplicationId,
        guild: GuildId,
        body: impl IntoBody,
    ) -> Request<Vec<ApplicationCommand>> {
        Request::new(self.0.clone(), Method::Put, routes::application_guild_commands(app, guild), body.into_body())
    }
}
