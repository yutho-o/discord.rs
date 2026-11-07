//! The ingest task, one per shard: frame, typed decode, cache apply in strict order, handlers.

use super::queue::ShardQueue;
use crate::Client;
use crate::events;
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

/// Reads frames until the queue closes, and hands each to `events::deliver`, one at a time. That is
/// what keeps cache writes in event order. It waits only on room in the budget, never on a handler.
/// A panic while delivering one frame loses that frame and is logged: the session goes on.
pub async fn run(client: Client, queue: Arc<ShardQueue>) {
    while let Some(frame) = queue.next().await {
        let shed = queue.take_shed();
        if shed > 0 {
            client.0.stats.record_shed(shed);
        }
        let Some(d) = frame.d_str() else {
            client.0.stats.record_decode_failure();
            tracing::warn!(event = %frame.t, "dispatch body is not UTF-8");
            continue;
        };
        let delivered =
            AssertUnwindSafe(events::deliver(&client, frame.shard, frame.kind, &frame.t, d)).catch_unwind().await;
        if delivered.is_err() {
            tracing::error!(event = %frame.t, "delivering an event panicked");
        }
    }
}
