# Proof of concept status

The goal of this proof of concept is to find out whether the design holds up on a real bot before
porting the rest of discord.js. `docs/design/POC.md` is the binding scope.

## The one thing to know

**First contact with real Discord: 2026-11-07.** A bot built on this code logged in to a real server. `!ping` and the slash commands answered, as reported by the person running it. That one session is all the live evidence there is: it was short, in one server, and logs were read by hand.

Everything else rests on the loopback mock in `crates/mock`, which was written from the documentation and from the upstream sources. The mock agreeing with the code proves the code is consistent with our reading of the protocol, and nothing more.

## Run the bot

1. In the [developer portal](https://discord.com/developers/applications), create an application, then
   a bot. Copy its token.
2. On the **Bot** page, turn on **Message Content Intent**. Without it, Discord closes the connection
   with code 4014, and the bot stops with that error.
3. On **OAuth2 > URL Generator**, tick the `bot` and `applications.commands` scopes, and the `Send
   Messages` and `View Channels` permissions. Open the URL to invite the bot to a test server.
4. Turn on developer mode in Discord, right-click the server and copy its id.
5. Set the two variables in your own shell, and run:

   ```sh
   # bash
   export DISCORD_TOKEN=...
   export DISCORD_GUILD_ID=...
   cargo run -p discord-rust --example poc_bot
   ```

   ```powershell
   # PowerShell
   $env:DISCORD_TOKEN = "..."
   $env:DISCORD_GUILD_ID = "..."
   cargo run -p discord-rust --example poc_bot
   ```

Do not commit the token, and do not paste it anywhere. If it leaks, reset it in the portal.
Without `DISCORD_GUILD_ID` the commands register globally, which can take a while to appear.

## What to check

| Try | Expected | What it exercises |
| --- | --- | --- |
| Start the bot | `logged in as <name>` | TLS, zlib-stream, identify, `READY`, `GUILD_CREATE` decoding |
| Type `!ping` | A reply with the latency | The Message Content intent, the message decode, REST |
| `/ping` | A reply with the latency | Command registration, interaction callback |
| `/echo text:hi` | A reply only you can see | Options, ephemeral flag |
| `/vote question:Pizza?` | An embed with Yes and No buttons | Embeds, components |
| Click Yes, then No | The counts update on the same message | Component routing, update callback |
| `/whoami` | Your username | Cache read, then REST fetch |
| Ctrl-C | The process exits on its own, within seconds | Shutdown order |
| Leave it running an hour | Still answering | Heartbeats, reconnects if Discord asks for one |

The bot logs to the terminal. Lines to look for:

- `dispatch failed to decode`: a real payload does not match our types. This is the most likely kind of
  failure, because the model was checked against discord-api-types and never against live traffic. The
  line names the event and gives the decoder's error.
- `handler failed`: a handler returned an error, often a REST rejection, with Discord's message.
- `shard failed`: a terminal close code, such as 4004 for a bad token or 4014 for a missing intent.

## What the tests cover

`scripts/gate.sh` runs format, clippy, the tests, the docs, and a build without default features. It
needs no token and no network. Against the mock, the tests cover:

- Login, ready, every command of the bot and the button update, end to end through the public API.
- Reconnects: resume from the last sequence, failed resume, each class of close code.
- Handler isolation: a panic keeps the shard alive, and a slow handler does not stop heartbeats.
- Every event fires with an empty cache.
- REST: rate limit buckets, the retry matrix, cancelling a request at each await, error flattening.
- zlib frames split across WebSocket messages.

## Not verified

- Payloads beyond that first session. Only the events that session received have decoded live.
- A button click updating a message, and Ctrl-C, unless reported since.
- A Discord-initiated reconnect or resume, on the real server.
- Real rate limits. The mock's headers follow the documentation.
- Long runs. No test lasts more than seconds, so real heartbeat timing and session expiry are untested.
- Large guilds. The mock serves small payloads, and member chunking is not in the scope.
- Load. Nothing measures throughput or memory.

## In scope

Single shard, zlib-stream, REST with a rate limiter, messages, embeds, buttons and select menus, slash
commands with autocomplete, modals, and a basic cache of guilds, channels, roles, members, users and
messages.

## Cut, for the full port

Voice, multiple shards and clustering, collectors, file uploads, CDN helpers, zstd, formatters, cache
sweepers, permission computation, rate limit sublimits, code generation from discord-api-types, and API
parity tracking. Types and routes for these are absent rather than stubbed.

## Deliberate differences

Each one departs from `ARCHITECTURE.md` or from discord.js, for the reason given.

- **The socket closes first at shutdown.** `ARCHITECTURE.md` 10.1 closes the queue first. A frame read
  after that would be lost while its sequence number is already saved, so a resume could never get it
  back. Handlers answer over REST, so they do not need the socket. See `pipeline/shutdown.rs`.
- **Op 9 with `d: true` resumes over a new socket**, rather than on the same one. It costs a reconnect
  and is simpler to get right.
- **The rate limiter also teaches the named bucket.** Upstream stores what it learns on the old bucket
  only, so the next request on the route earns a predictable 429. See `crates/rest/src/limit`.
- **The `ClientReady` timer is not tracked** as a task, so shutdown does not wait on it.

## Open before the full port

- The package name `discord-rust` is a default. Renaming is cheap until the first publish.
- `ARCHITECTURE.md` section 15 lists the open questions.
