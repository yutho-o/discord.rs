//! The auto moderation rules of one guild.

use crate::{Bound, Call, Client};
use dr_builders::{CreateAutoModRule, EditAutoModRule};
use dr_model::{AutoModRule, AutoModRuleId, GuildId};

/// The auto moderation rules of one guild. Get one from `GuildHandle::automod`. Every call needs
/// `MANAGE_GUILD`. Rules are not cached: Discord sends none with the guild.
#[derive(Clone, Debug)]
pub struct AutoModManager {
    client: Client,
    guild: GuildId,
}

impl AutoModManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// Every rule of this guild.
    pub fn fetch_all(&self) -> Call<Vec<Bound<AutoModRule>>> {
        Call::new(self.client.clone(), self.client.rest().automod().list_rules(self.guild))
    }

    /// One rule.
    pub fn fetch(&self, id: AutoModRuleId) -> Call<Bound<AutoModRule>> {
        Call::new(self.client.clone(), self.client.rest().automod().get_rule(self.guild, id))
    }

    /// Creates a rule. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateAutoModRule) -> Call<Bound<AutoModRule>> {
        Call::new(self.client.clone(), self.client.rest().automod().create_rule(self.guild, body))
    }

    /// Edits a rule.
    pub fn edit(&self, id: AutoModRuleId, body: EditAutoModRule) -> Call<Bound<AutoModRule>> {
        Call::new(self.client.clone(), self.client.rest().automod().modify_rule(self.guild, id, body))
    }

    /// Deletes a rule.
    pub fn delete(&self, id: AutoModRuleId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().automod().delete_rule(self.guild, id))
    }
}
