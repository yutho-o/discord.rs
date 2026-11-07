//! The in-flight budget.

use crate::client::DispatchMode;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Which budget a handler counts against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Ordinary events.
    Events,
    /// Interactions, so that a flood of events cannot starve the 3 s reply.
    Interactions,
}

/// Bounds how many handlers run at once.
#[derive(Debug)]
pub struct Budget {
    events: Arc<Semaphore>,
    interactions: Arc<Semaphore>,
}

impl Budget {
    /// A budget for this mode. `OrderedPerShard` is one at a time in each lane.
    pub fn new(mode: DispatchMode) -> Self {
        let (events, interactions) = match mode {
            DispatchMode::Concurrent { max_in_flight, interactions } => (max_in_flight, interactions),
            DispatchMode::OrderedPerShard => (1, 1),
        };
        Self { events: Arc::new(Semaphore::new(events)), interactions: Arc::new(Semaphore::new(interactions)) }
    }

    /// Waits for room in a lane. `None` once the budget is closed for shutdown.
    pub async fn acquire(&self, lane: Lane) -> Option<OwnedSemaphorePermit> {
        let semaphore = match lane {
            Lane::Events => &self.events,
            Lane::Interactions => &self.interactions,
        };
        semaphore.clone().acquire_owned().await.ok()
    }

    /// Stops handing out room: later `acquire` calls give `None`. Handlers already running finish.
    pub fn close(&self) {
        self.events.close();
        self.interactions.close();
    }
}
