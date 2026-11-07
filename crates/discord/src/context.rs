//! `Context<S>`, the first argument of every handler.

use crate::error::{FailureKind, HandlerFailure, HandlerResult};
use crate::{Client, pipeline};
use futures::FutureExt;
use std::future::Future;
use std::ops::Deref;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

/// What a handler gets besides its event: the client, your state, and the shard. It derefs to
/// [`Client`], so `ctx.commands()` works.
pub struct Context<S> {
    /// The client.
    pub client: Client,
    /// Your state, from `ClientBuilder::state`.
    pub state: Arc<S>,
    /// The shard the event came from.
    pub shard: u32,
}

impl<S> Clone for Context<S> {
    fn clone(&self) -> Self {
        Self { client: self.client.clone(), state: self.state.clone(), shard: self.shard }
    }
}

impl<S> Deref for Context<S> {
    type Target = Client;
    fn deref(&self) -> &Client {
        &self.client
    }
}

impl<S> std::fmt::Debug for Context<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").field("shard", &self.shard).finish_non_exhaustive()
    }
}

impl<S> Context<S> {
    /// The gateway round trip of this shard, `None` before the first ack.
    pub fn latency(&self) -> Option<Duration> {
        self.client.shard_latency(self.shard)
    }

    /// Runs a background task outside the in-flight budget. It is tracked and drained at shutdown,
    /// and an error or panic in it reaches `on_handler_error`. Use this for anything long-lived, so
    /// that a handler does not sit on its permit.
    #[doc(alias = "spawnTask")]
    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Output = HandlerResult> + Send + 'static,
    {
        let client = self.client.clone();
        let shard = self.shard;
        self.client.0.tasks.spawn(async move {
            let kind = match AssertUnwindSafe(f).catch_unwind().await {
                Ok(Ok(())) => return,
                Ok(Err(e)) => FailureKind::Error(e),
                Err(p) => FailureKind::Panic(pipeline::dispatch::panic_message(p.as_ref())),
            };
            pipeline::dispatch::report_failure(&client, HandlerFailure::new("ctx.spawn", shard, kind));
        });
    }
}
