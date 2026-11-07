//! In-memory Discord state. No async runtime: everything here is sync and pure.
//!
//! Values are immutable `Arc<T>` snapshots: an update builds a new value, and `old` is what the swap
//! returned. Each shard's ingest task is the only writer for its guilds. No guard leaves this crate,
//! and no closure runs under a lock. See `ARCHITECTURE.md` 10.3.

pub mod apply;
pub mod permissions;
pub mod policy;
pub mod sharded;
pub mod store;
pub mod table;

pub use apply::Change;
pub use policy::{CacheConfig, Limit};
pub use sharded::Sharded;
pub use store::Cache;
pub use table::Table;

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Cache>();
};
