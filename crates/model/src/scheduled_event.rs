//! Guild scheduled events. Upstream: `payloads/v10/guildScheduledEvent.ts`,
//! `rest/v10/guildScheduledEvent.ts` and the scheduled event dispatches of `gateway/v10.ts` in
//! discord-api-types (MIT, (c) 2020 vladfrangu).
//!
//! Upstream's three variants, by entity type, are one struct here: they differ only in which of
//! `channel_id` and `entity_metadata` are set.

use crate::{ChannelId, GenericId, GuildId, Member, ScheduledEventId, User, UserId};
use dr_core::{Timestamp, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// Where an event happens.
    ScheduledEventEntityType: u16 {
        /// In a stage channel.
        STAGE_INSTANCE = 1,
        /// In a voice channel.
        VOICE = 2,
        /// Somewhere else, given by `entity_metadata.location`.
        EXTERNAL = 3,
    }
}

open_int! {
    /// Where an event is in its life.
    ScheduledEventStatus: u16 {
        SCHEDULED = 1,
        ACTIVE = 2,
        COMPLETED = 3,
        CANCELED = 4,
    }
}

open_int! {
    /// Who can see an event.
    ScheduledEventPrivacyLevel: u16 {
        /// Members of the guild. The only level so far.
        GUILD_ONLY = 2,
    }
}

impl Default for ScheduledEventPrivacyLevel {
    fn default() -> Self {
        Self::GUILD_ONLY
    }
}

open_int! {
    /// How often a recurring event repeats.
    RecurrenceFrequency: u8 {
        YEARLY = 0,
        MONTHLY = 1,
        WEEKLY = 2,
        DAILY = 3,
    }
}

open_int! {
    /// A day of the week, from Monday as 0.
    Weekday: u8 {
        MONDAY = 0,
        TUESDAY = 1,
        WEDNESDAY = 2,
        THURSDAY = 3,
        FRIDAY = 4,
        SATURDAY = 5,
        SUNDAY = 6,
    }
}

open_int! {
    /// A month, from January as 1.
    Month: u8 {
        JANUARY = 1,
        FEBRUARY = 2,
        MARCH = 3,
        APRIL = 4,
        MAY = 5,
        JUNE = 6,
        JULY = 7,
        AUGUST = 8,
        SEPTEMBER = 9,
        OCTOBER = 10,
        NOVEMBER = 11,
        DECEMBER = 12,
    }
}

/// An event planned in a guild.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildScheduledEvent {
    /// Its id.
    pub id: ScheduledEventId,
    /// Its guild.
    pub guild_id: GuildId,
    /// The stage or voice channel it happens in. `None` for an external event.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// Who created it. `None` for events created before October 2021.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_id: Option<UserId>,
    /// Its name.
    pub name: String,
    /// Its description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When it starts.
    pub scheduled_start_time: Timestamp,
    /// When it ends. Always set for an external event.
    #[serde(default)]
    pub scheduled_end_time: Option<Timestamp>,
    /// Who can see it.
    #[serde(default)]
    pub privacy_level: ScheduledEventPrivacyLevel,
    /// Where it is in its life.
    pub status: ScheduledEventStatus,
    /// Where it happens.
    pub entity_type: ScheduledEventEntityType,
    /// The stage instance of a stage event, once it started.
    #[serde(default)]
    pub entity_id: Option<GenericId>,
    /// Where an external event happens.
    #[serde(default)]
    pub entity_metadata: Option<ScheduledEventEntityMetadata>,
    /// Who created it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<User>,
    /// How many users are interested, when asked for with `with_user_count`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_count: Option<u32>,
    /// The hash of its cover image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// How it repeats. `None` for a one-time event.
    #[serde(default)]
    pub recurrence_rule: Option<RecurrenceRule>,
}

/// Where an external event happens.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScheduledEventEntityMetadata {
    /// The place, 1 to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// How an event repeats. Discord accepts only some combinations; see its documentation of the
/// recurrence rule object.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecurrenceRule {
    /// When the first occurrence starts.
    pub start: Timestamp,
    /// When the rule ends. Discord sets it, and ignores it on writes.
    #[serde(default)]
    pub end: Option<Timestamp>,
    /// How often.
    pub frequency: RecurrenceFrequency,
    /// Every how many `frequency` units, such as 2 for every other week.
    pub interval: u32,
    /// On which weekdays.
    #[serde(default)]
    pub by_weekday: Option<Vec<Weekday>>,
    /// On which weekday of which week of the month, such as the second Tuesday.
    #[serde(default)]
    pub by_n_weekday: Option<Vec<NWeekday>>,
    /// In which months.
    #[serde(default)]
    pub by_month: Option<Vec<Month>>,
    /// On which days of the month, from 1.
    #[serde(default)]
    pub by_month_day: Option<Vec<u8>>,
    /// On which days of the year. Discord sets it, and ignores it on writes.
    #[serde(default)]
    pub by_year_day: Option<Vec<u16>>,
    /// After how many occurrences it stops. Discord sets it, and ignores it on writes.
    #[serde(default)]
    pub count: Option<u32>,
}

impl RecurrenceRule {
    /// A rule from `start` that repeats every `interval` units of `frequency`, with no other
    /// constraint.
    pub fn new(start: Timestamp, frequency: RecurrenceFrequency, interval: u32) -> Self {
        Self {
            start,
            end: None,
            frequency,
            interval,
            by_weekday: None,
            by_n_weekday: None,
            by_month: None,
            by_month_day: None,
            by_year_day: None,
            count: None,
        }
    }
}

/// The `n`th `day` of a month, such as the second Tuesday.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NWeekday {
    /// Which week, 1 to 5.
    pub n: u8,
    /// Which day.
    pub day: Weekday,
}

/// A user interested in an event, from `GET .../scheduled-events/{event}/users`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduledEventUser {
    /// The event.
    pub guild_scheduled_event_id: ScheduledEventId,
    /// The user.
    pub user: User,
    /// Their member, when asked for with `with_member`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

/// `GUILD_SCHEDULED_EVENT_USER_ADD` and `GUILD_SCHEDULED_EVENT_USER_REMOVE`: someone marked or
/// unmarked an event as interesting.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduledEventUserEvent {
    /// The event.
    pub guild_scheduled_event_id: ScheduledEventId,
    /// Who.
    pub user_id: UserId,
    /// The guild.
    pub guild_id: GuildId,
}
