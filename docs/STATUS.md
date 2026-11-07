# Status

discord.rs is an alpha. A real bot runs on it, and much of discord.js is not ported yet. Expect the API
to change until the first release.

## What has run against real Discord

On 2026-11-07, a bot built on this code logged in to a real server. `!ping` and the slash commands
answered, as reported by the person running it. That session was short, in one server, and its logs were
read by hand. The batches of additions below came after it, moderation first, then files, channels and
permissions, then threads, webhooks and collectors, and **have not run against real Discord**.

Everything else rests on the loopback mock in `crates/mock`, which was written from the documentation and
from the upstream sources. The mock agreeing with the code proves the code is consistent with our reading
of the protocol, and nothing more.

## What exists

| Area | What you can do |
| --- | --- |
| Gateway | Several shards in one process or split across processes, one identify budget, zlib-stream, resume and reconnect, presence on every shard |
| REST | Rate limiter with sublimits (such as channel renames), retries, typed errors, audit log reasons |
| Messages | Send, reply, edit, delete, list, bulk delete, pin, embeds, components, file uploads |
| Channels | Create, edit, delete, reorder, set and remove one permission overwrite |
| Permissions | What a member may do in a guild or a channel, computed from the cache as Discord does |
| Threads | Start from a message or not, private threads, forum posts with files, members, archives |
| Webhooks | Create, list, edit, delete, post with the webhook's token alone, from its URL, edit its messages |
| Collectors | Wait in code for the next messages, clicks or reactions that match, with a time limit |
| Reactions | Add and remove your own, remove someone else's, clear all |
| Interactions | Slash and context menu commands, autocomplete, buttons, select menus, modals |
| Moderation | Kick, ban, unban, list bans, timeout, edit members, permission check on a command's user |
| Roles | List, create, edit, delete, give, take |
| Members | Fetch every member of a large guild, search or name members, over the gateway in chunks |
| Cache | Guilds, channels, roles, members, users, messages |
| Events | `ClientReady`, messages (create, update, delete, bulk delete), reactions (add, remove), guilds, channels, threads, members, member chunks, bans, roles, webhooks, interactions. Others reach `on_raw`. |

## Not ported yet, roughly in the order planned

1. Voice.
2. Polls, stickers, scheduled events, automod, audit log reads, invites, stage channels, monetization and
   OAuth2.
3. CDN helpers, formatters, cache sweepers, zstd, and code generation from discord-api-types.

Types and routes for these are absent rather than stubbed.

Known limits of what exists:

- A cached message's reaction list does not follow reaction events.
- `set_presence` lasts until the next new session, which goes back to the presence given at login.
- Permissions are computed from the cache only. A role missing from it adds nothing, so a cold cache
  can under-count, never over-count. Interactions carry the exact value in `member.permissions`, which
  `has_permissions` uses.
- A collector sees only the events of the intents the client was built with, and keeps at most 256
  matching events that were not read yet. Older ones are kept, newer ones dropped with a warning.
- A file is read into memory before it is sent, and the size limit is left to Discord, which answers
  413 past it.

## Run a bot

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
   cargo run -p discord-rust --example basic_bot
   ```

   ```powershell
   # PowerShell
   $env:DISCORD_TOKEN = "..."
   $env:DISCORD_GUILD_ID = "..."
   cargo run -p discord-rust --example basic_bot
   ```

Do not commit the token, and do not paste it anywhere. If it leaks, reset it in the portal.
Without `DISCORD_GUILD_ID` the commands register globally, which can take a while to appear.

### `basic_bot`

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

### `moderation_bot`

Use a test server and a throwaway account: these commands really ban. Give the bot the Ban Members,
Moderate Members, Manage Messages, Manage Roles and Manage Channels permissions, and put its role above the roles it hands
out. `DISCORD_GUILD_ID` is required.

| Try | Expected |
| --- | --- |
| Start the bot | Its status shows "Watching the server" |
| `/timeout` on the throwaway account | The account cannot speak for that long |
| `/role` on the throwaway account | The account gains the role |
| `/purge count:5` | The 5 latest messages disappear |
| `/lock` in a test channel | Nobody but admins can write there any more |
| `/ban` on the throwaway account | The account is banned, and the terminal logs it |
| Any of these from an account with no such permission | A refusal, and nothing happens |
| React to a message | The terminal logs it |

### Reading the logs

- `dispatch failed to decode`: a real payload does not match our types. This is the most likely kind of
  failure, because the model was checked against discord-api-types and not against all live traffic. The
  line names the event and gives the decoder's error.
- `handler failed`: a handler returned an error, often a REST rejection, with Discord's message.
- `shard failed`: a terminal close code, such as 4004 for a bad token or 4014 for a missing intent.

## What the tests cover

`scripts/gate.sh` runs format, clippy, the tests, the docs, and a build without default features. It
needs no token and no network. Against the mock, the tests cover:

- Login, ready, every command of `basic_bot` and the button update, end to end through the public API.
- Moderation, roles, message management, reactions and presence, including what is refused before sending.
- File uploads as `multipart/form-data`, on messages, edits and interaction replies, including file names
  that try to break the part headers and content that looks like a boundary.
- Threads, forum posts, thread members and archives, and the thread events in the cache.
- Webhooks, including posting with no bot token, a wrong token, and which token each edit uses.
- Collectors: filters, time limits, streams, clicks on the right message, a bounded backlog.
- Channel creation, edits, overwrites and positions, and the permission algorithm case by case: owner,
  administrator, the order of overwrites, timeouts, threads.
- Reconnects: resume from the last sequence, failed resume, each class of close code.
- Handler isolation: a panic keeps the shard alive, and a slow handler does not stop heartbeats.
- Every event fires with an empty cache.
- REST: rate limit buckets, the retry matrix, cancelling a request at each await, error flattening.
- zlib frames split across WebSocket messages.

## Not verified

- Payloads beyond that first session. Only the events that session received have decoded live.
- Everything in `moderation_bot`, including the new events, against real Discord.
- File uploads against real Discord, which may parse `multipart/form-data` more strictly than the mock.
- A button click updating a message, and Ctrl-C, unless reported since.
- A Discord-initiated reconnect or resume, on the real server.
- Real rate limits. The mock's headers follow the documentation.
- Long runs. No test lasts more than seconds, so real heartbeat timing and session expiry are untested.
- Large guilds and several shards against real Discord. The mock splits guilds and chunks members as documented,
  but serves a handful of them.
- Load. Nothing measures throughput or memory.

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

## Design documents

`ARCHITECTURE.md` and `docs/design/` are the working documents of the build. They keep its vocabulary, and
`docs/design/POC.md` records the first plan. This file says what is true today.

## Open questions

- The package name `discord-rust` is a default. Renaming is cheap until the first publish.
- `ARCHITECTURE.md` section 15 lists the others.
