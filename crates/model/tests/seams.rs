//! Whole gateway payloads through the real types together, through `Dispatch::decode`, the path a
//! shard uses.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#![recursion_limit = "512"]

use dr_core::Id;
use dr_model::{
    ApplicationCommand, ApplicationCommandOption, ApplicationIntegrationType, ButtonStyle, ChannelFlags, Component,
    Dispatch, DispatchKind, Emoji, GuildMemberFlags, GuildVerificationLevel, Interaction, InteractionContextType,
    InteractionData, Message, MessageComponentEmoji, MessageFlags, OverwriteType, find_component_by_custom_id,
};
use serde_json::{Value, json};

fn dispatch(t: &str, body: &Value) -> Dispatch {
    Dispatch::decode(DispatchKind::from_t(t), t, &body.to_string()).unwrap()
}

/// Runs `f` on a thread as small as a tokio worker's, so that an overflow aborts the test binary
/// instead of passing on the larger stack the main test thread may have.
fn on_worker_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(f).unwrap().join().unwrap();
}

fn user() -> Value {
    json!({"id":"100","username":"alice","discriminator":"0","global_name":"Alice","avatar":null,"public_flags":64})
}

fn member() -> Value {
    json!({"roles":["700"],"joined_at":"2021-01-01T00:00:00+00:00","deaf":false,"mute":false,"flags":3,"nick":"al"})
}

fn button_row() -> Value {
    json!({"type":1,"components":[
        {"type":2,"style":1,"label":"Yes","custom_id":"vote:yes","emoji":{"id":null,"name":"\u{1f525}"}},
        {"type":2,"style":5,"label":"Docs","url":"https://example.com","emoji":{"id":"80351110224678912","name":"ok","animated":true}},
        {"type":3,"custom_id":"pick","options":[{"label":"A","value":"a","emoji":{"name":"\u{1f525}"}}]},
        {"type":99,"custom_id":"future","extra":[1,2,3]}
    ]})
}

/// A `MESSAGE_CREATE` with a field from each of the structs the packages wrote separately.
fn full_message() -> Value {
    json!({
        "id":"1000","channel_id":"500","guild_id":"200","channel_type":0,
        "author":user(),"member":member(),"content":"hello","type":19,"tts":false,
        "timestamp":"2021-01-01T00:00:00+00:00","edited_timestamp":null,"pinned":false,
        "mention_everyone":false,"mentions":[{"id":"101","username":"bob","discriminator":"0","global_name":null,"avatar":null,"member":member()}],
        "mention_roles":["700"],"mention_channels":[{"id":"500","guild_id":"200","type":0,"name":"general"}],
        "attachments":[{"id":"900","filename":"a.png","size":3,"url":"https://x/a","proxy_url":"https://y/a","flags":8}],
        "embeds":[{"title":"t","fields":[{"name":"n","value":"v"}],"footer":{"text":"f"},"author":{"name":"a"}}],
        "reactions":[{"count":1,"count_details":{"burst":0,"normal":1},"me":false,"me_burst":false,"burst_colors":[],"emoji":{"id":null,"name":"\u{1f525}"}}],
        "nonce":"abc","flags":32768,"webhook_id":"600","application_id":"300",
        "activity":{"type":1,"party_id":"p"},
        "message_reference":{"type":0,"message_id":"999","channel_id":"500","guild_id":"200"},
        "referenced_message":{"id":"999","channel_id":"500","author":user(),"content":"before","components":[button_row()]},
        "interaction_metadata":{"id":"1100","type":2,"user":user(),"authorizing_integration_owners":{"0":"200"}},
        "interaction":{"id":"1100","type":2,"name":"ping","user":user(),"member":member()},
        "components":[button_row()],"position":4,
        "message_snapshots":[{"message":{"content":"fw","type":0,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"components":[button_row()]}}],
        "call":{"participants":["100"],"ended_timestamp":null},
        "shared_client_theme":{"colors":["ff0000"],"gradient_angle":90,"base_mix":50,"base_theme":1},
        "resolved":{"users":{"100":user()},"roles":{"700":{"id":"700","permissions":"8"}}}
    })
}

#[test]
fn a_full_message_create_decodes_through_every_package() {
    let Dispatch::MessageCreate(m) = dispatch("MESSAGE_CREATE", &full_message()) else { panic!() };

    assert_eq!(m.author.display_name(), "Alice");
    assert_eq!(
        m.member.as_ref().unwrap().flags,
        Some(GuildMemberFlags::DID_REJOIN | GuildMemberFlags::COMPLETED_ONBOARDING)
    );
    assert!(m.flags.unwrap().contains(MessageFlags::IS_COMPONENTS_V2));
    assert!(m.attachments[0].is_spoiler());
    assert_eq!(m.embeds[0].length(), 5);
    assert_eq!(m.reactions[0].emoji.identifier().as_deref(), Some("%F0%9F%94%A5"));
    assert!(m.reactions[0].emoji.available, "a reaction emoji is usable");
    assert_eq!(m.position, Some(4));

    let Component::ActionRow(row) = &m.components[0] else { panic!() };
    let Component::Button(yes) = &row.components[0] else { panic!() };
    assert_eq!(yes.style, ButtonStyle::PRIMARY);
    assert_eq!(yes.emoji.as_ref().unwrap().name.as_deref(), Some("\u{1f525}"));
    assert_eq!(row.components[3].kind(), 99);

    // M2a's lookup over a tree that M1b's Message decoded.
    assert!(find_component_by_custom_id(&m.components, "vote:yes").is_some());
    assert!(find_component_by_custom_id(&m.components, "future").is_some());

    let reply = m.referenced_message.as_option().unwrap();
    assert!(find_component_by_custom_id(&reply.components, "pick").is_some());
    let owners = &m.interaction_metadata.as_ref().unwrap().authorizing_integration_owners;
    assert_eq!(owners[&ApplicationIntegrationType::GUILD_INSTALL], Id::new(200));
    assert!(m.resolved.as_ref().unwrap().users.contains_key(&Id::new(100)));
}

#[test]
fn a_full_message_survives_a_round_trip() {
    let m: Message = dr_core::decode(&full_message().to_string()).unwrap();
    let first = serde_json::to_value(&m).unwrap();
    let again: Message = dr_core::decode(&first.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), first);

    // The unknown component stays byte-exact and the emoji stays in its small form.
    let children = &first["components"][0]["components"];
    assert_eq!(children[3], json!({"type":99,"custom_id":"future","extra":[1,2,3]}));
    assert_eq!(children[1]["emoji"], json!({"id":"80351110224678912","name":"ok","animated":true}));
    assert_eq!(first["reactions"][0]["emoji"], json!({"name":"\u{1f525}"}));
}

/// The cache patches a stored `Message` from this event. A field that `Message` keeps and the event
/// does not read would go stale there, as `call.ended_timestamp` does when a call ends.
#[test]
fn message_update_carries_every_field_message_has() {
    let body = full_message();
    let Dispatch::MessageUpdate(update) = dispatch("MESSAGE_UPDATE", &body) else { panic!() };
    let written = serde_json::to_value(&update).unwrap();

    let message: Message = dr_core::decode(&body.to_string()).unwrap();
    let Value::Object(message) = serde_json::to_value(&message).unwrap() else { panic!() };
    for (key, value) in &message {
        assert_eq!(written.get(key), Some(value), "MESSAGE_UPDATE loses `{key}`");
    }
}

/// Same for a stored `Member`. `permissions` exists only on interaction payloads.
#[test]
fn member_update_carries_every_field_member_has() {
    let body = json!({
        "guild_id":"200","user":user(),"nick":"al","avatar":"aa","banner":"bb","roles":["700"],
        "joined_at":"2021-01-01T00:00:00+00:00","premium_since":"2021-01-01T00:00:00+00:00",
        "deaf":true,"mute":true,"flags":1099511627778_u64,"pending":true,
        "communication_disabled_until":"2021-01-02T00:00:00+00:00",
        "avatar_decoration_data":{"asset":"a","sku_id":"1"},
        "collectibles":{"nameplate":{"sku_id":"2","asset":"b","label":"c","palette":"berry"}}
    });
    let Dispatch::GuildMemberUpdate(update) = dispatch("GUILD_MEMBER_UPDATE", &body) else { panic!() };
    assert!(update.flags.unwrap().contains(GuildMemberFlags::COMPLETED_ONBOARDING));
    assert_eq!(update.flags.unwrap().bits(), (1 << 40) | 2, "unknown bits are kept");

    let written = serde_json::to_value(&update).unwrap();
    let member: dr_model::Member = dr_core::decode(&body.to_string()).unwrap();
    let Value::Object(member) = serde_json::to_value(&member).unwrap() else { panic!() };
    for (key, value) in &member {
        assert_eq!(written.get(key), Some(value), "GUILD_MEMBER_UPDATE loses `{key}`");
    }
}

#[test]
fn guild_create_decodes_channels_members_and_roles_of_other_packages() {
    let body = json!({
        "id":"200","name":"g","owner_id":"100","verification_level":2,"system_channel_flags":1,
        "premium_tier":3,"features":["COMMUNITY","SOMETHING_NEW"],"preferred_locale":"fr",
        "roles":[{"id":"200","name":"@everyone","permissions":"8","colors":{"primary_color":1,"secondary_color":null,"tertiary_color":null},"tags":{"premium_subscriber":null}}],
        "emojis":[{"id":"80351110224678912","name":"ok","roles":["700"],"user":user(),"available":false}],
        "members":[{"user":user(),"roles":["200"],"joined_at":"2021-01-01T00:00:00+00:00","flags":2}],
        "channels":[{"id":"500","type":0,"name":"general","flags":2,
            "permission_overwrites":[{"id":"200","type":777,"allow":"8","deny":"0"},{"id":"100","type":1,"allow":"0","deny":"8"}]},
            {"id":"501","type":4242}],
        "stickers":[],"threads":[],"presences":[],"voice_states":[],"member_count":1,"large":false
    });
    let Dispatch::GuildCreate(g) = dispatch("GUILD_CREATE", &body) else { panic!() };
    assert_eq!(g.verification_level, Some(GuildVerificationLevel::MEDIUM));
    assert!(g.roles[0].is_everyone(g.id));
    assert!(g.roles[0].tags.as_ref().unwrap().premium_subscriber);
    assert!(!g.emojis[0].available);
    assert_eq!(g.members[0].display_name(), Some("Alice"));
    assert_eq!(g.members[0].flags, Some(GuildMemberFlags::COMPLETED_ONBOARDING));
    assert!(g.channels[0].flags.unwrap().contains(ChannelFlags::PINNED));
    assert_eq!(g.channels[0].permission_overwrites[0].kind, OverwriteType(777));
    assert_eq!(g.channels[0].permission_overwrites[1].kind, OverwriteType::MEMBER);
    assert_eq!(g.channels[1].kind.get(), 4242);
}

fn interaction(kind: u32, data: &Value, extra: &Value) -> Value {
    let mut body = json!({
        "id":"1100","application_id":"300","type":kind,"token":"secret-interaction-token","version":1,
        "guild_id":"200","channel_id":"499","channel":{"id":"500","type":0,"name":"general"},
        "member":{"user":user(),"roles":["700"],"joined_at":"2021-01-01T00:00:00+00:00","permissions":"8","deaf":false,"mute":false,"flags":0},
        "app_permissions":"442368","locale":"fr","guild_locale":"en-US","entitlements":[],
        "authorizing_integration_owners":{"0":"200"},"context":0,"attachment_size_limit":10485760,
        "data":data
    });
    if let Value::Object(extra) = extra {
        body.as_object_mut().unwrap().extend(extra.clone());
    }
    body
}

fn decode_interaction(body: &Value) -> Box<Interaction> {
    let Dispatch::InteractionCreate(i) = dispatch("INTERACTION_CREATE", body) else { panic!() };
    i
}

#[test]
fn a_slash_command_resolves_objects_of_every_package() {
    let data = json!({
        "id":"800","name":"whoami","type":1,
        "options":[{"name":"sub","type":1,"options":[
            {"name":"who","type":6,"value":"100"},{"name":"role","type":8,"value":"700"},
            {"name":"where","type":7,"value":"500"},{"name":"file","type":11,"value":"900"},
            {"name":"text","type":3,"value":"hi"}]}],
        "resolved":{
            "users":{"100":user()},
            "members":{"100":{"roles":["700"],"joined_at":"2021-01-01T00:00:00+00:00","permissions":"8"}},
            "roles":{"700":{"id":"700","name":"r","permissions":"8"}},
            "channels":{"500":{"id":"500","type":0,"name":"general","permissions":"8"}},
            "attachments":{"900":{"id":"900","filename":"a.png","size":3,"url":"u","proxy_url":"p","flags":8}},
            "messages":{"1000":full_message()}
        }
    });
    let i = decode_interaction(&interaction(2, &data, &json!({})));

    assert_eq!(i.invoker().map(dr_model::User::display_name), Some("Alice"));
    assert_eq!(i.channel_id, Some(Id::new(500)), "channel.id wins over the deprecated channel_id");
    assert_eq!(i.context, Some(InteractionContextType::GUILD));
    assert_eq!(i.authorizing_integration_owners[&ApplicationIntegrationType::GUILD_INSTALL], Id::new(200));
    assert!(i.is_chat_input_command());

    let command = i.command_data().unwrap();
    let options = command.hoisted();
    assert_eq!(options.subcommand, Some("sub"));
    assert_eq!(options.user("who"), Some(Id::new(100)));
    assert_eq!(options.string("text"), Some("hi"));
    let resolved = command.resolved.as_ref().unwrap();
    assert_eq!(resolved.user_of(options.get("who").unwrap()).unwrap().username, "alice");
    assert_eq!(resolved.member_with_user(Id::new(100)).unwrap().display_name(), Some("Alice"));
    assert_eq!(resolved.role_of(options.get("role").unwrap()).unwrap().name, "r");
    assert_eq!(resolved.channel_of(options.get("where").unwrap()).unwrap().name.as_deref(), Some("general"));
    assert!(resolved.attachment_of(options.get("file").unwrap()).unwrap().is_spoiler());
    assert_eq!(resolved.messages[&Id::new(1000)].position, Some(4));
}

#[test]
fn a_button_click_carries_the_message_it_is_on() {
    let data = json!({"custom_id":"vote:yes","component_type":2});
    let extra = json!({"message":full_message()});
    let i = decode_interaction(&interaction(3, &data, &extra));

    assert!(i.is_button());
    assert_eq!(i.component_data().unwrap().custom_id, "vote:yes");
    let on = i.message.as_ref().unwrap();
    assert!(find_component_by_custom_id(&on.components, &i.component_data().unwrap().custom_id).is_some());

    // The token reaches the wire form, which the mock uses, and never `Debug`.
    let written = serde_json::to_value(&i).unwrap();
    assert_eq!(written["token"], "secret-interaction-token");
    assert!(!format!("{i:?}").contains("secret-interaction-token"));
}

#[test]
fn a_modal_submission_reads_rows_and_labels() {
    let data = json!({"custom_id":"m","components":[
        {"type":1,"components":[{"type":4,"custom_id":"name","value":"typed"}]},
        {"type":18,"id":3,"component":{"type":3,"custom_id":"pick","values":["a"]}},
        {"type":10,"id":4},
        {"type":99,"id":5}
    ]});
    let i = decode_interaction(&interaction(5, &data, &json!({})));
    let modal = i.modal_data().unwrap();
    assert_eq!(modal.text_input_value("name"), Some("typed"));
    assert_eq!(modal.component("pick").unwrap().values(), ["a".to_owned()]);
}

#[test]
fn an_emoji_parsed_from_text_goes_on_a_button() {
    let custom: MessageComponentEmoji = Emoji::parse("<a:ok:80351110224678912>").unwrap().into();
    assert_eq!(serde_json::to_value(&custom).unwrap(), json!({"id":"80351110224678912","name":"ok","animated":true}));
    let unicode: MessageComponentEmoji = Emoji::parse("\u{1f525}").unwrap().into();
    assert_eq!(serde_json::to_value(&unicode).unwrap(), json!({"name":"\u{1f525}"}));
}

#[test]
fn a_command_uses_the_same_integration_and_context_types_as_an_interaction() {
    let c: ApplicationCommand = dr_core::decode(
        r#"{"id":"800","application_id":"300","name":"x","description":"d","version":"1","type":1,
           "integration_types":[0,1],"contexts":[0,2]}"#,
    )
    .unwrap();
    let types: Option<Vec<ApplicationIntegrationType>> = c.integration_types.clone();
    let contexts: Option<Vec<InteractionContextType>> = c.contexts.clone();
    assert_eq!(types.unwrap()[1], ApplicationIntegrationType::USER_INSTALL);
    assert_eq!(contexts.unwrap()[1], InteractionContextType::PRIVATE_CHANNEL);
}

/// Wraps `leaf` in `depth` levels of `{ open ... close }`.
fn nest(open: &str, leaf: &str, close: &str, depth: usize) -> String {
    let mut out = String::with_capacity((open.len() + close.len()) * depth + leaf.len());
    for _ in 0..depth {
        out.push_str(open);
    }
    out.push_str(leaf);
    for _ in 0..depth {
        out.push_str(close);
    }
    out
}

const DEPTHS: [usize; 5] = [1, 16, 200, 5_000, 100_000];

/// Each `tagged_union!` arm starts a new parser, which resets serde_json's recursion limit, so a
/// union that can hold itself needs its own limit, or wire data aborts the process.
#[test]
fn deep_modal_rows_do_not_overflow_the_stack() {
    on_worker_stack(|| {
        for depth in DEPTHS {
            let data = nest(r#"{"type":1,"components":["#, r#"{"type":4,"custom_id":"a","value":"b"}"#, "]}", depth);
            let body = format!(
                r#"{{"id":"1","application_id":"2","type":5,"token":"t","data":{{"custom_id":"m","components":[{data}]}}}}"#
            );
            let d = Dispatch::decode(DispatchKind::InteractionCreate, "INTERACTION_CREATE", &body).unwrap();
            let Dispatch::InteractionCreate(i) = d else { panic!() };
            assert_eq!(i.token.expose(), "t", "the token stays usable");
            if depth == 1 {
                assert!(i.modal_data().is_some());
            } else {
                assert!(matches!(i.data, Some(InteractionData::Malformed { .. })), "depth {depth}");
            }
        }
    });
}

#[test]
fn deep_modal_labels_do_not_overflow_the_stack() {
    on_worker_stack(|| {
        for depth in DEPTHS {
            let data = nest(r#"{"type":18,"component":"#, r#"{"type":4,"custom_id":"a","value":"b"}"#, "}", depth);
            let body = format!(
                r#"{{"id":"1","application_id":"2","type":5,"token":"t","data":{{"custom_id":"m","components":[{data}]}}}}"#
            );
            let d = Dispatch::decode(DispatchKind::InteractionCreate, "INTERACTION_CREATE", &body).unwrap();
            let Dispatch::InteractionCreate(i) = d else { panic!() };
            assert_eq!(i.token.expose(), "t", "the token stays usable");
            if depth == 1 {
                assert!(i.modal_data().is_some());
            } else {
                assert!(matches!(i.data, Some(InteractionData::Malformed { .. })), "depth {depth}");
            }
        }
    });
}

#[test]
fn deep_options_do_not_overflow_the_stack() {
    on_worker_stack(|| {
        for depth in DEPTHS {
            let options =
                nest(r#"{"name":"a","type":1,"options":["#, r#"{"name":"b","type":3,"value":"c"}"#, "]}", depth);
            let body = format!(
                r#"{{"id":"1","application_id":"2","type":2,"token":"t","data":{{"id":"3","type":1,"options":[{options}]}}}}"#
            );
            drop(Dispatch::decode(DispatchKind::InteractionCreate, "INTERACTION_CREATE", &body));
        }
    });
}

#[test]
fn deep_command_options_do_not_overflow_the_stack() {
    on_worker_stack(|| {
        for depth in DEPTHS {
            let options = nest(r#"{"name":"a","description":"d","type":1,"options":["#, "", "]}", depth);
            let body = format!(r#"{{"id":"1","application_id":"2","name":"x","options":[{options}]}}"#);
            drop(dr_core::decode::<ApplicationCommand>(&body));
            drop(dr_core::decode::<ApplicationCommandOption>(&options));
        }
    });
}

/// Every recursive route through a message at once, each at its own limit.
#[test]
fn messages_nested_in_replies_rows_and_resolved_data_do_not_overflow_the_stack() {
    on_worker_stack(|| {
        for depth in DEPTHS {
            let rows =
                nest(r#"{"type":1,"components":["#, r#"{"type":2,"style":1,"custom_id":"a"}"#, "]}", depth.min(16));
            let base = format!(r#"{{"id":"1","channel_id":"2","author":{{"id":"3"}},"components":[{rows}]}}"#);

            let replies =
                nest(r#"{"id":"1","channel_id":"2","author":{"id":"3"},"referenced_message":"#, &base, "}", depth);
            drop(Dispatch::decode(DispatchKind::MessageCreate, "MESSAGE_CREATE", &replies));
            drop(Dispatch::decode(DispatchKind::MessageUpdate, "MESSAGE_UPDATE", &replies));

            let resolved = nest(
                r#"{"id":"1","channel_id":"2","author":{"id":"3"},"resolved":{"messages":{"1":"#,
                &base,
                "}}}",
                depth,
            );
            drop(Dispatch::decode(DispatchKind::MessageCreate, "MESSAGE_CREATE", &resolved));

            let on_click = format!(
                r#"{{"id":"1","application_id":"2","type":3,"token":"t","message":{replies},"data":{{"custom_id":"a","component_type":2,"resolved":{{"messages":{{"1":{resolved}}}}}}}}}"#
            );
            drop(Dispatch::decode(DispatchKind::InteractionCreate, "INTERACTION_CREATE", &on_click));
        }
    });
}
