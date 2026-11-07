//! What crosses the seam between the socket task and the rest of the client.

use compact_str::CompactString;
use dr_model::{DispatchKind, UpdatePresence};
use futures::future::BoxFuture;
use serde_json::value::RawValue;
use std::ops::Range;
use std::time::Duration;

/// One dispatch, handed downstream after the sequence advanced. The body is not decoded yet.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Which shard it came from.
    pub shard: u32,
    /// Which event, from `t`.
    pub kind: DispatchKind,
    /// The event name.
    pub t: CompactString,
    /// The whole text of the frame. Handing over the buffer avoids a copy of the body.
    pub buf: Vec<u8>,
    /// Where `d` is inside `buf`.
    pub d: Range<usize>,
}

impl Frame {
    /// The raw text of `d`. `None` only if the frame was built with a bad range or bad UTF-8, which
    /// a shard never does.
    pub fn d_str(&self) -> Option<&str> {
        self.buf.get(self.d.clone()).and_then(|b| std::str::from_utf8(b).ok())
    }
}

/// Whether the socket task may keep reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep reading.
    Continue,
    /// Stop reading until [`FrameSink::wait_capacity`] completes. Heartbeats continue.
    Pause,
}

/// Where a shard sends what it reads. The facade's queue implements this.
pub trait FrameSink: Send + Sync + 'static {
    /// Takes a frame. Called after the sequence advanced. Must not block.
    fn push(&self, frame: Frame) -> Flow;
    /// Completes once reading may continue after [`Flow::Pause`].
    fn wait_capacity(&self) -> BoxFuture<'_, ()>;
    /// Reports a lifecycle change. Must not block.
    fn notice(&self, notice: ShardNotice);
}

/// A lifecycle change of a shard.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ShardNotice {
    /// `READY` arrived.
    Ready {
        /// The shard.
        shard: u32,
    },
    /// `RESUMED` arrived.
    Resumed {
        /// The shard.
        shard: u32,
    },
    /// The connection ended, and the shard will reconnect.
    Disconnected {
        /// The shard.
        shard: u32,
        /// The close code, if there was one.
        code: Option<u16>,
    },
    /// A terminal close code: the shard has stopped.
    Failed {
        /// The shard.
        shard: u32,
        /// The close code.
        code: u16,
    },
    /// The queue over its soft limit shed events.
    Overload {
        /// The shard.
        shard: u32,
        /// Events shed so far.
        shed: u64,
    },
    /// The identify budget is low, so the manager is waiting for it to reset.
    IdentifyBudgetLow {
        /// Identifies remaining.
        remaining: u32,
    },
    /// Discord refused a gateway command with `RATE_LIMITED`.
    RateLimited {
        /// The shard.
        shard: u32,
        /// The refused opcode.
        opcode: u32,
        /// How long to wait.
        retry_after: Duration,
        /// The nonce of the request, if it had one.
        nonce: Option<String>,
    },
}

/// Where a shard is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShardStatus {
    /// Not started.
    Idle,
    /// Opening a connection, or waiting to reconnect.
    Connecting,
    /// Sent identify, waiting for `READY`.
    Identifying,
    /// Sent resume, waiting for `RESUMED`.
    Resuming,
    /// Connected and serving.
    Ready,
    /// Stopped for good on this close code.
    Failed(u16),
}

/// How to end a shard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShutdownMode {
    /// Close 1000: the session ends. The default.
    #[default]
    Final,
    /// Close 4200 and flush the session store, so the next start can resume.
    Resumable,
}

/// Something to send to Discord over the gateway. FIFO, and it survives reconnects.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum GatewayCommand {
    /// Op 3.
    UpdatePresence(UpdatePresence),
    /// Any other opcode. The 4096 byte limit still applies.
    Raw {
        /// The opcode.
        op: u32,
        /// The body, as JSON.
        d: Box<RawValue>,
    },
}
