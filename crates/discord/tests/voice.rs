//! Voice through the client, on the mock: join with op 4, play, the cached voice states, a kick and
//! leave.
#![cfg(feature = "voice")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wait_until;
use discord::prelude::*;
use discord::voice::{OpusFrames, PlaybackState, VoiceStatus};
use dr_mock::MockServer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

struct Bot {
    server: MockServer,
    client: Client,
    guild: GuildId,
    channel: ChannelId,
    alice: UserId,
    run: tokio::task::JoinHandle<Result<(), discord::Error>>,
}

async fn start() -> Bot {
    let server = MockServer::start().await.unwrap();
    let world = server.world();
    let guild = world.add_guild("g");
    let channel = world.add_channel(guild, "Lounge");
    let alice = world.add_user("alice");
    world.add_member(guild, alice);
    world.set_voice_state(guild.get(), alice.get(), Some(channel.get()), false);
    let ready = Arc::new(AtomicU32::new(0));
    let runner = Client::builder("test-token")
        .state(ready.clone())
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_VOICE_STATES)
        .on(ClientReady, |ctx, _| async move {
            ctx.state.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    wait_until("ready", || ready.load(Ordering::SeqCst) == 1).await;
    Bot { server, client, guild, channel, alice, run }
}

#[tokio::test]
async fn join_the_users_channel_play_and_leave() {
    let bot = start().await;
    let state = bot.client.cache().voice_state(bot.guild, bot.alice).expect("from GUILD_CREATE");
    assert_eq!(state.channel_id, Some(bot.channel));

    let connection = bot.client.voice().join(bot.guild, state.channel_id.unwrap()).await.unwrap();
    assert_eq!(connection.status(), VoiceStatus::Ready);
    assert_eq!(connection.channel_id(), bot.channel);
    let op4 = bot.server.frames(0).into_iter().find(|f| f.op == 4).unwrap();
    assert_eq!(op4.d["channel_id"], bot.channel.to_string());
    assert_eq!(op4.d["guild_id"], bot.guild.to_string());
    let identify = bot.server.voice().frames().into_iter().find(|f| f.op == 0).unwrap();
    assert!(identify.d["session_id"].as_str().unwrap().starts_with("mock-voice-session-"));
    let me = bot.client.cache().me().unwrap().id;
    assert_eq!(bot.client.cache().voice_state(bot.guild, me).unwrap().channel_id, Some(bot.channel));

    let playback = connection.play(OpusFrames::new([vec![0xfc, 1], vec![0xfc, 2]]));
    assert_eq!(playback.finished().await, PlaybackState::Ended);
    wait_until("the audio", || bot.server.voice().audio().len() >= 2).await;
    assert_eq!(bot.server.voice().audio()[..2], [vec![0xfc, 1], vec![0xfc, 2]]);
    assert!(bot.client.voice().get(bot.guild).is_some());

    bot.client.voice().leave(bot.guild).await.unwrap();
    wait_until("op 4 to leave", || bot.server.frames(0).iter().any(|f| f.op == 4 && f.d["channel_id"].is_null())).await;
    wait_until("destroyed", || connection.status() == VoiceStatus::Destroyed).await;
    assert!(bot.client.voice().get(bot.guild).is_none());
    wait_until("out of the cache", || bot.client.cache().voice_state(bot.guild, me).is_none()).await;
    bot.client.shutdown(ShutdownMode::Final).await;
    bot.run.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_disconnect_from_discord_closes_the_connection() {
    let bot = start().await;
    let connection = bot.client.voice().join(bot.guild, bot.channel).await.unwrap();
    let me = bot.client.cache().me().unwrap().id;
    let left = bot.server.world().set_voice_state(bot.guild.get(), me.get(), None, false);
    bot.server.world().emit("VOICE_STATE_UPDATE", left);
    wait_until("destroyed", || connection.status() == VoiceStatus::Destroyed).await;
    assert!(bot.client.voice().get(bot.guild).is_none());
    bot.client.shutdown(ShutdownMode::Final).await;
    bot.run.await.unwrap().unwrap();
}

#[tokio::test]
async fn joining_before_the_client_runs_fails() {
    let client = Client::builder("t").build().unwrap().client();
    let error = client.voice().join(GuildId::new(1), ChannelId::new(2)).await.unwrap_err();
    assert!(matches!(error, discord::Error::NotReady), "{error:?}");
}
