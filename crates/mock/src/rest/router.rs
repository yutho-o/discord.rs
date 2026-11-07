//! Under `/api/v10`: records every request, applies script rules, then the routes in `handlers`.

use super::{Action, RecordedRequest, RestState, handlers, limits};
use crate::Fault;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use std::panic::resume_unwind;
use std::sync::Arc;

const MAX_BODY: usize = 32 << 20;
const PREFIX: &str = "/api/v10";

/// Builds the router.
pub(crate) fn build(state: Arc<RestState>) -> Router {
    Router::new().fallback(handle).with_state(state)
}

/// Ends the connection with no answer. Unwinding out of the handler takes the connection's task with
/// it, and `resume_unwind` keeps the panic hook quiet.
fn reset() -> ! {
    resume_unwind(Box::new("mock connection reset"))
}

fn plain(status: StatusCode, content_type: &'static str, body: &'static str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap_or_default()
}

async fn handle(State(state): State<Arc<RestState>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, MAX_BODY).await.map(|b| b.to_vec()).unwrap_or_default();
    let full = parts.uri.path().to_owned();
    let path = full.strip_prefix(PREFIX).unwrap_or(&full).to_owned();
    let recorded = RecordedRequest {
        method: parts.method.clone(),
        path: path.clone(),
        query: parts.uri.query().map(str::to_owned),
        headers: parts
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
            .collect(),
        body,
    };
    state.requests.lock().push(recorded.clone());

    let mut after = None;
    match state.take_rule(&parts.method, &path) {
        Some(Action::Respond { status, headers, body }) => {
            let mut response = Response::builder().status(status);
            for (name, value) in headers {
                response = response.header(name, value);
            }
            return response.body(Body::from(body)).unwrap_or_default();
        }
        Some(Action::Fault(Fault::Latency(wait))) => tokio::time::sleep(wait).await,
        Some(Action::Fault(Fault::ResetBeforeResponse)) => reset(),
        Some(Action::Fault(Fault::NonJson5xx)) => {
            return plain(StatusCode::BAD_GATEWAY, "text/html", "<html>bad gateway</html>");
        }
        Some(Action::Fault(Fault::HeaderlessRateLimit)) => {
            let body = r#"{"message":"You are being rate limited.","retry_after":0.05,"global":false}"#;
            return plain(StatusCode::TOO_MANY_REQUESTS, "application/json", body);
        }
        Some(Action::Fault(Fault::CloudflareRateLimit)) => {
            let mut response = plain(StatusCode::TOO_MANY_REQUESTS, "text/html", "<html>error 1015</html>");
            response.headers_mut().insert(header::RETRY_AFTER, axum::http::HeaderValue::from_static("1"));
            return response;
        }
        Some(Action::Fault(fault)) => after = Some(fault),
        None => {}
    }

    let mut response = handlers::route(&state, &recorded).await;
    limits::add_headers(&mut response, parts.method.as_str(), &path);
    match after {
        Some(Fault::ResetAfterResponse) => reset(),
        Some(Fault::TruncatedBody) => {
            // A length past the end of the body makes hyper drop the connection mid-response.
            response.headers_mut().insert(header::CONTENT_LENGTH, axum::http::HeaderValue::from_static("1000000"));
            response
        }
        _ => response,
    }
}
