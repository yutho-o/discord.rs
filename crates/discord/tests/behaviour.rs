//! The facade, with no server at all.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use discord::builders::{CommandList, UserContextCommand};
use discord::interactions::ReplyState;
use discord::prelude::*;
use discord::{Call, ConfigError, FailureKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

fn message_json() -> &'static str {
    r#"{"id":"10","channel_id":"20","author":{"id":"30","username":"u"},"content":"hi"}"#
}

#[test]
fn a_handler_that_can_never_fire_fails_the_build() {
    let err = Client::builder("t").on(MessageCreate, |_ctx, _msg| async move { Ok(()) }).build().unwrap_err();
    let Error::Config(ConfigError::MissingIntent { event, .. }) = &err else { panic!("{err:?}") };
    assert_eq!(*event, "MESSAGE_CREATE");
    assert!(err.to_string().contains("MESSAGE_CREATE"), "{err}");
}

#[test]
fn allow_missing_intents_turns_the_check_off() {
    let built =
        Client::builder("t").allow_missing_intents().on(MessageCreate, |_ctx, _msg| async move { Ok(()) }).build();
    assert!(built.is_ok());
}

#[test]
fn one_intent_of_several_is_enough() {
    let built = Client::builder("t")
        .intents(Intents::DIRECT_MESSAGES)
        .on(MessageCreate, |_ctx, _msg| async move { Ok(()) })
        .build();
    assert!(built.is_ok());
}

#[test]
fn bound_is_two_pointers_and_derefs_and_clones_cheaply() {
    assert_eq!(size_of::<Bound<Message>>(), 16);
    let client = Client::builder("t").build().unwrap().client();
    let msg: Message = serde_json::from_str(message_json()).unwrap();
    let bound = client.bind(msg);
    assert_eq!(bound.content, "hi");
    let copy = bound.clone();
    assert!(Arc::ptr_eq(bound.data(), copy.data()));
}

#[tokio::test]
async fn a_failed_call_surfaces_its_error_when_awaited() {
    let client = Client::builder("t").build().unwrap().client();
    let call: Call<Bound<Message>> = Call::failed(client, Error::NotReady);
    assert!(matches!(call.await, Err(Error::NotReady)));
}

#[test]
fn reply_state_reserve_rollback_commit() {
    let state = ReplyState::default();
    state.reserve().unwrap();
    assert!(state.reserve().is_err(), "a second reply while the first is in flight must lose");
    state.rollback();
    state.reserve().unwrap();
    state.commit();
    assert!(state.is_committed());
    assert!(state.reserve().is_err());
}

#[test]
fn command_bodies_of_different_shapes_serialize_together() {
    let list: CommandList = commands![
        SlashCommandBuilder::new("echo", "says").string_option("text", "what", |o| o.required(true)),
        SlashCommandBuilder::new("admin", "does").subcommand(Subcommand::new("ban", "bans")),
        UserContextCommand::new("Inspect"),
    ]
    .into_iter()
    .collect();
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json[0]["options"][0]["required"], true);
    assert_eq!(json[1]["options"][0]["name"], "ban");
    assert_eq!(json[2]["type"], 2);
    assert!(json[2].get("description").is_none());
}

#[tokio::test]
async fn deliver_runs_handlers_with_state_and_routes_failures() {
    struct Seen(AtomicU32);
    let (tx, mut rx) = mpsc::channel(8);
    let runner = Client::builder("t")
        .state(Seen(AtomicU32::new(0)))
        .intents(Intents::GUILD_MESSAGES)
        .on(MessageCreate, |ctx, msg| async move {
            ctx.state.0.fetch_add(1, Ordering::SeqCst);
            match msg.content.as_str() {
                "hi" => Err("failed on purpose".into()),
                _ => panic!("panicked on purpose"),
            }
        })
        .on_handler_error(move |failure| tx.try_send(failure).unwrap())
        .build()
        .unwrap();
    let client = runner.client();
    let kind = discord::model::DispatchKind::MessageCreate;

    discord::events::deliver(&client, 0, kind, "MESSAGE_CREATE", message_json()).await;
    let failure = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    assert_eq!(failure.event, "MESSAGE_CREATE");
    assert!(matches!(failure.kind, FailureKind::Error(_)));

    let other = message_json().replace(r#""content":"hi""#, r#""content":"boom""#);
    discord::events::deliver(&client, 0, kind, "MESSAGE_CREATE", &other).await;
    let failure = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(&failure.kind, FailureKind::Panic(p) if p == "panicked on purpose"), "{:?}", failure.kind);
    assert_eq!(client.stats().handler_panics(), 1);
    assert_eq!(client.stats().handler_errors(), 1);
}

#[tokio::test]
async fn a_body_that_does_not_decode_is_counted_and_reaches_on_raw() {
    let (tx, mut rx) = mpsc::channel(8);
    let runner = Client::builder("t")
        .on_raw(move |_ctx, raw| {
            let tx = tx.clone();
            async move {
                tx.try_send(raw).unwrap();
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let kind = discord::model::DispatchKind::MessageCreate;
    discord::events::deliver(&client, 0, kind, "MESSAGE_CREATE", r#"{"id":"not a snowflake"}"#).await;
    let raw = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
    assert_eq!(raw.t, "MESSAGE_CREATE");
    assert!(raw.decode_error.is_some());
    assert_eq!(client.stats().decode_failures(), 1);
}
