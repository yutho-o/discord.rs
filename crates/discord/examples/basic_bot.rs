//! A small bot: `!ping`, `/ping`, `/echo`, `/vote` with buttons, and `/whoami`.
//!
//! Set `DISCORD_TOKEN`, and `DISCORD_GUILD_ID` to register commands in one guild, where they appear at
//! once. Run with `cargo run -p discord-rust --example basic_bot`.

use discord::prelude::*;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// Yes and no counts, per vote message.
#[derive(Default)]
struct Votes {
    tallies: Mutex<HashMap<MessageId, (u32, u32)>>,
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // Shows the warnings of the library, such as a payload it could not decode.
    tracing_subscriber::fmt::init();

    let token = std::env::var("DISCORD_TOKEN").map_err(|_| "set DISCORD_TOKEN to your bot's token")?;
    let guild_id: Option<GuildId> = std::env::var("DISCORD_GUILD_ID").ok().map(|v| v.parse()).transpose()?;

    Client::builder(token)
        .state(Votes::default())
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT)
        .on(ClientReady, move |ctx, ready| async move {
            println!("logged in as {}", ready.user.username);
            let cmds = commands![
                SlashCommandBuilder::new("ping", "Shows the gateway latency"),
                SlashCommandBuilder::new("echo", "Repeats what you say, to you alone").string_option(
                    "text",
                    "What to repeat",
                    |o| o.required(true)
                ),
                SlashCommandBuilder::new("vote", "Starts a vote")
                    .string_option("question", "What to vote on", |o| o.required(true)),
                SlashCommandBuilder::new("whoami", "Shows who you are"),
            ];
            match guild_id {
                Some(guild) => {
                    ctx.commands().set_guild(guild, cmds).await?;
                }
                None => {
                    ctx.commands().set_global(cmds).await?;
                }
            }
            Ok(())
        })
        .on(MessageCreate, |ctx, msg| async move {
            if msg.content == "!ping" {
                msg.reply(format!("pong: {:?}", ctx.latency())).no_mentions().await?;
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
                let mut tallies = ctx.state.tallies.lock().unwrap_or_else(PoisonError::into_inner);
                let tally = tallies.entry(click.message.id).or_default();
                if click.custom_id == "vote:yes" {
                    tally.0 += 1;
                } else {
                    tally.1 += 1;
                }
                *tally
            };
            let question = click.message.embeds.first().and_then(|e| e.description.clone()).unwrap_or_default();
            let tally = EmbedBuilder::new().title("Vote").description(question).field("Yes", yes.to_string(), true);
            click.update(EditMessage::new().embed(tally.field("No", no.to_string(), true))).await?;
            Ok(())
        })
        .on_command("whoami", |ctx, cmd| async move {
            let user = ctx.users().resolve(cmd.user.id).await?;
            cmd.reply(format!("You are {}", user.username)).ephemeral().await?;
            Ok(())
        })
        .on_handler_error(|failure| eprintln!("{} failed: {}", failure.event, failure.kind))
        .build()?
        .run_until(tokio::signal::ctrl_c())
        .await?;
    Ok(())
}
