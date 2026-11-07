//! File uploads: messages, edits and interaction replies, as `multipart/form-data`, on the mock.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{callback_path, slash};
use discord::interactions::CommandInteraction;
use discord::model::DispatchKind as K;
use discord::prelude::*;
use dr_mock::{MockServer, RecordedRequest};
use serde_json::json;
use tokio::sync::mpsc;

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
    (server, runner.client(), rx)
}

fn last(server: &MockServer) -> RecordedRequest {
    server.requests().pop().unwrap()
}

#[tokio::test]
async fn a_message_with_files_goes_as_multipart() {
    let (server, client, _rx) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");

    let body = CreateMessage::new()
        .content("the report")
        .embed(EmbedBuilder::new().image("attachment://chart.png"))
        .attach(CreateAttachment::bytes(b"\x89PNG".to_vec(), "chart.png").content_type("image/png"))
        .attach(CreateAttachment::bytes("line one\r\n--not a boundary", "notes.txt").description("Notes").spoiler());
    let message = client.channel(channel).send(body).await.unwrap();

    let request = last(&server);
    let content_type = request.header("content-type").unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="), "{content_type}");
    let parts = request.parts().unwrap();
    let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["payload_json", "files[0]", "files[1]"]);
    assert_eq!(parts[0].content_type.as_deref(), Some("application/json"));
    assert_eq!(parts[1].data, b"\x89PNG");
    assert_eq!(parts[1].content_type.as_deref(), Some("image/png"));
    assert_eq!(parts[1].filename.as_deref(), Some("chart.png"));
    assert_eq!(parts[2].data, b"line one\r\n--not a boundary", "content that looks like a boundary survives");
    assert_eq!(parts[2].filename.as_deref(), Some("SPOILER_notes.txt"));
    assert_eq!(parts[2].content_type.as_deref(), Some("application/octet-stream"));

    let payload = request.json();
    assert_eq!(payload["content"], "the report");
    assert_eq!(payload["attachments"][0]["id"], 0);
    assert_eq!(payload["attachments"][0]["filename"], "chart.png");
    assert_eq!(payload["attachments"][1]["id"], 1);
    assert_eq!(payload["attachments"][1]["description"], "Notes");

    let names: Vec<&str> = message.attachments.iter().map(|a| a.filename.as_str()).collect();
    assert_eq!(names, ["chart.png", "SPOILER_notes.txt"]);
    assert_eq!(message.attachments[0].size, 4);
}

#[tokio::test]
async fn a_message_without_files_stays_json() {
    let (server, client, _rx) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    client.channel(channel).send("hi").await.unwrap();
    let request = last(&server);
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert!(request.json().get("attachments").is_none());
}

#[tokio::test]
async fn a_file_alone_is_a_valid_message() {
    let (server, client, _rx) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let message = client.channel(channel).send(CreateAttachment::bytes("x", "x.txt")).await.unwrap();
    assert_eq!(message.attachments.len(), 1);
}

#[tokio::test]
async fn too_many_files_or_a_bad_name_send_nothing() {
    let (server, client, _rx) = fixture().await;
    let channel = client.channel(1.into());
    let mut body = CreateMessage::new().content("x");
    for i in 0..11 {
        body = body.attach(CreateAttachment::bytes("x", format!("{i}.txt")));
    }
    assert!(channel.send(body).await.is_err());
    assert!(channel.send(CreateAttachment::bytes("x", "")).await.is_err(), "an empty name");
    assert!(server.requests().is_empty(), "nothing was sent");
}

#[tokio::test]
async fn a_quote_in_a_file_name_cannot_break_the_header() {
    let (server, client, _rx) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let name = "a\"; name=\"payload_json\r\nX: y.txt";
    client.channel(channel).send(CreateAttachment::bytes("x", name)).await.unwrap();
    let parts = last(&server).parts().unwrap();
    assert_eq!(parts.len(), 2, "no part was injected");
    assert_eq!(parts[1].name, "files[0]");
    assert_eq!(parts[1].filename.as_deref(), Some("a%22; name=%22payload_json%0D%0AX: y.txt"));
}

#[tokio::test]
async fn an_edit_keeps_only_the_attachments_it_lists() {
    let (server, client, _rx) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let body = CreateMessage::new()
        .attach(CreateAttachment::bytes("a", "a.txt"))
        .attach(CreateAttachment::bytes("b", "b.txt"));
    let message = client.channel(channel).send(body).await.unwrap();
    let keep = message.attachments[0].id;

    let edit = EditMessage::new().keep_attachment(keep).attach(CreateAttachment::bytes("c", "c.txt"));
    let edited = message.edit(edit).await.unwrap();
    let payload = last(&server).json();
    assert_eq!(payload["attachments"][0], json!({"id": keep.to_string()}));
    assert_eq!(payload["attachments"][1]["id"], 0);
    let names: Vec<&str> = edited.attachments.iter().map(|a| a.filename.as_str()).collect();
    assert_eq!(names, ["a.txt", "c.txt"]);

    let cleared = edited.edit(EditMessage::new().clear_attachments()).await.unwrap();
    assert_eq!(last(&server).json(), json!({"attachments": []}));
    assert!(cleared.attachments.is_empty());

    let untouched = cleared.edit(EditMessage::new().content("x")).await.unwrap();
    assert!(last(&server).json().get("attachments").is_none(), "an edit without files leaves them alone");
    assert_eq!(untouched.content, "x");
}

#[tokio::test]
async fn interaction_replies_carry_their_files() {
    let (server, client, mut rx) = fixture().await;
    let body = slash(server.world(), 1, 2, 3, "test", json!([]));
    discord::events::deliver(&client, 0, K::InteractionCreate, "INTERACTION_CREATE", &body.to_string()).await;
    let cmd = rx.recv().await.unwrap();

    cmd.reply(CreateMessage::new().content("here").attach(CreateAttachment::bytes("x", "x.txt")))
        .ephemeral()
        .await
        .unwrap();
    let callback = server.requests().into_iter().find(|r| r.path == callback_path(&body)).unwrap();
    assert!(callback.header("content-type").unwrap().starts_with("multipart/form-data"));
    let payload = callback.json();
    assert_eq!(payload["type"], 4);
    assert_eq!(payload["data"]["flags"], 64);
    assert_eq!(payload["data"]["attachments"][0]["filename"], "x.txt");
    assert_eq!(callback.files()[0].data, b"x");

    cmd.follow_up(CreateAttachment::bytes("y", "y.txt")).await.unwrap();
    assert_eq!(last(&server).files()[0].data, b"y");
    cmd.edit_reply(EditMessage::new().attach(CreateAttachment::bytes("z", "z.txt"))).await.unwrap();
    assert_eq!(last(&server).files()[0].data, b"z");
}
