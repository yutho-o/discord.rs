//! What the builders refuse before anything is sent, and the wire form of what they accept.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_builders::{
    ActionRowBuilder, ButtonBuilder, CommandList, CreateMessage, EditMessage, EmbedBuilder, ModalBuilder,
    SlashCommandBuilder, Subcommand, TextInputBuilder, UserContextCommand, commands,
};
use dr_core::IntoBody;
use dr_model::{ButtonStyle, TextInputStyle};
use serde_json::{Value, json};

fn wire(body: impl serde::Serialize) -> Value {
    serde_json::to_value(body).unwrap()
}

#[test]
fn a_plain_message_is_only_what_was_set() {
    let body = wire(CreateMessage::from("hi"));
    assert_eq!(body, json!({"content": "hi"}));
}

#[test]
fn message_limits() {
    assert!(CreateMessage::from("x".repeat(2000)).validate().is_ok());
    assert!(CreateMessage::from("x".repeat(2001)).validate().is_err());
    assert!(CreateMessage::new().validate().is_err(), "an empty message");
    assert!(CreateMessage::from("x".repeat(2001)).skip_validation().validate().is_ok());
    assert!(CreateMessage::from("x".repeat(2001)).into_body().is_err(), "encoding checks too");
}

#[test]
fn an_edit_writes_only_what_was_asked() {
    assert_eq!(wire(EditMessage::new().content("x")), json!({"content": "x"}));
    assert_eq!(wire(EditMessage::new().clear_content())["content"], Value::Null);
    let cleared = wire(EditMessage::new().clear_components());
    assert_eq!(cleared["components"], json!([]));
}

#[test]
fn a_reply_has_its_reference_and_no_mentions() {
    let body = wire(CreateMessage::from("x").reply_to(5.into()).no_mentions());
    assert_eq!(body["message_reference"]["message_id"], "5");
    assert_eq!(body["allowed_mentions"]["parse"], json!([]));
}

#[test]
fn embed_limits() {
    let long = EmbedBuilder::new().title("x".repeat(257));
    assert!(CreateMessage::new().embed(long).validate().is_err());
    assert!(CreateMessage::new().embed(EmbedBuilder::new().title("x")).validate().is_ok());
    let mut too_many = CreateMessage::new();
    for _ in 0..11 {
        too_many = too_many.embed(EmbedBuilder::new().title("x"));
    }
    assert!(too_many.validate().is_err());
}

#[test]
fn buttons_and_rows() {
    let row = ActionRowBuilder::new().success("a", "A").danger("b", "B");
    let body = wire(CreateMessage::new().row(row));
    assert_eq!(body["components"][0]["components"][0]["style"], 3);
    assert_eq!(body["components"][0]["components"][1]["style"], 4);

    let mut full = ActionRowBuilder::new();
    for i in 0..6 {
        full = full.success(format!("id{i}"), "x");
    }
    assert!(CreateMessage::new().row(full).validate().is_err(), "six in a row");

    let long_id = ActionRowBuilder::new().success("x".repeat(101), "x");
    assert!(CreateMessage::new().row(long_id).validate().is_err());
    let long_label = ActionRowBuilder::new().success("a", "x".repeat(81));
    assert!(CreateMessage::new().row(long_label).validate().is_err());
    let link = ButtonBuilder::new(ButtonStyle::LINK).label("x").url("https://example.com");
    assert!(link.validate().is_ok());
    let link_with_id = ButtonBuilder::new(ButtonStyle::LINK).label("x").url("https://example.com").custom_id("a");
    assert!(link_with_id.validate().is_err());
}

#[test]
fn commands_of_mixed_shapes() {
    let list: CommandList = commands![
        SlashCommandBuilder::new("echo", "says").string_option("text", "what", |o| o.required(true)),
        SlashCommandBuilder::new("admin", "does").subcommand(Subcommand::new("ban", "bans")),
        UserContextCommand::new("Inspect"),
    ]
    .into_iter()
    .collect();
    assert!(list.into_body().is_ok());
}

#[test]
fn a_bad_command_name_is_refused() {
    let list: CommandList = commands![SlashCommandBuilder::new("Has Spaces", "x")].into_iter().collect();
    assert!(list.into_body().is_err());
    let long: CommandList = commands![SlashCommandBuilder::new("x", "y".repeat(101))].into_iter().collect();
    assert!(long.into_body().is_err());
}

#[test]
fn modals() {
    let input = TextInputBuilder::new("name", "Your name", TextInputStyle::SHORT);
    let modal = ModalBuilder::new("form:1", "Title").text_input(input);
    assert!(modal.validate().is_ok());
    let body = wire(&modal);
    assert_eq!(body["custom_id"], "form:1");
    assert!(ModalBuilder::new("form:1", "Title").validate().is_err(), "a modal needs an input");
}
