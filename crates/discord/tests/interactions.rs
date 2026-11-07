//! Routing and the reply state machine, with real HTTP to the mock and no gateway.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{callback_path, slash};
use discord::interactions::CommandInteraction;
use discord::model::DispatchKind as K;
use discord::prelude::*;
use discord::{Error, InteractionError};
use dr_mock::{Fault, Method, MockServer};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::mpsc;

/// A client on the mock whose `/test` command hands its interaction to the test.
async fn fixture() -> (MockServer, Client, mpsc::Receiver<Bound<CommandInteraction>>) {
    let server = MockServer::start().await.unwrap();
    let (tx, rx) = mpsc::channel(4);
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .on_command("test", move |_ctx, cmd| {
            let tx = tx.clone();
            async move {
                tx.send(cmd).await?;
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    (server, client, rx)
}

async fn command(
    server: &MockServer,
    client: &Client,
    rx: &mut mpsc::Receiver<Bound<CommandInteraction>>,
) -> (Bound<CommandInteraction>, Value) {
    let body = slash(server.world(), 1, 2, 3, "test", json!([]));
    discord::events::deliver(client, 0, K::InteractionCreate, "INTERACTION_CREATE", &body.to_string()).await;
    (rx.recv().await.unwrap(), body)
}

#[tokio::test]
async fn reply_state_unchanged_on_http_failure() {
    let (server, client, mut rx) = fixture().await;
    let (cmd, body) = command(&server, &client, &mut rx).await;
    let path = callback_path(&body);
    server.rest().on(Method::POST, path.clone()).times(1).respond(
        400,
        &[("content-type", "application/json")],
        r#"{"code":50035,"message":"Invalid Form Body"}"#,
    );
    let failed = cmd.reply("first").await.unwrap_err();
    assert!(matches!(failed, Error::Rest(_)), "{failed:?}");
    assert!(!cmd.reply_state.is_committed());
    assert!(matches!(cmd.edit_reply(EditMessage::new()).await, Err(Error::Interaction(InteractionError::NotReplied))));

    cmd.reply("second").await.unwrap();
    assert!(cmd.reply_state.is_committed());
    let again = cmd.reply("third").await.unwrap_err();
    assert!(matches!(again, Error::Interaction(InteractionError::AlreadyReplied)), "{again:?}");
    let sent = server.requests().into_iter().filter(|r| r.path == path).count();
    assert_eq!(sent, 2, "the third never reached the network");
}

#[tokio::test]
async fn an_invalid_reply_sends_nothing_and_can_be_retried() {
    let (server, client, mut rx) = fixture().await;
    let (cmd, body) = command(&server, &client, &mut rx).await;
    let failed = cmd.reply(CreateMessage::new().content("x".repeat(2001))).await.unwrap_err();
    assert!(matches!(failed, Error::Validation(_)), "{failed:?}");
    assert!(server.requests().iter().all(|r| r.path != callback_path(&body)));
    cmd.reply("fine").await.unwrap();
}

#[tokio::test]
async fn two_replies_at_once_have_one_winner() {
    let (server, client, mut rx) = fixture().await;
    let (cmd, body) = command(&server, &client, &mut rx).await;
    let (a, b) = tokio::join!(cmd.reply("a").into_future(), cmd.reply("b").into_future());
    assert!(a.is_ok() != b.is_ok(), "{a:?} {b:?}");
    let sent = server.requests().into_iter().filter(|r| r.path == callback_path(&body)).count();
    assert_eq!(sent, 1);
}

#[tokio::test]
async fn a_dropped_reply_gives_the_right_to_answer_back() {
    let (server, client, mut rx) = fixture().await;
    let (cmd, body) = command(&server, &client, &mut rx).await;
    let path = callback_path(&body);
    server.rest().on(Method::POST, path).times(1).fault(Fault::Latency(Duration::from_secs(30)));
    let dropped = tokio::time::timeout(Duration::from_millis(200), cmd.reply("slow").into_future()).await;
    assert!(dropped.is_err(), "the timeout dropped it");
    assert!(!cmd.reply_state.is_committed());
    cmd.reply("again").await.unwrap();
    assert!(cmd.reply_state.is_committed());
}

#[tokio::test]
async fn the_whole_reply_flow() {
    let (server, client, mut rx) = fixture().await;
    let (cmd, body) = command(&server, &client, &mut rx).await;
    assert!(matches!(cmd.follow_up("early").await, Err(Error::Interaction(InteractionError::NotReplied))));
    cmd.defer().ephemeral().await.unwrap();
    let callback = server.requests().into_iter().find(|r| r.path == callback_path(&body)).unwrap();
    assert_eq!(callback.json()["type"], 5);
    assert_eq!(callback.json()["data"]["flags"], 64);
    assert!(callback.header("authorization").is_none(), "a callback carries no bot token");

    let edited = cmd.edit_reply(EditMessage::new().content("done")).await.unwrap();
    assert_eq!(edited.content, "done");
    assert_eq!(cmd.fetch_reply().await.unwrap().content, "done");
    let further = cmd.follow_up("more").ephemeral().await.unwrap();
    assert_eq!(further.content, "more");
    cmd.delete_reply().await.unwrap();
    assert!(cmd.fetch_reply().await.is_err());
}

#[tokio::test]
async fn options_read_through_subcommands() {
    let (server, client, mut rx) = fixture().await;
    let options = json!([{
        "name": "ban", "type": 1,
        "options": [
            {"name": "who", "type": 6, "value": "77"},
            {"name": "days", "type": 4, "value": 3},
            {"name": "silent", "type": 5, "value": true},
            {"name": "why", "type": 3, "value": "spam"},
        ],
    }]);
    let body = slash(server.world(), 1, 2, 3, "test", options);
    discord::events::deliver(&client, 0, K::InteractionCreate, "INTERACTION_CREATE", &body.to_string()).await;
    let cmd = rx.recv().await.unwrap();
    let options = cmd.options();
    assert_eq!(options.subcommand(), Some("ban"));
    assert_eq!(options.user("who"), Some(77.into()));
    assert_eq!(options.integer("days"), Some(3));
    assert_eq!(options.boolean("silent"), Some(true));
    assert_eq!(options.require_string("why").unwrap(), "spam");
    let missing = options.require_string("nope").unwrap_err();
    assert_eq!(missing, InteractionError::Option("nope".to_owned()));
}

#[tokio::test]
async fn modals_and_autocomplete_route_by_prefix_and_name() {
    let server = MockServer::start().await.unwrap();
    let (modal_tx, mut modals) = mpsc::channel(1);
    let (auto_tx, mut autos) = mpsc::channel(1);
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .on_modal("form:", move |_ctx, modal| {
            let tx = modal_tx.clone();
            async move {
                modal.reply(format!("got {}", modal.value("name").unwrap_or("nothing"))).await?;
                tx.send(modal.custom_id.clone()).await?;
                Ok(())
            }
        })
        .on_autocomplete("pick", move |_ctx, auto| {
            let tx = auto_tx.clone();
            async move {
                let focused = auto.focused().unwrap_or_default().to_owned();
                auto.respond([discord::interactions::AutocompleteChoice::string("One", "1")]).await?;
                tx.send(focused).await?;
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let member = json!({"user": {"id": "3", "username": "u"}, "roles": [], "permissions": "0"});
    let modal = json!({
        "id": "101", "application_id": "9", "type": 5, "token": "tm", "version": 1, "member": member,
        "data": {"custom_id": "form:a", "components": [{"type": 1, "components": [
            {"type": 4, "custom_id": "name", "value": "Ada"},
        ]}]},
    });
    discord::events::deliver(&client, 0, K::InteractionCreate, "INTERACTION_CREATE", &modal.to_string()).await;
    assert_eq!(modals.recv().await.unwrap(), "form:a");
    let sent = server.requests().into_iter().find(|r| r.path == "/interactions/101/tm/callback").unwrap();
    assert_eq!(sent.json()["data"]["content"], "got Ada");

    let auto = json!({
        "id": "102", "application_id": "9", "type": 4, "token": "ta", "version": 1, "member": member,
        "data": {"id": "1", "name": "pick", "type": 1, "options": [
            {"name": "what", "type": 3, "value": "o", "focused": true},
        ]},
    });
    discord::events::deliver(&client, 0, K::InteractionCreate, "INTERACTION_CREATE", &auto.to_string()).await;
    assert_eq!(autos.recv().await.unwrap(), "what");
    let sent = server.requests().into_iter().find(|r| r.path == "/interactions/102/ta/callback").unwrap();
    assert_eq!(sent.json()["type"], 8);
    assert_eq!(sent.json()["data"]["choices"][0]["value"], "1");
}
