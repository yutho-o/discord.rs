//! Playback: frames from a source, one every frame length, as upstream's `AudioPlayer` does. Five
//! frames of silence end each stretch of audio, so that the listeners' decoders do not glitch.

use crate::VoiceError;
use crate::connection::{VoiceConnection, VoiceStatus};
use crate::dave::SILENCE_FRAME;
use crate::source::{AudioSource, opus_duration};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

const FRAME: Duration = Duration::from_millis(20);
const SILENCE_FRAMES: usize = 5;
/// Behind by more than this, playback starts its clock over instead of rushing.
const MAX_LAG: Duration = Duration::from_millis(100);

/// Where a playback is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    /// Sending audio, or waiting for the connection to be ready.
    Playing,
    /// Paused: silence was sent, and nothing more until it resumes.
    Paused,
    /// The source ran out.
    Ended,
    /// Stopped, by `stop`, a newer `play`, or the connection's end.
    Stopped,
    /// The source failed with this.
    Failed(String),
}

impl PlaybackState {
    /// Ended, stopped or failed.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Ended | Self::Stopped | Self::Failed(_))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Control {
    paused: bool,
    stopped: bool,
}

struct Shared {
    state: watch::Sender<PlaybackState>,
    control: watch::Sender<Control>,
    played_micros: AtomicU64,
}

/// A handle to what a connection plays. Clone it freely.
#[derive(Clone)]
pub struct Playback(Arc<Shared>);

impl std::fmt::Debug for Playback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playback").field("state", &self.state()).finish_non_exhaustive()
    }
}

impl Playback {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Shared {
            state: watch::channel(PlaybackState::Playing).0,
            control: watch::channel(Control::default()).0,
            played_micros: AtomicU64::new(0),
        }))
    }

    /// Where it is now.
    pub fn state(&self) -> PlaybackState {
        self.0.state.borrow().clone()
    }

    /// How much audio was sent.
    pub fn played(&self) -> Duration {
        Duration::from_micros(self.0.played_micros.load(Ordering::Relaxed))
    }

    /// Pauses. Nothing if it already finished.
    pub fn pause(&self) {
        self.0.control.send_modify(|c| c.paused = true);
    }

    /// Resumes after a pause.
    pub fn resume(&self) {
        self.0.control.send_modify(|c| c.paused = false);
    }

    /// Stops for good.
    pub fn stop(&self) {
        self.0.control.send_modify(|c| c.stopped = true);
    }

    /// Waits until it ended, stopped or failed, and returns which.
    pub async fn finished(&self) -> PlaybackState {
        let mut state = self.0.state.subscribe();
        loop {
            let now = state.borrow_and_update().clone();
            if now.is_finished() {
                return now;
            }
            if state.changed().await.is_err() {
                return PlaybackState::Stopped;
            }
        }
    }

    fn set(&self, state: PlaybackState) {
        self.0.state.send_replace(state);
    }
}

async fn silence(connection: &VoiceConnection) {
    let mut next = Instant::now();
    for _ in 0..SILENCE_FRAMES {
        let _ = connection.send_opus(&SILENCE_FRAME).await;
        next += FRAME;
        sleep_until(next).await;
    }
    connection.set_speaking(false);
}

/// Waits for the connection to be ready. False when the playback stopped or the connection ended.
async fn ready(connection: &VoiceConnection, control: &mut watch::Receiver<Control>) -> bool {
    let mut status = connection.status_changes();
    loop {
        if control.borrow().stopped {
            return false;
        }
        match *status.borrow_and_update() {
            VoiceStatus::Ready => return true,
            VoiceStatus::Destroyed => return false,
            _ => {}
        }
        tokio::select! {
            changed = status.changed() => if changed.is_err() { return false },
            _ = control.changed() => {}
        }
    }
}

pub(crate) async fn run_player(connection: VoiceConnection, mut source: Box<dyn AudioSource>, playback: Playback) {
    let mut control = playback.0.control.subscribe();
    let mut next = Instant::now();
    let mut sent_audio = false;
    let end = loop {
        let now = *control.borrow_and_update();
        if now.stopped {
            break PlaybackState::Stopped;
        }
        if now.paused {
            if sent_audio {
                silence(&connection).await;
                sent_audio = false;
            }
            playback.set(PlaybackState::Paused);
            if control.wait_for(|c| !c.paused || c.stopped).await.is_err() {
                break PlaybackState::Stopped;
            }
            playback.set(PlaybackState::Playing);
            next = Instant::now();
            continue;
        }
        if connection.status() != VoiceStatus::Ready {
            if !ready(&connection, &mut control).await {
                break PlaybackState::Stopped;
            }
            next = Instant::now();
        }
        let frame = match source.next_frame().await {
            Ok(Some(frame)) => frame,
            Ok(None) => break PlaybackState::Ended,
            Err(e) => break PlaybackState::Failed(e.to_string()),
        };
        let length = opus_duration(&frame).unwrap_or(FRAME);
        match connection.send_opus(&frame).await {
            Ok(()) => sent_audio = true,
            Err(VoiceError::Destroyed) => {}
            Err(e) => tracing::debug!(error = %e, "a voice frame was not sent"),
        }
        playback.0.played_micros.fetch_add(u64::try_from(length.as_micros()).unwrap_or(0), Ordering::Relaxed);
        next += length;
        let now = Instant::now();
        if now > next + MAX_LAG {
            next = now;
        }
        sleep_until(next).await;
    };
    if sent_audio && connection.status() == VoiceStatus::Ready {
        silence(&connection).await;
    }
    playback.set(end);
}
