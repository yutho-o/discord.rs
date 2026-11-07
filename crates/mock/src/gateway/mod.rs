//! The gateway half: Hello, Identify, `READY`, heartbeats, Resume and scripted behaviour.

pub mod compress;
pub mod script;
pub mod session;

pub use script::GatewayScript;

use crate::World;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// One frame as received from a client. Tests assert on these.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RecordedFrame {
    /// The shard of the connection, from its identify. 0 until then.
    pub shard: u32,
    /// The opcode.
    pub op: u32,
    /// The body.
    pub d: Value,
}

/// What a script asks of the open connections.
#[derive(Clone, Debug)]
pub(crate) enum Command {
    Dispatch(String, Value),
    Close(u16),
    Reconnect,
    InvalidSession(bool),
    Stall,
}

/// One dispatch a session has sent, kept for replay on resume.
pub(crate) type Sent = (u64, String, Value);

/// What the gateway half keeps, between the connections and the scripts.
#[derive(Debug)]
pub struct GatewayState {
    pub(crate) world: Arc<World>,
    pub(crate) url: String,
    pub(crate) voice_url: String,
    pub(crate) frames: Mutex<Vec<RecordedFrame>>,
    pub(crate) open: Mutex<Vec<mpsc::Sender<Command>>>,
    pub(crate) pending: Mutex<Vec<Command>>,
    pub(crate) sessions: Mutex<HashMap<String, Vec<Sent>>>,
    pub(crate) drop_acks: AtomicU32,
    pub(crate) split: AtomicU32,
    pub(crate) chunk_size: AtomicU32,
    pub(crate) next_session: AtomicU64,
    pub(crate) connections: AtomicU32,
    pub(crate) heartbeat_interval_ms: AtomicU64,
}

impl GatewayState {
    pub(crate) fn new(world: Arc<World>, url: String, voice_url: String) -> Self {
        Self {
            world,
            url,
            voice_url,
            frames: Mutex::new(Vec::new()),
            open: Mutex::new(Vec::new()),
            pending: Mutex::new(Vec::new()),
            sessions: Mutex::new(HashMap::new()),
            drop_acks: AtomicU32::new(0),
            split: AtomicU32::new(1),
            chunk_size: AtomicU32::new(1000),
            next_session: AtomicU64::new(1),
            connections: AtomicU32::new(0),
            heartbeat_interval_ms: AtomicU64::new(session::HEARTBEAT_INTERVAL_MS),
        }
    }

    pub(crate) fn frames(&self, shard: u32) -> Vec<RecordedFrame> {
        self.frames.lock().iter().filter(|f| f.shard == shard).cloned().collect()
    }

    /// Sends to the connections open now, or keeps the command for the next one.
    pub(crate) fn command(&self, command: Command) {
        let mut open = self.open.lock();
        open.retain(|c| !c.is_closed());
        if open.is_empty() {
            self.pending.lock().push(command);
            return;
        }
        for connection in open.iter() {
            // A connection whose queue is full is one the test has stopped caring about.
            let _ = connection.try_send(command.clone());
        }
    }
}

/// Accepts connections until the task is aborted, each in a task of its own.
pub(crate) async fn serve(listener: TcpListener, state: Arc<GatewayState>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(session::run(stream, state.clone()));
            }
            Err(e) => {
                tracing::warn!(error = %e, "mock gateway listener ended");
                return;
            }
        }
    }
}
