//! The pipeline's promises: a handler cannot hurt the shard, and shutdown is orderly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{message, wait_until};
use discord::prelude::*;
use dr_mock::MockServer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::test]
async fn panic_in_handler_keeps_shard_alive() {
    let server = MockServer::start().await.unwrap();
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let good = Arc::new(AtomicU32::new(0));
    let counted = good.clone();
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .intents(Intents::GUILD_MESSAGES)
        .on(MessageCreate, move |_ctx, msg| {
            let counted = counted.clone();
            async move {
                assert_ne!(msg.content, "panic", "on purpose");
                counted.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    for text in ["panic", "panic", "fine"] {
        server.gateway().dispatch("MESSAGE_CREATE", message(server.world(), guild, channel, 7, text));
    }
    wait_until("the handler after the panics", || good.load(Ordering::SeqCst) == 1).await;
    assert_eq!(client.stats().handler_panics(), 2);
    assert_eq!(server.gateway().connections(), 1, "the shard never reconnected");
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn slow_handler_does_not_stop_heartbeats() {
    let server = MockServer::start().await.unwrap();
    server.gateway().heartbeat_interval(Duration::from_millis(40));
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let (release, wait) = tokio::sync::watch::channel(false);
    let started = Arc::new(AtomicU32::new(0));
    let running = started.clone();
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .intents(Intents::GUILD_MESSAGES)
        .shutdown_drain(Duration::from_secs(5))
        .on(MessageCreate, move |_ctx, _msg| {
            let mut wait = wait.clone();
            let running = running.clone();
            async move {
                running.fetch_add(1, Ordering::SeqCst);
                wait.wait_for(|go| *go).await?;
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    server.gateway().dispatch("MESSAGE_CREATE", message(server.world(), guild, channel, 7, "slow"));
    wait_until("the slow handler", || started.load(Ordering::SeqCst) == 1).await;
    let beats = || server.frames(0).iter().filter(|f| f.op == 1).count();
    let before = beats();
    wait_until("three heartbeats during the handler", || beats() >= before + 3).await;
    // The events behind the slow one still flow too.
    server.gateway().dispatch("MESSAGE_CREATE", message(server.world(), guild, channel, 7, "next"));
    wait_until("the next handler", || started.load(Ordering::SeqCst) == 2).await;
    assert_eq!(server.gateway().connections(), 1, "no zombie reconnect");
    release.send(true).unwrap();
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_missed_ack_reconnects_and_resumes() {
    let server = MockServer::start().await.unwrap();
    server.gateway().heartbeat_interval(Duration::from_millis(40));
    server.gateway().drop_acks(1);
    let runner = Client::builder("t").endpoints(server.endpoints()).build().unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("the zombie reconnect", || server.frames(0).iter().any(|f| f.op == 6)).await;
    assert_eq!(server.frames(0).iter().filter(|f| f.op == 2).count(), 1);
    wait_until("a latency reading", || client.shard_latency(0).is_some()).await;
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn shutdown_waits_for_handlers_and_is_idempotent() {
    let server = MockServer::start().await.unwrap();
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let finished = Arc::new(AtomicU32::new(0));
    let done = finished.clone();
    let started = Arc::new(AtomicU32::new(0));
    let running = started.clone();
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .intents(Intents::GUILD_MESSAGES)
        .on(MessageCreate, move |_ctx, _msg| {
            let done = done.clone();
            let running = running.clone();
            async move {
                running.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await;
                done.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    server.gateway().dispatch("MESSAGE_CREATE", message(server.world(), guild, channel, 7, "x"));
    wait_until("the handler", || started.load(Ordering::SeqCst) == 1).await;
    tokio::join!(client.shutdown(ShutdownMode::Final), client.shutdown(ShutdownMode::Final));
    assert_eq!(finished.load(Ordering::SeqCst), 1, "shutdown waited for the handler");
    run.await.unwrap().unwrap();
    client.shutdown(ShutdownMode::Final).await;
}

#[tokio::test]
async fn run_until_stops_when_the_future_completes() {
    let server = MockServer::start().await.unwrap();
    let runner = Client::builder("t").endpoints(server.endpoints()).build().unwrap();
    let stop = tokio::time::sleep(Duration::from_millis(200));
    tokio::time::timeout(Duration::from_secs(20), runner.run_until(stop)).await.unwrap().unwrap();
    assert!(server.frames(0).iter().any(|f| f.op == 2), "it did connect first");
}
