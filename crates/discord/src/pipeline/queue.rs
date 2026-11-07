//! The byte-budget queue between the socket task and the ingest task.

use crate::client::QueueLimits;
use dr_gateway::{Flow, Frame, FrameSink, ShardNotice};
use dr_model::DispatchKind;
use futures::future::BoxFuture;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Notify, watch};

#[derive(Debug, Default)]
struct Lanes {
    interactions: VecDeque<Frame>,
    events: VecDeque<Frame>,
    bytes: usize,
    closed: bool,
}

/// One shard's queue, with a priority lane for `INTERACTION_CREATE`. Below `shed_bytes` it is normal.
/// Above it, sheddable events are dropped and counted, and a `ShardNotice::Overload` goes out. Above
/// `pause_bytes`, `push` returns `Flow::Pause`.
#[derive(Debug)]
pub struct ShardQueue {
    limits: QueueLimits,
    lanes: Mutex<Lanes>,
    readable: Notify,
    capacity: Notify,
    shed: AtomicU64,
    failed: watch::Sender<Option<u16>>,
}

/// Events that say nothing the next one will not say again. Dropping one loses no state.
fn sheddable(kind: DispatchKind) -> bool {
    matches!(kind, DispatchKind::TypingStart | DispatchKind::PresenceUpdate)
}

impl ShardQueue {
    /// An empty queue.
    pub fn new(limits: QueueLimits) -> Self {
        Self {
            limits,
            lanes: Mutex::new(Lanes::default()),
            readable: Notify::new(),
            capacity: Notify::new(),
            shed: AtomicU64::new(0),
            failed: watch::Sender::new(None),
        }
    }

    /// The next frame, interactions first. `None` once the queue is closed and empty. Only the
    /// ingest task calls this.
    pub async fn next(&self) -> Option<Frame> {
        loop {
            {
                let mut lanes = self.lanes.lock();
                let frame = lanes.interactions.pop_front().or_else(|| lanes.events.pop_front());
                if let Some(frame) = frame {
                    lanes.bytes = lanes.bytes.saturating_sub(frame.buf.len());
                    if lanes.bytes < self.limits.shed_bytes {
                        self.capacity.notify_one();
                    }
                    return Some(frame);
                }
                if lanes.closed {
                    return None;
                }
            }
            // `notify_one` keeps a permit when nobody waits, so a push right here is not missed.
            self.readable.notified().await;
        }
    }

    /// Stops accepting frames. What is queued can still be read, for the shutdown drain.
    pub fn close(&self) {
        self.lanes.lock().closed = true;
        self.readable.notify_one();
        self.capacity.notify_one();
    }

    /// Takes the count of events shed since the last call.
    pub fn take_shed(&self) -> u64 {
        self.shed.swap(0, Ordering::Relaxed)
    }

    /// Bytes waiting.
    pub fn queued_bytes(&self) -> usize {
        self.lanes.lock().bytes
    }

    /// Follows the terminal close code of the shard, if it fails.
    pub fn failures(&self) -> watch::Receiver<Option<u16>> {
        self.failed.subscribe()
    }
}

impl FrameSink for ShardQueue {
    fn push(&self, frame: Frame) -> Flow {
        let shard = frame.shard;
        let kind = frame.kind;
        let size = frame.buf.len();
        let flow = {
            let mut lanes = self.lanes.lock();
            if lanes.closed {
                return Flow::Continue;
            }
            if lanes.bytes >= self.limits.shed_bytes && sheddable(kind) {
                drop(lanes);
                self.shed.fetch_add(1, Ordering::Relaxed);
                self.notice(ShardNotice::Overload { shard, shed: 1 });
                return Flow::Continue;
            }
            lanes.bytes = lanes.bytes.saturating_add(size);
            if kind == DispatchKind::InteractionCreate {
                lanes.interactions.push_back(frame);
            } else {
                lanes.events.push_back(frame);
            }
            if lanes.bytes >= self.limits.pause_bytes { Flow::Pause } else { Flow::Continue }
        };
        self.readable.notify_one();
        flow
    }

    fn wait_capacity(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            loop {
                {
                    let lanes = self.lanes.lock();
                    if lanes.closed || lanes.bytes < self.limits.shed_bytes {
                        return;
                    }
                }
                self.capacity.notified().await;
            }
        })
    }

    fn notice(&self, notice: ShardNotice) {
        match notice {
            ShardNotice::Failed { shard, code } => {
                tracing::error!(shard, code, "shard failed");
                self.failed.send_replace(Some(code));
            }
            ShardNotice::Overload { shard, shed } => tracing::warn!(shard, shed, "shard queue is shedding events"),
            other => tracing::debug!(notice = ?other, "shard notice"),
        }
    }
}
