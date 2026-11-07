//! `ctx.commands()`, command registration.

use crate::{Bound, Call, Client, Error};
use dr_builders::{CommandData, CommandList};
use dr_model::{ApplicationCommand, ApplicationId, GuildId};

/// Application commands. Get one from [`Client::commands`]. Everything here needs the application id
/// from `READY`, and before that fails with [`Error::NotReady`] when awaited.
#[derive(Clone, Debug)]
pub struct CommandsManager {
    client: Client,
}

impl Client {
    /// Application commands.
    pub fn commands(&self) -> CommandsManager {
        CommandsManager { client: self.clone() }
    }
}

impl CommandsManager {
    /// Runs `make` with the application id, or fails with `NotReady` before `READY`.
    fn with_app<T: crate::Bindable>(&self, make: impl FnOnce(ApplicationId) -> Call<T>) -> Call<T> {
        match self.client.application_id() {
            Some(app) => make(app),
            None => Call::failed(self.client.clone(), Error::NotReady),
        }
    }

    /// Replaces every global command with these. They can take a while to appear. A body that fails
    /// validation surfaces when this is awaited, with nothing sent.
    pub fn set_global<I, C>(&self, commands: I) -> Call<Vec<Bound<ApplicationCommand>>>
    where
        I: IntoIterator<Item = C>,
        C: Into<CommandData>,
    {
        let list: CommandList = commands.into_iter().map(Into::into).collect();
        self.with_app(|app| {
            let request = self.client.rest().application_commands().bulk_overwrite_global_commands(app, list);
            Call::new(self.client.clone(), request)
        })
    }

    /// Replaces every command of a guild with these. They appear at once.
    pub fn set_guild<I, C>(&self, guild: GuildId, commands: I) -> Call<Vec<Bound<ApplicationCommand>>>
    where
        I: IntoIterator<Item = C>,
        C: Into<CommandData>,
    {
        let list: CommandList = commands.into_iter().map(Into::into).collect();
        self.with_app(|app| {
            let request = self.client.rest().application_commands().bulk_overwrite_guild_commands(app, guild, list);
            Call::new(self.client.clone(), request)
        })
    }

    /// The global commands from Discord.
    pub fn list_global(&self) -> Call<Vec<Bound<ApplicationCommand>>> {
        self.with_app(|app| {
            Call::new(self.client.clone(), self.client.rest().application_commands().get_global_commands(app))
        })
    }

    /// The commands of a guild from Discord.
    pub fn list_guild(&self, guild: GuildId) -> Call<Vec<Bound<ApplicationCommand>>> {
        self.with_app(|app| {
            Call::new(self.client.clone(), self.client.rest().application_commands().get_guild_commands(app, guild))
        })
    }
}
