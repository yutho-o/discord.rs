//! One voice connection: the voice gateway, the UDP socket, encryption, and the task that runs them.
//!
//! The task identifies, finds this client's public address with IP discovery, selects a protocol,
//! and on the session description holds the key. A close it can resume from (below 4000, or 4015)
//! resumes up to 5 times; any other leaves the connection `Disconnected`, waiting for a new voice
//! server from the main gateway or for `destroy`.

use crate::crypto::{EncryptionMode, RtpHeader, Transport};
use crate::dave::{Dave, DaveOut, max_protocol_version};
use crate::player::{Playback, run_player};
use crate::protocol::{
    self, BinaryFrame, ClientDisconnect, ClientsConnect, Heartbeat, Hello, Identify, Incoming, Ready, Resume,
    SelectProtocol, SelectProtocolData, SessionDescription, Speaking, VoiceFrame, op,
};
use crate::source::{AudioSource, opus_duration};
use crate::{VoiceError, udp};
use bytes::Bytes;
use dr_core::Secret;
use dr_model::{ChannelId, GuildId, UserId};
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, Interval, MissedTickBehavior, interval, sleep, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_tls_with_config};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const KEEPALIVE: Duration = Duration::from_secs(5);
const MAX_RESUMES: u32 = 5;
const RECEIVE_BACKLOG: usize = 256;
/// Commands waiting for the connection task: speaking changes, a new server, the end.
const COMMAND_BACKLOG: usize = 64;

/// Where and as whom to connect: the voice server from `VOICE_SERVER_UPDATE`, the session from the
/// bot's own `VOICE_STATE_UPDATE`.
#[derive(Clone, Debug)]
pub struct ConnectionInfo {
    /// The voice server's host and port, with no scheme. A `ws://` or `wss://` URL is used as is,
    /// which is how a test points at a local server.
    pub endpoint: String,
    /// The guild.
    pub guild_id: GuildId,
    /// The channel joined.
    pub channel_id: ChannelId,
    /// The bot's user id.
    pub user_id: UserId,
    /// The voice session. Redacted in `Debug`.
    pub session_id: Secret,
    /// The voice token. Redacted in `Debug`.
    pub token: Secret,
}

/// How a connection behaves.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VoiceOptions {
    /// Speak DAVE when the build has it. Discord requires it of every client, so turning it off
    /// only suits a test server.
    pub dave: bool,
    /// How long [`VoiceConnection::connect`] waits for the connection to be ready.
    pub ready_timeout: Duration,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self { dave: true, ready_timeout: Duration::from_secs(15) }
    }
}

/// Where a connection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceStatus {
    /// Connecting to a voice server.
    Connecting,
    /// Audio can flow.
    Ready,
    /// The socket dropped, and the session is resuming.
    Resuming,
    /// The voice gateway closed with this code, 0 when it could not be reached. It reconnects when
    /// given a new voice server.
    Disconnected(u16),
    /// Destroyed. It does nothing more.
    Destroyed,
}

/// An Opus packet someone sent, transport and DAVE encryption removed.
#[derive(Clone, Debug)]
pub struct VoicePacket {
    /// Who sent it.
    pub user_id: UserId,
    /// Their SSRC.
    pub ssrc: u32,
    /// The RTP sequence.
    pub sequence: u16,
    /// The RTP timestamp, in 48 kHz samples.
    pub timestamp: u32,
    /// The Opus packet.
    pub opus: Bytes,
}

enum Command {
    Text(String),
    Reconnect(Box<ConnectionInfo>),
    Disconnect,
}

struct Media {
    udp: Arc<UdpSocket>,
    transport: Arc<Transport>,
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    counter: u32,
    speaking: bool,
}

pub(crate) struct Inner {
    info: Mutex<ConnectionInfo>,
    options: VoiceOptions,
    status: watch::Sender<VoiceStatus>,
    commands: mpsc::Sender<Command>,
    media: Mutex<Option<Media>>,
    dave: Mutex<Option<Dave>>,
    ping: Mutex<Option<Duration>>,
    last_error: Mutex<Option<String>>,
    listeners: Mutex<Vec<mpsc::Sender<VoicePacket>>>,
    users: Mutex<HashMap<u32, u64>>,
    members: Mutex<BTreeSet<u64>>,
    pub(crate) playback: Mutex<Option<Playback>>,
}

/// A voice connection. Clone it freely: every clone is the same connection.
#[derive(Clone)]
pub struct VoiceConnection {
    pub(crate) inner: Arc<Inner>,
}

impl std::fmt::Debug for VoiceConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let info = self.inner.info.lock();
        f.debug_struct("VoiceConnection")
            .field("guild_id", &info.guild_id)
            .field("channel_id", &info.channel_id)
            .field("status", &*self.inner.status.borrow())
            .finish_non_exhaustive()
    }
}

impl VoiceConnection {
    /// Connects and waits until audio can flow. Fails if the server cannot be reached, refuses the
    /// session, or the connection is not ready within `options.ready_timeout`.
    pub async fn connect(info: ConnectionInfo, options: VoiceOptions) -> Result<Self, VoiceError> {
        let limit = options.ready_timeout;
        let connection = Self::start(info, options);
        match timeout(limit, connection.wait_ready()).await {
            Ok(Ok(())) => Ok(connection),
            Ok(Err(e)) => {
                connection.destroy();
                Err(e)
            }
            Err(_) => {
                connection.destroy();
                Err(VoiceError::Timeout("the voice connection to be ready"))
            }
        }
    }

    /// Starts connecting in the background and returns at once. Watch [`Self::status`].
    pub fn start(info: ConnectionInfo, options: VoiceOptions) -> Self {
        let (commands, rx) = mpsc::channel(COMMAND_BACKLOG);
        let inner = Arc::new(Inner {
            info: Mutex::new(info),
            options,
            status: watch::channel(VoiceStatus::Connecting).0,
            commands,
            media: Mutex::new(None),
            dave: Mutex::new(None),
            ping: Mutex::new(None),
            last_error: Mutex::new(None),
            listeners: Mutex::new(Vec::new()),
            users: Mutex::new(HashMap::new()),
            members: Mutex::new(BTreeSet::new()),
            playback: Mutex::new(None),
        });
        tokio::spawn(run(inner.clone(), rx));
        Self { inner }
    }

    /// Waits until the connection is ready. Fails once it is disconnected or destroyed.
    pub async fn wait_ready(&self) -> Result<(), VoiceError> {
        let mut status = self.inner.status.subscribe();
        loop {
            let now = *status.borrow_and_update();
            match now {
                VoiceStatus::Ready => return Ok(()),
                VoiceStatus::Destroyed => return Err(VoiceError::Destroyed),
                VoiceStatus::Disconnected(code) => return Err(self.error_for(code)),
                VoiceStatus::Connecting | VoiceStatus::Resuming => {}
            }
            if status.changed().await.is_err() {
                return Err(VoiceError::Destroyed);
            }
        }
    }

    fn error_for(&self, code: u16) -> VoiceError {
        match (code, self.inner.last_error.lock().clone()) {
            (0, Some(e)) => VoiceError::Connect(e),
            (0, None) => VoiceError::Connect("unreachable".into()),
            (code, _) => VoiceError::Closed { code, reason: protocol::close_reason(code) },
        }
    }

    /// Where the connection is now.
    pub fn status(&self) -> VoiceStatus {
        *self.inner.status.borrow()
    }

    /// A receiver of status changes.
    pub fn status_changes(&self) -> watch::Receiver<VoiceStatus> {
        self.inner.status.subscribe()
    }

    /// The guild.
    pub fn guild_id(&self) -> GuildId {
        self.inner.info.lock().guild_id
    }

    /// The channel joined.
    pub fn channel_id(&self) -> ChannelId {
        self.inner.info.lock().channel_id
    }

    /// The last voice gateway heartbeat round trip.
    pub fn ping(&self) -> Option<Duration> {
        *self.inner.ping.lock()
    }

    /// The DAVE version in use, 0 when the call is not end-to-end encrypted.
    pub fn dave_version(&self) -> u16 {
        self.inner.dave.lock().as_ref().map_or(0, Dave::version)
    }

    /// The code users can compare to check the call is end-to-end encrypted, once it is.
    pub fn privacy_code(&self) -> Option<String> {
        self.inner.dave.lock().as_ref().and_then(Dave::privacy_code)
    }

    /// Points the connection at a new voice server or channel, as a later `VOICE_SERVER_UPDATE` or a
    /// move asks. It reconnects with a new session.
    pub fn update_server(&self, info: ConnectionInfo) {
        *self.inner.info.lock() = info.clone();
        let _ = self.inner.commands.try_send(Command::Reconnect(Box::new(info)));
    }

    /// Closes the connection for good, and stops what plays. It does not leave the channel on the
    /// main gateway: the facade's `leave` does both.
    pub fn destroy(&self) {
        if let Some(playback) = self.inner.playback.lock().take() {
            playback.stop();
        }
        let _ = self.inner.commands.try_send(Command::Disconnect);
    }

    /// Plays `source`, stopping whatever played before. Audio waits while the connection is not
    /// ready.
    pub fn play(&self, source: impl AudioSource) -> Playback {
        let playback = Playback::new();
        if let Some(old) = self.inner.playback.lock().replace(playback.clone()) {
            old.stop();
        }
        tokio::spawn(run_player(self.clone(), Box::new(source), playback.clone()));
        playback
    }

    /// The audio others send, as it arrives. Up to 256 packets wait; later ones are dropped while
    /// the receiver is not read. Dropping it unsubscribes.
    pub fn receive(&self) -> mpsc::Receiver<VoicePacket> {
        let (tx, rx) = mpsc::channel(RECEIVE_BACKLOG);
        self.inner.listeners.lock().push(tx);
        rx
    }

    /// Sends one Opus frame now: DAVE, then transport encryption, then UDP. Sets the bot as
    /// speaking first. A frame sent before DAVE has a key is dropped, as Discord would not play it.
    pub async fn send_opus(&self, frame: &[u8]) -> Result<(), VoiceError> {
        let sealed = self.inner.dave.lock().as_mut().map_or_else(|| Some(frame.to_vec()), |d| d.encrypt(frame));
        let Some(sealed) = sealed else { return Ok(()) };
        let samples = opus_duration(frame).map_or(960, |d| u32::try_from(d.as_micros() * 48 / 1000).unwrap_or(960));
        let (udp, packet, speak) = {
            let mut media = self.inner.media.lock();
            let m = media.as_mut().ok_or(VoiceError::Destroyed)?;
            m.counter = m.counter.wrapping_add(1);
            let header = RtpHeader::build(m.sequence, m.timestamp, m.ssrc);
            let packet = m.transport.seal(&header, m.counter, &sealed)?;
            m.sequence = m.sequence.wrapping_add(1);
            m.timestamp = m.timestamp.wrapping_add(samples);
            let speak = (!m.speaking).then_some(m.ssrc);
            m.speaking = true;
            (m.udp.clone(), packet, speak)
        };
        if let Some(ssrc) = speak {
            self.send_speaking(ssrc, 1);
        }
        udp.send(&packet).await.map_err(VoiceError::Io)?;
        Ok(())
    }

    /// Tells Discord the bot stopped or started speaking. Playback does this by itself.
    pub fn set_speaking(&self, speaking: bool) {
        let ssrc = {
            let mut media = self.inner.media.lock();
            let Some(m) = media.as_mut() else { return };
            if m.speaking == speaking {
                return;
            }
            m.speaking = speaking;
            m.ssrc
        };
        self.send_speaking(ssrc, u8::from(speaking));
    }

    fn send_speaking(&self, ssrc: u32, flags: u8) {
        let body = Speaking { speaking: flags, delay: 0, ssrc, user_id: None };
        if let Ok(text) = VoiceFrame::new(op::SPEAKING, body).to_text() {
            let _ = self.inner.commands.try_send(Command::Text(text));
        }
    }
}

/// The URL of a voice server.
pub fn voice_url(endpoint: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.starts_with("ws://") || base.starts_with("wss://") {
        format!("{base}/?v=8")
    } else {
        format!("wss://{base}/?v=8")
    }
}

enum Outcome {
    Stop,
    NewServer(Box<ConnectionInfo>),
    Closed { code: u16, seq: i64, reached_ready: bool },
}

async fn run(inner: Arc<Inner>, mut commands: mpsc::Receiver<Command>) {
    let mut info = inner.info.lock().clone();
    let mut resume: Option<i64> = None;
    let mut resumes = 0;
    loop {
        match session(&inner, &info, resume, &mut commands).await {
            Outcome::Stop => break,
            Outcome::NewServer(new) => {
                info = *new;
                resume = None;
                resumes = 0;
                reset_media(&inner);
                inner.status.send_replace(VoiceStatus::Connecting);
            }
            Outcome::Closed { code, seq, reached_ready } => {
                if reached_ready {
                    resumes = 0;
                }
                let was_ready = reached_ready || resume.is_some();
                if was_ready && protocol::can_resume(code) && resumes < MAX_RESUMES {
                    resumes += 1;
                    resume = Some(seq);
                    inner.status.send_replace(VoiceStatus::Resuming);
                    sleep(Duration::from_millis(250) * resumes).await;
                    continue;
                }
                tracing::debug!(code, reason = protocol::close_reason(code), "voice connection disconnected");
                reset_media(&inner);
                inner.status.send_replace(VoiceStatus::Disconnected(code));
                // Waits for a new server, or the end.
                let next = loop {
                    match commands.recv().await {
                        Some(Command::Reconnect(new)) => break Some(new),
                        Some(Command::Disconnect) | None => break None,
                        Some(_) => {}
                    }
                };
                let Some(new) = next else { break };
                info = *new;
                resume = None;
                resumes = 0;
                inner.status.send_replace(VoiceStatus::Connecting);
            }
        }
    }
    reset_media(&inner);
    if let Some(playback) = inner.playback.lock().take() {
        playback.stop();
    }
    inner.listeners.lock().clear();
    inner.status.send_replace(VoiceStatus::Destroyed);
}

fn reset_media(inner: &Inner) {
    *inner.media.lock() = None;
    *inner.dave.lock() = None;
    inner.users.lock().clear();
    inner.members.lock().clear();
}

async fn send_text(ws: &mut Ws, text: String) -> bool {
    ws.send(Message::text(text)).await.is_ok()
}

async fn send_frame<T: serde::Serialize>(ws: &mut Ws, opcode: u8, d: T) -> bool {
    match VoiceFrame::new(opcode, d).to_text() {
        Ok(text) => send_text(ws, text).await,
        Err(_) => false,
    }
}

async fn send_dave(ws: &mut Ws, out: Vec<DaveOut>) -> bool {
    for item in out {
        let sent = match item {
            DaveOut::Json(text) => ws.send(Message::text(text)).await,
            DaveOut::Binary(bytes) => ws.send(Message::binary(bytes)).await,
        };
        if sent.is_err() {
            return false;
        }
    }
    true
}

async fn tick(beat: &mut Option<Interval>) {
    match beat {
        Some(i) => {
            i.tick().await;
        }
        None => std::future::pending().await,
    }
}

async fn recv(udp: &Option<Arc<UdpSocket>>, buf: &mut [u8]) -> std::io::Result<usize> {
    match udp {
        Some(u) => u.recv(buf).await,
        None => std::future::pending().await,
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0))
}

/// One WebSocket session, until it closes or is told to stop.
async fn session(
    inner: &Arc<Inner>,
    info: &ConnectionInfo,
    resume: Option<i64>,
    commands: &mut mpsc::Receiver<Command>,
) -> Outcome {
    let mut seq = resume.unwrap_or(-1);
    let closed = |code: u16, seq: i64| Outcome::Closed { code, seq, reached_ready: false };
    let connector = match dr_gateway::connector::ws_connector() {
        Ok(c) => c,
        Err(e) => {
            *inner.last_error.lock() = Some(e.to_string());
            return closed(0, seq);
        }
    };
    let url = voice_url(&info.endpoint);
    let mut ws = match timeout(CONNECT_TIMEOUT, connect_async_tls_with_config(url, None, true, Some(connector))).await {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(e)) => {
            *inner.last_error.lock() = Some(e.to_string());
            return closed(if resume.is_some() { 1006 } else { 0 }, seq);
        }
        Err(_) => {
            *inner.last_error.lock() = Some("no answer from the voice server".into());
            return closed(if resume.is_some() { 1006 } else { 0 }, seq);
        }
    };
    let server_id = info.guild_id.to_string();
    let first = match resume {
        Some(ack) => {
            let body =
                Resume { server_id, session_id: info.session_id.clone(), token: info.token.clone(), seq_ack: ack };
            send_frame(&mut ws, op::RESUME, body).await
        }
        None => {
            let version = if inner.options.dave { max_protocol_version() } else { 0 };
            let body = Identify {
                server_id,
                user_id: info.user_id.to_string(),
                session_id: info.session_id.clone(),
                token: info.token.clone(),
                max_dave_protocol_version: version,
            };
            send_frame(&mut ws, op::IDENTIFY, body).await
        }
    };
    if !first {
        return closed(1006, seq);
    }

    let user = info.user_id.get();
    let mut beat: Option<Interval> = None;
    let mut missed = 0u32;
    let mut sent_at = Instant::now();
    let mut reached_ready = false;
    let mut udp: Option<Arc<UdpSocket>> = inner.media.lock().as_ref().map(|m| m.udp.clone());
    let mut ssrc = inner.media.lock().as_ref().map_or(0, |m| m.ssrc);
    let mut keepalive = interval(KEEPALIVE);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut keepalive_counter = 0u32;
    let mut buf = vec![0u8; 4096];
    let end = |code: u16, seq: i64, reached_ready: bool| Outcome::Closed { code, seq, reached_ready };

    loop {
        tokio::select! {
            message = ws.next() => {
                let message = match message {
                    Some(Ok(m)) => m,
                    Some(Err(_)) | None => return end(1006, seq, reached_ready),
                };
                match message {
                    Message::Text(text) => {
                        let Ok(frame) = serde_json::from_str::<Incoming>(text.as_str()) else { continue };
                        if let Some(s) = frame.seq {
                            seq = s;
                        }
                        match frame.op {
                            op::HELLO => {
                                let Ok(hello) = serde_json::from_value::<Hello>(frame.d) else { continue };
                                let period = Duration::from_secs_f64((hello.heartbeat_interval / 1000.0).max(0.05));
                                let mut i = tokio::time::interval_at(Instant::now() + period, period);
                                i.set_missed_tick_behavior(MissedTickBehavior::Delay);
                                beat = Some(i);
                            }
                            op::READY => {
                                let Ok(ready) = serde_json::from_value::<Ready>(frame.d) else { continue };
                                ssrc = ready.ssrc;
                                let (socket, select) = match discover(&ready).await {
                                    Ok(found) => found,
                                    Err(e) => {
                                        *inner.last_error.lock() = Some(e.to_string());
                                        let _ = ws.close(None).await;
                                        return end(0, seq, false);
                                    }
                                };
                                udp = Some(socket);
                                *inner.users.lock() = HashMap::new();
                                if !send_frame(&mut ws, op::SELECT_PROTOCOL, select).await {
                                    return end(1006, seq, false);
                                }
                            }
                            op::SESSION_DESCRIPTION => {
                                let Ok(description) = serde_json::from_value::<SessionDescription>(frame.d) else { continue };
                                let Some(socket) = udp.clone() else { continue };
                                let transport = match EncryptionMode::from_name(&description.mode)
                                    .ok_or_else(|| VoiceError::NoMode(vec![description.mode.clone()]))
                                    .and_then(|mode| Ok(Transport::new(mode, &description.secret_key)?))
                                {
                                    Ok(t) => Arc::new(t),
                                    Err(e) => {
                                        *inner.last_error.lock() = Some(e.to_string());
                                        let _ = ws.close(None).await;
                                        return end(0, seq, false);
                                    }
                                };
                                *inner.media.lock() = Some(Media {
                                    udp: socket,
                                    transport,
                                    ssrc,
                                    sequence: rand::random(),
                                    timestamp: rand::random(),
                                    counter: 0,
                                    speaking: false,
                                });
                                let version = if inner.options.dave { description.dave_protocol_version } else { 0 };
                                let (dave, out) = Dave::new(version, user, info.channel_id.get());
                                *inner.dave.lock() = Some(dave);
                                if !send_dave(&mut ws, out).await {
                                    return end(1006, seq, false);
                                }
                                reached_ready = true;
                                inner.status.send_replace(VoiceStatus::Ready);
                            }
                            op::RESUMED => {
                                reached_ready = true;
                                if let Some(m) = inner.media.lock().as_mut() {
                                    m.speaking = false;
                                }
                                inner.status.send_replace(VoiceStatus::Ready);
                            }
                            op::HEARTBEAT_ACK => {
                                missed = 0;
                                *inner.ping.lock() = Some(sent_at.elapsed());
                            }
                            op::SPEAKING => {
                                if let Ok(speaking) = serde_json::from_value::<Speaking>(frame.d)
                                    && let Some(id) = speaking.user_id.and_then(|u| u.parse::<u64>().ok())
                                {
                                    inner.users.lock().insert(speaking.ssrc, id);
                                }
                            }
                            op::CLIENTS_CONNECT => {
                                if let Ok(c) = serde_json::from_value::<ClientsConnect>(frame.d) {
                                    inner.members.lock().extend(c.user_ids.iter().filter_map(|u| u.parse::<u64>().ok()));
                                }
                            }
                            op::CLIENT_DISCONNECT => {
                                if let Ok(c) = serde_json::from_value::<ClientDisconnect>(frame.d)
                                    && let Ok(id) = c.user_id.parse::<u64>()
                                {
                                    inner.members.lock().remove(&id);
                                    inner.users.lock().retain(|_, u| *u != id);
                                }
                            }
                            opcode @ (op::DAVE_PREPARE_TRANSITION | op::DAVE_EXECUTE_TRANSITION | op::DAVE_PREPARE_EPOCH) => {
                                let out = inner.dave.lock().as_mut().map(|d| d.on_json(opcode, &frame.d)).unwrap_or_default();
                                if !send_dave(&mut ws, out).await {
                                    return end(1006, seq, reached_ready);
                                }
                            }
                            _ => {}
                        }
                    }
                    Message::Binary(bytes) => {
                        let Some(frame) = BinaryFrame::parse(&bytes) else { continue };
                        seq = i64::from(frame.seq);
                        let mut members: Vec<u64> = inner.members.lock().iter().copied().collect();
                        members.push(user);
                        let out = inner
                            .dave
                            .lock()
                            .as_mut()
                            .map(|d| d.on_binary(frame.op, frame.payload, &members))
                            .unwrap_or_default();
                        if !send_dave(&mut ws, out).await {
                            return end(1006, seq, reached_ready);
                        }
                    }
                    Message::Close(frame) => {
                        let code = frame.map_or(1005, |f| u16::from(f.code));
                        return end(code, seq, reached_ready);
                    }
                    _ => {}
                }
            }
            () = tick(&mut beat) => {
                if missed >= 3 {
                    tracing::debug!("voice heartbeats unanswered, reconnecting");
                    let _ = ws.close(None).await;
                    return end(1006, seq, reached_ready);
                }
                missed += 1;
                sent_at = Instant::now();
                if !send_frame(&mut ws, op::HEARTBEAT, Heartbeat { t: now_ms(), seq_ack: seq }).await {
                    return end(1006, seq, reached_ready);
                }
            }
            _ = keepalive.tick(), if udp.is_some() => {
                if let Some(socket) = &udp {
                    let _ = socket.send(&udp::keepalive(keepalive_counter)).await;
                    keepalive_counter = keepalive_counter.wrapping_add(1);
                }
            }
            received = recv(&udp, &mut buf) => {
                if let Ok(n) = received {
                    let out = receive(inner, &buf[..n]);
                    if !out.is_empty() && !send_dave(&mut ws, out).await {
                        return end(1006, seq, reached_ready);
                    }
                }
            }
            command = commands.recv() => match command {
                Some(Command::Text(text)) => {
                    if !send_text(&mut ws, text).await {
                        return end(1006, seq, reached_ready);
                    }
                }
                Some(Command::Reconnect(new)) => {
                    let _ = ws.close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() })).await;
                    return Outcome::NewServer(new);
                }
                Some(Command::Disconnect) | None => {
                    let _ = ws.close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() })).await;
                    return Outcome::Stop;
                }
            },
        }
    }
}

/// Binds a socket, finds this client's public address, and builds the protocol selection.
async fn discover(ready: &Ready) -> Result<(Arc<UdpSocket>, SelectProtocol), VoiceError> {
    let mode = EncryptionMode::choose(&ready.modes).ok_or_else(|| VoiceError::NoMode(ready.modes.clone()))?;
    let server: SocketAddr = format!("{}:{}", ready.ip, ready.port)
        .parse()
        .map_err(|_| VoiceError::Protocol(format!("bad voice server address {}:{}", ready.ip, ready.port)))?;
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(VoiceError::Io)?;
    socket.connect(server).await.map_err(VoiceError::Io)?;
    let mut buf = [0u8; 128];
    for _ in 0..3 {
        socket.send(&udp::discovery_request(ready.ssrc)).await.map_err(VoiceError::Io)?;
        let answer = timeout(DISCOVERY_TIMEOUT, async {
            loop {
                let n = socket.recv(&mut buf).await?;
                if let Some(found) = udp::parse_discovery(&buf[..n]) {
                    return Ok::<_, std::io::Error>(found);
                }
            }
        })
        .await;
        if let Ok(Ok((ip, port))) = answer {
            let data = SelectProtocolData { address: ip.to_string(), port, mode: mode.name().to_owned() };
            return Ok((Arc::new(socket), SelectProtocol { protocol: "udp".into(), data }));
        }
    }
    Err(VoiceError::Connect("IP discovery got no answer".into()))
}

/// A UDP packet from the server: audio from someone, to hand to the receivers.
fn receive(inner: &Inner, packet: &[u8]) -> Vec<DaveOut> {
    if inner.listeners.lock().is_empty() {
        return Vec::new();
    }
    let Some(header) = RtpHeader::parse(packet) else { return Vec::new() };
    let Some(user) = inner.users.lock().get(&header.ssrc).copied() else { return Vec::new() };
    let Some(transport) = inner.media.lock().as_ref().map(|m| m.transport.clone()) else { return Vec::new() };
    let Ok(rtp) = transport.open(packet) else { return Vec::new() };
    let (opus, out) = match inner.dave.lock().as_mut() {
        Some(dave) => dave.decrypt(user, &rtp.opus),
        None => (Some(rtp.opus), Vec::new()),
    };
    if let Some(opus) = opus {
        let packet = VoicePacket {
            user_id: UserId::new(user),
            ssrc: rtp.ssrc,
            sequence: rtp.sequence,
            timestamp: rtp.timestamp,
            opus: Bytes::from(opus),
        };
        inner.listeners.lock().retain(|tx| match tx.try_send(packet.clone()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_urls() {
        assert_eq!(voice_url("abc.discord.media:443"), "wss://abc.discord.media:443/?v=8");
        assert_eq!(voice_url("ws://127.0.0.1:5000"), "ws://127.0.0.1:5000/?v=8");
    }
}
