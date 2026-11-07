//! Error types of the facade.

pub use dr_core::BoxError;

use dr_core::ValidationError;
use dr_gateway::GatewayError;
use dr_model::{Intents, JsonErrorCode};
use dr_rest::RestError;

/// What a handler returns. Any error type converts into it through `?`.
pub type HandlerResult = Result<(), BoxError>;

/// Why a facade operation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A REST call failed.
    #[error(transparent)]
    Rest(RestError),
    /// The gateway failed.
    #[error(transparent)]
    Gateway(#[from] GatewayError),
    /// A builder rejected its content before anything was sent.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// An interaction was misused.
    #[error(transparent)]
    Interaction(#[from] InteractionError),
    /// Something that needs `READY` ran before it, such as registering commands.
    #[error("the client is not ready yet")]
    NotReady,
    /// A wait ran out.
    #[error("timed out: {what}")]
    Timeout {
        /// What was being waited for.
        what: &'static str,
    },
    /// The client was set up in a way that cannot work.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl From<RestError> for Error {
    /// `RestError::Validation` becomes `Error::Validation`, so a caller has one place to match it.
    fn from(e: RestError) -> Self {
        match e {
            RestError::Validation(v) => Self::Validation(v),
            other => Self::Rest(other),
        }
    }
}

impl Error {
    /// Discord's error code, for API errors that have one.
    pub fn code(&self) -> Option<JsonErrorCode> {
        match self {
            Self::Rest(r) => r.code(),
            _ => None,
        }
    }

    /// True when the request reached Discord and was applied although this call returned an error.
    /// Do not repeat the send.
    pub fn request_succeeded(&self) -> bool {
        matches!(self, Self::Rest(r) if r.request_succeeded())
    }

    /// True when trying again later may work.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Rest(r) if r.is_retryable())
    }

    /// True when the failure is a rate limit.
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, Self::Rest(r) if r.is_rate_limit())
    }
}

/// How an interaction was misused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InteractionError {
    /// It was already replied to. Use `edit_reply` or `follow_up`.
    #[error("the interaction was already replied to")]
    AlreadyReplied,
    /// It needs a reply first, for `edit_reply` and the like.
    #[error("the interaction was not replied to yet")]
    NotReplied,
    /// A required option is missing or of the wrong type.
    #[error("required option `{0}` is missing or of the wrong type")]
    Option(String),
    /// The 3 s to answer ran out. The interaction token is no longer good for a first reply.
    #[error("the deadline to answer the interaction has passed")]
    DeadlineExceeded,
}

/// How a client was set up in a way that cannot work.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// A handler can never fire, since the client does not ask for the intent its event needs.
    /// `allow_missing_intents` turns this off.
    #[error("the handler for {event} can never fire: it needs one of {needs:?}, and the client has {have:?}")]
    MissingIntent {
        /// The event's name, such as `MESSAGE_CREATE`.
        event: &'static str,
        /// The intents of which any one is enough.
        needs: Intents,
        /// The intents the client asks for.
        have: Intents,
    },
}

/// A handler failed. Reaches `on_handler_error`, and always a log line. The shard is unaffected.
#[derive(Debug)]
#[non_exhaustive]
pub struct HandlerFailure {
    /// The event's name, or the kind of task: `ctx.spawn`.
    pub event: &'static str,
    /// The shard it ran for.
    pub shard: u32,
    /// How it failed.
    pub kind: FailureKind,
}

impl HandlerFailure {
    /// A failure of a handler for `event`.
    pub fn new(event: &'static str, shard: u32, kind: FailureKind) -> Self {
        Self { event, shard, kind }
    }
}

/// How a handler failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum FailureKind {
    /// It returned this error.
    Error(BoxError),
    /// It panicked with this message.
    Panic(String),
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error(e) => write!(f, "{e}"),
            Self::Panic(p) => write!(f, "panic: {p}"),
        }
    }
}
