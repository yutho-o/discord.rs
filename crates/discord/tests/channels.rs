//! Channel creation, edits, overwrites and positions, and permissions read from the cache.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use discord::builders::MAX_SLOW_MODE;
use discord::model::DispatchKind as K;
use discord::model::GenericId;
use discord::prelude::*;
use dr_mock::{Method, MockServer, RecordedRequest};
use serde_json::{Value, json};

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

fn last(server: &MockServer) -> RecordedRequest {
    server.requests().pop().unwrap()
}

#[tokio::test]
async fn a_channel_can_be_created_edited_and_deleted() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let everyone = RoleId::new(guild.get());
    let body = CreateChannel::new("staff")
        .topic("Moderators only")
        .rate_limit_per_user(10)
        .permission_overwrite(PermissionOverwrite::role(everyone, Permissions::empty(), Permissions::VIEW_CHANNEL));
    let channel = client.guild(guild).create_channel(body).reason("new team").await.unwrap();

    let request = last(&server);
    assert_eq!((request.method.clone(), request.path.as_str()), (Method::POST, &*format!("/guilds/{guild}/channels")));
    assert_eq!(request.header("x-audit-log-reason"), Some("new%20team"));
    assert_eq!(
        request.json(),
        json!({
            "name": "staff",
            "topic": "Moderators only",
            "rate_limit_per_user": 10,
            "permission_overwrites": [{"id": guild.to_string(), "type": 0, "allow": "0", "deny": "1024"}],
        })
    );
    assert_eq!(channel.name.as_deref(), Some("staff"));
    assert_eq!(channel.permission_overwrites.len(), 1);

    let edited = channel.handle().edit(EditChannel::new().name("team").clear_topic()).await.unwrap();
    assert_eq!(last(&server).json(), json!({"name": "team", "topic": null}), "an edit writes only what was asked for");
    assert_eq!(edited.name.as_deref(), Some("team"));
    assert_eq!(edited.topic, None);

    let deleted = channel.handle().delete().await.unwrap();
    assert_eq!(last(&server).method, Method::DELETE);
    assert_eq!(deleted.id, channel.id);
    assert!(client.channel(channel.id).fetch().await.is_err());
}

#[tokio::test]
async fn a_voice_channel_goes_with_its_type() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let body = CreateChannel::new("Lounge").kind(ChannelType::GUILD_VOICE).bitrate(64_000).user_limit(5);
    let channel = client.guild(guild).create_channel(body).await.unwrap();
    assert_eq!(last(&server).json(), json!({"name": "Lounge", "type": 2, "bitrate": 64000, "user_limit": 5}));
    assert_eq!(channel.kind, ChannelType::GUILD_VOICE);
}

#[tokio::test]
async fn bad_channel_bodies_send_nothing() {
    let (server, client) = fixture().await;
    let guild = client.guild(1.into());
    assert!(guild.create_channel(CreateChannel::new("")).await.is_err(), "an empty name");
    assert!(guild.create_channel(CreateChannel::new("x").topic("a".repeat(1025))).await.is_err(), "a long topic");
    let forum = CreateChannel::new("x").kind(ChannelType::GUILD_FORUM).topic("a".repeat(1025));
    assert!(server.requests().is_empty());
    guild.create_channel(forum).await.ok();
    assert_eq!(server.requests().len(), 1, "a forum topic may be longer");
    let slow = EditChannel::new().rate_limit_per_user(MAX_SLOW_MODE + 1);
    assert!(client.channel(2.into()).edit(slow).await.is_err());
    assert!(client.channel(2.into()).edit(EditChannel::new().bitrate(10)).await.is_err());
    assert_eq!(server.requests().len(), 1, "nothing more was sent");
}

#[tokio::test]
async fn one_overwrite_can_be_set_and_removed() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let alice = server.world().add_user("alice");
    let handle = client.channel(channel);

    let deny = PermissionOverwrite::member(alice, Permissions::empty(), Permissions::SEND_MESSAGES);
    handle.set_permission(deny).reason("spam").await.unwrap();
    let request = last(&server);
    assert_eq!(
        (request.method.clone(), request.path.as_str()),
        (Method::PUT, &*format!("/channels/{channel}/permissions/{alice}"))
    );
    assert_eq!(request.json(), json!({"type": 1, "allow": "0", "deny": "2048"}));
    assert_eq!(handle.fetch().await.unwrap().permission_overwrites.len(), 1);

    handle.remove_permission(GenericId::new(alice.get())).await.unwrap();
    assert_eq!(last(&server).method, Method::DELETE);
    assert!(handle.fetch().await.unwrap().permission_overwrites.is_empty());
}

#[tokio::test]
async fn channels_move_together() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let a = server.world().add_channel(guild, "a");
    let b = server.world().add_channel(guild, "b");
    let positions = vec![ChannelPosition::new(a, 1), ChannelPosition::new(b, 0).lock_permissions(true)];
    client.guild(guild).set_channel_positions(positions).await.unwrap();
    let request = last(&server);
    assert_eq!(request.method, Method::PATCH);
    assert_eq!(
        request.json(),
        json!([{"id": a.to_string(), "position": 1}, {"id": b.to_string(), "position": 0, "lock_permissions": true}])
    );
    assert_eq!(client.channel(b).fetch().await.unwrap().position, Some(0));
}

/// A guild in the cache: `@everyone` can read and write, `mod` can also manage messages, the
/// `#staff` channel hides from `@everyone` and shows to `mod`, and `#rules` is read-only for
/// `@everyone`. The bot (id 9) is a member with `mod`, alice (id 5) has no role.
async fn cached_guild() -> (MockServer, Client) {
    let (server, client) = fixture().await;
    let user = |id: &str, name: &str| json!({"id": id, "username": name});
    let ready = json!({
        "v": 10, "user": user("9", "bot"), "guilds": [], "session_id": "s", "resume_gateway_url": "ws://x",
        "application": {"id": "9", "flags": 0},
    });
    let role = |id: &str, permissions: u64| {
        json!({"id": id, "name": id, "color": 0, "hoist": false, "position": 0, "permissions": permissions.to_string(),
               "managed": false, "mentionable": false})
    };
    let (view, send, manage) = (1u64 << 10, 1u64 << 11, 1u64 << 13);
    let deny_view = json!({"id": "1", "type": 0, "allow": "0", "deny": view.to_string()});
    let guild = json!({
        "id": "1", "name": "g", "owner_id": "77",
        "roles": [role("1", view | send), role("20", manage)],
        "channels": [
            {"id": "2", "type": 0, "guild_id": "1", "name": "general"},
            {"id": "3", "type": 0, "guild_id": "1", "name": "staff",
             "permission_overwrites": [deny_view, {"id": "20", "type": 0, "allow": view.to_string(), "deny": "0"}]},
            {"id": "4", "type": 0, "guild_id": "1", "name": "rules",
             "permission_overwrites": [{"id": "1", "type": 0, "allow": "0", "deny": send.to_string()}]},
        ],
        "members": [
            {"user": user("9", "bot"), "roles": ["20"], "joined_at": "2020-01-01T00:00:00+00:00"},
            {"user": user("5", "alice"), "roles": [], "joined_at": "2020-01-01T00:00:00+00:00"},
        ],
    });
    let thread = json!({"id": "6", "type": 11, "guild_id": "1", "parent_id": "3", "name": "t"});
    let events: [(K, &str, Value); 3] = [
        (K::Ready, "READY", ready),
        (K::GuildCreate, "GUILD_CREATE", guild),
        (K::ChannelCreate, "CHANNEL_CREATE", thread),
    ];
    for (kind, name, body) in events {
        discord::events::deliver(&client, 0, kind, name, &body.to_string()).await;
    }
    (server, client)
}

#[tokio::test]
async fn permissions_come_from_the_cache() {
    let (_server, client) = cached_guild().await;
    let (alice, bot) = (UserId::new(5), UserId::new(9));
    let (general, staff, rules, thread) = (ChannelId::new(2), ChannelId::new(3), ChannelId::new(4), ChannelId::new(6));
    let can = |channel: ChannelId, user: UserId, p: Permissions| {
        client.channel(channel).permissions_for(user).unwrap().contains(p)
    };

    assert!(can(general, alice, Permissions::SEND_MESSAGES));
    assert!(!can(general, alice, Permissions::MANAGE_MESSAGES));
    assert!(!can(staff, alice, Permissions::VIEW_CHANNEL), "the channel hides from @everyone");
    assert!(can(staff, bot, Permissions::VIEW_CHANNEL), "and shows to mod");
    assert!(!can(rules, alice, Permissions::SEND_MESSAGES));
    assert!(can(rules, alice, Permissions::VIEW_CHANNEL));
    assert!(!can(thread, alice, Permissions::VIEW_CHANNEL), "a thread uses its parent's overwrites");
    assert!(can(thread, bot, Permissions::VIEW_CHANNEL));

    assert_eq!(client.channel(staff).my_permissions(), client.channel(staff).permissions_for(bot));
    let guild_wide = client.guild(1.into()).my_permissions().unwrap();
    assert!(guild_wide.contains(Permissions::MANAGE_MESSAGES | Permissions::SEND_MESSAGES));
    assert!(client.channel(ChannelId::new(99)).permissions_for(alice).is_none(), "an unknown channel");
    assert!(client.channel(general).permissions_for(UserId::new(42)).is_none(), "an unknown member");
}

#[tokio::test]
async fn a_member_value_computes_its_own_permissions() {
    let (server, client) = cached_guild().await;
    let guild = GuildId::new(1);
    server.world().add_guild("unused");
    let alice = client.guild(guild).members().get(UserId::new(5)).unwrap();
    assert!(alice.permissions().unwrap().contains(Permissions::SEND_MESSAGES));
    assert!(!alice.permissions_in(ChannelId::new(3)).unwrap().contains(Permissions::VIEW_CHANNEL));
    assert!(alice.permissions_in(ChannelId::new(99)).is_none());
}
