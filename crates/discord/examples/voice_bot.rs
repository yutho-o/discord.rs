//! A voice bot: `/join` joins your voice channel, `/play` plays an Ogg Opus file, `/stop` stops it,
//! `/leave` leaves.
//!
//! The library does not encode audio. Convert a file once with
//! `ffmpeg -i song.mp3 -c:a libopus -b:a 96k -ar 48000 -ac 2 song.opus`.
//!
//! Set `DISCORD_TOKEN`, `DISCORD_GUILD_ID` and `VOICE_FILE` (the `.opus` file). Run with
//! `cargo run -p discord-rust --features voice --example voice_bot`.

use discord::prelude::*;
use discord::voice::{JoinOptions, OggOpus, Playback};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// The file to play, and what is playing in each guild.
struct Jukebox {
    file: String,
    playing: Mutex<HashMap<GuildId, Playback>>,
}

impl Jukebox {
    fn take(&self, guild: GuildId) -> Option<Playback> {
        self.playing.lock().unwrap_or_else(PoisonError::into_inner).remove(&guild)
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt::init();

    let token = std::env::var("DISCORD_TOKEN").map_err(|_| "set DISCORD_TOKEN to your bot's token")?;
    let guild_id: GuildId = std::env::var("DISCORD_GUILD_ID").map_err(|_| "set DISCORD_GUILD_ID")?.parse()?;
    let file = std::env::var("VOICE_FILE").map_err(|_| "set VOICE_FILE to an Ogg Opus file")?;

    Client::builder(token)
        .state(Jukebox { file, playing: Mutex::default() })
        .intents(Intents::GUILDS | Intents::GUILD_VOICE_STATES)
        .on(ClientReady, move |ctx, ready| async move {
            println!("logged in as {}", ready.user.username);
            let cmds = commands![
                SlashCommandBuilder::new("join", "Joins your voice channel"),
                SlashCommandBuilder::new("play", "Plays the file"),
                SlashCommandBuilder::new("stop", "Stops playing"),
                SlashCommandBuilder::new("leave", "Leaves the voice channel"),
            ];
            ctx.commands().set_guild(guild_id, cmds).await?;
            Ok(())
        })
        .on_command("join", |ctx, cmd| async move {
            let Some(guild) = cmd.guild_id else { return Ok(()) };
            let Some(channel) = ctx.cache().voice_state(guild, cmd.user.id).and_then(|s| s.channel_id) else {
                cmd.reply("Join a voice channel first.").ephemeral().await?;
                return Ok(());
            };
            // Joining takes a few round trips: answer at once, then edit.
            cmd.defer().await?;
            let connection = ctx.voice().join_with(guild, channel, JoinOptions::default().deafened()).await?;
            let dave = match connection.dave_version() {
                0 => "without end-to-end encryption".to_owned(),
                v => format!("with DAVE v{v}"),
            };
            cmd.edit_reply(EditMessage::new().content(format!("Joined, {dave}."))).await?;
            Ok(())
        })
        .on_command("play", |ctx, cmd| async move {
            let Some(guild) = cmd.guild_id else { return Ok(()) };
            let Some(connection) = ctx.voice().get(guild) else {
                cmd.reply("Use /join first.").ephemeral().await?;
                return Ok(());
            };
            let source = OggOpus::open(&ctx.state.file).await?;
            if let Some(old) = ctx.state.take(guild) {
                old.stop();
            }
            let playback = connection.play(source);
            ctx.state.playing.lock().unwrap_or_else(PoisonError::into_inner).insert(guild, playback.clone());
            cmd.reply("Playing.").await?;
            println!("{guild} finished playing: {:?}", playback.finished().await);
            Ok(())
        })
        .on_command("stop", |ctx, cmd| async move {
            let Some(guild) = cmd.guild_id else { return Ok(()) };
            if let Some(playback) = ctx.state.take(guild) {
                playback.stop();
            }
            cmd.reply("Stopped.").await?;
            Ok(())
        })
        .on_command("leave", |ctx, cmd| async move {
            let Some(guild) = cmd.guild_id else { return Ok(()) };
            ctx.state.take(guild);
            ctx.voice().leave(guild).await?;
            cmd.reply("Bye.").await?;
            Ok(())
        })
        .on_handler_error(|failure| eprintln!("{} failed: {}", failure.event, failure.kind))
        .build()?
        .run_until(tokio::signal::ctrl_c())
        .await?;
    Ok(())
}
