//! `Interaction`, its option tree and the callback bodies, against `payloads/v10/interactions.ts` and
//! `payloads/v10/_interactions/**` of discord-api-types, and the resolver rules of discord.js
//! (`CommandInteractionOptionResolver.js`, `ContextMenuCommandInteraction.js`, `BaseInteraction.js`,
//! `ModalComponentResolver.js`).

#![allow(clippy::unwrap_used, clippy::panic)] // test helpers are not `#[test]` functions, so clippy.toml does not cover them

use dr_core::{IntoBody, decode};
use dr_model::{
    ApplicationCommandOptionType, ApplicationCommandType, ApplicationIntegrationType, AutocompleteResponseData,
    ChannelType, CommandData, ComponentData, ComponentType, DeferredResponseData, GenericId, HoistedOptions,
    Interaction, InteractionContextType, InteractionData, InteractionOption, InteractionResponse,
    InteractionResponseType, InteractionType, MessageFlags, ModalResponseData, ModalSubmitComponent, ModalSubmitData,
    OptionValue, Permissions,
};
use serde_json::{Value, json};

const USER: &str =
    r#"{"id":"80351110224678912","username":"nelly","discriminator":"0","global_name":"Nelly","avatar":null}"#;

/// The envelope common to every kind, with `data` and the extra keys spliced in.
fn envelope(kind: u32, rest: &str) -> String {
    format!(
        r#"{{"id":"1100000000000000000","application_id":"775799577604522054","type":{kind},"token":"secret-interaction-token","version":1,"app_permissions":"442368","locale":"en-US","entitlements":[],"attachment_size_limit":10485760,"authorizing_integration_owners":{{"0":"290926798626357250"}},"context":0{rest}}}"#
    )
}

fn in_guild() -> String {
    format!(
        r#","guild_id":"290926798626357250","guild":{{"id":"290926798626357250","locale":"fr","features":["COMMUNITY","SOME_NEW_FEATURE"]}},"guild_locale":"fr","channel_id":"645027906669510667","channel":{{"id":"645027906669510667","type":0,"name":"general","guild_id":"290926798626357250"}},"member":{{"user":{USER},"roles":["539082325061763072"],"joined_at":"2017-03-13T19:19:14.040000+00:00","permissions":"2147483647","pending":false,"nick":null,"avatar":null}}"#
    )
}

fn data_of(kind: u32, data: &str) -> Interaction {
    decode(&envelope(kind, &format!(r#","data":{data}{}"#, in_guild()))).unwrap()
}

fn command(data: &str) -> CommandData {
    match data_of(2, data).data {
        Some(InteractionData::ApplicationCommand(d)) => d,
        other => panic!("expected a command, got {other:?}"),
    }
}

fn autocomplete(data: &str) -> CommandData {
    match data_of(4, data).data {
        Some(InteractionData::Autocomplete(d)) => d,
        other => panic!("expected autocomplete, got {other:?}"),
    }
}

fn component(data: &str) -> ComponentData {
    match data_of(3, data).data {
        Some(InteractionData::MessageComponent(d)) => d,
        other => panic!("expected a component, got {other:?}"),
    }
}

fn modal(data: &str) -> ModalSubmitData {
    match data_of(5, data).data {
        Some(InteractionData::ModalSubmit(d)) => d,
        other => panic!("expected a modal, got {other:?}"),
    }
}

/// Decode, encode, decode, encode: the second encoding must equal the first.
fn assert_stable(i: &Interaction) {
    let first = serde_json::to_string(i).unwrap();
    let again: Interaction = decode(&first).unwrap();
    assert_eq!(first, serde_json::to_string(&again).unwrap());
}

// ---- the envelope, `_interactions/base.ts` ---------------------------------------------------

#[test]
fn ping_has_no_data_and_no_locale() {
    // `ping.ts`: `Omit<APIBaseInteraction<InteractionType.Ping, undefined>, 'locale'>`.
    let i: Interaction = decode(
        r#"{"id":"1","application_id":"2","type":1,"token":"t","version":1,"entitlements":[],"attachment_size_limit":0,"app_permissions":"0","authorizing_integration_owners":{}}"#,
    )
    .unwrap();
    assert_eq!(i.kind, InteractionType::PING);
    assert!(i.data.is_none() && i.locale.is_none() && i.user.is_none() && i.member.is_none());
    assert!(i.is_ping());
    assert!(!i.is_repliable());
    assert!(i.invoker().is_none());
    assert_stable(&i);
}

#[test]
fn every_base_field_is_read() {
    let i = data_of(2, r#"{"id":"3","type":1,"name":"ping"}"#);
    assert_eq!(i.id.get(), 1100000000000000000);
    assert_eq!(i.application_id.get(), 775799577604522054);
    assert_eq!(i.token.expose(), "secret-interaction-token");
    assert_eq!(i.version, 1);
    assert_eq!(i.guild_id.unwrap().get(), 290926798626357250);
    let guild = i.guild.as_ref().unwrap();
    assert_eq!(guild.id, i.guild_id.unwrap());
    assert_eq!(guild.locale, "fr");
    assert_eq!(guild.features.len(), 2);
    assert_eq!(guild.features[1].as_str(), "SOME_NEW_FEATURE");
    assert_eq!(i.channel.as_ref().unwrap().kind, ChannelType::GUILD_TEXT);
    assert_eq!(i.channel_id.unwrap().get(), 645027906669510667);
    assert_eq!(i.member.as_ref().unwrap().permissions, Some(Permissions::from_bits_retain(2147483647)));
    assert_eq!(i.member_permissions(), Some(Permissions::from_bits_retain(2147483647)));
    assert_eq!(i.app_permissions, Some(Permissions::from_bits_retain(442368)));
    assert_eq!(i.locale.as_deref(), Some("en-US"));
    assert_eq!(i.guild_locale.as_deref(), Some("fr"));
    assert_eq!(i.context, Some(InteractionContextType::GUILD));
    assert_eq!(i.attachment_size_limit, 10485760);
    assert_eq!(
        i.authorizing_integration_owners.get(&ApplicationIntegrationType::GUILD_INSTALL).map(|v: &GenericId| v.get()),
        Some(290926798626357250)
    );
    assert_stable(&i);
}

#[test]
fn context_and_install_values_this_build_does_not_know_are_kept() {
    let i: Interaction = decode(
        r#"{"id":"1","application_id":"2","type":1,"token":"t","context":9,"authorizing_integration_owners":{"7":"5","1":"6"}}"#,
    )
    .unwrap();
    assert_eq!(i.context, Some(InteractionContextType(9)));
    assert_eq!(i.authorizing_integration_owners.get(&ApplicationIntegrationType(7)).map(|v| v.get()), Some(5));
    assert_eq!(
        i.authorizing_integration_owners.get(&ApplicationIntegrationType::USER_INSTALL).map(|v| v.get()),
        Some(6)
    );
    let back: Value = serde_json::to_value(&i).unwrap();
    assert_eq!(back["context"], json!(9));
    assert_eq!(back["authorizing_integration_owners"], json!({"1": "6", "7": "5"}));
}

#[test]
fn null_owner_map_and_null_data_are_empty() {
    let i: Interaction = decode(
        r#"{"id":"1","application_id":"2","type":1,"token":"t","data":null,"authorizing_integration_owners":null}"#,
    )
    .unwrap();
    assert!(i.data.is_none());
    assert!(i.authorizing_integration_owners.is_empty());
}

#[test]
fn debug_redacts_the_token_and_serialize_keeps_it() {
    let i = data_of(2, r#"{"id":"3","type":1,"name":"ping"}"#);
    let debug = format!("{i:?}");
    assert!(!debug.contains("secret-interaction-token"), "{debug}");
    assert!(debug.contains("Secret(***)"), "{debug}");
    let back: Value = serde_json::to_value(&i).unwrap();
    assert_eq!(back["token"], json!("secret-interaction-token"));
}

#[test]
fn channel_id_comes_from_channel_first() {
    // `[client 16.52]`: upstream reads `data.channel?.id`, not `channel_id`.
    let both: Interaction = decode(
        r#"{"id":"1","application_id":"2","type":2,"token":"t","channel_id":"10","channel":{"id":"20","type":1}}"#,
    )
    .unwrap();
    assert_eq!(both.channel_id.unwrap().get(), 20);

    let only_object: Interaction =
        decode(r#"{"id":"1","application_id":"2","type":2,"token":"t","channel":{"id":"20","type":1}}"#).unwrap();
    assert_eq!(only_object.channel_id.unwrap().get(), 20);

    let only_id: Interaction =
        decode(r#"{"id":"1","application_id":"2","type":2,"token":"t","channel_id":"10"}"#).unwrap();
    assert_eq!(only_id.channel_id.unwrap().get(), 10);
}

#[test]
fn unknown_channel_kind_in_the_partial_channel_is_kept() {
    let i: Interaction =
        decode(r#"{"id":"1","application_id":"2","type":2,"token":"t","channel":{"id":"20","type":777}}"#).unwrap();
    assert_eq!(i.channel.unwrap().kind, ChannelType(777));
}

#[test]
fn invoker_in_a_guild_comes_from_the_member_and_in_a_dm_from_user() {
    // `BaseInteraction`: `data.user ?? data.member.user`.
    let guild = data_of(2, r#"{"id":"3","type":1,"name":"ping"}"#);
    assert_eq!(guild.invoker().unwrap().username, "nelly");
    assert!(guild.is_guild() && guild.in_guild() && !guild.is_dm());

    let dm: Interaction = decode(&envelope(
        2,
        &format!(r#","data":{{"id":"3","type":1,"name":"ping"}},"user":{USER},"channel":{{"id":"9","type":1}}"#),
    ))
    .unwrap();
    assert_eq!(dm.invoker().unwrap().username, "nelly");
    assert!(dm.is_dm() && !dm.is_guild() && !dm.in_guild());
    assert_eq!(dm.member_permissions(), None);
}

#[test]
fn guild_id_without_member_is_not_in_guild() {
    let i: Interaction = decode(r#"{"id":"1","application_id":"2","type":2,"token":"t","guild_id":"5"}"#).unwrap();
    assert!(i.is_guild());
    assert!(!i.in_guild());
}

#[test]
fn missing_required_fields_fail_with_a_path() {
    let no_token = decode::<Interaction>(r#"{"id":"1","application_id":"2","type":2}"#).unwrap_err();
    assert!(no_token.to_string().contains("token"), "{no_token}");
    let no_id = decode::<Interaction>(r#"{"application_id":"2","type":2,"token":"t"}"#).unwrap_err();
    assert!(no_id.to_string().contains("id"), "{no_id}");
    assert!(decode::<Interaction>(r#"{"id":"1","type":2,"token":"t"}"#).is_err());
    assert!(decode::<Interaction>(r#"{"id":"1","application_id":"2","token":"t"}"#).is_err());
}

#[test]
fn bad_member_is_a_decode_failure_of_the_event_with_a_path() {
    let e = decode::<Interaction>(r#"{"id":"1","application_id":"2","type":2,"token":"t","member":{"roles":7}}"#)
        .unwrap_err();
    assert!(e.path.contains("member"), "{e}");
}

#[test]
fn unknown_keys_are_ignored_and_key_order_does_not_matter() {
    // `type` after `data`: the arm is chosen once the whole object has been read.
    let i: Interaction = decode(
        r#"{"future_key":{"a":[1,2]},"data":{"name":"ping","future":true,"type":1,"id":"3"},"token":"t","type":2,"application_id":"2","id":"1"}"#,
    )
    .unwrap();
    assert_eq!(i.command_data().unwrap().name, "ping");
}

// ---- `data` chosen by `type` -------------------------------------------------------------------

#[test]
fn an_unknown_interaction_kind_keeps_its_data_byte_exact() {
    let raw = r#"{"weird"  :  [1, 2.50, {"x":null}],"type":"text"}"#;
    let i: Interaction =
        decode(&format!(r#"{{"id":"1","application_id":"2","type":777,"token":"t","data":{raw}}}"#)).unwrap();
    assert_eq!(i.kind, InteractionType(777));
    match &i.data {
        Some(InteractionData::Unknown { raw: kept }) => assert_eq!(kept.get(), raw),
        other => panic!("{other:?}"),
    }
    assert!(serde_json::to_string(&i).unwrap().contains(raw));
    assert!(i.is_repliable(), "upstream only excludes ping and autocomplete");
    assert!(!i.is_command() && !i.is_message_component() && !i.is_modal_submit() && !i.is_autocomplete());
}

#[test]
fn a_ping_with_data_is_kept_raw() {
    let i: Interaction = decode(r#"{"id":"1","application_id":"2","type":1,"token":"t","data":{"a":1}}"#).unwrap();
    assert!(matches!(i.data, Some(InteractionData::Unknown { .. })));
}

#[test]
fn data_that_does_not_decode_keeps_the_interaction_and_its_token() {
    // The router sends this to `Interaction::Unknown` and counts it. It must never fail the shard, and
    // it must leave enough to answer.
    let raw = r#"{"id":"not a snowflake","type":1,"name":"x"}"#;
    let i = data_of(2, raw);
    assert_eq!(i.token.expose(), "secret-interaction-token");
    match &i.data {
        Some(InteractionData::Malformed { raw: kept, error }) => {
            assert_eq!(kept.get(), raw);
            assert!(error.contains("id"), "{error}");
        }
        other => panic!("{other:?}"),
    }
    assert!(i.command_data().is_none());
    assert!(i.is_command(), "the kind still says what it was");
    assert!(!i.is_chat_input_command());
    assert!(serde_json::to_string(&i).unwrap().contains(raw));
}

#[test]
fn malformed_data_on_every_known_kind() {
    for (kind, raw) in [
        (2, r#"{"id":"3"}"#),            // no `type`
        (4, r#"{"type":1,"name":"x"}"#), // no `id`
        (3, r#"{"custom_id":"x"}"#),     // no `component_type`
        (3, r#"{"component_type":"nope"}"#),
        (5, r#"{"custom_id":"x","components":[{"id":1}]}"#), // a component with no tag
        (5, r#"{"custom_id":"x","components":[{"type":18}]}"#), // a label with no component
        (5, r#"{"custom_id":"x","components":7}"#),
    ] {
        let i = data_of(kind, raw);
        assert!(matches!(i.data, Some(InteractionData::Malformed { .. })), "{raw}: {:?}", i.data);
    }
}

#[test]
fn data_of_the_wrong_json_type_does_not_fail_the_interaction() {
    assert!(matches!(data_of(3, "7").data, Some(InteractionData::Malformed { .. })));
    assert!(matches!(data_of(3, r#""text""#).data, Some(InteractionData::Malformed { .. })));
}

// ---- commands, `_applicationCommands/**` --------------------------------------------------------

const ALL_OPTIONS: &str = r#"{
    "id":"771825006014889984","type":1,"name":"blep","guild_id":"290926798626357250",
    "options":[
        {"name":"s","type":3,"value":"café \"quoted\""},
        {"name":"i","type":4,"value":-42},
        {"name":"b","type":5,"value":true},
        {"name":"u","type":6,"value":"80351110224678912"},
        {"name":"c","type":7,"value":"645027906669510667"},
        {"name":"r","type":8,"value":"539082325061763072"},
        {"name":"mu","type":9,"value":"80351110224678912"},
        {"name":"mr","type":9,"value":"539082325061763072"},
        {"name":"n","type":10,"value":1.5},
        {"name":"a","type":11,"value":"1000000000000000001"}
    ],
    "resolved":{
        "users":{"80351110224678912":{"id":"80351110224678912","username":"nelly","discriminator":"0","global_name":null,"avatar":null}},
        "members":{"80351110224678912":{"roles":[],"joined_at":"2017-03-13T19:19:14.040000+00:00","permissions":"8","pending":false,"nick":"nel","avatar":null}},
        "roles":{"539082325061763072":{"id":"539082325061763072","name":"mods","permissions":"8","position":1,"color":0,"hoist":false,"managed":false,"mentionable":true}},
        "channels":{"645027906669510667":{"id":"645027906669510667","type":0,"name":"general","permissions":"3072","app_permissions":"1024"}},
        "attachments":{"1000000000000000001":{"id":"1000000000000000001","filename":"a.png","size":12,"url":"https://x/a.png","proxy_url":"https://y/a.png"}}
    }
}"#;

#[test]
fn chat_input_with_every_option_kind() {
    let i = data_of(2, ALL_OPTIONS);
    assert!(i.is_command() && i.is_chat_input_command() && !i.is_context_menu_command());
    assert!(i.is_repliable() && !i.is_autocomplete());
    let d = i.command_data().unwrap();
    assert_eq!(d.id.get(), 771825006014889984);
    assert_eq!(d.kind, ApplicationCommandType::CHAT_INPUT);
    assert_eq!(d.name, "blep");
    assert_eq!(d.guild_id.unwrap().get(), 290926798626357250);

    let o = d.hoisted();
    assert!(o.group.is_none() && o.subcommand.is_none());
    assert_eq!(o.options.len(), 10);
    assert_eq!(o.string("s"), Some("café \"quoted\""));
    assert_eq!(o.integer("i"), Some(-42));
    assert_eq!(o.boolean("b"), Some(true));
    assert_eq!(o.number("n"), Some(1.5));
    assert_eq!(o.user("u").unwrap().get(), 80351110224678912);
    assert_eq!(o.channel("c").unwrap().get(), 645027906669510667);
    assert_eq!(o.role("r").unwrap().get(), 539082325061763072);
    assert_eq!(o.attachment("a").unwrap().get(), 1000000000000000001);
    assert_eq!(o.mentionable("mu").unwrap().get(), 80351110224678912);
    assert_eq!(o.mentionable("mr").unwrap().get(), 539082325061763072);
    assert!(o.get("nope").is_none());
    assert_stable(&i);
}

#[test]
fn a_getter_of_the_wrong_kind_gives_none() {
    // Upstream throws `CommandInteractionOptionType`. The facade turns `None` into its error.
    let d = command(ALL_OPTIONS);
    let o = d.hoisted();
    assert_eq!(o.string("i"), None);
    assert_eq!(o.integer("s"), None);
    assert_eq!(o.integer("n"), None);
    assert_eq!(o.number("i"), None);
    assert_eq!(o.boolean("s"), None);
    assert_eq!(o.user("r"), None);
    assert_eq!(o.role("u"), None);
    assert_eq!(o.channel("u"), None);
    assert_eq!(o.attachment("u"), None);
    assert_eq!(o.mentionable("u"), None);
}

#[test]
fn a_mentionable_answers_only_for_what_it_names() {
    // `getUser` and `getRole` accept `Mentionable` and read `option.user` and `option.role`, which only
    // the matching entry of `resolved` fills in.
    let d = command(ALL_OPTIONS);
    let o = d.hoisted();
    assert_eq!(o.user("mu").unwrap().get(), 80351110224678912);
    assert_eq!(o.role("mu"), None);
    assert_eq!(o.role("mr").unwrap().get(), 539082325061763072);
    assert_eq!(o.user("mr"), None);

    let bare = HoistedOptions::new(&d.options, None);
    assert_eq!(bare.user("mu"), None);
    assert_eq!(bare.role("mr"), None);
    assert_eq!(bare.user("u").unwrap().get(), 80351110224678912, "a plain user option needs no resolved data");
}

#[test]
fn resolved_data_is_found_by_the_option_value() {
    let d = command(ALL_OPTIONS);
    let resolved = d.resolved.as_ref().unwrap();
    let o = d.hoisted();
    assert_eq!(resolved.user_of(o.get("u").unwrap()).unwrap().username, "nelly");
    assert_eq!(resolved.member_of(o.get("u").unwrap()).unwrap().nick.as_deref(), Some("nel"));
    assert_eq!(resolved.role_of(o.get("r").unwrap()).unwrap().name, "mods");
    let channel = resolved.channel_of(o.get("c").unwrap()).unwrap();
    assert_eq!(channel.kind, ChannelType::GUILD_TEXT);
    assert_eq!(channel.name.as_deref(), Some("general"));
    assert_eq!(channel.permissions, Some(Permissions::from_bits_retain(3072)));
    assert_eq!(channel.app_permissions, Some(Permissions::from_bits_retain(1024)));
    assert_eq!(resolved.attachment_of(o.get("a").unwrap()).unwrap().filename, "a.png");
    assert!(resolved.user_of(o.get("s").unwrap()).is_none(), "a string is never looked up");
}

#[test]
fn resolved_member_merges_with_its_user() {
    // `transformOption`: `{ user, ...member }`.
    let d = command(ALL_OPTIONS);
    let resolved = d.resolved.as_ref().unwrap();
    let id = dr_model::UserId::new(80351110224678912);
    assert!(resolved.members.get(&id).unwrap().user.is_none());
    let merged = resolved.member_with_user(id).unwrap();
    assert_eq!(merged.user.unwrap().username, "nelly");
    assert_eq!(merged.nick.as_deref(), Some("nel"));
    assert_eq!(merged.permissions, Some(Permissions::ADMINISTRATOR));
    assert!(resolved.member_with_user(dr_model::UserId::new(1)).is_none());
}

#[test]
fn no_options_and_null_lists() {
    let d = command(r#"{"id":"3","type":1,"name":"ping"}"#);
    assert!(d.options.is_empty() && d.resolved.is_none() && d.target_id.is_none());
    let d = command(r#"{"id":"3","type":1,"name":"ping","options":null,"resolved":{"users":null}}"#);
    assert!(d.options.is_empty());
    assert!(d.resolved.unwrap().users.is_empty());
}

#[test]
fn subcommand_is_hoisted() {
    let d = command(
        r#"{"id":"3","type":1,"name":"perm","options":[{"name":"get","type":1,"options":[{"name":"who","type":6,"value":"5"}]}]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.group, None);
    assert_eq!(o.subcommand, Some("get"));
    assert_eq!(o.user("who").unwrap().get(), 5);
    assert_eq!(d.options.len(), 1, "the full tree stays as received");
    assert_eq!(d.options[0].kind, ApplicationCommandOptionType::SUB_COMMAND);
    assert!(d.options[0].value.is_none());
}

#[test]
fn subcommand_group_is_hoisted() {
    let d = command(
        r#"{"id":"3","type":1,"name":"perm","options":[{"name":"user","type":2,"options":[{"name":"get","type":1,"options":[{"name":"n","type":4,"value":7}]}]}]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.group, Some("user"));
    assert_eq!(o.subcommand, Some("get"));
    assert_eq!(o.integer("n"), Some(7));
}

#[test]
fn hoisting_empty_subcommand_and_missing_options() {
    let d = command(r#"{"id":"3","type":1,"name":"x","options":[{"name":"run","type":1}]}"#);
    let o = d.hoisted();
    assert_eq!(o.subcommand, Some("run"));
    assert!(o.options.is_empty());

    let d = command(r#"{"id":"3","type":1,"name":"x","options":[{"name":"g","type":2,"options":null}]}"#);
    let o = d.hoisted();
    assert_eq!(o.group, Some("g"));
    assert_eq!(o.subcommand, None);
    assert!(o.options.is_empty());
}

#[test]
fn only_a_leading_subcommand_is_hoisted() {
    let d = command(
        r#"{"id":"3","type":1,"name":"x","options":[{"name":"s","type":3,"value":"a"},{"name":"run","type":1,"options":[]}]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.subcommand, None);
    assert_eq!(o.options.len(), 2);
}

#[test]
fn integer_tolerance() {
    let d = command(
        r#"{"id":"3","type":1,"name":"x","options":[
            {"name":"float","type":4,"value":1.0},
            {"name":"max","type":4,"value":9007199254740991},
            {"name":"frac","type":4,"value":1.5},
            {"name":"huge","type":4,"value":18446744073709551615},
            {"name":"null","type":4,"value":null}
        ]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.integer("float"), Some(1));
    assert_eq!(o.integer("max"), Some(9007199254740991));
    assert_eq!(o.integer("frac"), None);
    assert!(matches!(o.get("frac").unwrap().value, Some(OptionValue::Unknown(_))));
    assert_eq!(o.integer("huge"), None);
    assert!(o.get("null").unwrap().value.is_none());
}

#[test]
fn number_accepts_an_integer_on_the_wire() {
    let d = command(r#"{"id":"3","type":1,"name":"x","options":[{"name":"n","type":10,"value":3}]}"#);
    assert_eq!(d.hoisted().number("n"), Some(3.0));
}

#[test]
fn an_unknown_option_kind_keeps_its_value_byte_exact() {
    let raw = r#"{"nested" : [1, 2]}"#;
    let text = format!(r#"{{"value":{raw},"name":"new","type":99}}"#);
    let o: InteractionOption = decode(&text).unwrap();
    assert_eq!(o.kind, ApplicationCommandOptionType(99));
    match &o.value {
        Some(OptionValue::Unknown(kept)) => assert_eq!(kept.get(), raw),
        other => panic!("{other:?}"),
    }
    assert!(serde_json::to_string(&o).unwrap().contains(raw));
}

#[test]
fn a_value_of_an_unexpected_shape_does_not_lose_the_interaction() {
    let d = command(
        r#"{"id":"3","type":1,"name":"x","options":[
            {"name":"s","type":3,"value":5},
            {"name":"b","type":5,"value":"yes"},
            {"name":"u","type":6,"value":"not a snowflake"},
            {"name":"i","type":4,"value":{"a":1}}
        ]}"#,
    );
    let o = d.hoisted();
    for name in ["s", "b", "u", "i"] {
        assert!(matches!(o.get(name).unwrap().value, Some(OptionValue::Unknown(_))), "{name}");
    }
    assert_eq!(o.string("s"), None);
    assert_eq!(o.boolean("b"), None);
    assert_eq!(o.user("u"), None);
    assert_eq!(o.integer("i"), None);
}

#[test]
fn an_option_with_no_type_fails_the_data_only() {
    let i = data_of(2, r#"{"id":"3","type":1,"name":"x","options":[{"name":"s","value":"a"}]}"#);
    match &i.data {
        Some(InteractionData::Malformed { error, .. }) => assert!(error.contains("options"), "{error}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn value_key_order_does_not_matter() {
    let a: InteractionOption = decode(r#"{"value":7,"type":4,"name":"n"}"#).unwrap();
    let b: InteractionOption = decode(r#"{"name":"n","type":4,"value":7}"#).unwrap();
    assert_eq!(a.as_i64(), Some(7));
    assert_eq!(b.as_i64(), Some(7));
}

#[test]
fn options_write_back_their_wire_form() {
    let text = r#"{"name":"g","type":2,"options":[{"name":"c","type":1,"options":[{"name":"s","type":3,"value":"a"},{"name":"i","type":4,"value":-1},{"name":"n","type":10,"value":1.5},{"name":"b","type":5,"value":false},{"name":"u","type":6,"value":"5"},{"name":"p","type":4,"value":"12","focused":true}]}]}"#;
    let o: InteractionOption = decode(text).unwrap();
    assert_eq!(serde_json::to_string(&o).unwrap(), text);
}

#[test]
fn user_context_menu() {
    let i = data_of(
        2,
        r#"{"id":"3","type":2,"name":"High Five","target_id":"80351110224678912","resolved":{
            "users":{"80351110224678912":{"id":"80351110224678912","username":"nelly","discriminator":"0","global_name":null,"avatar":null}},
            "members":{"80351110224678912":{"roles":[],"joined_at":null,"permissions":"8","pending":false,"nick":"nel"}}
        }}"#,
    );
    assert!(i.is_context_menu_command() && i.is_user_context_menu_command());
    assert!(!i.is_message_context_menu_command() && !i.is_chat_input_command());
    let d = i.command_data().unwrap();
    assert_eq!(d.target_id.unwrap().get(), 80351110224678912);
    assert_eq!(d.target_user().unwrap().username, "nelly");
    assert_eq!(d.target_member().unwrap().nick.as_deref(), Some("nel"));
    assert!(d.target_message().is_none());
    assert_stable(&i);
}

#[test]
fn message_context_menu() {
    let i = data_of(
        2,
        &format!(
            r#"{{"id":"3","type":3,"name":"Bookmark","target_id":"900","resolved":{{"messages":{{"900":{{"id":"900","channel_id":"645027906669510667","author":{USER},"content":"hello","timestamp":"2021-05-19T02:12:51.710000+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0}}}}}}}}"#
        ),
    );
    assert!(i.is_context_menu_command() && i.is_message_context_menu_command());
    let d = i.command_data().unwrap();
    assert_eq!(d.kind, ApplicationCommandType::MESSAGE);
    let message = d.target_message().unwrap();
    assert_eq!(message.content, "hello");
    assert_eq!(message.author.username, "nelly");
    assert!(d.target_user().is_none());
    assert_stable(&i);
}

#[test]
fn context_menu_without_resolved_has_no_target_object() {
    let d = command(r#"{"id":"3","type":2,"name":"x","target_id":"5"}"#);
    assert!(d.target_user().is_none() && d.target_member().is_none() && d.target_message().is_none());
    assert_eq!(d.target_id.unwrap().get(), 5);
}

#[test]
fn primary_entry_point() {
    let i = data_of(2, r#"{"id":"3","type":4,"name":"launch"}"#);
    assert!(i.is_primary_entry_point_command());
    assert!(!i.is_chat_input_command() && !i.is_context_menu_command());
    assert!(i.is_command());
}

#[test]
fn an_unknown_command_kind_is_kept() {
    let i = data_of(2, r#"{"id":"3","type":55,"name":"x"}"#);
    assert_eq!(i.command_data().unwrap().kind, ApplicationCommandType(55));
    assert!(i.is_command() && !i.is_chat_input_command() && !i.is_context_menu_command());
}

// ---- autocomplete, `autocomplete.ts` ------------------------------------------------------------

#[test]
fn autocomplete_integer_and_number_arrive_as_text() {
    // `_chatInput/integer.ts`, `number.ts`: `Type extends ApplicationCommandAutocomplete ? string : number`.
    let i = data_of(
        4,
        r#"{"id":"3","type":1,"name":"find","options":[
            {"name":"count","type":4,"value":"12"},
            {"name":"ratio","type":10,"value":"1.","focused":true},
            {"name":"q","type":3,"value":"ab"}
        ]}"#,
    );
    assert!(i.is_autocomplete() && !i.is_command() && !i.is_repliable());
    assert!(!i.is_chat_input_command(), "upstream requires type ApplicationCommand");
    let d = i.command_data().unwrap();
    let o = d.hoisted();
    let focused = o.focused().unwrap();
    assert_eq!(focused.name, "ratio");
    assert!(matches!(focused.value, Some(OptionValue::Partial(_))));
    assert_eq!(focused.typed_text(), Some("1."));
    assert_eq!(o.get("count").unwrap().typed_text(), Some("12"));
    assert_eq!(o.integer("count"), None, "text is not an integer yet");
    assert_eq!(o.number("ratio"), None);
    assert_eq!(o.get("q").unwrap().typed_text(), Some("ab"));
    assert_eq!(o.string("q"), Some("ab"));
    assert_stable(&i);
}

#[test]
fn autocomplete_with_nothing_typed_or_not_a_number() {
    let d = autocomplete(
        r#"{"id":"3","type":1,"name":"find","options":[{"name":"n","type":4,"value":"","focused":true},{"name":"m","type":10,"value":"abc"}]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.focused().unwrap().typed_text(), Some(""));
    assert_eq!(o.get("m").unwrap().typed_text(), Some("abc"));
}

#[test]
fn autocomplete_focus_inside_a_subcommand() {
    let d = autocomplete(
        r#"{"id":"3","type":1,"name":"find","options":[{"name":"g","type":2,"options":[{"name":"s","type":1,"options":[{"name":"q","type":3,"value":"x","focused":true}]}]}]}"#,
    );
    let o = d.hoisted();
    assert_eq!(o.group, Some("g"));
    assert_eq!(o.subcommand, Some("s"));
    assert_eq!(o.focused().unwrap().name, "q");
}

#[test]
fn no_focused_option_is_none() {
    let d = autocomplete(r#"{"id":"3","type":1,"name":"find","options":[{"name":"q","type":3,"value":"x"}]}"#);
    assert!(d.hoisted().focused().is_none());
    assert!(!d.options[0].focused);
}

// ---- components, `messageComponents.ts` --------------------------------------------------------

#[test]
fn button_click_with_its_message() {
    let i: Interaction = decode(&envelope(
        3,
        &format!(
            r#","data":{{"custom_id":"vote:yes","component_type":2}},"message":{{"id":"900","channel_id":"645027906669510667","author":{USER},"content":"Vote","components":[{{"type":1,"components":[{{"type":2,"style":1,"custom_id":"vote:yes","label":"Yes"}}]}}],"flags":64}}{}"#,
            in_guild()
        ),
    ))
    .unwrap();
    assert!(i.is_message_component() && i.is_button() && i.is_repliable());
    assert!(!i.is_select_menu());
    let d = i.component_data().unwrap();
    assert_eq!(d.custom_id, "vote:yes");
    assert_eq!(d.component_type, ComponentType::BUTTON);
    assert!(d.values.is_empty() && d.resolved.is_none());
    let message = i.message.as_ref().unwrap();
    assert_eq!(message.content, "Vote");
    assert_eq!(message.components.len(), 1);
    assert_eq!(message.flags, Some(MessageFlags::EPHEMERAL));
    assert_stable(&i);
}

#[test]
fn each_select_menu_kind() {
    type Guard = fn(&Interaction) -> bool;
    let cases: [(u16, Guard); 5] = [
        (3, Interaction::is_string_select_menu),
        (5, Interaction::is_user_select_menu),
        (6, Interaction::is_role_select_menu),
        (7, Interaction::is_mentionable_select_menu),
        (8, Interaction::is_channel_select_menu),
    ];
    for (kind, guard) in cases {
        let i = data_of(3, &format!(r#"{{"custom_id":"s","component_type":{kind},"values":["a","b"]}}"#));
        assert!(i.is_select_menu(), "{kind}");
        assert!(guard(&i), "{kind}");
        assert!(!i.is_button(), "{kind}");
        for (other, other_guard) in cases {
            assert_eq!(other_guard(&i), other == kind, "{kind} against {other}");
        }
        assert_eq!(i.component_data().unwrap().values, ["a", "b"]);
    }
}

#[test]
fn auto_populated_select_with_resolved_data() {
    let d = component(&format!(
        r#"{{"custom_id":"who","component_type":5,"values":["80351110224678912"],"resolved":{{"users":{{"80351110224678912":{USER}}},"members":{{"80351110224678912":{{"roles":[],"permissions":"8"}}}}}}}}"#
    ));
    assert_eq!(d.component_type, ComponentType::USER_SELECT);
    let resolved = d.resolved.unwrap();
    assert_eq!(resolved.users.get(&dr_model::UserId::new(80351110224678912)).unwrap().username, "nelly");
    assert_eq!(resolved.members.len(), 1);
}

#[test]
fn an_unknown_component_kind_is_kept_and_is_no_select() {
    let i = data_of(3, r#"{"custom_id":"s","component_type":77,"values":null}"#);
    assert_eq!(i.component_data().unwrap().component_type, ComponentType(77));
    assert!(i.is_message_component());
    assert!(!i.is_button() && !i.is_select_menu());
    assert!(i.component_data().unwrap().values.is_empty());
}

// ---- modals, `modalSubmit.ts` -------------------------------------------------------------------

#[test]
fn legacy_action_rows_with_text_inputs() {
    let i = data_of(
        5,
        r#"{"custom_id":"feedback","components":[
            {"type":1,"components":[{"type":4,"custom_id":"name","value":"Nelly"}]},
            {"type":1,"components":[{"type":4,"custom_id":"about","value":""}]}
        ]}"#,
    );
    assert!(i.is_modal_submit() && i.is_repliable());
    let d = i.modal_data().unwrap();
    assert_eq!(d.custom_id, "feedback");
    assert_eq!(d.text_input_value("name"), Some("Nelly"));
    assert_eq!(d.text_input_value("about"), Some(""));
    assert_eq!(d.text_input_value("nope"), None);
    assert_eq!(d.text_inputs(), [("name", "Nelly"), ("about", "")]);
    assert_stable(&i);
}

#[test]
fn labels_with_every_leaf_kind() {
    let text = r#"{"custom_id":"m","components":[
        {"type":10,"id":1},
        {"type":18,"id":2,"component":{"type":4,"id":3,"custom_id":"text","value":"hello"}},
        {"type":18,"component":{"type":3,"custom_id":"string","values":["a","b"]}},
        {"type":18,"component":{"type":5,"custom_id":"user","values":["11"]}},
        {"type":18,"component":{"type":6,"custom_id":"role","values":["12"]}},
        {"type":18,"component":{"type":7,"custom_id":"mention","values":["11","12"]}},
        {"type":18,"component":{"type":8,"custom_id":"chan","values":["13"]}},
        {"type":18,"component":{"type":19,"custom_id":"file","values":["14"]}},
        {"type":18,"component":{"type":21,"custom_id":"radio","value":null}},
        {"type":18,"component":{"type":21,"custom_id":"radio2","value":"x"}},
        {"type":18,"component":{"type":22,"custom_id":"group","values":["p","q"]}},
        {"type":18,"component":{"type":23,"custom_id":"tick","value":true}}
    ],"resolved":{"users":{"11":{"id":"11","username":"a"}},"roles":{"12":{"id":"12","permissions":"0"}},"channels":{"13":{"id":"13","type":0,"name":"c"}},"attachments":{"14":{"id":"14","filename":"f"}}}}"#;
    let i = data_of(5, text);
    let d = i.modal_data().unwrap();
    assert_eq!(d.components.len(), 12);
    assert_eq!(d.components[0].kind(), 10);
    let hoisted = d.hoisted();
    assert_eq!(hoisted.len(), 11, "the top-level text display is ignored");

    assert_eq!(d.text_input_value("text"), Some("hello"));
    assert_eq!(d.text_inputs(), [("text", "hello")]);
    assert!(matches!(d.component("string"), Some(ModalSubmitComponent::StringSelect(_))));
    assert_eq!(d.component("string").unwrap().values(), ["a", "b"]);
    assert!(matches!(d.component("user"), Some(ModalSubmitComponent::UserSelect(_))));
    assert!(matches!(d.component("role"), Some(ModalSubmitComponent::RoleSelect(_))));
    assert!(matches!(d.component("mention"), Some(ModalSubmitComponent::MentionableSelect(_))));
    assert!(matches!(d.component("chan"), Some(ModalSubmitComponent::ChannelSelect(_))));
    assert!(matches!(d.component("file"), Some(ModalSubmitComponent::FileUpload(_))));
    assert_eq!(d.component("group").unwrap().values(), ["p", "q"]);
    match d.component("radio") {
        Some(ModalSubmitComponent::RadioGroup(r)) => assert_eq!(r.value, None),
        other => panic!("{other:?}"),
    }
    match d.component("radio2") {
        Some(ModalSubmitComponent::RadioGroup(r)) => assert_eq!(r.value.as_deref(), Some("x")),
        other => panic!("{other:?}"),
    }
    match d.component("tick") {
        Some(ModalSubmitComponent::Checkbox(c)) => assert!(c.value),
        other => panic!("{other:?}"),
    }
    assert!(d.component("radio").unwrap().values().is_empty());
    assert_eq!(d.text_input_value("string"), None, "a select is not a text input");
    let resolved = d.resolved.as_ref().unwrap();
    assert_eq!(resolved.users.len(), 1);
    assert_eq!(resolved.roles.len(), 1);
    assert_eq!(resolved.channels.len(), 1);
    assert_eq!(resolved.attachments.len(), 1);
    assert_stable(&i);
}

#[test]
fn radio_group_writes_null_when_nothing_was_chosen() {
    // `value: string | null`: always written.
    let d =
        modal(r#"{"custom_id":"m","components":[{"type":18,"component":{"type":21,"custom_id":"r","value":null}}]}"#);
    let back = serde_json::to_value(&d).unwrap();
    assert_eq!(back["components"][0]["component"], json!({"type": 21, "custom_id": "r", "value": null}));
}

#[test]
fn an_unknown_modal_component_round_trips_byte_exact() {
    let raw = r#"{"custom_id":"x",  "type":99, "payload":[1,  2]}"#;
    let d = modal(&format!(
        r#"{{"custom_id":"m","components":[{raw},{{"type":18,"component":{raw}}},{{"type":1,"components":[{raw}]}}]}}"#
    ));
    assert!(matches!(&d.components[0], ModalSubmitComponent::Unknown { kind: 99, .. }));
    let hoisted = d.hoisted();
    assert_eq!(hoisted.len(), 2, "unknown at the top is not hoisted, as a leaf it is");
    assert!(hoisted[0].custom_id().is_none());
    assert!(hoisted[0].values().is_empty());
    assert_eq!(serde_json::to_string(&d.components[0]).unwrap(), raw);
    assert!(serde_json::to_string(&d).unwrap().contains(raw));
}

#[test]
fn hoisting_goes_one_level_down_and_the_last_duplicate_wins() {
    // `ModalComponentResolver`: `next.components` and `next.component`, set into a map by `customId`.
    let d = modal(
        r#"{"custom_id":"m","components":[
            {"type":1,"components":[{"type":4,"custom_id":"a","value":"first"},{"type":4,"custom_id":"a","value":"second"}]},
            {"type":18,"component":{"type":18,"component":{"type":4,"custom_id":"deep","value":"x"}}},
            {"type":4,"custom_id":"top","value":"x"}
        ]}"#,
    );
    assert_eq!(d.text_input_value("a"), Some("second"));
    assert_eq!(d.text_input_value("deep"), None);
    assert_eq!(d.text_input_value("top"), None);
}

#[test]
fn modal_opened_from_a_component_carries_the_message() {
    let i: Interaction = decode(&envelope(
        5,
        &format!(
            r#","data":{{"custom_id":"m","components":null}},"message":{{"id":"900","channel_id":"1","author":{USER}}}{}"#,
            in_guild()
        ),
    ))
    .unwrap();
    assert!(i.modal_data().unwrap().components.is_empty());
    assert_eq!(i.message.unwrap().id.get(), 900);
}

// ---- responses, `responses.ts` ------------------------------------------------------------------

fn body(b: impl IntoBody) -> Value {
    serde_json::from_slice(&b.into_body().unwrap().json.unwrap()).unwrap()
}

#[test]
fn callback_kinds_match_upstream() {
    for (kind, wire) in [
        (InteractionResponseType::PONG, 1),
        (InteractionResponseType::CHANNEL_MESSAGE_WITH_SOURCE, 4),
        (InteractionResponseType::DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE, 5),
        (InteractionResponseType::DEFERRED_MESSAGE_UPDATE, 6),
        (InteractionResponseType::UPDATE_MESSAGE, 7),
        (InteractionResponseType::APPLICATION_COMMAND_AUTOCOMPLETE_RESULT, 8),
        (InteractionResponseType::MODAL, 9),
    ] {
        assert_eq!(kind.get(), wire);
    }
}

#[test]
fn pong_is_a_bare_type() {
    assert_eq!(body(InteractionResponse::pong()), json!({"type": 1}));
}

#[test]
fn deferred_update_is_a_bare_type() {
    assert_eq!(body(InteractionResponse::deferred_update()), json!({"type": 6}));
}

#[test]
fn message_and_update_wrap_their_data() {
    let data = json!({"content": "hi", "flags": 64});
    assert_eq!(body(InteractionResponse::message(data.clone())), json!({"type": 4, "data": data}));
    assert_eq!(body(InteractionResponse::update(data.clone())), json!({"type": 7, "data": data}));
}

#[test]
fn deferred_message_holds_only_flags() {
    assert_eq!(body(InteractionResponse::deferred(None)), json!({"type": 5}));
    assert_eq!(
        body(InteractionResponse::deferred(Some(MessageFlags::EPHEMERAL))),
        json!({"type": 5, "data": {"flags": 64}})
    );
    assert_eq!(serde_json::to_value(DeferredResponseData::default()).unwrap(), json!({}));
}

#[test]
fn autocomplete_response_lists_choices() {
    let choices = vec![json!({"name": "One", "value": 1}), json!({"name": "Two", "value": "2"})];
    assert_eq!(
        body(InteractionResponse::autocomplete(choices)),
        json!({"type": 8, "data": {"choices": [{"name": "One", "value": 1}, {"name": "Two", "value": "2"}]}})
    );
    assert_eq!(
        body(InteractionResponse::autocomplete(Vec::<Value>::new())),
        json!({"type": 8, "data": {"choices": []}}),
        "an empty list is how to show no suggestions"
    );
}

#[test]
fn modal_response_has_its_three_fields() {
    let text = r#"{"custom_id":"m","title":"T","components":[{"type":1,"components":[{"type":4,"custom_id":"a","style":1,"label":"L"}]}]}"#;
    let data: ModalResponseData = decode(text).unwrap();
    assert_eq!(data.components.len(), 1);
    assert_eq!(
        body(InteractionResponse::modal(data)),
        json!({"type": 9, "data": {
            "custom_id": "m", "title": "T",
            "components": [{"type": 1, "components": [{"type": 4, "custom_id": "a", "style": 1, "label": "L"}]}]
        }})
    );
}

#[test]
fn new_builds_any_kind_including_ones_this_build_does_not_name() {
    assert_eq!(
        body(InteractionResponse::<()>::new(InteractionResponseType(12), None)),
        json!({"type": 12}),
        "`LaunchActivity`"
    );
    assert_eq!(
        body(InteractionResponse::new(InteractionResponseType::UPDATE_MESSAGE, Some(json!({"content": "x"})))),
        json!({"type": 7, "data": {"content": "x"}})
    );
}

#[test]
fn a_callback_can_be_read_back_for_the_mock() {
    let r: InteractionResponse<Value> = decode(r#"{"type":4,"data":{"content":"hi"}}"#).unwrap();
    assert_eq!(r.kind, InteractionResponseType::CHANNEL_MESSAGE_WITH_SOURCE);
    assert_eq!(r.data.unwrap()["content"], json!("hi"));
    let bare: InteractionResponse<Value> = decode(r#"{"type":1}"#).unwrap();
    assert!(bare.data.is_none());
    let d: InteractionResponse<AutocompleteResponseData<Value>> =
        decode(r#"{"type":8,"data":{"choices":[]}}"#).unwrap();
    assert!(d.data.unwrap().choices.is_empty());
}
