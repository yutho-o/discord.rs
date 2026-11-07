//! Where to connect. `gateway` does not depend on `rest`: the facade implements the trait over it.

use dr_core::BoxError;
use futures::future::BoxFuture;
use std::time::Duration;

/// The answer of `GET /gateway/bot`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct GatewayInfo {
    /// The WebSocket URL.
    pub url: String,
    /// The shard count Discord recommends. The proof of concept runs one whatever this says.
    pub recommended_shards: u32,
    /// Identifies left in the budget.
    pub remaining: u32,
    /// Size of the budget.
    pub total: u32,
    /// When the budget resets.
    pub reset_after: Duration,
    /// Shards that may identify at once.
    pub max_concurrency: u32,
}

impl GatewayInfo {
    /// Info with the given URL and an ample budget, for one shard.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            recommended_shards: 1,
            remaining: 1000,
            total: 1000,
            reset_after: Duration::ZERO,
            max_concurrency: 1,
        }
    }
}

/// Finds out where to connect.
pub trait GatewayInfoProvider: Send + Sync + 'static {
    /// Fetches the current gateway info.
    fn gateway_info(&self) -> BoxFuture<'_, Result<GatewayInfo, BoxError>>;
}
