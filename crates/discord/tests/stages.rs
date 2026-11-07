//! Stage instances: start, edit, end, the events and the cache.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::model::JsonErrorCode;
use discord::prelude::*;
use dr_mock::{Method, MockServer};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn a_stage_goes_live_changes_topic_and_ends() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let stage_channel = world.add_stage_channel(guild, "Town hall");
    let text = world.add_channel(guild, "general");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS)
        .on(StageInstanceCreate, |ctx, s| async move {
            ctx.state.lock().push(format!("live: {}", s.topic));
            Ok(())
        })
        .on(StageInstanceUpdate, |ctx, s| async move {
            let old = s.old.as_ref().map_or("?", |o| o.topic.as_str()).to_owned();
            ctx.state.lock().push(format!("topic: {old} -> {}", s.new.topic));
            Ok(())
        })
        .on(StageInstanceDelete, |ctx, s| async move {
            ctx.state.lock().push(format!("ended, cached {}", s.cached.is_some()));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;
    let channel = client.channel(stage_channel);

    let stage =
        channel.start_stage(CreateStageInstance::new(stage_channel, "Q&A").notify()).reason("weekly").await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!((request.method.clone(), request.path.as_str()), (Method::POST, "/stage-instances"));
    assert_eq!(
        request.json(),
        json!({"channel_id": stage_channel.to_string(), "topic": "Q&A", "send_start_notification": true})
    );
    assert_eq!(stage.channel_id, stage_channel);
    wait_until("cached", || client.cache().stage_instance(guild, stage_channel).is_some()).await;
    assert_eq!(channel.fetch_stage().await.unwrap().topic, "Q&A");

    let error = channel.start_stage(CreateStageInstance::new(stage_channel, "again")).await.unwrap_err();
    assert!(error.code().is_some(), "{error:?}");
    let error = client.channel(text).start_stage(CreateStageInstance::new(text, "no")).await.unwrap_err();
    assert!(error.code().is_some(), "a text channel cannot go live: {error:?}");

    channel.edit_stage(EditStageInstance::new().topic("AMA")).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json(), json!({"topic": "AMA"}));
    channel.end_stage().await.unwrap();
    wait_until("not live", || client.cache().stage_instance(guild, stage_channel).is_none()).await;
    let error = channel.fetch_stage().await.unwrap_err();
    assert_eq!(error.code(), Some(JsonErrorCode::UNKNOWN_STAGE_INSTANCE));
    wait_until("three events", || seen.lock().len() == 3).await;
    assert_eq!(*seen.lock(), ["live: Q&A", "topic: Q&A -> AMA", "ended, cached true"]);

    let before = server.requests().len();
    let error = channel.edit_stage(EditStageInstance::new().topic("")).await.unwrap_err();
    assert!(matches!(error, Error::Validation(_)), "{error:?}");
    let long = "x".repeat(121);
    assert!(channel.start_stage(CreateStageInstance::new(stage_channel, long)).await.is_err());
    assert_eq!(server.requests().len(), before);
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}
