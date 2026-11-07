//! The scheduled events of one guild.

use crate::{Bound, Call, Client};
use dr_builders::{CreateScheduledEvent, EditScheduledEvent};
use dr_model::{GuildId, GuildScheduledEvent, ScheduledEventId, ScheduledEventStatus, ScheduledEventUser};
use dr_rest::api::EventUserPage;

/// The scheduled events of one guild. Get one from `GuildHandle::scheduled_events`.
#[derive(Clone, Debug)]
pub struct ScheduledEventsManager {
    client: Client,
    guild: GuildId,
}

impl ScheduledEventsManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// The event from the cache. No I/O.
    pub fn get(&self, id: ScheduledEventId) -> Option<Bound<GuildScheduledEvent>> {
        self.client.cache().scheduled_event(self.guild, id).map(|e| self.client.bind_arc(e))
    }

    /// Every cached event of this guild, in no particular order. No I/O.
    pub fn cached(&self) -> Vec<Bound<GuildScheduledEvent>> {
        let events = self.client.cache().guild_scheduled_events(self.guild);
        events.into_iter().map(|e| self.client.bind_arc(e)).collect()
    }

    /// Every event of this guild, from Discord, each with how many users are interested.
    pub fn fetch_all(&self) -> Call<Vec<Bound<GuildScheduledEvent>>> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().list(self.guild, true))
    }

    /// One event, from Discord, with how many users are interested.
    pub fn fetch(&self, id: ScheduledEventId) -> Call<Bound<GuildScheduledEvent>> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().get(self.guild, id, true))
    }

    /// Creates an event. Needs `CREATE_EVENTS`. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateScheduledEvent) -> Call<Bound<GuildScheduledEvent>> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().create(self.guild, body))
    }

    /// Edits an event. Needs `MANAGE_EVENTS`, or to have created it with `CREATE_EVENTS`.
    pub fn edit(&self, id: ScheduledEventId, body: EditScheduledEvent) -> Call<Bound<GuildScheduledEvent>> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().modify(self.guild, id, body))
    }

    /// Starts an event now.
    pub fn start(&self, id: ScheduledEventId) -> Call<Bound<GuildScheduledEvent>> {
        self.edit(id, EditScheduledEvent::new().status(ScheduledEventStatus::ACTIVE))
    }

    /// Ends an active event.
    pub fn end(&self, id: ScheduledEventId) -> Call<Bound<GuildScheduledEvent>> {
        self.edit(id, EditScheduledEvent::new().status(ScheduledEventStatus::COMPLETED))
    }

    /// Cancels an event that has not started.
    pub fn cancel(&self, id: ScheduledEventId) -> Call<Bound<GuildScheduledEvent>> {
        self.edit(id, EditScheduledEvent::new().status(ScheduledEventStatus::CANCELED))
    }

    /// Deletes an event, with the same permissions as an edit.
    pub fn delete(&self, id: ScheduledEventId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().delete(self.guild, id))
    }

    /// The users interested in an event, one page of them.
    pub fn users(&self, id: ScheduledEventId, page: EventUserPage) -> Call<Vec<Bound<ScheduledEventUser>>> {
        Call::new(self.client.clone(), self.client.rest().scheduled_events().users(self.guild, id, page))
    }
}
