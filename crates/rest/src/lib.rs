//! Rate limited Discord REST client. It has no dependency on the gateway, the cache or the builders:
//! bodies arrive through [`dr_core::IntoBody`].
//!
//! One non-generic path, [`Rest::execute`], owns the limiter, the retry loop and the error mapping.
//! [`Request`] adds only a thin generic decode tail.

pub mod api;
pub mod client;
pub mod encode;
pub mod error;
pub mod limit;
pub mod request;
pub mod retry;
pub mod route;
pub mod transport;

pub use client::{RejectOnRateLimit, Rest, RestBuilder, RestConfig};
pub use error::{FieldError, RateLimitInfo, RequestSummary, RestError};
pub use request::{Auth, Empty, Method, Request, RequestOptions, RequestParts};
pub use retry::RetryClass;
pub use route::RouteInfo;
pub use transport::{PreparedRequest, RawResponse, ReqwestTransport, Transport, TransportError, TransportErrorKind};

const _: fn() = || {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Rest>();
    assert_send::<Request<dr_model::Message>>();
    assert_send::<RestError>();
};
