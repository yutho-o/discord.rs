//! A moderation bot: `/ban`, `/timeout`, `/purge`, `/role`, `/lock`, and a log of bans and reactions.
//!
//! Set `DISCORD_TOKEN`, and `DISCORD_GUILD_ID` to register commands in one guild. The bot needs the
//! Ban Members, Moderate Members, Manage Messages, Manage Roles and Manage Channels permissions, and its role must be
//! above the roles it hands out. Run with `cargo run -p discord-rust --example moderation_bot`.
//!
//! Each command acts with the bot's powers, so each handler checks the permissions of the person who
//! used it. `default_member_permissions` only hides the command, and server admins can change it.

use discord::prelude::*;
use std::time::Duration;

/// Answers and returns from the handler unless the person holds this permission.
macro_rules! require {
    ($cmd:expr, $permission:expr) => {
        if !$cmd.has_permissions($permission) {
            $cmd.reply("You do not have the permission to do that.").ephemeral().await?;
            return Ok(());
        }
    };
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt::init();
    let token = std::env::var("DISCORD_TOKEN").map_err(|_| "set DISCORD_TOKEN to your bot's token")?;
    let guild_id: GuildId = std::env::var("DISCORD_GUILD_ID").map_err(|_| "set DISCORD_GUILD_ID")?.parse()?;

    let client = Client::builder(token)
        .intents(Intents::GUILDS | Intents::GUILD_MODERATION | Intents::GUILD_MESSAGE_REACTIONS)
        .on(ClientReady, move |ctx, ready| async move {
            println!("logged in as {}", ready.user.username);
            let status = UpdatePresence::new(PresenceStatus::Online).activity(Activity::watching("the server"));
            ctx.set_presence(status)?;
            let cmds = commands![
                SlashCommandBuilder::new("ban", "Bans a member")
                    .default_member_permissions(Permissions::BAN_MEMBERS)
                    .user_option("user", "Who to ban", |o| o.required(true))
                    .string_option("reason", "Why", |o| o),
                SlashCommandBuilder::new("timeout", "Times a member out")
                    .default_member_permissions(Permissions::MODERATE_MEMBERS)
                    .user_option("user", "Who to time out", |o| o.required(true))
                    .integer_option("minutes", "For how long", |o| o.required(true)),
                SlashCommandBuilder::new("purge", "Deletes recent messages")
                    .default_member_permissions(Permissions::MANAGE_MESSAGES)
                    .integer_option("count", "How many, 2 to 100", |o| o.required(true)),
                SlashCommandBuilder::new("role", "Gives a member a role")
                    .default_member_permissions(Permissions::MANAGE_ROLES)
                    .user_option("user", "Who", |o| o.required(true))
                    .role_option("role", "Which role", |o| o.required(true)),
                SlashCommandBuilder::new("lock", "Stops everyone from writing in this channel")
                    .default_member_permissions(Permissions::MANAGE_CHANNELS),
            ];
            ctx.commands().set_guild(guild_id, cmds).await?;
            Ok(())
        })
        .on_command("ban", move |ctx, cmd| async move {
            require!(cmd, Permissions::BAN_MEMBERS);
            let user = cmd.options().require_user("user")?;
            let reason = cmd.options().string("reason").unwrap_or("no reason given").to_owned();
            ctx.guild(guild_id).members().ban(user).reason(reason).await?;
            cmd.reply("Banned.").ephemeral().await?;
            Ok(())
        })
        .on_command("timeout", move |ctx, cmd| async move {
            require!(cmd, Permissions::MODERATE_MEMBERS);
            let user = cmd.options().require_user("user")?;
            let minutes = u64::try_from(cmd.options().require_integer("minutes")?)?;
            ctx.guild(guild_id).members().timeout(user, Duration::from_secs(minutes * 60)).await?;
            cmd.reply(format!("Timed out for {minutes} minutes.")).ephemeral().await?;
            Ok(())
        })
        .on_command("purge", |ctx, cmd| async move {
            require!(cmd, Permissions::MANAGE_MESSAGES);
            let channel = cmd.channel_id.ok_or("this command needs a channel")?;
            let count = u8::try_from(cmd.options().require_integer("count")?)?;
            let messages = ctx.channel(channel).messages();
            let page = messages.list(MessagePage::new().limit(count)).await?;
            let deleted = page.len();
            messages.bulk_delete(page.iter().map(|m| m.id)).await?;
            cmd.reply(format!("Deleted {deleted} messages.")).ephemeral().await?;
            Ok(())
        })
        .on_command("role", move |ctx, cmd| async move {
            require!(cmd, Permissions::MANAGE_ROLES);
            let user = cmd.options().require_user("user")?;
            let role = cmd.options().require_role("role")?;
            ctx.guild(guild_id).members().add_role(user, role).await?;
            cmd.reply("Done.").ephemeral().await?;
            Ok(())
        })
        .on_command("lock", move |ctx, cmd| async move {
            require!(cmd, Permissions::MANAGE_CHANNELS);
            let channel = ctx.channel(cmd.channel_id.ok_or("this command needs a channel")?);
            // Computed from the cache: the bot checks it can before it tries.
            if !channel.my_permissions().is_some_and(|p| p.contains(Permissions::MANAGE_ROLES)) {
                cmd.reply("I cannot change the permissions of this channel.").ephemeral().await?;
                return Ok(());
            }
            // The @everyone role has the guild's id.
            let everyone = RoleId::new(guild_id.get());
            let deny = PermissionOverwrite::role(everyone, Permissions::empty(), Permissions::SEND_MESSAGES);
            channel.set_permission(deny).reason("locked with /lock").await?;
            cmd.reply("Channel locked.").await?;
            Ok(())
        })
        .on(GuildBanAdd, |_ctx, ban| async move {
            println!("{} was banned from {}", ban.user.username, ban.guild_id);
            Ok(())
        })
        .on(MessageReactionAdd, |_ctx, reaction| async move {
            println!("{} reacted to {}", reaction.user_id, reaction.message_id);
            Ok(())
        })
        .on_handler_error(|failure| eprintln!("{} failed: {}", failure.event, failure.kind))
        .build()?;
    client.run_until(tokio::signal::ctrl_c()).await?;
    Ok(())
}
