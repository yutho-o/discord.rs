//! The shard over a real socket to the mock, uncompressed unless the `zlib` feature is on.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::Secret;
use dr_gateway::shard::close::{CloseAction, action};
use dr_gateway::{
    Compression, Flow, Frame, FrameSink, GatewayCommand, GatewayError, MemorySessionStore, SessionStore, ShardManager,
    ShardNotice, ShardStatus, ShardsHandle, ShutdownMode,
};
use dr_mock::MockServer;
use dr_model::Intents;
use futures::future::BoxFuture;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Sink {
    frames: Mutex<Vec<(String, String)>>,
    notices: Mutex<Vec<ShardNotice>>,
}

impl FrameSink for Sink {
    fn push(&self, frame: Frame) -> Flow {
        let body = frame.d_str().unwrap_or_default().to_owned();
        self.frames.lock().push((frame.t.to_string(), body));
        Flow::Continue
    }
    fn wait_capacity(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
    fn notice(&self, notice: ShardNotice) {
        self.notices.lock().push(notice);
    }
}

impl Sink {
    fn names(&self) -> Vec<String> {
        self.frames.lock().iter().map(|(t, _)| t.clone()).collect()
    }
}

async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn start(server: &MockServer, token: &str) -> (Arc<Sink>, ShardsHandle) {
    let sink = Arc::new(Sink::default());
    let handle = ShardManager::builder(Secret::new(token.to_owned()), Intents::GUILDS)
        .endpoints(server.endpoints())
        .compression(Compression::None)
        .build()
        .start(sink.clone())
        .await
        .unwrap();
    (sink, handle)
}

async fn ready(server: &MockServer, token: &str) -> (Arc<Sink>, ShardsHandle) {
    let (sink, handle) = start(server, token).await;
    wait_until("READY", || sink.names().iter().any(|t| t == "READY")).await;
    (sink, handle)
}

#[test]
fn the_close_code_table() {
    for code in [4004, 4010, 4011, 4012, 4013, 4014] {
        assert_eq!(action(Some(code)), CloseAction::Fatal, "{code}");
    }
    for code in [1000, 4003, 4005, 4007, 4008] {
        assert_eq!(action(Some(code)), CloseAction::Identify, "{code}");
    }
    for code in [1001, 1006, 4000, 4001, 4002, 4009] {
        assert_eq!(action(Some(code)), CloseAction::Resume, "{code}");
    }
    assert_eq!(action(None), CloseAction::Resume);
}

#[tokio::test]
async fn identify_carries_what_it_should() {
    let server = MockServer::start().await.unwrap();
    let (_sink, handle) = ready(&server, "Bot abc").await;
    let frames = server.frames(0);
    let identify = frames.iter().find(|f| f.op == 2).unwrap();
    assert_eq!(identify.d["token"], "abc", "the prefix is stripped");
    assert_eq!(identify.d["intents"], 1);
    assert_eq!(identify.d["shard"], json!([0, 1]));
    assert_eq!(identify.d["properties"]["browser"], identify.d["properties"]["device"]);
    assert!(identify.d.get("compress").is_none() || identify.d["compress"] == false, "payload compression is off");
    assert_eq!(handle.status(0), ShardStatus::Ready);
    handle.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn resume_goes_on_from_the_last_sequence() {
    let server = MockServer::start().await.unwrap();
    let (sink, handle) = ready(&server, "abc").await;
    server.gateway().dispatch("TYPING_START", json!({"n": 1}));
    wait_until("the first event", || sink.names().iter().any(|t| t == "TYPING_START")).await;
    server.gateway().close(4000);
    wait_until("the resume", || server.frames(0).iter().any(|f| f.op == 6)).await;
    let frames = server.frames(0);
    let resume = frames.iter().find(|f| f.op == 6).unwrap();
    assert_eq!(resume.d["seq"], 2, "READY and the event");
    assert_eq!(resume.d["session_id"], "mock-session-1");
    assert_eq!(frames.iter().filter(|f| f.op == 2).count(), 1);
    wait_until("the status", || handle.status(0) == ShardStatus::Ready).await;
    handle.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn a_failed_resume_identifies_again() {
    let server = MockServer::start().await.unwrap();
    let (sink, handle) = ready(&server, "abc").await;
    server.gateway().invalid_session(false);
    wait_until("a second READY", || sink.names().iter().filter(|t| *t == "READY").count() == 2).await;
    assert_eq!(server.frames(0).iter().filter(|f| f.op == 2).count(), 2);
    handle.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn a_resumable_invalid_session_resumes() {
    let server = MockServer::start().await.unwrap();
    let (_sink, handle) = ready(&server, "abc").await;
    server.gateway().invalid_session(true);
    wait_until("the resume", || server.frames(0).iter().any(|f| f.op == 6)).await;
    assert_eq!(server.frames(0).iter().filter(|f| f.op == 2).count(), 1);
    handle.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn close_1000_identifies_and_a_terminal_code_stops() {
    let server = MockServer::start().await.unwrap();
    let (sink, handle) = ready(&server, "abc").await;
    server.gateway().close(1000);
    wait_until("a second identify", || server.frames(0).iter().filter(|f| f.op == 2).count() == 2).await;
    wait_until("a second READY", || sink.names().iter().filter(|t| *t == "READY").count() == 2).await;

    server.gateway().close(4014);
    wait_until("the failure", || {
        sink.notices.lock().iter().any(|n| matches!(n, ShardNotice::Failed { code: 4014, .. }))
    })
    .await;
    assert_eq!(handle.status(0), ShardStatus::Failed(4014));
    let result = handle.send(0, GatewayCommand::Raw { op: 3, d: serde_json::value::to_raw_value(&json!({})).unwrap() });
    assert!(matches!(result, Err(GatewayError::NotRunning)), "{result:?}");
}

#[tokio::test]
async fn a_bad_token_fails_at_once() {
    let server = MockServer::start().await.unwrap();
    let (sink, _handle) = start(&server, dr_mock::gateway::session::BAD_TOKEN).await;
    wait_until("the failure", || {
        sink.notices.lock().iter().any(|n| matches!(n, ShardNotice::Failed { code: 4004, .. }))
    })
    .await;
    assert_eq!(server.gateway().connections(), 1, "no retry on a terminal code");
}

#[tokio::test]
async fn commands_are_sent_once_ready() {
    let server = MockServer::start().await.unwrap();
    let (_sink, handle) = ready(&server, "abc").await;
    let d = serde_json::value::to_raw_value(&json!({"status": "idle"})).unwrap();
    handle.send(0, GatewayCommand::Raw { op: 3, d }).unwrap();
    wait_until("the command", || server.frames(0).iter().any(|f| f.op == 3)).await;
    let big = serde_json::value::to_raw_value(&"x".repeat(5000)).unwrap();
    let too_big = handle.send(0, GatewayCommand::Raw { op: 3, d: big });
    assert!(matches!(too_big, Err(GatewayError::PayloadTooLarge { .. })), "{too_big:?}");
    assert!(matches!(
        handle.send(1, GatewayCommand::Raw { op: 3, d: serde_json::value::to_raw_value(&1).unwrap() }),
        Err(GatewayError::UnknownShard { shard: 1 })
    ));
    handle.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn a_session_in_the_store_is_resumed_on_the_next_start() {
    let server = MockServer::start().await.unwrap();
    let store = Arc::new(MemorySessionStore::new());
    let sink = Arc::new(Sink::default());
    let manager = || {
        ShardManager::builder(Secret::new("abc".to_owned()), Intents::GUILDS)
            .endpoints(server.endpoints())
            .compression(Compression::None)
            .session_store(store.clone())
            .build()
    };
    let handle = manager().start(sink.clone()).await.unwrap();
    wait_until("READY", || sink.names().iter().any(|t| t == "READY")).await;
    handle.shutdown(ShutdownMode::Resumable).await;
    assert!(store.load(0).await.is_some(), "a resumable shutdown keeps the session");
    assert_eq!(server.frames(0).iter().filter(|f| f.op == 6).count(), 0);

    let handle = manager().start(sink.clone()).await.unwrap();
    wait_until("the resume", || server.frames(0).iter().any(|f| f.op == 6)).await;
    assert_eq!(server.frames(0).iter().filter(|f| f.op == 2).count(), 1);
    handle.shutdown(ShutdownMode::Final).await;
    assert!(store.load(0).await.is_none(), "a final shutdown forgets it");
}

#[cfg(feature = "zlib")]
#[tokio::test]
async fn zlib_frames_split_across_messages_still_decode() {
    let server = MockServer::start().await.unwrap();
    server.gateway().split_frames(3);
    let sink = Arc::new(Sink::default());
    let handle = ShardManager::builder(Secret::new("abc".to_owned()), Intents::GUILDS)
        .endpoints(server.endpoints())
        .compression(Compression::ZlibStream)
        .build()
        .start(sink.clone())
        .await
        .unwrap();
    wait_until("READY", || sink.names().iter().any(|t| t == "READY")).await;
    server.gateway().dispatch("TYPING_START", json!({"text": "x".repeat(10_000)}));
    wait_until("a large event", || sink.names().iter().any(|t| t == "TYPING_START")).await;
    handle.shutdown(ShutdownMode::Final).await;
}
