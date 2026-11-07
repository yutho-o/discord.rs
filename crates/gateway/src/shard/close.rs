//! The close code table of `ARCHITECTURE.md` 10.1.

/// What to do after a connection closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAction {
    /// Reconnect and resume the session.
    Resume,
    /// Reconnect, forget the session and identify.
    Identify,
    /// Stop for good.
    Fatal,
}

/// The action for a close code received from the peer. Our own 4200 never reaches this.
pub const fn action(code: Option<u16>) -> CloseAction {
    match code {
        Some(1000 | 4003 | 4005 | 4007 | 4008) => CloseAction::Identify,
        Some(4004 | 4010..=4014) => CloseAction::Fatal,
        // Any other, including no code at all: try to resume. An op 9 corrects it if that is wrong.
        _ => CloseAction::Resume,
    }
}
