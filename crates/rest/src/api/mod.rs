//! Endpoint groups, one file per group. Every method builds a [`crate::Request`] from a route in
//! `dr_model::routes`, and sets the retry class of `ARCHITECTURE.md` 10.2 where the default for the
//! method is wrong. Names are the upstream ones in snake case.

pub mod application_commands;
pub mod channels;
pub mod gateway;
pub mod guilds;
pub mod interactions;
pub mod users;

pub use application_commands::ApplicationCommandsApi;
pub use channels::ChannelsApi;
pub use gateway::GatewayApi;
pub use guilds::GuildsApi;
pub use interactions::InteractionsApi;
pub use users::UsersApi;
