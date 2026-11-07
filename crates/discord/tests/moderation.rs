//! Moderation, roles, message management, reactions and presence, on the mock.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::Error;
use discord::model::DispatchKind as K;
use discord::prelude::*;
use dr_mock::{Method, MockServer, RecordedRequest};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await.unwrap();
    let client = Client::builder("t").endpoints(server.endpoints()).build().unwrap().client();
    (server, client)
}

fn last(server: &MockServer) -> RecordedRequest {
    server.requests().pop().unwrap()
}

#[tokio::test]
async fn a_user_can_be_banned_listed_and_unbanned() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let alice = server.world().add_user("alice");
    let bob = server.world().add_user("bob");
    server.world().add_member(guild, alice);
    let bans = client.guild(guild).bans();

    let member = client.guild(guild).members().fetch(alice).await.unwrap();
    member.ban().reason("spam").await.unwrap();
    let request = last(&server);
    assert_eq!(
        (request.method.clone(), request.path.as_str()),
        (Method::PUT, &*format!("/guilds/{guild}/bans/{alice}"))
    );
    assert_eq!(request.header("x-audit-log-reason"), Some("spam"));
    assert!(client.guild(guild).members().fetch(alice).await.is_err(), "a ban removes the member");

    bans.create(bob, CreateBan::new().delete_message_seconds(3600)).await.unwrap();
    assert_eq!(last(&server).json(), json!({"delete_message_seconds": 3600}));

    assert_eq!(bans.fetch(alice).await.unwrap().reason.as_deref(), Some("spam"));
    assert_eq!(bans.list(UserPage::new()).await.unwrap().len(), 2);
    let page = bans.list(UserPage::new().limit(1)).await.unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(last(&server).query.as_deref(), Some("limit=1"));
    let rest = bans.list(UserPage::new().limit(5).after(page[0].user.id)).await.unwrap();
    assert_eq!(rest.len(), 1, "the page after the first");

    bans.remove(alice).await.unwrap();
    assert!(bans.fetch(alice).await.is_err());
}

#[tokio::test]
async fn a_ban_keeping_too_much_history_is_refused_before_sending() {
    let (server, client) = fixture().await;
    let result = client.guild(1.into()).bans().create(2.into(), CreateBan::new().delete_message_seconds(604_801)).await;
    assert!(result.is_err());
    assert!(server.requests().is_empty(), "nothing was sent");
}

#[tokio::test]
async fn a_timeout_writes_a_date_and_clearing_writes_null() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let alice = server.world().add_user("alice");
    server.world().add_member(guild, alice);
    let members = client.guild(guild).members();

    let member = members.timeout(alice, Duration::from_secs(600)).reason("calm down").await.unwrap();
    assert_eq!(member.guild_id, Some(guild), "the response has none");
    let body = last(&server).json();
    let until = body["communication_disabled_until"].as_str().unwrap();
    let parsed = discord::Timestamp::parse(until).unwrap_or_else(|| panic!("{until} is not a timestamp"));
    assert!(parsed.0 > 0);
    assert_eq!(body.as_object().unwrap().len(), 1, "an edit writes only what was asked for");

    member.clear_timeout().await.unwrap();
    assert_eq!(last(&server).json(), json!({"communication_disabled_until": null}));

    let sent = server.requests().len();
    assert!(members.timeout(alice, Duration::from_secs(29 * 24 * 60 * 60)).await.is_err(), "over 28 days");
    assert_eq!(server.requests().len(), sent, "nothing was sent");
    assert!(members.timeout(alice, Duration::MAX).await.is_err(), "no overflow");
}

#[tokio::test]
async fn a_member_edit_writes_only_what_was_asked_for() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let alice = server.world().add_user("alice");
    server.world().add_member(guild, alice);
    let members = client.guild(guild).members();

    members.edit(alice, EditMember::new().nick("Al")).await.unwrap();
    assert_eq!(last(&server).json(), json!({"nick": "Al"}));
    members.edit(alice, EditMember::new().clear_nick()).await.unwrap();
    assert_eq!(last(&server).json(), json!({"nick": null}));
    assert!(members.edit(alice, EditMember::new().nick("x".repeat(33))).await.is_err());

    assert_eq!(members.list(UserPage::new().limit(10)).await.unwrap()[0].guild_id, Some(guild));
}

#[tokio::test]
async fn roles_go_through_their_whole_life() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let alice = server.world().add_user("alice");
    server.world().add_member(guild, alice);
    let roles = client.guild(guild).roles();

    let body = CreateRole::new().name("Mods").color(0xFF0000).permissions(Permissions::BAN_MEMBERS).hoist(true);
    let role = roles.create(body).reason("setup").await.unwrap();
    assert_eq!(role.name, "Mods");
    let sent = last(&server).json();
    assert_eq!(sent["permissions"], "4", "permissions travel as a string");
    assert_eq!(sent["color"], 0xFF0000);

    let member = client.guild(guild).members().fetch(alice).await.unwrap();
    member.add_role(role.id).await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/guilds/{guild}/members/{alice}/roles/{}", role.id));
    assert_eq!(client.guild(guild).members().fetch(alice).await.unwrap().roles, [role.id]);
    member.remove_role(role.id).await.unwrap();
    assert!(client.guild(guild).members().fetch(alice).await.unwrap().roles.is_empty());

    let edited = roles.handle(role.id).edit(EditRole::new().name("Admins")).await.unwrap();
    assert_eq!(edited.name, "Admins");
    assert_eq!(last(&server).json(), json!({"name": "Admins"}));
    assert_eq!(roles.fetch_all().await.unwrap().len(), 1);
    roles.handle(role.id).delete().await.unwrap();
    assert!(roles.fetch_all().await.unwrap().is_empty());

    assert!(roles.create(CreateRole::new().color(0x0100_0000)).await.is_err(), "colour too large");
    assert!(roles.create(CreateRole::new().name("")).await.is_err(), "empty name");
}

#[tokio::test]
async fn a_partial_member_fails_with_nothing_sent() {
    let (server, client) = fixture().await;
    let partial: Member = serde_json::from_value(json!({"roles": []})).unwrap();
    let result = client.bind(partial).ban().await;
    assert!(matches!(result, Err(Error::Other(_))), "{result:?}");
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn messages_can_be_listed_pinned_and_bulk_deleted() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let mut sent = Vec::new();
    for text in ["one", "two", "three"] {
        sent.push(client.channel(channel).send(text).await.unwrap());
    }
    let messages = client.channel(channel).messages();

    let newest = messages.list(MessagePage::new().limit(2)).await.unwrap();
    assert_eq!(newest.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), ["three", "two"]);
    assert_eq!(last(&server).query.as_deref(), Some("limit=2"));
    let older = messages.list(MessagePage::new().before(newest[1].id)).await.unwrap();
    assert_eq!(older.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), ["one"]);

    sent[0].pin().reason("important").await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/channels/{channel}/messages/pins/{}", sent[0].id));
    assert_eq!(request.method, Method::PUT);
    assert!(messages.fetch(sent[0].id).await.unwrap().pinned);
    sent[0].unpin().await.unwrap();
    assert!(!messages.fetch(sent[0].id).await.unwrap().pinned);

    let before = server.requests().len();
    assert!(messages.bulk_delete([sent[0].id]).await.is_err(), "one message is too few");
    assert!(messages.bulk_delete((0..101).map(MessageId::from)).await.is_err(), "101 is too many");
    assert_eq!(server.requests().len(), before, "nothing was sent");

    messages.bulk_delete([sent[0].id, sent[1].id]).reason("cleanup").await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("/channels/{channel}/messages/bulk-delete"));
    assert_eq!(request.json(), json!({"messages": [sent[0].id.to_string(), sent[1].id.to_string()]}));
    assert_eq!(messages.list(MessagePage::new()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn reactions_use_encoded_paths() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let message = client.channel(channel).send("hi").await.unwrap();
    let base = format!("/channels/{channel}/messages/{}/reactions", message.id);

    message.unreact("👍").await.unwrap();
    assert_eq!(last(&server).path, format!("{base}/%F0%9F%91%8D/@me"));
    message.remove_reaction("custom:42", 7.into()).await.unwrap();
    let request = last(&server);
    assert_eq!(request.path, format!("{base}/custom%3A42/7"));
    assert_eq!(request.method, Method::DELETE);
    message.clear_reactions().await.unwrap();
    assert_eq!(last(&server).path, base);
}

#[tokio::test]
async fn presence_needs_a_running_client_and_reaches_the_gateway() {
    let server = MockServer::start().await.unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let runner = Client::builder("t")
        .endpoints(server.endpoints())
        .on(ClientReady, move |_ctx, _ready| {
            let tx = tx.clone();
            async move {
                tx.send(()).await?;
                Ok(())
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let presence = || UpdatePresence::new(PresenceStatus::DoNotDisturb).activity(Activity::watching("the server"));
    assert!(matches!(client.set_presence(presence()), Err(Error::NotReady)));

    let run = tokio::spawn(runner.run());
    rx.recv().await.unwrap();
    client.set_presence(presence()).unwrap();
    wait_until("the presence frame", || server.frames(0).iter().any(|f| f.op == 3)).await;
    let frames = server.frames(0);
    let frame = frames.iter().find(|f| f.op == 3).unwrap();
    assert_eq!(frame.d["status"], "dnd");
    assert_eq!(frame.d["activities"][0]["name"], "the server");
    assert_eq!(frame.d["activities"][0]["type"], Activity::WATCHING);

    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn the_new_events_fire_and_the_cache_follows() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let builder = Client::builder("t").cache(CacheConfig::standard().messages_per_channel(10)).intents(
        Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::GUILD_MODERATION | Intents::GUILD_MESSAGE_REACTIONS,
    );
    macro_rules! record {
        ($builder:expr, $($token:ident),*) => {{
            let builder = $builder;
            $(
                let log = seen.clone();
                let builder = builder.on($token, move |_ctx, _payload| {
                    let log = log.clone();
                    async move {
                        log.lock().unwrap().push(stringify!($token));
                        Ok(())
                    }
                });
            )*
            builder
        }};
    }
    let runner = record!(
        builder,
        MessageDeleteBulk,
        MessageReactionAdd,
        MessageReactionRemove,
        GuildBanAdd,
        GuildBanRemove,
        GuildRoleCreate,
        GuildRoleUpdate,
        GuildRoleDelete
    )
    .build()
    .unwrap();
    let client = runner.client();
    let cache = client.cache();
    let deliver = |kind, t: &'static str, body: serde_json::Value| {
        let client = client.clone();
        async move { discord::events::deliver(&client, 0, kind, t, &body.to_string()).await }
    };

    let user = json!({"id": "5", "username": "u"});
    let role = |name: &str| json!({"guild_id": "1", "role": {"id": "7", "name": name, "permissions": "0"}});
    deliver(K::GuildRoleCreate, "GUILD_ROLE_CREATE", role("a")).await;
    assert_eq!(cache.role(1.into(), 7.into()).unwrap().name, "a");
    deliver(K::GuildRoleUpdate, "GUILD_ROLE_UPDATE", role("b")).await;
    assert_eq!(cache.role(1.into(), 7.into()).unwrap().name, "b");
    deliver(K::GuildRoleDelete, "GUILD_ROLE_DELETE", json!({"guild_id": "1", "role_id": "7"})).await;
    assert!(cache.role(1.into(), 7.into()).is_none());

    for id in ["3", "4", "9"] {
        let body = json!({"id": id, "channel_id": "2", "author": user, "content": "x"});
        deliver(K::MessageCreate, "MESSAGE_CREATE", body).await;
    }
    let bulk = json!({"ids": ["3", "4", "1000"], "channel_id": "2", "guild_id": "1"});
    deliver(K::MessageDeleteBulk, "MESSAGE_DELETE_BULK", bulk).await;
    let left: Vec<_> = cache.channel_messages(2.into()).iter().map(|m| m.id.get()).collect();
    assert_eq!(left, [9], "only the named messages leave the cache");

    let reaction = json!({
        "user_id": "5", "channel_id": "2", "message_id": "9", "guild_id": "1",
        "emoji": {"id": null, "name": "👍"}, "burst": false, "type": 0, "burst_colors": [],
        "member": {"user": user, "roles": []}, "message_author_id": "5",
    });
    deliver(K::MessageReactionAdd, "MESSAGE_REACTION_ADD", reaction.clone()).await;
    deliver(K::MessageReactionRemove, "MESSAGE_REACTION_REMOVE", reaction).await;
    let ban = json!({"guild_id": "1", "user": user});
    deliver(K::GuildBanAdd, "GUILD_BAN_ADD", ban.clone()).await;
    deliver(K::GuildBanRemove, "GUILD_BAN_REMOVE", ban).await;

    assert_eq!(client.stats().decode_failures(), 0, "every body above is valid");
    wait_until("every event", || seen.lock().unwrap().len() == 8).await;
    let mut seen = seen.lock().unwrap().clone();
    seen.sort_unstable();
    assert_eq!(
        seen,
        [
            "GuildBanAdd",
            "GuildBanRemove",
            "GuildRoleCreate",
            "GuildRoleDelete",
            "GuildRoleUpdate",
            "MessageDeleteBulk",
            "MessageReactionAdd",
            "MessageReactionRemove",
        ]
    );
}

#[tokio::test]
async fn a_reaction_event_leads_to_its_message() {
    let (server, client) = fixture().await;
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "c");
    let message = client.channel(channel).send("hi").await.unwrap();
    let event: discord::model::MessageReactionAddEvent = serde_json::from_value(json!({
        "user_id": "5", "channel_id": channel.to_string(), "message_id": message.id.to_string(),
        "emoji": {"id": null, "name": "x"},
    }))
    .unwrap();
    let event = client.bind(event);
    assert_eq!(event.fetch_message().await.unwrap().content, "hi");
    assert!(event.guild().is_none(), "no guild_id in a DM");
    assert_eq!(event.user().id(), 5.into());
}

#[tokio::test]
async fn a_handler_with_no_matching_intent_is_a_build_error() {
    let result =
        Client::builder("t").intents(Intents::GUILDS).on(GuildBanAdd, |_ctx, _event| async move { Ok(()) }).build();
    assert!(result.is_err(), "bans need GUILD_MODERATION");
}

#[tokio::test]
async fn a_permission_check_fails_closed() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let client = Client::builder("t")
        .on_command("ban", move |_ctx, cmd| {
            let tx = tx.clone();
            async move {
                let held = cmd.has_permissions(Permissions::BAN_MEMBERS);
                let both = cmd.has_permissions(Permissions::BAN_MEMBERS | Permissions::KICK_MEMBERS);
                tx.send((held, both)).await?;
                Ok(())
            }
        })
        .build()
        .unwrap()
        .client();
    let user = json!({"id": "5", "username": "u"});
    let base = json!({
        "id": "4", "application_id": "9", "type": 2, "token": "tok", "version": 1, "channel_id": "2",
        "data": {"id": "6", "name": "ban", "type": 1},
    });
    let in_guild = |permissions: Option<&str>| {
        let mut body = base.clone();
        body["guild_id"] = json!("1");
        body["member"] = json!({"user": user, "roles": []});
        if let Some(permissions) = permissions {
            body["member"]["permissions"] = json!(permissions);
        }
        body
    };
    let mut in_dm = base.clone();
    in_dm["user"] = user.clone();

    for (body, expected, why) in [
        (in_guild(Some("4")), (true, false), "holds ban only"),
        (in_guild(Some("6")), (true, true), "holds both"),
        (in_guild(Some("2")), (false, false), "holds another permission"),
        (in_guild(None), (false, false), "Discord sent none"),
        (in_dm, (false, false), "in a DM"),
    ] {
        discord::events::deliver(&client, 0, K::InteractionCreate, "INTERACTION_CREATE", &body.to_string()).await;
        assert_eq!(rx.recv().await.unwrap(), expected, "{why}");
    }
}
