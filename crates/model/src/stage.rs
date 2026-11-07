//! Stage instances: the live session of a stage channel. Upstream: `payloads/v10/stageInstance.ts`
//! and `rest/v10/stageInstance.ts` in discord-api-types (MIT, (c) 2020 vladfrangu).

use crate::{ChannelId, GuildId, ScheduledEventId, StageInstanceId};
use dr_core::open_int;
use serde::{Deserialize, Serialize};

open_int! {
    /// Who can find a stage.
    StageInstancePrivacyLevel: u16 {
        /// Discoverable outside the guild. Deprecated by Discord.
        PUBLIC = 1,
        /// Members of the guild.
        GUILD_ONLY = 2,
    }
}

/// A stage channel that is live, with its topic.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StageInstance {
    /// Its id.
    pub id: StageInstanceId,
    /// Its guild.
    pub guild_id: GuildId,
    /// Its stage channel.
    pub channel_id: ChannelId,
    /// What it is about, 1 to 120 characters.
    pub topic: String,
    /// Who can find it.
    pub privacy_level: StageInstancePrivacyLevel,
    /// Whether stage discovery is off. Deprecated by Discord.
    #[serde(default)]
    pub discoverable_disabled: bool,
    /// The scheduled event it was started for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_scheduled_event_id: Option<ScheduledEventId>,
}
