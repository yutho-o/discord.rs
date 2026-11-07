//! Auto moderation endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{AutoModRule, AutoModRuleId, GuildId, routes};

/// Auto moderation endpoints. Get one from [`Rest::automod`].
#[derive(Clone, Copy, Debug)]
pub struct AutoModApi<'a>(pub(crate) &'a Rest);

impl AutoModApi<'_> {
    /// `GET /guilds/{guild}/auto-moderation/rules`.
    pub fn list_rules(&self, guild: GuildId) -> Request<Vec<AutoModRule>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_automod_rules(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/auto-moderation/rules/{rule}`.
    pub fn get_rule(&self, guild: GuildId, rule: AutoModRuleId) -> Request<AutoModRule> {
        Request::new(self.0.clone(), Method::Get, routes::guild_automod_rule(guild, rule), Ok(RequestBody::empty()))
    }

    /// `POST /guilds/{guild}/auto-moderation/rules`. Use `.reason(...)` for the audit log.
    pub fn create_rule(&self, guild: GuildId, body: impl IntoBody) -> Request<AutoModRule> {
        Request::new(self.0.clone(), Method::Post, routes::guild_automod_rules(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/auto-moderation/rules/{rule}`.
    pub fn modify_rule(&self, guild: GuildId, rule: AutoModRuleId, body: impl IntoBody) -> Request<AutoModRule> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_automod_rule(guild, rule), body.into_body())
    }

    /// `DELETE /guilds/{guild}/auto-moderation/rules/{rule}`.
    pub fn delete_rule(&self, guild: GuildId, rule: AutoModRuleId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_automod_rule(guild, rule), Ok(RequestBody::empty()))
    }
}
