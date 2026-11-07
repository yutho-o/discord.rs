//! The shutdown order: close the socket, drain the queue, wait for in-flight handlers and
//! `ctx.spawn` tasks, then give up on the rest.
//!
//! The socket closes first, unlike the order `ARCHITECTURE.md` 10.1 lists. A frame read after the
//! queue closed would be lost, while its sequence number would already be in the saved session, so a
//! resume could never get it back. Handlers answering over REST do not need the socket.

use crate::Client;
use dr_gateway::ShutdownMode;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;
use tokio::task::AbortHandle;

/// Where a client is in its life.
#[derive(Debug)]
pub struct Lifecycle {
    started: AtomicBool,
    finished: watch::Sender<bool>,
    ingest: Mutex<Option<AbortHandle>>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self { started: AtomicBool::new(false), finished: watch::Sender::new(false), ingest: Mutex::new(None) }
    }
}

impl Lifecycle {
    /// True once a shutdown began.
    pub fn is_stopping(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    /// Keeps the ingest task's handle, to abort it past the drain.
    pub fn set_ingest(&self, handle: AbortHandle) {
        *self.ingest.lock() = Some(handle);
    }

    /// Waits until a shutdown finished.
    pub async fn finished(&self) {
        let mut finished = self.finished.subscribe();
        // The sender lives in `self`, so this cannot fail while we are borrowing it.
        let _ = finished.wait_for(|done| *done).await;
    }
}

/// Runs the shutdown order. Idempotent: a second call waits for the first. Bounded by
/// `shutdown_drain`.
pub async fn run(client: &Client, mode: ShutdownMode) {
    let life = &client.0.lifecycle;
    if life.started.swap(true, Ordering::SeqCst) {
        life.finished().await;
        return;
    }
    let drain = client.0.options.shutdown_drain;
    if let Some(gateway) = client.0.gateway.get()
        && tokio::time::timeout(drain, gateway.shutdown(mode)).await.is_err()
    {
        tracing::warn!("the gateway did not close within the drain");
    }
    if let Some(queue) = client.0.queue.get() {
        queue.close();
    }
    client.0.tasks.close();
    if tokio::time::timeout(drain, client.0.tasks.wait()).await.is_err() {
        tracing::warn!(left = client.0.tasks.len(), "handlers still running after the drain");
        client.0.budget.close();
        if let Some(ingest) = life.ingest.lock().take() {
            ingest.abort();
        }
    }
    life.finished.send_replace(true);
}
