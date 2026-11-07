//! The socket task: `select!` over commands, heartbeat, write queue and read, guarded by `!paused`.
//!
//! One deliberate deviation from `ARCHITECTURE.md` 10.1: an op 9 with `d: true` resumes over a *new*
//! socket rather than the same one. Discord's documentation describes reconnecting, and a new socket
//! works either way, whereas resuming on the old one was never checked against a live session.

use super::ShardOptions;
use super::close::{self, CloseAction};
use super::heartbeat::Heartbeat;
use super::identify::{identify_json, resume_json};
use super::send::SendBudget;
use super::state::{Session, Shared};
use crate::compress::Decompressor;
use crate::connector::connect_url;
use crate::envelope;
use crate::events::{Flow, Frame, ShardNotice, ShardStatus, ShutdownMode};
use crate::session::SessionInfo;
use compact_str::CompactString;
use dr_core::Secret;
use dr_model::{DispatchKind, GatewayFrame, GatewayOpcode as Opcode};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::{Instant, sleep, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connect, TLS, upgrade and Hello together.
const HELLO_TIMEOUT: Duration = Duration::from_secs(60);
/// From identify to `READY`.
const READY_TIMEOUT: Duration = Duration::from_secs(15);
/// From resume to `RESUMED`.
const RESUMED_TIMEOUT: Duration = Duration::from_secs(30);
/// Waiting for the peer's close frame.
const CLOSE_WAIT: Duration = Duration::from_secs(5);
/// A stuck write counts as a dead connection.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long reading may stay paused before the connection is replaced.
const PAUSE_LIMIT: Duration = Duration::from_secs(30);
/// `GUILD_CREATE` is unbounded, so this is large on purpose.
const MAX_MESSAGE: usize = 256 * 1024 * 1024;
/// Minimum gap between identifies, 5 s per slot upstream.
const IDENTIFY_GAP: Duration = Duration::from_secs(5);
const SESSION_SAVE_EVERY: Duration = Duration::from_secs(30);
/// Our own close code, which tells Discord the session stays resumable.
const RESUMABLE_CLOSE: u16 = 4200;

/// Everything the socket task needs. Lives in an `Arc`, so a restart after a panic keeps the session.
pub(super) struct Ctx {
    pub(super) options: ShardOptions,
    pub(super) shared: Arc<Shared>,
    pub(super) connector: Connector,
}

/// How a connection ended.
enum End {
    /// Shutdown was asked for.
    Stop,
    /// A terminal close code.
    Failed(u16),
    /// Connect again. `identify` forgets the session first.
    Reconnect { code: Option<u16>, identify: bool, delay: Duration },
}

impl End {
    const fn resume(code: Option<u16>) -> Self {
        Self::Reconnect { code, identify: false, delay: Duration::ZERO }
    }
    const fn identify(code: Option<u16>) -> Self {
        Self::Reconnect { code, identify: true, delay: Duration::ZERO }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitHello,
    Identifying,
    Resuming,
    Ready,
}

/// Runs the socket task, restarting it after a panic. The session lives in `Shared`, so it survives.
pub(super) async fn supervise(ctx: Arc<Ctx>) {
    let mut restarts: u32 = 0;
    loop {
        match tokio::spawn(run(Arc::clone(&ctx))).await {
            Ok(()) => break,
            Err(e) => {
                tracing::error!(shard = ctx.options.shard_id, error = %e, "shard task died, restarting it");
                restarts = restarts.saturating_add(1);
                let mut rx = ctx.shared.shutdown.subscribe();
                tokio::select! {
                    () = sleep(backoff(restarts)) => {}
                    _ = rx.wait_for(Option::is_some) => break,
                }
            }
        }
    }
    ctx.shared.done.send_replace(true);
}

/// Floor 500 ms, plus exponential backoff with full jitter, base 1 s, cap 60 s.
fn backoff(failures: u32) -> Duration {
    let ceiling =
        Duration::from_secs(1).saturating_mul(1u32 << failures.saturating_sub(1).min(6)).min(Duration::from_secs(60));
    Duration::from_millis(500) + ceiling.mul_f64(rand::random::<f64>())
}

async fn run(ctx: Arc<Ctx>) {
    let shard = ctx.options.shard_id;
    let mut failures: u32 = 0;
    let mut last_identify: Option<Instant> = None;
    let mut extra = Duration::ZERO;
    let mut first = true;
    let mut shutdown = ctx.shared.shutdown.subscribe();
    loop {
        if shutdown.borrow().is_some() {
            return;
        }
        let delay = if first { Duration::ZERO } else { backoff(failures) + extra };
        first = false;
        ctx.shared.set_status(ShardStatus::Connecting);
        tokio::select! {
            () = sleep(delay) => {}
            _ = shutdown.wait_for(Option::is_some) => return,
        }
        let started = Instant::now();
        let end = connection(&ctx, &mut shutdown, &mut last_identify).await;
        if started.elapsed() >= Duration::from_secs(60) {
            failures = 0;
        }
        failures = failures.saturating_add(1);
        match end {
            End::Stop => return,
            End::Failed(code) => {
                tracing::error!(shard, code, "terminal gateway close code");
                ctx.shared.forget_session();
                ctx.options.session_store.save(shard, None).await;
                ctx.shared.set_status(ShardStatus::Failed(code));
                ctx.options.sink.notice(ShardNotice::Failed { shard, code });
                return;
            }
            End::Reconnect { code, identify, delay } => {
                tracing::debug!(shard, ?code, identify, "gateway connection ended");
                if identify {
                    ctx.shared.forget_session();
                    ctx.options.session_store.save(shard, None).await;
                }
                extra = delay;
                ctx.options.sink.notice(ShardNotice::Disconnected { shard, code });
            }
        }
    }
}

fn session_info(ctx: &Ctx) -> Option<SessionInfo> {
    let (session, sequence) = ctx.shared.resumable()?;
    Some(SessionInfo::new(session.id, session.resume_url, sequence, ctx.options.shard_id, ctx.options.shard_count))
}

/// Writes the session through on its own task: a slow store must not delay the socket.
fn save_session(ctx: &Arc<Ctx>) {
    let info = session_info(ctx);
    let ctx = Arc::clone(ctx);
    tokio::spawn(async move {
        ctx.options.session_store.save(ctx.options.shard_id, info).await;
    });
}

async fn write(ws: &mut Ws, text: String) -> bool {
    matches!(timeout(WRITE_TIMEOUT, ws.send(Message::text(text))).await, Ok(Ok(())))
}

async fn close_and_wait(ws: &mut Ws, code: u16) {
    let frame = CloseFrame { code: CloseCode::from(code), reason: "".into() };
    let _ = timeout(WRITE_TIMEOUT, ws.close(Some(frame))).await;
    let _ = timeout(CLOSE_WAIT, async {
        while let Some(Ok(message)) = ws.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    })
    .await;
}

#[derive(Deserialize)]
struct ReadyHead {
    session_id: Secret,
    resume_gateway_url: String,
}

async fn beat(ws: &mut Ws, ctx: &Ctx, heartbeat: &mut Heartbeat) -> bool {
    let Ok(text) = serde_json::to_string(&GatewayFrame::heartbeat(ctx.shared.sequence())) else { return false };
    heartbeat.on_sent(Instant::now());
    write(ws, text).await
}

async fn connection(
    ctx: &Arc<Ctx>,
    shutdown: &mut watch::Receiver<Option<ShutdownMode>>,
    last_identify: &mut Option<Instant>,
) -> End {
    let options = &ctx.options;
    let shard = options.shard_id;
    let resumable = ctx.shared.resumable();
    let base = resumable.as_ref().map_or(options.url.as_str(), |(s, _): &(Session, u64)| s.resume_url.as_str());
    let url = connect_url(base, options.compression);
    let Ok(mut decompressor) = Decompressor::new(options.compression) else {
        return End::Failed(0);
    };

    let config = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
    let connect = connect_async_tls_with_config(url, Some(config), true, Some(ctx.connector.clone()));
    let hello_deadline = Instant::now() + HELLO_TIMEOUT;
    let mut ws: Ws = tokio::select! {
        _ = shutdown.wait_for(Option::is_some) => return End::Stop,
        result = timeout(HELLO_TIMEOUT, connect) => match result {
            Ok(Ok((ws, _))) => ws,
            Ok(Err(e)) => {
                // tungstenite's error text holds no token: the URL has none.
                tracing::warn!(shard, error = %e, "gateway connect failed");
                return End::resume(None);
            }
            Err(_) => return End::identify(None),
        },
    };

    let mut phase = Phase::AwaitHello;
    let mut phase_deadline = Some(hello_deadline);
    let mut heartbeat: Option<Heartbeat> = None;
    let mut next_beat: Option<Instant> = None;
    let mut identify_at: Option<Instant> = None;
    let mut paused_at: Option<Instant> = None;
    let mut budget = SendBudget::new(Instant::now());
    let mut send_wake: Option<Instant> = None;
    let mut next_save = Instant::now() + SESSION_SAVE_EVERY;

    loop {
        // Drain what the budget allows. Only Ready sends these, which fixes upstream defect 15.1.
        if phase == Phase::Ready && send_wake.is_none_or(|at| Instant::now() >= at) {
            send_wake = None;
            loop {
                let Some(text) = ctx.shared.queue.lock().pop_front() else { break };
                match budget.try_take(Instant::now(), rand::random::<f64>()) {
                    Ok(()) => {
                        if !write(&mut ws, text.clone()).await {
                            ctx.shared.queue.lock().push_front(text);
                            return End::resume(None);
                        }
                    }
                    Err(wait) => {
                        ctx.shared.queue.lock().push_front(text);
                        send_wake = Some(Instant::now() + wait);
                        break;
                    }
                }
            }
        }

        let idle_forever = Instant::now() + Duration::from_secs(86_400);
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                let mode = if changed.is_ok() { *shutdown.borrow() } else { Some(ShutdownMode::Final) };
                let mode = mode.unwrap_or_default();
                if mode == ShutdownMode::Resumable {
                    options.session_store.save(shard, session_info(ctx)).await;
                    close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                } else {
                    ctx.shared.forget_session();
                    options.session_store.save(shard, None).await;
                    close_and_wait(&mut ws, 1000).await;
                }
                return End::Stop;
            }
            () = sleep_until(next_beat.unwrap_or(idle_forever)), if next_beat.is_some() => {
                let Some(hb) = heartbeat.as_mut() else { continue };
                // While reading is paused an ack cannot arrive, so a missing one proves nothing.
                if hb.is_zombie() && paused_at.is_none() {
                    tracing::warn!(shard, "missed heartbeat ack, reconnecting");
                    close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                    return End::resume(None);
                }
                let interval = hb.interval();
                if !beat(&mut ws, ctx, hb).await {
                    return End::resume(None);
                }
                next_beat = Some(Instant::now() + interval);
            }
            () = sleep_until(phase_deadline.unwrap_or(idle_forever)), if phase_deadline.is_some() => {
                tracing::warn!(shard, "gateway handshake timed out");
                let resuming = phase == Phase::Resuming;
                close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                return if resuming { End::resume(None) } else { End::identify(None) };
            }
            () = sleep_until(identify_at.unwrap_or(idle_forever)), if identify_at.is_some() => {
                identify_at = None;
                let Ok(text) = identify_json(options) else { return End::identify(None) };
                *last_identify = Some(Instant::now());
                if !write(&mut ws, text).await {
                    return End::identify(None);
                }
                phase_deadline = Some(Instant::now() + READY_TIMEOUT);
            }
            () = sleep_until(send_wake.unwrap_or(idle_forever)), if send_wake.is_some() => {}
            () = ctx.shared.queue_ready.notified() => {}
            () = options.sink.wait_capacity(), if paused_at.is_some() => {
                paused_at = None;
            }
            () = sleep_until(paused_at.map_or(idle_forever, |at| at + PAUSE_LIMIT)), if paused_at.is_some() => {
                tracing::warn!(shard, "reading stayed paused too long, reconnecting");
                close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                return End::resume(None);
            }
            () = sleep_until(next_save), if phase == Phase::Ready => {
                next_save = Instant::now() + SESSION_SAVE_EVERY;
                save_session(ctx);
            }
            message = ws.next(), if paused_at.is_none() => {
                let text = match message {
                    Some(Ok(Message::Text(text))) => text.as_str().to_owned(),
                    Some(Ok(Message::Binary(bytes))) => match decompressor.push(&bytes) {
                        Ok(Some(raw)) => match String::from_utf8(raw) {
                            Ok(text) => text,
                            Err(_) => {
                                tracing::warn!(shard, "dropped a gateway frame that was not UTF-8");
                                continue;
                            }
                        },
                        Ok(None) => continue,
                        Err(e) => {
                            tracing::warn!(shard, error = %e, "inflate failed, reconnecting");
                            return End::resume(None);
                        }
                    },
                    Some(Ok(Message::Close(frame))) => {
                        let code = frame.map(|f| u16::from(f.code));
                        return match close::action(code) {
                            CloseAction::Resume => End::resume(code),
                            CloseAction::Identify => End::identify(code),
                            CloseAction::Fatal => End::Failed(code.unwrap_or(0)),
                        };
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        tracing::debug!(shard, error = %e, "gateway read failed");
                        return End::resume(None);
                    }
                    None => return End::resume(None),
                };

                let Ok(head) = envelope::parse(&text) else {
                    tracing::warn!(shard, "dropped an unparsable gateway frame");
                    continue;
                };
                let op = Opcode::from(head.op);
                if op == Opcode::DISPATCH {
                    // The sequence advances before anything else, so an event that later fails to
                    // decode is still acknowledged.
                    if let Some(s) = head.s {
                        ctx.shared.advance(s);
                    }
                    let (Some(t), Some(d)) = (head.t.as_deref(), head.d) else { continue };
                    let kind = DispatchKind::from_t(t);
                    let name = CompactString::new(t);
                    if t == "READY" {
                        match serde_json::from_str::<ReadyHead>(d.get()) {
                            Ok(ready) => {
                                *ctx.shared.session.lock() = Some(Session { id: ready.session_id, resume_url: ready.resume_gateway_url });
                            }
                            Err(e) => tracing::warn!(shard, error = %e, "READY had no session, a resume will not be possible"),
                        }
                        phase = Phase::Ready;
                        phase_deadline = None;
                        ctx.shared.set_status(ShardStatus::Ready);
                        save_session(ctx);
                        options.sink.notice(ShardNotice::Ready { shard });
                    } else if t == "RESUMED" {
                        phase = Phase::Ready;
                        phase_deadline = None;
                        ctx.shared.set_status(ShardStatus::Ready);
                        save_session(ctx);
                        options.sink.notice(ShardNotice::Resumed { shard });
                    }
                    let range = envelope::d_range(&text, d.get());
                    drop(head);
                    let Some(range) = range else {
                        tracing::warn!(shard, event = %name, "could not locate the event body");
                        continue;
                    };
                    let frame = Frame { shard, kind, t: name, buf: text.into_bytes(), d: range };
                    if options.sink.push(frame) == Flow::Pause {
                        paused_at = Some(Instant::now());
                    }
                } else if op == Opcode::HEARTBEAT {
                    // A server request: beat now, skipping the ack check, and still expect an ack.
                    if let Some(hb) = heartbeat.as_mut() {
                        let interval = hb.interval();
                        if !beat(&mut ws, ctx, hb).await {
                            return End::resume(None);
                        }
                        next_beat = Some(Instant::now() + interval);
                    }
                } else if op == Opcode::RECONNECT {
                    close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                    return End::resume(None);
                } else if op == Opcode::INVALID_SESSION {
                    let can_resume = head.d.is_some_and(|d| d.get().trim() == "true");
                    // 1 to 5 s, as Discord asks, added to the normal reconnect delay.
                    let delay = Duration::from_secs(1) + Duration::from_secs(4).mul_f64(rand::random::<f64>());
                    close_and_wait(&mut ws, RESUMABLE_CLOSE).await;
                    return End::Reconnect { code: None, identify: !can_resume, delay };
                } else if op == Opcode::HELLO {
                    let interval = head.d.and_then(|d| serde_json::from_str::<dr_model::Hello>(d.get()).ok());
                    let Some(interval) = interval else {
                        tracing::warn!(shard, "bad Hello, reconnecting");
                        return End::resume(None);
                    };
                    let hb = Heartbeat::new(Duration::from_millis(interval.heartbeat_interval));
                    next_beat = Some(Instant::now() + hb.first_delay(rand::random::<f64>()));
                    heartbeat = Some(hb);
                    if phase != Phase::AwaitHello {
                        continue;
                    }
                    if let Some((session, seq)) = ctx.shared.resumable() {
                        let Ok(text) = resume_json(options, &session.id, seq) else { return End::identify(None) };
                        phase = Phase::Resuming;
                        ctx.shared.set_status(ShardStatus::Resuming);
                        phase_deadline = Some(Instant::now() + RESUMED_TIMEOUT);
                        if !write(&mut ws, text).await {
                            return End::resume(None);
                        }
                    } else {
                        phase = Phase::Identifying;
                        ctx.shared.set_status(ShardStatus::Identifying);
                        phase_deadline = None;
                        let earliest = last_identify.map_or_else(Instant::now, |at| at + IDENTIFY_GAP);
                        identify_at = Some(match &options.identify_limiter {
                            Some(limiter) => limiter.reserve(shard, earliest),
                            None => earliest,
                        });
                    }
                } else if op == Opcode::HEARTBEAT_ACK
                    && let Some(hb) = heartbeat.as_mut() {
                        hb.on_ack(Instant::now());
                        if let Some(latency) = hb.latency() {
                            ctx.shared.set_latency(latency);
                        }
                    }
                // An unknown opcode is ignored.
            }
        }
    }
}
