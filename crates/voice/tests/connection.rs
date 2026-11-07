//! A voice connection against the mock voice server: identify, IP discovery, the key, playback,
//! receiving, resuming and closing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_core::Secret;
use dr_mock::{MockServer, VOICE_TOKEN};
use dr_model::{ChannelId, GuildId, UserId};
use dr_voice::source::ogg_opus_bytes;
use dr_voice::{
    ConnectionInfo, OggOpus, OpusFrames, PlaybackState, VoiceConnection, VoiceError, VoiceOptions, VoiceStatus,
};
use std::time::{Duration, Instant};

fn info(server: &MockServer) -> ConnectionInfo {
    ConnectionInfo {
        endpoint: server.voice().url(),
        guild_id: GuildId::new(10),
        channel_id: ChannelId::new(20),
        user_id: UserId::new(30),
        session_id: Secret::new("sess".to_owned()),
        token: Secret::new(VOICE_TOKEN.to_owned()),
    }
}

async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Opus CELT 20 ms frames, told apart by their second byte.
fn frame(n: u8) -> Vec<u8> {
    vec![0xfc, n, 0xaa]
}

#[tokio::test]
async fn it_connects_and_plays_opus_frames() {
    let server = MockServer::start().await.unwrap();
    let connection = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap();
    assert_eq!(connection.status(), VoiceStatus::Ready);
    assert_eq!(connection.dave_version(), 0);

    let frames = server.voice().frames();
    let identify = frames.iter().find(|f| f.op == 0).unwrap();
    assert_eq!(identify.d["server_id"], "10");
    assert_eq!(identify.d["user_id"], "30");
    assert!(identify.d["max_dave_protocol_version"].as_u64().unwrap() >= 1, "DAVE is offered");
    let select = frames.iter().find(|f| f.op == 1).unwrap();
    assert_eq!(select.d["data"]["mode"], "aead_aes256_gcm_rtpsize", "the preferred mode");
    assert_eq!(select.d["data"]["address"], "127.0.0.1");

    let start = Instant::now();
    let playback = connection.play(OpusFrames::new((0..5).map(frame)));
    assert_eq!(playback.finished().await, PlaybackState::Ended);
    assert!(start.elapsed() >= Duration::from_millis(80), "paced at 20 ms: {:?}", start.elapsed());
    assert_eq!(playback.played(), Duration::from_millis(100));

    wait_until("5 frames and 5 of silence", || server.voice().audio().len() == 10).await;
    let audio = server.voice().audio();
    assert_eq!(audio[..5], (0..5).map(frame).collect::<Vec<_>>()[..]);
    assert!(audio[5..].iter().all(|f| f == &[0xf8, 0xff, 0xfe]), "silence ends the audio");
    let speaking = || {
        server.voice().frames().into_iter().filter(|f| f.op == 5).map(|f| f.d["speaking"].clone()).collect::<Vec<_>>()
    };
    wait_until("the bot stops speaking", || speaking().last() == Some(&serde_json::json!(0))).await;
    assert_eq!(speaking(), [1, 0]);

    connection.destroy();
    wait_until("destroyed", || connection.status() == VoiceStatus::Destroyed).await;
    assert!(matches!(connection.wait_ready().await, Err(VoiceError::Destroyed)));
}

#[tokio::test]
async fn the_other_mode_and_an_ogg_file() {
    let server = MockServer::start().await.unwrap();
    server.voice().offer_modes(&["aead_xchacha20_poly1305_rtpsize"]);
    let connection = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap();
    let ogg = ogg_opus_bytes(&[&frame(1), &frame(2)]);
    let playback = connection.play(OggOpus::from_bytes(ogg));
    assert_eq!(playback.finished().await, PlaybackState::Ended);
    wait_until("two frames", || server.voice().audio().len() >= 2).await;
    assert_eq!(server.voice().audio()[..2], [frame(1), frame(2)]);
    connection.destroy();
}

#[tokio::test]
async fn a_bad_token_or_no_common_mode_fails_to_connect() {
    let server = MockServer::start().await.unwrap();
    let mut bad = info(&server);
    bad.token = Secret::new("wrong".to_owned());
    let error = VoiceConnection::connect(bad, VoiceOptions::default()).await.unwrap_err();
    assert!(matches!(error, VoiceError::Closed { code: 4004, .. }), "{error:?}");

    server.voice().offer_modes(&["xsalsa20_poly1305"]);
    let error = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap_err();
    assert!(matches!(error, VoiceError::Connect(_)), "{error:?}");
}

#[tokio::test]
async fn audio_from_others_is_received() {
    let server = MockServer::start().await.unwrap();
    let connection = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap();
    let mut incoming = connection.receive();
    server.voice().speak(77, 5555, &[&frame(9), &frame(8)]).await;
    let first = tokio::time::timeout(Duration::from_secs(5), incoming.recv()).await.unwrap().unwrap();
    assert_eq!(first.user_id, UserId::new(77));
    assert_eq!(first.ssrc, 5555);
    assert_eq!(first.opus.as_ref(), frame(9));
    let second = incoming.recv().await.unwrap();
    assert_eq!(second.opus.as_ref(), frame(8));
    connection.destroy();
}

#[tokio::test]
async fn a_dropped_socket_resumes_and_a_kick_disconnects() {
    let server = MockServer::start().await.unwrap();
    let connection = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap();
    server.voice().close(4015);
    wait_until("resumed", || server.voice().frames().iter().any(|f| f.op == 7)).await;
    wait_until("ready again", || connection.status() == VoiceStatus::Ready).await;
    let resume = server.voice().frames().into_iter().find(|f| f.op == 7).unwrap();
    assert_eq!(resume.d["server_id"], "10");
    assert_eq!(server.voice().connections(), 2);

    let playback = connection.play(OpusFrames::new([frame(3)]));
    assert_eq!(playback.finished().await, PlaybackState::Ended, "audio flows after a resume");

    server.voice().close(4014);
    let mut changes = connection.status_changes();
    let seen =
        tokio::time::timeout(Duration::from_secs(5), changes.wait_for(|s| matches!(s, VoiceStatus::Disconnected(_))))
            .await
            .map(|r| r.map(|s| *s));
    assert!(seen.is_ok(), "status {:?}, connections {}", connection.status(), server.voice().connections());
    assert_eq!(connection.status(), VoiceStatus::Disconnected(4014));
    connection.update_server(info(&server));
    wait_until("reconnected", || connection.status() == VoiceStatus::Ready).await;
    assert_eq!(server.voice().frames().iter().filter(|f| f.op == 0).count(), 2, "a new identify");
    connection.destroy();
}

#[tokio::test]
async fn pause_resume_and_stop() {
    let server = MockServer::start().await.unwrap();
    let connection = VoiceConnection::connect(info(&server), VoiceOptions::default()).await.unwrap();
    let playback = connection.play(OpusFrames::new((0..200).map(|i| frame(i as u8))));
    wait_until("some audio", || server.voice().audio().len() >= 2).await;
    playback.pause();
    wait_until("paused", || playback.state() == PlaybackState::Paused).await;
    let at_pause = server.voice().audio().len();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.voice().audio().len(), at_pause, "nothing while paused");
    playback.resume();
    wait_until("more audio", || server.voice().audio().len() > at_pause + 1).await;

    let next = connection.play(OpusFrames::new([frame(1)]));
    assert_eq!(playback.finished().await, PlaybackState::Stopped, "a new play stops the old one");
    assert_eq!(next.finished().await, PlaybackState::Ended);
    connection.destroy();
}
