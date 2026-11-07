//! Interactions: routing, the reply state machine and option reading.

pub mod autocomplete;
pub mod command;
pub mod component;
pub mod modal;
pub mod options;
pub mod reply;
pub mod router;

pub use autocomplete::{AutocompleteChoice, AutocompleteInteraction};
pub use command::CommandInteraction;
pub use component::ComponentInteraction;
pub use modal::ModalInteraction;
pub use options::CommandOptions;
pub use reply::{FollowUpRequest, ReplyRequest, ReplyState};

use crate::Bound;

/// The payload of `InteractionCreate`. Routed handlers get the inner value directly.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Interaction {
    /// A slash or context menu command.
    Command(Bound<CommandInteraction>),
    /// A button or select.
    Component(Bound<ComponentInteraction>),
    /// An autocomplete request.
    Autocomplete(Bound<AutocompleteInteraction>),
    /// A modal was submitted.
    ModalSubmit(Bound<ModalInteraction>),
    /// A kind this build does not know.
    Unknown(Bound<dr_model::Interaction>),
}
