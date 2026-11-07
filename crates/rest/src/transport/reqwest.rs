//! The default transport. This module shadows the crate of the same name, so the crate is
//! `::reqwest` in here.

use super::{PreparedRequest, RawResponse, Transport, TransportError, TransportErrorKind};
use crate::RestError;
use crate::request::Method;
use futures::future::BoxFuture;
use std::time::Duration;

/// How long reading the body may take once the headers arrived.
const BODY_TIMEOUT: Duration = Duration::from_secs(30);

/// Sends through `reqwest`, with the TLS config of [`super::tls`] and redirects off. Timeouts wrap
/// the send future for the headers only, `ARCHITECTURE.md` 10.2, with no total timeout in reqwest.
/// Maps connect, DNS and TLS failures to `NotSent`, and everything else to `Ambiguous`.
#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: ::reqwest::Client,
}

fn map_error(e: ::reqwest::Error) -> TransportError {
    let kind = if e.is_connect() { TransportErrorKind::NotSent } else { TransportErrorKind::Ambiguous };
    // `without_url`: an interaction URL holds a token.
    TransportError { kind, message: e.without_url().to_string() }
}

impl ReqwestTransport {
    /// Builds the client. A TLS configuration failure becomes `RestError::Network` with `NotSent`.
    pub fn new() -> Result<Self, RestError> {
        let tls = super::tls::tls_http()
            .map_err(|e| TransportError { kind: TransportErrorKind::NotSent, message: e.to_string() })?;
        let client = ::reqwest::Client::builder()
            .tls_backend_preconfigured((*tls).clone())
            .redirect(::reqwest::redirect::Policy::none())
            .build()
            .map_err(map_error)?;
        Ok(Self { client })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, req: PreparedRequest) -> BoxFuture<'static, Result<RawResponse, TransportError>> {
        let method = match req.method {
            Method::Get => ::reqwest::Method::GET,
            Method::Post => ::reqwest::Method::POST,
            Method::Put => ::reqwest::Method::PUT,
            Method::Patch => ::reqwest::Method::PATCH,
            Method::Delete => ::reqwest::Method::DELETE,
        };
        let mut builder = self.client.request(method, &req.url);
        for (name, value) in req.headers {
            builder = builder.header(name, value);
        }
        // An empty POST, PUT or PATCH still sends `Content-Length: 0`, which a `Bytes` body does.
        if matches!(req.method, Method::Post | Method::Put | Method::Patch) || !req.body.is_empty() {
            builder = builder.body(req.body);
        }
        let header_limit = req.timeout;
        Box::pin(async move {
            let response = tokio::time::timeout(header_limit, builder.send())
                .await
                .map_err(|_| TransportError {
                    kind: TransportErrorKind::Ambiguous,
                    message: String::from("no response headers before the timeout"),
                })?
                .map_err(map_error)?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
                .collect();
            let body = tokio::time::timeout(BODY_TIMEOUT, response.bytes())
                .await
                .map_err(|_| TransportError {
                    kind: TransportErrorKind::Ambiguous,
                    message: String::from("the response body stalled"),
                })?
                .map_err(map_error)?;
            Ok(RawResponse { status, headers, body })
        })
    }
}
