//! Envelope, dispatch table, event bodies and send payloads, checked against discord-api-types
//! `gateway/v10.ts` and the wire rules of `ARCHITECTURE.md` 7.4, 8.2 and 10.1.
//!
//! The fixtures are complete, realistic payloads, so that they keep decoding when the payload structs
//! gain fields.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::{Patch, Secret, Timestamp};
use dr_model::{
    Activity, Dispatch, DispatchKind, Envelope, GatewayFrame, Heartbeat, Hello, Identify, IdentifyProperties, Intents,
    MessageFlags, PresenceStatus, Resume, UpdatePresence,
};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::HashSet;

const USER: &str = r#"{"id":"100","username":"alice","discriminator":"0","global_name":"Alice","avatar":null}"#;

fn ready() -> String {
    r#"{"v":10,"user":{"id":"100","username":"bot","discriminator":"0","global_name":null,"avatar":null,"bot":true},
    "guilds":[{"id":"200","unavailable":true},{"id":"201","unavailable":true}],
    "session_id":"sess-abc","resume_gateway_url":"wss://resume.example/","shard":[0,1],
    "application":{"id":"300","flags":8667136,"flags_new":"8667136"},"session_type":"normal","_trace":["x"]}"#
        .to_owned()
}

fn member(guild_id: Option<&str>) -> String {
    let guild = guild_id.map_or(String::new(), |g| format!(r#""guild_id":"{g}","#));
    format!(
        r#"{{{guild}"user":{USER},"nick":"al","avatar":null,"banner":null,"roles":["400","401"],
        "joined_at":"2021-01-01T00:00:00.000000+00:00","premium_since":null,"deaf":false,"mute":false,"flags":0,
        "pending":false,"communication_disabled_until":null}}"#
    )
}

fn channel(kind: u32) -> String {
    format!(
        r#"{{"id":"500","type":{kind},"guild_id":"200","name":"general","position":3,"topic":null,"nsfw":false,
        "parent_id":null,"last_message_id":null,"rate_limit_per_user":0,"permission_overwrites":[],"flags":0}}"#
    )
}

fn role() -> &'static str {
    r#"{"id":"200","name":"@everyone","color":0,"colors":{"primary_color":0,"secondary_color":null,"tertiary_color":null},
    "hoist":false,"icon":null,"unicode_emoji":null,"position":0,"permissions":"1071698660929","managed":false,
    "mentionable":false,"flags":0}"#
}

fn guild_base() -> String {
    format!(
        r#""id":"200","name":"Test","icon":null,"splash":null,"discovery_splash":null,"owner_id":"100",
        "afk_channel_id":null,"afk_timeout":300,"verification_level":0,"default_message_notifications":0,
        "explicit_content_filter":0,"roles":[{}],"emojis":[],"features":["COMMUNITY","SOME_NEW_FEATURE"],"mfa_level":0,
        "application_id":null,"system_channel_id":"500","system_channel_flags":0,"rules_channel_id":null,
        "vanity_url_code":null,"description":null,"banner":null,"premium_tier":0,"preferred_locale":"en-US",
        "public_updates_channel_id":null,"nsfw_level":0,"premium_progress_bar_enabled":false,
        "safety_alerts_channel_id":null,"stickers":[]"#,
        role()
    )
}

fn guild_create() -> String {
    format!(
        r#"{{{},"joined_at":"2021-01-01T00:00:00+00:00","large":false,"member_count":2,"voice_states":[],
        "members":[{}],"channels":[{}],"threads":[],"presences":[],"stage_instances":[],
        "guild_scheduled_events":[],"soundboard_sounds":[]}}"#,
        guild_base(),
        member(None),
        channel(0)
    )
}

fn message() -> String {
    format!(
        r#"{{"id":"600","channel_id":"500","guild_id":"200","author":{USER},
        "member":{{"nick":null,"avatar":null,"roles":["400"],"joined_at":"2021-01-01T00:00:00+00:00","premium_since":null,
        "deaf":false,"mute":false,"flags":0,"pending":false}},
        "content":"!ping","timestamp":"2021-01-01T00:00:00.123456+00:00","edited_timestamp":null,"tts":false,
        "mention_everyone":false,"mentions":[{{"id":"101","username":"bob","discriminator":"0","global_name":null,
        "avatar":null,"member":{{"roles":[],"joined_at":"2021-01-01T00:00:00+00:00","deaf":false,"mute":false,"flags":0}}}}],
        "mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0,"flags":0,"channel_type":0,
        "components":[]}}"#
    )
}

fn interaction() -> &'static str {
    r#"{"id":"700","application_id":"300","type":2,"token":"interaction-token","version":1,
    "data":{"id":"800","name":"ping","type":1},"guild_id":"200","channel_id":"500",
    "member":{"user":{"id":"100","username":"alice","discriminator":"0","global_name":null,"avatar":null},"roles":[],
    "joined_at":"2021-01-01T00:00:00+00:00","deaf":false,"mute":false,"flags":0,"permissions":"1071698660929"},
    "app_permissions":"1071698660929","locale":"en-US","guild_locale":"en-US","entitlements":[],"authorizing_integration_owners":{}}"#
}

fn dispatch(t: &str, d: &str) -> Dispatch {
    Dispatch::decode(DispatchKind::from_t(t), t, d).unwrap_or_else(|e| panic!("{t}: {e}"))
}

fn identify() -> Identify {
    Identify {
        token: Secret::new("tok"),
        properties: IdentifyProperties {
            os: "linux".into(),
            browser: "discord.rs".into(),
            device: "discord.rs".into(),
        },
        compress: false,
        large_threshold: None,
        shard: [0, 1],
        presence: None,
        intents: Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
        capabilities: 0,
    }
}

fn presence() -> UpdatePresence {
    UpdatePresence { since: None, activities: Vec::new(), status: PresenceStatus::Online, afk: false }
}

/// The six characters of the JSON escape for `A`.
fn json_escape_of_capital_a() -> String {
    ['\\', 'u', '0', '0', '4', '1'].into_iter().collect()
}

// ---- envelope -----------------------------------------------------------------------------------

#[test]
fn envelope_reads_a_dispatch_without_touching_the_body() {
    // The body is not valid for any type: the envelope does not care.
    let frame = r#"{"op":0,"s":42,"t":"MESSAGE_CREATE","d":{"id":12,"nested":[1,{"x":null}]}}"#;
    let env: Envelope<'_> = serde_json::from_str(frame).unwrap();
    assert_eq!(env.op, 0);
    assert_eq!(env.s, Some(42));
    assert_eq!(env.t.as_deref(), Some("MESSAGE_CREATE"));
    assert_eq!(env.d.unwrap().get(), r#"{"id":12,"nested":[1,{"x":null}]}"#);
}

#[test]
fn envelope_borrows_t_and_d_from_the_frame() {
    let frame = String::from(r#"{"op":0,"s":1,"t":"READY","d":{}}"#);
    let env: Envelope<'_> = serde_json::from_str(&frame).unwrap();
    assert!(matches!(env.t, Some(Cow::Borrowed("READY"))));
    let range = frame.as_bytes().as_ptr_range();
    assert!(range.contains(&env.d.unwrap().get().as_ptr()));
}

#[test]
fn envelope_accepts_an_escaped_event_name() {
    // A borrowed `&str` would fail here, `ARCHITECTURE.md` 7.4 rule 6.
    let frame = format!(r#"{{"op":0,"s":1,"t":"RE{}DY","d":null}}"#, json_escape_of_capital_a());
    let env: Envelope<'_> = serde_json::from_str(&frame).unwrap();
    assert!(matches!(&env.t, Some(Cow::Owned(t)) if t == "READY"), "{:?}", env.t);
}

#[test]
fn envelope_of_a_non_dispatch_has_null_t_and_s() {
    // `_NonDispatchPayload`: t and s are null.
    let env: Envelope<'_> = serde_json::from_str(r#"{"op":11,"s":null,"t":null,"d":null}"#).unwrap();
    assert_eq!(env.op, 11);
    assert_eq!(env.s, None);
    assert!(env.t.is_none());
    assert!(env.d.is_none());

    let hello = r#"{"t":null,"s":null,"op":10,"d":{"heartbeat_interval":41250,"_trace":["a"]}}"#;
    let env: Envelope<'_> = serde_json::from_str(hello).unwrap();
    let body: Hello = serde_json::from_str(env.d.unwrap().get()).unwrap();
    assert_eq!(body.heartbeat_interval, 41250);
}

#[test]
fn envelope_tolerates_absent_fields_any_order_and_unknown_keys() {
    let env: Envelope<'_> = serde_json::from_str(r#"{"op":1}"#).unwrap();
    assert_eq!((env.op, env.s), (1, None));
    assert!(env.t.is_none() && env.d.is_none());

    let env: Envelope<'_> = serde_json::from_str(r#"{"d":[1],"future":{"a":1},"t":"X","s":7,"op":0}"#).unwrap();
    assert_eq!((env.op, env.s, env.t.as_deref()), (0, Some(7), Some("X")));
}

#[test]
fn envelope_keeps_an_unknown_opcode() {
    // Tolerance level 1. A `u8` opcode would reject 999 and lose the frame.
    let env: Envelope<'_> = serde_json::from_str(r#"{"op":999,"d":{"a":1}}"#).unwrap();
    assert_eq!(env.op, 999);
    let env: Envelope<'_> = serde_json::from_str(r#"{"op":4294967295}"#).unwrap();
    assert_eq!(env.op, u32::MAX);
}

#[test]
fn envelope_keeps_a_sequence_beyond_32_bits() {
    let env: Envelope<'_> = serde_json::from_str(r#"{"op":0,"s":5000000000,"t":"X","d":0}"#).unwrap();
    assert_eq!(env.s, Some(5_000_000_000));
}

#[test]
fn envelope_rejects_what_is_not_a_frame() {
    for bad in ["", "not json", "[]", r#"{"d":{}}"#, r#"{"op":"0"}"#, r#"{"op":-1}"#, r#"{"op":0,"t":5}"#] {
        assert!(serde_json::from_str::<Envelope<'_>>(bad).is_err(), "{bad}");
    }
}

#[test]
fn envelope_debug_hides_the_body() {
    let frame = r#"{"op":0,"s":1,"t":"READY","d":{"session_id":"sess-abc"}}"#;
    let env: Envelope<'_> = serde_json::from_str(frame).unwrap();
    let text = format!("{env:?}");
    assert!(!text.contains("sess-abc"), "{text}");
    assert!(text.contains("READY") && text.contains("d_len"), "{text}");
}

#[test]
fn hello_needs_its_interval() {
    assert!(serde_json::from_str::<Hello>("{}").is_err());
}

// ---- dispatch kind ------------------------------------------------------------------------------

/// `GatewayDispatchEvents` of discord-api-types `gateway/v10.ts`, `[gateway 12]`.
const UPSTREAM_EVENTS: [&str; 79] = [
    "APPLICATION_COMMAND_PERMISSIONS_UPDATE",
    "AUTO_MODERATION_ACTION_EXECUTION",
    "AUTO_MODERATION_RULE_CREATE",
    "AUTO_MODERATION_RULE_DELETE",
    "AUTO_MODERATION_RULE_UPDATE",
    "CHANNEL_CREATE",
    "CHANNEL_DELETE",
    "CHANNEL_INFO",
    "CHANNEL_PINS_UPDATE",
    "CHANNEL_UPDATE",
    "ENTITLEMENT_CREATE",
    "ENTITLEMENT_DELETE",
    "ENTITLEMENT_UPDATE",
    "GUILD_AUDIT_LOG_ENTRY_CREATE",
    "GUILD_BAN_ADD",
    "GUILD_BAN_REMOVE",
    "GUILD_CREATE",
    "GUILD_DELETE",
    "GUILD_EMOJIS_UPDATE",
    "GUILD_INTEGRATIONS_UPDATE",
    "GUILD_MEMBERS_CHUNK",
    "GUILD_MEMBER_ADD",
    "GUILD_MEMBER_REMOVE",
    "GUILD_MEMBER_UPDATE",
    "GUILD_ROLE_CREATE",
    "GUILD_ROLE_DELETE",
    "GUILD_ROLE_UPDATE",
    "GUILD_SCHEDULED_EVENT_CREATE",
    "GUILD_SCHEDULED_EVENT_DELETE",
    "GUILD_SCHEDULED_EVENT_UPDATE",
    "GUILD_SCHEDULED_EVENT_USER_ADD",
    "GUILD_SCHEDULED_EVENT_USER_REMOVE",
    "GUILD_SOUNDBOARD_SOUNDS_UPDATE",
    "GUILD_SOUNDBOARD_SOUND_CREATE",
    "GUILD_SOUNDBOARD_SOUND_DELETE",
    "GUILD_SOUNDBOARD_SOUND_UPDATE",
    "GUILD_STICKERS_UPDATE",
    "GUILD_UPDATE",
    "INTEGRATION_CREATE",
    "INTEGRATION_DELETE",
    "INTEGRATION_UPDATE",
    "INTERACTION_CREATE",
    "INVITE_CREATE",
    "INVITE_DELETE",
    "MESSAGE_CREATE",
    "MESSAGE_DELETE",
    "MESSAGE_DELETE_BULK",
    "MESSAGE_POLL_VOTE_ADD",
    "MESSAGE_POLL_VOTE_REMOVE",
    "MESSAGE_REACTION_ADD",
    "MESSAGE_REACTION_REMOVE",
    "MESSAGE_REACTION_REMOVE_ALL",
    "MESSAGE_REACTION_REMOVE_EMOJI",
    "MESSAGE_UPDATE",
    "PRESENCE_UPDATE",
    "RATE_LIMITED",
    "READY",
    "RESUMED",
    "SOUNDBOARD_SOUNDS",
    "STAGE_INSTANCE_CREATE",
    "STAGE_INSTANCE_DELETE",
    "STAGE_INSTANCE_UPDATE",
    "SUBSCRIPTION_CREATE",
    "SUBSCRIPTION_DELETE",
    "SUBSCRIPTION_UPDATE",
    "THREAD_CREATE",
    "THREAD_DELETE",
    "THREAD_LIST_SYNC",
    "THREAD_MEMBERS_UPDATE",
    "THREAD_MEMBER_UPDATE",
    "THREAD_UPDATE",
    "TYPING_START",
    "USER_UPDATE",
    "VOICE_CHANNEL_EFFECT_SEND",
    "VOICE_CHANNEL_START_TIME_UPDATE",
    "VOICE_CHANNEL_STATUS_UPDATE",
    "VOICE_SERVER_UPDATE",
    "VOICE_STATE_UPDATE",
    "WEBHOOKS_UPDATE",
];

/// The proof of concept cuts the 79 typed arms of the full port (`POC.md`). What it must still prove
/// is that none of the 79 upstream events is lost: each is typed, or arrives raw with its name.
#[test]
fn dispatch_table_has_79_events() {
    assert_eq!(UPSTREAM_EVENTS.iter().collect::<HashSet<_>>().len(), 79);

    let mut typed = 0;
    let mut raw = 0;
    for t in UPSTREAM_EVENTS {
        let kind = DispatchKind::from_t(t);
        if kind == DispatchKind::Unknown {
            raw += 1;
            match Dispatch::decode(kind, t, r#"{"a":1}"#).unwrap() {
                Dispatch::Unknown { t: name, raw } => {
                    assert_eq!(name, t);
                    assert_eq!(raw.get(), r#"{"a":1}"#);
                }
                other => panic!("{t} became {other:?}"),
            }
        } else {
            typed += 1;
            assert_eq!(kind.name(), Some(t));
        }
    }
    assert_eq!((typed, raw), (DispatchKind::COUNT - 1, 79 - (DispatchKind::COUNT - 1)));
}

#[test]
fn every_kind_is_an_upstream_event_and_round_trips_through_its_name() {
    for kind in DispatchKind::ALL {
        match kind.name() {
            Some(name) => {
                assert!(UPSTREAM_EVENTS.contains(&name), "{name}");
                assert_eq!(DispatchKind::from_t(name), kind);
            }
            None => assert_eq!(kind, DispatchKind::Unknown),
        }
    }
}

#[test]
fn kind_indexes_a_table_with_no_gap() {
    assert_eq!(DispatchKind::ALL.len(), DispatchKind::COUNT);
    for (i, kind) in DispatchKind::ALL.into_iter().enumerate() {
        assert_eq!(kind.index(), i, "{kind:?}");
    }
    assert_eq!(DispatchKind::Unknown.index(), DispatchKind::COUNT - 1);
}

#[test]
fn from_t_is_exact() {
    for t in ["", "ready", "Ready", "READY ", " READY", "MESSAGE_CREATE2", "SOMETHING_NEW"] {
        assert_eq!(DispatchKind::from_t(t), DispatchKind::Unknown, "{t:?}");
    }
}

#[test]
fn only_typing_and_presence_can_be_shed() {
    let shed: Vec<_> = DispatchKind::ALL.into_iter().filter(|k| k.is_sheddable()).collect();
    assert_eq!(shed, [DispatchKind::TypingStart, DispatchKind::PresenceUpdate]);
}

// ---- typed arms ---------------------------------------------------------------------------------

#[test]
fn every_typed_arm_decodes_and_reports_its_kind() {
    let member_add = member(Some("200"));
    let member_update = format!(r#"{{"guild_id":"200","user":{USER},"roles":[],"joined_at":null,"avatar":null}}"#);
    let member_remove = format!(r#"{{"guild_id":"200","user":{USER}}}"#);
    let table: Vec<(DispatchKind, String)> = vec![
        (DispatchKind::Ready, ready()),
        (DispatchKind::Resumed, "null".to_owned()),
        (DispatchKind::GuildCreate, guild_create()),
        (DispatchKind::GuildUpdate, format!("{{{}}}", guild_base())),
        (DispatchKind::GuildDelete, r#"{"id":"200"}"#.to_owned()),
        (DispatchKind::ChannelCreate, channel(0)),
        (DispatchKind::ChannelUpdate, channel(0)),
        (DispatchKind::ChannelDelete, channel(0)),
        (DispatchKind::GuildMemberAdd, member_add),
        (DispatchKind::GuildMemberUpdate, member_update),
        (DispatchKind::GuildMemberRemove, member_remove),
        (DispatchKind::MessageCreate, message()),
        (DispatchKind::MessageUpdate, r#"{"id":"600","channel_id":"500"}"#.to_owned()),
        (DispatchKind::MessageDelete, r#"{"id":"600","channel_id":"500"}"#.to_owned()),
        (DispatchKind::InteractionCreate, interaction().to_owned()),
        (DispatchKind::MessageDeleteBulk, r#"{"ids":["600"],"channel_id":"500"}"#.to_owned()),
        (DispatchKind::MessageReactionAdd, r#"{"user_id":"1","channel_id":"500","message_id":"600","emoji":{"id":null,"name":"x"},"burst":false,"type":0}"#.to_owned()),
        (DispatchKind::MessageReactionRemove, r#"{"user_id":"1","channel_id":"500","message_id":"600","emoji":{"id":null,"name":"x"},"burst":false,"type":0}"#.to_owned()),
        (DispatchKind::GuildBanAdd, format!(r#"{{"guild_id":"200","user":{USER}}}"#)),
        (DispatchKind::GuildBanRemove, format!(r#"{{"guild_id":"200","user":{USER}}}"#)),
        (DispatchKind::GuildRoleCreate, r#"{"guild_id":"200","role":{"id":"7","name":"r","permissions":"0"}}"#.to_owned()),
        (DispatchKind::GuildRoleUpdate, r#"{"guild_id":"200","role":{"id":"7","name":"r","permissions":"0"}}"#.to_owned()),
        (DispatchKind::GuildRoleDelete, r#"{"guild_id":"200","role_id":"7"}"#.to_owned()),
        (DispatchKind::ThreadCreate, r#"{"id":"8","type":11,"guild_id":"200","parent_id":"500","newly_created":true}"#.to_owned()),
        (DispatchKind::ThreadUpdate, r#"{"id":"8","type":11,"guild_id":"200","parent_id":"500"}"#.to_owned()),
        (DispatchKind::ThreadDelete, r#"{"id":"8","type":11,"guild_id":"200","parent_id":"500"}"#.to_owned()),
        (DispatchKind::ThreadListSync, r#"{"guild_id":"200","threads":[{"id":"8","type":11}],"members":[{"id":"8","user_id":"1","join_timestamp":"2020-01-01T00:00:00+00:00","flags":0}]}"#.to_owned()),
        (DispatchKind::ThreadMemberUpdate, r#"{"guild_id":"200","id":"8","user_id":"1","join_timestamp":"2020-01-01T00:00:00+00:00","flags":1}"#.to_owned()),
        (DispatchKind::ThreadMembersUpdate, r#"{"id":"8","guild_id":"200","member_count":2,"removed_member_ids":["1"]}"#.to_owned()),
        (DispatchKind::WebhooksUpdate, r#"{"guild_id":"200","channel_id":"500"}"#.to_owned()),
        (DispatchKind::VoiceStateUpdate, r#"{"guild_id":"200","channel_id":"500","user_id":"1","session_id":"s","deaf":false,"mute":false,"self_deaf":false,"self_mute":true,"self_video":false,"suppress":false}"#.to_owned()),
        (DispatchKind::VoiceServerUpdate, r#"{"token":"t","guild_id":"200","endpoint":"x.discord.media:443"}"#.to_owned()),
        (DispatchKind::MessagePollVoteAdd, r#"{"user_id":"1","channel_id":"500","message_id":"9","guild_id":"200","answer_id":1}"#.to_owned()),
        (DispatchKind::MessagePollVoteRemove, r#"{"user_id":"1","channel_id":"500","message_id":"9","answer_id":2}"#.to_owned()),
        (DispatchKind::GuildEmojisUpdate, r#"{"guild_id":"200","emojis":[{"id":"5","name":"blob"}]}"#.to_owned()),
        (DispatchKind::GuildStickersUpdate, r#"{"guild_id":"200","stickers":[{"id":"6","name":"s","tags":"t","type":2,"format_type":1,"description":null}]}"#.to_owned()),
        (DispatchKind::GuildScheduledEventCreate, r#"{"id":"7","guild_id":"200","channel_id":"500","name":"n","scheduled_start_time":"2030-01-01T00:00:00+00:00","scheduled_end_time":null,"privacy_level":2,"status":1,"entity_type":2,"entity_id":null,"entity_metadata":null,"recurrence_rule":null}"#.to_owned()),
        (DispatchKind::GuildScheduledEventUpdate, r#"{"id":"7","guild_id":"200","channel_id":"500","name":"n","scheduled_start_time":"2030-01-01T00:00:00+00:00","scheduled_end_time":null,"privacy_level":2,"status":1,"entity_type":2,"entity_id":null,"entity_metadata":null,"recurrence_rule":null}"#.to_owned()),
        (DispatchKind::GuildScheduledEventDelete, r#"{"id":"7","guild_id":"200","channel_id":"500","name":"n","scheduled_start_time":"2030-01-01T00:00:00+00:00","scheduled_end_time":null,"privacy_level":2,"status":1,"entity_type":2,"entity_id":null,"entity_metadata":null,"recurrence_rule":null}"#.to_owned()),
        (DispatchKind::GuildScheduledEventUserAdd, r#"{"guild_scheduled_event_id":"7","user_id":"1","guild_id":"200"}"#.to_owned()),
        (DispatchKind::GuildScheduledEventUserRemove, r#"{"guild_scheduled_event_id":"7","user_id":"1","guild_id":"200"}"#.to_owned()),
        (DispatchKind::StageInstanceCreate, r#"{"id":"8","guild_id":"200","channel_id":"500","topic":"t","privacy_level":2,"discoverable_disabled":true}"#.to_owned()),
        (DispatchKind::StageInstanceUpdate, r#"{"id":"8","guild_id":"200","channel_id":"500","topic":"t","privacy_level":2,"discoverable_disabled":true}"#.to_owned()),
        (DispatchKind::StageInstanceDelete, r#"{"id":"8","guild_id":"200","channel_id":"500","topic":"t","privacy_level":2,"discoverable_disabled":true}"#.to_owned()),
        (DispatchKind::AutoModerationRuleCreate, r#"{"id":"9","guild_id":"200","name":"n","creator_id":"1","event_type":1,"trigger_type":1,"trigger_metadata":{"keyword_filter":["x"]},"actions":[{"type":1}],"enabled":true,"exempt_roles":[],"exempt_channels":[]}"#.to_owned()),
        (DispatchKind::AutoModerationRuleUpdate, r#"{"id":"9","guild_id":"200","name":"n","creator_id":"1","event_type":1,"trigger_type":1,"trigger_metadata":{"keyword_filter":["x"]},"actions":[{"type":1}],"enabled":true,"exempt_roles":[],"exempt_channels":[]}"#.to_owned()),
        (DispatchKind::AutoModerationRuleDelete, r#"{"id":"9","guild_id":"200","name":"n","creator_id":"1","event_type":1,"trigger_type":1,"trigger_metadata":{"keyword_filter":["x"]},"actions":[{"type":1}],"enabled":true,"exempt_roles":[],"exempt_channels":[]}"#.to_owned()),
        (DispatchKind::AutoModerationActionExecution, r#"{"guild_id":"200","action":{"type":2,"metadata":{"channel_id":"500"}},"rule_id":"9","rule_trigger_type":1,"user_id":"1","content":"","matched_keyword":null,"matched_content":null}"#.to_owned()),
        (DispatchKind::GuildAuditLogEntryCreate, r#"{"guild_id":"200","id":"11","target_id":"1","user_id":"2","action_type":22,"changes":[],"reason":"spam"}"#.to_owned()),
        (DispatchKind::GuildMembersChunk, r#"{"guild_id":"200","members":[],"chunk_index":0,"chunk_count":1,"not_found":["9"],"nonce":"n"}"#.to_owned()),
    ];
    assert_eq!(table.len(), DispatchKind::COUNT - 3, "all but typing, presence and unknown");
    for (kind, body) in table {
        let t = kind.name().unwrap();
        let event = Dispatch::decode(kind, t, &body).unwrap_or_else(|e| panic!("{t}: {e}"));
        assert_eq!(event.kind(), kind, "{t}");
        assert!(!matches!(event, Dispatch::Unknown { .. }), "{t}");
        assert!(!format!("{:?}", event.clone()).is_empty());
    }
}

#[test]
fn ready_reads_every_upstream_field() {
    let Dispatch::Ready(r) = dispatch("READY", &ready()) else { panic!() };
    assert_eq!(r.v, 10);
    assert_eq!(r.user.id.get(), 100);
    assert!(r.user.bot);
    assert_eq!(r.guilds.len(), 2);
    assert!(r.guilds.iter().all(|g| g.unavailable));
    assert_eq!(r.session_id.expose(), "sess-abc");
    assert_eq!(r.resume_gateway_url, "wss://resume.example/");
    assert_eq!(r.shard, Some([0, 1]));
    assert_eq!(r.application.id.get(), 300);
    assert_eq!(r.application.flags.unwrap().bits(), 8_667_136);
    assert_eq!(r.application.flags_new.as_deref(), Some("8667136"));
}

#[test]
fn ready_debug_redacts_the_session() {
    let event = dispatch("READY", &ready());
    let text = format!("{event:?}");
    assert!(!text.contains("sess-abc"), "{text}");
    assert!(text.contains("Secret(***)"), "{text}");
}

#[test]
fn ready_needs_the_session_and_the_application() {
    // Required by the field policy of `ARCHITECTURE.md` 7.7, and the failure says where.
    let user = r#""user":{"id":"1"}"#;
    let err = Dispatch::decode(DispatchKind::Ready, "READY", &format!(r#"{{{user},"session_id":"s"}}"#)).unwrap_err();
    assert!(err.source.to_string().contains("application"), "{err}");
    let body = format!(r#"{{{user},"application":{{"id":"1"}}}}"#);
    let err = Dispatch::decode(DispatchKind::Ready, "READY", &body).unwrap_err();
    assert!(err.source.to_string().contains("session_id"), "{err}");
}

#[test]
fn ready_tolerates_missing_optionals_and_a_null_guild_list() {
    let body = r#"{"user":{"id":"1"},"guilds":null,"session_id":"s","application":{"id":"2"}}"#;
    let Dispatch::Ready(r) = dispatch("READY", body) else { panic!() };
    assert_eq!(r.v, 0);
    assert!(r.guilds.is_empty());
    assert!(r.resume_gateway_url.is_empty());
    assert_eq!(r.shard, None);
    assert!(r.application.flags.is_none() && r.application.flags_new.is_none());
}

#[test]
fn resumed_never_fails_on_its_body() {
    // `_DataPayload<Resumed, undefined>`, and Discord sends `{"_trace":[...]}`.
    for body in ["", "null", r#"{"_trace":["gateway-prd"]}"#, "not json"] {
        assert!(matches!(dispatch("RESUMED", body), Dispatch::Resumed), "{body:?}");
    }
}

#[test]
fn guild_create_reads_the_create_only_fields() {
    let Dispatch::GuildCreate(g) = dispatch("GUILD_CREATE", &guild_create()) else { panic!() };
    assert_eq!(g.id.get(), 200);
    assert_eq!(g.name, "Test");
    assert!(!g.unavailable);
    assert_eq!(g.member_count, Some(2));
    assert!(g.joined_at.is_some());
    assert_eq!(g.roles.len(), 1);
    assert_eq!(g.members.len(), 1);
    assert_eq!(g.channels.len(), 1);
    assert_eq!(g.features.len(), 2);
    assert_eq!(g.features[1].as_str(), "SOME_NEW_FEATURE");
}

#[test]
fn guild_update_has_no_create_only_fields() {
    let Dispatch::GuildUpdate(g) = dispatch("GUILD_UPDATE", &format!("{{{}}}", guild_base())) else { panic!() };
    assert!(g.members.is_empty() && g.channels.is_empty());
    assert_eq!(g.member_count, None);
    assert_eq!(g.joined_at, None);
}

#[test]
fn guild_delete_tells_an_outage_from_a_removal() {
    // `[types 9.2]`: `unavailable: true` is an outage, absent means the bot was removed.
    let Dispatch::GuildDelete(g) = dispatch("GUILD_DELETE", r#"{"id":"200","unavailable":true}"#) else { panic!() };
    assert_eq!(g.id.get(), 200);
    assert!(g.unavailable);
    let Dispatch::GuildDelete(g) = dispatch("GUILD_DELETE", r#"{"id":"200"}"#) else { panic!() };
    assert!(!g.unavailable);
}

#[test]
fn channel_events_keep_an_unknown_kind() {
    for t in ["CHANNEL_CREATE", "CHANNEL_UPDATE", "CHANNEL_DELETE"] {
        // Tolerance level 4, and a kind wider than `u8`.
        let (Dispatch::ChannelCreate(c) | Dispatch::ChannelUpdate(c) | Dispatch::ChannelDelete(c)) =
            dispatch(t, &channel(777))
        else {
            panic!("{t}")
        };
        assert_eq!(c.id.get(), 500);
        assert_eq!(c.kind.get(), 777);
        assert_eq!(c.guild_id.map(|g| g.get()), Some(200));
        assert_eq!(serde_json::to_value(&c).unwrap()["type"], 777);
    }
}

#[test]
fn channel_event_with_no_id_fails_with_its_path() {
    let err = Dispatch::decode(DispatchKind::ChannelCreate, "CHANNEL_CREATE", r#"{"type":0}"#).unwrap_err();
    assert!(err.source.to_string().contains("`id`"), "{err}");
}

#[test]
fn member_add_carries_its_guild() {
    let Dispatch::GuildMemberAdd(m) = dispatch("GUILD_MEMBER_ADD", &member(Some("200"))) else { panic!() };
    assert_eq!(m.guild_id.map(|g| g.get()), Some(200));
    assert_eq!(m.user.as_ref().map(|u| u.id.get()), Some(100));
    assert_eq!(m.nick.as_deref(), Some("al"));
    assert_eq!(m.roles.len(), 2);
    assert!(m.joined_at.is_some());
}

#[test]
fn member_add_needs_guild_and_user() {
    // `Member` defaults every field, and upstream requires both on this event.
    let decode = |body: &str| Dispatch::decode(DispatchKind::GuildMemberAdd, "GUILD_MEMBER_ADD", body).unwrap_err();
    let err = decode(&member(None));
    assert_eq!(err.path, "guild_id");
    assert!(err.source.to_string().contains("guild_id"), "{err}");
    assert_eq!(decode(r#"{"guild_id":"200","roles":[]}"#).path, "user");
    assert_eq!(decode("{}").path, "guild_id");
}

fn member_update(extra: &str) -> Box<dr_model::GuildMemberUpdateEvent> {
    let body = format!(r#"{{"guild_id":"200","user":{USER},"roles":["400"],"joined_at":null{extra}}}"#);
    let Dispatch::GuildMemberUpdate(m) = dispatch("GUILD_MEMBER_UPDATE", &body) else { panic!() };
    m
}

#[test]
fn member_update_tells_absent_from_null() {
    let m = member_update("");
    assert_eq!(m.guild_id.get(), 200);
    assert_eq!(m.user.id.get(), 100);
    assert_eq!(m.roles.len(), 1);
    assert_eq!(m.joined_at, None);
    assert_eq!(m.pending, None);
    assert!(m.nick.is_absent());
    assert!(m.avatar.is_absent());
    assert!(m.banner.is_absent());
    assert!(m.premium_since.is_absent());
    assert!(m.communication_disabled_until.is_absent());

    let m = member_update(
        r#","nick":null,"avatar":null,"banner":null,"premium_since":null,"communication_disabled_until":null"#,
    );
    assert!(m.nick.is_null());
    assert!(m.avatar.is_null());
    assert!(m.banner.is_null());
    assert!(m.premium_since.is_null());
    assert!(m.communication_disabled_until.is_null());
}

#[test]
fn member_update_reads_every_upstream_value() {
    let m = member_update(
        r#","nick":"al","avatar":"aa","banner":"bb","premium_since":"2021-01-01T00:00:00+00:00",
        "communication_disabled_until":"2021-01-02T00:00:00+00:00","deaf":true,"mute":false,"flags":1099511627777,
        "pending":true,"avatar_decoration_data":null,"collectibles":null"#,
    );
    assert_eq!(m.nick.as_option().map(String::as_str), Some("al"));
    assert_eq!(m.avatar.as_option().map(String::as_str), Some("aa"));
    assert_eq!(m.banner.as_option().map(String::as_str), Some("bb"));
    assert!(m.premium_since.as_option().is_some());
    assert!(m.communication_disabled_until.as_option().is_some());
    assert_eq!((m.deaf, m.mute), (Some(true), Some(false)));
    assert_eq!(m.flags.map(|f| f.bits()), Some((1 << 40) | 1));
    assert_eq!(m.pending, Some(true));
}

#[test]
fn member_update_writes_back_what_it_read() {
    let absent = serde_json::to_value(member_update("")).unwrap();
    for key in ["nick", "avatar", "banner", "premium_since", "pending", "deaf", "mute", "flags"] {
        assert!(absent.get(key).is_none(), "{key}");
    }
    assert_eq!(absent["joined_at"], Value::Null);
    let null = serde_json::to_value(member_update(r#","nick":null"#)).unwrap();
    assert_eq!(null["nick"], Value::Null);
}

#[test]
fn member_remove_reads_guild_and_user() {
    let body = format!(r#"{{"guild_id":"200","user":{USER}}}"#);
    let Dispatch::GuildMemberRemove(m) = dispatch("GUILD_MEMBER_REMOVE", &body) else { panic!() };
    assert_eq!((m.guild_id.get(), m.user.id.get()), (200, 100));
}

#[test]
fn member_remove_with_no_user_is_a_failure() {
    let err =
        Dispatch::decode(DispatchKind::GuildMemberRemove, "GUILD_MEMBER_REMOVE", r#"{"guild_id":"1"}"#).unwrap_err();
    assert!(err.source.to_string().contains("user"), "{err}");
}

#[test]
fn message_create_reads_the_extra_fields() {
    let Dispatch::MessageCreate(m) = dispatch("MESSAGE_CREATE", &message()) else { panic!() };
    assert_eq!(m.id.get(), 600);
    assert_eq!(m.channel_id.get(), 500);
    assert_eq!(m.guild_id.map(|g| g.get()), Some(200));
    assert_eq!(m.author.id.get(), 100);
    assert_eq!(m.content, "!ping");
    // `member` has no `user`, and `mentions` are `APIUserWithMember[]`.
    assert!(m.member.as_ref().unwrap().user.is_none());
    assert_eq!(m.mentions.len(), 1);
    assert_eq!(m.mentions[0].id.get(), 101);
    assert_eq!(m.timestamp, Some(Timestamp::parse("2021-01-01T00:00:00.123456+00:00").unwrap()));
    assert!(m.referenced_message.is_absent());
}

#[test]
fn message_create_failure_gives_a_path_and_keeps_going() {
    let err =
        Dispatch::decode(DispatchKind::MessageCreate, "MESSAGE_CREATE", r#"{"id":"1","channel_id":"2"}"#).unwrap_err();
    assert!(err.source.to_string().contains("author"), "{err}");

    let body = format!(r#"{{"id":"1","channel_id":"2","author":{USER},"content":5}}"#);
    let err = Dispatch::decode(DispatchKind::MessageCreate, "MESSAGE_CREATE", &body).unwrap_err();
    assert_eq!(err.path, "content");

    let body = r#"{"id":"1","channel_id":"2","author":{"id":"abc"}}"#;
    let err = Dispatch::decode(DispatchKind::MessageCreate, "MESSAGE_CREATE", body).unwrap_err();
    assert_eq!(err.path, "author.id");
}

#[test]
fn message_update_partial_body_leaves_everything_unset() {
    // The embeds-only update of `[client 18]`.
    let body = r#"{"id":"600","channel_id":"500","guild_id":"200","embeds":[{"title":"t"}]}"#;
    let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", body) else { panic!() };
    assert_eq!((m.id.get(), m.channel_id.get()), (600, 500));
    assert_eq!(m.guild_id.map(|g| g.get()), Some(200));
    assert_eq!(m.embeds.as_ref().unwrap()[0].title.as_deref(), Some("t"));
    assert!(m.author.is_none() && m.member.is_none() && m.content.is_none() && m.timestamp.is_none());
    assert!(m.kind.is_none() && m.channel_type.is_none() && m.tts.is_none() && m.mention_everyone.is_none());
    assert!(m.mentions.is_none() && m.mention_roles.is_none() && m.attachments.is_none());
    assert!(m.reactions.is_none() && m.components.is_none() && m.flags.is_none() && m.pinned.is_none());
    assert!(m.nonce.is_none() && m.webhook_id.is_none() && m.application_id.is_none());
    assert!(m.message_reference.is_none());
    assert!(m.edited_timestamp.is_absent());
    assert!(m.referenced_message.is_absent());
}

#[test]
fn message_update_with_no_id_is_a_failure() {
    assert!(Dispatch::decode(DispatchKind::MessageUpdate, "MESSAGE_UPDATE", r#"{"channel_id":"1"}"#).is_err());
    assert!(Dispatch::decode(DispatchKind::MessageUpdate, "MESSAGE_UPDATE", r#"{"id":"1"}"#).is_err());
}

#[test]
fn message_update_full_body_reads_every_field() {
    let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", &message()) else { panic!() };
    assert_eq!(m.author.as_ref().unwrap().id.get(), 100);
    assert_eq!(m.content.as_deref(), Some("!ping"));
    assert!(m.member.as_ref().unwrap().user.is_none());
    assert!(m.timestamp.is_some());
    assert!(m.edited_timestamp.is_null());
    assert_eq!(m.kind.map(|k| k.get()), Some(0));
    assert_eq!(m.channel_type.map(|k| k.get()), Some(0));
    assert_eq!(m.tts, Some(false));
    assert_eq!(m.mention_everyone, Some(false));
    assert_eq!(m.mentions.as_ref().unwrap().len(), 1);
    assert_eq!(m.pinned, Some(false));
    assert_eq!(m.flags, Some(MessageFlags::empty()));
}

#[test]
fn message_update_edited_timestamp_is_tri_state() {
    let decode = |extra: &str| {
        let body = format!(r#"{{"id":"1","channel_id":"2"{extra}}}"#);
        let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", &body) else { panic!() };
        m.edited_timestamp
    };
    assert_eq!(decode(""), Patch::Absent);
    assert_eq!(decode(r#","edited_timestamp":null"#), Patch::Null);
    let value = decode(r#","edited_timestamp":"2021-01-01T00:00:00+00:00""#);
    assert_eq!(value, Patch::Value(Timestamp::parse("2021-01-01T00:00:00+00:00").unwrap()));
}

#[test]
fn message_update_referenced_message_is_tri_state() {
    let decode = |extra: String| {
        let body = format!(r#"{{"id":"1","channel_id":"2"{extra}}}"#);
        let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", &body) else { panic!() };
        m.referenced_message
    };
    assert!(decode(String::new()).is_absent());
    assert!(decode(r#","referenced_message":null"#.to_owned()).is_null());
    let value = decode(format!(r#","referenced_message":{}"#, message()));
    assert_eq!(value.as_option().unwrap().id.get(), 600);
}

#[test]
fn message_update_keeps_unknown_flag_bits() {
    let body = format!(r#"{{"id":"1","channel_id":"2","flags":{}}}"#, (1u64 << 40) | (1 << 6));
    let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", &body) else { panic!() };
    let flags = m.flags.unwrap();
    assert!(flags.contains(MessageFlags::EPHEMERAL));
    assert_eq!(flags.bits(), (1 << 40) | (1 << 6));
}

#[test]
fn message_update_null_list_reads_as_not_sent() {
    let body = r#"{"id":"1","channel_id":"2","embeds":null,"components":null,"content":null}"#;
    let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", body) else { panic!() };
    assert!(m.embeds.is_none() && m.components.is_none() && m.content.is_none());
}

#[test]
fn message_update_writes_only_what_it_read() {
    let body = r#"{"id":"1","channel_id":"2","pinned":true,"edited_timestamp":null}"#;
    let Dispatch::MessageUpdate(m) = dispatch("MESSAGE_UPDATE", body) else { panic!() };
    let written = serde_json::to_value(&m).unwrap();
    assert_eq!(written, json!({"id":"1","channel_id":"2","pinned":true,"edited_timestamp":null}));
}

#[test]
fn message_delete_guild_is_optional() {
    let Dispatch::MessageDelete(m) = dispatch("MESSAGE_DELETE", r#"{"id":"1","channel_id":"2"}"#) else { panic!() };
    assert_eq!((m.id.get(), m.channel_id.get(), m.guild_id), (1, 2, None));
    let Dispatch::MessageDelete(m) = dispatch("MESSAGE_DELETE", r#"{"id":"1","channel_id":"2","guild_id":"3"}"#) else {
        panic!()
    };
    assert_eq!(m.guild_id.map(|g| g.get()), Some(3));
}

#[test]
fn interaction_create_reads_and_redacts() {
    let event = dispatch("INTERACTION_CREATE", interaction());
    let text = format!("{event:?}");
    assert!(!text.contains("interaction-token"), "{text}");
    let Dispatch::InteractionCreate(i) = event else { panic!() };
    assert_eq!(i.id.get(), 700);
    assert_eq!(i.application_id.get(), 300);
    assert_eq!(i.kind.get(), 2);
    assert_eq!(i.token.expose(), "interaction-token");
    assert_eq!(i.guild_id.map(|g| g.get()), Some(200));
}

#[test]
fn interaction_create_keeps_an_unknown_kind() {
    let body = r#"{"id":"700","application_id":"300","type":99,"token":"t","version":1}"#;
    let Dispatch::InteractionCreate(i) = dispatch("INTERACTION_CREATE", body) else { panic!() };
    assert_eq!(i.kind.get(), 99);
}

// ---- unknown arm --------------------------------------------------------------------------------

#[test]
fn unknown_event_keeps_its_name_and_exact_body() {
    let body = format!(r#"{{ "b" : 1.50, "a":[ "{}", null ] }}"#, json_escape_of_capital_a());
    let body = body.as_str();
    match dispatch("BRAND_NEW_EVENT", body) {
        Dispatch::Unknown { t, raw } => {
            assert_eq!(t, "BRAND_NEW_EVENT");
            assert_eq!(raw.get(), body);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn typing_and_presence_arrive_raw_and_keep_their_kind() {
    for (t, kind) in [("TYPING_START", DispatchKind::TypingStart), ("PRESENCE_UPDATE", DispatchKind::PresenceUpdate)] {
        let event = dispatch(t, r#"{"x":1}"#);
        assert!(matches!(&event, Dispatch::Unknown { t: name, .. } if name == t));
        assert_eq!(event.kind(), kind);
    }
    assert_eq!(dispatch("BRAND_NEW_EVENT", "1").kind(), DispatchKind::Unknown);
}

#[test]
fn unknown_event_body_can_be_any_json_value_or_nothing() {
    for body in ["null", "1", r#""text""#, "[]", "true"] {
        let Dispatch::Unknown { raw, .. } = dispatch("X", body) else { panic!() };
        assert_eq!(raw.get(), body);
    }
    let Dispatch::Unknown { raw, .. } = dispatch("X", "") else { panic!() };
    assert_eq!(raw.get(), "null");
}

#[test]
fn unknown_event_with_broken_json_is_a_failure() {
    assert!(Dispatch::decode(DispatchKind::Unknown, "X", "{broken").is_err());
    assert!(Dispatch::decode(DispatchKind::Unknown, "X", "1 2").is_err());
}

#[test]
fn unknown_event_debug_hides_its_body() {
    // An event with no typed arm may hold a secret, as VOICE_SERVER_UPDATE did before it had one.
    let body = r#"{"token":"voice-secret","guild_id":"1","endpoint":"x"}"#;
    let event = dispatch("VOICE_CHANNEL_EFFECT_SEND", body);
    let text = format!("{event:?}");
    assert!(!text.contains("voice-secret"), "{text}");
    assert!(text.contains("VOICE_CHANNEL_EFFECT_SEND"), "{text}");
    assert!(text.contains(&body.len().to_string()), "{text}");
}

#[test]
fn typed_event_given_an_empty_or_null_body_is_a_failure() {
    for kind in DispatchKind::ALL {
        let Some(t) = kind.name() else { continue };
        if kind == DispatchKind::Resumed || kind.is_sheddable() {
            continue;
        }
        for body in ["", "null", "[]"] {
            assert!(Dispatch::decode(kind, t, body).is_err(), "{t} {body:?}");
        }
    }
}

// ---- send payloads, byte for byte ---------------------------------------------------------------

#[test]
fn heartbeat_writes_a_null_rather_than_omitting_it() {
    // `[types 9.2]`: `d` is `number | null`, and null must be serialized.
    let frame = GatewayFrame::heartbeat(None);
    assert_eq!(serde_json::to_string(&frame).unwrap(), r#"{"op":1,"d":null}"#);
    let frame: GatewayFrame<Heartbeat> = GatewayFrame::heartbeat(Some(251));
    assert_eq!(serde_json::to_string(&frame).unwrap(), r#"{"op":1,"d":251}"#);
}

#[test]
fn identify_writes_shard_capabilities_and_compress_always() {
    // `[gateway 3.6]`.
    let frame = GatewayFrame::identify(identify());
    assert_eq!(
        serde_json::to_string(&frame).unwrap(),
        r#"{"op":2,"d":{"token":"tok","properties":{"os":"linux","browser":"discord.rs","device":"discord.rs"},"compress":false,"shard":[0,1],"intents":33281,"capabilities":0}}"#
    );
}

#[test]
fn identify_writes_threshold_and_presence_only_when_set() {
    let mut d = identify();
    d.large_threshold = Some(250);
    d.presence = Some(presence());
    let value = serde_json::to_value(GatewayFrame::identify(d)).unwrap();
    assert_eq!(value["d"]["large_threshold"], 250);
    assert_eq!(value["d"]["presence"], json!({"since":null,"activities":[],"status":"online","afk":false}));
}

#[test]
fn identify_writes_intents_as_a_number_and_keeps_unknown_bits() {
    let mut d = identify();
    d.intents = Intents::from_bits_retain((1 << 30) | 1);
    let value = serde_json::to_value(&d).unwrap();
    assert_eq!(value["intents"], (1u64 << 30) | 1);
}

#[test]
fn resume_writes_the_three_fields() {
    let frame = GatewayFrame::resume(Resume { token: "tok".into(), session_id: "sess".into(), seq: 1337 });
    assert_eq!(
        serde_json::to_string(&frame).unwrap(),
        r#"{"op":6,"d":{"token":"tok","session_id":"sess","seq":1337}}"#
    );
}

#[test]
fn presence_update_writes_null_since_and_every_field() {
    let frame = GatewayFrame::presence(presence());
    assert_eq!(
        serde_json::to_string(&frame).unwrap(),
        r#"{"op":3,"d":{"since":null,"activities":[],"status":"online","afk":false}}"#
    );
}

#[test]
fn presence_update_with_activities() {
    let d = UpdatePresence {
        since: Some(1_600_000_000_000),
        activities: vec![
            Activity { name: "a game".into(), kind: Activity::PLAYING, state: None, url: None },
            Activity {
                name: "stream".into(),
                kind: Activity::STREAMING,
                state: None,
                url: Some("https://example.test/s".into()),
            },
            Activity { name: "Custom Status".into(), kind: Activity::CUSTOM, state: Some("hi".into()), url: None },
        ],
        status: PresenceStatus::Idle,
        afk: true,
    };
    assert_eq!(
        serde_json::to_string(&GatewayFrame::presence(d)).unwrap(),
        concat!(
            r#"{"op":3,"d":{"since":1600000000000,"activities":["#,
            r#"{"name":"a game","type":0},"#,
            r#"{"name":"stream","type":1,"url":"https://example.test/s"},"#,
            r#"{"name":"Custom Status","type":4,"state":"hi"}],"#,
            r#""status":"idle","afk":true}}"#
        )
    );
}

#[test]
fn activity_kinds_match_upstream() {
    // `ActivityType` of `payloads/v10/gateway.ts`, an implicit count from 0.
    assert_eq!(
        [
            Activity::PLAYING,
            Activity::STREAMING,
            Activity::LISTENING,
            Activity::WATCHING,
            Activity::CUSTOM,
            Activity::COMPETING
        ],
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn presence_status_covers_upstream_and_keeps_unknown_values() {
    // `PresenceUpdateStatus` of `payloads/v10/gateway.ts`.
    for (status, text) in [
        (PresenceStatus::Online, "online"),
        (PresenceStatus::DoNotDisturb, "dnd"),
        (PresenceStatus::Idle, "idle"),
        (PresenceStatus::Invisible, "invisible"),
        (PresenceStatus::Offline, "offline"),
    ] {
        assert_eq!(serde_json::to_value(&status).unwrap(), text);
        assert_eq!(serde_json::from_value::<PresenceStatus>(json!(text)).unwrap(), status);
    }
    let unknown: PresenceStatus = serde_json::from_str(r#""sleeping""#).unwrap();
    assert!(matches!(unknown, PresenceStatus::Unknown(_)));
    assert_eq!(serde_json::to_string(&unknown).unwrap(), r#""sleeping""#);
}

#[test]
fn send_payload_debug_redacts_token_and_session() {
    let text = format!("{:?}", GatewayFrame::identify(identify()));
    assert!(!text.contains("tok\""), "{text}");
    assert!(text.contains("Secret(***)"), "{text}");

    let resume = Resume { token: "real-token".into(), session_id: "real-session".into(), seq: 1 };
    let text = format!("{:?}", GatewayFrame::resume(resume));
    assert!(!text.contains("real-token") && !text.contains("real-session"), "{text}");
}

#[test]
fn identify_reads_back_with_upstream_defaults() {
    // `compress?`, `large_threshold?`, `shard?`, `presence?`, `capabilities?` are optional upstream.
    let text = r#"{"op":2,"d":{"token":"tok","properties":{"os":"o","browser":"b","device":"d"},"intents":513}}"#;
    let frame: GatewayFrame<Identify> = serde_json::from_str(text).unwrap();
    assert_eq!(frame.op, 2);
    assert_eq!(frame.d.token.expose(), "tok");
    assert_eq!(frame.d.shard, [0, 1]);
    assert_eq!(frame.d.capabilities, 0);
    assert!(!frame.d.compress);
    assert!(frame.d.large_threshold.is_none() && frame.d.presence.is_none());
    assert_eq!(frame.d.intents, Intents::GUILDS | Intents::GUILD_MESSAGES);
}

#[test]
fn identify_with_no_token_or_intents_does_not_read() {
    assert!(serde_json::from_str::<Identify>(r#"{"properties":{},"intents":1}"#).is_err());
    assert!(serde_json::from_str::<Identify>(r#"{"token":"t","properties":{}}"#).is_err());
    assert!(serde_json::from_str::<Identify>(r#"{"token":"t","intents":1}"#).is_err());
}

#[test]
fn send_payloads_round_trip() {
    let mut d = identify();
    d.large_threshold = Some(50);
    d.presence = Some(presence());
    let first = serde_json::to_string(&GatewayFrame::identify(d)).unwrap();
    let read: GatewayFrame<Identify> = serde_json::from_str(&first).unwrap();
    assert_eq!(serde_json::to_string(&read).unwrap(), first);

    let first = r#"{"op":6,"d":{"token":"t","session_id":"s","seq":5}}"#;
    let read: GatewayFrame<Resume> = serde_json::from_str(first).unwrap();
    assert_eq!(serde_json::to_string(&read).unwrap(), first);
    assert_eq!(read.d.seq, 5);

    let read: GatewayFrame<Heartbeat> = serde_json::from_str(r#"{"op":1,"d":null}"#).unwrap();
    assert_eq!(read.d, None);
}

#[test]
fn voice_events_hide_their_secrets() {
    let body = r#"{"token":"voice-secret","guild_id":"1","endpoint":"x"}"#;
    let Dispatch::VoiceServerUpdate(server) = dispatch("VOICE_SERVER_UPDATE", body) else { panic!() };
    assert_eq!(server.token.expose(), "voice-secret");
    assert!(!format!("{server:?}").contains("voice-secret"));
    let body = r#"{"guild_id":"1","channel_id":null,"user_id":"2","session_id":"sess-secret"}"#;
    let Dispatch::VoiceStateUpdate(state) = dispatch("VOICE_STATE_UPDATE", body) else { panic!() };
    assert!(state.channel_id.is_none(), "a leave");
    assert!(!format!("{state:?}").contains("sess-secret"));
}
