//! Running one handler: budget permit, then a task of its own, then `catch_unwind`.

use super::budget::Lane;
use crate::Client;
use crate::error::{FailureKind, HandlerFailure};
use crate::events::Erased;
use futures::FutureExt;
use std::any::Any;
use std::panic::AssertUnwindSafe;

/// Starts `handler` in a task. Waits only for room in the lane, never for the handler, so a slow
/// handler holds up nothing but its own permit. A closed budget drops the event silently: shutdown
/// is under way.
pub async fn spawn_handler<P: Send + 'static>(
    client: &Client,
    lane: Lane,
    event: &'static str,
    shard: u32,
    handler: Erased<P>,
    payload: P,
) {
    let Some(permit) = client.0.budget.acquire(lane).await else { return };
    let owner = client.clone();
    client.0.tasks.spawn(async move {
        // The call is inside the async block so that a panic before the first await is caught too.
        let run = AssertUnwindSafe(async { handler(owner.clone(), shard, payload).await }).catch_unwind().await;
        drop(permit);
        let kind = match run {
            Ok(Ok(())) => return,
            Ok(Err(e)) => FailureKind::Error(e),
            Err(p) => FailureKind::Panic(panic_message(p.as_ref())),
        };
        report_failure(&owner, HandlerFailure::new(event, shard, kind));
    });
}

/// Counts a failure, logs it, and hands it to `on_handler_error`. A panic inside that callback is the
/// user's own, and is contained by the task.
pub fn report_failure(client: &Client, failure: HandlerFailure) {
    match &failure.kind {
        FailureKind::Panic(_) => client.0.stats.record_handler_panic(),
        _ => client.0.stats.record_handler_error(),
    }
    tracing::error!(event = failure.event, shard = failure.shard, failure = %failure.kind, "handler failed");
    if let Some(callback) = &client.0.handlers.on_failure {
        callback(failure);
    }
}

/// The text of a panic payload, empty when it is not a string.
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}
