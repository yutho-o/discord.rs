//! The apply functions, from wire JSON to what the reads return.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_cache::{Cache, CacheConfig, Limit};
use dr_model::{
    Channel, Guild, GuildId, GuildMemberRemoveEvent, GuildMemberUpdateEvent, Member, Message, MessageDeleteEvent,
    MessageUpdateEvent, ReadyEvent, UnavailableGuild,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;

fn from<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn guild_json() -> Value {
    json!({
        "id": "1", "name": "g",
        "channels": [{"id": "10", "type": 0}, {"id": "11", "type": 0}],
        "roles": [{"id": "1", "name": "@everyone", "color": 0, "hoist": false, "position": 0, "permissions": "0", "managed": false, "mentionable": false}],
        "members": [{"user": {"id": "100", "username": "alice"}, "roles": []}],
    })
}

fn message_json(id: u64) -> Value {
    json!({"id": id.to_string(), "channel_id": "10", "guild_id": "1", "author": {"id": "100", "username": "alice"}, "content": "hi"})
}

fn ready_json() -> Value {
    json!({"v": 10, "user": {"id": "999", "username": "bot"}, "guilds": [], "session_id": "s", "resume_gateway_url": "ws://x", "application": {"id": "9"}})
}

fn cached_guild() -> Cache {
    let cache = Cache::new(CacheConfig::standard().messages_per_channel(2));
    cache.apply_guild_create(0, from::<Guild>(guild_json()));
    cache
}

#[test]
fn guild_create_spreads_into_the_tables() {
    let cache = Cache::new(CacheConfig::standard());
    let payload = cache.apply_guild_create(0, from::<Guild>(guild_json()));
    assert_eq!(payload.channels.len(), 2, "the event keeps its lists");
    let g = GuildId::new(1);
    let stored = cache.guild(g).unwrap();
    assert!(stored.channels.is_empty() && stored.members.is_empty() && stored.roles.is_empty(), "the copy does not");
    assert_eq!(cache.guild_ids(), vec![g]);
    assert_eq!(cache.guild_channels(g).len(), 2);
    assert_eq!(cache.guild_roles(g).len(), 1);
    assert_eq!(cache.channel(10.into()).unwrap().guild_id, Some(g), "filled in on the way");
    let member = cache.member(g, 100.into()).unwrap();
    assert_eq!(member.guild_id, Some(g));
    assert_eq!(cache.user(100.into()).unwrap().username, "alice");
}

#[test]
fn guild_update_keeps_the_tables_and_fields_only_create_sends() {
    let cache = Cache::new(CacheConfig::standard());
    let mut body = guild_json();
    body["joined_at"] = json!("2020-01-01T00:00:00.000000+00:00");
    cache.apply_guild_create(0, from::<Guild>(body));
    let change = cache.apply_guild_update(0, from::<Guild>(json!({"id": "1", "name": "renamed"})));
    assert_eq!(change.old.unwrap().name, "g");
    assert_eq!(change.new.name, "renamed");
    let stored = cache.guild(1.into()).unwrap();
    assert_eq!(stored.name, "renamed");
    assert!(stored.joined_at.is_some());
    assert_eq!(cache.guild_channels(1.into()).len(), 2);
}

#[test]
fn the_guild_delete_matrix() {
    // An outage: the guild stays, marked unavailable, and so do its tables.
    let cache = cached_guild();
    let event: UnavailableGuild = from(json!({"id": "1", "unavailable": true}));
    assert!(cache.apply_guild_delete(0, event).is_some());
    assert!(cache.guild(1.into()).unwrap().unavailable);
    assert!(cache.channel(10.into()).is_some());

    // A removal: everything under it goes.
    let cache = cached_guild();
    cache.apply_message_create(0, from::<Message>(message_json(1)));
    let event: UnavailableGuild = from(json!({"id": "1"}));
    assert_eq!(cache.apply_guild_delete(0, event).unwrap().name, "g");
    assert!(cache.guild(1.into()).is_none());
    assert!(cache.channel(10.into()).is_none());
    assert!(cache.channel_messages(10.into()).is_empty());
    assert!(cache.guild_ids().is_empty());

    // An unknown guild: nothing to return, and no failure.
    let cache = Cache::new(CacheConfig::standard());
    assert!(cache.apply_guild_delete(0, from(json!({"id": "1"}))).is_none());
    assert!(cache.apply_guild_delete(0, from(json!({"id": "1", "unavailable": true}))).is_none());
}

#[test]
fn a_guild_coming_back_replaces_its_channels_and_keeps_messages_of_those_left() {
    let cache = cached_guild();
    cache.apply_message_create(0, from::<Message>(message_json(1)));
    let mut body = guild_json();
    body["channels"] = json!([{"id": "10", "type": 0}]);
    cache.apply_guild_create(0, from::<Guild>(body));
    assert!(cache.channel(11.into()).is_none());
    assert!(cache.channel(10.into()).is_some());
    assert_eq!(cache.channel_messages(10.into()).len(), 1);
}

#[test]
fn channels_update_and_delete() {
    let cache = cached_guild();
    let update: Channel = from(json!({"id": "10", "type": 0, "guild_id": "1", "name": "new"}));
    let change = cache.apply_channel_update(0, update);
    assert!(change.old.is_some());
    assert_eq!(cache.channel(10.into()).unwrap().name.as_deref(), Some("new"));
    let same = cache.guild_channels(1.into()).into_iter().find(|c| c.id == 10.into()).unwrap();
    assert!(Arc::ptr_eq(&same, &change.new), "the guild's table has the same value");

    cache.apply_message_create(0, from::<Message>(message_json(1)));
    let gone = cache.apply_channel_delete(0, from(json!({"id": "10", "type": 0, "guild_id": "1"})));
    assert!(gone.is_some());
    assert!(cache.channel(10.into()).is_none());
    assert_eq!(cache.guild_channels(1.into()).len(), 1);
    assert!(cache.channel_messages(10.into()).is_empty());
}

#[test]
fn dm_channels_stop_at_their_limit() {
    let cache = Cache::new(CacheConfig::standard().dm_channels(2));
    for id in [1, 2, 3] {
        cache.apply_channel_create(0, from::<Channel>(json!({"id": id.to_string(), "type": 1})));
    }
    assert!(cache.channel(1.into()).is_none(), "the oldest went");
    assert!(cache.channel(2.into()).is_some());
    assert!(cache.channel(3.into()).is_some());
}

#[test]
fn member_update_applies_each_of_the_three_states() {
    let cache = Cache::new(CacheConfig::standard());
    let member: Member = from(
        json!({"guild_id": "1", "user": {"id": "5", "username": "u"}, "nick": "old", "avatar": "a", "roles": [], "pending": true}),
    );
    cache.apply_member_add(0, member);
    let event: GuildMemberUpdateEvent = from(
        json!({"guild_id": "1", "user": {"id": "5", "username": "u"}, "roles": ["7"], "nick": null, "pending": false}),
    );
    let change = cache.apply_member_update(0, event);
    let old = change.old.unwrap();
    assert_eq!(old.nick.as_deref(), Some("old"));
    assert_eq!(change.new.nick, None, "null clears");
    assert_eq!(change.new.avatar.as_deref(), Some("a"), "absent keeps");
    assert_eq!(change.new.roles.len(), 1);
    assert!(!change.new.pending);
    assert_eq!(change.new.guild_id, Some(1.into()));

    let removed: GuildMemberRemoveEvent = from(json!({"guild_id": "1", "user": {"id": "5", "username": "u"}}));
    assert!(cache.apply_member_remove(0, removed).is_some());
    assert!(cache.member(1.into(), 5.into()).is_none());
}

#[test]
fn the_member_limits_always_keep_the_bot() {
    let cache = Cache::new(CacheConfig::standard().members(Limit::None));
    cache.apply_ready(0, from::<ReadyEvent>(ready_json()));
    for id in ["5", "999"] {
        let member: Member = from(json!({"guild_id": "1", "user": {"id": id, "username": "u"}, "roles": []}));
        cache.apply_member_add(0, member);
    }
    assert!(cache.member(1.into(), 5.into()).is_none());
    assert!(cache.member(1.into(), 999.into()).is_some());

    let cache = Cache::new(CacheConfig::standard().members(Limit::PerGuild(1)));
    for id in ["5", "6"] {
        let member: Member = from(json!({"guild_id": "1", "user": {"id": id, "username": "u"}, "roles": []}));
        cache.apply_member_add(0, member);
    }
    assert!(cache.member(1.into(), 5.into()).is_some());
    assert!(cache.member(1.into(), 6.into()).is_none());
}

#[test]
fn messages_are_a_fifo_with_a_limit() {
    let cache = cached_guild();
    for id in [1, 2, 3] {
        cache.apply_message_create(0, from::<Message>(message_json(id)));
    }
    let kept: Vec<u64> = cache.channel_messages(10.into()).iter().map(|m| m.id.get()).collect();
    assert_eq!(kept, [2, 3]);
    assert!(cache.message(10.into(), 1.into()).is_none());

    let gone: MessageDeleteEvent = from(json!({"id": "2", "channel_id": "10"}));
    assert_eq!(cache.apply_message_delete(0, gone).unwrap().id.get(), 2);
    assert_eq!(cache.channel_messages(10.into()).len(), 1);
}

#[test]
fn a_partial_message_update_merges_or_stands_alone() {
    let cache = cached_guild();
    cache.apply_message_create(0, from::<Message>(message_json(1)));
    let event: MessageUpdateEvent = from(
        json!({"id": "1", "channel_id": "10", "content": "edited", "edited_timestamp": "2020-01-01T00:00:00.000000+00:00"}),
    );
    let change = cache.apply_message_update(0, event);
    assert_eq!(change.old.unwrap().content, "hi");
    assert_eq!(change.new.content, "edited");
    assert_eq!(change.new.author.username, "alice", "the rest stays");
    assert!(change.new.edited_timestamp.is_some());
    assert_eq!(cache.message(10.into(), 1.into()).unwrap().content, "edited");

    let event: MessageUpdateEvent = from(json!({"id": "1", "channel_id": "10", "referenced_message": null}));
    let change = cache.apply_message_update(0, event);
    assert_eq!(change.new.content, "edited");
    assert!(change.new.referenced_message.is_null(), "null is kept apart from absent");

    let unseen: MessageUpdateEvent = from(json!({"id": "77", "channel_id": "10", "content": "x"}));
    let change = cache.apply_message_update(0, unseen);
    assert!(change.old.is_none());
    assert_eq!(change.new.content, "x");
    assert_eq!(change.new.id.get(), 77);
    assert!(cache.message(10.into(), 77.into()).is_none(), "a partial with no author is not stored");
}

#[test]
fn the_none_policy_stores_only_the_bot() {
    let cache = Cache::new(CacheConfig::none());
    cache.apply_ready(0, from::<ReadyEvent>(ready_json()));
    let guild = cache.apply_guild_create(0, from::<Guild>(guild_json()));
    assert_eq!(guild.name, "g", "the event is still built");
    cache.apply_message_create(0, from::<Message>(message_json(1)));
    assert_eq!(cache.me().unwrap().username, "bot");
    assert!(cache.guild(1.into()).is_none());
    assert!(cache.user(100.into()).is_none());
    assert!(cache.user(999.into()).is_none());
    assert!(cache.channel(10.into()).is_none());
    assert!(cache.message(10.into(), 1.into()).is_none());
}

#[test]
fn a_rest_result_never_replaces_gateway_state() {
    let cache = cached_guild();
    let fetched: Channel = from(json!({"id": "10", "type": 0, "guild_id": "1", "name": "stale"}));
    cache.insert_channel_if_absent(Arc::new(fetched));
    assert_eq!(cache.channel(10.into()).unwrap().name, None);

    let fresh: Channel = from(json!({"id": "20", "type": 0, "guild_id": "1"}));
    cache.insert_channel_if_absent(Arc::new(fresh));
    assert!(cache.guild_channels(1.into()).iter().any(|c| c.id == 20.into()));

    let partial = Member::default();
    cache.insert_member_if_absent(1.into(), Arc::new(partial));
    assert!(cache.member(1.into(), 0.into()).is_none(), "no user, not stored");
}
