//! Scripted gateway behaviour. Each call acts on the connections open at that moment, or on the next
//! one if none is open.

use super::{Command, GatewayState};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Scripts the gateway half. Get one from `MockServer::gateway`.
#[derive(Clone, Copy, Debug)]
pub struct GatewayScript<'a> {
    state: &'a GatewayState,
}

impl<'a> GatewayScript<'a> {
    pub(crate) fn new(state: &'a GatewayState) -> Self {
        Self { state }
    }

    /// Sends a dispatch with the next sequence number.
    pub fn dispatch(self, t: &str, d: serde_json::Value) {
        self.state.command(Command::Dispatch(t.to_owned(), d));
    }

    /// Closes with this code.
    pub fn close(self, code: u16) {
        self.state.command(Command::Close(code));
    }

    /// Ignores the next `n` heartbeats, with no ACK: makes the client see a zombie connection.
    pub fn drop_acks(self, n: u32) {
        self.state.drop_acks.store(n, Ordering::SeqCst);
    }

    /// Sends op 7, Reconnect.
    pub fn send_reconnect(self) {
        self.state.command(Command::Reconnect);
    }

    /// Sends op 9, Invalid Session.
    pub fn invalid_session(self, resumable: bool) {
        self.state.command(Command::InvalidSession(resumable));
    }

    /// Splits each compressed frame across `n` WebSocket messages, to test the client's buffering.
    pub fn split_frames(self, n: u32) {
        self.state.split.store(n.max(1), Ordering::SeqCst);
    }

    /// Stops sending anything, ACKs included, while keeping the socket open.
    pub fn stall(self) {
        self.state.command(Command::Stall);
    }

    /// Sets the heartbeat interval that later connections are told in Hello.
    pub fn heartbeat_interval(self, interval: Duration) {
        let ms = u64::try_from(interval.as_millis()).unwrap_or(u64::MAX);
        self.state.heartbeat_interval_ms.store(ms, Ordering::SeqCst);
    }

    /// How many connections have been accepted.
    pub fn connections(self) -> u32 {
        self.state.connections.load(Ordering::SeqCst)
    }
}
