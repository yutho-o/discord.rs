//! One gateway connection.

use super::compress::Deflater;
use super::{Command, GatewayState, RecordedFrame};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

/// The heartbeat interval sent in Hello, unless a script changes it.
pub const HEARTBEAT_INTERVAL_MS: u64 = 41_250;

/// What the mock treats as a bad token: it closes with 4004.
pub const BAD_TOKEN: &str = "bad-token";

struct Connection {
    socket: WebSocketStream<TcpStream>,
    state: Arc<GatewayState>,
    deflater: Option<Deflater>,
    session: Option<String>,
    seq: u64,
    shard: u32,
    /// The shard count of its identify. 1 until then.
    shard_count: u32,
    stalled: bool,
    /// Dispatches scripted before this connection identified.
    held: Vec<(String, Value)>,
}

/// Runs one connection to its end.
pub(crate) async fn run(stream: TcpStream, state: Arc<GatewayState>) {
    let (commands, mut inbox) = mpsc::channel(256);
    serve(stream, state.clone(), commands, &mut inbox).await;
    // What was still queued for this connection goes to the next one.
    inbox.close();
    while let Ok(command) = inbox.try_recv() {
        state.pending.lock().push(command);
    }
}

async fn serve(
    stream: TcpStream,
    state: Arc<GatewayState>,
    commands: mpsc::Sender<Command>,
    inbox: &mut mpsc::Receiver<Command>,
) {
    let mut compressed = false;
    #[allow(clippy::result_large_err)] // The signature is tungstenite's.
    let callback = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        compressed = request.uri().query().is_some_and(|q| q.contains("compress=zlib-stream"));
        Ok(response)
    };
    let Ok(socket) = tokio_tungstenite::accept_hdr_async(stream, callback).await else { return };
    state.connections.fetch_add(1, Ordering::SeqCst);

    state.open.lock().push(commands);
    let mut world = state.world.subscribe();
    let mut conn = Connection {
        socket,
        state: state.clone(),
        deflater: compressed.then(Deflater::new),
        session: None,
        seq: 0,
        shard: 0,
        shard_count: 1,
        stalled: false,
        held: Vec::new(),
    };

    let interval = state.heartbeat_interval_ms.load(Ordering::SeqCst);
    if !conn.send(&json!({"op": 10, "s": null, "t": null, "d": {"heartbeat_interval": interval}})).await {
        return;
    }
    let pending: Vec<Command> = std::mem::take(&mut *state.pending.lock());
    for command in pending {
        if !conn.command(command).await {
            return;
        }
    }

    loop {
        let go_on = tokio::select! {
            frame = conn.socket.next() => match frame {
                Some(Ok(Message::Text(text))) => conn.frame(text.as_bytes()).await,
                Some(Ok(Message::Binary(bytes))) => conn.frame(&bytes).await,
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => false,
                Some(Ok(_)) => true,
            },
            Some(command) = inbox.recv() => conn.command(command).await,
            event = world.recv() => match event {
                Ok(event) if !conn.owns(&event.t, &event.d) => true,
                Ok(event) => conn.dispatch(event.t, event.d).await,
                Err(broadcast::error::RecvError::Lagged(_)) => true,
                Err(broadcast::error::RecvError::Closed) => false,
            },
        };
        if !go_on {
            return;
        }
    }
}

impl Connection {
    /// Whether an event is for this shard: the one of its guild, shard 0 when it has none.
    fn owns(&self, t: &str, d: &Value) -> bool {
        let guild_field = if matches!(t, "GUILD_CREATE" | "GUILD_UPDATE" | "GUILD_DELETE") { "id" } else { "guild_id" };
        let guild = d.get(guild_field).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok());
        let shard = guild.map_or(0, |g| (g >> 22) % u64::from(self.shard_count.max(1)));
        shard == u64::from(self.shard)
    }

    /// Sends one payload. False when the connection is gone.
    async fn send(&mut self, payload: &Value) -> bool {
        if self.stalled {
            return true;
        }
        let text = payload.to_string();
        let Some(deflater) = &mut self.deflater else {
            return self.socket.send(Message::text(text)).await.is_ok();
        };
        let bytes = deflater.frame(text.as_bytes());
        let parts = usize::try_from(self.state.split.load(Ordering::SeqCst)).unwrap_or(1).max(1);
        let size = bytes.len().div_ceil(parts).max(1);
        for chunk in bytes.chunks(size) {
            if self.socket.send(Message::binary(chunk.to_vec())).await.is_err() {
                return false;
            }
        }
        true
    }

    /// Sends a dispatch with the next sequence number, and keeps it for replay.
    async fn dispatch(&mut self, t: String, d: Value) -> bool {
        let Some(session) = self.session.clone() else {
            self.held.push((t, d));
            return true;
        };
        self.seq += 1;
        let payload = json!({"op": 0, "s": self.seq, "t": t, "d": d});
        if let Some(sent) = self.state.sessions.lock().get_mut(&session) {
            sent.push((self.seq, t, d));
        }
        self.send(&payload).await
    }

    async fn close(&mut self, code: u16) {
        let frame = CloseFrame { code: code.into(), reason: "".into() };
        // The peer may be gone already, which is all a close would have told it.
        let _ = self.socket.close(Some(frame)).await;
    }

    async fn command(&mut self, command: Command) -> bool {
        match command {
            Command::Dispatch(t, d) => self.dispatch(t, d).await,
            Command::Close(code) => {
                self.close(code).await;
                false
            }
            Command::Reconnect => self.send(&json!({"op": 7, "s": null, "t": null, "d": null})).await,
            Command::InvalidSession(resumable) => {
                if !resumable && let Some(session) = self.session.take() {
                    self.state.sessions.lock().remove(&session);
                }
                self.send(&json!({"op": 9, "s": null, "t": null, "d": resumable})).await
            }
            Command::Stall => {
                self.stalled = true;
                true
            }
        }
    }

    /// Handles one frame from the client.
    async fn frame(&mut self, bytes: &[u8]) -> bool {
        let Ok(frame) = serde_json::from_slice::<Value>(bytes) else {
            self.close(4002).await;
            return false;
        };
        let op = frame.get("op").and_then(Value::as_u64).and_then(|op| u32::try_from(op).ok()).unwrap_or(u32::MAX);
        let d = frame.get("d").cloned().unwrap_or(Value::Null);
        if op == 2
            && let Some(shard) = d.pointer("/shard/0").and_then(Value::as_u64).and_then(|s| u32::try_from(s).ok())
        {
            self.shard = shard;
            self.shard_count =
                d.pointer("/shard/1").and_then(Value::as_u64).and_then(|s| u32::try_from(s).ok()).unwrap_or(1).max(1);
        }
        self.state.frames.lock().push(RecordedFrame { shard: self.shard, op, d: d.clone() });
        match op {
            1 => {
                let dropped =
                    self.state.drop_acks.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
                dropped || self.send(&json!({"op": 11, "s": null, "t": null, "d": null})).await
            }
            2 => self.identify(&d).await,
            6 => self.resume(&d).await,
            4 => self.voice_state(&d).await,
            8 => self.request_members(&d).await,
            3 | 31 => true,
            _ => {
                self.close(4001).await;
                false
            }
        }
    }

    /// Op 4: moves the bot's voice state, and on a join hands out the mock voice server, as Discord
    /// does with `VOICE_STATE_UPDATE` then `VOICE_SERVER_UPDATE`.
    async fn voice_state(&mut self, d: &Value) -> bool {
        let world = self.state.world.clone();
        let Some(guild) = d.get("guild_id").and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()) else {
            self.close(4001).await;
            return false;
        };
        let channel = d.get("channel_id").and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok());
        let bot = world.bot_id().get();
        let state = world.set_voice_state(guild, bot, channel, d.get("self_mute") == Some(&Value::Bool(true)));
        if !self.dispatch("VOICE_STATE_UPDATE".to_owned(), state).await {
            return false;
        }
        if channel.is_none() {
            return true;
        }
        let server = json!({
            "token": crate::voice::VOICE_TOKEN,
            "guild_id": guild.to_string(),
            "endpoint": self.state.voice_url,
        });
        self.dispatch("VOICE_SERVER_UPDATE".to_owned(), server).await
    }

    /// Op 8: answers with `GUILD_MEMBERS_CHUNK` events, as Discord does: every member, those whose
    /// username starts with `query` (up to `limit`), or the `user_ids` named.
    async fn request_members(&mut self, d: &Value) -> bool {
        let snowflake = |v: &Value| v.as_str().and_then(|s| s.parse::<u64>().ok());
        let Some(guild) = d.get("guild_id").and_then(snowflake) else {
            self.close(4001).await;
            return false;
        };
        let world = self.state.world.clone();
        let all = world.members(guild, usize::MAX, 0);
        let user_of = |m: &Value| m.pointer("/user/id").and_then(snowflake);
        let mut not_found = Vec::new();
        let members: Vec<Value> = if let Some(ids) = d.get("user_ids").and_then(Value::as_array) {
            let ids: Vec<u64> = ids.iter().filter_map(snowflake).collect();
            not_found = ids.iter().filter(|id| world.member(guild, **id).is_none()).map(u64::to_string).collect();
            all.into_iter().filter(|m| user_of(m).is_some_and(|u| ids.contains(&u))).collect()
        } else {
            let query = d.get("query").and_then(Value::as_str).unwrap_or_default().to_lowercase();
            let limit = d
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|l| *l > 0)
                .map_or(usize::MAX, |l| usize::try_from(l).unwrap_or(usize::MAX));
            let name =
                |m: &Value| m.pointer("/user/username").and_then(Value::as_str).unwrap_or_default().to_lowercase();
            all.into_iter().filter(|m| name(m).starts_with(&query)).take(limit).collect()
        };
        let size = usize::try_from(self.state.chunk_size.load(Ordering::SeqCst)).unwrap_or(1000).max(1);
        let chunks: Vec<&[Value]> = if members.is_empty() { vec![&[]] } else { members.chunks(size).collect() };
        let count = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let mut body = json!({
                "guild_id": guild.to_string(),
                "members": chunk,
                "chunk_index": index,
                "chunk_count": count,
            });
            if index == 0 && !not_found.is_empty() {
                body["not_found"] = json!(not_found);
            }
            if let Some(nonce) = d.get("nonce") {
                body["nonce"] = nonce.clone();
            }
            if !self.dispatch("GUILD_MEMBERS_CHUNK".to_owned(), body).await {
                return false;
            }
        }
        true
    }

    async fn identify(&mut self, d: &Value) -> bool {
        if self.session.is_some() {
            self.close(4005).await;
            return false;
        }
        let token = d.get("token").and_then(Value::as_str).unwrap_or_default();
        if token.is_empty() || token == BAD_TOKEN || token.starts_with("Bot ") {
            self.close(4004).await;
            return false;
        }
        let number = self.state.next_session.fetch_add(1, Ordering::SeqCst);
        let session = format!("mock-session-{number}");
        self.state.sessions.lock().insert(session.clone(), Vec::new());
        self.session = Some(session.clone());
        self.seq = 0;

        let world = self.state.world.clone();
        // As on Discord, a shard gets only its own guilds: `(guild_id >> 22) % shard_count`.
        let (shard, count) = (u64::from(self.shard), u64::from(self.shard_count));
        let ids: Vec<_> = world.guild_ids().into_iter().filter(|id| (id.get() >> 22) % count == shard).collect();
        let guilds: Vec<Value> = ids.iter().map(|id| json!({"id": id.to_string(), "unavailable": true})).collect();
        let ready = json!({
            "v": 10,
            "user": world.bot_user(),
            "guilds": guilds,
            "session_id": session,
            "resume_gateway_url": self.state.url,
            "shard": [self.shard, self.shard_count],
            "application": {"id": world.application_id().to_string(), "flags": 0},
        });
        if !self.dispatch("READY".to_owned(), ready).await {
            return false;
        }
        for id in ids {
            if let Some(guild) = world.guild_payload(id)
                && !self.dispatch("GUILD_CREATE".to_owned(), guild).await
            {
                return false;
            }
        }
        for (t, d) in std::mem::take(&mut self.held) {
            if self.owns(&t, &d) && !self.dispatch(t, d).await {
                return false;
            }
        }
        true
    }

    async fn resume(&mut self, d: &Value) -> bool {
        let session = d.get("session_id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let after = d.get("seq").and_then(Value::as_u64).unwrap_or(0);
        let sent = self.state.sessions.lock().get(&session).cloned();
        let Some(sent) = sent else {
            return self.send(&json!({"op": 9, "s": null, "t": null, "d": false})).await;
        };
        self.seq = sent.last().map_or(0, |(seq, _, _)| *seq);
        self.session = Some(session);
        for (seq, t, body) in sent {
            if seq > after && !self.send(&json!({"op": 0, "s": seq, "t": t, "d": body})).await {
                return false;
            }
        }
        if !self.dispatch("RESUMED".to_owned(), json!({})).await {
            return false;
        }
        for (t, d) in std::mem::take(&mut self.held) {
            if !self.dispatch(t, d).await {
                return false;
            }
        }
        true
    }
}
