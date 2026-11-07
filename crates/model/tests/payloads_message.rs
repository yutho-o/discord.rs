//! Messages, embeds, attachments, reactions and emoji, against the shapes of
//! `payloads/v10/{message,emoji}.ts` of discord-api-types.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::{Id, Patch, Timestamp, decode};
use dr_model::{
    ApplicationIntegrationType, Attachment, AttachmentFlags, BaseThemeType, ChannelType, Embed, EmbedFlags,
    EmbedMediaFlags, EmbedType, Emoji, InteractionType, Message, MessageActivityType, MessageFlags, MessageReference,
    MessageReferenceType, MessageType, Nonce, Reaction,
};
use serde_json::{Value, json};

const USER: u64 = 80_351_110_224_678_912;
const MESSAGE: u64 = 334_385_199_974_967_042;

fn author() -> Value {
    json!({ "id": USER.to_string(), "username": "Nelly", "discriminator": "0", "global_name": null, "avatar": null })
}

/// The fields every `APIMessage` has.
fn base() -> Value {
    json!({
        "id": MESSAGE.to_string(),
        "channel_id": "290926798999357250",
        "author": author(),
        "content": "hello",
        "timestamp": "2017-07-11T17:27:07.299000+00:00",
        "edited_timestamp": null,
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0,
    })
}

fn with(mut message: Value, key: &str, value: Value) -> Value {
    message[key] = value;
    message
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> T {
    decode(&value.to_string()).unwrap()
}

/// Serializes and reads back, which must give the same JSON again.
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> Value {
    let first = serde_json::to_value(value).unwrap();
    let again: T = parse(&first);
    assert_eq!(first, serde_json::to_value(&again).unwrap());
    first
}

#[test]
fn base_message_decodes_every_field() {
    let m: Message = parse(&base());
    assert_eq!(m.id, Id::new(MESSAGE));
    assert_eq!(m.channel_id, Id::new(290_926_798_999_357_250));
    assert_eq!(m.author.id, Id::new(USER));
    assert_eq!(m.author.username, "Nelly");
    assert_eq!(m.content, "hello");
    assert_eq!(m.timestamp, Timestamp::parse("2017-07-11T17:27:07.299+00:00"));
    assert_eq!(m.edited_timestamp, None);
    assert_eq!(m.kind, MessageType::DEFAULT);
    assert!(!m.tts && !m.mention_everyone && !m.pinned);
    assert!(m.mentions.is_empty() && m.mention_roles.is_empty() && m.mention_channels.is_empty());
    assert!(m.attachments.is_empty() && m.embeds.is_empty() && m.reactions.is_empty());
    assert!(m.components.is_empty() && m.message_snapshots.is_empty());
    assert!(m.guild_id.is_none() && m.member.is_none() && m.channel_type.is_none());
    assert!(m.nonce.is_none() && m.webhook_id.is_none() && m.activity.is_none() && m.application_id.is_none());
    assert!(m.message_reference.is_none() && m.flags.is_none() && m.referenced_message.is_absent());
    assert!(m.interaction_metadata.is_none() && m.interaction.is_none());
    assert!(m.position.is_none() && m.call.is_none() && m.shared_client_theme.is_none());
}

#[test]
fn optional_message_fields_decode() {
    let m: Message = parse(&json!({
        "id": MESSAGE.to_string(),
        "channel_id": "2",
        "guild_id": "3",
        "channel_type": 5,
        "author": author(),
        "member": { "roles": ["41771983423143936"], "joined_at": "2015-04-26T06:26:56.936000+00:00", "nick": "n" },
        "content": "",
        "timestamp": "2021-01-01T00:00:00+00:00",
        "edited_timestamp": "2021-01-02T00:00:00.5Z",
        "tts": true,
        "mention_everyone": true,
        "mentions": [author()],
        "mention_roles": ["41771983423143936"],
        "mention_channels": [{ "id": "7", "guild_id": "8", "type": 0, "name": "general" }],
        "attachments": [{ "id": "9", "filename": "a.png", "size": 1, "url": "u", "proxy_url": "p" }],
        "embeds": [{ "title": "t" }],
        "reactions": [{
            "count": 1, "count_details": { "burst": 0, "normal": 1 }, "me": true, "me_burst": false,
            "emoji": { "id": null, "name": "x" }, "burst_colors": [],
        }],
        "nonce": "abc",
        "pinned": true,
        "webhook_id": "10",
        "type": 19,
        "activity": { "type": 3, "party_id": "spotify:1" },
        "application_id": "11",
        "message_reference": { "type": 0, "message_id": "12", "channel_id": "13", "guild_id": "14" },
        "flags": 64,
        "position": 4,
        "call": { "participants": ["15"], "ended_timestamp": null },
        "shared_client_theme": { "colors": ["5865f2"], "gradient_angle": 90, "base_mix": 58, "base_theme": 2 },
    }));
    assert_eq!(m.guild_id, Some(Id::new(3)));
    assert_eq!(m.channel_type, Some(ChannelType::GUILD_ANNOUNCEMENT));
    let member = m.member.as_ref().unwrap();
    assert!(member.user.is_none(), "a member nested in a message has no user");
    assert_eq!(member.nick.as_deref(), Some("n"));
    assert_eq!(m.edited_timestamp, Timestamp::parse("2021-01-02T00:00:00.500+00:00"));
    assert!(m.tts && m.mention_everyone && m.pinned);
    assert_eq!(m.mentions.len(), 1);
    assert_eq!(m.mention_roles, [Id::new(41_771_983_423_143_936)]);
    let mention = &m.mention_channels[0];
    assert_eq!((mention.id, mention.guild_id), (Id::new(7), Id::new(8)));
    assert_eq!((mention.kind, mention.name.as_str()), (ChannelType::GUILD_TEXT, "general"));
    assert_eq!(m.attachments.len(), 1);
    assert_eq!(m.embeds.len(), 1);
    assert_eq!(m.reactions.len(), 1);
    assert_eq!(m.nonce, Some(Nonce::Str("abc".into())));
    assert_eq!(m.webhook_id, Some(Id::new(10)));
    assert_eq!(m.kind, MessageType::REPLY);
    let activity = m.activity.as_ref().unwrap();
    assert_eq!((activity.kind, activity.party_id.as_deref()), (MessageActivityType::LISTEN, Some("spotify:1")));
    assert_eq!(m.application_id, Some(Id::new(11)));
    let reference = m.message_reference.as_ref().unwrap();
    assert_eq!(reference.kind, Some(MessageReferenceType::DEFAULT));
    assert_eq!(reference.message_id, Some(Id::new(12)));
    assert_eq!(reference.channel_id, Some(Id::new(13)));
    assert_eq!(reference.guild_id, Some(Id::new(14)));
    assert_eq!(m.flags, Some(MessageFlags::EPHEMERAL));
    assert_eq!(m.position, Some(4));
    let call = m.call.as_ref().unwrap();
    assert_eq!((call.participants.as_slice(), call.ended_timestamp), ([Id::new(15)].as_slice(), None));
    let theme = m.shared_client_theme.as_ref().unwrap();
    assert_eq!(theme.colors, ["5865f2"]);
    assert_eq!((theme.gradient_angle, theme.base_mix, theme.base_theme), (90, 58, Some(BaseThemeType::LIGHT)));
}

#[test]
fn required_fields_are_decode_failures_with_a_path() {
    for key in ["id", "channel_id", "author"] {
        let mut value = base();
        value.as_object_mut().unwrap().remove(key);
        let error = decode::<Message>(&value.to_string()).unwrap_err();
        assert!(error.source.to_string().contains(key), "{key}: {error}");
    }
    let no_user_id = with(base(), "author", json!({ "username": "x" }));
    assert!(decode::<Message>(&no_user_id.to_string()).unwrap_err().path.contains("author"));
}

#[test]
fn missing_primitives_and_lists_cost_only_a_default() {
    let m: Message = parse(&json!({ "id": "1", "channel_id": "2", "author": { "id": "3" } }));
    assert_eq!(m.content, "");
    assert_eq!(m.kind, MessageType::DEFAULT);
    assert!(m.timestamp.is_none() && m.mentions.is_empty() && !m.pinned);
}

#[test]
fn null_lists_read_as_empty() {
    let mut value = base();
    for key in ["mentions", "mention_roles", "mention_channels", "attachments", "embeds", "reactions", "components"] {
        value[key] = Value::Null;
    }
    value["message_snapshots"] = Value::Null;
    let m: Message = parse(&value);
    assert!(m.mentions.is_empty() && m.reactions.is_empty() && m.components.is_empty());
    assert!(m.message_snapshots.is_empty() && m.mention_channels.is_empty());
}

#[test]
fn unknown_keys_and_cut_features_are_ignored() {
    let value = with(base(), "brand_new_field", json!({ "a": 1 }));
    let value = with(value, "poll", json!({ "question": { "text": "?" } }));
    let value = with(value, "sticker_items", json!([{ "id": "1", "name": "s", "format_type": 1 }]));
    let value = with(value, "thread", json!({ "id": "1", "type": 11 }));
    let value = with(value, "application", json!({ "id": "1", "name": "a" }));
    let value = with(value, "role_subscription_data", json!({ "tier_name": "t" }));
    let m: Message = parse(&value);
    assert_eq!(m.content, "hello");
    let written = round_trip(&m);
    for key in ["brand_new_field", "poll", "sticker_items", "thread", "application"] {
        assert!(written.get(key).is_none(), "{key}");
    }
    assert!(written.get("resolved").is_none(), "absent stays absent");
}

/// Left out at first, with the other keys above, until the merge brought `ResolvedData` in reach.
#[test]
fn resolved_data_is_read_and_written_back() {
    let resolved = json!({ "users": { "1": { "id": "1", "username": "u", "discriminator": "0", "global_name": null, "avatar": null } } });
    let m: Message = parse(&with(base(), "resolved", resolved.clone()));
    assert_eq!(m.resolved.as_ref().unwrap().users[&Id::new(1)].username, "u");
    assert_eq!(round_trip(&m)["resolved"], resolved);
}

#[test]
fn unknown_kinds_are_kept() {
    let m: Message = parse(&with(base(), "type", json!(777)));
    assert_eq!(m.kind, MessageType(777));
    assert_eq!(round_trip(&m)["type"], 777);
    assert!(m.is_system());
    let channel_type: Message = parse(&with(base(), "channel_type", json!(99)));
    assert_eq!(channel_type.channel_type, Some(ChannelType(99)));
}

#[test]
fn unknown_flag_bits_are_kept() {
    let bits = (1u64 << 40) | (1 << 6);
    let m: Message = parse(&with(base(), "flags", json!(bits)));
    let flags = m.flags.unwrap();
    assert!(flags.contains(MessageFlags::EPHEMERAL));
    assert_eq!(flags.bits(), bits);
    assert_eq!(round_trip(&m)["flags"], bits);
    let as_string: Message = parse(&with(base(), "flags", json!(bits.to_string())));
    assert_eq!(as_string.flags.unwrap().bits(), bits);
}

#[test]
fn referenced_message_has_three_states() {
    let absent: Message = parse(&base());
    assert!(absent.referenced_message.is_absent());
    assert!(round_trip(&absent).get("referenced_message").is_none());

    let deleted: Message = parse(&with(base(), "referenced_message", Value::Null));
    assert!(deleted.referenced_message.is_null());
    assert_eq!(round_trip(&deleted)["referenced_message"], Value::Null);

    let inner = with(base(), "content", json!("original"));
    let reply: Message = parse(&with(with(base(), "type", json!(19)), "referenced_message", inner));
    let Patch::Value(target) = &reply.referenced_message else { panic!("expected a value") };
    assert_eq!(target.content, "original");
    assert_eq!(round_trip(&reply)["referenced_message"]["content"], "original");
}

/// Wraps `inner` in `depth` levels of `key`, as text: `Value` itself recurses when it is dropped.
fn nest(template: &Value, key: &str, inner: &str, depth: usize) -> String {
    let mut wrapper = template.clone();
    wrapper[key] = Value::Null;
    let (head, tail) =
        wrapper.to_string().split_once(&format!("\"{key}\":null")).map(|(h, t)| (h.to_owned(), t.to_owned())).unwrap();
    let mut text = String::new();
    for _ in 0..depth {
        text.push_str(&head);
        text.push_str(&format!("\"{key}\":"));
    }
    text.push_str(inner);
    for _ in 0..depth {
        text.push_str(&tail);
    }
    text
}

#[test]
fn nested_references_within_the_limit_decode() {
    let text = nest(&base(), "referenced_message", &base().to_string(), 8);
    let mut message: &Message = &decode(&text).unwrap();
    let mut levels = 0;
    while let Patch::Value(next) = &message.referenced_message {
        message = next;
        levels += 1;
    }
    assert_eq!(levels, 8);
}

#[test]
fn deep_referenced_messages_fail_rather_than_overflow_the_stack() {
    for depth in [9, 127, 128, 129, 300, 100_000] {
        let text = nest(&base(), "referenced_message", "null", depth);
        let error = decode::<Message>(&text).unwrap_err();
        assert!(error.path.starts_with("referenced_message"), "{depth}: {}", error.path);
    }
    // The count is back at zero after every failure, so an ordinary message still decodes.
    let one = nest(&base(), "referenced_message", &base().to_string(), 1);
    assert!(decode::<Message>(&one).is_ok());
}

#[test]
fn deep_interaction_metadata_fails_rather_than_overflows_the_stack() {
    let template = json!({ "id": "1", "type": 5, "user": { "id": "2" } });
    let leaf = template.to_string();
    for depth in [9, 128, 100_000] {
        let metadata = nest(&template, "triggering_interaction_metadata", &leaf, depth);
        let text = nest(&base(), "interaction_metadata", &metadata, 1);
        assert!(decode::<Message>(&text).is_err(), "{depth}");
    }
    let metadata = nest(&template, "triggering_interaction_metadata", &leaf, 2);
    assert!(decode::<Message>(&nest(&base(), "interaction_metadata", &metadata, 1)).is_ok());
}

#[test]
fn nonce_reads_both_forms() {
    let int: Message = parse(&with(base(), "nonce", json!(12345)));
    assert_eq!(int.nonce, Some(Nonce::Int(12345)));
    let text: Message = parse(&with(base(), "nonce", json!("12345")));
    assert_eq!(text.nonce, Some(Nonce::Str("12345".into())));
    assert_eq!(round_trip(&int)["nonce"], 12345);
    assert_eq!(round_trip(&text)["nonce"], "12345");
}

#[test]
fn serialized_message_has_discords_shape() {
    let written = round_trip(&parse::<Message>(&base()));
    let object = written.as_object().unwrap();
    // `| null` is written, a required list is written, and an unset `?` field is left out.
    assert_eq!(object["edited_timestamp"], Value::Null);
    assert_eq!(object["mentions"], json!([]));
    assert_eq!(object["embeds"], json!([]));
    assert_eq!(object["id"], MESSAGE.to_string());
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "attachments",
            "author",
            "channel_id",
            "content",
            "edited_timestamp",
            "embeds",
            "id",
            "mention_everyone",
            "mention_roles",
            "mentions",
            "pinned",
            "timestamp",
            "tts",
            "type",
        ]
    );
}

#[test]
fn components_keep_unknown_kinds() {
    let m: Message = parse(&with(
        base(),
        "components",
        json!([
            { "type": 1, "components": [{ "type": 2, "style": 1, "custom_id": "a", "label": "b" }] },
            { "type": 777, "future": { "x": [1, 2, 3] } },
        ]),
    ));
    assert_eq!(m.components.len(), 2);
    let written = round_trip(&m);
    assert_eq!(written["components"][1], json!({ "type": 777, "future": { "x": [1, 2, 3] } }));
    let missing_tag = with(base(), "components", json!([{ "style": 1 }]));
    assert!(decode::<Message>(&missing_tag.to_string()).unwrap_err().path.contains("components"));
}

#[test]
fn message_reference_writes_only_what_is_set() {
    let reply = MessageReference {
        message_id: Some(Id::new(1)),
        fail_if_not_exists: Some(false),
        ..MessageReference::default()
    };
    assert_eq!(serde_json::to_value(&reply).unwrap(), json!({ "message_id": "1", "fail_if_not_exists": false }));

    // Message forwarding, from the upstream `MessageReferenceType` doc comment.
    let forward: MessageReference = parse(&json!({ "type": 1, "message_id": "1", "channel_id": "2" }));
    assert_eq!(forward.kind, Some(MessageReferenceType::FORWARD));
    let older: MessageReference = parse(&json!({ "channel_id": "2" }));
    assert_eq!(older.kind, None);
    assert_eq!(older.message_id, None);
    assert_eq!(serde_json::to_value(MessageReference::default()).unwrap(), json!({}));
}

#[test]
fn forward_snapshot_holds_the_message_subset() {
    let m: Message = parse(&with(
        with(base(), "content", json!("")),
        "message_snapshots",
        json!([{
            "guild_id": "1",
            "message": {
                "content": "forwarded",
                "type": 0,
                "timestamp": "2024-01-01T00:00:00+00:00",
                "edited_timestamp": null,
                "flags": 16384,
                "mentions": [author()],
                "mention_roles": ["5"],
                "attachments": [{ "id": "6", "filename": "f", "size": 0, "url": "u", "proxy_url": "p" }],
                "embeds": [{ "title": "t" }],
                "components": [{ "type": 777 }],
                "stickers": [],
            },
        }]),
    ));
    let snapshot = &m.message_snapshots[0].message;
    assert_eq!(m.content, "");
    assert_eq!(snapshot.content, "forwarded");
    assert_eq!(snapshot.kind, MessageType::DEFAULT);
    assert_eq!(snapshot.timestamp, Timestamp::parse("2024-01-01T00:00:00Z"));
    assert_eq!(snapshot.flags.unwrap().bits(), 1 << 14);
    assert_eq!(snapshot.mentions.len(), 1);
    assert_eq!(snapshot.mention_roles, [Id::new(5)]);
    assert_eq!(snapshot.attachments.len(), 1);
    assert_eq!(snapshot.embeds.len(), 1);
    assert_eq!(snapshot.components.len(), 1);
    let written = round_trip(&m);
    assert!(written["message_snapshots"][0].get("guild_id").is_none(), "upstream says it does not exist");
    assert!(decode::<Message>(&with(base(), "message_snapshots", json!([{}])).to_string()).is_err());
}

#[test]
fn interaction_metadata_decodes_each_kind() {
    let user = author();
    let m: Message = parse(&with(
        base(),
        "interaction_metadata",
        json!({
            "id": "1",
            "type": 5,
            "user": user,
            "authorizing_integration_owners": { "0": "2", "1": "0" },
            "original_response_message_id": "3",
            "triggering_interaction_metadata": {
                "id": "4",
                "type": 3,
                "user": user,
                "authorizing_integration_owners": {},
                "interacted_message_id": "5",
            },
        }),
    ));
    let metadata = m.interaction_metadata.as_ref().unwrap();
    assert_eq!(metadata.id, Id::new(1));
    assert_eq!(metadata.kind, InteractionType::MODAL_SUBMIT);
    assert_eq!(metadata.user.id, Id::new(USER));
    assert_eq!(metadata.authorizing_integration_owners[&ApplicationIntegrationType::GUILD_INSTALL], Id::new(2));
    assert_eq!(metadata.authorizing_integration_owners[&ApplicationIntegrationType::USER_INSTALL], Id::new(0));
    assert_eq!(metadata.original_response_message_id, Some(Id::new(3)));
    let trigger = metadata.triggering_interaction_metadata.as_ref().unwrap();
    assert_eq!(trigger.kind, InteractionType::MESSAGE_COMPONENT);
    assert_eq!(trigger.interacted_message_id, Some(Id::new(5)));
    round_trip(&m);

    let command: Message = parse(&with(
        base(),
        "interaction_metadata",
        json!({
            "id": "1",
            "type": 2,
            "user": user,
            "authorizing_integration_owners": {},
            "target_user": { "id": "6" },
            "target_message_id": "7",
        }),
    ));
    let metadata = command.interaction_metadata.as_ref().unwrap();
    assert_eq!(metadata.kind, InteractionType::APPLICATION_COMMAND);
    assert_eq!(metadata.target_user.as_ref().unwrap().id, Id::new(6));
    assert_eq!(metadata.target_message_id, Some(Id::new(7)));
}

#[test]
fn deprecated_interaction_record_decodes() {
    let m: Message = parse(&with(
        base(),
        "interaction",
        json!({
            "id": "1", "type": 2, "name": "group sub", "user": author(),
            "member": { "roles": [], "joined_at": "2015-04-26T06:26:56.936000+00:00" },
        }),
    ));
    let interaction = m.interaction.as_ref().unwrap();
    assert_eq!((interaction.id, interaction.kind), (Id::new(1), InteractionType::APPLICATION_COMMAND));
    assert_eq!(interaction.name, "group sub");
    assert_eq!(interaction.user.id, Id::new(USER));
    assert!(interaction.member.as_ref().unwrap().user.is_none());
}

#[test]
fn attachment_decodes_every_field() {
    let a: Attachment = parse(&json!({
        "id": "1",
        "filename": "voice-message.ogg",
        "title": "Сообщение",
        "description": "alt",
        "content_type": "audio/ogg",
        "size": 5_000_000_000_u64,
        "url": "https://cdn/a",
        "proxy_url": "https://media/a",
        "height": 16,
        "width": 32,
        "placeholder": "abc",
        "placeholder_version": 1,
        "ephemeral": true,
        "duration_secs": 2.46,
        "waveform": "AAAA",
        "flags": 8,
        "clip_participants": [{ "id": "2" }],
        "clip_created_at": "2024-01-01T00:00:00+00:00",
    }));
    assert_eq!(a.id, Id::new(1));
    assert_eq!(a.filename, "voice-message.ogg");
    assert_eq!(a.title.as_deref(), Some("Сообщение"));
    assert_eq!(a.description.as_deref(), Some("alt"));
    assert_eq!(a.content_type.as_deref(), Some("audio/ogg"));
    assert_eq!(a.size, 5_000_000_000);
    assert_eq!(a.url, "https://cdn/a");
    assert_eq!(a.proxy_url, "https://media/a");
    assert_eq!((a.height, a.width), (Some(16), Some(32)));
    assert_eq!((a.placeholder.as_deref(), a.placeholder_version), (Some("abc"), Some(1)));
    assert!(a.ephemeral);
    assert_eq!(a.duration_secs, Some(2.46));
    assert_eq!(a.waveform.as_deref(), Some("AAAA"));
    assert_eq!(a.flags, Some(AttachmentFlags::IS_SPOILER));
    assert!(a.is_spoiler());
    assert_eq!(a.clip_participants[0].id, Id::new(2));
    assert_eq!(a.clip_created_at, Timestamp::parse("2024-01-01T00:00:00Z"));
    round_trip(&a);
}

#[test]
fn attachment_tolerates_null_dimensions_and_absent_flags() {
    let a: Attachment = parse(
        &json!({ "id": "1", "filename": "f", "size": 0, "url": "u", "proxy_url": "p", "height": null, "width": null }),
    );
    assert_eq!((a.height, a.width, a.flags), (None, None, None));
    assert!(!a.is_spoiler() && !a.ephemeral);
    assert_eq!(
        round_trip(&a),
        json!({ "id": "1", "filename": "f", "size": 0, "url": "u", "proxy_url": "p" }),
        "unset optional fields are not written"
    );
    assert!(decode::<Attachment>(r#"{"filename":"f"}"#).is_err(), "id is required");
}

#[test]
fn attachment_flag_values_match_upstream() {
    let expected = [
        (AttachmentFlags::IS_CLIP, 1 << 0),
        (AttachmentFlags::IS_THUMBNAIL, 1 << 1),
        (AttachmentFlags::IS_REMIX, 1 << 2),
        (AttachmentFlags::IS_SPOILER, 1 << 3),
        (AttachmentFlags::CONTAINS_EXPLICIT_CONTENT, 1 << 4),
        (AttachmentFlags::IS_ANIMATED, 1 << 5),
        (AttachmentFlags::CONTAINS_GORE_CONTENT, 1 << 6),
        (AttachmentFlags::CONTAINS_SELF_HARM_CONTENT, 1 << 7),
    ];
    for (flag, bits) in expected {
        assert_eq!(flag.bits(), bits);
    }
    assert_eq!(EmbedFlags::CONTAINS_EXPLICIT_CONTENT.bits(), 1 << 4);
    assert_eq!(EmbedFlags::IS_CONTENT_INVENTORY_ENTRY.bits(), 1 << 5);
    assert_eq!(EmbedFlags::CONTAINS_GORE_CONTENT.bits(), 1 << 6);
    assert_eq!(EmbedFlags::CONTAINS_SELF_HARM_CONTENT.bits(), 1 << 7);
    assert_eq!(EmbedMediaFlags::IS_ANIMATED.bits(), 1 << 5);
}

#[test]
fn embed_decodes_every_field() {
    let media = json!({
        "url": "https://x/m",
        "proxy_url": "https://y/m",
        "height": 10,
        "width": 20,
        "content_type": "image/png",
        "placeholder": "ph",
        "placeholder_version": 1,
        "description": "alt",
        "flags": 32,
    });
    let e: Embed = parse(&json!({
        "title": "t",
        "type": "rich",
        "description": "d",
        "url": "https://x",
        "timestamp": "2021-01-01T00:00:00.000Z",
        "color": 0x00FF_00FF,
        "footer": { "text": "ft", "icon_url": "fi", "proxy_icon_url": "fp" },
        "image": media,
        "thumbnail": media,
        "video": media,
        "provider": { "name": "pn", "url": "pu" },
        "author": { "name": "an", "url": "au", "icon_url": "ai", "proxy_icon_url": "ap" },
        "fields": [{ "name": "n", "value": "v", "inline": true }, { "name": "n2", "value": "v2" }],
        "components": [{ "type": 17, "components": [] }],
        "flags": 32,
    }));
    assert_eq!(e.title.as_deref(), Some("t"));
    assert_eq!(e.kind, Some(EmbedType::Rich));
    assert_eq!(e.description.as_deref(), Some("d"));
    assert_eq!(e.url.as_deref(), Some("https://x"));
    assert_eq!(e.timestamp, Timestamp::parse("2021-01-01T00:00:00Z"));
    assert_eq!(e.color, Some(0x00FF_00FF));
    let footer = e.footer.as_ref().unwrap();
    assert_eq!(
        (footer.text.as_str(), footer.icon_url.as_deref(), footer.proxy_icon_url.as_deref()),
        ("ft", Some("fi"), Some("fp"))
    );
    for image in [e.image.as_ref().unwrap(), e.thumbnail.as_ref().unwrap()] {
        assert_eq!(image.url, "https://x/m");
        assert_eq!(image.proxy_url.as_deref(), Some("https://y/m"));
        assert_eq!((image.height, image.width), (Some(10), Some(20)));
        assert_eq!(image.content_type.as_deref(), Some("image/png"));
        assert_eq!((image.placeholder.as_deref(), image.placeholder_version), (Some("ph"), Some(1)));
        assert_eq!(image.description.as_deref(), Some("alt"));
        assert_eq!(image.flags, Some(EmbedMediaFlags::IS_ANIMATED));
    }
    let video = e.video.as_ref().unwrap();
    assert_eq!(video.url.as_deref(), Some("https://x/m"));
    assert_eq!(video.proxy_url.as_deref(), Some("https://y/m"));
    assert_eq!((video.height, video.width), (Some(10), Some(20)));
    assert_eq!(video.content_type.as_deref(), Some("image/png"));
    assert_eq!((video.placeholder.as_deref(), video.placeholder_version), (Some("ph"), Some(1)));
    assert_eq!(video.description.as_deref(), Some("alt"));
    assert_eq!(video.flags, Some(EmbedMediaFlags::IS_ANIMATED));
    let provider = e.provider.as_ref().unwrap();
    assert_eq!((provider.name.as_deref(), provider.url.as_deref()), (Some("pn"), Some("pu")));
    let author = e.author.as_ref().unwrap();
    assert_eq!(author.name, "an");
    assert_eq!(
        (author.url.as_deref(), author.icon_url.as_deref(), author.proxy_icon_url.as_deref()),
        (Some("au"), Some("ai"), Some("ap"))
    );
    assert_eq!(e.fields.len(), 2);
    assert_eq!((e.fields[0].name.as_str(), e.fields[0].value.as_str(), e.fields[0].inline), ("n", "v", true));
    assert!(!e.fields[1].inline);
    assert_eq!(e.components.len(), 1);
    assert_eq!(e.flags, Some(EmbedFlags::IS_CONTENT_INVENTORY_ENTRY));
    let written = round_trip(&e);
    assert_eq!(written["type"], "rich");
    assert!(written["fields"][1].get("inline").is_none());
}

#[test]
fn empty_embed_writes_nothing() {
    let e: Embed = parse(&json!({}));
    assert_eq!(serde_json::to_value(&e).unwrap(), json!({}));
    assert_eq!(serde_json::to_value(Embed::default()).unwrap(), json!({}));
    let video_without_url: Embed = parse(&json!({ "video": { "height": 1 } }));
    assert_eq!(video_without_url.video.unwrap().url, None);
}

#[test]
fn embed_types_match_upstream_and_unknown_is_kept() {
    let expected = [
        (EmbedType::Rich, "rich"),
        (EmbedType::Image, "image"),
        (EmbedType::Video, "video"),
        (EmbedType::GIFV, "gifv"),
        (EmbedType::Article, "article"),
        (EmbedType::Link, "link"),
        (EmbedType::AutoModerationMessage, "auto_moderation_message"),
        (EmbedType::PollResult, "poll_result"),
        (EmbedType::Components, "components"),
    ];
    for (kind, text) in expected {
        let embed: Embed = parse(&json!({ "type": text }));
        assert_eq!(embed.kind, Some(kind));
    }
    let unknown: Embed = parse(&json!({ "type": "hologram" }));
    assert!(matches!(&unknown.kind, Some(EmbedType::Unknown(u)) if u.as_str() == "hologram"));
    assert_eq!(round_trip(&unknown)["type"], "hologram");
}

// Vectors from discord.js packages/util/__tests__/embedLength.test.ts (Apache-2.0).
#[test]
fn embed_length_counts_the_limited_text() {
    let e: Embed = parse(&json!({
        "title": "yeet",
        "description": "yeet",
        "fields": [{ "name": "yeet", "value": "yeet" }],
        "author": { "name": "yeet" },
        "footer": { "text": "yeet" },
    }));
    assert_eq!(e.length(), "yeet".len() * 6);
    assert_eq!(Embed::default().length(), 0);
}

#[test]
fn embed_length_counts_utf16_units() {
    let e: Embed = parse(&json!({ "title": "🔥", "description": "é" }));
    assert_eq!(e.length(), 3);
}

#[test]
fn hex_color_pads_to_six_digits() {
    assert_eq!(parse::<Embed>(&json!({ "color": 255 })).hex_color().as_deref(), Some("#0000ff"));
    assert_eq!(parse::<Embed>(&json!({ "color": 0 })).hex_color().as_deref(), Some("#000000"));
    assert_eq!(parse::<Embed>(&json!({ "color": 0x00AB_CDEF })).hex_color().as_deref(), Some("#abcdef"));
    assert_eq!(Embed::default().hex_color(), None);
}

#[test]
fn reaction_decodes_every_field() {
    let r: Reaction = parse(&json!({
        "count": 3,
        "count_details": { "burst": 1, "normal": 2 },
        "me": true,
        "me_burst": true,
        "emoji": { "id": "41771983429993937", "name": "LUL", "animated": true },
        "burst_colors": ["#ffffff"],
    }));
    assert_eq!(r.count, 3);
    assert_eq!((r.count_details.burst, r.count_details.normal), (1, 2));
    assert!(r.me && r.me_burst);
    assert_eq!(r.emoji.id, Some(Id::new(41_771_983_429_993_937)));
    assert_eq!(r.burst_colors, ["#ffffff"]);
    round_trip(&r);
}

#[test]
fn reaction_requires_its_nested_structs() {
    let details = json!({ "burst": 0, "normal": 1 });
    let no_emoji = json!({ "count": 1, "count_details": details });
    assert!(decode::<Reaction>(&no_emoji.to_string()).unwrap_err().source.to_string().contains("emoji"));
    let no_details = json!({ "count": 1, "emoji": { "name": "x" } });
    assert!(decode::<Reaction>(&no_details.to_string()).unwrap_err().source.to_string().contains("count_details"));
}

#[test]
fn full_emoji_decodes_every_field() {
    let e: Emoji = parse(&json!({
        "id": "41771983429993937",
        "name": "LUL",
        "roles": ["41771983429993000"],
        "user": { "id": "96008815106887111", "username": "Luigi" },
        "require_colons": true,
        "managed": true,
        "animated": true,
        "available": false,
    }));
    assert_eq!(e.id, Some(Id::new(41_771_983_429_993_937)));
    assert_eq!(e.name.as_deref(), Some("LUL"));
    assert_eq!(e.roles, [Id::new(41_771_983_429_993_000)]);
    assert_eq!(e.user.as_ref().unwrap().username, "Luigi");
    assert!(e.require_colons && e.managed && e.animated && !e.available);
    let written = round_trip(&e);
    assert_eq!(written["available"], false);
    assert_eq!(written["animated"], true);
}

#[test]
fn partial_emoji_defaults() {
    let unicode: Emoji = parse(&json!({ "id": null, "name": "🔥" }));
    assert_eq!(unicode.id, None);
    assert_eq!(unicode.name.as_deref(), Some("🔥"));
    assert!(unicode.roles.is_empty() && unicode.user.is_none());
    assert!(!unicode.require_colons && !unicode.managed && !unicode.animated);
    assert!(unicode.available, "a left-out `available` must not read as unusable");

    // A reaction emoji whose custom emoji was deleted.
    let deleted: Emoji = parse(&json!({ "id": "1", "name": null }));
    assert_eq!(deleted.name, None);

    let null_roles: Emoji = parse(&json!({ "name": "x", "roles": null }));
    assert!(null_roles.roles.is_empty());
}

#[test]
fn emoji_writes_only_the_partial_form() {
    assert_eq!(serde_json::to_value(Emoji::unicode("🔥")).unwrap(), json!({ "name": "🔥" }));
    let custom = Emoji::custom(Id::new(1), "x", false);
    assert_eq!(serde_json::to_value(&custom).unwrap(), json!({ "id": "1", "name": "x" }));
    let animated = Emoji::custom(Id::new(1), "x", true);
    assert_eq!(serde_json::to_value(&animated).unwrap(), json!({ "id": "1", "name": "x", "animated": true }));
}

#[test]
fn emoji_identifier_is_what_a_reaction_route_takes() {
    assert_eq!(Emoji::custom(Id::new(1), "x", false).identifier().as_deref(), Some("x:1"));
    assert_eq!(Emoji::custom(Id::new(1), "x", true).identifier().as_deref(), Some("a:x:1"));
    assert_eq!(Emoji::unicode("🔥").identifier().as_deref(), Some("%F0%9F%94%A5"));
    // `encodeURIComponent` leaves exactly this set alone.
    assert_eq!(Emoji::unicode("azAZ09-_.!~*'()").identifier().as_deref(), Some("azAZ09-_.!~*'()"));
    assert_eq!(Emoji::unicode("a b:%/@").identifier().as_deref(), Some("a%20b%3A%25%2F%40"));
    let nameless: Emoji = parse(&json!({ "id": "1", "name": null }));
    assert_eq!(nameless.identifier(), None);
}

// Vectors from discord.js packages/formatters/__tests__/formatters.test.ts (Apache-2.0).
#[test]
fn emoji_display_matches_format_emoji() {
    let (still, moving) = (Id::new(851_461_487_498_493_952), Id::new(827_220_205_352_255_549));
    assert_eq!(Emoji::custom(still, "test", false).to_string(), "<:test:851461487498493952>");
    assert_eq!(Emoji::custom(moving, "test", true).to_string(), "<a:test:827220205352255549>");
    let nameless: Emoji = parse(&json!({ "id": "851461487498493952" }));
    assert_eq!(nameless.to_string(), "<:emoji:851461487498493952>");
    assert_eq!(Emoji::unicode("🔥").to_string(), "🔥");
    assert_eq!(parse::<Emoji>(&json!({})).to_string(), "");
}

#[test]
fn emoji_parse_follows_upstream() {
    const ID: &str = "851461487498493952";
    let check = |text: &str, id: Option<u64>, name: Option<&str>, animated: bool| {
        let e = Emoji::parse(text).unwrap_or_else(|| panic!("{text} should parse"));
        assert_eq!(e.id, id.map(Id::new), "{text}");
        assert_eq!(e.name.as_deref(), name, "{text}");
        assert_eq!(e.animated, animated, "{text}");
    };
    let id = 851_461_487_498_493_952;
    check("🔥", None, Some("🔥"), false);
    check("%F0%9F%94%A5", None, Some("🔥"), false);
    check(ID, Some(id), None, false);
    check(&format!("<:test:{ID}>"), Some(id), Some("test"), false);
    check(&format!("<a:test:{ID}>"), Some(id), Some("test"), true);
    check(&format!("test:{ID}"), Some(id), Some("test"), false);
    check(&format!("a:test:{ID}"), Some(id), Some("test"), true);
    check(&format!("<:te%73t:{ID}>"), Some(id), Some("test"), false);
    // The first `name:` wins, and what follows is not an id.
    check(&format!("te:st:{ID}"), None, Some("te"), false);
    check(&format!("hello <:test:{ID}> there"), Some(id), Some("test"), false);
    // The name is `ab`, since the pattern does not need the optional `a:` here.
    check(&format!("ab:{ID}"), Some(id), Some("ab"), false);
    // An id of the wrong length is dropped, leaving a name.
    check("test:123", None, Some("test"), false);
    check("test:", None, Some("test"), false);
    // Twenty digits take the first nineteen.
    check("test:12345678901234567890", Some(1_234_567_890_123_456_789), Some("test"), false);
    // A name of 33 word characters cannot match from its start, and the pattern is not anchored.
    check(&format!("{}:{ID}", "x".repeat(33)), Some(id), Some(&"x".repeat(32)), false);
    check(&format!("{}:{ID}", "x".repeat(32)), Some(id), Some(&"x".repeat(32)), false);
}

#[test]
fn emoji_parse_rejects_what_upstream_rejects() {
    for text in ["", ":", "a:b:1", "::", "é:1", "<:x:851461487498493952>"] {
        assert!(Emoji::parse(text).is_none(), "{text:?}");
    }
    // Upstream's `decodeURIComponent` throws on these.
    for text in ["%", "%F", "%GG", "%FF", "%E0%A4%A"] {
        assert!(Emoji::parse(text).is_none(), "{text:?}");
    }
}

#[test]
fn partial_message_is_only_ids() {
    let m = Message::partial(Id::new(5), Id::new(6));
    assert_eq!((m.id, m.channel_id), (Id::new(5), Id::new(6)));
    assert_eq!(m.author.id, Id::new(0));
    assert!(m.is_partial());
    assert!(m.referenced_message.is_absent());
    assert_eq!(m.kind, MessageType::DEFAULT);
    assert!(!parse::<Message>(&base()).is_partial());
    // Still decodable after a trip through the wire form.
    round_trip(&m);
}

#[test]
fn created_at_comes_from_the_id_not_the_timestamp() {
    let m: Message = parse(&with(base(), "timestamp", json!("2000-01-01T00:00:00+00:00")));
    // 334385199974967042 >> 22, plus the Discord epoch.
    assert_eq!(m.created_at_ms(), (334_385_199_974_967_042_i64 >> 22) + 1_420_070_400_000);
    assert_eq!(m.created_at_ms(), Id::<dr_core::marker::MessageMarker>::new(MESSAGE).timestamp_ms());
    assert_ne!(Some(m.created_at_ms()), m.timestamp.map(Timestamp::as_millis));
}

#[test]
fn in_guild_follows_guild_id() {
    assert!(!parse::<Message>(&base()).in_guild());
    assert!(parse::<Message>(&with(base(), "guild_id", json!("1"))).in_guild());
}

#[test]
fn system_message_types_match_upstream() {
    // `NonSystemMessageTypes` in discord.js packages/discord.js/src/util/Constants.js (Apache-2.0).
    for kind in [0, 19, 20, 23] {
        assert!(!parse::<Message>(&with(base(), "type", json!(kind))).is_system(), "{kind}");
    }
    for kind in [1, 7, 8, 18, 21, 24, 46] {
        assert!(parse::<Message>(&with(base(), "type", json!(kind))).is_system(), "{kind}");
    }
}

// Vectors from discord.js packages/formatters/__tests__/formatters.test.ts (Apache-2.0).
#[test]
fn url_matches_message_link() {
    let (channel, message) = (Id::new(123_456_789_012_345_678), Id::new(102_938_475_657_483));
    let dm = Message::partial(message, channel);
    assert_eq!(dm.url(), "https://discord.com/channels/@me/123456789012345678/102938475657483");
    let mut guild = dm;
    guild.guild_id = Some(Id::new(987_654_321_987_654));
    assert_eq!(guild.url(), "https://discord.com/channels/987654321987654/123456789012345678/102938475657483");
}

#[test]
fn message_display_is_its_content() {
    assert_eq!(parse::<Message>(&base()).to_string(), "hello");
}

#[test]
fn bad_field_types_fail_with_the_field_in_the_path() {
    let cases = [
        ("timestamp", json!("not a time")),
        ("mentions", json!(1)),
        ("flags", json!("nope")),
        ("reactions", json!([{ "count": 1 }])),
    ];
    for (key, value) in cases {
        let error = decode::<Message>(&with(base(), key, value).to_string()).unwrap_err();
        assert!(error.path.contains(key), "{key}: path was {:?}", error.path);
    }
}
