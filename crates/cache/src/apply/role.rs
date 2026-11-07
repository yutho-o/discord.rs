//! Role events.

use crate::Cache;
use dr_model::{GuildRoleDeleteEvent, GuildRoleEvent, Role};
use std::sync::Arc;

impl Cache {
    /// `GUILD_ROLE_CREATE` and `GUILD_ROLE_UPDATE`. Returns the old role, if there was one.
    pub fn apply_role_upsert(&self, shard: u32, event: &GuildRoleEvent) -> Option<Arc<Role>> {
        let _ = shard;
        if !self.config.is_enabled() {
            return None;
        }
        self.slot(event.guild_id).roles.insert(event.role.id, Arc::new(event.role.clone()))
    }

    /// `GUILD_ROLE_DELETE`. Returns the role as it was.
    pub fn apply_role_delete(&self, shard: u32, event: &GuildRoleDeleteEvent) -> Option<Arc<Role>> {
        let _ = shard;
        self.guilds.get(&event.guild_id)?.roles.remove(&event.role_id)
    }
}
