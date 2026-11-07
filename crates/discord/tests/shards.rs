//! Several shards in one client, on the mock: each identifies with its own id, gets only its own
//! guilds, and `ClientReady` fires once, after all of them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::prelude::*;
use dr_mock::MockServer;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A guild id on `shard` of 2: `(id >> 22) % 2 == shard`.
fn on_shard(shard: u64, n: u64) -> u64 {
    ((2 * n + shard) << 22) | n
}

#[derive(Default)]
struct Seen {
    ready: AtomicUsize,
    guilds: Mutex<Vec<(u32, GuildId)>>,
    ready_guilds: AtomicUsize,
}

#[tokio::test]
async fn two_shards_split_the_guilds_and_fire_ready_once() {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let a = world.add_guild_with_id(on_shard(0, 1), "zero");
    let b = world.add_guild_with_id(on_shard(1, 2), "one");
    let c = world.add_guild_with_id(on_shard(1, 3), "one too");
    let seen = Arc::new(Seen::default());

    let runner = Client::builder("test-token")
        .state(seen.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS)
        .shards(2)
        .on(ClientReady, |ctx, ready| async move {
            ctx.state.ready_guilds.store(ready.guilds.len(), Ordering::SeqCst);
            ctx.state.ready.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .on(GuildCreate, |ctx, guild| async move {
            ctx.state.guilds.lock().unwrap().push((ctx.shard, guild.id));
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());

    wait_until("ClientReady", || seen.ready.load(Ordering::SeqCst) == 1).await;
    assert_eq!(seen.ready_guilds.load(Ordering::SeqCst), 3, "the guilds of both READYs");
    let mut guilds = seen.guilds.lock().unwrap().clone();
    guilds.sort();
    assert_eq!(guilds, [(0, a), (1, b), (1, c)]);

    assert_eq!(client.shard_count(), Some(2));
    assert_eq!(client.shard_ids(), [0, 1]);
    assert_eq!(client.shard_for(b), Some(1));
    for shard in [0, 1] {
        let identify = server.frames(shard).into_iter().find(|f| f.op == 2).unwrap();
        assert_eq!(identify.d["shard"], serde_json::json!([shard, 2]));
    }

    client.set_presence(UpdatePresence::new(PresenceStatus::Idle)).unwrap();
    wait_until("a presence on each shard", || {
        [0, 1].iter().all(|s| server.frames(*s).iter().any(|f| f.op == 3 && f.d["status"] == "idle"))
    })
    .await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(seen.ready.load(Ordering::SeqCst), 1, "once");
    client.shutdown(ShutdownMode::Final).await;
    run.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_shard_id_above_the_count_fails_the_run() {
    let server = MockServer::start().await.unwrap();
    let runner =
        Client::builder("test-token").endpoints(server.endpoints()).shards(2).shard_ids([0, 2]).build().unwrap();
    assert!(runner.run().await.is_err());
}
