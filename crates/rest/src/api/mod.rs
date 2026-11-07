//! Endpoint groups, one file per group. Every method builds a [`crate::Request`] from a route in
//! `dr_model::routes`, and sets the retry class of `ARCHITECTURE.md` 10.2 where the default for the
//! method is wrong. Names are the upstream ones in snake case.

pub mod application_commands;
pub mod automod;
pub mod channels;
pub mod expressions;
pub mod gateway;
pub mod guilds;
pub mod interactions;
pub mod scheduled_events;
pub mod stage_instances;
pub mod users;
pub mod webhooks;

pub use application_commands::ApplicationCommandsApi;
pub use automod::AutoModApi;
pub use channels::{ArchivedThreadPage, ChannelsApi, MessagePage, ThreadMemberPage, VoterPage};
pub use expressions::ExpressionsApi;
pub use gateway::GatewayApi;
pub use guilds::{AuditLogQuery, GuildsApi, UserPage};
pub use interactions::InteractionsApi;
pub use scheduled_events::{EventUserPage, ScheduledEventsApi};
pub use stage_instances::StageInstancesApi;
pub use users::UsersApi;
pub use webhooks::{ExecuteQuery, WebhooksApi};
