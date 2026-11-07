//! `Rest`, `RestBuilder` and `RestConfig`.

use crate::api::{ApplicationCommandsApi, ChannelsApi, GatewayApi, GuildsApi, InteractionsApi, UsersApi};
use crate::limit::Limiter;
use crate::request::{Method, Request, RequestParts};
use crate::transport::{RawResponse, ReqwestTransport, Transport};
use crate::{RestError, retry};
use dr_core::{Endpoints, RequestBody, Secret};
use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;

/// The `url` of the `User-Agent` form Discord requires, `DiscordBot (url, version)`. A placeholder:
/// the project has no public home yet.
const USER_AGENT_URL: &str = "https://github.com/discord-rs/discord.rs";

/// Which requests fail with `RateLimited` instead of waiting out a 429.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub enum RejectOnRateLimit {
    /// Always wait. The default.
    #[default]
    Never,
    /// Never wait.
    Always,
    /// Do not wait on a route starting with one of these prefixes.
    Routes(Vec<String>),
}

/// The settings a [`Rest`] was built with. The retry loop and the limiter read them.
#[derive(Clone, Debug)]
pub struct RestConfig {
    /// Bot token, sent on `Auth::Default` requests. Redacted in `Debug`.
    pub token: Option<Secret>,
    /// Base of every request, `Endpoints::api` plus `/v10`.
    pub api_base: String,
    /// Retries after the first attempt.
    pub retries: u32,
    /// Timeout per attempt, for the headers.
    pub timeout: Duration,
    /// Added to every rate limit wait, once.
    pub offset: Duration,
    /// Requests that may wait in the limiter before `QueueFull`.
    pub queue_limit: usize,
    /// Global requests per second.
    pub global_per_second: u32,
    /// Which requests fail instead of waiting out a 429.
    pub reject_on_rate_limit: RejectOnRateLimit,
    /// Invalid requests in the window before `CircuitOpen`.
    pub invalid_request_breaker: u32,
    /// `User-Agent` value.
    pub user_agent: String,
}

pub(crate) struct RestInner {
    pub(crate) config: RestConfig,
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) limiter: Limiter,
}

/// The REST client. Cheap to clone, every clone shares the limiter.
#[derive(Clone)]
pub struct Rest(pub(crate) Arc<RestInner>);

impl std::fmt::Debug for Rest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rest").field("config", &self.0.config).finish_non_exhaustive()
    }
}

impl Rest {
    /// A builder with the defaults of `ARCHITECTURE.md` 10.2.
    pub fn builder() -> RestBuilder {
        RestBuilder::default()
    }

    /// The one non-generic path: limiter, retry loop and error mapping, in [`retry::run`].
    pub fn execute(&self, parts: RequestParts) -> BoxFuture<'static, Result<RawResponse, RestError>> {
        Box::pin(retry::run(self.clone(), parts))
    }

    /// A request to any path under the API base. `path` is already encoded, and starts with `/`.
    pub fn raw<T: DeserializeOwned>(&self, method: Method, path: impl Into<String>) -> Request<T> {
        Request::new(self.clone(), method, path.into(), Ok(RequestBody::empty()))
    }

    /// The settings of this client.
    pub fn config(&self) -> &RestConfig {
        &self.0.config
    }

    /// The transport in use. The retry loop sends through this.
    pub fn transport(&self) -> &Arc<dyn Transport> {
        &self.0.transport
    }

    /// The limiter in use. The retry loop waits on this.
    pub fn limiter(&self) -> &Limiter {
        &self.0.limiter
    }

    /// Channel and message endpoints.
    pub fn channels(&self) -> ChannelsApi<'_> {
        ChannelsApi(self)
    }

    /// User endpoints.
    pub fn users(&self) -> UsersApi<'_> {
        UsersApi(self)
    }

    /// Gateway information endpoints.
    pub fn gateway(&self) -> GatewayApi<'_> {
        GatewayApi(self)
    }

    /// Guild and member endpoints.
    pub fn guilds(&self) -> GuildsApi<'_> {
        GuildsApi(self)
    }

    /// Interaction callback and webhook endpoints.
    pub fn interactions(&self) -> InteractionsApi<'_> {
        InteractionsApi(self)
    }

    /// Application command endpoints.
    pub fn application_commands(&self) -> ApplicationCommandsApi<'_> {
        ApplicationCommandsApi(self)
    }
}

/// Builds a [`Rest`].
#[derive(Clone, Default)]
pub struct RestBuilder {
    token: Option<Secret>,
    endpoints: Endpoints,
    transport: Option<Arc<dyn Transport>>,
    retries: Option<u32>,
    timeout: Option<Duration>,
    offset: Option<Duration>,
    queue_limit: Option<usize>,
    global_per_second: Option<u32>,
    reject_on_rate_limit: RejectOnRateLimit,
    invalid_request_breaker: Option<u32>,
    user_agent_appendix: Option<String>,
}

impl RestBuilder {
    /// Sets the bot token.
    pub fn token(mut self, token: Secret) -> Self {
        self.token = Some(token);
        self
    }
    /// Sets the base URLs. Tests point this at the mock.
    pub fn endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }
    /// Replaces the transport. The default is [`ReqwestTransport`].
    pub fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }
    /// Sets the retries after the first attempt. The default is 3.
    pub fn retries(mut self, n: u32) -> Self {
        self.retries = Some(n);
        self
    }
    /// Sets the timeout per attempt. The default is 15 s.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }
    /// Sets the offset added to every rate limit wait. The default is 50 ms.
    pub fn offset(mut self, d: Duration) -> Self {
        self.offset = Some(d);
        self
    }
    /// Sets the requests that may wait in the limiter. The default is 10 000.
    pub fn queue_limit(mut self, n: usize) -> Self {
        self.queue_limit = Some(n);
        self
    }
    /// Sets the global requests per second. The default is 50.
    pub fn global_per_second(mut self, n: u32) -> Self {
        self.global_per_second = Some(n);
        self
    }
    /// Sets which requests fail instead of waiting out a 429.
    pub fn reject_on_rate_limit(mut self, r: RejectOnRateLimit) -> Self {
        self.reject_on_rate_limit = r;
        self
    }
    /// Sets the invalid requests in the window before `CircuitOpen`. The default is 9000.
    pub fn invalid_request_breaker(mut self, n: u32) -> Self {
        self.invalid_request_breaker = Some(n);
        self
    }
    /// Adds text to the end of the `User-Agent` header.
    pub fn user_agent_appendix(mut self, s: impl Into<String>) -> Self {
        self.user_agent_appendix = Some(s.into());
        self
    }
    /// Builds the client. Creating the default transport can fail, which is the only failure here.
    pub fn build(self) -> Result<Rest, RestError> {
        let transport: Arc<dyn Transport> = match self.transport {
            Some(t) => t,
            None => Arc::new(ReqwestTransport::new()?),
        };
        let mut user_agent = format!("DiscordBot ({USER_AGENT_URL}, {})", env!("CARGO_PKG_VERSION"));
        if let Some(a) = &self.user_agent_appendix {
            user_agent.push(' ');
            user_agent.push_str(a);
        }
        let config = RestConfig {
            token: self.token,
            api_base: format!("{}/v10", self.endpoints.api.trim_end_matches('/')),
            retries: self.retries.unwrap_or(3),
            timeout: self.timeout.unwrap_or(Duration::from_secs(15)),
            offset: self.offset.unwrap_or(Duration::from_millis(50)),
            queue_limit: self.queue_limit.unwrap_or(10_000),
            global_per_second: self.global_per_second.unwrap_or(50),
            reject_on_rate_limit: self.reject_on_rate_limit,
            invalid_request_breaker: self.invalid_request_breaker.unwrap_or(9_000),
            user_agent,
        };
        let limiter = Limiter::new(&config);
        Ok(Rest(Arc::new(RestInner { config, transport, limiter })))
    }
}
