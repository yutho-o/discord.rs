//! Wire tests for `User`, `Member`, `Role`, `Guild`, `UnavailableGuild` and the id types. The full
//! fixtures hold every key of the matching interface in `discord-api-types/payloads/v10`, so that
//! a field missing from a struct fails `every_key_survives`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![recursion_limit = "256"]

use dr_core::{Id, Patch, Timestamp, decode};
use dr_model::{
    ChannelId, ChannelType, CurrentUser, Guild, GuildDefaultMessageNotifications, GuildExplicitContentFilter,
    GuildFeature, GuildHubType, GuildId, GuildMFALevel, GuildMemberFlags, GuildNSFWLevel, GuildPremiumTier,
    GuildSystemChannelFlags, GuildVerificationLevel, Member, NameplatePalette, Nonce, Permissions, Role, RoleColors,
    RoleFlags, RoleId, RoleTags, UnavailableGuild, User, UserFlags, UserId, UserPremiumType, UserPrimaryGuild,
};
use serde_json::{Value, json};

const TIME: &str = "2021-01-01T00:00:00.123456+00:00";
const TIME_US: i64 = 1_609_459_200_123_456;

/// Fails unless every key of `input`, at any depth, is in `output` with the same value.
fn every_key_survives(input: &Value, output: &Value, path: &str) {
    match (input, output) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                let here = format!("{path}.{k}");
                let got = b.get(k).unwrap_or_else(|| panic!("{here} was lost: {output}"));
                every_key_survives(v, got, &here);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: length changed");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                every_key_survives(x, y, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(input, output, "{path}: value changed"),
    }
}

fn keys(v: &Value) -> Vec<&str> {
    let mut k: Vec<&str> = v.as_object().expect("an object").keys().map(String::as_str).collect();
    k.sort_unstable();
    k
}

fn full_user() -> Value {
    json!({
        "id": "80351110224678912",
        "username": "nelly",
        "discriminator": "1337",
        "global_name": "Nelly",
        "avatar": "8342729096ea3675442027381ff50dfe",
        "bot": true,
        "system": true,
        "mfa_enabled": true,
        "banner": "06c16474723fe537c283b8efa61a30c8",
        "accent_color": 16_711_680,
        "locale": "en-US",
        "verified": true,
        "email": "nelly@example.invalid",
        "flags": 64,
        "premium_type": 2,
        "public_flags": 131_072,
        "avatar_decoration": "a_old",
        "avatar_decoration_data": { "asset": "a_fff", "sku_id": "1144058844004233369" },
        "collectibles": {
            "nameplate": {
                "sku_id": "2247558840304243311",
                "asset": "nameplates/nameplates/twilight/",
                "label": "",
                "palette": "bubble_gum"
            }
        },
        "primary_guild": {
            "identity_guild_id": "1234567890",
            "identity_enabled": true,
            "tag": "ABCD",
            "badge": "a_badge"
        }
    })
}

#[test]
fn user_full_payload_reads_every_field() {
    let u: User = serde_json::from_value(full_user()).unwrap();
    assert_eq!(u.id, UserId::new(80_351_110_224_678_912));
    assert_eq!(u.username, "nelly");
    assert_eq!(u.discriminator, "1337");
    assert_eq!(u.global_name.as_deref(), Some("Nelly"));
    assert_eq!(u.avatar.as_deref(), Some("8342729096ea3675442027381ff50dfe"));
    assert!(u.bot && u.system);
    assert_eq!(u.mfa_enabled, Some(true));
    assert_eq!(u.banner, Patch::Value("06c16474723fe537c283b8efa61a30c8".to_owned()));
    assert_eq!(u.accent_color, Patch::Value(0xFF_00_00));
    assert_eq!(u.locale.as_deref(), Some("en-US"));
    assert_eq!(u.verified, Some(true));
    assert_eq!(u.email.as_deref(), Some("nelly@example.invalid"));
    assert_eq!(u.flags.map(|f| f.bits()), Some(64));
    assert_eq!(u.premium_type, Some(UserPremiumType::NITRO));
    assert_eq!(u.public_flags.map(|f| f.bits()), Some(131_072));
    assert_eq!(u.avatar_decoration.as_deref(), Some("a_old"));
    let decoration = u.avatar_decoration_data.as_ref().unwrap();
    assert_eq!((decoration.asset.as_str(), decoration.sku_id.get()), ("a_fff", 1_144_058_844_004_233_369));
    let plate = u.collectibles.as_ref().unwrap().nameplate.as_ref().unwrap();
    assert_eq!(plate.palette, Some(NameplatePalette::BubbleGum));
    assert_eq!(plate.asset, "nameplates/nameplates/twilight/");
    assert_eq!(plate.sku_id.get(), 2_247_558_840_304_243_311);
    let primary = u.primary_guild.as_ref().unwrap();
    assert_eq!(primary.identity_guild_id, Some(GuildId::new(1_234_567_890)));
    assert_eq!(primary.identity_enabled, Some(true));
    assert_eq!(primary.tag.as_deref(), Some("ABCD"));
    assert_eq!(primary.badge.as_deref(), Some("a_badge"));
}

#[test]
fn user_full_payload_round_trips() {
    let input = full_user();
    let u: User = serde_json::from_value(input.clone()).unwrap();
    let output = serde_json::to_value(&u).unwrap();
    every_key_survives(&input, &output, "user");
    let again: User = serde_json::from_value(output).unwrap();
    assert_eq!(u, again);
}

#[test]
fn user_with_only_an_id_decodes_and_invents_no_optional_keys() {
    let u: User = decode(r#"{"id":"1"}"#).unwrap();
    assert_eq!(u, User::partial(UserId::new(1)));
    assert!(u.is_partial());
    let out = serde_json::to_value(&u).unwrap();
    assert_eq!(keys(&out), ["avatar", "discriminator", "global_name", "id", "username"]);
    assert_eq!(out["global_name"], Value::Null);
    assert_eq!(out["avatar"], Value::Null);
}

#[test]
fn user_without_an_id_fails_with_a_path() {
    let nested = decode::<Vec<User>>(r#"[{"id":"1"},{"username":"x"}]"#).unwrap_err();
    assert!(nested.path.starts_with("[1]"), "path was {}", nested.path);
    assert!(nested.source.to_string().contains("missing field `id`"), "{}", nested.source);
}

#[test]
fn user_unknown_keys_are_ignored() {
    let u: User = decode(r#"{"id":"1","username":"a","a_new_field":{"nested":[1,2]},"clan":null}"#).unwrap();
    assert_eq!(u.username, "a");
}

#[test]
fn user_banner_and_accent_color_tell_absent_from_null() {
    let absent: User = decode(r#"{"id":"1"}"#).unwrap();
    assert!(absent.banner.is_absent() && absent.accent_color.is_absent());

    let null: User = decode(r#"{"id":"1","banner":null,"accent_color":null}"#).unwrap();
    assert!(null.banner.is_null() && null.accent_color.is_null());
    let out = serde_json::to_value(&null).unwrap();
    assert_eq!(out["banner"], Value::Null);
    assert_eq!(out["accent_color"], Value::Null);
    assert!(out.as_object().unwrap().contains_key("banner"));

    let value: User = decode(r#"{"id":"1","banner":"h","accent_color":5}"#).unwrap();
    assert_eq!(value.banner.as_option().map(String::as_str), Some("h"));
    assert_eq!(value.accent_color.as_option(), Some(&5));
}

#[test]
fn user_nullable_keys_read_null_as_none() {
    let u: User = decode(
        r#"{"id":"1","email":null,"avatar_decoration":null,"avatar_decoration_data":null,
            "collectibles":null,"primary_guild":null,"global_name":null,"avatar":null}"#,
    )
    .unwrap();
    assert_eq!(u, User::partial(UserId::new(1)));
}

#[test]
fn user_primary_guild_may_be_cleared_by_the_system() {
    let u: User = decode(
        r#"{"id":"1","primary_guild":{"identity_guild_id":null,"identity_enabled":null,"tag":null,"badge":null}}"#,
    )
    .unwrap();
    assert_eq!(u.primary_guild, Some(UserPrimaryGuild::default()));
}

#[test]
fn user_empty_collectibles_have_no_nameplate() {
    let u: User = decode(r#"{"id":"1","collectibles":{}}"#).unwrap();
    assert!(u.collectibles.unwrap().nameplate.is_none());
}

#[test]
fn user_flags_keep_unknown_high_bits() {
    let raw = (1u64 << 62) | (1 << 44);
    let u: User = decode(&format!(r#"{{"id":"1","public_flags":{raw},"flags":"{raw}"}}"#)).unwrap();
    assert_eq!(u.public_flags, Some(UserFlags::from_bits_retain(raw)));
    assert_eq!(u.flags, Some(UserFlags::from_bits_retain(raw)));
    let out = serde_json::to_value(&u).unwrap();
    assert_eq!(out["public_flags"], json!(raw));
}

#[test]
fn user_unknown_enum_values_are_kept() {
    let u: User =
        decode(r#"{"id":"1","premium_type":99,"collectibles":{"nameplate":{"sku_id":"2","palette":"neon"}}}"#).unwrap();
    assert_eq!(u.premium_type, Some(UserPremiumType(99)));
    let palette = u.collectibles.as_ref().unwrap().nameplate.as_ref().unwrap().palette.as_ref().unwrap();
    assert_eq!(palette.as_str(), "neon");
    assert!(matches!(palette, NameplatePalette::Unknown(_)));
    let out = serde_json::to_value(&u).unwrap();
    assert_eq!(out["premium_type"], json!(99));
    assert_eq!(out["collectibles"]["nameplate"]["palette"], json!("neon"));
}

#[test]
fn user_premium_type_values_match_upstream() {
    assert_eq!(UserPremiumType::NONE.get(), 0);
    assert_eq!(UserPremiumType::NITRO_CLASSIC.get(), 1);
    assert_eq!(UserPremiumType::NITRO.get(), 2);
    assert_eq!(UserPremiumType::NITRO_BASIC.get(), 3);
}

#[test]
fn user_display_name_and_tag_follow_upstream() {
    let mut u = User::partial(UserId::new(1));
    assert_eq!(u.tag(), None);
    u.username = "nelly".to_owned();
    u.discriminator = "0".to_owned();
    assert_eq!(u.tag().as_deref(), Some("nelly"));
    u.discriminator = "0000".to_owned();
    assert_eq!(u.tag().as_deref(), Some("nelly"));
    u.discriminator = "1337".to_owned();
    assert_eq!(u.tag().as_deref(), Some("nelly#1337"));

    assert_eq!(u.display_name(), "nelly");
    u.global_name = Some("Nelly".to_owned());
    assert_eq!(u.display_name(), "Nelly");
    u.global_name = Some(String::new());
    assert_eq!(u.display_name(), "", "upstream uses ??, so an empty name stays");
}

#[test]
fn current_user_is_a_user() {
    let u: CurrentUser = decode(r#"{"id":"1","username":"bot","bot":true,"verified":true}"#).unwrap();
    assert!(u.bot);
    assert_eq!(u.verified, Some(true));
}

fn full_member() -> Value {
    json!({
        "user": { "id": "7", "username": "u", "discriminator": "0", "global_name": null, "avatar": null },
        "guild_id": "2",
        "nick": "nick",
        "avatar": "member_avatar",
        "banner": "member_banner",
        "roles": ["10", "11"],
        "joined_at": TIME,
        "premium_since": TIME,
        "deaf": true,
        "mute": true,
        "flags": 1,
        "pending": true,
        "permissions": "2048",
        "communication_disabled_until": TIME,
        "avatar_decoration_data": { "asset": "a", "sku_id": "3" },
        "collectibles": { "nameplate": { "sku_id": "4", "asset": "p", "label": "", "palette": "sky" } }
    })
}

#[test]
fn member_full_payload_reads_every_field() {
    let m: Member = serde_json::from_value(full_member()).unwrap();
    assert_eq!(m.user.as_ref().unwrap().id, UserId::new(7));
    assert_eq!(m.guild_id, Some(GuildId::new(2)));
    assert_eq!(m.nick.as_deref(), Some("nick"));
    assert_eq!(m.avatar.as_deref(), Some("member_avatar"));
    assert_eq!(m.banner.as_deref(), Some("member_banner"));
    assert_eq!(m.roles, [RoleId::new(10), RoleId::new(11)]);
    assert_eq!(m.joined_at, Some(Timestamp(TIME_US)));
    assert_eq!(m.premium_since, Some(Timestamp(TIME_US)));
    assert!(m.deaf && m.mute && m.pending);
    assert_eq!(m.flags, Some(GuildMemberFlags::DID_REJOIN));
    assert_eq!(m.permissions, Some(Permissions::SEND_MESSAGES));
    assert_eq!(m.communication_disabled_until, Some(Timestamp(TIME_US)));
    assert_eq!(m.avatar_decoration_data.as_ref().unwrap().asset, "a");
    assert_eq!(m.collectibles.unwrap().nameplate.unwrap().palette, Some(NameplatePalette::Sky));
}

#[test]
fn member_full_payload_round_trips() {
    let input = full_member();
    let m: Member = serde_json::from_value(input.clone()).unwrap();
    let output = serde_json::to_value(&m).unwrap();
    every_key_survives(&input, &output, "member");
    assert_eq!(m, serde_json::from_value(output).unwrap());
}

#[test]
fn member_nested_in_a_message_has_no_user() {
    let m: Member = decode(
        r#"{"roles":[],"premium_since":null,"pending":false,"nick":null,"mute":false,
            "joined_at":"2021-01-01T00:00:00+00:00","flags":0,"deaf":false,
            "communication_disabled_until":null,"banner":null,"avatar":null}"#,
    )
    .unwrap();
    assert!(m.user.is_none());
    assert_eq!(m.display_name(), None);
    assert!(!m.is_partial());
    let out = serde_json::to_value(&m).unwrap();
    assert!(!out.as_object().unwrap().contains_key("user"), "no invented user key: {out}");
}

#[test]
fn member_from_a_partial_update_decodes() {
    // GatewayGuildMemberUpdateDispatchData: `deaf`, `mute` and `flags` optional, `joined_at` nullable.
    let m: Member = decode(r#"{"guild_id":"2","roles":["10"],"user":{"id":"7"},"joined_at":null}"#).unwrap();
    assert_eq!(m.guild_id, Some(GuildId::new(2)));
    assert_eq!(m.roles, [RoleId::new(10)]);
    assert!(m.is_partial());
    assert!(!m.deaf && !m.mute && !m.pending);
    assert_eq!(m.flags, None);
}

#[test]
fn member_with_nothing_decodes_to_the_default() {
    let m: Member = decode("{}").unwrap();
    assert_eq!(m, Member::default());
    assert!(m.roles.is_empty());
    let n: Member = decode(r#"{"roles":null}"#).unwrap();
    assert!(n.roles.is_empty());
}

#[test]
fn member_with_a_bad_timestamp_fails_with_a_path() {
    let e = decode::<Member>(r#"{"premium_since":"yesterday"}"#).unwrap_err();
    assert_eq!(e.path, "premium_since");
}

#[test]
fn member_flags_keep_unknown_bits() {
    let m: Member = decode(r#"{"flags":3145729}"#).unwrap();
    let flags = m.flags.unwrap();
    assert!(flags.contains(GuildMemberFlags::DID_REJOIN));
    assert_eq!(flags.bits(), 3_145_729);
}

#[test]
fn member_flag_bits_match_upstream() {
    let cases = [
        (GuildMemberFlags::DID_REJOIN, 0),
        (GuildMemberFlags::COMPLETED_ONBOARDING, 1),
        (GuildMemberFlags::BYPASSES_VERIFICATION, 2),
        (GuildMemberFlags::STARTED_ONBOARDING, 3),
        (GuildMemberFlags::IS_GUEST, 4),
        (GuildMemberFlags::STARTED_HOME_ACTIONS, 5),
        (GuildMemberFlags::COMPLETED_HOME_ACTIONS, 6),
        (GuildMemberFlags::AUTOMOD_QUARANTINED_USERNAME_OR_GUILD_NICKNAME, 7),
        (GuildMemberFlags::AUTOMOD_QUARANTINED_BIO, 8),
        (GuildMemberFlags::DM_SETTINGS_UPSELL_ACKNOWLEDGED, 9),
        (GuildMemberFlags::AUTO_MOD_QUARANTINED_GUILD_TAG, 10),
    ];
    for (flag, bit) in cases {
        assert_eq!(flag.bits(), 1 << bit, "{flag:?}");
    }
}

#[test]
fn member_display_name_prefers_the_nickname() {
    let mut u = User::partial(UserId::new(1));
    u.username = "name".to_owned();
    u.global_name = Some("Global".to_owned());
    let mut m = Member { user: Some(u), ..Member::default() };
    assert_eq!(m.display_name(), Some("Global"));
    m.nick = Some("Nick".to_owned());
    assert_eq!(m.display_name(), Some("Nick"));
    m.user = None;
    assert_eq!(m.display_name(), Some("Nick"));
}

fn full_role() -> Value {
    json!({
        "id": "41771983423143936",
        "name": "WE DEM BOYZZ!!!!!!",
        "color": 3_447_003,
        "colors": { "primary_color": 11_127_295, "secondary_color": 16_759_788, "tertiary_color": 16_761_760 },
        "hoist": true,
        "icon": "cf3ced8600b777c9486c6d8d84fb4327",
        "unicode_emoji": "x",
        "position": 1,
        "permissions": "66321471",
        "managed": true,
        "mentionable": true,
        "tags": {
            "bot_id": "5",
            "premium_subscriber": null,
            "integration_id": "6",
            "subscription_listing_id": "7",
            "available_for_purchase": null,
            "guild_connections": null
        },
        "flags": 1
    })
}

#[test]
fn role_full_payload_reads_every_field() {
    let r: Role = serde_json::from_value(full_role()).unwrap();
    assert_eq!(r.id, RoleId::new(41_771_983_423_143_936));
    assert_eq!(r.name, "WE DEM BOYZZ!!!!!!");
    assert_eq!(r.color, 3_447_003);
    assert_eq!(
        r.colors,
        RoleColors { primary_color: 11_127_295, secondary_color: Some(16_759_788), tertiary_color: Some(16_761_760) }
    );
    assert!(r.hoist && r.managed && r.mentionable);
    assert_eq!(r.icon.as_deref(), Some("cf3ced8600b777c9486c6d8d84fb4327"));
    assert_eq!(r.unicode_emoji.as_deref(), Some("x"));
    assert_eq!(r.position, 1);
    assert_eq!(r.permissions.bits(), 66_321_471);
    assert_eq!(r.flags, Some(RoleFlags::IN_PROMPT));
    let tags = r.tags.unwrap();
    assert_eq!(tags.bot_id, Some(UserId::new(5)));
    assert_eq!(tags.integration_id.map(Id::get), Some(6));
    assert_eq!(tags.subscription_listing_id.map(Id::get), Some(7));
    assert!(tags.premium_subscriber && tags.available_for_purchase && tags.guild_connections);
}

#[test]
fn role_full_payload_round_trips() {
    let input = full_role();
    let r: Role = serde_json::from_value(input.clone()).unwrap();
    let output = serde_json::to_value(&r).unwrap();
    every_key_survives(&input, &output, "role");
    assert_eq!(r, serde_json::from_value(output).unwrap());
}

#[test]
fn role_tags_null_means_true_and_absent_means_false() {
    let none: RoleTags = decode("{}").unwrap();
    assert_eq!(none, RoleTags::default());
    assert!(!none.premium_subscriber && !none.available_for_purchase && !none.guild_connections);
    assert_eq!(serde_json::to_string(&none).unwrap(), "{}");

    let booster: RoleTags = decode(r#"{"premium_subscriber":null}"#).unwrap();
    assert!(booster.premium_subscriber);
    assert!(!booster.available_for_purchase && !booster.guild_connections);
    assert_eq!(serde_json::to_string(&booster).unwrap(), r#"{"premium_subscriber":null}"#);

    let linked: RoleTags = decode(r#"{"guild_connections":null,"available_for_purchase":null}"#).unwrap();
    assert!(linked.guild_connections && linked.available_for_purchase && !linked.premium_subscriber);
}

#[test]
fn role_tags_any_value_means_present_as_upstream_tests_in() {
    let t: RoleTags = decode(r#"{"premium_subscriber":false,"guild_connections":{"x":[1]}}"#).unwrap();
    assert!(t.premium_subscriber, "'key' in data.tags ignores the value");
    assert!(t.guild_connections);
}

#[test]
fn role_tags_null_survive_a_decode_through_a_value() {
    // Payloads held in a `serde_json::Value`, as the mock does, must read the same as text.
    let t: RoleTags = serde_json::from_value(json!({ "premium_subscriber": null })).unwrap();
    assert!(t.premium_subscriber);
}

#[test]
fn role_with_only_the_required_keys_decodes() {
    let r: Role = decode(r#"{"id":"1","permissions":"0"}"#).unwrap();
    assert_eq!(r.colors, RoleColors::default());
    assert_eq!(r.flags, None);
    assert_eq!(r.tags, None);
    assert_eq!(r.position, 0);
}

#[test]
fn role_without_permissions_fails_with_a_path() {
    let e = decode::<Role>(r#"{"id":"1"}"#).unwrap_err();
    assert!(e.source.to_string().contains("missing field `permissions`"), "{}", e.source);
}

#[test]
fn role_permissions_keep_every_bit_and_write_a_decimal_string() {
    let raw = (1u64 << 63) | (1 << 40) | 2048;
    let r: Role = decode(&format!(r#"{{"id":"1","permissions":"{raw}"}}"#)).unwrap();
    assert_eq!(r.permissions, Permissions::from_bits_retain(raw));
    assert!(r.permissions.contains(Permissions::SEND_MESSAGES));
    let out = serde_json::to_value(&r).unwrap();
    assert_eq!(out["permissions"], json!(raw.to_string()));
    let as_number: Role = decode(r#"{"id":"1","permissions":2048}"#).unwrap();
    assert_eq!(as_number.permissions, Permissions::SEND_MESSAGES);
}

#[test]
fn role_colors_may_be_null_beyond_the_primary() {
    let r: Role = decode(
        r#"{"id":"1","permissions":"0","colors":{"primary_color":5,"secondary_color":null,"tertiary_color":null}}"#,
    )
    .unwrap();
    assert_eq!(r.colors, RoleColors { primary_color: 5, secondary_color: None, tertiary_color: None });
    let out = serde_json::to_value(&r).unwrap();
    assert_eq!(out["colors"]["secondary_color"], Value::Null);
}

#[test]
fn everyone_role_has_the_id_of_its_guild() {
    let g: Guild =
        decode(r#"{"id":"99","roles":[{"id":"99","permissions":"0"},{"id":"100","permissions":"0"}]}"#).unwrap();
    assert_eq!(g.everyone_role_id(), RoleId::new(99));
    assert!(g.roles[0].is_everyone(g.id));
    assert!(!g.roles[1].is_everyone(g.id));
}

/// Every key of `APIGuild`, plus `APIPartialGuild`, plus `GatewayGuildCreateDispatchData` fields the
/// proof of concept keeps. `stickers`, `threads`, `presences` and the like are cut features.
fn full_guild() -> Value {
    json!({
        "id": "197038439483310086",
        "name": "Discord Testers",
        "icon": "f64c482b807da4f539cff778d174971c",
        "icon_hash": "f64c482b807da4f539cff778d174971d",
        "splash": "s",
        "discovery_splash": "ds",
        "owner": true,
        "owner_id": "73193882359173120",
        "permissions": "2048",
        "region": "us-west",
        "afk_channel_id": "1",
        "afk_timeout": 300,
        "widget_enabled": true,
        "widget_channel_id": "2",
        "verification_level": 3,
        "default_message_notifications": 1,
        "explicit_content_filter": 2,
        "roles": [ { "id": "197038439483310086", "permissions": "0" } ],
        "emojis": [ { "id": "3", "name": "e" } ],
        "features": ["COMMUNITY", "A_FEATURE_FROM_THE_FUTURE"],
        "mfa_level": 1,
        "application_id": "4",
        "system_channel_id": "5",
        "system_channel_flags": 7,
        "rules_channel_id": "6",
        "max_presences": 40_000,
        "max_members": 500_000,
        "vanity_url_code": "discord-testers",
        "description": "d",
        "banner": "b",
        "premium_tier": 3,
        "premium_subscription_count": 33,
        "preferred_locale": "en-US",
        "public_updates_channel_id": "7",
        "max_video_channel_users": 25,
        "max_stage_video_channel_users": 50,
        "approximate_member_count": 60_000,
        "approximate_presence_count": 20_000,
        "nsfw_level": 2,
        "premium_progress_bar_enabled": true,
        "hub_type": 1,
        "safety_alerts_channel_id": "8",
        "incidents_data": {
            "invites_disabled_until": TIME,
            "dms_disabled_until": TIME,
            "dm_spam_detected_at": TIME,
            "raid_detected_at": TIME
        },
        "welcome_screen": {
            "description": "hello",
            "welcome_channels": [
                { "channel_id": "9", "description": "rules", "emoji_id": "10", "emoji_name": "r" }
            ]
        },
        "large": true,
        "unavailable": true,
        "member_count": 60_001,
        "joined_at": TIME,
        "channels": [ { "id": "11", "type": 0, "name": "general" } ],
        "members": [ { "user": { "id": "12" }, "roles": [], "joined_at": TIME } ]
    })
}

#[test]
fn guild_full_payload_reads_every_field() {
    let g: Guild = serde_json::from_value(full_guild()).unwrap();
    assert_eq!(g.id, GuildId::new(197_038_439_483_310_086));
    assert_eq!(g.name, "Discord Testers");
    assert_eq!(g.icon.as_deref(), Some("f64c482b807da4f539cff778d174971c"));
    assert_eq!(g.icon_hash.as_deref(), Some("f64c482b807da4f539cff778d174971d"));
    assert_eq!(g.splash.as_deref(), Some("s"));
    assert_eq!(g.discovery_splash.as_deref(), Some("ds"));
    assert_eq!(g.owner, Some(true));
    assert_eq!(g.owner_id, Some(UserId::new(73_193_882_359_173_120)));
    assert_eq!(g.permissions, Some(Permissions::SEND_MESSAGES));
    assert_eq!(g.region.as_deref(), Some("us-west"));
    assert_eq!(g.afk_channel_id, Some(ChannelId::new(1)));
    assert_eq!(g.afk_timeout, 300);
    assert_eq!(g.widget_enabled, Some(true));
    assert_eq!(g.widget_channel_id, Some(ChannelId::new(2)));
    assert_eq!(g.verification_level, Some(GuildVerificationLevel::HIGH));
    assert_eq!(g.default_message_notifications, Some(GuildDefaultMessageNotifications::ONLY_MENTIONS));
    assert_eq!(g.explicit_content_filter, Some(GuildExplicitContentFilter::ALL_MEMBERS));
    assert_eq!(g.roles.len(), 1);
    assert_eq!(g.emojis.len(), 1);
    assert_eq!(g.features[0], GuildFeature::Community);
    assert!(matches!(g.features[1], GuildFeature::Unknown(_)));
    assert_eq!(g.features[1].as_str(), "A_FEATURE_FROM_THE_FUTURE");
    assert_eq!(g.mfa_level, Some(GuildMFALevel::ELEVATED));
    assert_eq!(g.application_id.map(Id::get), Some(4));
    assert_eq!(g.system_channel_id, Some(ChannelId::new(5)));
    assert_eq!(
        g.system_channel_flags,
        Some(
            GuildSystemChannelFlags::SUPPRESS_JOIN_NOTIFICATIONS
                | GuildSystemChannelFlags::SUPPRESS_PREMIUM_SUBSCRIPTIONS
                | GuildSystemChannelFlags::SUPPRESS_GUILD_REMINDER_NOTIFICATIONS
        )
    );
    assert_eq!(g.rules_channel_id, Some(ChannelId::new(6)));
    assert_eq!(g.max_presences, Some(40_000));
    assert_eq!(g.max_members, Some(500_000));
    assert_eq!(g.vanity_url_code.as_deref(), Some("discord-testers"));
    assert_eq!(g.description.as_deref(), Some("d"));
    assert_eq!(g.banner.as_deref(), Some("b"));
    assert_eq!(g.premium_tier, Some(GuildPremiumTier::TIER3));
    assert_eq!(g.premium_subscription_count, Some(33));
    assert_eq!(g.preferred_locale, "en-US");
    assert_eq!(g.public_updates_channel_id, Some(ChannelId::new(7)));
    assert_eq!(g.max_video_channel_users, Some(25));
    assert_eq!(g.max_stage_video_channel_users, Some(50));
    assert_eq!(g.approximate_member_count, Some(60_000));
    assert_eq!(g.approximate_presence_count, Some(20_000));
    assert_eq!(g.nsfw_level, Some(GuildNSFWLevel::SAFE));
    assert!(g.premium_progress_bar_enabled);
    assert_eq!(g.hub_type, Some(GuildHubType::HIGH_SCHOOL));
    assert_eq!(g.safety_alerts_channel_id, Some(ChannelId::new(8)));
    let incidents = g.incidents_data.as_ref().unwrap();
    assert_eq!(incidents.invites_disabled_until, Some(Timestamp(TIME_US)));
    assert_eq!(incidents.dms_disabled_until, Some(Timestamp(TIME_US)));
    assert_eq!(incidents.dm_spam_detected_at, Some(Timestamp(TIME_US)));
    assert_eq!(incidents.raid_detected_at, Some(Timestamp(TIME_US)));
    let screen = g.welcome_screen.as_ref().unwrap();
    assert_eq!(screen.description.as_deref(), Some("hello"));
    assert_eq!(screen.welcome_channels[0].channel_id, ChannelId::new(9));
    assert_eq!(screen.welcome_channels[0].description, "rules");
    assert_eq!(screen.welcome_channels[0].emoji_id.map(Id::get), Some(10));
    assert_eq!(screen.welcome_channels[0].emoji_name.as_deref(), Some("r"));
    assert!(g.large && g.unavailable);
    assert_eq!(g.member_count, Some(60_001));
    assert_eq!(g.joined_at, Some(Timestamp(TIME_US)));
    assert_eq!(g.channels[0].kind, ChannelType::GUILD_TEXT);
    assert_eq!(g.members[0].user.as_ref().unwrap().id, UserId::new(12));
}

#[test]
fn guild_full_payload_round_trips() {
    let input = full_guild();
    let g: Guild = serde_json::from_value(input.clone()).unwrap();
    let output = serde_json::to_value(&g).unwrap();
    every_key_survives(&input, &output, "guild");
}

#[test]
fn guild_on_an_outage_is_only_an_id() {
    let g: Guild = decode(r#"{"id":"5","unavailable":true}"#).unwrap();
    assert!(g.unavailable);
    assert_eq!(g.name, "");
    assert!(g.owner_id.is_none() && g.verification_level.is_none());
    assert!(g.roles.is_empty() && g.channels.is_empty() && g.members.is_empty());
}

#[test]
fn guild_from_rest_has_no_gateway_only_keys() {
    let g: Guild = decode(r#"{"id":"5","name":"n","roles":[],"emojis":[],"features":[]}"#).unwrap();
    assert!(!g.unavailable && !g.large);
    assert_eq!(g.member_count, None);
    assert_eq!(g.joined_at, None);
    let out = serde_json::to_value(&g).unwrap();
    let map = out.as_object().unwrap();
    for gateway_only in ["unavailable", "large", "member_count", "joined_at", "channels", "members"] {
        assert!(!map.contains_key(gateway_only), "{gateway_only} was invented: {out}");
    }
    assert_eq!(out["icon"], Value::Null, "a nullable key is written");
}

#[test]
fn guild_lists_sent_as_null_read_as_empty() {
    let g: Guild =
        decode(r#"{"id":"5","roles":null,"emojis":null,"features":null,"channels":null,"members":null}"#).unwrap();
    assert!(g.roles.is_empty() && g.emojis.is_empty() && g.features.is_empty());
    assert!(g.channels.is_empty() && g.members.is_empty());
}

#[test]
fn guild_ignores_keys_of_cut_features_and_new_keys() {
    let g: Guild = decode(
        r#"{"id":"5","stickers":[{"id":"1"}],"presences":[{"user":{"id":"3"}}],
            "voice_states":[{}],"stage_instances":[{}],"guild_scheduled_events":[{}],
            "soundboard_sounds":[{}],"home_header":null,"a_new_key":{"x":1}}"#,
    )
    .unwrap();
    assert_eq!(g.id, GuildId::new(5));
}

#[test]
fn guild_create_threads_are_read() {
    let g: Guild = decode(r#"{"id":"5","threads":[{"id":"2","type":11,"parent_id":"3"}]}"#).unwrap();
    assert_eq!(g.threads.len(), 1);
    assert!(g.threads[0].is_thread());
    let none: Guild = decode(r#"{"id":"5","threads":null}"#).unwrap();
    assert!(none.threads.is_empty());
}

#[test]
fn guild_unknown_levels_and_flag_bits_are_kept() {
    let g: Guild = decode(
        r#"{"id":"5","verification_level":77,"default_message_notifications":78,"explicit_content_filter":79,
            "mfa_level":80,"premium_tier":81,"nsfw_level":82,"hub_type":83,"system_channel_flags":1099511627776}"#,
    )
    .unwrap();
    assert_eq!(g.verification_level, Some(GuildVerificationLevel(77)));
    assert_eq!(g.default_message_notifications, Some(GuildDefaultMessageNotifications(78)));
    assert_eq!(g.explicit_content_filter, Some(GuildExplicitContentFilter(79)));
    assert_eq!(g.mfa_level, Some(GuildMFALevel(80)));
    assert_eq!(g.premium_tier, Some(GuildPremiumTier(81)));
    assert_eq!(g.nsfw_level, Some(GuildNSFWLevel(82)));
    assert_eq!(g.hub_type, Some(GuildHubType(83)));
    assert_eq!(g.system_channel_flags.map(|f| f.bits()), Some(1 << 40));
    let out = serde_json::to_value(&g).unwrap();
    assert_eq!(out["verification_level"], json!(77));
    assert_eq!(out["system_channel_flags"], json!(1u64 << 40));
}

#[test]
fn guild_enum_values_match_upstream() {
    assert_eq!(GuildDefaultMessageNotifications::ALL_MESSAGES.get(), 0);
    assert_eq!(GuildExplicitContentFilter::DISABLED.get(), 0);
    assert_eq!(GuildExplicitContentFilter::MEMBERS_WITHOUT_ROLES.get(), 1);
    assert_eq!(GuildMFALevel::NONE.get(), 0);
    assert_eq!(GuildNSFWLevel::DEFAULT.get(), 0);
    assert_eq!(GuildNSFWLevel::EXPLICIT.get(), 1);
    assert_eq!(GuildNSFWLevel::AGE_RESTRICTED.get(), 3);
    assert_eq!(GuildVerificationLevel::NONE.get(), 0);
    assert_eq!(GuildVerificationLevel::LOW.get(), 1);
    assert_eq!(GuildVerificationLevel::MEDIUM.get(), 2);
    assert_eq!(GuildVerificationLevel::VERY_HIGH.get(), 4);
    assert_eq!(GuildPremiumTier::NONE.get(), 0);
    assert_eq!(GuildPremiumTier::TIER1.get(), 1);
    assert_eq!(GuildPremiumTier::TIER2.get(), 2);
    assert_eq!(GuildHubType::DEFAULT.get(), 0);
    assert_eq!(GuildHubType::COLLEGE.get(), 2);
}

#[test]
fn guild_system_channel_flag_bits_match_upstream() {
    let cases = [
        (GuildSystemChannelFlags::SUPPRESS_JOIN_NOTIFICATIONS, 0),
        (GuildSystemChannelFlags::SUPPRESS_PREMIUM_SUBSCRIPTIONS, 1),
        (GuildSystemChannelFlags::SUPPRESS_GUILD_REMINDER_NOTIFICATIONS, 2),
        (GuildSystemChannelFlags::SUPPRESS_JOIN_NOTIFICATION_REPLIES, 3),
        (GuildSystemChannelFlags::SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATIONS, 4),
        (GuildSystemChannelFlags::SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATION_REPLIES, 5),
    ];
    for (flag, bit) in cases {
        assert_eq!(flag.bits(), 1 << bit, "{flag:?}");
    }
}

#[test]
fn guild_nested_bad_member_reports_a_path_into_the_list() {
    let e = decode::<Guild>(r#"{"id":"5","members":[{},{"roles":"nope"}]}"#).unwrap_err();
    assert!(e.path.starts_with("members[1]"), "path was {}", e.path);
}

#[test]
fn guild_without_an_id_is_a_decode_failure() {
    let e = decode::<Guild>(r#"{"name":"n"}"#).unwrap_err();
    assert!(e.source.to_string().contains("missing field `id`"), "{}", e.source);
}

#[test]
fn guild_partial_has_only_its_id() {
    let g = Guild::partial(GuildId::new(5));
    let decoded: Guild = decode(r#"{"id":"5"}"#).unwrap();
    assert_eq!(serde_json::to_value(&g).unwrap(), serde_json::to_value(&decoded).unwrap());
}

#[test]
fn unavailable_guild_tells_an_outage_from_a_removal() {
    let outage: UnavailableGuild = decode(r#"{"id":"5","unavailable":true}"#).unwrap();
    assert!(outage.unavailable);
    assert_eq!(serde_json::to_value(&outage).unwrap(), json!({ "id": "5", "unavailable": true }));

    let removal: UnavailableGuild = decode(r#"{"id":"5"}"#).unwrap();
    assert!(!removal.unavailable);
    assert_eq!(serde_json::to_value(&removal).unwrap(), json!({ "id": "5" }));

    let explicit_false: UnavailableGuild = decode(r#"{"id":"5","unavailable":false}"#).unwrap();
    assert!(!explicit_false.unavailable);
}

#[test]
fn unavailable_guild_becomes_a_stub_guild() {
    let g = Guild::from(UnavailableGuild { id: GuildId::new(5), unavailable: true });
    assert!(g.unavailable);
    assert_eq!(g.id, GuildId::new(5));
    assert!(g.channels.is_empty());
}

#[test]
fn ids_read_strings_and_integers_and_write_strings() {
    let id: UserId = serde_json::from_str(r#""80351110224678912""#).unwrap();
    assert_eq!(id.get(), 80_351_110_224_678_912);
    let int: UserId = serde_json::from_str("80351110224678912").unwrap();
    assert_eq!(int, id);
    assert_eq!(serde_json::to_string(&id).unwrap(), r#""80351110224678912""#);
    assert_eq!(serde_json::from_str::<UserId>(r#""0""#).unwrap().get(), 0);
    assert!(serde_json::from_str::<UserId>(r#""abc""#).is_err());
    assert!(serde_json::from_str::<UserId>("-1").is_err());
}

#[test]
fn ids_cast_for_the_everyone_role() {
    let guild = GuildId::new(5);
    let role: RoleId = guild.cast();
    assert_eq!(role.get(), 5);
}

#[test]
fn nonce_reads_both_forms_and_writes_the_same_form() {
    assert_eq!(serde_json::from_str::<Nonce>("123").unwrap(), Nonce::Int(123));
    assert_eq!(serde_json::from_str::<Nonce>(r#""123""#).unwrap(), Nonce::Str("123".to_owned()));
    assert_eq!(serde_json::to_string(&Nonce::Int(123)).unwrap(), "123");
    assert_eq!(serde_json::to_string(&Nonce::Str("123".to_owned())).unwrap(), r#""123""#);
}

#[test]
fn nonce_converts_and_displays() {
    assert_eq!(Nonce::from(5), Nonce::Int(5));
    assert_eq!(Nonce::from("a"), Nonce::Str("a".to_owned()));
    assert_eq!(Nonce::from("a".to_owned()), Nonce::Str("a".to_owned()));
    assert_eq!(Nonce::Int(5).to_string(), "5");
    assert_eq!(Nonce::Str("a".to_owned()).to_string(), "a");
}
