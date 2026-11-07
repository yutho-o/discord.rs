//! The failure table of `ARCHITECTURE.md` 10.2, and cancellation at every await.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use bytes::Bytes;
use dr_core::Secret;
use dr_model::{ChannelId, InteractionId};
use dr_rest::{
    Empty, Method, PreparedRequest, RawResponse, Rest, RestError, RetryClass, Transport, TransportError,
    TransportErrorKind,
};
use futures::future::BoxFuture;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[derive(Clone, Debug)]
enum Step {
    Status(u16),
    Response(u16, Vec<(&'static str, &'static str)>, &'static str),
    Fail(TransportErrorKind),
    /// Waits for a permit from the gate before answering 200.
    Gated,
}

/// Plays a script of answers, then 200 forever.
struct Script {
    steps: Mutex<VecDeque<Step>>,
    sends: AtomicU32,
    gate: Arc<Semaphore>,
}

impl Script {
    fn new(steps: impl IntoIterator<Item = Step>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(steps.into_iter().collect()),
            sends: AtomicU32::new(0),
            gate: Arc::new(Semaphore::new(0)),
        })
    }
    fn sends(&self) -> u32 {
        self.sends.load(Ordering::SeqCst)
    }
}

fn ok() -> RawResponse {
    RawResponse { status: 200, headers: Vec::new(), body: Bytes::from_static(b"{}") }
}

impl Transport for Script {
    fn send(&self, _req: PreparedRequest) -> BoxFuture<'static, Result<RawResponse, TransportError>> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let step = self.steps.lock().pop_front();
        let gate = self.gate.clone();
        Box::pin(async move {
            match step {
                None => Ok(ok()),
                Some(Step::Status(status)) => Ok(RawResponse { status, ..ok() }),
                Some(Step::Response(status, headers, body)) => Ok(RawResponse {
                    status,
                    headers: headers.into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect(),
                    body: Bytes::from_static(body.as_bytes()),
                }),
                Some(Step::Fail(kind)) => Err(TransportError { kind, message: "scripted".to_owned() }),
                Some(Step::Gated) => {
                    let permit = gate.acquire().await;
                    drop(permit);
                    Ok(ok())
                }
            }
        })
    }
}

fn rest(script: &Arc<Script>) -> Rest {
    Rest::builder().token(Secret::new("t".to_owned())).transport(script.clone()).retries(1).build().unwrap()
}

#[tokio::test]
async fn retry_class_matrix() {
    use RetryClass::{Callback, Idempotent, Protected, Unprotected};
    use TransportErrorKind::{Ambiguous, NotSent};
    // The class, the first answer, and whether a second attempt follows.
    let table: &[(RetryClass, Step, bool)] = &[
        (Idempotent, Step::Fail(NotSent), true),
        (Idempotent, Step::Fail(Ambiguous), true),
        (Idempotent, Step::Status(500), true),
        (Idempotent, Step::Status(502), true),
        (Unprotected, Step::Fail(NotSent), true),
        (Unprotected, Step::Fail(Ambiguous), false),
        (Unprotected, Step::Status(500), false),
        (Unprotected, Step::Status(502), false),
        (Protected, Step::Fail(NotSent), true),
        (Protected, Step::Fail(Ambiguous), true),
        (Protected, Step::Status(500), false),
        (Protected, Step::Status(502), true),
        (Callback, Step::Fail(NotSent), true),
        (Callback, Step::Fail(Ambiguous), true),
        (Callback, Step::Status(500), false),
        (Callback, Step::Status(502), true),
    ];
    for (class, first, retried) in table {
        let script = Script::new([first.clone()]);
        let result = rest(&script).raw::<Value>(Method::Post, "/x").retry_class(*class).await;
        let expected = if *retried { 2 } else { 1 };
        assert_eq!(script.sends(), expected, "{class:?} after {first:?}");
        assert_eq!(result.is_ok(), *retried, "{class:?} after {first:?}: {result:?}");
    }
}

#[tokio::test]
async fn a_post_defaults_to_no_resend_after_an_ambiguous_failure() {
    let script = Script::new([Step::Fail(TransportErrorKind::Ambiguous)]);
    let result = rest(&script).raw::<Value>(Method::Post, "/x").await;
    assert!(result.is_err());
    assert_eq!(script.sends(), 1, "a POST must not be duplicated");
}

#[tokio::test]
async fn a_40060_after_an_ambiguous_callback_counts_as_success() {
    let script = Script::new([
        Step::Fail(TransportErrorKind::Ambiguous),
        Step::Response(400, vec![], r#"{"code":40060,"message":"Interaction has already been acknowledged."}"#),
    ]);
    let rest = rest(&script);
    let token = Secret::new("tok".to_owned());
    let result: Result<Empty, RestError> =
        rest.interactions().callback(InteractionId::new(1), &token, json!({"type": 6})).await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(script.sends(), 2);
}

#[tokio::test]
async fn a_40060_with_no_ambiguous_attempt_is_an_error() {
    let script = Script::new([Step::Response(
        400,
        vec![],
        r#"{"code":40060,"message":"Interaction has already been acknowledged."}"#,
    )]);
    let rest = rest(&script);
    let token = Secret::new("tok".to_owned());
    let result = rest.interactions().callback(InteractionId::new(1), &token, json!({"type": 6})).await;
    assert!(result.is_err());
}

const RATE_LIMITED: &str = r#"{"message":"You are being rate limited.","retry_after":0.2,"global":false}"#;

#[tokio::test]
async fn a_429_waits_and_does_not_count_as_a_retry() {
    let headers = vec![
        ("retry-after", "1"),
        ("x-ratelimit-bucket", "b"),
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset-after", "0.2"),
    ];
    let script =
        Script::new([Step::Response(429, headers.clone(), RATE_LIMITED), Step::Response(429, headers, RATE_LIMITED)]);
    let start = Instant::now();
    let result = rest(&script).raw::<Value>(Method::Post, "/channels/339942739275677727/messages").await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(script.sends(), 3, "two 429s and the real one, with retries set to 1");
    assert!(start.elapsed() >= Duration::from_millis(300), "{:?}", start.elapsed());
}

#[tokio::test]
async fn an_empty_bucket_makes_the_next_request_wait() {
    let headers = vec![
        ("x-ratelimit-bucket", "b"),
        ("x-ratelimit-limit", "1"),
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset-after", "0.3"),
    ];
    let script = Script::new([Step::Response(200, headers, "{}")]);
    let rest = rest(&script);
    let path = "/channels/339942739275677727/messages";
    rest.raw::<Value>(Method::Post, path).await.unwrap();
    let start = Instant::now();
    rest.raw::<Value>(Method::Post, path).await.unwrap();
    assert!(start.elapsed() >= Duration::from_millis(250), "{:?}", start.elapsed());
}

#[tokio::test]
async fn a_global_429_holds_every_route() {
    let headers = vec![("retry-after", "1"), ("x-ratelimit-global", "true")];
    let body = r#"{"message":"You are being rate limited.","retry_after":0.3,"global":true}"#;
    let script = Script::new([Step::Response(429, headers, body)]);
    let rest = rest(&script);
    let first = rest.raw::<Value>(Method::Get, "/users/1");
    let start = Instant::now();
    let (a, b) = tokio::join!(std::future::IntoFuture::into_future(first), async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        rest.raw::<Value>(Method::Get, "/guilds/222078108977594368").await
    });
    a.unwrap();
    b.unwrap();
    assert!(start.elapsed() >= Duration::from_millis(250), "{:?}", start.elapsed());
}

#[tokio::test]
async fn a_401_stops_everything_that_uses_the_token() {
    let script = Script::new([Step::Response(401, vec![], r#"{"code":0,"message":"401: Unauthorized"}"#)]);
    let rest = rest(&script);
    assert!(rest.raw::<Value>(Method::Get, "/users/1").await.is_err());
    let next = rest.raw::<Value>(Method::Get, "/users/2").await;
    assert!(matches!(next, Err(RestError::TokenInvalidated)), "{next:?}");
    assert_eq!(script.sends(), 1, "the second never left");
}

/// Runs `f` to the given point, then drops it, and checks the route still works afterwards.
#[tokio::test]
async fn cancel_at_every_await() {
    let path = "/channels/339942739275677727/messages";

    // In the send.
    let script = Script::new([Step::Gated]);
    let rest_a = rest(&script);
    let inflight = tokio::spawn(std::future::IntoFuture::into_future(rest_a.raw::<Value>(Method::Post, path)));
    while script.sends() == 0 {
        tokio::task::yield_now().await;
    }
    inflight.abort();
    assert!(inflight.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(5), rest_a.raw::<Value>(Method::Post, path).into_future())
        .await
        .expect("no permit was left behind by a cancelled send")
        .unwrap();
    assert_eq!(rest_a.limiter().queued(), 0);

    // In the queue behind another request on the same bucket.
    let script = Script::new([Step::Gated]);
    let rest_b = rest(&script);
    let first = tokio::spawn(std::future::IntoFuture::into_future(rest_b.raw::<Value>(Method::Post, path)));
    while script.sends() == 0 {
        tokio::task::yield_now().await;
    }
    let queued = tokio::spawn(std::future::IntoFuture::into_future(rest_b.raw::<Value>(Method::Post, path)));
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(script.sends(), 1, "the second waits its turn");
    queued.abort();
    assert!(queued.await.unwrap_err().is_cancelled());
    script.gate.add_permits(1);
    first.await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(5), rest_b.raw::<Value>(Method::Post, path).into_future())
        .await
        .expect("a cancelled waiter kept no place in the queue")
        .unwrap();
    assert_eq!(rest_b.limiter().queued(), 0);

    // While sleeping after a 429: the limiter stays usable on other routes.
    let headers = vec![
        ("retry-after", "30"),
        ("x-ratelimit-bucket", "b"),
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset-after", "30"),
    ];
    let script = Script::new([Step::Response(429, headers, r#"{"message":"x","retry_after":30,"global":false}"#)]);
    let rest_c = rest(&script);
    let sleeping = tokio::spawn(std::future::IntoFuture::into_future(rest_c.raw::<Value>(Method::Post, path)));
    while script.sends() == 0 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    sleeping.abort();
    assert!(sleeping.await.unwrap_err().is_cancelled());
    let other = ChannelId::new(392_063_687_801_700_356);
    tokio::time::timeout(Duration::from_secs(5), rest_c.channels().get(other).into_future())
        .await
        .expect("another route is not held up")
        .ok();
    assert_eq!(rest_c.limiter().queued(), 0);
}
