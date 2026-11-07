//! Voice states.

use crate::{Cache, Change};
use dr_model::VoiceState;
use std::sync::Arc;

impl Cache {
    /// `VOICE_STATE_UPDATE`. Stores where the user is, or forgets them once they left voice. A
    /// member in the event goes to the member table under the policy. `old` is where they were.
    pub fn apply_voice_state_update(&self, shard: u32, state: VoiceState) -> Change<VoiceState> {
        let _ = shard;
        let new = Arc::new(state);
        let Some(guild) = new.guild_id.filter(|_| self.config.is_enabled()) else {
            return Change { old: None, new };
        };
        let slot = self.slot(guild);
        if let Some(member) = &new.member {
            self.store_member(&slot, guild, member.clone());
        }
        let old = if new.channel_id.is_some() {
            slot.voice_states.insert(new.user_id, new.clone())
        } else {
            slot.voice_states.remove(&new.user_id)
        };
        Change { old, new }
    }
}
