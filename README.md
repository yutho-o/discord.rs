# discord.rs

A Rust port of [discord.js](https://github.com/discordjs/discord.js), for writing complete Discord bots
in Rust. The package is `discord-rust`, the library is `discord`, so a bot writes `use discord::prelude::*`.

This is an unofficial project. It is not affiliated with or endorsed by Discord Inc. or the discord.js
project.

**Status: proof of concept.** It passes its tests against a local mock, and has never run against real
Discord. `docs/POC_STATUS.md` says what works, what is cut and what is unverified.

## Try it

Create a bot in the [developer portal](https://discord.com/developers/applications), turn on its
Message Content intent, invite it to a test server, then:

```sh
export DISCORD_TOKEN=...
export DISCORD_GUILD_ID=...
cargo run -p discord-rust --example poc_bot
```

`docs/POC_STATUS.md` has the detailed steps and what to check. A bot looks like this:

```rust,no_run
use discord::prelude::*;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    Client::builder(std::env::var("DISCORD_TOKEN")?)
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT)
        .on(MessageCreate, |_ctx, msg| async move {
            if msg.content == "!ping" {
                msg.reply("pong").no_mentions().await?;
            }
            Ok(())
        })
        .build()?
        .run_until(tokio::signal::ctrl_c())
        .await?;
    Ok(())
}
```

## Developing

The gate is `scripts/gate.sh`. It needs only the stable toolchain, 1.99 or newer, and no Discord token:
every test runs against the loopback mock in `crates/mock`.

## Licence

Apache-2.0, see `LICENSE`, `NOTICE` and `THIRD_PARTY_NOTICES.md`.
