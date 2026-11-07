//! The REST half.

pub mod handlers;
pub mod limits;
pub mod router;

use crate::{Fault, World};
use axum::http::Method;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::net::TcpListener;

/// One request as received. Tests assert on these.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RecordedRequest {
    /// The method.
    pub method: Method,
    /// The path under `/api/v10`, starting with `/`, encoded as it was sent.
    pub path: String,
    /// The query string, without the `?`.
    pub query: Option<String>,
    /// The headers, with lower case names.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl RecordedRequest {
    /// The first header with this lower case name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The body as JSON, `Null` if it is not.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// What the REST half keeps, between the router and the scripts.
#[derive(Debug)]
pub struct RestState {
    pub(crate) world: Arc<World>,
    pub(crate) gateway_url: String,
    pub(crate) requests: Mutex<Vec<RecordedRequest>>,
    pub(crate) rules: Mutex<Vec<Rule>>,
}

impl RestState {
    pub(crate) fn new(world: Arc<World>, gateway_url: String) -> Self {
        Self { world, gateway_url, requests: Mutex::new(Vec::new()), rules: Mutex::new(Vec::new()) }
    }

    pub(crate) fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().clone()
    }

    /// The action of the first rule for this request, using up one of its uses.
    pub(crate) fn take_rule(&self, method: &Method, path: &str) -> Option<Action> {
        let mut rules = self.rules.lock();
        let index = rules.iter().position(|r| r.method == *method && r.path == path)?;
        let rule = rules.get_mut(index)?;
        let action = rule.action.clone();
        if let Some(times) = &mut rule.times {
            *times = times.saturating_sub(1);
            if *times == 0 {
                rules.remove(index);
            }
        }
        Some(action)
    }
}

/// What a rule does.
#[derive(Clone, Debug)]
pub(crate) enum Action {
    Respond { status: u16, headers: Vec<(String, String)>, body: Vec<u8> },
    Fault(Fault),
}

/// An installed rule.
#[derive(Debug)]
pub(crate) struct Rule {
    method: Method,
    path: String,
    times: Option<u32>,
    action: Action,
}

/// Serves until the task is aborted. A failure of the listener ends the task.
pub(crate) async fn serve(listener: TcpListener, state: Arc<RestState>) {
    let app = router::build(state);
    if let Err(e) = axum::serve(listener, app).await {
        tracing::warn!(error = %e, "mock REST listener ended");
    }
}

/// Scripts REST behaviour. A rule takes priority over the routes while it has uses left.
#[derive(Clone, Copy, Debug)]
pub struct RestScript<'a> {
    state: &'a RestState,
}

impl<'a> RestScript<'a> {
    pub(crate) fn new(state: &'a RestState) -> Self {
        Self { state }
    }

    /// Starts a rule for requests with this method and path under `/api/v10`.
    pub fn on(self, method: Method, path: impl Into<String>) -> RestRule<'a> {
        RestRule { state: self.state, method, path: path.into(), times: None }
    }
}

/// One rule being built. Nothing is installed until [`RestRule::respond`] or [`RestRule::fault`].
#[derive(Debug)]
#[must_use = "a rule is installed by `respond` or `fault`"]
pub struct RestRule<'a> {
    state: &'a RestState,
    method: Method,
    path: String,
    times: Option<u32>,
}

impl RestRule<'_> {
    /// Applies the rule to the next `n` matching requests, and then it is gone. The default is
    /// every matching request.
    pub fn times(mut self, n: u32) -> Self {
        self.times = Some(n);
        self
    }

    /// Installs a canned response. Scripted 429 and 5xx responses go through here.
    pub fn respond(self, status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) {
        let headers = headers.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        self.install(Action::Respond { status, headers, body: body.into() });
    }

    /// Installs a fault.
    pub fn fault(self, fault: Fault) {
        self.install(Action::Fault(fault));
    }

    fn install(self, action: Action) {
        self.state.rules.lock().push(Rule { method: self.method, path: self.path, times: self.times, action });
    }
}
