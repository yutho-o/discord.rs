//! Channel and application command payloads, checked field by field against discord-api-types
//! `payloads/v10/channel.ts` and `payloads/v10/_interactions/applicationCommands.ts`.
//!
//! The fixtures are written from those types. `round_trip` is the strict check: unknown keys are
//! ignored on read, so a field the struct lacks or misnames disappears and fails the comparison.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::{Timestamp, decode};
use dr_model::{
    ApplicationCommand, ApplicationCommandOption, ApplicationCommandOptionBound, ApplicationCommandOptionChoiceValue,
    ApplicationCommandOptionType, ApplicationCommandType, ApplicationIntegrationType, Channel, ChannelFlags, ChannelId,
    ChannelType, EntryPointCommandHandlerType, ForumLayoutType, GenericId, InteractionContextType, OverwriteType,
    PermissionOverwrite, Permissions, SortOrderType, ThreadAutoArchiveDuration, ThreadMemberFlags, ThreadMetadata,
    VideoQualityMode,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Decodes `value`, and asserts that writing it back gives the same JSON.
fn round_trip<T: DeserializeOwned + Serialize>(value: &Value) -> T {
    let decoded: T = decode(&value.to_string()).unwrap();
    assert_eq!(&serde_json::to_value(&decoded).unwrap(), value);
    decoded
}

fn channel(value: &Value) -> Channel {
    decode(&value.to_string()).unwrap()
}

fn of_kind(kind: u16) -> Channel {
    channel(&json!({"id": "1", "type": kind}))
}

// ---- channel arms, one fixture per member of the `APIChannel` union -----------------------------

fn overwrites() -> Value {
    json!([
        {"id": "41771983423143937", "type": 0, "allow": "1024", "deny": "2048"},
        {"id": "41771983423143938", "type": 1, "allow": "0", "deny": "4503599627370496"}
    ])
}

#[test]
fn guild_text_channel() {
    let c: Channel = round_trip(&json!({
        "id": "41771983423143937",
        "type": 0,
        "flags": 16,
        "guild_id": "41771983423143936",
        "name": "general",
        "position": 6,
        "topic": "24/7 chat about how to gank Mike #2",
        "nsfw": true,
        "parent_id": "399942396007890945",
        "last_message_id": "155117677105512449",
        "last_pin_timestamp": "2021-01-01T00:00:00.123456+00:00",
        "rate_limit_per_user": 2,
        "permission_overwrites": overwrites(),
        "default_auto_archive_duration": 1440,
        "default_thread_rate_limit_per_user": 30
    }));
    assert_eq!(c.kind, ChannelType::GUILD_TEXT);
    assert_eq!(c.flags, Some(ChannelFlags::REQUIRE_TAG));
    assert_eq!(c.name.as_deref(), Some("general"));
    assert_eq!(c.position, Some(6));
    assert!(c.nsfw);
    assert_eq!(c.rate_limit_per_user, Some(2));
    assert_eq!(c.default_auto_archive_duration, Some(ThreadAutoArchiveDuration::ONE_DAY));
    assert_eq!(c.default_thread_rate_limit_per_user, Some(30));
    assert_eq!(c.last_pin_timestamp, Timestamp::parse("2021-01-01T00:00:00.123456+00:00"));

    let [role, member] = c.permission_overwrites.as_slice() else { panic!("two overwrites") };
    assert_eq!(
        role,
        &PermissionOverwrite {
            id: GenericId::new(41_771_983_423_143_937),
            kind: OverwriteType::ROLE,
            allow: Permissions::VIEW_CHANNEL,
            deny: Permissions::SEND_MESSAGES,
        }
    );
    assert_eq!(member.kind, OverwriteType::MEMBER);
    assert_eq!(member.allow, Permissions::empty());
    // Bit 52 is known or not depending on the build. It must survive either way.
    assert_eq!(member.deny.bits(), 1 << 52);
}

#[test]
fn guild_announcement_channel() {
    let c: Channel = round_trip(&json!({
        "id": "41771983423143937",
        "type": 5,
        "guild_id": "41771983423143936",
        "name": "news",
        "position": 1,
        "topic": "announcements",
        "nsfw": true,
        "last_message_id": "155117677105512449"
    }));
    assert_eq!(c.kind, ChannelType::GUILD_ANNOUNCEMENT);
    assert_eq!(c.kind, ChannelType::GUILD_NEWS);
}

#[test]
fn guild_voice_channel() {
    let c: Channel = round_trip(&json!({
        "id": "155101607195836416",
        "type": 2,
        "guild_id": "41771983423143937",
        "name": "ROCKET CHEESE",
        "position": 5,
        "nsfw": true,
        "parent_id": "399942396007890945",
        "last_message_id": "155117677105512449",
        "rate_limit_per_user": 5,
        "bitrate": 64000,
        "user_limit": 10,
        "rtc_region": "brazil",
        "video_quality_mode": 2
    }));
    assert_eq!(c.bitrate, Some(64_000));
    assert_eq!(c.user_limit, Some(10));
    assert_eq!(c.rtc_region.as_deref(), Some("brazil"));
    assert_eq!(c.video_quality_mode, Some(VideoQualityMode::FULL));
    assert_eq!(c.video_quality(), VideoQualityMode::FULL);
}

#[test]
fn guild_stage_channel() {
    let c: Channel = round_trip(&json!({
        "id": "155101607195836416",
        "type": 13,
        "guild_id": "41771983423143937",
        "name": "stage",
        "position": 5,
        "bitrate": 40000,
        "user_limit": 0,
        "video_quality_mode": 1
    }));
    assert_eq!(c.kind, ChannelType::GUILD_STAGE_VOICE);
    assert_eq!(c.user_limit, Some(0));
}

#[test]
fn video_quality_is_auto_when_absent() {
    // "`1` when not present", payloads/v10/channel.ts.
    let c = of_kind(2);
    assert_eq!(c.video_quality_mode, None);
    assert_eq!(c.video_quality(), VideoQualityMode::AUTO);
}

#[test]
fn guild_category_channel() {
    let c: Channel = round_trip(&json!({
        "id": "399942396007890945",
        "type": 4,
        "guild_id": "290926798626357999",
        "name": "Test",
        "position": 0,
        "permission_overwrites": overwrites(),
        "nsfw": true
    }));
    assert_eq!(c.kind, ChannelType::GUILD_CATEGORY);
    assert_eq!(c.parent_id, None);
}

#[test]
fn dm_channel() {
    let v = json!({
        "id": "319674150115610528",
        "type": 1,
        "last_message_id": "3343820033257021450",
        "last_pin_timestamp": "2021-01-01T00:00:00+00:00",
        "recipients": [{"id": "82198898841029460", "username": "test", "discriminator": "9999", "avatar": "33ecab261d4681afa4d85a04691c4a01"}]
    });
    let c = channel(&v);
    assert_eq!(c.kind, ChannelType::DM);
    assert_eq!(c.name, None);
    let [recipient] = c.recipients.as_slice() else { panic!("one recipient") };
    assert_eq!(recipient.username, "test");
    assert_eq!(c.last_message_id.map(|i| i.get()), Some(3_343_820_033_257_021_450));

    // `name: null` is how Discord sends a DM, `APIDMChannel`.
    let named_null = channel(&json!({"id": "1", "type": 1, "name": null}));
    assert_eq!(named_null.name, None);
}

#[test]
fn group_dm_channel() {
    let c: Channel = round_trip(&json!({
        "id": "319674150115610528",
        "type": 3,
        "name": "Some test channel",
        "icon": "e1b0d3e3e1bb5f0d5e1c1d6f00000000",
        "owner_id": "82198810841029460",
        "application_id": "82198810841029461",
        "managed": true,
        "last_message_id": "3343820033257021450"
    }));
    assert_eq!(c.kind, ChannelType::GROUP_DM);
    assert_eq!(c.icon.as_deref(), Some("e1b0d3e3e1bb5f0d5e1c1d6f00000000"));
    assert_eq!(c.owner_id.map(|i| i.get()), Some(82_198_810_841_029_460));
    assert_eq!(c.application_id.map(|i| i.get()), Some(82_198_810_841_029_461));
    assert!(c.managed);
}

#[test]
fn thread_channel() {
    let c: Channel = round_trip(&json!({
        "id": "41771983423143937",
        "type": 11,
        "flags": 2,
        "guild_id": "41771983423143936",
        "name": "don't buy dota-2",
        "parent_id": "399942396007890945",
        "owner_id": "82198810841029460",
        "last_message_id": "155117677105512449",
        "last_pin_timestamp": "2021-01-01T00:00:00.000000+00:00",
        "rate_limit_per_user": 2,
        "message_count": 1,
        "member_count": 5,
        "total_message_sent": 9,
        "applied_tags": ["1001", "1002"],
        "thread_metadata": {
            "archived": true,
            "auto_archive_duration": 4320,
            "archive_timestamp": "2021-04-12T23:40:39.855793+00:00",
            "locked": true,
            "invitable": true,
            "create_timestamp": "2021-04-11T23:40:39.855793+00:00"
        },
        "member": {
            "id": "41771983423143937",
            "user_id": "82198810841029460",
            "join_timestamp": "2021-04-12T23:40:39.855793+00:00",
            "flags": 9
        }
    }));
    assert_eq!(c.flags, Some(ChannelFlags::PINNED));
    assert_eq!(c.message_count, Some(1));
    assert_eq!(c.member_count, Some(5));
    assert_eq!(c.total_message_sent, Some(9));
    assert_eq!(c.applied_tags, [GenericId::new(1001), GenericId::new(1002)]);

    let meta = c.thread_metadata.expect("metadata");
    assert_eq!(
        meta,
        ThreadMetadata {
            archived: true,
            auto_archive_duration: Some(ThreadAutoArchiveDuration::THREE_DAYS),
            archive_timestamp: Timestamp::parse("2021-04-12T23:40:39.855793+00:00"),
            locked: true,
            invitable: Some(true),
            create_timestamp: Timestamp::parse("2021-04-11T23:40:39.855793+00:00"),
        }
    );

    let member = c.member.expect("thread member");
    assert_eq!(member.id, Some(ChannelId::new(41_771_983_423_143_937)));
    assert_eq!(member.user_id.map(|i| i.get()), Some(82_198_810_841_029_460));
    assert_eq!(member.join_timestamp, Timestamp::parse("2021-04-12T23:40:39.855793+00:00"));
    assert_eq!(member.flags, ThreadMemberFlags::HAS_INTERACTED | ThreadMemberFlags::NO_MESSAGES);
    assert!(member.member.is_none());
}

#[test]
fn all_three_thread_kinds_are_threads() {
    for (kind, expected) in
        [(10, ChannelType::ANNOUNCEMENT_THREAD), (11, ChannelType::PUBLIC_THREAD), (12, ChannelType::PRIVATE_THREAD)]
    {
        let c = of_kind(kind);
        assert_eq!(c.kind, expected);
        assert!(c.is_thread());
    }
    assert_eq!(ChannelType::ANNOUNCEMENT_THREAD.get(), 10);
}

#[test]
fn thread_member_inside_guild_create_has_no_ids() {
    // "This field is omitted on the member sent within each thread in the `GUILD_CREATE` event".
    let c = channel(&json!({
        "id": "1",
        "type": 11,
        "member": {"join_timestamp": "2021-04-12T23:40:39.855793+00:00", "flags": 0}
    }));
    let member = c.member.expect("thread member");
    assert_eq!(member.id, None);
    assert_eq!(member.user_id, None);
    assert_eq!(member.flags, ThreadMemberFlags::empty());
}

#[test]
fn thread_member_with_guild_member_data() {
    let c = channel(&json!({
        "id": "1",
        "type": 11,
        "member": {
            "id": "1",
            "user_id": "2",
            "join_timestamp": "2021-04-12T23:40:39.855793+00:00",
            "flags": 1,
            "member": {"nick": "x", "roles": ["7"], "joined_at": "2021-04-12T23:40:39.855793+00:00"}
        }
    }));
    let member = c.member.expect("thread member").member.expect("guild member");
    assert_eq!(member.nick.as_deref(), Some("x"));
    assert_eq!(member.roles.len(), 1);
}

#[test]
fn thread_metadata_tolerates_missing_fields() {
    let c = channel(&json!({"id": "1", "type": 11, "thread_metadata": {}}));
    let meta = c.thread_metadata.expect("metadata");
    assert!(!meta.archived);
    assert!(!meta.locked);
    assert_eq!(meta.auto_archive_duration, None);
    assert_eq!(meta.archive_timestamp, None);
    assert_eq!(meta.invitable, None);
}

#[test]
fn forum_channel() {
    let c: Channel = round_trip(&json!({
        "id": "1",
        "type": 15,
        "flags": 16,
        "guild_id": "2",
        "name": "forum",
        "position": 3,
        "topic": "guidelines",
        "nsfw": true,
        "parent_id": "4",
        "last_message_id": "5",
        "rate_limit_per_user": 60,
        "last_pin_timestamp": "2021-01-01T00:00:00.000000+00:00",
        "default_auto_archive_duration": 10080,
        "default_thread_rate_limit_per_user": 120,
        "available_tags": [
            {"id": "11", "name": "bug", "moderated": true, "emoji_id": "12", "emoji_name": null},
            {"id": "13", "name": "idea", "moderated": false, "emoji_id": null, "emoji_name": "\u{1f4a1}"}
        ],
        "default_reaction_emoji": {"emoji_id": null, "emoji_name": "\u{1f44d}"},
        "default_sort_order": 1,
        "default_forum_layout": 2
    }));
    assert!(c.is_thread_only());
    assert_eq!(c.default_auto_archive_duration, Some(ThreadAutoArchiveDuration::ONE_WEEK));
    assert_eq!(c.default_sort_order, Some(SortOrderType::CREATION_DATE));
    assert_eq!(c.default_forum_layout, Some(ForumLayoutType::GALLERY_VIEW));

    let [bug, idea] = c.available_tags.as_slice() else { panic!("two tags") };
    assert_eq!(bug.id, GenericId::new(11));
    assert_eq!(bug.name, "bug");
    assert!(bug.moderated);
    assert_eq!(bug.emoji_id.map(|i| i.get()), Some(12));
    assert_eq!(bug.emoji_name, None);
    assert!(!idea.moderated);
    assert_eq!(idea.emoji_id, None);
    assert_eq!(idea.emoji_name.as_deref(), Some("\u{1f4a1}"));

    let reaction = c.default_reaction_emoji.expect("reaction");
    assert_eq!(reaction.emoji_id, None);
    assert_eq!(reaction.emoji_name.as_deref(), Some("\u{1f44d}"));
}

#[test]
fn forum_defaults_can_be_null() {
    // `default_reaction_emoji` and `default_sort_order` are `T | null`.
    let c = channel(&json!({
        "id": "1",
        "type": 15,
        "available_tags": [],
        "default_reaction_emoji": null,
        "default_sort_order": null,
        "default_forum_layout": 0
    }));
    assert!(c.available_tags.is_empty());
    assert_eq!(c.default_reaction_emoji, None);
    assert_eq!(c.default_sort_order, None);
    assert_eq!(c.default_forum_layout, Some(ForumLayoutType::NOT_SET));
}

#[test]
fn media_channel() {
    let c: Channel = round_trip(&json!({
        "id": "1",
        "type": 16,
        "guild_id": "2",
        "name": "media",
        "position": 0,
        "default_sort_order": 0
    }));
    assert_eq!(c.kind, ChannelType::GUILD_MEDIA);
    assert!(c.is_thread_only());
    assert_eq!(c.default_sort_order, Some(SortOrderType::LATEST_ACTIVITY));
}

#[test]
fn resolved_channel_permissions() {
    // `APIInteractionDataResolvedChannel`, payloads/v10/_interactions/base.ts.
    let c: Channel = round_trip(&json!({
        "id": "1",
        "type": 0,
        "name": "general",
        "permissions": "1024",
        "app_permissions": "2048"
    }));
    assert_eq!(c.permissions, Some(Permissions::VIEW_CHANNEL));
    assert_eq!(c.app_permissions, Some(Permissions::SEND_MESSAGES));
}

// ---- tolerance -----------------------------------------------------------------------------

#[test]
fn unknown_channel_kind_is_kept() {
    let c: Channel = round_trip(&json!({"id": "1", "type": 777, "name": "from the future"}));
    assert_eq!(c.kind, ChannelType(777));
    assert_eq!(c.kind.name(), None);
    assert_eq!(c.name.as_deref(), Some("from the future"));
    assert!(!c.is_text_based());
    assert!(!c.is_sendable());
    assert!(!c.is_thread());
    assert!(!c.is_voice_based());
    assert!(!c.is_dm_based());
    assert!(!c.is_thread_only());
}

#[test]
fn unknown_keys_are_ignored() {
    let c = channel(&json!({"id": "1", "type": 0, "brand_new": {"a": [1, 2]}, "other": null}));
    assert_eq!(c.kind, ChannelType::GUILD_TEXT);
}

#[test]
fn unknown_flag_bits_and_enum_values_are_kept() {
    let c: Channel = round_trip(&json!({
        "id": "1",
        "type": 15,
        "flags": (1u64 << 40) | 2,
        "default_sort_order": 9,
        "default_forum_layout": 9,
        "video_quality_mode": 9,
        "default_auto_archive_duration": 61
    }));
    let flags = c.flags.expect("flags");
    assert!(flags.contains(ChannelFlags::PINNED));
    assert_eq!(flags.bits(), (1 << 40) | 2);
    assert_eq!(c.default_sort_order, Some(SortOrderType(9)));
    assert_eq!(c.default_forum_layout, Some(ForumLayoutType(9)));
    assert_eq!(c.video_quality_mode, Some(VideoQualityMode(9)));
    assert_eq!(c.default_auto_archive_duration, Some(ThreadAutoArchiveDuration(61)));
}

#[test]
fn unknown_overwrite_kind_is_kept() {
    let c = channel(&json!({
        "id": "1",
        "type": 0,
        "permission_overwrites": [{"id": "2", "type": 777, "allow": "0", "deny": "0"}]
    }));
    assert_eq!(c.permission_overwrites[0].kind, OverwriteType(777));
}

#[test]
fn overwrite_bit_sets_are_read_as_string_or_integer() {
    let c = channel(&json!({
        "id": "1",
        "type": 0,
        "permission_overwrites": [{"id": "2", "allow": 1024, "deny": "2048"}]
    }));
    let o = &c.permission_overwrites[0];
    assert_eq!(o.kind, OverwriteType::ROLE);
    assert_eq!(o.allow, Permissions::VIEW_CHANNEL);
    assert_eq!(o.deny, Permissions::SEND_MESSAGES);
}

#[test]
fn overwrite_without_a_bit_set_is_a_decode_failure() {
    let text = json!({"id": "1", "type": 0, "permission_overwrites": [{"id": "2", "type": 0, "allow": "0"}]});
    let err = decode::<Channel>(&text.to_string()).unwrap_err();
    assert_eq!(err.path, "permission_overwrites[0]");
}

#[test]
fn only_id_and_type_are_required() {
    let c = channel(&json!({"id": "1", "type": 0}));
    assert_eq!(c.name, None);
    assert_eq!(c.position, None);
    assert!(!c.nsfw);
    assert!(c.permission_overwrites.is_empty());
    assert!(c.recipients.is_empty());
    assert!(c.available_tags.is_empty());
    assert!(c.applied_tags.is_empty());
    assert!(!c.managed);

    assert!(decode::<Channel>(r#"{"type": 0}"#).is_err());
    assert!(decode::<Channel>(r#"{"id": "1"}"#).is_err());
}

#[test]
fn nullable_fields_read_null() {
    let c = channel(&json!({
        "id": "1",
        "type": 0,
        "name": null,
        "topic": null,
        "parent_id": null,
        "last_message_id": null,
        "last_pin_timestamp": null,
        "application_id": null,
        "icon": null,
        "rtc_region": null,
        "rate_limit_per_user": null,
        "flags": null,
        "permission_overwrites": null,
        "recipients": null,
        "applied_tags": null,
        "available_tags": null
    }));
    assert_eq!(c.topic, None);
    assert_eq!(c.parent_id, None);
    assert_eq!(c.last_message_id, None);
    assert_eq!(c.last_pin_timestamp, None);
    assert_eq!(c.rtc_region, None);
    assert!(c.permission_overwrites.is_empty());
    assert!(c.recipients.is_empty());
}

#[test]
fn a_kind_beyond_the_repr_is_a_decode_failure() {
    // ARCHITECTURE 7.4 rule 5: open integers are `u16`, a deliberate cost.
    assert!(decode::<Channel>(r#"{"id": "1", "type": 70000}"#).is_err());
}

#[test]
fn absent_fields_are_not_written() {
    let v = serde_json::to_value(of_kind(0)).unwrap();
    assert_eq!(v, json!({"id": "1", "type": 0}));
    assert_eq!(serde_json::to_value(Channel::partial(ChannelId::new(1), ChannelType::GUILD_TEXT)).unwrap(), v);
}

// ---- predicates ----------------------------------------------------------------------------

#[test]
fn predicate_matrix() {
    // Upstream discord.js util/Constants.js: TextBasedChannelTypes, SendableChannels,
    // ThreadChannelTypes, VoiceBasedChannelTypes. BaseChannel.js: isDMBased. Thread-only is
    // ThreadOnlyChannel, whose subclasses are ForumChannel and MediaChannel.
    //
    // kind, text based, sendable, thread, voice based, dm based, thread only
    let table: &[(u16, [bool; 6])] = &[
        (0, [true, true, false, false, false, false]),
        (1, [true, true, false, false, true, false]),
        (2, [true, true, false, true, false, false]),
        (3, [true, false, false, false, true, false]),
        (4, [false, false, false, false, false, false]),
        (5, [true, true, false, false, false, false]),
        (10, [true, true, true, false, false, false]),
        (11, [true, true, true, false, false, false]),
        (12, [true, true, true, false, false, false]),
        (13, [true, true, false, true, false, false]),
        (14, [false, false, false, false, false, false]),
        (15, [false, false, false, false, false, true]),
        (16, [false, false, false, false, false, true]),
    ];
    for (kind, expected) in table {
        let c = of_kind(*kind);
        let got = [
            c.is_text_based(),
            c.is_sendable(),
            c.is_thread(),
            c.is_voice_based(),
            c.is_dm_based(),
            c.is_thread_only(),
        ];
        assert_eq!(&got, expected, "kind {kind}");
    }
}

#[test]
fn created_at_comes_from_the_snowflake() {
    // 175928847299117063 is the example of the Discord snowflake documentation: 2016-04-30T11:18:25.796Z.
    let c = channel(&json!({"id": "175928847299117063", "type": 0}));
    assert_eq!(c.created_at_ms(), 1_462_015_105_796);
}

// ---- values against upstream enums ---------------------------------------------------------

#[test]
fn enum_and_flag_values_match_upstream() {
    assert_eq!(VideoQualityMode::AUTO.get(), 1);
    assert_eq!(VideoQualityMode::FULL.get(), 2);
    assert_eq!(OverwriteType::ROLE.get(), 0);
    assert_eq!(OverwriteType::MEMBER.get(), 1);
    assert_eq!(SortOrderType::LATEST_ACTIVITY.get(), 0);
    assert_eq!(SortOrderType::CREATION_DATE.get(), 1);
    assert_eq!(ForumLayoutType::NOT_SET.get(), 0);
    assert_eq!(ForumLayoutType::LIST_VIEW.get(), 1);
    assert_eq!(ForumLayoutType::GALLERY_VIEW.get(), 2);
    assert_eq!(ThreadAutoArchiveDuration::ONE_HOUR.get(), 60);
    assert_eq!(ThreadAutoArchiveDuration::ONE_DAY.get(), 1440);
    assert_eq!(ThreadAutoArchiveDuration::THREE_DAYS.get(), 4320);
    assert_eq!(ThreadAutoArchiveDuration::ONE_WEEK.get(), 10080);

    assert_eq!(ChannelFlags::GUILD_FEED_REMOVED.bits(), 1 << 0);
    assert_eq!(ChannelFlags::PINNED.bits(), 1 << 1);
    assert_eq!(ChannelFlags::ACTIVE_CHANNELS_REMOVED.bits(), 1 << 2);
    assert_eq!(ChannelFlags::REQUIRE_TAG.bits(), 1 << 4);
    assert_eq!(ChannelFlags::IS_SPAM.bits(), 1 << 5);
    assert_eq!(ChannelFlags::IS_GUILD_RESOURCE_CHANNEL.bits(), 1 << 7);
    assert_eq!(ChannelFlags::CLYDE_AI.bits(), 1 << 8);
    assert_eq!(ChannelFlags::IS_SCHEDULED_FOR_DELETION.bits(), 1 << 9);
    assert_eq!(ChannelFlags::HIDE_MEDIA_DOWNLOAD_OPTIONS.bits(), 1 << 15);
    assert_eq!(ChannelFlags::CHANNEL_OBFUSCATED.bits(), 1 << 17);
    assert_eq!(ChannelFlags::IS_SPOILER_CHANNEL.bits(), 1 << 21);

    assert_eq!(ThreadMemberFlags::HAS_INTERACTED.bits(), 1 << 0);
    assert_eq!(ThreadMemberFlags::ALL_MESSAGES.bits(), 1 << 1);
    assert_eq!(ThreadMemberFlags::ONLY_MENTIONS.bits(), 1 << 2);
    assert_eq!(ThreadMemberFlags::NO_MESSAGES.bits(), 1 << 3);

    assert_eq!(ApplicationIntegrationType::GUILD_INSTALL.get(), 0);
    assert_eq!(ApplicationIntegrationType::USER_INSTALL.get(), 1);
    assert_eq!(InteractionContextType::GUILD.get(), 0);
    assert_eq!(InteractionContextType::BOT_DM.get(), 1);
    assert_eq!(InteractionContextType::PRIVATE_CHANNEL.get(), 2);
    assert_eq!(EntryPointCommandHandlerType::APP_HANDLER.get(), 1);
    assert_eq!(EntryPointCommandHandlerType::DISCORD_LAUNCH_ACTIVITY.get(), 2);
}

// ---- application commands ------------------------------------------------------------------

/// Every field of `APIApplicationCommand`, with the deprecated ones.
fn full_command() -> Value {
    json!({
        "id": "1001",
        "application_id": "1002",
        "guild_id": "1003",
        "type": 1,
        "name": "permissions",
        "name_localizations": {"fr": "permissions", "de": null},
        "name_localized": "permissions",
        "description": "Get or edit permissions",
        "description_localizations": {"fr": "Voir ou modifier"},
        "description_localized": "Voir ou modifier",
        "default_member_permissions": "8",
        "dm_permission": false,
        "default_permission": true,
        "nsfw": true,
        "integration_types": [0, 1],
        "contexts": [0, 1, 2],
        "version": "1004",
        "handler": 2,
        "options": []
    })
}

#[test]
fn full_application_command() {
    let mut v = full_command();
    // An empty list is not written back.
    v.as_object_mut().unwrap().remove("options");
    let c: ApplicationCommand = round_trip(&v);
    assert_eq!(c.id.get(), 1001);
    assert_eq!(c.application_id.get(), 1002);
    assert_eq!(c.guild_id.map(|i| i.get()), Some(1003));
    assert_eq!(c.kind, Some(ApplicationCommandType::CHAT_INPUT));
    assert_eq!(c.name, "permissions");
    let names = c.name_localizations.as_ref().expect("names");
    assert_eq!(names["fr"].as_deref(), Some("permissions"));
    assert_eq!(names["de"], None);
    assert_eq!(c.name_localized.as_deref(), Some("permissions"));
    assert_eq!(c.description_localized.as_deref(), Some("Voir ou modifier"));
    assert_eq!(c.default_member_permissions, Some(Permissions::ADMINISTRATOR));
    assert_eq!(c.dm_permission, Some(false));
    assert_eq!(c.default_permission, Some(true));
    assert!(c.nsfw);
    assert_eq!(
        c.integration_types,
        Some(vec![ApplicationIntegrationType::GUILD_INSTALL, ApplicationIntegrationType::USER_INSTALL])
    );
    assert_eq!(
        c.contexts,
        Some(vec![
            InteractionContextType::GUILD,
            InteractionContextType::BOT_DM,
            InteractionContextType::PRIVATE_CHANNEL
        ])
    );
    assert_eq!(c.version, Some(GenericId::new(1004)));
    assert_eq!(c.handler, Some(EntryPointCommandHandlerType::DISCORD_LAUNCH_ACTIVITY));
}

#[test]
fn default_member_permissions_null_and_zero_do_not_collapse() {
    let base = json!({"id": "1", "application_id": "2", "name": "x", "description": ""});

    let mut null = base.clone();
    null["default_member_permissions"] = Value::Null;
    let c: ApplicationCommand = round_trip(&null);
    assert_eq!(c.default_member_permissions, None);

    let mut zero = base.clone();
    zero["default_member_permissions"] = json!("0");
    let c: ApplicationCommand = round_trip(&zero);
    assert_eq!(c.default_member_permissions, Some(Permissions::empty()));

    // Upstream declares it required, so absent is an oddity that still decodes, as "default".
    let absent: ApplicationCommand = decode(&base.to_string()).unwrap();
    assert_eq!(absent.default_member_permissions, None);
    assert_eq!(serde_json::to_value(&absent).unwrap()["default_member_permissions"], Value::Null);
}

#[test]
fn context_menu_command_has_an_empty_description() {
    let v = json!({
        "id": "1",
        "application_id": "2",
        "type": 2,
        "name": "High Five",
        "description": "",
        "default_member_permissions": null,
        "version": "3"
    });
    let c: ApplicationCommand = round_trip(&v);
    assert_eq!(c.kind_or_default(), ApplicationCommandType::USER);
    assert!(c.description.is_empty());
    assert!(c.options.is_empty());
}

#[test]
fn absent_command_kind_means_chat_input() {
    let c: ApplicationCommand = decode(r#"{"id": "1", "application_id": "2", "name": "x"}"#).unwrap();
    assert_eq!(c.kind, None);
    assert_eq!(c.kind_or_default(), ApplicationCommandType::CHAT_INPUT);
}

#[test]
fn unknown_command_kind_is_kept() {
    let v = json!({
        "id": "1",
        "application_id": "2",
        "type": 99,
        "name": "x",
        "description": "",
        "default_member_permissions": null
    });
    let c: ApplicationCommand = round_trip(&v);
    assert_eq!(c.kind, Some(ApplicationCommandType(99)));
    assert_eq!(c.kind_or_default(), ApplicationCommandType(99));
}

#[test]
fn contexts_can_be_null() {
    // `contexts?: InteractionContextType[] | null`.
    let c: ApplicationCommand =
        decode(r#"{"id": "1", "application_id": "2", "name": "x", "contexts": null, "options": null}"#).unwrap();
    assert_eq!(c.contexts, None);
    assert!(c.options.is_empty());
}

#[test]
fn id_and_application_id_are_required() {
    assert!(decode::<ApplicationCommand>(r#"{"application_id": "2", "name": "x"}"#).is_err());
    assert!(decode::<ApplicationCommand>(r#"{"id": "1", "name": "x"}"#).is_err());
}

#[test]
fn command_creation_time_comes_from_the_snowflake() {
    let c: ApplicationCommand = decode(r#"{"id": "175928847299117063", "application_id": "2"}"#).unwrap();
    assert_eq!(c.created_at_ms(), 1_462_015_105_796);
}

/// Every option kind. Each object holds what its arm of the upstream union allows.
fn option_tree() -> Value {
    json!([
        {
            "type": 2,
            "name": "group",
            "name_localizations": {"fr": "groupe"},
            "name_localized": "groupe",
            "description": "a group",
            "description_localizations": {"fr": "un groupe"},
            "description_localized": "un groupe",
            "options": [{
                "type": 1,
                "name": "sub",
                "description": "a subcommand",
                "options": [
                    {
                        "type": 3,
                        "name": "text",
                        "description": "a string",
                        "required": true,
                        "min_length": 1,
                        "max_length": 6000,
                        "choices": [{
                            "name": "one",
                            "name_localizations": {"fr": "un", "de": null},
                            "name_localized": "un",
                            "value": "1"
                        }]
                    },
                    {"type": 3, "name": "search", "description": "autocompleted", "autocomplete": true},
                    {
                        "type": 4,
                        "name": "int",
                        "description": "an integer",
                        "min_value": -5,
                        "max_value": 9007199254740991_i64,
                        "choices": [{"name": "two", "value": 2}]
                    },
                    {"type": 5, "name": "bool", "description": "a boolean"},
                    {"type": 6, "name": "user", "description": "a user"},
                    {"type": 7, "name": "channel", "description": "a channel", "channel_types": [0, 11, 999]},
                    {"type": 8, "name": "role", "description": "a role"},
                    {"type": 9, "name": "mention", "description": "a mentionable"},
                    {
                        "type": 10,
                        "name": "num",
                        "description": "a number",
                        "min_value": 0.5,
                        "max_value": 2.5,
                        "choices": [{"name": "half", "value": 0.5}]
                    },
                    {"type": 11, "name": "file", "description": "an attachment", "file_types": ["image", ".png"]}
                ]
            }]
        },
        {"type": 99, "name": "new", "description": "a kind from the future", "unheard_of": [1]}
    ])
}

fn command_with_options() -> ApplicationCommand {
    let text = json!({"id": "1", "application_id": "2", "name": "x", "options": option_tree()});
    decode(&text.to_string()).unwrap()
}

fn find<'a>(options: &'a [ApplicationCommandOption], name: &str) -> &'a ApplicationCommandOption {
    options.iter().find(|o| o.name == name).unwrap_or_else(|| panic!("option {name}"))
}

#[test]
fn option_tree_round_trips_every_kind() {
    let mut tree = option_tree();
    // Unknown keys are not kept, by design.
    tree[1].as_object_mut().unwrap().remove("unheard_of");
    let _: Vec<ApplicationCommandOption> = round_trip(&tree);
}

#[test]
fn option_tree_nests_group_subcommand_option() {
    let c = command_with_options();
    let group = find(&c.options, "group");
    assert_eq!(group.kind, ApplicationCommandOptionType::SUB_COMMAND_GROUP);
    assert_eq!(group.name_localized.as_deref(), Some("groupe"));
    assert_eq!(group.description_localized.as_deref(), Some("un groupe"));
    assert_eq!(group.name_localizations.as_ref().expect("names")["fr"].as_deref(), Some("groupe"));
    assert_eq!(group.description_localizations.as_ref().expect("descriptions")["fr"].as_deref(), Some("un groupe"));
    let sub = find(&group.options, "sub");
    assert_eq!(sub.kind, ApplicationCommandOptionType::SUB_COMMAND);
    assert_eq!(sub.options.len(), 10);
}

#[test]
fn unknown_option_kind_is_kept() {
    let c = command_with_options();
    let new = find(&c.options, "new");
    assert_eq!(new.kind, ApplicationCommandOptionType(99));
    assert_eq!(new.description, "a kind from the future");
}

#[test]
fn string_option_fields() {
    let c = command_with_options();
    let sub = &c.options[0].options[0];
    let text = find(&sub.options, "text");
    assert!(text.required);
    assert_eq!(text.min_length, Some(1));
    assert_eq!(text.max_length, Some(6000));
    let [one] = text.choices.as_slice() else { panic!("one choice") };
    assert_eq!(one.name, "one");
    assert_eq!(one.name_localized.as_deref(), Some("un"));
    assert_eq!(one.value, ApplicationCommandOptionChoiceValue::String("1".into()));
    let names = one.name_localizations.as_ref().expect("names");
    assert_eq!(names["fr"].as_deref(), Some("un"));
    assert_eq!(names["de"], None);

    let search = find(&sub.options, "search");
    assert!(search.autocomplete);
    assert!(search.choices.is_empty());
    assert!(!search.required);
}

#[test]
fn numeric_option_fields_keep_their_form() {
    let c = command_with_options();
    let sub = &c.options[0].options[0];

    let int = find(&sub.options, "int");
    assert_eq!(int.min_value, Some(ApplicationCommandOptionBound::Integer(-5)));
    assert_eq!(int.max_value, Some(ApplicationCommandOptionBound::Integer(9_007_199_254_740_991)));
    assert_eq!(int.choices[0].value, ApplicationCommandOptionChoiceValue::Integer(2));

    let num = find(&sub.options, "num");
    assert_eq!(num.min_value, Some(ApplicationCommandOptionBound::Number(0.5)));
    assert_eq!(num.max_value.map(ApplicationCommandOptionBound::as_f64), Some(2.5));
    assert_eq!(num.choices[0].value, ApplicationCommandOptionChoiceValue::Number(0.5));

    assert_eq!(ApplicationCommandOptionBound::Integer(-5).as_f64(), -5.0);
}

#[test]
fn channel_and_attachment_option_fields() {
    let c = command_with_options();
    let sub = &c.options[0].options[0];
    let channel = find(&sub.options, "channel");
    // A channel kind this build does not know does not lose the command.
    assert_eq!(channel.channel_types, [ChannelType::GUILD_TEXT, ChannelType::PUBLIC_THREAD, ChannelType(999)]);
    assert_eq!(find(&sub.options, "file").file_types, ["image", ".png"]);
}

#[test]
fn option_lists_read_null() {
    let o: ApplicationCommandOption = decode(
        r#"{"type": 3, "name": "x", "description": "y",
            "options": null, "choices": null, "channel_types": null, "file_types": null,
            "name_localizations": null, "description_localizations": null,
            "min_value": null, "max_value": null, "min_length": null, "max_length": null}"#,
    )
    .unwrap();
    assert!(o.options.is_empty());
    assert!(o.choices.is_empty());
    assert!(o.channel_types.is_empty());
    assert!(o.file_types.is_empty());
    assert_eq!(o.name_localizations, None);
    assert_eq!(o.min_value, None);
}

#[test]
fn option_needs_a_kind() {
    assert!(decode::<ApplicationCommandOption>(r#"{"name": "x", "description": "y"}"#).is_err());
}

#[test]
fn bulk_overwrite_response_decodes() {
    let list: Vec<ApplicationCommand> = decode(
        r#"[{"id": "1", "application_id": "2", "type": 1, "name": "ping", "description": "Ping",
             "default_member_permissions": null, "version": "3", "dm_permission": true, "contexts": null,
             "integration_types": [0], "nsfw": false},
            {"id": "4", "application_id": "2", "guild_id": "5", "type": 3, "name": "Quote", "description": "",
             "default_member_permissions": "0", "version": "6"}]"#,
    )
    .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].kind, Some(ApplicationCommandType::CHAT_INPUT));
    assert_eq!(list[1].kind, Some(ApplicationCommandType::MESSAGE));
    assert_eq!(list[1].default_member_permissions, Some(Permissions::empty()));
}
