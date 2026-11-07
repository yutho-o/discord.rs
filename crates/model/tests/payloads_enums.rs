//! Enums, flags, opcodes, close codes, JSON error codes, the REST error body, routes and REST result
//! types of `dr_model`. The tables at the end are the upstream values, extracted from
//! discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu): `payloads/v10`, `gateway/v10.ts`,
//! `rest/common.ts`, `payloads/common.ts`.

use dr_core::Id;
use dr_model::rest::{GetApplicationCommandsQuery, GetGatewayBotResult, GetGuildQuery, PostInteractionCallbackQuery};
use dr_model::{
    AllowedMentionType, ApplicationCommandOptionType, ApplicationCommandType, ApplicationFlags, ButtonStyle,
    ChannelType, ComponentType, GATEWAY_VERSION, GatewayCapabilityBits, GatewayCloseCode, GatewayOpcode, GuildFeature,
    Intents, InteractionResponseType, InteractionType, JsonErrorCode, Locale, MessageFlags, MessageReferenceType,
    MessageType, Permissions, RestErrorBody, RestRateLimit, TextInputStyle, UserFlags, routes,
};
use serde_json::json;

// ------------------------------------------------------------------------------------------------
// Open integers
// ------------------------------------------------------------------------------------------------

#[test]
fn open_int_keeps_an_unknown_value_through_a_round_trip() {
    let kind: ChannelType = serde_json::from_str("777").unwrap();
    assert_eq!(kind, ChannelType(777));
    assert_eq!(kind.name(), None);
    assert_eq!(format!("{kind:?}"), "ChannelType(777)");
    assert_eq!(serde_json::to_string(&kind).unwrap(), "777");
}

#[test]
fn open_int_debug_and_name_use_the_canonical_const() {
    assert_eq!(format!("{:?}", ChannelType::GUILD_TEXT), "ChannelType::GUILD_TEXT");
    assert_eq!(ChannelType::GUILD_TEXT.name(), Some("GUILD_TEXT"));
    // An alias has the same value, so it reports the current upstream name.
    assert_eq!(ChannelType::GUILD_NEWS.name(), Some("GUILD_ANNOUNCEMENT"));
    assert_eq!(ComponentType::SELECT_MENU.name(), Some("STRING_SELECT"));
    assert_eq!(ApplicationCommandOptionType::SUB_COMMAND.name(), Some("SUBCOMMAND"));
    assert_eq!(JsonErrorCode::CHANNEL_SEND_RATE_LIMIT.name(), Some("CHANNEL_WRITE_RATE_LIMIT"));
    assert_eq!(
        JsonErrorCode::FAILED_TO_RESIZE_ASSET_BELOW_THE_MINIMUM_SIZE.name(),
        Some("FAILED_TO_RESIZE_ASSET_BELOW_THE_MAXIMUM_SIZE")
    );
}

#[test]
fn aliases_work_as_patterns() {
    let kind = |k: ChannelType| match k {
        ChannelType::GUILD_NEWS => "announcement",
        ChannelType::GUILD_NEWS_THREAD | ChannelType::GUILD_PUBLIC_THREAD | ChannelType::GUILD_PRIVATE_THREAD => {
            "thread"
        }
        _ => "other",
    };
    assert_eq!(kind(ChannelType(5)), "announcement");
    assert_eq!(kind(ChannelType::ANNOUNCEMENT_THREAD), "thread");
    assert_eq!(kind(ChannelType::PRIVATE_THREAD), "thread");
    assert_eq!(kind(ChannelType(0)), "other");
}

#[test]
fn a_value_above_the_repr_is_a_decode_failure() {
    // ARCHITECTURE 7.4 rule 5: deliberate, and the reason JsonErrorCode and GatewayOpcode are u32.
    assert!(serde_json::from_str::<ChannelType>("65536").is_err());
    assert!(serde_json::from_str::<ChannelType>("-1").is_err());
    assert_eq!(serde_json::from_str::<JsonErrorCode>("530007").unwrap(), JsonErrorCode::INVALID_CLIENT_SECRET);
    assert_eq!(serde_json::from_str::<JsonErrorCode>("340024").unwrap(), JsonErrorCode(340_024));
}

#[test]
fn opcodes_are_wide_and_tolerate_unknown_values() {
    assert_eq!(serde_json::from_str::<GatewayOpcode>("999").unwrap(), GatewayOpcode(999));
    assert_eq!(serde_json::from_str::<GatewayOpcode>("70000").unwrap(), GatewayOpcode(70_000));
    assert_eq!(GatewayOpcode::REQUEST_CHANNEL_INFO.get(), 43);
    assert_eq!(GATEWAY_VERSION, "10");
}

#[test]
fn close_codes_have_no_4006() {
    assert_eq!(GatewayCloseCode(4006).name(), None);
    assert_eq!(format!("{:?}", GatewayCloseCode(4006)), "GatewayCloseCode(4006)");
    assert_eq!(GatewayCloseCode(4007), GatewayCloseCode::INVALID_SEQ);
}

// ------------------------------------------------------------------------------------------------
// Open strings
// ------------------------------------------------------------------------------------------------

#[test]
fn open_str_keeps_an_unknown_value_and_reads_escapes() {
    let unknown: GuildFeature = serde_json::from_str("\"SOME_NEW_FEATURE\"").unwrap();
    assert!(matches!(unknown, GuildFeature::Unknown(_)));
    assert_eq!(unknown.as_str(), "SOME_NEW_FEATURE");
    assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"SOME_NEW_FEATURE\"");

    // An escaped `NEWS` is still the known variant.
    let escaped: GuildFeature = serde_json::from_str("\"N\\u0045WS\"").unwrap();
    assert_eq!(escaped, GuildFeature::News);
}

#[test]
fn locale_reads_the_wire_spelling() {
    assert_eq!(serde_json::from_str::<Locale>("\"en-US\"").unwrap(), Locale::EnglishUS);
    assert_eq!(serde_json::from_str::<Locale>("\"es-419\"").unwrap(), Locale::SpanishLATAM);
    assert_eq!(serde_json::to_string(&Locale::Swedish).unwrap(), "\"sv-SE\"");
    assert_eq!(Locale::from("xx-XX").as_str(), "xx-XX");
}

#[test]
fn allowed_mention_type_writes_the_plural_wire_values() {
    assert_eq!(serde_json::to_string(&AllowedMentionType::Roles).unwrap(), "\"roles\"");
    assert_eq!(serde_json::to_string(&AllowedMentionType::Users).unwrap(), "\"users\"");
    assert_eq!(serde_json::to_string(&AllowedMentionType::Everyone).unwrap(), "\"everyone\"");
}

// ------------------------------------------------------------------------------------------------
// Flags
// ------------------------------------------------------------------------------------------------

#[test]
fn permissions_write_a_string_and_read_both_forms() {
    let p = Permissions::ADMINISTRATOR | Permissions::BYPASS_SLOWMODE;
    let text = serde_json::to_string(&p).unwrap();
    assert_eq!(text, format!("\"{}\"", 8u64 | (1u64 << 52)));
    assert_eq!(serde_json::from_str::<Permissions>(&text).unwrap(), p);
    assert_eq!(serde_json::from_str::<Permissions>("8").unwrap(), Permissions::ADMINISTRATOR);
}

#[test]
fn number_flags_write_a_number_and_read_both_forms() {
    assert_eq!(serde_json::to_string(&MessageFlags::EPHEMERAL).unwrap(), "64");
    assert_eq!(serde_json::from_str::<MessageFlags>("64").unwrap(), MessageFlags::EPHEMERAL);
    assert_eq!(serde_json::from_str::<MessageFlags>("\"64\"").unwrap(), MessageFlags::EPHEMERAL);
    assert_eq!(serde_json::to_string(&(Intents::GUILDS | Intents::GUILD_MESSAGES)).unwrap(), "513");
}

#[test]
fn unknown_bits_round_trip_and_survive_operators() {
    let raw = (1u64 << 40) | (1 << 6);
    let flags: MessageFlags = serde_json::from_str(&raw.to_string()).unwrap();
    assert_eq!(flags.bits(), raw);
    assert_eq!(serde_json::to_string(&flags).unwrap(), raw.to_string());
    assert_eq!(format!("{flags:?}"), "MessageFlags(EPHEMERAL | 0x10000000000)");

    let more = flags | MessageFlags::LOADING;
    assert_eq!(more.bits(), raw | (1 << 7));
    assert_eq!((more - MessageFlags::EPHEMERAL).bits(), (1u64 << 40) | (1 << 7));
    assert!(flags.contains(MessageFlags::EPHEMERAL));
}

#[test]
fn bits_above_32_are_not_truncated() {
    // Upstream truncates at 32 bits, ARCHITECTURE 7.6.
    assert_eq!(UserFlags::QUARANTINED.bits(), 1 << 44);
    assert_eq!(UserFlags::COLLABORATOR.bits(), 1 << 50);
    assert_eq!(UserFlags::RESTRICTED_COLLABORATOR.bits(), 1 << 51);
    let flags: UserFlags = serde_json::from_str("17592186044416").unwrap();
    assert_eq!(flags, UserFlags::QUARANTINED);

    let top: Permissions = serde_json::from_str("\"18446744073709551615\"").unwrap();
    assert_eq!(top.bits(), u64::MAX);
    assert_eq!(serde_json::to_string(&top).unwrap(), "\"18446744073709551615\"");
}

#[test]
fn bad_flag_input_is_an_error() {
    assert!(serde_json::from_str::<Permissions>("\"abc\"").is_err());
    assert!(serde_json::from_str::<Permissions>("-1").is_err());
    assert!(serde_json::from_str::<Permissions>("\"18446744073709551616\"").is_err());
}

#[test]
fn deprecated_flag_names_are_the_same_bit_and_debug_prints_the_current_one() {
    assert_eq!(Intents::GUILD_BANS, Intents::GUILD_MODERATION);
    assert_eq!(Intents::GUILD_EMOJIS_AND_STICKERS, Intents::GUILD_EXPRESSIONS);
    assert_eq!(Permissions::MANAGE_EMOJIS_AND_STICKERS, Permissions::MANAGE_GUILD_EXPRESSIONS);
    assert_eq!(format!("{:?}", Intents::GUILD_BANS), "Intents(GUILD_MODERATION)");
    assert_eq!(format!("{:?}", Permissions::MANAGE_EMOJIS_AND_STICKERS), "Permissions(MANAGE_GUILD_EXPRESSIONS)");
}

#[test]
fn intents_of_the_poc_bot() {
    let intents = Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT;
    assert_eq!(intents.bits(), 1 | (1 << 9) | (1 << 15));
}

// ------------------------------------------------------------------------------------------------
// REST error body
// ------------------------------------------------------------------------------------------------

#[test]
fn error_body_form_shape_keeps_errors_raw() {
    // Vector from discord.js packages/rest/__tests__/DiscordAPIError.test.ts (Apache-2.0).
    let text = r#"{"code":50035,"errors":{"username":{"_errors":[{"code":"BASE_TYPE_BAD_LENGTH","message":"Must be between 2 and 32 in length."}]}},"message":"Invalid Form Body"}"#;
    let body: RestErrorBody = serde_json::from_str(text).unwrap();
    assert_eq!(body.code, Some(JsonErrorCode::INVALID_FORM_BODY_OR_CONTENT_TYPE));
    assert_eq!(body.message, "Invalid Form Body");
    assert_eq!(
        body.errors.unwrap().get(),
        r#"{"username":{"_errors":[{"code":"BASE_TYPE_BAD_LENGTH","message":"Must be between 2 and 32 in length."}]}}"#
    );
    assert_eq!(body.error, None);
    assert!(!body.global);
}

#[test]
fn error_body_code_zero_is_not_absence() {
    // Vector from discord.js packages/rest/__tests__/DiscordAPIError.test.ts (Apache-2.0).
    let body: RestErrorBody = serde_json::from_str(r#"{"message":"401: Unauthorized","code":0}"#).unwrap();
    assert_eq!(body.code, Some(JsonErrorCode::GENERAL_ERROR));
    assert_eq!(body.message, "401: Unauthorized");
}

#[test]
fn error_body_oauth_shape() {
    // Vector from discord.js packages/rest/__tests__/DiscordAPIError.test.ts (Apache-2.0).
    let text = r#"{"error":"invalid_request","error_description":"Invalid \"code\" in request."}"#;
    let body: RestErrorBody = serde_json::from_str(text).unwrap();
    assert_eq!(body.code, None);
    assert_eq!(body.message, "");
    assert_eq!(body.error.as_deref(), Some("invalid_request"));
    assert_eq!(body.error_description.as_deref(), Some("Invalid \"code\" in request."));
}

#[test]
fn error_body_rate_limit_shape() {
    let text = r#"{"message":"You are being rate limited.","retry_after":64.57,"global":true,"code":20028}"#;
    let body: RestErrorBody = serde_json::from_str(text).unwrap();
    assert_eq!(body.retry_after, Some(64.57));
    assert!(body.global);
    assert_eq!(body.code, Some(JsonErrorCode::CHANNEL_WRITE_RATE_LIMIT));

    let limit: RestRateLimit = serde_json::from_str(text).unwrap();
    assert!(limit.global);
    assert_eq!(limit.retry_after, 64.57);
    assert_eq!(limit.code, Some(JsonErrorCode::CHANNEL_WRITE_RATE_LIMIT));

    // A whole number of seconds also reads.
    let whole: RestRateLimit = serde_json::from_str(r#"{"message":"m","retry_after":2,"global":false}"#).unwrap();
    assert_eq!(whole.retry_after, 2.0);
    assert_eq!(whole.code, None);
}

#[test]
fn rate_limit_writes_every_field_discord_always_sends() {
    let limit = RestRateLimit { code: None, global: false, message: "slow".to_owned(), retry_after: 1.5 };
    assert_eq!(serde_json::to_value(&limit).unwrap(), json!({"global": false, "message": "slow", "retry_after": 1.5}));
}

#[test]
fn error_body_tolerates_nulls_missing_fields_and_new_fields() {
    let body: RestErrorBody = serde_json::from_str("{}").unwrap();
    assert_eq!(body.code, None);
    assert_eq!(body.message, "");
    assert!(body.errors.is_none());

    let body: RestErrorBody = serde_json::from_str(
        r#"{"code":null,"message":null,"errors":null,"error":null,"error_description":null,"retry_after":null,"global":null,"new_field":{"x":1}}"#,
    )
    .unwrap();
    assert_eq!(body.code, None);
    assert_eq!(body.message, "");
    assert!(body.errors.is_none());
    assert!(!body.global);
}

#[test]
fn error_body_with_an_odd_code_keeps_the_message() {
    for code in [r#""abc""#, "-1", "1.5", "99999999999", "{}", "[]"] {
        let text = format!(r#"{{"code":{code},"message":"kept"}}"#);
        let body: RestErrorBody = serde_json::from_str(&text).unwrap();
        assert_eq!(body.code, None, "{code}");
        assert_eq!(body.message, "kept", "{code}");
    }
}

#[test]
fn error_body_keeps_an_unknown_code() {
    let body: RestErrorBody = serde_json::from_str(r#"{"code":999999,"message":"m"}"#).unwrap();
    assert_eq!(body.code, Some(JsonErrorCode(999_999)));
}

#[test]
fn error_body_writes_only_what_it_holds() {
    let body = RestErrorBody {
        code: Some(JsonErrorCode::UNKNOWN_CHANNEL),
        message: "Unknown Channel".to_owned(),
        ..RestErrorBody::default()
    };
    assert_eq!(serde_json::to_value(&body).unwrap(), json!({"code": 10003, "message": "Unknown Channel"}));
    let back: RestErrorBody = serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap();
    assert_eq!(back.code, Some(JsonErrorCode::UNKNOWN_CHANNEL));
}

// ------------------------------------------------------------------------------------------------
// Routes, checked against `Routes` in rest/v10/index.ts and how packages/core calls it
// ------------------------------------------------------------------------------------------------

#[test]
fn routes_match_upstream() {
    let channel = Id::new(1);
    let message = Id::new(2);
    let user = Id::new(3);
    let guild = Id::new(4);
    let application = Id::new(5);
    let command = Id::new(6);
    let interaction = Id::new(7);

    assert_eq!(routes::API_VERSION, "10");
    assert_eq!(routes::channel(channel), "/channels/1");
    assert_eq!(routes::channel_messages(channel), "/channels/1/messages");
    assert_eq!(routes::channel_message(channel, message), "/channels/1/messages/2");
    assert_eq!(routes::channel_typing(channel), "/channels/1/typing");
    assert_eq!(routes::user(user), "/users/3");
    assert_eq!(routes::gateway_bot(), "/gateway/bot");
    assert_eq!(routes::guild(guild), "/guilds/4");
    assert_eq!(routes::guild_channels(guild), "/guilds/4/channels");
    assert_eq!(routes::guild_member(guild, user), "/guilds/4/members/3");
    assert_eq!(routes::interaction_callback(interaction, "tok"), "/interactions/7/tok/callback");
    assert_eq!(routes::webhook(application, "tok"), "/webhooks/5/tok");
    assert_eq!(routes::webhook_message(application, "tok", message), "/webhooks/5/tok/messages/2");
    assert_eq!(routes::application_commands(application), "/applications/5/commands");
    assert_eq!(routes::application_command(application, command), "/applications/5/commands/6");
    assert_eq!(routes::application_guild_commands(application, guild), "/applications/5/guilds/4/commands");
    assert_eq!(routes::application_guild_command(application, guild, command), "/applications/5/guilds/4/commands/6");
}

#[test]
fn at_me_and_at_original_are_spelled_as_in_real_traffic() {
    // core calls `Routes.user('@me')` and passes '@original' explicitly, so the escaping wrapper
    // encodes them. `.../reactions/{emoji}/@me` is a literal in the route table, and stays one.
    assert_eq!(routes::current_user(), "/users/%40me");
    assert_eq!(routes::webhook_original_message(Id::new(5), "tok"), "/webhooks/5/tok/messages/%40original");
    assert_eq!(
        routes::channel_message_own_reaction(Id::new(1), Id::new(2), "%F0%9F%91%8D"),
        "/channels/1/messages/2/reactions/%F0%9F%91%8D/@me"
    );
}

// ------------------------------------------------------------------------------------------------
// REST results and queries
// ------------------------------------------------------------------------------------------------

#[test]
fn gateway_bot_result_reads_the_documented_shape() {
    let text = r#"{"url":"wss://gateway.discord.gg/","shards":9,"session_start_limit":{"total":1000,"remaining":999,"reset_after":14400000,"max_concurrency":1},"new":true}"#;
    let info: GetGatewayBotResult = serde_json::from_str(text).unwrap();
    assert_eq!(info.url, "wss://gateway.discord.gg/");
    assert_eq!(info.shards, 9);
    assert_eq!(info.session_start_limit.total, 1000);
    assert_eq!(info.session_start_limit.remaining, 999);
    assert_eq!(info.session_start_limit.reset_after, 14_400_000);
    assert_eq!(info.session_start_limit.max_concurrency, 1);
}

#[test]
fn gateway_bot_result_needs_the_nested_limit_but_defaults_primitives() {
    assert!(serde_json::from_str::<GetGatewayBotResult>(r#"{"url":"x","shards":1}"#).is_err());
    let info: GetGatewayBotResult = serde_json::from_str(r#"{"session_start_limit":{}}"#).unwrap();
    assert_eq!(info.url, "");
    assert_eq!(info.shards, 0);
    assert_eq!(info.session_start_limit.max_concurrency, 0);
}

#[test]
fn queries_leave_out_unset_values() {
    assert_eq!(serde_json::to_value(GetGuildQuery::default()).unwrap(), json!({}));
    assert_eq!(serde_json::to_value(GetGuildQuery { with_counts: Some(true) }).unwrap(), json!({"with_counts": true}));
    assert_eq!(serde_json::to_value(GetApplicationCommandsQuery::default()).unwrap(), json!({}));
    assert_eq!(
        serde_json::to_value(GetApplicationCommandsQuery { with_localizations: Some(false) }).unwrap(),
        json!({"with_localizations": false})
    );
    assert_eq!(serde_json::to_value(PostInteractionCallbackQuery::default()).unwrap(), json!({}));
    assert_eq!(
        serde_json::to_value(PostInteractionCallbackQuery { with_response: Some(true) }).unwrap(),
        json!({"with_response": true})
    );
}

// ------------------------------------------------------------------------------------------------
// Upstream values. `$count` is the number of members upstream has, duplicates included.
// ------------------------------------------------------------------------------------------------

macro_rules! check_int {
    ($ty:ident, $count:literal; $($k:ident = $v:literal,)*) => {{
        let table: &[(&str, $ty, u64)] = &[$((stringify!($k), $ty::$k, $v)),*];
        assert_eq!(table.len(), $count, stringify!($ty));
        for (name, value, wire) in table {
            assert_eq!(u64::from(value.get()), *wire, "{}::{}", stringify!($ty), name);
            assert!(value.name().is_some(), "{}::{} has no name", stringify!($ty), name);
            assert_eq!(serde_json::to_string(value).unwrap(), wire.to_string());
            assert_eq!(serde_json::from_str::<$ty>(&wire.to_string()).unwrap(), *value);
        }
    }};
}

macro_rules! check_str {
    ($ty:ident, $count:literal; $($k:ident = $v:literal,)*) => {{
        let table: &[($ty, &str)] = &[$(($ty::$k, $v)),*];
        assert_eq!(table.len(), $count, stringify!($ty));
        for (value, wire) in table {
            assert_eq!(value.as_str(), *wire);
            assert_eq!(&$ty::from(*wire), value);
            assert!(!matches!(value, $ty::Unknown(_)), "{}", wire);
            assert_eq!(serde_json::from_str::<$ty>(&format!("\"{wire}\"")).unwrap(), *value);
        }
    }};
}

macro_rules! check_flags {
    ($ty:ident, $count:literal; $($k:ident = $v:literal,)*) => {{
        let table: &[(&str, $ty, u64)] = &[$((stringify!($k), $ty::$k, $v)),*];
        assert_eq!(table.len(), $count, stringify!($ty));
        for (name, value, bits) in table {
            assert_eq!(value.bits(), *bits, "{}::{}", stringify!($ty), name);
            // No named bit is an unknown bit in `Debug`.
            assert!(!format!("{value:?}").contains("0x"), "{}::{}", stringify!($ty), name);
            assert_eq!(serde_json::from_str::<$ty>(&bits.to_string()).unwrap(), *value);
        }
    }};
}

#[test]
fn int_enums_match_upstream() {
    check_int! { ChannelType, 17;
        GUILD_TEXT = 0,
        DM = 1,
        GUILD_VOICE = 2,
        GROUP_DM = 3,
        GUILD_CATEGORY = 4,
        GUILD_ANNOUNCEMENT = 5,
        ANNOUNCEMENT_THREAD = 10,
        PUBLIC_THREAD = 11,
        PRIVATE_THREAD = 12,
        GUILD_STAGE_VOICE = 13,
        GUILD_DIRECTORY = 14,
        GUILD_FORUM = 15,
        GUILD_MEDIA = 16,
        GUILD_NEWS = 5,
        GUILD_NEWS_THREAD = 10,
        GUILD_PUBLIC_THREAD = 11,
        GUILD_PRIVATE_THREAD = 12,
    }
    check_int! { MessageType, 38;
        DEFAULT = 0,
        RECIPIENT_ADD = 1,
        RECIPIENT_REMOVE = 2,
        CALL = 3,
        CHANNEL_NAME_CHANGE = 4,
        CHANNEL_ICON_CHANGE = 5,
        CHANNEL_PINNED_MESSAGE = 6,
        USER_JOIN = 7,
        GUILD_BOOST = 8,
        GUILD_BOOST_TIER1 = 9,
        GUILD_BOOST_TIER2 = 10,
        GUILD_BOOST_TIER3 = 11,
        CHANNEL_FOLLOW_ADD = 12,
        GUILD_DISCOVERY_DISQUALIFIED = 14,
        GUILD_DISCOVERY_REQUALIFIED = 15,
        GUILD_DISCOVERY_GRACE_PERIOD_INITIAL_WARNING = 16,
        GUILD_DISCOVERY_GRACE_PERIOD_FINAL_WARNING = 17,
        THREAD_CREATED = 18,
        REPLY = 19,
        CHAT_INPUT_COMMAND = 20,
        THREAD_STARTER_MESSAGE = 21,
        GUILD_INVITE_REMINDER = 22,
        CONTEXT_MENU_COMMAND = 23,
        AUTO_MODERATION_ACTION = 24,
        ROLE_SUBSCRIPTION_PURCHASE = 25,
        INTERACTION_PREMIUM_UPSELL = 26,
        STAGE_START = 27,
        STAGE_END = 28,
        STAGE_SPEAKER = 29,
        STAGE_RAISE_HAND = 30,
        STAGE_TOPIC = 31,
        GUILD_APPLICATION_PREMIUM_SUBSCRIPTION = 32,
        GUILD_INCIDENT_ALERT_MODE_ENABLED = 36,
        GUILD_INCIDENT_ALERT_MODE_DISABLED = 37,
        GUILD_INCIDENT_REPORT_RAID = 38,
        GUILD_INCIDENT_REPORT_FALSE_ALARM = 39,
        PURCHASE_NOTIFICATION = 44,
        POLL_RESULT = 46,
    }
    check_int! { MessageReferenceType, 2;
        DEFAULT = 0,
        FORWARD = 1,
    }
    check_int! { InteractionType, 5;
        PING = 1,
        APPLICATION_COMMAND = 2,
        MESSAGE_COMPONENT = 3,
        APPLICATION_COMMAND_AUTOCOMPLETE = 4,
        MODAL_SUBMIT = 5,
    }
    check_int! { InteractionResponseType, 9;
        PONG = 1,
        CHANNEL_MESSAGE_WITH_SOURCE = 4,
        DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE = 5,
        DEFERRED_MESSAGE_UPDATE = 6,
        UPDATE_MESSAGE = 7,
        APPLICATION_COMMAND_AUTOCOMPLETE_RESULT = 8,
        MODAL = 9,
        PREMIUM_REQUIRED = 10,
        LAUNCH_ACTIVITY = 12,
    }
    check_int! { ComponentType, 22;
        ACTION_ROW = 1,
        BUTTON = 2,
        STRING_SELECT = 3,
        TEXT_INPUT = 4,
        USER_SELECT = 5,
        ROLE_SELECT = 6,
        MENTIONABLE_SELECT = 7,
        CHANNEL_SELECT = 8,
        SECTION = 9,
        TEXT_DISPLAY = 10,
        THUMBNAIL = 11,
        MEDIA_GALLERY = 12,
        FILE = 13,
        SEPARATOR = 14,
        CONTENT_INVENTORY_ENTRY = 16,
        CONTAINER = 17,
        LABEL = 18,
        FILE_UPLOAD = 19,
        RADIO_GROUP = 21,
        CHECKBOX_GROUP = 22,
        CHECKBOX = 23,
        SELECT_MENU = 3,
    }
    check_int! { ButtonStyle, 6;
        PRIMARY = 1,
        SECONDARY = 2,
        SUCCESS = 3,
        DANGER = 4,
        LINK = 5,
        PREMIUM = 6,
    }
    check_int! { TextInputStyle, 2;
        SHORT = 1,
        PARAGRAPH = 2,
    }
    check_int! { ApplicationCommandType, 4;
        CHAT_INPUT = 1,
        USER = 2,
        MESSAGE = 3,
        PRIMARY_ENTRY_POINT = 4,
    }
    check_int! { ApplicationCommandOptionType, 11;
        SUBCOMMAND = 1,
        SUBCOMMAND_GROUP = 2,
        STRING = 3,
        INTEGER = 4,
        BOOLEAN = 5,
        USER = 6,
        CHANNEL = 7,
        ROLE = 8,
        MENTIONABLE = 9,
        NUMBER = 10,
        ATTACHMENT = 11,
    }
    check_int! { JsonErrorCode, 246;
        GENERAL_ERROR = 0,
        UNKNOWN_ACCOUNT = 10001,
        UNKNOWN_APPLICATION = 10002,
        UNKNOWN_CHANNEL = 10003,
        UNKNOWN_GUILD = 10004,
        UNKNOWN_INTEGRATION = 10005,
        UNKNOWN_INVITE = 10006,
        UNKNOWN_MEMBER = 10007,
        UNKNOWN_MESSAGE = 10008,
        UNKNOWN_PERMISSION_OVERWRITE = 10009,
        UNKNOWN_PROVIDER = 10010,
        UNKNOWN_ROLE = 10011,
        UNKNOWN_TOKEN = 10012,
        UNKNOWN_USER = 10013,
        UNKNOWN_EMOJI = 10014,
        UNKNOWN_WEBHOOK = 10015,
        UNKNOWN_WEBHOOK_SERVICE = 10016,
        UNKNOWN_SESSION = 10020,
        UNKNOWN_ASSET = 10021,
        UNKNOWN_BAN = 10026,
        UNKNOWN_SKU = 10027,
        UNKNOWN_STORE_LISTING = 10028,
        UNKNOWN_ENTITLEMENT = 10029,
        UNKNOWN_BUILD = 10030,
        UNKNOWN_LOBBY = 10031,
        UNKNOWN_BRANCH = 10032,
        UNKNOWN_STORE_DIRECTORY_LAYOUT = 10033,
        UNKNOWN_REDISTRIBUTABLE = 10036,
        UNKNOWN_GIFT_CODE = 10038,
        UNKNOWN_STREAM = 10049,
        UNKNOWN_PREMIUM_SERVER_SUBSCRIBE_COOLDOWN = 10050,
        UNKNOWN_GUILD_TEMPLATE = 10057,
        UNKNOWN_DISCOVERABLE_SERVER_CATEGORY = 10059,
        UNKNOWN_STICKER = 10060,
        UNKNOWN_STICKER_PACK = 10061,
        UNKNOWN_INTERACTION = 10062,
        UNKNOWN_APPLICATION_COMMAND = 10063,
        UNKNOWN_VOICE_STATE = 10065,
        UNKNOWN_APPLICATION_COMMAND_PERMISSIONS = 10066,
        UNKNOWN_STAGE_INSTANCE = 10067,
        UNKNOWN_GUILD_MEMBER_VERIFICATION_FORM = 10068,
        UNKNOWN_GUILD_WELCOME_SCREEN = 10069,
        UNKNOWN_GUILD_SCHEDULED_EVENT = 10070,
        UNKNOWN_GUILD_SCHEDULED_EVENT_USER = 10071,
        UNKNOWN_TAG = 10087,
        UNKNOWN_SOUND = 10097,
        UNKNOWN_INVITE_TARGET_USERS_JOB = 10124,
        UNKNOWN_INVITE_TARGET_USERS = 10129,
        BOTS_CANNOT_USE_THIS_ENDPOINT = 20001,
        ONLY_BOTS_CAN_USE_THIS_ENDPOINT = 20002,
        EXPLICIT_CONTENT_CANNOT_BE_SENT_TO_THE_DESIRED_RECIPIENT = 20009,
        NOT_AUTHORIZED_TO_PERFORM_THIS_ACTION_ON_THIS_APPLICATION = 20012,
        THIS_ACTION_REQUIRES_A_PREMIUM_SUBSCRIPTION = 20015,
        ACTION_CANNOT_BE_PERFORMED_DUE_TO_SLOWMODE_RATE_LIMIT = 20016,
        THE_MAZE_ISNT_MEANT_FOR_YOU = 20017,
        ONLY_THE_OWNER_OF_THIS_ACCOUNT_CAN_PERFORM_THIS_ACTION = 20018,
        ANNOUNCEMENT_EDIT_LIMIT_EXCEEDED = 20022,
        UNDER_MINIMUM_AGE = 20024,
        CHANNEL_WRITE_RATE_LIMIT = 20028,
        CHANNEL_SEND_RATE_LIMIT = 20028,
        SERVER_WRITE_RATE_LIMIT = 20029,
        SERVER_SEND_RATE_LIMIT = 20029,
        STAGE_TOPIC_SERVER_NAME_SERVER_DESCRIPTION_OR_CHANNEL_NAMES_CONTAIN_DISALLOWED_WORDS = 20031,
        GUILD_PREMIUM_SUBSCRIPTION_LEVEL_TOO_LOW = 20035,
        MAXIMUM_NUMBER_OF_GUILDS_REACHED = 30001,
        MAXIMUM_NUMBER_OF_FRIENDS_REACHED = 30002,
        MAXIMUM_NUMBER_OF_PINS_REACHED_FOR_THE_CHANNEL = 30003,
        MAXIMUM_NUMBER_OF_RECIPIENTS_REACHED = 30004,
        MAXIMUM_NUMBER_OF_GUILD_ROLES_REACHED = 30005,
        MAXIMUM_NUMBER_OF_WEBHOOKS_REACHED = 30007,
        MAXIMUM_NUMBER_OF_EMOJIS_REACHED = 30008,
        MAXIMUM_NUMBER_OF_REACTIONS_REACHED = 30010,
        MAXIMUM_NUMBER_OF_GROUP_DMS_REACHED = 30011,
        MAXIMUM_NUMBER_OF_GUILD_CHANNELS_REACHED = 30013,
        MAXIMUM_NUMBER_OF_ATTACHMENTS_IN_A_MESSAGE_REACHED = 30015,
        MAXIMUM_NUMBER_OF_INVITES_REACHED = 30016,
        MAXIMUM_NUMBER_OF_ANIMATED_EMOJIS_REACHED = 30018,
        MAXIMUM_NUMBER_OF_SERVER_MEMBERS_REACHED = 30019,
        MAXIMUM_NUMBER_OF_SERVER_CATEGORIES_REACHED = 30030,
        GUILD_ALREADY_HAS_TEMPLATE = 30031,
        MAXIMUM_NUMBER_OF_APPLICATION_COMMANDS_REACHED = 30032,
        MAXIMUM_THREAD_PARTICIPANTS_REACHED = 30033,
        MAXIMUM_DAILY_APPLICATION_COMMAND_CREATES_REACHED = 30034,
        MAXIMUM_NUMBER_OF_NON_GUILD_MEMBER_BANS_HAS_BEEN_EXCEEDED = 30035,
        MAXIMUM_NUMBER_OF_BAN_FETCHES_HAS_BEEN_REACHED = 30037,
        MAXIMUM_NUMBER_OF_UNCOMPLETED_GUILD_SCHEDULED_EVENTS_REACHED = 30038,
        MAXIMUM_NUMBER_OF_STICKERS_REACHED = 30039,
        MAXIMUM_NUMBER_OF_PRUNE_REQUESTS_HAS_BEEN_REACHED = 30040,
        MAXIMUM_NUMBER_OF_GUILD_WIDGET_SETTINGS_UPDATES_HAS_BEEN_REACHED = 30042,
        MAXIMUM_NUMBER_OF_SOUNDBOARD_SOUNDS_REACHED = 30045,
        MAXIMUM_NUMBER_OF_EDITS_TO_MESSAGES_OLDER_THAN_ONE_HOUR_REACHED = 30046,
        MAXIMUM_NUMBER_OF_PINNED_THREADS_IN_FORUM_HAS_BEEN_REACHED = 30047,
        MAXIMUM_NUMBER_OF_TAGS_IN_FORUM_HAS_BEEN_REACHED = 30048,
        BITRATE_IS_TOO_HIGH_FOR_CHANNEL_OF_THIS_TYPE = 30052,
        MAXIMUM_NUMBER_OF_PREMIUM_EMOJIS_REACHED = 30056,
        MAXIMUM_NUMBER_OF_WEBHOOKS_PER_GUILD_REACHED = 30058,
        MAXIMUM_NUMBER_OF_CHANNEL_PERMISSION_OVERWRITES_REACHED = 30060,
        THE_CHANNELS_FOR_THIS_GUILD_ARE_TOO_LARGE = 30061,
        UNAUTHORIZED = 40001,
        VERIFY_YOUR_ACCOUNT = 40002,
        OPENING_DIRECT_MESSAGES_TOO_FAST = 40003,
        SEND_MESSAGES_HAS_BEEN_TEMPORARILY_DISABLED = 40004,
        REQUEST_ENTITY_TOO_LARGE = 40005,
        FEATURE_TEMPORARILY_DISABLED_SERVER_SIDE = 40006,
        USER_BANNED_FROM_THIS_GUILD = 40007,
        ONLY_ONE_CHANNEL_CAN_HAVE_A_PARENT_ID_MODIFIED_AT_A_TIME = 40009,
        CONNECTION_HAS_BEEN_REVOKED = 40012,
        ONLY_CONSUMABLE_SK_US_CAN_BE_CONSUMED = 40018,
        YOU_CAN_ONLY_DELETE_SANDBOX_ENTITLEMENTS = 40019,
        TARGET_USER_IS_NOT_CONNECTED_TO_VOICE = 40032,
        THIS_MESSAGE_WAS_ALREADY_CROSSPOSTED = 40033,
        APPLICATION_COMMAND_WITH_THAT_NAME_ALREADY_EXISTS = 40041,
        APPLICATION_INTERACTION_FAILED_TO_SEND = 40043,
        CANNOT_SEND_A_MESSAGE_IN_A_FORUM_CHANNEL = 40058,
        INTERACTION_HAS_ALREADY_BEEN_ACKNOWLEDGED = 40060,
        TAG_NAMES_MUST_BE_UNIQUE = 40061,
        SERVICE_RESOURCE_IS_BEING_RATE_LIMITED = 40062,
        THERE_ARE_NO_TAGS_AVAILABLE_THAT_CAN_BE_SET_BY_NON_MODERATORS = 40066,
        TAG_REQUIRED_TO_CREATE_A_FORUM_POST_IN_THIS_CHANNEL = 40067,
        AN_ENTITLEMENT_HAS_ALREADY_BEEN_GRANTED_FOR_THIS_RESOURCE = 40074,
        THIS_INTERACTION_HAS_HIT_THE_MAXIMUM_NUMBER_OF_FOLLOW_UP_MESSAGES = 40094,
        CLOUDFLARE_IS_BLOCKING_YOUR_REQUEST = 40333,
        MISSING_ACCESS = 50001,
        INVALID_ACCOUNT_TYPE = 50002,
        CANNOT_EXECUTE_ACTION_ON_DM_CHANNEL = 50003,
        GUILD_WIDGET_DISABLED = 50004,
        CANNOT_EDIT_MESSAGE_AUTHORED_BY_ANOTHER_USER = 50005,
        CANNOT_SEND_AN_EMPTY_MESSAGE = 50006,
        CANNOT_SEND_MESSAGES_TO_THIS_USER = 50007,
        CANNOT_SEND_MESSAGES_IN_NON_TEXT_CHANNEL = 50008,
        CHANNEL_VERIFICATION_LEVEL_TOO_HIGH_FOR_YOU_TO_GAIN_ACCESS = 50009,
        OAUTH2_APPLICATION_DOES_NOT_HAVE_BOT = 50010,
        OAUTH2_APPLICATION_LIMIT_REACHED = 50011,
        INVALID_OAUTH2_STATE = 50012,
        MISSING_PERMISSIONS = 50013,
        INVALID_TOKEN = 50014,
        NOTE_WAS_TOO_LONG = 50015,
        PROVIDED_TOO_FEW_OR_TOO_MANY_MESSAGES_TO_DELETE = 50016,
        INVALID_MFA_LEVEL = 50017,
        MESSAGE_CAN_ONLY_BE_PINNED_IN_THE_CHANNEL_IT_WAS_SENT_IN = 50019,
        INVITE_CODE_INVALID_OR_TAKEN = 50020,
        CANNOT_EXECUTE_ACTION_ON_SYSTEM_MESSAGE = 50021,
        CANNOT_EXECUTE_ACTION_ON_THIS_CHANNEL_TYPE = 50024,
        INVALID_OAUTH2_ACCESS_TOKEN = 50025,
        MISSING_REQUIRED_OAUTH2_SCOPE = 50026,
        INVALID_WEBHOOK_TOKEN = 50027,
        INVALID_ROLE = 50028,
        INVALID_RECIPIENTS = 50033,
        ONE_OF_THE_MESSAGES_PROVIDED_WAS_TOO_OLD_FOR_BULK_DELETE = 50034,
        INVALID_FORM_BODY_OR_CONTENT_TYPE = 50035,
        INVITE_ACCEPTED_TO_GUILD_WITHOUT_THE_BOT_BEING_IN = 50036,
        INVALID_ACTIVITY_ACTION = 50039,
        INVALID_API_VERSION = 50041,
        FILE_UPLOADED_EXCEEDS_MAXIMUM_SIZE = 50045,
        INVALID_FILE_UPLOADED = 50046,
        CANNOT_SELF_REDEEM_THIS_GIFT = 50054,
        INVALID_GUILD = 50055,
        INVALID_SKU = 50057,
        INVALID_REQUEST_ORIGIN = 50067,
        INVALID_MESSAGE_TYPE = 50068,
        PAYMENT_SOURCE_REQUIRED_TO_REDEEM_GIFT = 50070,
        CANNOT_MODIFY_A_SYSTEM_WEBHOOK = 50073,
        CANNOT_DELETE_CHANNEL_REQUIRED_FOR_COMMUNITY_GUILDS = 50074,
        CANNOT_EDIT_STICKERS_WITHIN_MESSAGE = 50080,
        INVALID_STICKER_SENT = 50081,
        INVALID_ACTION_ON_ARCHIVED_THREAD = 50083,
        INVALID_THREAD_NOTIFICATION_SETTINGS = 50084,
        PARAMETER_EARLIER_THAN_CREATION = 50085,
        COMMUNITY_SERVER_CHANNELS_MUST_BE_TEXT_CHANNELS = 50086,
        THE_ENTITY_TYPE_OF_THE_EVENT_IS_DIFFERENT_FROM_THE_ENTITY_YOU_ARE_TRYING_TO_START_THE_EVENT_FOR = 50091,
        SERVER_NOT_AVAILABLE_IN_YOUR_LOCATION = 50095,
        SERVER_NEEDS_MONETIZATION_ENABLED_TO_PERFORM_THIS_ACTION = 50097,
        SERVER_NEEDS_MORE_BOOSTS_TO_PERFORM_THIS_ACTION = 50101,
        REQUEST_BODY_CONTAINS_INVALID_JSON = 50109,
        PROVIDED_FILE_IS_INVALID = 50110,
        PROVIDED_FILE_TYPE_IS_INVALID = 50123,
        PROVIDED_FILE_DURATION_EXCEEDS_MAXIMUM_LENGTH = 50124,
        OWNER_CANNOT_BE_PENDING_MEMBER = 50131,
        OWNERSHIP_CANNOT_BE_MOVED_TO_A_BOT_USER = 50132,
        FAILED_TO_RESIZE_ASSET_BELOW_THE_MAXIMUM_SIZE = 50138,
        FAILED_TO_RESIZE_ASSET_BELOW_THE_MINIMUM_SIZE = 50138,
        CANNOT_MIX_SUBSCRIPTION_AND_NON_SUBSCRIPTION_ROLES_FOR_AN_EMOJI = 50144,
        CANNOT_CONVERT_BETWEEN_PREMIUM_EMOJI_AND_NORMAL_EMOJI = 50145,
        UPLOADED_FILE_NOT_FOUND = 50146,
        SPECIFIED_EMOJI_IS_INVALID = 50151,
        VOICE_MESSAGES_DO_NOT_SUPPORT_ADDITIONAL_CONTENT = 50159,
        VOICE_MESSAGES_MUST_HAVE_A_SINGLE_AUDIO_ATTACHMENT = 50160,
        VOICE_MESSAGES_MUST_HAVE_SUPPORTING_METADATA = 50161,
        VOICE_MESSAGES_CANNOT_BE_EDITED = 50162,
        CANNOT_DELETE_GUILD_SUBSCRIPTION_INTEGRATION = 50163,
        CANNOT_SEND_VOICE_EFFECT_WHEN_USER_IS_SERVER_MUTED_DEAFENED_OR_SUPPRESSED = 50167,
        YOU_CANNOT_SEND_VOICE_MESSAGES_IN_THIS_CHANNEL = 50173,
        THE_USER_ACCOUNT_MUST_FIRST_BE_VERIFIED = 50178,
        PROVIDED_FILE_DOES_NOT_HAVE_A_VALID_DURATION = 50192,
        CANNOT_SEND_MESSAGES_TO_THIS_USER_DUE_TO_HAVING_NO_MUTUAL_GUILDS = 50278,
        YOU_DO_NOT_HAVE_PERMISSION_TO_SEND_THIS_STICKER = 50600,
        TWO_FACTOR_AUTHENTICATION_IS_REQUIRED = 60003,
        NO_USERS_WITH_DISCORD_TAG_EXIST = 80004,
        REACTION_WAS_BLOCKED = 90001,
        USER_CANNOT_USE_BURST_REACTIONS = 90002,
        INDEX_NOT_YET_AVAILABLE = 110000,
        APPLICATION_NOT_YET_AVAILABLE = 110001,
        API_RESOURCE_OVERLOADED = 130000,
        THE_STAGE_IS_ALREADY_OPEN = 150006,
        CANNOT_REPLY_WITHOUT_PERMISSION_TO_READ_MESSAGE_HISTORY = 160002,
        THREAD_ALREADY_CREATED_FOR_MESSAGE = 160004,
        THREAD_LOCKED = 160005,
        MAXIMUM_ACTIVE_THREADS = 160006,
        MAXIMUM_ACTIVE_ANNOUNCEMENT_THREADS = 160007,
        CANNOT_REFERENCE_A_MESSAGE_WITHOUT_PERMISSION_TO_READ_MESSAGE_HISTORY = 160009,
        NSFW_CHANNEL_MESSAGE_REFERENCE_NOT_ALLOWED = 160010,
        CANNOT_FORWARD_MESSAGE_WITH_UNREADABLE_CONTENT = 160014,
        INVALID_JSON_FOR_UPLOADED_LOTTIE_FILE = 170001,
        UPLOADED_LOTTIES_CANNOT_CONTAIN_RASTERIZED_IMAGES = 170002,
        STICKER_MAXIMUM_FRAMERATE_EXCEEDED = 170003,
        STICKER_FRAME_COUNT_EXCEEDS_MAXIMUM_OF_1000_FRAMES = 170004,
        LOTTIE_ANIMATION_MAXIMUM_DIMENSIONS_EXCEEDED = 170005,
        STICKER_FRAMERATE_IS_TOO_SMALL_OR_TOO_LARGE = 170006,
        STICKER_ANIMATION_DURATION_EXCEEDS_MAXIMUM_OF_5_SECONDS = 170007,
        CANNOT_UPDATE_A_FINISHED_EVENT = 180000,
        FAILED_TO_CREATE_STAGE_NEEDED_FOR_STAGE_EVENT = 180002,
        MESSAGE_WAS_BLOCKED_BY_AUTOMATIC_MODERATION = 200000,
        TITLE_WAS_BLOCKED_BY_AUTOMATIC_MODERATION = 200001,
        WEBHOOKS_POSTED_TO_FORUM_CHANNELS_MUST_HAVE_A_THREAD_NAME_OR_THREAD_ID = 220001,
        WEBHOOKS_POSTED_TO_FORUM_CHANNELS_CANNOT_HAVE_BOTH_A_THREAD_NAME_AND_THREAD_ID = 220002,
        WEBHOOKS_CAN_ONLY_CREATE_THREADS_IN_FORUM_CHANNELS = 220003,
        WEBHOOK_SERVICES_CANNOT_BE_USED_IN_FORUM_CHANNELS = 220004,
        MESSAGE_BLOCKED_BY_HARMFUL_LINKS_FILTER = 240000,
        ACCESS_TO_JOINING_NEW_SERVERS_HAS_BEEN_LIMITED_FOR_THE_USER = 340015,
        CANNOT_ENABLE_ONBOARDING_REQUIREMENTS_ARE_NOT_MET = 350000,
        CANNOT_UPDATE_ONBOARDING_WHILE_BELOW_REQUIREMENTS = 350001,
        ACCESS_TO_FILE_UPLOADS_HAS_BEEN_LIMITED_FOR_THIS_GUILD = 400001,
        FAILED_TO_BAN_USERS = 500000,
        POLL_VOTING_BLOCKED = 520000,
        POLL_EXPIRED = 520001,
        INVALID_CHANNEL_TYPE_FOR_POLL_CREATION = 520002,
        CANNOT_EDIT_A_POLL_MESSAGE = 520003,
        CANNOT_USE_AN_EMOJI_INCLUDED_WITH_THE_POLL = 520004,
        CANNOT_EXPIRE_A_NON_POLL_MESSAGE = 520006,
        PROVISIONAL_ACCOUNTS_PERMISSION_NOT_GRANTED = 530000,
        ID_TOKEN_JWT_EXPIRED = 530001,
        ID_TOKEN_JWT_ISSUER_MISMATCH = 530002,
        ID_TOKEN_JWT_AUDIENCE_MISMATCH = 530003,
        ID_TOKEN_JWT_ISSUED_TOO_LONG_AGO = 530004,
        FAILED_TO_GENERATE_UNIQUE_USERNAME = 530006,
        INVALID_CLIENT_SECRET = 530007,
    }
    check_int! { GatewayOpcode, 13;
        DISPATCH = 0,
        HEARTBEAT = 1,
        IDENTIFY = 2,
        PRESENCE_UPDATE = 3,
        VOICE_STATE_UPDATE = 4,
        RESUME = 6,
        RECONNECT = 7,
        REQUEST_GUILD_MEMBERS = 8,
        INVALID_SESSION = 9,
        HELLO = 10,
        HEARTBEAT_ACK = 11,
        REQUEST_SOUNDBOARD_SOUNDS = 31,
        REQUEST_CHANNEL_INFO = 43,
    }
    check_int! { GatewayCloseCode, 14;
        UNKNOWN_ERROR = 4000,
        UNKNOWN_OPCODE = 4001,
        DECODE_ERROR = 4002,
        NOT_AUTHENTICATED = 4003,
        AUTHENTICATION_FAILED = 4004,
        ALREADY_AUTHENTICATED = 4005,
        INVALID_SEQ = 4007,
        RATE_LIMITED = 4008,
        SESSION_TIMED_OUT = 4009,
        INVALID_SHARD = 4010,
        SHARDING_REQUIRED = 4011,
        INVALID_API_VERSION = 4012,
        INVALID_INTENTS = 4013,
        DISALLOWED_INTENTS = 4014,
    }
}

#[test]
fn str_enums_match_upstream() {
    check_str! { GuildFeature, 40;
        AnimatedBanner = "ANIMATED_BANNER",
        AnimatedIcon = "ANIMATED_ICON",
        ApplicationCommandPermissionsV2 = "APPLICATION_COMMAND_PERMISSIONS_V2",
        AutoModeration = "AUTO_MODERATION",
        Banner = "BANNER",
        Community = "COMMUNITY",
        CreatorMonetizableProvisional = "CREATOR_MONETIZABLE_PROVISIONAL",
        CreatorStorePage = "CREATOR_STORE_PAGE",
        DeveloperSupportServer = "DEVELOPER_SUPPORT_SERVER",
        Discoverable = "DISCOVERABLE",
        EnhancedRoleColors = "ENHANCED_ROLE_COLORS",
        Featurable = "FEATURABLE",
        GuestsEnabled = "GUESTS_ENABLED",
        GuildTags = "GUILD_TAGS",
        HasDirectoryEntry = "HAS_DIRECTORY_ENTRY",
        Hub = "HUB",
        InvitesDisabled = "INVITES_DISABLED",
        InviteSplash = "INVITE_SPLASH",
        LinkedToHub = "LINKED_TO_HUB",
        MemberVerificationGateEnabled = "MEMBER_VERIFICATION_GATE_ENABLED",
        MonetizationEnabled = "MONETIZATION_ENABLED",
        MoreSoundboard = "MORE_SOUNDBOARD",
        MoreStickers = "MORE_STICKERS",
        News = "NEWS",
        Partnered = "PARTNERED",
        PinPermissionMigrationComplete = "PIN_PERMISSION_MIGRATION_COMPLETE",
        PreviewEnabled = "PREVIEW_ENABLED",
        PrivateThreads = "PRIVATE_THREADS",
        PruneRequiresAdmin = "PRUNE_REQUIRES_ADMIN",
        RaidAlertsDisabled = "RAID_ALERTS_DISABLED",
        RelayEnabled = "RELAY_ENABLED",
        RoleIcons = "ROLE_ICONS",
        RoleSubscriptionsAvailableForPurchase = "ROLE_SUBSCRIPTIONS_AVAILABLE_FOR_PURCHASE",
        RoleSubscriptionsEnabled = "ROLE_SUBSCRIPTIONS_ENABLED",
        Soundboard = "SOUNDBOARD",
        TicketedEventsEnabled = "TICKETED_EVENTS_ENABLED",
        VanityURL = "VANITY_URL",
        Verified = "VERIFIED",
        VIPRegions = "VIP_REGIONS",
        WelcomeScreenEnabled = "WELCOME_SCREEN_ENABLED",
    }
    check_str! { AllowedMentionType, 3;
        Everyone = "everyone",
        Roles = "roles",
        Users = "users",
    }
    check_str! { Locale, 32;
        Indonesian = "id",
        EnglishUS = "en-US",
        EnglishGB = "en-GB",
        Bulgarian = "bg",
        ChineseCN = "zh-CN",
        ChineseTW = "zh-TW",
        Croatian = "hr",
        Czech = "cs",
        Danish = "da",
        Dutch = "nl",
        Finnish = "fi",
        French = "fr",
        German = "de",
        Greek = "el",
        Hindi = "hi",
        Hungarian = "hu",
        Italian = "it",
        Japanese = "ja",
        Korean = "ko",
        Lithuanian = "lt",
        Norwegian = "no",
        Polish = "pl",
        PortugueseBR = "pt-BR",
        Romanian = "ro",
        Russian = "ru",
        SpanishES = "es-ES",
        SpanishLATAM = "es-419",
        Swedish = "sv-SE",
        Thai = "th",
        Turkish = "tr",
        Ukrainian = "uk",
        Vietnamese = "vi",
    }
}

#[test]
fn flags_match_upstream() {
    check_flags! { Intents, 23;
        GUILDS = 1,
        GUILD_MEMBERS = 2,
        GUILD_MODERATION = 4,
        GUILD_BANS = 4,
        GUILD_EXPRESSIONS = 8,
        GUILD_EMOJIS_AND_STICKERS = 8,
        GUILD_INTEGRATIONS = 16,
        GUILD_WEBHOOKS = 32,
        GUILD_INVITES = 64,
        GUILD_VOICE_STATES = 128,
        GUILD_PRESENCES = 256,
        GUILD_MESSAGES = 512,
        GUILD_MESSAGE_REACTIONS = 1024,
        GUILD_MESSAGE_TYPING = 2048,
        DIRECT_MESSAGES = 4096,
        DIRECT_MESSAGE_REACTIONS = 8192,
        DIRECT_MESSAGE_TYPING = 16384,
        MESSAGE_CONTENT = 32768,
        GUILD_SCHEDULED_EVENTS = 65536,
        AUTO_MODERATION_CONFIGURATION = 1048576,
        AUTO_MODERATION_EXECUTION = 2097152,
        GUILD_MESSAGE_POLLS = 16777216,
        DIRECT_MESSAGE_POLLS = 33554432,
    }
    check_flags! { GatewayCapabilityBits, 1;
        CHANNEL_OBFUSCATION = 32768,
    }
    check_flags! { Permissions, 53;
        CREATE_INSTANT_INVITE = 1,
        KICK_MEMBERS = 2,
        BAN_MEMBERS = 4,
        ADMINISTRATOR = 8,
        MANAGE_CHANNELS = 16,
        MANAGE_GUILD = 32,
        ADD_REACTIONS = 64,
        VIEW_AUDIT_LOG = 128,
        PRIORITY_SPEAKER = 256,
        STREAM = 512,
        VIEW_CHANNEL = 1024,
        SEND_MESSAGES = 2048,
        SEND_TTS_MESSAGES = 4096,
        MANAGE_MESSAGES = 8192,
        EMBED_LINKS = 16384,
        ATTACH_FILES = 32768,
        READ_MESSAGE_HISTORY = 65536,
        MENTION_EVERYONE = 131072,
        USE_EXTERNAL_EMOJIS = 262144,
        VIEW_GUILD_INSIGHTS = 524288,
        CONNECT = 1048576,
        SPEAK = 2097152,
        MUTE_MEMBERS = 4194304,
        DEAFEN_MEMBERS = 8388608,
        MOVE_MEMBERS = 16777216,
        USE_VAD = 33554432,
        CHANGE_NICKNAME = 67108864,
        MANAGE_NICKNAMES = 134217728,
        MANAGE_ROLES = 268435456,
        MANAGE_WEBHOOKS = 536870912,
        MANAGE_EMOJIS_AND_STICKERS = 1073741824,
        MANAGE_GUILD_EXPRESSIONS = 1073741824,
        USE_APPLICATION_COMMANDS = 2147483648,
        REQUEST_TO_SPEAK = 4294967296,
        MANAGE_EVENTS = 8589934592,
        MANAGE_THREADS = 17179869184,
        CREATE_PUBLIC_THREADS = 34359738368,
        CREATE_PRIVATE_THREADS = 68719476736,
        USE_EXTERNAL_STICKERS = 137438953472,
        SEND_MESSAGES_IN_THREADS = 274877906944,
        USE_EMBEDDED_ACTIVITIES = 549755813888,
        MODERATE_MEMBERS = 1099511627776,
        VIEW_CREATOR_MONETIZATION_ANALYTICS = 2199023255552,
        USE_SOUNDBOARD = 4398046511104,
        CREATE_GUILD_EXPRESSIONS = 8796093022208,
        CREATE_EVENTS = 17592186044416,
        USE_EXTERNAL_SOUNDS = 35184372088832,
        SEND_VOICE_MESSAGES = 70368744177664,
        SET_VOICE_CHANNEL_STATUS = 281474976710656,
        SEND_POLLS = 562949953421312,
        USE_EXTERNAL_APPS = 1125899906842624,
        PIN_MESSAGES = 2251799813685248,
        BYPASS_SLOWMODE = 4503599627370496,
    }
    check_flags! { MessageFlags, 14;
        CROSSPOSTED = 1,
        IS_CROSSPOST = 2,
        SUPPRESS_EMBEDS = 4,
        SOURCE_MESSAGE_DELETED = 8,
        URGENT = 16,
        HAS_THREAD = 32,
        EPHEMERAL = 64,
        LOADING = 128,
        FAILED_TO_MENTION_SOME_ROLES_IN_THREAD = 256,
        SHOULD_SHOW_LINK_NOT_DISCORD_WARNING = 1024,
        SUPPRESS_NOTIFICATIONS = 4096,
        IS_VOICE_MESSAGE = 8192,
        HAS_SNAPSHOT = 16384,
        IS_COMPONENTS_V2 = 32768,
    }
    check_flags! { UserFlags, 23;
        STAFF = 1,
        PARTNER = 2,
        HYPESQUAD = 4,
        BUG_HUNTER_LEVEL1 = 8,
        MFA_SMS = 16,
        PREMIUM_PROMO_DISMISSED = 32,
        HYPE_SQUAD_ONLINE_HOUSE1 = 64,
        HYPE_SQUAD_ONLINE_HOUSE2 = 128,
        HYPE_SQUAD_ONLINE_HOUSE3 = 256,
        PREMIUM_EARLY_SUPPORTER = 512,
        TEAM_PSEUDO_USER = 1024,
        HAS_UNREAD_URGENT_MESSAGES = 8192,
        BUG_HUNTER_LEVEL2 = 16384,
        VERIFIED_BOT = 65536,
        VERIFIED_DEVELOPER = 131072,
        CERTIFIED_MODERATOR = 262144,
        BOT_HTTP_INTERACTIONS = 524288,
        SPAMMER = 1048576,
        DISABLE_PREMIUM = 2097152,
        ACTIVE_DEVELOPER = 4194304,
        QUARANTINED = 17592186044416,
        COLLABORATOR = 1125899906842624,
        RESTRICTED_COLLABORATOR = 2251799813685248,
    }
    check_flags! { ApplicationFlags, 16;
        EMBEDDED_RELEASED = 2,
        MANAGED_EMOJI = 4,
        EMBEDDED_IAP = 8,
        GROUP_DM_CREATE = 16,
        APPLICATION_AUTO_MODERATION_RULE_CREATE_BADGE = 64,
        RPC_HAS_CONNECTED = 2048,
        GATEWAY_PRESENCE = 4096,
        GATEWAY_PRESENCE_LIMITED = 8192,
        GATEWAY_GUILD_MEMBERS = 16384,
        GATEWAY_GUILD_MEMBERS_LIMITED = 32768,
        VERIFICATION_PENDING_GUILD_LIMIT = 65536,
        EMBEDDED = 131072,
        GATEWAY_MESSAGE_CONTENT = 262144,
        GATEWAY_MESSAGE_CONTENT_LIMITED = 524288,
        EMBEDDED_FIRST_PARTY = 1048576,
        APPLICATION_COMMAND_BADGE = 8388608,
    }
}
