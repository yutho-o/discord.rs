//! Component tree against `payloads/v10/message.ts` and `_interactions/modalSubmit.ts` of
//! discord-api-types, and `findComponentByCustomId` of discord.js.

#![allow(clippy::unwrap_used, clippy::panic)] // test helpers are not `#[test]` functions, so clippy.toml does not cover them

use dr_core::decode;
use dr_model::component::{
    AutoPopulatedSelect, ChannelSelect, MessageComponentEmoji, SelectMenuDefaultValue, SelectMenuDefaultValueType,
};
use dr_model::{
    ActionRow, Button, ButtonStyle, ButtonView, ChannelType, Component, EmojiId, GenericId, SelectOption, StringSelect,
    TextInput, TextInputStyle, find_component_by_custom_id,
};
use serde::Deserialize;
use serde_json::{Value, json};

fn read(v: &Value) -> Component {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
}

/// Decoding and writing again keeps every field.
fn round_trips(v: Value) -> Component {
    let c = read(&v);
    assert_eq!(serde_json::to_value(&c).unwrap(), v);
    let text = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), v);
    c
}

fn deep_rows(depth: usize) -> String {
    format!("{}{}", r#"{"type":1,"components":["#.repeat(depth), "]}".repeat(depth))
}

#[test]
fn action_row_of_every_button_style() {
    let c = round_trips(json!({
        "type": 1,
        "id": 1,
        "components": [
            { "type": 2, "id": 2, "style": 1, "custom_id": "a", "label": "A" },
            { "type": 2, "style": 2, "custom_id": "b", "emoji": { "name": "\u{1f525}" } },
            { "type": 2, "style": 3, "custom_id": "c", "label": "C", "disabled": true },
            {
                "type": 2,
                "style": 4,
                "custom_id": "d",
                "label": "D",
                "emoji": { "id": "41771983429993937", "name": "x", "animated": true },
            },
            { "type": 2, "style": 5, "url": "https://example.com", "label": "E" },
            { "type": 2, "style": 6, "sku_id": "1088510058284990888" },
        ],
    }));
    let Component::ActionRow(row) = &c else { panic!("{c:?}") };
    assert_eq!(row.id, Some(1));
    assert_eq!(c.kind(), 1);
    assert_eq!(c.id(), Some(1));
    assert_eq!(c.custom_id(), None);
    let buttons: Vec<&Button> = row
        .components
        .iter()
        .map(|c| match c {
            Component::Button(b) => b,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(buttons.len(), 6);

    match buttons[0].view() {
        ButtonView::Interaction { style, custom_id, label, emoji } => {
            assert_eq!(style, ButtonStyle::PRIMARY);
            assert_eq!(custom_id, "a");
            assert_eq!(label, Some("A"));
            assert!(emoji.is_none());
        }
        other => panic!("{other:?}"),
    }
    match buttons[3].view() {
        ButtonView::Interaction { style, emoji: Some(e), .. } => {
            assert_eq!(style, ButtonStyle::DANGER);
            assert_eq!(e.id.map(|i| i.to_string()).as_deref(), Some("41771983429993937"));
            assert_eq!(e.animated, Some(true));
        }
        other => panic!("{other:?}"),
    }
    assert!(buttons[2].disabled);
    match buttons[4].view() {
        ButtonView::Link { url, label, .. } => {
            assert_eq!(url, "https://example.com");
            assert_eq!(label, Some("E"));
        }
        other => panic!("{other:?}"),
    }
    match buttons[5].view() {
        ButtonView::Premium { sku_id } => assert_eq!(sku_id.to_string(), "1088510058284990888"),
        other => panic!("{other:?}"),
    }

    // Guards from discord-api-types utils/v10.ts.
    assert_eq!(buttons.iter().map(|b| b.is_link()).collect::<Vec<_>>(), [false, false, false, false, true, false]);
    assert_eq!(buttons.iter().map(|b| b.is_interaction()).collect::<Vec<_>>(), [true, true, true, true, false, false]);
}

#[test]
fn button_with_unknown_style_is_kept_and_counts_as_interaction() {
    let c = round_trips(json!({ "type": 2, "style": 99, "custom_id": "future" }));
    let Component::Button(b) = &c else { panic!("{c:?}") };
    assert_eq!(b.style, ButtonStyle(99));
    assert!(b.is_interaction());
    assert!(!b.is_link());
    assert!(matches!(b.view(), ButtonView::Interaction { custom_id: "future", .. }));
}

#[test]
fn button_missing_the_field_of_its_style_is_malformed_not_an_error() {
    for (style, v) in [
        (ButtonStyle::PRIMARY, json!({ "type": 2, "style": 1, "url": "https://example.com" })),
        (ButtonStyle::LINK, json!({ "type": 2, "style": 5, "custom_id": "x" })),
        (ButtonStyle::PREMIUM, json!({ "type": 2, "style": 6, "custom_id": "x" })),
        (ButtonStyle(99), json!({ "type": 2, "style": 99 })),
    ] {
        let c = round_trips(v);
        let Component::Button(b) = &c else { panic!("{c:?}") };
        assert!(matches!(b.view(), ButtonView::Malformed { style: s } if s == style), "{b:?}");
    }
}

#[test]
fn button_without_style_fails_the_arm() {
    let e = serde_json::from_value::<Component>(json!({ "type": 2, "custom_id": "x" })).unwrap_err();
    assert!(e.to_string().contains("missing field `style`"), "{e}");
}

#[test]
fn emoji_of_discord_reads_and_writes_only_the_three_fields() {
    let c = read(&json!({
        "type": 2,
        "style": 1,
        "custom_id": "x",
        "emoji": { "id": null, "name": "\u{1f525}", "animated": false, "roles": [], "managed": false },
    }));
    assert_eq!(
        serde_json::to_value(&c).unwrap(),
        json!({ "type": 2, "style": 1, "custom_id": "x", "emoji": { "name": "\u{1f525}", "animated": false } })
    );

    let mut b = Button::new(ButtonStyle::SUCCESS);
    b.custom_id = Some("y".into());
    b.emoji = Some(MessageComponentEmoji::custom(EmojiId::new(7), "party"));
    assert_eq!(
        serde_json::to_value(Component::Button(b)).unwrap(),
        json!({ "type": 2, "style": 3, "custom_id": "y", "emoji": { "id": "7", "name": "party" } })
    );
    let unicode = MessageComponentEmoji::unicode("\u{1f525}");
    assert_eq!(serde_json::to_value(unicode).unwrap(), json!({ "name": "\u{1f525}" }));
}

#[test]
fn string_select_round_trips_and_reports_custom_id() {
    let c = round_trips(json!({
        "type": 3,
        "id": 5,
        "custom_id": "pick",
        "placeholder": "Choose",
        "min_values": 0,
        "max_values": 25,
        "disabled": true,
        "required": false,
        "options": [
            { "label": "A", "value": "a", "description": "first", "emoji": { "name": "\u{1f525}" }, "default": true },
            { "label": "B", "value": "b" },
        ],
    }));
    assert_eq!(c.custom_id(), Some("pick"));
    assert_eq!(c.id(), Some(5));
    let Component::StringSelect(s) = c else { panic!() };
    assert_eq!(s.options.len(), 2);
    assert!(s.options[0].default);
    assert!(!s.options[1].default);
    assert_eq!(s.max_values, Some(25));
}

#[test]
fn string_select_with_null_options_reads_as_empty() {
    let c = read(&json!({ "type": 3, "custom_id": "pick", "options": null }));
    let Component::StringSelect(s) = c else { panic!() };
    assert!(s.options.is_empty());
}

#[test]
fn auto_populated_selects_get_their_own_arms() {
    let user = json!({
        "type": 5,
        "custom_id": "u",
        "default_values": [{ "type": "user", "id": "1" }],
    });
    let role = json!({ "type": 6, "custom_id": "r", "default_values": [{ "type": "role", "id": "2" }] });
    let mentionable = json!({
        "type": 7,
        "custom_id": "m",
        "placeholder": "who",
        "min_values": 1,
        "max_values": 2,
        "default_values": [{ "type": "role", "id": "2" }, { "type": "user", "id": "1" }],
    });
    let channel = json!({
        "type": 8,
        "custom_id": "c",
        "channel_types": [0, 2, 999],
        "default_values": [{ "type": "channel", "id": "3" }],
    });

    let Component::UserSelect(s) = round_trips(user) else { panic!() };
    assert_eq!(s.custom_id, "u");
    assert_eq!(s.default_values[0].kind, SelectMenuDefaultValueType::User);
    let Component::RoleSelect(s) = round_trips(role) else { panic!() };
    assert_eq!(s.default_values[0].kind, SelectMenuDefaultValueType::Role);
    let Component::MentionableSelect(s) = round_trips(mentionable) else { panic!() };
    assert_eq!(s.default_values.len(), 2);
    let c = round_trips(channel);
    assert_eq!(c.custom_id(), Some("c"));
    let Component::ChannelSelect(s) = c else { panic!() };
    assert_eq!(s.default_values[0].id, GenericId::new(3));
    assert_eq!(s.channel_types, [ChannelType::GUILD_TEXT, ChannelType::GUILD_VOICE, ChannelType(999)]);
}

#[test]
fn unknown_default_value_type_is_kept() {
    let Component::UserSelect(s) =
        round_trips(json!({ "type": 5, "custom_id": "u", "default_values": [{ "type": "thread", "id": "1" }] }))
    else {
        panic!()
    };
    assert_eq!(s.default_values[0].kind.as_str(), "thread");
}

#[test]
fn select_constructors_write_minimal_payloads() {
    let mut user = AutoPopulatedSelect::new("u");
    user.default_values.push(SelectMenuDefaultValue::new(SelectMenuDefaultValueType::User, GenericId::new(1)));
    assert_eq!(
        serde_json::to_value(Component::UserSelect(user)).unwrap(),
        json!({ "type": 5, "custom_id": "u", "default_values": [{ "type": "user", "id": "1" }] })
    );
    assert_eq!(
        serde_json::to_value(Component::ChannelSelect(ChannelSelect::new("c"))).unwrap(),
        json!({ "type": 8, "custom_id": "c" })
    );
    let mut string = StringSelect::new("s");
    string.options.push(SelectOption::new("L", "v"));
    assert_eq!(
        serde_json::to_value(Component::StringSelect(string)).unwrap(),
        json!({ "type": 3, "custom_id": "s", "options": [{ "label": "L", "value": "v" }] })
    );
}

#[test]
fn text_input_definition_round_trips() {
    let c = round_trips(json!({
        "type": 4,
        "id": 9,
        "custom_id": "name",
        "style": 2,
        "label": "Name",
        "placeholder": "yours",
        "value": "prefill",
        "min_length": 1,
        "max_length": 4000,
        "required": false,
    }));
    let Component::TextInput(t) = c else { panic!() };
    assert_eq!(t.style, Some(TextInputStyle::PARAGRAPH));
    assert_eq!(t.max_length, Some(4000));
}

/// The modal submission shape of `_interactions/modalSubmit.ts`: a row of text inputs with only
/// `custom_id` and `value`, then kinds this tree keeps raw.
#[test]
fn modal_submission_components_decode() {
    let row_json =
        json!({ "type": 1, "id": 1, "components": [{ "type": 4, "id": 2, "custom_id": "name", "value": "typed" }] });
    let list: Vec<Component> = serde_json::from_value(json!([
        row_json,
        { "type": 18, "id": 3, "component": { "type": 21, "id": 4, "custom_id": "radio", "value": null } },
        { "type": 18, "id": 5, "component": { "type": 23, "id": 6, "custom_id": "check", "value": true } },
        { "type": 10, "id": 7 },
    ]))
    .unwrap();
    let Component::ActionRow(row) = &list[0] else { panic!() };
    let Component::TextInput(t) = &row.components[0] else { panic!() };
    assert_eq!(t.custom_id, "name");
    assert_eq!(t.value.as_deref(), Some("typed"));
    assert_eq!(t.style, None);
    assert_eq!(t.label, None);
    assert_eq!(serde_json::to_value(&list[0]).unwrap(), row_json);
    assert_eq!(list[1].kind(), 18);
    assert_eq!(list[1].id(), Some(3));
    assert_eq!(list[3].kind(), 10);
    assert!(matches!(list[1], Component::Unknown { .. }));
}

/// Shapes from `payloads/v10/message.ts`: every kind that is not typed here.
#[test]
fn every_untyped_kind_survives_byte_exact() {
    let cases = [
        r#"{"id":1,"type":9,"components":[{"type":10,"content":"hi"}],"accessory":{"type":11,"media":{"url":"attachment://a.png"}}}"#,
        r#"{"type":10,"content":"a \"quoted\" é text"}"#,
        r#"{"type":11,"media":{"url":"https://example.com/a.png"},"spoiler":false}"#,
        r#"{"type":12,"items":[{"media":{"url":"https://example.com/a.png"},"description":null}]}"#,
        r#"{"type":13,"file":{"url":"attachment://a.txt"},"name":"a.txt","size":1024}"#,
        r#"{"type":14,"divider":true,"spacing":2}"#,
        r#"{"type":16}"#,
        r#"{"type":17,"accent_color":null,"components":[]}"#,
        r#"{"type":18,"label":"L","component":{"type":4,"custom_id":"x","style":1}}"#,
        r#"{"type":19,"custom_id":"f","file_types":[".png"]}"#,
        r#"{"type":21,"custom_id":"r","options":[]}"#,
        r#"{"type":22,"custom_id":"g","options":[]}"#,
        r#"{"type":23,"custom_id":"b","default":true}"#,
    ];
    for text in cases {
        let c: Component = serde_json::from_str(text).unwrap();
        assert!(matches!(c, Component::Unknown { .. }), "{text}");
        assert_eq!(serde_json::to_string(&c).unwrap(), text);
    }
}

#[test]
fn unknown_kind_keeps_tag_and_bytes_when_nested_in_a_row() {
    let text = r#"{"components":[{"z":[1, 2],"type":777,"a":"b"},{"type":4000000000}],"type":1}"#;
    let c: Component = serde_json::from_str(text).unwrap();
    let Component::ActionRow(row) = &c else { panic!("{c:?}") };
    let Component::Unknown { kind, raw } = &row.components[0] else { panic!() };
    assert_eq!(*kind, 777);
    assert_eq!(raw.get(), r#"{"z":[1, 2],"type":777,"a":"b"}"#);
    assert_eq!(row.components[1].kind(), 4_000_000_000);
    assert_eq!(
        serde_json::to_string(&c).unwrap(),
        r#"{"type":1,"components":[{"z":[1, 2],"type":777,"a":"b"},{"type":4000000000}]}"#
    );
    assert_eq!(
        serde_json::to_value(&c).unwrap(),
        json!({ "type": 1, "components": [{ "z": [1, 2], "type": 777, "a": "b" }, { "type": 4_000_000_000u64 }] })
    );
}

#[test]
fn tag_may_come_last_and_a_row_may_have_no_children() {
    let c: Component = serde_json::from_str(r#"{"style":1,"custom_id":"x","type":2}"#).unwrap();
    assert!(matches!(c, Component::Button(_)));
    for text in [r#"{"type":1}"#, r#"{"type":1,"components":null}"#, r#"{"type":1,"components":[]}"#] {
        let Component::ActionRow(row) = serde_json::from_str(text).unwrap() else { panic!() };
        assert!(row.components.is_empty(), "{text}");
    }
}

#[test]
fn unknown_fields_are_ignored_on_typed_arms() {
    let c: Component = serde_json::from_str(r#"{"type":2,"style":1,"custom_id":"x","brand_new":{"a":1}}"#).unwrap();
    assert!(matches!(c, Component::Button(_)));
}

#[test]
fn missing_tag_or_bad_arm_fails_with_a_path_stopping_at_the_arm() {
    #[derive(Debug, Deserialize)]
    struct Holder {
        #[allow(dead_code)]
        components: Vec<Component>,
    }
    let e = decode::<Holder>(r#"{"components":[{"style":1}]}"#).unwrap_err();
    assert_eq!(e.path, "components[0]");
    assert!(e.source.to_string().contains("missing field `type`"), "{e:?}");

    let e = decode::<Holder>(r#"{"components":[{"type":1},{"type":2,"style":"big"}]}"#).unwrap_err();
    assert_eq!(e.path, "components[1]");

    let e = decode::<Holder>(r#"{"components":[{"type":"1"}]}"#).unwrap_err();
    assert_eq!(e.path, "components[0]");
    let e = decode::<Holder>(r#"{"components":[3]}"#).unwrap_err();
    assert_eq!(e.path, "components[0]");
}

/// Each union arm starts a fresh parser, which resets serde_json's recursion limit, so without a
/// limit of its own this payload overflows the stack and aborts the process.
#[test]
fn deeply_nested_rows_are_an_error_not_a_stack_overflow() {
    let e = serde_json::from_str::<Component>(&deep_rows(100_000)).unwrap_err();
    assert!(e.to_string().contains("nested too deeply"), "{e}");
    // The failure released its depth count: a shallow decode still works on this thread.
    assert!(serde_json::from_str::<Component>(&deep_rows(16)).is_ok());
    assert!(serde_json::from_str::<Component>(&deep_rows(17)).is_err());
    assert!(serde_json::from_str::<Component>(&deep_rows(2)).is_ok());
}

/// A sibling row and repeated decodes do not use up the limit.
#[test]
fn row_depth_limit_counts_nesting_not_rows() {
    let wide = format!(r#"{{"type":1,"components":[{}]}}"#, vec![deep_rows(15); 50].join(","));
    assert!(serde_json::from_str::<Component>(&wide).is_ok());
    for _ in 0..50 {
        assert!(serde_json::from_str::<Component>(&deep_rows(16)).is_ok());
    }
}

mod find {
    use super::*;

    // Tree shapes from discord.js packages/discord.js/src/util/Components.js (Apache-2.0).
    fn tree() -> Vec<Component> {
        serde_json::from_value(json!([
            { "type": 1, "components": [
                { "type": 2, "style": 5, "url": "https://example.com" },
                { "type": 2, "style": 1, "custom_id": "in-row" },
            ]},
            {
                "type": 9,
                "components": [{ "type": 10, "content": "text", "custom_id": "text-in-section" }],
                "accessory": { "type": 2, "style": 1, "custom_id": "accessory" },
            },
            { "type": 17, "components": [
                { "type": 10, "content": "x" },
                { "type": 1, "components": [{ "type": 3, "custom_id": "select-in-container", "options": [] }] },
                {
                    "type": 9,
                    "components": [],
                    "accessory": { "type": 2, "style": 2, "custom_id": "accessory-in-container" },
                },
                { "type": 17, "components": [
                    { "type": 1, "components": [{ "type": 2, "style": 1, "custom_id": "second-level" }] },
                ]},
            ]},
            { "type": 4, "custom_id": "top-input", "value": "v" },
            { "type": 777, "custom_id": "top-unknown" },
        ]))
        .unwrap()
    }

    #[test]
    fn flattens_like_upstream() {
        let tree = tree();
        for id in [
            "in-row",
            "accessory",
            "text-in-section",
            "select-in-container",
            "accessory-in-container",
            "second-level",
            "top-input",
            "top-unknown",
        ] {
            assert!(find_component_by_custom_id(&tree, id).is_some(), "{id}");
        }
        assert!(find_component_by_custom_id(&tree, "nope").is_none());
    }

    #[test]
    fn returns_the_component_with_its_type() {
        let tree = tree();
        let Some(Component::Button(b)) = find_component_by_custom_id(&tree, "accessory") else { panic!() };
        assert_eq!(b.style, ButtonStyle::PRIMARY);
        let Some(Component::StringSelect(_)) = find_component_by_custom_id(&tree, "select-in-container") else {
            panic!()
        };
        let Some(Component::TextInput(t)) = find_component_by_custom_id(&tree, "top-input") else { panic!() };
        assert_eq!(t.value.as_deref(), Some("v"));
        let Some(Component::Unknown { kind: 777, .. }) = find_component_by_custom_id(&tree, "top-unknown") else {
            panic!()
        };
    }

    #[test]
    fn first_in_document_order_wins() {
        let first = json!({ "type": 2, "style": 1, "custom_id": "x", "label": "first" });
        let second = json!({ "type": 2, "style": 1, "custom_id": "x", "label": "second" });
        let tree: Vec<Component> = serde_json::from_value(json!([
            { "type": 17, "components": [{ "type": 1, "components": [first] }] },
            { "type": 1, "components": [second] },
        ]))
        .unwrap();
        let Some(Component::Button(b)) = find_component_by_custom_id(&tree, "x") else { panic!() };
        assert_eq!(b.label.as_deref(), Some("first"));
    }

    #[test]
    fn a_row_is_flattened_only_one_level_as_upstream() {
        let tree: Vec<Component> = serde_json::from_value(json!([
            { "type": 1, "components": [{ "type": 1, "components": [{ "type": 2, "style": 1, "custom_id": "x" }] }] },
        ]))
        .unwrap();
        assert!(find_component_by_custom_id(&tree, "x").is_none());
    }

    #[test]
    fn escaped_custom_id_on_a_raw_arm_matches() {
        let tree: Vec<Component> = serde_json::from_str(r#"[{"type":777,"custom_id":"abc"}]"#).unwrap();
        assert!(find_component_by_custom_id(&tree, "abc").is_some());
    }

    #[test]
    fn malformed_raw_arms_are_skipped() {
        let tree: Vec<Component> = serde_json::from_value(json!([
            { "type": 17, "components": "not a list" },
            { "type": 9, "components": [{ "no": "tag" }] },
            { "type": 777, "custom_id": 5 },
            { "type": 1, "components": [{ "type": 2, "style": 1, "custom_id": "x" }] },
        ]))
        .unwrap();
        assert!(find_component_by_custom_id(&tree, "x").is_some());
    }

    #[test]
    fn container_depth_is_limited() {
        fn containers(depth: usize) -> String {
            let leaf = r#"{"type":2,"style":1,"custom_id":"x"}"#;
            format!("[{}{}{}]", r#"{"type":17,"components":["#.repeat(depth), leaf, "]}".repeat(depth))
        }
        let near: Vec<Component> = serde_json::from_str(&containers(8)).unwrap();
        assert!(find_component_by_custom_id(&near, "x").is_some());
        let far: Vec<Component> = serde_json::from_str(&containers(9)).unwrap();
        assert!(find_component_by_custom_id(&far, "x").is_none());
        let huge: Vec<Component> = serde_json::from_str(&containers(100_000)).unwrap();
        assert!(find_component_by_custom_id(&huge, "x").is_none());
    }
}

/// What the builders rely on.
#[test]
fn constructors_keep_the_builders_contract() {
    let mut b = Button::new(ButtonStyle::PRIMARY);
    b.custom_id = Some("id".into());
    b.label = Some("L".into());
    let row = ActionRow::new(vec![Component::Button(b)]);
    assert_eq!(
        serde_json::to_value(Component::ActionRow(row)).unwrap(),
        json!({ "type": 1, "components": [{ "type": 2, "style": 1, "custom_id": "id", "label": "L" }] })
    );
    assert_eq!(
        serde_json::to_value(Component::TextInput(TextInput::new("i", TextInputStyle::SHORT, "L"))).unwrap(),
        json!({ "type": 4, "custom_id": "i", "style": 1, "label": "L" })
    );
    let c = Component::Button(Button::new(ButtonStyle::LINK));
    assert_eq!(c.kind(), 2);
    assert_eq!(c.id(), None);
    assert_eq!(c.custom_id(), None);
}

/// The row limit has to fit well inside the smallest stack a decode runs on.
#[test]
fn decode_at_the_row_limit_fits_a_small_stack() {
    let run = std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
        assert!(serde_json::from_str::<Component>(&deep_rows(16)).is_ok());
        assert!(serde_json::from_str::<Component>(&deep_rows(100_000)).is_err());
    });
    run.unwrap().join().unwrap();
}

#[test]
fn size_of_component_stays_small() {
    let size = std::mem::size_of::<Component>();
    println!("size_of::<Component>() = {size}");
    assert!(size <= 256, "{size}");
}
