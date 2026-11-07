//! The proof of concept end to end, on the mock, through the public API only: a real socket, zlib,
//! REST over HTTP, and a bot built the way `examples/poc_bot.rs` builds one.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{callback_path, click, message, slash, string_option, wait_until};
use discord::prelude::*;
use discord::{Error, FailureKind};
use dr_mock::{Method, MockServer, RecordedRequest};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

#[derive(Default)]
struct State {
    ready: AtomicU32,
    seen: Mutex<Vec<String>>,
    tallies: Mutex<HashMap<MessageId, (u32, u32)>>,
    panics: AtomicU32,
}

struct Bot {
    server: MockServer,
    guild: GuildId,
    channel: ChannelId,
    alice: u64,
    state: Arc<Shared>,
    client: Client,
    run: JoinHandle<Result<(), Error>>,
}

struct Shared(Arc<State>);

/// A bot like the example's, on a mock with one guild, one channel and one other user.
async fn start() -> Bot {
    let server = MockServer::start().await.unwrap();
    let guild = server.world().add_guild("guild");
    let channel = server.world().add_channel(guild, "general");
    let alice = server.world().add_user("alice");
    server.world().add_member(guild, alice);
    let state = Arc::new(State::default());
    let client_state = state.clone();

    let runner = Client::builder("test-token")
        .state(client_state)
        .endpoints(server.endpoints())
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT)
        .on(ClientReady, |ctx, _ready| async move {
            ctx.state.ready.fetch_add(1, Ordering::SeqCst);
            let cmds = commands![
                SlashCommandBuilder::new("ping", "Shows the latency"),
                SlashCommandBuilder::new("echo", "Repeats").string_option("text", "What", |o| o.required(true)),
                SlashCommandBuilder::new("vote", "Votes").string_option("question", "What", |o| o.required(true)),
                SlashCommandBuilder::new("whoami", "Shows who you are"),
            ];
            ctx.commands().set_global(cmds).await?;
            Ok(())
        })
        .on(MessageCreate, |ctx, msg| async move {
            ctx.state.seen.lock().unwrap().push(msg.content.clone());
            if msg.content == "!ping" {
                msg.reply(format!("pong: {:?}", ctx.latency())).no_mentions().await?;
            }
            if msg.content == "!boom" {
                panic!("boom");
            }
            Ok(())
        })
        .on_command("ping", |ctx, cmd| async move {
            cmd.reply(format!("pong: {:?}", ctx.latency())).await?;
            Ok(())
        })
        .on_command("echo", |_ctx, cmd| async move {
            let text = cmd.options().require_string("text")?;
            cmd.reply(text).ephemeral().no_mentions().await?;
            Ok(())
        })
        .on_command("vote", |_ctx, cmd| async move {
            let question = cmd.options().require_string("question")?;
            let body = CreateMessage::new()
                .embed(EmbedBuilder::new().title("Vote").description(question))
                .row(ActionRowBuilder::new().success("vote:yes", "Yes").danger("vote:no", "No"));
            cmd.reply(body).await?;
            Ok(())
        })
        .on_component("vote:", |ctx, click| async move {
            let (yes, no) = {
                let mut tallies = ctx.state.tallies.lock().unwrap();
                let tally = tallies.entry(click.message.id).or_default();
                if click.custom_id == "vote:yes" {
                    tally.0 += 1;
                } else {
                    tally.1 += 1;
                }
                *tally
            };
            let question = click.message.embeds.first().and_then(|e| e.description.clone()).unwrap_or_default();
            let tally = EmbedBuilder::new()
                .title("Vote")
                .description(question)
                .field("Yes", yes.to_string(), true)
                .field("No", no.to_string(), true);
            click.update(EditMessage::new().embed(tally)).await?;
            Ok(())
        })
        .on_command("whoami", |ctx, cmd| async move {
            let user = ctx.users().resolve(cmd.user.id).await?;
            cmd.reply(format!("You are {}", user.username)).ephemeral().await?;
            Ok(())
        })
        .on_handler_error({
            let state = state.clone();
            move |failure| {
                if matches!(failure.kind, FailureKind::Panic(_)) {
                    state.panics.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .build()
        .unwrap();
    let client = runner.client();
    let run = tokio::spawn(runner.run());
    let shared = Arc::new(Shared(state));
    let bot = Bot { server, guild, channel, alice: alice.get(), state: shared, client, run };
    let ready = bot.state.0.clone();
    wait_until("ClientReady", || ready.ready.load(Ordering::SeqCst) == 1).await;
    bot
}

impl Bot {
    fn requests(&self, method: Method, path: &str) -> Vec<RecordedRequest> {
        self.server.requests().into_iter().filter(|r| r.method == method && r.path == path).collect()
    }

    /// Dispatches an interaction, and waits for its one callback.
    async fn callback_of(&self, interaction: Value) -> Value {
        let path = callback_path(&interaction);
        self.server.gateway().dispatch("INTERACTION_CREATE", interaction);
        let mut found = Vec::new();
        wait_until("the callback", || {
            found = self.requests(Method::POST, &path);
            !found.is_empty()
        })
        .await;
        assert_eq!(found.len(), 1, "exactly one callback");
        found.remove(0).json()
    }

    fn seen(&self) -> Vec<String> {
        self.state.0.seen.lock().unwrap().clone()
    }

    async fn send_user_message(&self, text: &str) {
        let body = message(self.server.world(), self.guild, self.channel, self.alice, text);
        self.server.gateway().dispatch("MESSAGE_CREATE", body);
        wait_until(text, || self.seen().iter().any(|s| s == text)).await;
    }

    async fn stop(self) {
        self.client.shutdown(ShutdownMode::Final).await;
        self.run.await.unwrap().unwrap();
    }
}

#[tokio::test]
async fn login_reaches_ready_with_a_full_cache() {
    let bot = start().await;
    let frames = bot.server.frames(0);
    let identify = frames.iter().find(|f| f.op == 2).expect("an identify");
    assert_eq!(identify.d["token"], "test-token");
    let intents = identify.d["intents"].as_u64().unwrap();
    assert_ne!(intents & (1 << 15), 0, "MESSAGE_CONTENT");
    assert_eq!(identify.d["shard"], json!([0, 1]));

    assert_eq!(bot.client.application_id(), Some(bot.server.world().application_id()));
    assert_eq!(bot.client.users().me().unwrap().username, "mockbot");
    assert_eq!(bot.client.guilds().get(bot.guild).unwrap().name, "guild");
    assert_eq!(bot.client.channels().get(bot.channel).unwrap().guild_id, Some(bot.guild));

    // `ClientReady` registered the commands, with the bot token.
    let world = bot.server.world();
    wait_until("the commands", || world.commands(None).len() == 4).await;
    let path = format!("/applications/{}/commands", world.application_id());
    let put = bot.requests(Method::PUT, &path);
    assert_eq!(put[0].header("authorization"), Some("Bot test-token"));
    assert_eq!(put[0].json()[1]["options"][0]["name"], "text");
    bot.stop().await;
}

#[tokio::test]
async fn ping_message_gets_a_reply_with_a_nonce() {
    let bot = start().await;
    bot.send_user_message("!ping").await;
    let path = format!("/channels/{}/messages", bot.channel);
    wait_until("the reply", || !bot.requests(Method::POST, &path).is_empty()).await;
    let body = bot.requests(Method::POST, &path)[0].json();
    assert!(body["content"].as_str().unwrap().starts_with("pong"), "{body}");
    assert!(body["message_reference"]["message_id"].is_string(), "{body}");
    assert_eq!(body["allowed_mentions"]["parse"], json!([]), "{body}");
    assert!(body["nonce"].is_string(), "{body}");
    assert_eq!(body["enforce_nonce"], true, "{body}");
    bot.stop().await;
}

#[tokio::test]
async fn each_slash_command_answers() {
    let bot = start().await;
    let world = bot.server.world();
    let (g, c, a) = (bot.guild, bot.channel, bot.alice);

    let ping = bot.callback_of(slash(world, g, c, a, "ping", json!([]))).await;
    assert_eq!(ping["type"], 4);
    assert!(ping["data"]["content"].as_str().unwrap().starts_with("pong"));

    let options = json!([string_option("text", "hello")]);
    let echo = bot.callback_of(slash(world, g, c, a, "echo", options)).await;
    assert_eq!(echo["data"]["content"], "hello");
    assert_eq!(echo["data"]["flags"], 64, "ephemeral");

    let options = json!([string_option("question", "Pizza?")]);
    let vote = bot.callback_of(slash(world, g, c, a, "vote", options)).await;
    assert_eq!(vote["data"]["embeds"][0]["description"], "Pizza?");
    let row = &vote["data"]["components"][0];
    assert_eq!(row["type"], 1);
    assert_eq!(row["components"][0]["custom_id"], "vote:yes");
    assert_eq!(row["components"][1]["custom_id"], "vote:no");

    // Alice is a cached member, so her user comes from the cache.
    let whoami = bot.callback_of(slash(world, g, c, a, "whoami", json!([]))).await;
    assert_eq!(whoami["data"]["content"], "You are alice");
    assert!(bot.requests(Method::GET, &format!("/users/{a}")).is_empty(), "served from the cache");

    // This user was never seen, so it comes from a fetch.
    let bob = world.add_user("bob").get();
    let mut body = slash(world, g, c, bob, "whoami", json!([]));
    body["member"]["user"]["username"] = json!("bob");
    bot.client.cache();
    let whoami = bot.callback_of(body).await;
    assert_eq!(whoami["data"]["content"], "You are bob");
    assert_eq!(bot.requests(Method::GET, &format!("/users/{bob}")).len(), 1, "fetched once");
    bot.stop().await;
}

#[tokio::test]
async fn button_clicks_update_the_message() {
    let bot = start().await;
    let world = bot.server.world();
    let (g, c, a) = (bot.guild, bot.channel, bot.alice);
    let mut on = message(world, g, c, a, "");
    on["embeds"] = json!([{"title": "Vote", "description": "Pizza?"}]);

    let first = bot.callback_of(click(world, g, c, a, "vote:yes", on.clone())).await;
    assert_eq!(first["type"], 7, "UPDATE_MESSAGE");
    let embed = &first["data"]["embeds"][0];
    assert_eq!(embed["description"], "Pizza?");
    assert_eq!(embed["fields"][0]["value"], "1");
    assert_eq!(embed["fields"][1]["value"], "0");

    let second = bot.callback_of(click(world, g, c, a, "vote:no", on)).await;
    let embed = &second["data"]["embeds"][0];
    assert_eq!(embed["fields"][0]["value"], "1");
    assert_eq!(embed["fields"][1]["value"], "1");
    bot.stop().await;
}

/// Asserts a resume happened after this many identifies, with no event lost or repeated.
async fn assert_resumed(bot: &Bot, before: &str, after: &str) {
    wait_until("a resume", || bot.server.frames(0).iter().any(|f| f.op == 6)).await;
    wait_until("the event after", || bot.seen().iter().any(|s| s == after)).await;
    let frames = bot.server.frames(0);
    assert_eq!(frames.iter().filter(|f| f.op == 2).count(), 1, "no second identify");
    let resume = frames.iter().find(|f| f.op == 6).unwrap();
    assert_eq!(resume.d["token"], "test-token");
    assert!(resume.d["seq"].as_u64().unwrap() >= 2, "{:?}", resume.d);
    let seen = bot.seen();
    assert_eq!(seen.iter().filter(|s| *s == before).count(), 1, "{seen:?}");
    assert_eq!(seen.iter().filter(|s| *s == after).count(), 1, "{seen:?}");
}

#[tokio::test]
async fn forced_reconnect_resumes_without_losing_events() {
    let bot = start().await;
    bot.send_user_message("before").await;
    let world = bot.server.world();
    bot.server.gateway().send_reconnect();
    wait_until("a second connection", || bot.server.gateway().connections() == 2).await;
    bot.server.gateway().dispatch("MESSAGE_CREATE", message(world, bot.guild, bot.channel, bot.alice, "after"));
    assert_resumed(&bot, "before", "after").await;
    assert_eq!(bot.state.0.ready.load(Ordering::SeqCst), 1, "ClientReady fires once");
    bot.stop().await;
}

#[tokio::test]
async fn events_sent_while_disconnected_are_replayed() {
    let bot = start().await;
    bot.send_user_message("before").await;
    let world = bot.server.world();
    // These go out on the old connection, right behind the close, or are held for the next one.
    bot.server.gateway().close(4000);
    bot.server.gateway().dispatch("MESSAGE_CREATE", message(world, bot.guild, bot.channel, bot.alice, "after"));
    assert_resumed(&bot, "before", "after").await;
    bot.stop().await;
}

#[tokio::test]
async fn invalid_session_identifies_again() {
    let bot = start().await;
    bot.send_user_message("before").await;
    bot.server.gateway().invalid_session(false);
    wait_until("a second identify", || bot.server.frames(0).iter().filter(|f| f.op == 2).count() == 2).await;
    bot.send_user_message("after").await;
    assert!(bot.server.frames(0).iter().all(|f| f.op != 6), "not resumable, so no resume");
    assert_eq!(bot.state.0.ready.load(Ordering::SeqCst), 1, "ClientReady fires once");
    bot.stop().await;
}

#[tokio::test]
async fn a_429_is_retried_without_a_duplicate_send() {
    let bot = start().await;
    let path = format!("/channels/{}/messages", bot.channel);
    bot.server.rest().on(Method::POST, path.clone()).times(1).respond(
        429,
        &[
            ("content-type", "application/json"),
            ("x-ratelimit-bucket", "abc"),
            ("x-ratelimit-limit", "5"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset-after", "0.1"),
            ("retry-after", "1"),
            ("x-ratelimit-scope", "user"),
        ],
        r#"{"message":"You are being rate limited.","retry_after":0.1,"global":false}"#,
    );
    bot.send_user_message("!ping").await;
    wait_until("the retry", || bot.requests(Method::POST, &path).len() == 2).await;
    let sent = bot.requests(Method::POST, &path);
    assert_eq!(sent[0].json()["nonce"], sent[1].json()["nonce"], "the retry is the same message");
    // The mock stores what got past the script, so the channel has the one reply and no other.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(bot.requests(Method::POST, &path).len(), 2);
    let replies = bot.seen().iter().filter(|s| s.starts_with("pong")).count();
    assert_eq!(replies, 1, "one MESSAGE_CREATE for the reply: {:?}", bot.seen());
    bot.stop().await;
}

#[tokio::test]
async fn a_panicking_handler_leaves_the_bot_running() {
    let bot = start().await;
    bot.send_user_message("!boom").await;
    let state = bot.state.0.clone();
    wait_until("the panic report", || state.panics.load(Ordering::SeqCst) == 1).await;
    assert_eq!(bot.client.stats().handler_panics(), 1);
    bot.send_user_message("!ping").await;
    let path = format!("/channels/{}/messages", bot.channel);
    wait_until("the reply after the panic", || !bot.requests(Method::POST, &path).is_empty()).await;
    bot.stop().await;
}

#[tokio::test]
async fn a_bad_token_ends_run_with_the_close_code() {
    let server = MockServer::start().await.unwrap();
    let runner = Client::builder(dr_mock::gateway::session::BAD_TOKEN).endpoints(server.endpoints()).build().unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(20), runner.run()).await.unwrap().unwrap_err();
    assert!(matches!(error, Error::Gateway(discord::gateway::GatewayError::Closed { code: 4004 })), "{error:?}");
}
