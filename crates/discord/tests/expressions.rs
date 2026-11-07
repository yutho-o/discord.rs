//! Guild emojis and stickers: creation, edits, deletion, the update events and the cache, stickers
//! on messages.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::builders::{CreateEmoji, CreateSticker, EditEmoji, EditSticker, MAX_STICKER_BYTES};
use discord::model::{EmojiId, JsonErrorCode, StickerFormatType, StickerId, StickerType};
use discord::prelude::*;
use dr_mock::{Method, MockServer};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really";

#[derive(Default)]
struct Seen {
    emojis: Mutex<Vec<(usize, Option<usize>)>>,
    stickers: Mutex<Vec<(usize, Option<usize>)>>,
}

#[tokio::test]
async fn emojis_are_created_edited_and_deleted_and_the_cache_follows() {
    let server = MockServer::start().await.unwrap();
    let guild = server.world().add_guild("g");
    let seen = Arc::new(Seen::default());
    let runner = Client::builder("t")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_EXPRESSIONS)
        .on(GuildEmojisUpdate, |ctx, update| async move {
            ctx.state.emojis.lock().push((update.new.emojis.len(), update.old.as_ref().map(|o| o.emojis.len())));
            Ok(())
        })
        .on(GuildStickersUpdate, |ctx, update| async move {
            ctx.state.stickers.lock().push((update.new.stickers.len(), update.old.as_ref().map(|o| o.stickers.len())));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || client.cache().guild(guild).is_some()).await;
    let emojis = client.guild(guild).emojis();

    let role = RoleId::new(77);
    let emoji =
        emojis.create(CreateEmoji::new("party_blob", PNG, "image/png").roles([role])).reason("fun").await.unwrap();
    let request = server.requests().pop().unwrap();
    assert_eq!((request.method.clone(), request.path.as_str()), (Method::POST, &*format!("/guilds/{guild}/emojis")));
    assert_eq!(request.header("x-audit-log-reason"), Some("fun"));
    let body = request.json();
    assert_eq!(body["name"], "party_blob");
    assert_eq!(body["roles"], json!(["77"]));
    assert!(body["image"].as_str().unwrap().starts_with("data:image/png;base64,"));
    let id = emoji.id.unwrap();
    wait_until("the emoji in the cache", || emojis.get(id).is_some()).await;
    assert_eq!(emojis.cached().len(), 1);

    let renamed = emojis.edit(id, EditEmoji::new().name("blob").roles([])).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json(), json!({"name": "blob", "roles": []}));
    assert_eq!(renamed.name.as_deref(), Some("blob"));
    wait_until("the rename in the cache", || emojis.get(id).and_then(|e| e.name) == Some("blob".into())).await;
    assert_eq!(emojis.fetch_all().await.unwrap().len(), 1);
    assert_eq!(emojis.fetch(id).await.unwrap().user.as_ref().map(|u| u.id), Some(server.world().bot_id()));

    emojis.delete(id).await.unwrap();
    wait_until("gone from the cache", || emojis.cached().is_empty()).await;
    assert_eq!(*seen.emojis.lock(), [(1, Some(0)), (1, Some(1)), (0, Some(1))]);
    let error = emojis.delete(id).await.unwrap_err();
    assert_eq!(error.code(), Some(JsonErrorCode::UNKNOWN_EMOJI));

    // Stickers go as a form: text fields, then the file in the field `file`.
    let stickers = client.guild(guild).stickers();
    let file = CreateAttachment::bytes(PNG, "pizza.png").content_type("image/png");
    let sticker = stickers.create(CreateSticker::new("Pizza", "pizza", file).description("A slice")).await.unwrap();
    let parts = server.requests().pop().unwrap().parts().unwrap();
    let names: Vec<_> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["name", "description", "tags", "file"]);
    assert_eq!(parts[0].data, b"Pizza");
    assert_eq!(parts[3].filename.as_deref(), Some("pizza.png"));
    assert_eq!(parts[3].data, PNG);
    assert_eq!((sticker.kind, sticker.format_type), (StickerType::GUILD, StickerFormatType::PNG));
    assert_eq!(sticker.description.as_deref(), Some("A slice"));
    wait_until("the sticker in the cache", || stickers.get(sticker.id).is_some()).await;

    stickers.edit(sticker.id, EditSticker::new().clear_description().tags("slice")).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json(), json!({"description": null, "tags": "slice"}));
    assert_eq!(client.fetch_sticker(sticker.id).await.unwrap().tags, "slice");
    stickers.delete(sticker.id).await.unwrap();
    wait_until("the sticker is gone", || stickers.cached().is_empty()).await;
    assert_eq!(*seen.stickers.lock(), [(1, Some(0)), (1, Some(1)), (0, Some(1))]);
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn bad_emoji_and_sticker_bodies_send_nothing() {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    let guild = client.guild(GuildId::new(1));
    let path = |e: Error| match e {
        Error::Validation(v) => v.path,
        other => panic!("{other:?}"),
    };
    let e = guild.emojis().create(CreateEmoji::new("no spaces", PNG, "image/png")).await.unwrap_err();
    assert_eq!(path(e), "name");
    let e = guild.emojis().create(CreateEmoji::new("x", PNG, "image/png")).await.unwrap_err();
    assert_eq!(path(e), "name");
    let big = vec![0u8; 256 * 1024 + 1];
    let e = guild.emojis().create(CreateEmoji::new("big", &big, "image/png")).await.unwrap_err();
    assert_eq!(path(e), "image");
    let e = guild.emojis().edit(EmojiId::new(1), EditEmoji::new().name("a-b")).await.unwrap_err();
    assert_eq!(path(e), "name");

    let file = || CreateAttachment::bytes(PNG, "s.png");
    let e = guild.stickers().create(CreateSticker::new("P", "pizza", file())).await.unwrap_err();
    assert_eq!(path(e), "name");
    let e = guild.stickers().create(CreateSticker::new("Pizza", "", file())).await.unwrap_err();
    assert_eq!(path(e), "tags");
    let e = guild.stickers().create(CreateSticker::new("Pizza", "p", file()).description("x")).await.unwrap_err();
    assert_eq!(path(e), "description");
    let huge = CreateAttachment::bytes(vec![0u8; MAX_STICKER_BYTES + 1], "s.png");
    let e = guild.stickers().create(CreateSticker::new("Pizza", "p", huge)).await.unwrap_err();
    assert_eq!(path(e), "file");

    let four = (1..=4).fold(CreateMessage::new(), |m, i| m.sticker(StickerId::new(i)));
    let e = client.channel(ChannelId::new(5)).send(four).await.unwrap_err();
    assert_eq!(path(e), "sticker_ids");
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn a_message_carries_stickers() {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    let channel = server.world().add_channel(server.world().add_guild("g"), "general");
    client.channel(channel).send(CreateMessage::new().sticker(StickerId::new(749_054_660_769_218_631))).await.unwrap();
    assert_eq!(server.requests().pop().unwrap().json()["sticker_ids"], json!(["749054660769218631"]));

    let message = json!({
        "id": "9", "channel_id": channel.to_string(), "author": {"id": "1", "username": "a"},
        "sticker_items": [{"id": "749054660769218631", "name": "Wave", "format_type": 3}],
    });
    let message: Message = serde_json::from_value(message).unwrap();
    assert_eq!(message.sticker_items[0].name, "Wave");
    assert_eq!(message.sticker_items[0].format_type.extension(), Some("json"));
}
