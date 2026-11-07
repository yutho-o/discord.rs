//! A mock voice server: the voice gateway (version 8) and its UDP socket, on loopback.
//!
//! It identifies with the token the main gateway handed out, answers IP discovery, hands out a key
//! in the session description (DAVE version 0), and decrypts the audio it receives so a test can
//! read it back. It can also speak to the client: [`VoiceScript::speak`].

use dr_voice::EncryptionMode;
use dr_voice::crypto::{RtpHeader, Transport};
use dr_voice::protocol::op;
use dr_voice::udp;
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// The voice token the mock's main gateway hands out.
pub const VOICE_TOKEN: &str = "mock-voice-token";

/// A JSON frame a voice client sent.
#[derive(Clone, Debug)]
pub struct VoiceFrame {
    /// The opcode.
    pub op: u8,
    /// The body.
    pub d: Value,
}

#[derive(Clone, Debug)]
enum Command {
    Close(u16),
    Text(String),
}

/// The voice half's state.
#[derive(Debug)]
pub struct VoiceState {
    pub(crate) url: String,
    udp: Arc<UdpSocket>,
    frames: Mutex<Vec<VoiceFrame>>,
    audio: Mutex<Vec<Vec<u8>>>,
    keepalives: AtomicU32,
    transport: Mutex<Option<Arc<Transport>>>,
    client: Mutex<Option<SocketAddr>>,
    open: Mutex<Vec<mpsc::Sender<Command>>>,
    connections: AtomicU32,
    modes: Mutex<Vec<String>>,
    next_seq: AtomicU32,
}

impl VoiceState {
    pub(crate) async fn bind() -> std::io::Result<(Arc<Self>, TcpListener)> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let udp = UdpSocket::bind(("127.0.0.1", 0)).await?;
        let modes =
            vec![EncryptionMode::Aes256Gcm.name().to_owned(), EncryptionMode::XChaCha20Poly1305.name().to_owned()];
        let state = Arc::new(Self {
            url: format!("ws://{}", listener.local_addr()?),
            udp: Arc::new(udp),
            frames: Mutex::new(Vec::new()),
            audio: Mutex::new(Vec::new()),
            keepalives: AtomicU32::new(0),
            transport: Mutex::new(None),
            client: Mutex::new(None),
            open: Mutex::new(Vec::new()),
            connections: AtomicU32::new(0),
            modes: Mutex::new(modes),
            next_seq: AtomicU32::new(0),
        });
        Ok((state, listener))
    }

    fn command(&self, command: Command) {
        let mut open = self.open.lock();
        open.retain(|c| !c.is_closed());
        for connection in open.iter() {
            let _ = connection.try_send(command.clone());
        }
    }
}

/// Serves the voice gateway and the UDP socket until aborted.
pub(crate) async fn serve(listener: TcpListener, state: Arc<VoiceState>) {
    let accept = async {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tokio::spawn(session(stream, state.clone()));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mock voice listener ended");
                    return;
                }
            }
        }
    };
    // In one task, so that aborting it stops both.
    tokio::select! {
        () = accept => {}
        () = serve_udp(state.clone()) => {}
    }
}

async fn serve_udp(state: Arc<VoiceState>) {
    let mut buf = vec![0u8; 2048];
    loop {
        let Ok((n, peer)) = state.udp.recv_from(&mut buf).await else { return };
        let packet = &buf[..n];
        if n == udp::DISCOVERY_LEN && packet[0..2] == [0, 1] {
            let ssrc = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
            let ip = match peer.ip() {
                std::net::IpAddr::V4(ip) => ip,
                std::net::IpAddr::V6(_) => std::net::Ipv4Addr::LOCALHOST,
            };
            *state.client.lock() = Some(peer);
            let _ = state.udp.send_to(&udp::discovery_response(ssrc, ip, peer.port()), peer).await;
        } else if n == 8 {
            state.keepalives.fetch_add(1, Ordering::SeqCst);
        } else if let Some(transport) = state.transport.lock().clone()
            && let Ok(rtp) = transport.open(packet)
        {
            state.audio.lock().push(rtp.opus);
        }
    }
}

/// Closes with `code` and waits for the client's answer, so that what the client sent meanwhile does not
/// turn the close into a reset.
async fn close(ws: &mut tokio_tungstenite::WebSocketStream<TcpStream>, code: u16) {
    let _ = ws.close(Some(CloseFrame { code: CloseCode::from(code), reason: "".into() })).await;
    let drain = async { while let Some(Ok(_)) = ws.next().await {} };
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), drain).await;
}

async fn session(stream: TcpStream, state: Arc<VoiceState>) {
    let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let number = state.connections.fetch_add(1, Ordering::SeqCst);
    let (tx, mut rx) = mpsc::channel(16);
    state.open.lock().push(tx);
    let send = |op: u8, d: Value| Message::text(json!({"op": op, "d": d}).to_string());
    if ws.send(send(op::HELLO, json!({"heartbeat_interval": 41_250.0}))).await.is_err() {
        return;
    }
    let ssrc = 1000 + number;
    loop {
        tokio::select! {
            message = ws.next() => {
                let Some(Ok(Message::Text(text))) = message else { return };
                let Ok(frame) = serde_json::from_str::<Value>(text.as_str()) else { return };
                let opcode = frame["op"].as_u64().and_then(|o| u8::try_from(o).ok()).unwrap_or(u8::MAX);
                let d = frame["d"].clone();
                state.frames.lock().push(VoiceFrame { op: opcode, d: d.clone() });
                let reply = match opcode {
                    op::IDENTIFY | op::RESUME => {
                        if d["token"] != VOICE_TOKEN || d["session_id"].as_str().is_none_or(str::is_empty) {
                            close(&mut ws, 4004).await;
                            return;
                        }
                        if opcode == op::RESUME {
                            Some(send(op::RESUMED, Value::Null))
                        } else {
                            let port = state.udp.local_addr().map_or(0, |a| a.port());
                            let modes = state.modes.lock().clone();
                            Some(send(op::READY, json!({"ssrc": ssrc, "ip": "127.0.0.1", "port": port, "modes": modes})))
                        }
                    }
                    op::SELECT_PROTOCOL => {
                        let mode = d.pointer("/data/mode").and_then(Value::as_str).and_then(EncryptionMode::from_name);
                        let Some(mode) = mode else {
                            close(&mut ws, 4016).await;
                            return;
                        };
                        let key: Vec<u8> = (0..32u8).map(|i| i.wrapping_mul(7).wrapping_add(u8::try_from(number % 256).unwrap_or(0))).collect();
                        let Ok(transport) = Transport::new(mode, &key) else { return };
                        *state.transport.lock() = Some(Arc::new(transport));
                        Some(send(op::SESSION_DESCRIPTION, json!({"mode": mode.name(), "secret_key": key, "dave_protocol_version": 0})))
                    }
                    op::HEARTBEAT => Some(send(op::HEARTBEAT_ACK, json!({"t": d["t"]}))),
                    _ => None,
                };
                if let Some(reply) = reply
                    && ws.send(reply).await.is_err()
                {
                    return;
                }
            }
            command = rx.recv() => match command {
                Some(Command::Close(code)) => {
                    close(&mut ws, code).await;
                    return;
                }
                Some(Command::Text(text)) => {
                    if ws.send(Message::text(text)).await.is_err() {
                        return;
                    }
                }
                None => return,
            },
        }
    }
}

/// Scripts the mock voice server: `server.voice().close(4014)`.
#[derive(Debug)]
pub struct VoiceScript<'a> {
    state: &'a VoiceState,
}

impl<'a> VoiceScript<'a> {
    pub(crate) fn new(state: &'a VoiceState) -> Self {
        Self { state }
    }

    /// The voice gateway's URL, as `VOICE_SERVER_UPDATE` names it.
    pub fn url(&self) -> String {
        self.state.url.clone()
    }

    /// Every JSON frame clients sent, in order.
    pub fn frames(&self) -> Vec<VoiceFrame> {
        self.state.frames.lock().clone()
    }

    /// The Opus frames received over UDP, decrypted, in order.
    pub fn audio(&self) -> Vec<Vec<u8>> {
        self.state.audio.lock().clone()
    }

    /// UDP keepalives received.
    pub fn keepalives(&self) -> u32 {
        self.state.keepalives.load(Ordering::SeqCst)
    }

    /// How many voice gateway connections were opened.
    pub fn connections(&self) -> u32 {
        self.state.connections.load(Ordering::SeqCst)
    }

    /// Offers only these encryption modes to later connections.
    pub fn offer_modes(&self, modes: &[&str]) {
        *self.state.modes.lock() = modes.iter().map(|m| (*m).to_owned()).collect();
    }

    /// Closes the open voice connections with `code`.
    pub fn close(&self, code: u16) {
        self.state.command(Command::Close(code));
    }

    /// Makes `user` speak: op 5 with their SSRC, then each Opus frame over UDP, encrypted with the
    /// session's key. Nothing happens before a client selected its protocol.
    pub async fn speak(&self, user: u64, ssrc: u32, frames: &[&[u8]]) {
        let body =
            json!({"op": op::SPEAKING, "d": {"speaking": 1, "ssrc": ssrc, "user_id": user.to_string(), "delay": 0}});
        self.state.command(Command::Text(body.to_string()));
        // The op 5 frame goes first, so the client knows the SSRC.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let (Some(transport), Some(client)) = (self.state.transport.lock().clone(), *self.state.client.lock()) else {
            return;
        };
        for (i, frame) in frames.iter().enumerate() {
            let seq = self.state.next_seq.fetch_add(1, Ordering::SeqCst);
            let header =
                RtpHeader::build(u16::try_from(seq % 65_536).unwrap_or(0), 960 * u32::try_from(i).unwrap_or(0), ssrc);
            if let Ok(packet) = transport.seal(&header, seq, frame) {
                let _ = self.state.udp.send_to(&packet, client).await;
            }
        }
    }
}
