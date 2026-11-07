//! Between the socket task and the handlers, `ARCHITECTURE.md` 10.1.
//!
//! ```text
//! socket task -> FrameSink (queue) -> ingest task -> events::deliver -> dispatch -> handlers
//! ```

pub mod budget;
pub mod dispatch;
pub mod ingest;
pub mod queue;
pub mod ready;
pub mod shutdown;
