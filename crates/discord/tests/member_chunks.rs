//! Members over the gateway: op 8 and its `GUILD_MEMBERS_CHUNK` answer, on the mock.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::prelude::*;
use dr_mock::MockServer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

struct Bot {
    server: MockServer,
    client: Client,
    guild: GuildId,
    users: Vec<UserId>,
    chunks: Arc<AtomicU32>,
    run: tokio::task::JoinHandle<Result<(), discord::Error>>,
}

async fn start(intents: Intents) -> Bot {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("big");
    let users: Vec<UserId> = ["alice", "albert", "bob", "carol", "dave"].iter().map(|n| world.add_user(n)).collect();
    for user in &users {
        world.add_member(guild, *user);
    }
    server.gateway().member_chunk_size(2);
    let chunks = Arc::new(AtomicU32::new(0));
    let ready = Arc::new(AtomicU32::new(0));
    let runner = Client::builder("test-token")
        .state((chunks.clone(), ready.clone()))
        .endpoints(server.endpoints())
        .intents(intents)
        .on(ClientReady, |ctx, _| async move {
            ctx.state.1.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .on(GuildMembersChunk, |ctx, _| async move {
            ctx.state.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || ready.load(Ordering::SeqCst) == 1).await;
    Bot { server, client, guild, users, chunks, run }
}

impl Bot {
    async fn stop(self) {
        self.client.shutdown(ShutdownMode::Final).await;
        self.run.await.unwrap().unwrap();
    }
}

#[tokio::test]
async fn every_member_comes_in_chunks() {
    let bot = start(Intents::GUILDS | Intents::GUILD_MEMBERS).await;
    let members = bot.client.guild(bot.guild).members().fetch_all().await.unwrap();
    let mut ids: Vec<UserId> = members.iter().map(|m| m.user.as_ref().unwrap().id).collect();
    ids.sort();
    let mut want = bot.users.clone();
    want.sort();
    assert_eq!(ids, want);
    assert!(members.iter().all(|m| m.guild_id == Some(bot.guild)));
    wait_until("the handler saw 3 chunks", || bot.chunks.load(Ordering::SeqCst) == 3).await;

    let request = bot.server.frames(0).into_iter().find(|f| f.op == 8).unwrap();
    assert_eq!(request.d["guild_id"], bot.guild.to_string());
    assert_eq!(request.d["query"], "");
    assert_eq!(request.d["limit"], 0);
    assert!(request.d["nonce"].is_string());
    bot.stop().await;
}

#[tokio::test]
async fn a_search_and_named_users() {
    let bot = start(Intents::GUILDS).await;
    let members = bot.client.guild(bot.guild).members();
    let found = members.search_gateway("AL", 10).await.unwrap();
    let mut names: Vec<String> = found.iter().map(|m| m.user.as_ref().unwrap().username.clone()).collect();
    names.sort();
    assert_eq!(names, ["albert", "alice"]);

    let stranger = UserId::new(999_999);
    let named = members.fetch_many([bot.users[2], stranger]).await.unwrap();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].user.as_ref().unwrap().username, "bob");
    bot.stop().await;
}

#[tokio::test]
async fn listing_everyone_needs_the_intent_and_a_running_client() {
    let bot = start(Intents::GUILDS).await;
    let error = bot.client.guild(bot.guild).members().fetch_all().await.unwrap_err();
    assert!(matches!(error, discord::Error::Config(_)), "{error:?}");
    assert!(bot.server.frames(0).iter().all(|f| f.op != 8), "nothing was sent");
    bot.stop().await;

    let idle = Client::builder("t").intents(Intents::GUILD_MEMBERS).build().unwrap().client();
    let error = idle.guild(GuildId::new(1)).members().fetch_all().await.unwrap_err();
    assert!(matches!(error, discord::Error::NotReady), "{error:?}");
}
