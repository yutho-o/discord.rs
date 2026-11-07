//! A loopback mock of the Discord REST API and gateway, the test kit of discord.rs. It never reaches
//! the network, and it runs on the tokio runtime of the test that starts it.
//!
//! Two ports: axum serves REST under `/api/v10`, and a tungstenite listener serves the gateway. Each
//! half keeps its own state, and they share only a [`World`].

pub mod fault;
pub mod gateway;
pub mod rest;
pub mod world;

pub use axum::http::Method;
pub use fault::Fault;
pub use gateway::{GatewayScript, RecordedFrame};
pub use rest::{RecordedRequest, RestRule, RestScript};
pub use world::{World, WorldEvent};

use dr_core::Endpoints;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// A running mock. Dropping it stops both listeners.
#[derive(Debug)]
pub struct MockServer {
    world: Arc<World>,
    rest: Arc<rest::RestState>,
    gateway: Arc<gateway::GatewayState>,
    api_url: String,
    gateway_url: String,
    tasks: Vec<JoinHandle<()>>,
}

impl MockServer {
    /// Binds two loopback ports, picked by the OS, and starts serving. A bind failure is the only
    /// failure.
    pub async fn start() -> std::io::Result<Self> {
        let world = Arc::new(World::new());
        let rest_listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let gateway_listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let api_url = format!("http://{}/api", rest_listener.local_addr()?);
        let gateway_url = format!("ws://{}", gateway_listener.local_addr()?);

        let rest = Arc::new(rest::RestState::new(world.clone(), gateway_url.clone()));
        let gateway = Arc::new(gateway::GatewayState::new(world.clone(), gateway_url.clone()));
        let tasks = vec![
            tokio::spawn(rest::serve(rest_listener, rest.clone())),
            tokio::spawn(gateway::serve(gateway_listener, gateway.clone())),
        ];
        Ok(Self { world, rest, gateway, api_url, gateway_url, tasks })
    }

    /// The URLs to hand to a client under test.
    pub fn endpoints(&self) -> Endpoints {
        Endpoints::custom(self.api_url.clone(), Some(self.gateway_url.clone()))
    }

    /// The in-memory Discord state, which both halves read.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Scripts REST behaviour: `server.rest().on(Method::GET, "/users/1").times(2).respond(...)`.
    pub fn rest(&self) -> RestScript<'_> {
        RestScript::new(&self.rest)
    }

    /// Scripts gateway behaviour: `server.gateway().close(4000)`.
    pub fn gateway(&self) -> GatewayScript<'_> {
        GatewayScript::new(&self.gateway)
    }

    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.rest.requests()
    }

    /// Every frame received from clients of this shard so far, in order.
    pub fn frames(&self, shard: u32) -> Vec<RecordedFrame> {
        self.gateway.frames(shard)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}
