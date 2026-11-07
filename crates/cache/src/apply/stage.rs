//! Stage instance events.

use crate::{Cache, Change};
use dr_model::StageInstance;
use std::sync::Arc;

impl Cache {
    /// `STAGE_INSTANCE_CREATE` and `STAGE_INSTANCE_UPDATE`. `old` is the stage as it was.
    pub fn apply_stage_upsert(&self, shard: u32, stage: StageInstance) -> Change<StageInstance> {
        let _ = shard;
        let new = Arc::new(stage);
        if !self.config.is_enabled() {
            return Change { old: None, new };
        }
        let old = self.slot(new.guild_id).stage_instances.insert(new.channel_id, new.clone());
        Change { old, new }
    }

    /// `STAGE_INSTANCE_DELETE`: the stage ended. Returns it as it was cached.
    pub fn apply_stage_delete(&self, shard: u32, stage: &StageInstance) -> Option<Arc<StageInstance>> {
        let _ = shard;
        self.guilds.get(&stage.guild_id)?.stage_instances.remove(&stage.channel_id)
    }
}
