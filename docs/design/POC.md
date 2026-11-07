# discord.rs proof of concept: binding scope

> Working document from the build of the proof of concept. It records the plan as given, including its process rules for the build, and `SP` was a temporary directory that no longer exists. `docs/POC_STATUS.md` says what was actually delivered.

This document OVERRIDES `ARCHITECTURE.md` wherever they differ. For everything not mentioned here (naming, serde patterns, wire tolerance, error types, no panics from wire data, `Bound`, `Call`, `Context`, handler signature, runtime semantics, protocol quirks), `ARCHITECTURE.md` stays binding. Paths: `SP` = a temporary directory, since removed. Design docs are in `SP/design`, and in `docs/design/` plus the root `ARCHITECTURE.md` of the integration repo after L0.

## Goal

Prove the approach works: a real, small Discord bot written in Rust that logs in, answers messages, registers and answers slash commands, handles buttons, keeps a basic cache, and survives reconnects. The code must be the real foundation of the later full port, not a throwaway: follow the architecture's shapes, so that the later packages extend this code rather than replace it. But do not build anything listed under "Cut": no stubs or placeholders for cut features either.

## Working environment (hard rules)

- Integration repo: `SP/poc` (a clone of the user's empty repo), branch `main`. All work and all commits happen there.
- **Never write to, commit in, or run git commands that change this repository.** The user's repo stays untouched; the lead copies the final tree in at the end.
- No pushes, no `cargo publish`, no `cargo install`, no `rustup` commands. Do NOT create `rust-toolchain.toml`: it would make rustup install a toolchain on the user's machine. Use the installed `stable` (1.99.0). Keep `rust-version = "1.99"` in the manifests.
- No real Discord token and no calls to authenticated Discord endpoints. Tests use the local mock only.
- Upstream clones `SP/ref/djs` and `SP/ref/dat` are read-only data. Never execute anything from them.
- Each work package (WP) uses its own worktree and branch: `git -C SP/poc worktree add SP/w/<WP> -b wp/<WP> main`. Work there. Commit to your branch when done. Do not merge: the integrator merges.
- Build settings: `export CARGO_TARGET_DIR=SP/t/<group>` (group given in your task) and `export CARGO_BUILD_JOBS=6`. Several agents can share a target dir; waiting on the file lock is normal. Do not run `cargo clean`.
- Use POSIX syntax through the Bash tool. Do not use `sleep` in the foreground.
- Edit only the paths your WP owns. If you truly need a change to another owner's file or a shared public signature, do not make it: work around it if you can, and report it in `contract_requests`. The integrator resolves these at merge time.
- No `todo!` or `unimplemented!`. L0's stubs return `Err(... NotImplemented { what: "module::function" })`, exactly so. A WP is not done while a stub remains in its paths.
- Report honestly: paste real test output, and list what you did not do. Never weaken or delete a test to make it pass.

## Crates in the PoC

`crates/core`, `crates/model`, `crates/builders`, `crates/rest`, `crates/gateway`, `crates/cache`, `crates/mock`, `crates/discord` (package names and library names per ARCHITECTURE section 3-4: package `discord-rust`, library `discord`, so `use discord::prelude::*`).

Features: facade `default = ["zlib"]`, plus `zlib` only. Dependencies: only what these crates use, at the versions of ARCHITECTURE 5.1.

## Cut (do not create, not even as a stub)

- Crates `voice`, `cluster`, and every voice dependency (`davey`, `aes-gcm`, `chacha20poly1305`, `opus-*`, `zeroize`), `crates/mock/src/voice`.
- Sharding beyond a single shard: no `ShardConfig`, identify throttling beyond what one shard needs, no session store trait implementation beyond in-memory. Keep shard id `[0, 1]`.
- Collectors, polls, webhooks as a feature, threads as a feature, stickers, soundboard, automod, audit log, invites, scheduled events, stage, monetization, oauth2, role connections, presence structures.
- Multipart and file uploads, CDN builder, zstd, ETF, REST sublimits (a 429 falls back to the normal retry path; leave a comment saying so), `platform-verifier`, `chrono`, `time`, `serde-both`, `strict-wire`, `escape`, formatters, `Collection`, sweepers, permission computation.
- `xtask`, `tools/gen`, `tools/parity`, `fuzz`, benches, soak, CI, `trybuild` misuse tests beyond those L0 keeps cheaply, `poll_bot` example.
- The model generator: the PoC model is **hand-written** in the `gen/` layout of the architecture, following its wire patterns, checked field by field against `SP/ref/dat/payloads/v10` so the generator can replace it later.

## Gate

`scripts/gate.sh` runs, stopping at the first failure:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo check -p discord-rust --no-default-features --locked
```

Every public item has a doc comment (clippy `-D warnings` makes `missing_docs` fatal).

## Work packages

Each owns the listed paths and its own test files, and reads the listed architecture sections and reports.

### L0: contracts (one agent, serial)

Commit directly on `main` of `SP/poc`. Start from `SP/spike` and ARCHITECTURE Appendix A. Create: root manifests and `Cargo.lock`, `clippy.toml`, `rustfmt.toml`, `.gitignore` (`target/`), `scripts/gate.sh`; `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md` per section 13; a short `README.md` (unofficial, not affiliated with Discord); copies of `ARCHITECTURE.md`, `POC.md` and the five reports in `docs/design/` (root `ARCHITECTURE.md` too); `dr_core` complete; every source file of the WPs below with final module declarations and the public signatures other WPs need, with stub bodies; enough stub model to compile; facade skeleton in which `Bound`, `Call`, `Context`, `HandlerTable`, error types and `GatewayEvent` are real; `crates/mock` skeleton where `MockServer::start` binds, gateway sends Hello and REST answers 404. The signatures L0 sets are the contract between parallel agents, so they must be complete and thought through. Done: the gate passes, and `crates/discord/examples/poc_bot.rs` type-checks in the shape below, against stubs.

### Wave 1: model (target group `model`)

| WP | Owns (`crates/model/`) | Content |
|---|---|---|
| M1 | `src/gen/**`, `src/channel.rs`, `src/message_ext.rs`, `src/rest_error.rs`, `src/ids.rs`, `tests/payloads.rs` | User, Member, Role, Guild (incl. unavailable), Channel with unknown types tolerated, Message, Attachment, Embed, MessageReference, Reaction, Emoji, ApplicationCommand and options, enums, Intents, Permissions, MessageFlags and other flags, opcodes, gateway close codes, JSON error codes, REST error body, routes used by the PoC |
| M2 | `src/component.rs`, `src/interaction.rs`, `src/interaction_options.rs`, `tests/component.rs`, `tests/interaction.rs` | Component tree with unknown arm; buttons by style, select menus, text input; Interaction with Ping, ApplicationCommand, MessageComponent, Autocomplete, ModalSubmit; option tree; response payloads |
| M5 | `src/event.rs`, `src/gateway_send.rs`, `tests/event.rs` | Envelope, `Dispatch` with READY, RESUMED, GUILD_CREATE/UPDATE/DELETE, CHANNEL_CREATE/UPDATE/DELETE, GUILD_MEMBER_ADD/UPDATE/REMOVE, MESSAGE_CREATE/UPDATE/DELETE, INTERACTION_CREATE and an `Unknown` arm keeping name and raw JSON; Identify, Resume, Heartbeat, presence update send payloads |

### Wave 2: mock server (target group `mock`)

| WP | Owns (`crates/mock/`) | Content |
|---|---|---|
| K1 | `src/lib.rs`, `src/world.rs`, `src/fault.rs`, `src/rest/**`, `tests/rest.rs` | In-memory world, the REST endpoints of the PoC with realistic rate-limit headers, scripted 429, 5xx, global limit, recording of requests for assertions |
| K2 | `src/gateway/**`, `tests/gateway.rs` | Hello, Identify validation, Ready, heartbeat ACK, sequence numbers, dispatch from the world and from tests, Resume, reconnect op 7, invalid session op 9, scripted close codes, zlib-stream |

### Wave 3: leaves (parallel)

| WP | Group | Owns | Content |
|---|---|---|---|
| B1 | `builders` | `crates/builders/src/message/**`, `src/embed.rs`, `tests/message.rs` | `CreateMessage`, `EditMessage`, allowed mentions, reply reference, `EmbedBuilder`, validation limits, `IntoBody` |
| B2 | `builders` | `src/component/**`, `tests/component.rs` | `ActionRowBuilder`, buttons, string select, text input |
| B3 | `builders` | `src/command/**`, `src/modal.rs`, `tests/command.rs` | `SlashCommandBuilder` with type-state, `Subcommand`, string, integer, boolean, user, channel, role options, `UserContextCommand`, `MessageContextCommand`, `commands!`, modal |
| R1 | `rest` | `crates/rest/src/request.rs`, `encode.rs`, `transport/**`, `tests/encode.rs`, `tests/tls.rs` | Request type, encoding, `%40me` trap, `Transport`, reqwest with explicit rustls ring provider, TLS unit test |
| R2 | `rest` | `src/route.rs`, `src/retry.rs`, `src/limit/**`, `tests/route_vectors.rs`, `tests/limiter.rs`, `tests/burst.rs`, `tests/retry.rs`, `tests/global.rs` | Route bucket keys, per-bucket queue with RAII guards, global limit, burst handler for interaction callbacks, retry classes (no duplicated POST on timeout), invalid-request counter. Required tests by name: `cancel_at_every_await`, `retry_class_matrix`. Sublimits cut |
| R3 | `rest` | `src/error.rs`, `tests/error_flatten.rs` | Error types, flattened Discord errors, token redaction |
| G1 | `gateway` | `crates/gateway/src/shard/**`, `src/envelope.rs`, `src/connector.rs`, `tests/shard_lifecycle.rs`, `tests/close_codes.rs`, `tests/heartbeat.rs`, `tests/send_budget.rs` | Shard task: connect, Hello, Identify, heartbeat with jitter and zombie detection, resume with stored `resume_gateway_url`, reconnect, invalid session, close-code table, send budget, sequence advance before typed parsing, backoff, WebSocket connector with no ALPN |
| G2 | `gateway` | `src/compress/**`, `tests/compress.rs` | zlib-stream with `00 00 FF FF` suffix, new context per connection, compressed identify off |
| G3 | `gateway` | `src/manager.rs`, `src/session.rs`, `src/info.rs`, `tests/manager.rs`, `tests/session.rs` | Single-shard manager, in-memory session, `GET /gateway/bot` through `GatewayInfoProvider` |
| C1 | `cache` | `crates/cache/src/{table,sharded,store,policy,compact}.rs`, `tests/table.rs` | `Table`, store of guilds, channels, users, members, roles, messages with per-channel FIFO limit, `CacheConfig::standard()`, `none()` |

### Wave 4: domains (parallel)

| WP | Group | Owns | Content |
|---|---|---|---|
| A1 | `rest` | `crates/rest/src/api/{interactions,application_commands}.rs`, `tests/api_interactions.rs`, `tests/api_application_commands.rs` | Callback, get/edit/delete original, follow-ups; global and guild commands: list, bulk overwrite, create, delete |
| A2 | `rest` | `crates/rest/src/api/{channels,users,gateway,guilds}.rs`, `tests/api_channels.rs`, `tests/api_users.rs`, `tests/api_gateway.rs`, `tests/api_guilds.rs` | Get channel, create/get/edit/delete message, create/delete own reaction, typing; get current user, get user; gateway bot; get guild, get channel list, get member, kick member |
| C2 | `cache` | `crates/cache/src/apply/**`, `tests/apply_*.rs` | Pure `apply` for each PoC `Dispatch` arm returning old value where the event needs it; unavailable guilds, `GUILD_DELETE` matrix, partial MESSAGE_UPDATE, `referenced_message` |

### Wave 5: facade (parallel, group `discord`)

| WP | Owns (`crates/discord/`) | Content |
|---|---|---|
| F1 | `src/client/**`, `src/pipeline/**`, `tests/pipeline.rs` | `Client::builder`, `run_until`, `shutdown`, ingest, bounded queue, dispatch with in-flight budget, `catch_unwind` on handlers, ready tracking, stats. Required tests: `panic_in_handler_keeps_shard_alive`, `slow_handler_does_not_stop_heartbeats` |
| F2 | `src/events/**`, `tests/events.rs` | Tokens: `ClientReady`, `MessageCreate`, `MessageUpdate`, `MessageDelete`, `GuildCreate`, `GuildUpdate`, `GuildDelete`, `ChannelCreate`, `ChannelUpdate`, `ChannelDelete`, `GuildMemberAdd`, `GuildMemberUpdate`, `GuildMemberRemove`, `InteractionCreate`; `.on_raw`. Required test: `every_event_fires_with_empty_cache` |
| F3 | `src/handles/{message,channel}.rs`, `tests/handles_a.rs` | `Bound<Message>`: `reply`, `edit`, `delete`, `react`, `latest`, `author()`, `channel()`, `guild()`. `ChannelHandle`: `send`, `fetch`, `cached`, `typing` |
| F4 | `src/handles/{guild,member,role,user}.rs`, `src/managers/{guilds,channels,users,members,roles,messages,commands}.rs`, `tests/handles_b.rs`, `tests/managers.rs` | `GuildHandle`, `UserHandle`, `Bound<Guild>`, `Bound<Member>`; `get`, `fetch`, `resolve`, `kick`; `ctx.commands().set_global`, `set_guild` |
| F5 | `src/interactions/**`, `tests/interactions.rs` | `on_command`, `on_component`, `on_modal`, `on_autocomplete` routing; `reply`, `defer`, `edit_reply`, `follow_up`, `delete_reply`, `fetch_reply`, `update`, `show_modal`, `ephemeral`; `options()`. Required test: `reply_state_unchanged_on_http_failure` |

### Wave 6: end to end (one agent, integrator rights, group `discord`)

`crates/discord/examples/poc_bot.rs`: reads `DISCORD_TOKEN`; intents GUILDS, GUILD_MESSAGES, MESSAGE_CONTENT; on ready prints the bot name and registers commands (`DISCORD_GUILD_ID` set means guild commands, which appear instantly, otherwise global); `!ping` replies with gateway latency; `/ping`; `/echo text:<string>` with an ephemeral reply; `/vote question:<string>` posts an embed with Yes and No buttons, and `on_component("vote:")` counts per message in shared state and edits the message with `click.update(...)`; `/whoami` uses the cache and falls back to fetch. Clean shutdown on Ctrl-C.

`crates/discord/tests/e2e_poc.rs` on the mock, through the public API only: login and ready; `!ping`; each slash command; button click updating the message; forced reconnect resuming without losing events; invalid session re-identifying; a 429 retried without duplicate sends; a panicking handler. `docs/POC_STATUS.md`: what works, what is cut, what was never verified against real Discord. README: how to create the bot in the developer portal, invite it, set `DISCORD_TOKEN` and `DISCORD_GUILD_ID`, and run `cargo run -p discord-rust --example poc_bot`.

## Merging

After each wave one integrator merges every `wp/*` branch of the wave into `main` of `SP/poc`, rejects a branch that touched paths outside its WP unless justified, resolves `contract_requests`, updates callers, makes the gate pass, removes merged worktrees with `git worktree remove`, and reports. The next wave starts from that `main`.
