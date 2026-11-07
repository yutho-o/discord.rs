# discord.rs: binding architecture

Date: 2026-11-07. Toolchain: rustc and cargo 1.99.0 (b940084d7 2026-09-28), Windows 11, 32 logical CPUs.
Upstream pins: discord.js `5bb74922a1fb36e0d20e2656fa1bfb8944758882` (2026-11-06, branch `main`, not 14.x, even though `package.json` says 14.16.3). discord-api-types `00cbe03ba50ba45a491bef6e94724b913805b5ed` (2026-11-06, 0.38.57).
Target repository: this repository, empty at `43cdb95`.

The request this document answers: "Ont va reprendre discord.js et le mettre en rust pour cree discord.rs pour cree des bots discord complet en rust".

## 0. How to use this document

1. **This document is binding.** Precedence: this document, then the reports, then your own judgement. The three proposals are not binding, and they are not copied into the repository.
2. **Reports are reference data.** L0 copies `gateway.md`, `rest.md`, `client.md`, `voice.md` and `types.md` to `docs/design/`. `[gateway 7]` means section 7 of `docs/design/gateway.md`. They hold the exhaustive tables (212 endpoints, 129 routes, 79 events, validation rules, byte layouts), which are not repeated here. Where a report says "decide" or "recommend", this document has decided. Where they conflict, this document wins.
3. **If this document is wrong or silent,** do not improvise on anything that crosses a crate boundary. Ask the integrator (section 14.1). Inside your own module, decide and write it down in a code comment.
4. **Evidence tags:**
   - **[spike]** compiled and run for this document, in `scratchpad/spike` (section 16). `scratchpad` means a temporary directory, since removed. From L0 on, what matters in it is on branch `wp-inputs` (section 14.3).
   - **[probe]** compiled or run in an earlier probe by a proposal author, on this machine.
   - **[src]** read in the upstream source at the pinned commits.
   - **[external]** from Discord's documentation or memory. Not verified here. Needs a live check.

Nothing in this project has ever run against a logged-in Discord session. Only unauthenticated `GET /gateway` and a Hello were observed [probe]. Treat every [external] item as a claim, not a fact.

## 1. Verdict

### 1.1 Scores

| Criterion (each /10) | proposal-robust | proposal-idiomatic | proposal-parity |
|---|---:|---:|---:|
| Parity with discord.js | 7.5 | 7 | 9.5 |
| Rust ergonomics | 8 | 8 | 6.5 |
| Robustness | 9.5 | 7 | 6 |
| Buildability by parallel agents | 9.5 | 6.5 | 7 |
| **Total /40** | **34.5** | **28.5** | **29** |

**Winner: proposal-robust.** It is the only one whose failure handling, wire tolerance and work split were all concrete and checked.

Why the others lost:

- **Idiomatic:** `msg.reply(&ctx, ...)` everywhere, an unbounded raw frame channel, extension-trait imports, a mostly sequential build order. Good ideas, weak on survival and on the split.
- **Parity:** `Deref` as inheritance, PascalCase consts, a `Weak` client that can panic on use, typed decode on the shard task (which stalls heartbeats), unbounded spawn, `Value`-based unions, an eager cache, and the largest hand-written surface.

Why the winner did not score 10: its parity is approximate, no machine checks it, compact cache types double some accessors, and it left three holes the spike had to fill (section 1.3).

### 1.2 Grafts

| From | Graft | Note |
|---|---|---|
| parity | Machine-checked parity manifest, `PARITY.md` migration guide | **Mechanism changed.** Its generated `let _ = Type::method;` test does not compile for any method taking `impl Trait` (E0283) [spike]. Use rustdoc JSON instead, section 11.8 |
| parity | `#[doc(alias = "jsName")]` on every mapped item | Visible in rustdoc JSON, so checkable [spike] |
| parity | Translation rule table | Section 7.2, restricted to rules that fit this design |
| parity | `msg.latest()` for "re-read the current cached version" | Replaces mutable identity |
| parity | Test kit usable by bot authors | `MockServer` plus `.endpoints(...)`. No dependency cycle |
| parity | `SHARDS` and `SHARD_COUNT` env support, `ClientValue`-style naming on the typed RPC | Cluster, later |
| idiomatic | `trybuild` misuse catalogue | 12 cases all fail as intended [spike] |
| idiomatic | `get` (cache), `fetch` (HTTP), `resolve` (cache then HTTP) | Removes `fetch` overloading and `{force}` |
| idiomatic | `Leaf` and `Parent` type-state on slash commands | Mixed arrays need the `commands![]` macro, which compiles [spike] |
| idiomatic | Non-generic REST core with a thin generic decode tail | [spike] |
| idiomatic | Direction-split serde derives behind `serde-both` | Field attributes stay legal in both modes [spike]. Savings measured at 20% to 40% by different probes: unknown until L1 |
| idiomatic | `.only_user(id)` data filter on collectors | Runs at ingest with no user code [spike] |
| idiomatic | Dedicated `std::thread` voice clock | tokio `sleep_until` on this machine: p50 15.57 ms, p99 31.95 ms for a 20 ms tick. `std::thread::sleep`: p50 20.05, p99 20.41. Sleep then spin the last 2 ms: 20.00 [probe] |
| idiomatic | Explicit TLS provider, never `install_default` | [spike] |
| idiomatic | Generator override table, fail closed | Section 7.7 |

Rejected: `ListenerId` and dynamic listeners from parity (the handler table stays frozen, `client.subscribe` covers the dynamic case). Reply type-state from idiomatic (clones and retries need a runtime state, so it stays an atomic). Channel as a 12-arm enum from parity (channels change type at runtime, `[client 16.27]`).

### 1.3 What the spike changed in the winner

1. **`EditMessage` naming settled.** `CreateMessage` and `EditMessage` are bodies. Requests are `SendMessageRequest`, `ReplyRequest`, `Fetch<T>` and `Call<T>`.
2. **Builder naming settled.** A second `Embed` or `ActionRow` next to the wire types would collide. Request bodies are verb-noun, nested data keeps the upstream `*Builder` name. So `EmbedBuilder`, `ActionRowBuilder`, `SlashCommandBuilder`.
3. **`rest` must not depend on `builders`.** The winner did not say how a builder reaches an endpoint. `IntoBody` lives in `core`, `builders` implements it, and validation errors surface at `.await` [spike].
4. **`Payload: Clone`** is required, since one event can have several handlers [spike].
5. **`state()` exists only on `ClientBuilder<()>`.** Calling it twice is a compile error [spike].
6. **Panics:** one task with `catch_unwind`, rather than a spawn plus a join per call [spike].
7. **`serde(flatten)` is forbidden on the receive path.** The winner left it open. A `tagged_union!` under `flatten` fails at runtime with `invalid type: newtype struct` [spike].
8. **Order changed:** the real model comes before the leaves, rather than swapping P0 stubs at the end of a parallel wave, which would invalidate every agent's code.
9. **`--all-features` is forbidden.** `opus2` does not build on this machine. Use the `full` feature [probe, spike].
10. **The facade has no `rest`, `gateway` or `cache` features.** `Client` needs all three, and `CacheConfig::none()` covers the rest. Lower crates cover standalone use.
11. **Library names are pinned** (`dr_core`, ...), since a crate's own integration tests use its library name and not the dependency alias. The first attempt failed with E0432 [spike].

## 2. Goals and non-goals

### Goals

- **G1 Complete bots.** All 79 gateway events, 212 REST methods, cache, sharding, interactions, components V2, collectors, builders, formatters, `Collection`, and voice send and receive with DAVE.
- **G2 Survive Discord.** No panic from wire data. A new enum value, flag bit, field or union arm never loses an event. Memory stays bounded under load. No duplicate sends from retries. A wedged handler cannot kill a shard.
- **G3 Feel like Rust and read like discord.js.** `msg.reply("x").await?`, no lifetimes in user code, no trait imports for methods, misuse fails to compile where that is possible.
- **G4 Layers.** `rest`, `gateway`, `cache`, `model`, `builders` and `voice` are each usable alone.
- **G5 Testable without a token.** Every behaviour except Discord's own reaction runs against a loopback mock.
- **G6 Buildable by parallel agents** who touch disjoint directories.
- **G7 Checked parity.** Every public discord.js item maps to a Rust path or a stated cut, in CI.

### Non-goals for v1

- Identical names or shapes to discord.js. Section 7.2 is the rule.
- RPC, voice gateway v4, API versions other than 10, ETF.
- `WorkerShardingStrategy`, `broadcastEval`, `_eval`, `fetchClientValues`, `jsonTransformer`, the global validation toggle, `closeTimeout`, `Events.Invalidated`, `ClientInvalidProvidedShards` (`[client 16.69]`).
- `@discordjs/proxy` port, live resharding, pluggable cache backends, per-key ordered dispatch, async collector filters, an HTTP interactions endpoint, user accounts.
- Adding or removing handlers after `build()`.
- C in the default build. An MSRV below 1.99, until CI proves one.

If time runs short, cut in this order: voice receive, voice, P2 managers, zstd. **Never cut:** the mock server, drift detection, backpressure, retry classes.

## 3. Names

Checked 2026-11-07 against `index.crates.io` (404 means free) and `cargo search`.

| Name | Status |
|---|---|
| `discord`, `discord-rs` (same as `discord_rs`), `discordrs` | Taken. `discordrs` 3.2.0 describes itself as "discord.rs" |
| `discord.rs` | Not a legal crate name |
| `discord-rust`, `discord-rust-{core,model,builders,rest,gateway,cache,voice,mock,cluster,macros,util}`, `discord-dot-rs`, `discordium`, `discordium-core`, `corde` | Free |

**Binding default, subject to open question 1:** package `discord-rust`, `[lib] name = "discord"`, so a bot writes `use discord::prelude::*`. Sub-crates are packages `discord-rust-*` with library names `dr_*`.

A rename must stay cheap, so:

- Directories are name-independent: `crates/core`, `crates/rest`, ...
- Internal code only ever says `dr_core`, `dr_rest`, ... Dependency keys are aliases: `dr-core = { package = "discord-rust-core", path = "crates/core" }` [spike].
- Each sub-crate pins `[lib] name = "dr_<dir>"`, or its own tests in `tests/` fail with E0432 [spike].
- A rename therefore edits only `[package] name` in each manifest and the `package =` values in the root table.

Names are not reserved until published. Work package H8 publishes a real alpha of every crate once the facade works, never a placeholder, and only after the lead has approved the name and the publication. Publishing is the one step in this plan that cannot be undone, so no agent does it unprompted.

## 4. Crate graph

```
                     dr_core              ids, open_int!, open_str!, open_flags!, tagged_union!,
                        |                 Patch, Timestamp, Secret, IntoBody, Endpoints. No tokio.
                     dr_model             generated payloads, enums, flags, routes; hand-written
                        |                 unions, events. No tokio.
    ____________________|___________________________________________
   |            |            |            |            |            |
dr_builders  dr_rest     dr_gateway    dr_cache     dr_voice     dr_mock
no tokio     tokio       tokio         NO tokio     tokio        tokio, axum
   |____________|____________|____________|____________|         (dev-dependency of
                             |                                    rest, gateway, voice, facade)
                         discord                  package discord-rust: Client, Bound, handles,
                             |                    managers, pipeline, interactions, collectors
                         dr_cluster               later: spawner and typed IPC
```

| Package (directory) | Library | Depends on | tokio |
|---|---|---|---|
| `discord-rust-core` (`crates/core`) | `dr_core` | serde, serde_json, serde_path_to_error, bitflags, compact_str, bytes, thiserror. Optional chrono, time | No |
| `discord-rust-model` (`crates/model`) | `dr_model` | core, serde, serde_json | No |
| `discord-rust-builders` (`crates/builders`) | `dr_builders` | core, model, serde, serde_json, hashlink, url, base64 (soundboard data URLs), bytes (attachments). Optional fancy-regex | No |
| `discord-rust-rest` (`crates/rest`) | `dr_rest` | core, model, reqwest, rustls, webpki-roots, bytes, futures, tokio, percent-encoding, form_urlencoded, infer, rand, parking_lot, hashbrown, foldhash, tracing, thiserror | Yes |
| `discord-rust-gateway` (`crates/gateway`) | `dr_gateway` | core, model, tokio-tungstenite, rustls, webpki-roots, futures, tokio, tokio-util, bytes, rand, parking_lot, tracing, thiserror. Optional flate2, zstd | Yes |
| `discord-rust-cache` (`crates/cache`) | `dr_cache` | core, model, parking_lot, hashbrown, foldhash, arc-swap, smallvec, compact_str | **No** |
| `discord-rust-voice` (`crates/voice`) | `dr_voice` | core, model, davey, aes-gcm, chacha20poly1305, zeroize, tokio-tungstenite, futures, tokio, bytes, rand, tracing, thiserror. Optional opus-pure, opus2 | Yes |
| `discord-rust-mock` (`crates/mock`) | `dr_mock` | core, model, axum, tokio-tungstenite, futures, tokio, serde, serde_json, flate2, zstd | Yes |
| `discord-rust` (`crates/discord`) | `discord` | all above except mock, futures, tokio, tokio-util, tracing, thiserror | Yes |
| `discord-rust-cluster` (`crates/cluster`) | `dr_cluster` | core, rest, gateway, serde | Yes, L5 |
| `xtask` (`xtask`) | binary | serde, serde_json. Not published | No |
| `tools/gen`, `tools/parity` | Node 26 scripts over `typescript` 5.9.3. Not published | | |

Rules:

1. Dependencies point down only. `rest`, `gateway`, `cache`, `voice` and `builders` never see each other. The facade wires them through `IntoBody`, `GatewayInfoProvider` and `GatewaySender` [spike].
2. `cache` has no tokio, so `apply` is a pure function under test and no lock can cross an `.await` by construction.
3. `mock` depends on `core` and `model` only. It can be a dev-dependency of anything, and the published test kit needs no facade.
4. Only `model` uses `tagged_union!`, since its private tag wrapper derives through the calling crate's `serde`.
5. Facade features, exact: `default = ["zlib"]`, `zlib`, `zstd`, `voice`, `opus-pure`, `opus-libopus`, `platform-verifier`, `escape`, `chrono`, `time`, `serde-both`, `strict-wire`, `test-util`, and `full`, which is everything except `opus-libopus` and `strict-wire`. Sub-crates use the same names and the facade forwards them: `dr_gateway` has `zlib` and `zstd`, `dr_builders` has `escape`, `dr_voice` has `opus-pure` and `opus-libopus`, `dr_model` has `serde-both` and `strict-wire`, `dr_core` has `chrono` and `time`.

## 5. Dependencies

### 5.1 Root `Cargo.toml`, exact

All versions confirmed with `cargo search` on 2026-11-07. `cargo search` shows pre-releases as latest for `rustls` (0.24.0-dev.2) and `smallvec` (2.0.0-beta.4), so the newest stable was read from the index. This table built together on Windows, and `cargo tree -i aws-lc-rs` finds nothing [spike]. One exception: `opus2` is not built, because its CMake step cannot run MSBuild on this machine [probe].

```toml
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
version = "0.1.0-alpha.0"
edition = "2024"
rust-version = "1.99"
license = "Apache-2.0"

[workspace.dependencies]
# internal: the alias is what code sees, the package is what crates.io sees
dr-core     = { package = "discord-rust-core",     path = "crates/core",     version = "0.1.0-alpha.0" }
dr-model    = { package = "discord-rust-model",    path = "crates/model",    version = "0.1.0-alpha.0" }
dr-builders = { package = "discord-rust-builders", path = "crates/builders", version = "0.1.0-alpha.0" }
dr-rest     = { package = "discord-rust-rest",     path = "crates/rest",     version = "0.1.0-alpha.0" }
dr-gateway  = { package = "discord-rust-gateway",  path = "crates/gateway",  version = "0.1.0-alpha.0" }
dr-cache    = { package = "discord-rust-cache",    path = "crates/cache",    version = "0.1.0-alpha.0" }
dr-voice    = { package = "discord-rust-voice",    path = "crates/voice",    version = "0.1.0-alpha.0" }
dr-mock     = { package = "discord-rust-mock",     path = "crates/mock",     version = "0.1.0-alpha.0" }
# runtime and network
tokio = { version = "1.54.1", default-features = false }
tokio-util = { version = "0.7.20", default-features = false }
futures = { version = "0.3.34", default-features = false, features = ["std", "async-await"] }
reqwest = { version = "0.13.5", default-features = false, features = ["rustls-no-provider", "http2"] }
rustls = { version = "0.23.46", default-features = false, features = ["ring", "std", "tls12"] }
webpki-roots = "1.0.11"
rustls-platform-verifier = "0.7.1"
tokio-tungstenite = { version = "0.30.0", features = ["rustls-tls-webpki-roots"] }
flate2 = { version = "1.1.11", default-features = false, features = ["zlib-rs"] }
zstd = "0.14.3"
# data
serde = { version = "1.0.229", features = ["derive"] }
serde_json = { version = "1.0.154", features = ["raw_value"] }
serde_path_to_error = "0.1.20"
bitflags = "2.13.2"
compact_str = { version = "0.10.0", features = ["serde"] }
bytes = "1.12.1"
thiserror = "2.0.21"
tracing = "0.1.44"
# containers
parking_lot = "0.13.0"
hashbrown = { version = "0.17.1", default-features = false, features = ["inline-more"] }
foldhash = "0.2.0"
smallvec = { version = "1.16.5", features = ["union", "serde"] }
arc-swap = "1.9.3"
hashlink = "0.12.2"
# rest helpers
percent-encoding = "2.3.2"
form_urlencoded = "1.2.2"
infer = "0.22.0"
url = "2.5.8"
base64 = "0.23.1"
rand = "0.10.3"
fancy-regex = "0.20.0"
# voice: davey is pinned exactly, it is young and breaking
davey = "=0.2.0"
aes-gcm = "0.11.1"
chacha20poly1305 = "0.11.0"
opus-pure = "0.2.2"
opus2 = "0.4.0"
zeroize = "1.9.1"
# optional conversions
time = { version = "0.3.55", default-features = false, features = ["std"] }
chrono = { version = "0.4.45", default-features = false, features = ["std"] }
# dev
axum = { version = "0.8.9", default-features = false, features = ["http1", "tokio", "json", "query"] }
proptest = "1.12.0"
insta = "1.49.0"
criterion = "0.8.2"
loom = "0.7.2"
trybuild = "1.0.122"
tracing-subscriber = "0.3.23"

[workspace.lints.rust]
unsafe_code = "deny"
unused_must_use = "deny"
missing_docs = "warn"          # raised to deny at the end of L4

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
await_holding_lock = "deny"
await_holding_refcell_ref = "deny"

[profile.dev]
debug = "line-tables-only"
```

`rust-toolchain.toml` pins `channel = "1.99.0"` with `rustfmt` and `clippy`. On a machine with only `stable`, rustup installs that named toolchain on the first command. `clippy.toml` sets `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests` to `true`, and `disallowed-methods` for `tokio::sync::mpsc::unbounded_channel` and `rustls::crypto::CryptoProvider::install_default` [spike]. The repository has no git remote yet, so `repository` is added to the table at publish time.

Per-crate tokio features: `rest` = `rt`, `time`, `sync`, `macros`, `fs`. `gateway` = `rt`, `time`, `sync`, `macros`, `net`, `fs`. `voice` = `rt`, `time`, `sync`, `macros`, `net`, `process`, `io-util`. `mock` = `rt-multi-thread`, `time`, `sync`, `macros`, `net`. Facade = `rt`, `time`, `sync`, `macros`; `rt-multi-thread` and `signal` are the bot author's, and in the facade's dev-dependencies along with `test-util`. `tokio-util` = `rt` (for `TaskTracker`, used by the spike).

Tools, not dependencies, confirmed by `cargo search`: cargo-nextest 0.9.151, cargo-hack 0.6.46, cargo-public-api 0.52.0, cargo-semver-checks 0.51.0, cargo-fuzz 0.13.2, cargo-deny 0.20.2. **None is installed on this machine.** Every acceptance command in this document uses plain `cargo`. CI may install them. FFmpeg is also absent here, so tests that need it are `#[ignore]` unless `FFMPEG` is set. Node 26.10.0 and CMake 4.4.4 are present, NASM is not.

### 5.2 Rejected

| Crate | Why not |
|---|---|
| dashmap, papaya, scc | Equal read speed in the probe, 16 KB and 54 KB per empty map, and dashmap guards invite deadlocks across `.await`. A small `Table` is enough |
| hyper directly | More code for pooling, HTTP/2 and proxies. `Transport` allows a swap later |
| fastwebsockets, tokio-websockets | No benchmark favours them on a TLS and inflate workload. `tokio-websockets` is the fallback |
| simd-json, sonic-rs | `RawValue` interplay is unproven. Reconsider after benchmarks |
| aws-lc-rs | Needs CMake and NASM on Windows |
| wiremock, mockito | Stateless. We need a gateway that reacts to a REST call |
| songbird as a dependency | Reference only (ISC). Not on `davey` 0.2, wrong layering |
| chrono, time as required | `Timestamp` is `i64` microseconds with a small parser [spike]. Conversions are optional features |
| indexmap | `shift_remove` is O(n). `Collection` needs FIFO removal, so `hashlink` |
| moka | The cache has no TTL-by-entry need that sweepers do not cover |

### 5.3 Traps, record these in code comments where they apply

- `reqwest::ClientBuilder::tls_backend_preconfigured` takes `impl Any`, so a `rustls` version mismatch fails only at `build()`, at runtime. Keep the unit test that builds a client [spike].
- HTTP needs ALPN `h2` and `http/1.1`. The WebSocket connector needs **no ALPN**: clone the config and clear it [spike, probe].
- The TLS config passes the provider explicitly: `ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_safe_default_protocol_versions()?`. Never call `install_default` [spike].
- `davey` 0.2.0 brings `aes-gcm` 0.10 and `chacha20poly1305` 0.10 as duplicates of our 0.11 [spike]. Not a correctness issue. `rand` also appears at 0.8 and 0.9 through dependencies [spike].
- `parking_lot` 0.13 removed `read_recursive`. `Table` runs no closures under a lock anyway.
- `opus2`: libopus_sys cannot run MSBuild here [probe]. It stays optional and outside `full`.
- `regex` is **not** a dependency. The `\d`, `\w`, `\s` and `.` classes differ from JS. Hand-parse routes. `fancy-regex` in `escape` uses `[0-9]` explicitly.
- `Vec::splice` panics where JS `splice` clamps. `Collection` uses a `js_splice` helper.
- Full stack: 261 normal packages with voice [spike]. A cold test build of all of it took about a minute on this machine.


## 6. Repository layout

The tag after `#` is the owning work package (section 14). `INT` is the integrator. A work package owns the listed path and anything it adds beneath it: if it owns `limit/`, it may add `limit/foo.rs`. It may never edit a file owned by another package.

```
discord.rs/
  Cargo.toml  Cargo.lock  rust-toolchain.toml  clippy.toml  rustfmt.toml      # INT
  LICENSE  NOTICE  THIRD_PARTY_NOTICES.md  README.md  CONTRIBUTING.md         # INT
  ARCHITECTURE.md                                                             # INT
  PARITY.md                                                                   # H1, rendered
  .github/workflows/ci.yml                                                    # INT
  docs/design/{gateway,rest,client,voice,types}.md                            # INT, copied, read only
  docs/guide/                                                                 # H5
  parity/{items.json,manifest.toml}                                           # H1
  tools/gen/{package.json,gen.cjs,extract.cjs,emit.cjs,overrides.toml}        # M1
  tools/parity/{package.json,extract.cjs}                                     # H1
  xtask/src/main.rs                                                           # INT; subcommands below
  xtask/src/{gate.rs}                                                         # INT
  xtask/src/{drift.rs,fixtures.rs}                                            # M7
  xtask/src/{parity.rs}                                                       # H1
  fixtures/gateway/  fixtures/rest/  fixtures/voice/                          # M7, K2; K1 adds rest/
  fuzz/                                                                       # H3, not in the workspace
  crates/
    core/                                     # L0, then frozen. Change only through INT
      Cargo.toml  LICENSE  NOTICE
      src/lib.rs id.rs macros.rs patch.rs secret.rs time.rs wire.rs endpoints.rs
      tests/wire_patterns.rs
    model/
      src/lib.rs                              # INT
      src/gen/mod.rs                          # M1
      src/gen/{enums,flags,errors,opcodes,routes,cdn_routes}.rs                 # M1
      src/gen/payloads/*.rs  src/gen/rest/*.rs  src/gen/gateway.rs  src/gen/voice.rs  # M1
      src/component.rs                        # M2  tree of tagged_union!, button-by-style
      src/channel.rs  src/message_ext.rs  src/role_tags.rs                      # M3
      src/interaction.rs  src/interaction_options.rs                            # M4
      src/event.rs  src/gateway_send.rs       # M5  envelope, Dispatch table, send payloads
      src/audit_log.rs  src/rest_error.rs  src/oauth2.rs  src/voice_binary.rs  src/patterns.rs  src/ids.rs   # M6
      tests/fixtures.rs  tests/forward_compat.rs                                # M7
    builders/
      src/lib.rs                              # INT
      src/message/{mod,create,edit,attachment,allowed_mentions,poll,reply}.rs  src/embed.rs  # B1
      src/component/{mod,action_row,button,select,text_input,label,v2}.rs       # B2
      src/command/{mod,slash,options,subcommand,context_menu}.rs  src/modal.rs  # B3
      src/body/{channel,thread,guild,member,role,emoji,sticker,webhook,invite,scheduled_event,stage,automod,soundboard,onboarding}.rs  # B4
      src/fmt.rs  src/escape.rs  src/collection.rs  src/util.rs                 # B5
      tests/{message,component,command,body,fmt,escape,collection}.rs           # as above
    rest/
      src/lib.rs  src/client.rs               # INT  Rest, RestBuilder, group accessors; frozen at L0
      src/request.rs  src/encode.rs  src/multipart.rs  src/transport/{mod,reqwest,tls}.rs   # R1
      src/route.rs  src/retry.rs  src/limit/{mod,bucket,global,burst,invalid,sweep}.rs      # R2
      src/error.rs  src/cdn.rs                # R3
      src/api/mod.rs                          # INT
      src/api/application_commands.rs         # A01  ... one per group, section 14.5
      tests/{route_vectors,limiter,sublimit,burst,retry,global,encode,multipart,error_flatten,cdn,tls}.rs   # R1-R3
      tests/api_<group>.rs                    # A01-A18
    gateway/
      src/lib.rs                              # INT
      src/error.rs  src/events.rs             # L0, frozen
      src/shard/{mod,state,task,heartbeat,send,identify,close}.rs  src/envelope.rs  src/connector.rs   # G1
      src/compress/{mod,zlib,zstd}.rs         # G2
      src/manager.rs  src/throttle.rs  src/session.rs  src/info.rs              # G3
      tests/{shard_lifecycle,close_codes,heartbeat,send_budget}.rs              # G1
      tests/compress.rs                       # G2
      tests/{manager,throttle,session}.rs     # G3
    cache/
      src/lib.rs                              # INT
      src/table.rs  src/sharded.rs  src/store.rs  src/policy.rs  src/compact.rs  src/sweep.rs  src/permissions.rs   # C1
      src/apply/mod.rs                        # INT  frozen dispatcher over `Dispatch`
      src/apply/{guild,channel,thread,member,message,reaction,voice,expression,misc}.rs   # C2-C9, section 14.5
      tests/{table,permissions,sweep}.rs      # C1
      tests/apply_<domain>.rs                 # C2-C9
    voice/
      src/lib.rs                              # INT
      src/networking/{mod,ws,udp,rtp,crypto}.rs                                 # V1
      src/dave.rs                             # V2  the only file that names `davey`
      src/audio/{mod,player,clock}.rs         # V3
      src/audio/{resource,graph,ffmpeg,ogg,webm,codec,volume,probe}.rs          # V4
      src/receive/{mod,ssrc,speaking,stream}.rs                                 # V5
      src/connection.rs  src/manager.rs       # V6
      tests/<as above, one per package>.rs
    mock/
      src/lib.rs  src/world.rs  src/fault.rs  # L0 skeleton, then K1
      src/rest/{mod,router,limits,handlers}.rs                                  # K1
      src/gateway/{mod,session,compress,script}.rs                              # K2
      src/voice/{mod,ws,udp}.rs               # K3
    discord/                                  # package discord-rust, lib discord
      src/lib.rs  src/prelude.rs  src/error.rs  src/bound.rs  src/call.rs  src/context.rs   # L0, frozen
      src/client/{mod,builder,runner,options,stats}.rs                          # L0 skeleton, then F1
      src/pipeline/{mod,queue,ingest,dispatch,budget,ready,shutdown,subscribe}.rs   # F1
      src/events/mod.rs                       # L0 trait and first tokens, then F2
      src/events/{tokens,table}.rs            # F2
      src/handles/{message,reaction,poll,channel,thread,webhook}.rs             # F3
      src/handles/{guild,member,role,user,presence,voice_state,emoji,sticker,soundboard}.rs   # F4
      src/handles/{invite,scheduled_event,stage,automod,audit_log,application,entitlement,misc}.rs   # F5
      src/managers/{guilds,channels,users,members,roles,messages,reactions,threads}.rs    # F6
      src/managers/{bans,emojis,stickers,soundboard,invites,webhooks,scheduled_events,stage_instances,automod,commands,entitlements,overwrites,voice_states,presences}.rs   # F7
      src/interactions/{mod,command,component,autocomplete,modal,options,reply,router}.rs   # F8
      src/collectors/{mod,registry,message,reaction,interaction}.rs             # F9
      src/voice.rs  src/sharding.rs  src/test_util.rs                           # F10
      examples/poll_bot.rs                    # INT, frozen shape, must compile from L0 on
      examples/*.rs                           # H5
      tests/ui/*.rs  tests/misuse.rs          # L0, then INT
      tests/e2e_*.rs                          # H3
      tests/soak.rs  benches/                 # H4
    cluster/                                  # H7
```

Module declarations (`pub mod x;`) in `lib.rs` and every `mod.rs` marked `INT` are written at L0 and do not change. Every file under `src/` listed above exists from L0 on, as a stub that compiles. **Exception:** a work package creates its own files under `tests/`. Cargo discovers them with no manifest edit, and an acceptance command that names a test fails until that test exists, which is the point. L0 writes only `core/tests/wire_patterns.rs`, `model/tests/wire.rs`, `discord/tests/{behaviour,misuse}.rs` and `discord/tests/ui/`.

## 7. Code conventions

### 7.1 Naming

| Thing | Rule | Example |
|---|---|---|
| Wire struct | Upstream name, strip `API` and `RESTAPI` | `APIMessage` to `Message` |
| Gateway dispatch data | Event name plus `Event` | `GatewayMessageDeleteDispatchData` to `MessageDeleteEvent` |
| REST JSON body and query | Under `dr_model::rest`, method and path, `Body` or `Query` | `RESTPostAPIChannelMessageJSONBody` to `rest::PostChannelMessageBody` |
| TS field `type` | `kind`, `#[serde(rename = "type")]` | `channel.kind` |
| `open_int!` const and flag bit | `SCREAMING_SNAKE_CASE` of the upstream member | `ChannelType::GUILD_TEXT`, `Intents::GUILD_MESSAGES` |
| `open_str!` variant | Upstream member spelling | `GuildFeature::Community` |
| Request body builder | Verb-noun | `CreateMessage`, `EditMessage`, `CreateChannel`, `EditMember` |
| Nested data builder | Upstream `*Builder` name | `EmbedBuilder`, `ActionRowBuilder`, `SlashCommandBuilder` |
| Awaitable request | `*Request`, or `Call<T>` when it has only common options | `SendMessageRequest`, `ReplyRequest`, `Call<()>` |
| Fetch | `Fetch<T>`, a `Call<T>` plus `.cache(bool)` | `guild.members().fetch(id)` |
| Id-only actor, scoped manager | `*Handle`, `*Manager` | `ChannelHandle`, `MembersManager` |
| Compact cached form | `Cached*`, only for 4 types | `CachedMember`, `CachedUser`, `CachedPresence`, `CachedVoiceState` |
| Event token | Upstream `Events.X` name if any, otherwise PascalCase of `t` | `MessageCreate`, `ChannelInfo` |
| Error enum | `*Error` | `RestError` |
| Setter on a builder | Bare noun, no `set_` or `with_` | `.content("x")` |
| Clear a field | `clear_*`, writes `Patch::Null` or an empty list | `.clear_components()` |
| Flag-like option, meaningful only when on | No argument | `.ephemeral()`, `.no_mentions()` |
| Option where both values mean something | `bool` argument | `.required(true)` |
| Duration | `Duration`, never a bare integer | `.timeout(Duration::from_secs(60))` |
| Getter | No `get_` | `msg.channel()` |
| Predicate | `is_*`, `has_*` | `is_text_based()` |

### 7.2 Translation rules from discord.js

A discord.js user who learns this table can guess nearly every name. `parity/manifest.toml` enforces it.

| discord.js | discord.rs | Example |
|---|---|---|
| Method | snake_case of the JS name | `createMessageComponentCollector` to `collect_components`, an explicit manifest exception. `setNSFW` to `set_nsfw` |
| Data property | Public field through `Deref` on `Bound<T>` | `message.content` |
| Getter computed from caches | Method | `message.channel()`, `member.permissions()` |
| Hot counter | Method | `guild.member_count()` (`[client 16.26]`) |
| `x.type` | `x.kind`, with `#[doc(alias = "type")]` | `channel.kind` |
| Manager property | Method returning a zero-storage manager | `guild.members()` |
| `manager.cache.get(id)` | `manager.get(id)`, sync | `guild.members().get(id)` |
| `manager.fetch(id, {force: true})` | `manager.fetch(id)`, always HTTP | |
| `manager.fetch(id)` | `manager.resolve(id)`, cache then HTTP. **Different name on purpose** | |
| `manager.fetch()` with no id | `manager.list()` or `fetch_all()` | |
| Options object | Chained setters on an awaitable request | `.limit(50).await` |
| `reply('x')` or `reply({...})` | `impl Into<CreateMessage>` | `.reply("x")` |
| Overload | Split by name | `first()` and `first_n(n)` |
| `options.getString(n)` | `options().string(n)` returns `Option<&str>` | |
| `options.getString(n, true)` | `options().require_string(n)` returns `Result` | |
| `X \| null` | `Option<T>` | |
| Absent versus null matters | `Patch<T>` | `referenced_message` |
| `throw`, rejected promise | `Result<T, Error>` | |
| `time: 15000` | `Duration` | |
| `*Resolvable` | `impl Into<Id<_>>`, or a dedicated enum | |
| `channel.isTextBased()` | `channel.is_text_based()` | |
| Class inheritance | Flat struct and capability checks | `Channel` |
| Mixin | Inherent methods, emitted by a local macro where shared | `Bound<CommandInteraction>` and `Bound<ComponentInteraction>` |
| `client.on(Events.X, f)` | `.on(X, f)` on the builder | |
| `client.once(Events.X, f)` | `client.await_event(X)` | |

Every mapped item carries `#[doc(alias = "<jsName>")]`, so rustdoc search finds `createMessageComponentCollector`.

### 7.3 Errors and panics

- Each crate has its own `#[non_exhaustive]` `thiserror` enum. The facade's `Error` wraps them. Signatures are in section 8.
- **No `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` in library code.** The workspace lints enforce the first three, and review enforces the rest. In tests they are fine.
- Stubs return `Err(...::NotImplemented { what })`, never `todo!`, so integration tests fail cleanly.
- `#![deny(clippy::indexing_slicing)]` on parsers of untrusted bytes: `voice/src/networking/*`, `voice/src/receive/*`, `voice/src/audio/{ogg,webm}.rs`, `gateway/src/compress/*`.
- Error messages are lowercase with no trailing period, and they chain through `#[source]`.
- Tokens are `Secret`. Webhook and interaction tokens in routes are masked before they reach an error or a log (`[rest 14.41]`). `Debug` on anything holding a token, session id or voice key must redact (`[client 16.11]`, `[voice A9]`).

### 7.4 Serde patterns

Direction per TypeScript shape. `[types 3.3]` has the census.

| TypeScript | Direction | Rust | Attributes |
|---|---|---|---|
| `x: T`, primitive or collection | receive | `T` | `#[serde(default)]` |
| `x: T`, `Id` or a required nested struct | receive | `T` | none, so a missing one is a decode failure |
| `x?: T`, `x: T \| null`, or both | receive | `Option<T>` | `#[serde(default)]` |
| `x?: T \| null` where absent differs from null | receive | `Patch<T>` | `#[serde(default)]` |
| `x?: T \| null` | send | `Patch<T>` | `#[serde(default, skip_serializing_if = "Patch::is_absent")]` |
| `x?: T` | send | `Option<T>` | `#[serde(skip_serializing_if = "Option::is_none")]` |
| `x: T \| null`, always written | send | `Option<T>` | none, so `null` is emitted. Heartbeat `d` works this way |
| `null` seen on a non-nullable list | receive | `Vec<T>` | `#[serde(default, deserialize_with = "null_default")]`, only where the override table asks |
| One bad element must not lose a list | receive | `Lenient<T>` | only where the override table asks |
| Number arriving as `1.0` | receive | `i64` | `#[serde(deserialize_with = "lenient_i64")]`, only where the override table asks. Plain `i64` rejects `1.0` [spike] |

Binding rules [spike unless noted]:

1. **Never `#[serde(flatten)]` on the receive path.** A `tagged_union!` beneath it fails at runtime. Generator output contains none. The tagged union's private `Serialize` wrapper uses `flatten` on the send path only, where it works.
2. **Never `#[serde(tag = "...")]`.** It cannot read integer tags.
3. **Never `deny_unknown_fields`,** except through the `strict-wire` feature: `#[cfg_attr(feature = "strict-wire", serde(deny_unknown_fields))]`. CI runs fixtures with it. A failure means "regenerate", never a user-visible error.
4. **Tags and opcodes peek as `u32`.** A narrow `kind: u8` or `op: u8` rejects the very payload it is meant to tolerate (`"type": 777`, `op: 999`).
5. **Open integers are `u16` unless a known value needs more.** `JsonErrorCode` is `u32`, since 340024 exists. A value beyond the repr is a decode failure, a deliberate cost.
6. **Strings that may contain escapes use `Cow<str>` or owned forms,** never a borrowed `&str`, or an escaped value fails to parse.
7. **Direction split.** Receive-only structs `derive(Deserialize)` plus `#[cfg_attr(feature = "serde-both", derive(Serialize))]`. Send-only structs mirror this. `skip_serializing_if` attributes stay legal on a `Deserialize`-only struct. Anything reachable from a `tagged_union!` arm always derives both, since the macro emits both. `full` turns on `serde-both`, so CI round-trips fixtures.

```rust
open_int! {
    /// Channel kinds. Unknown values are preserved.
    ChannelType: u16 {
        GUILD_TEXT = 0, DM = 1, GUILD_VOICE = 2, /* ... */ GUILD_MEDIA = 16;
        /// Upstream alias. Duplicates go here, or `name()` would have an unreachable arm.
        alias GUILD_NEWS = GUILD_ANNOUNCEMENT;
    }
}
// Consts work as patterns, including aliases. Debug prints `ChannelType::GUILD_TEXT` or `ChannelType(99)`.
match channel.kind {
    ChannelType::GUILD_TEXT | ChannelType::GUILD_NEWS => {}
    other => tracing::debug!(?other, "ignored"),
}

open_str! { GuildFeature { Community = "COMMUNITY", News = "NEWS" } }
// `Unknown` holds a private `Unrecognized`, so it cannot hold a known string. Reads go through CompactString,
// so an escaped "NEWS" still becomes `GuildFeature::News`.

tagged_union! {
    #[tag = "type"]
    /// Message component.
    pub enum Component {
        1 => ActionRow(ActionRow),
        2 => Button(Button),
        10 => TextDisplay(TextDisplay),
        _ => Unknown,
    }
}
// Adds `Unknown { kind: u32, raw: Box<RawValue> }`, which re-serializes byte-exact, plus `kind()`.
// A tag may appear anywhere in the object. A missing tag or a bad arm is an error: the enclosing decode
// reports it, with a path that stops at the arm, e.g. `components[0]`.

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "serde-both", derive(Serialize))]
#[cfg_attr(feature = "strict-wire", serde(deny_unknown_fields))]
pub struct Message {
    pub id: MessageId,                    // required: missing means DecodeFailed
    pub channel_id: ChannelId,
    pub author: User,
    #[serde(default)] pub guild_id: Option<GuildId>,
    #[serde(default)] pub content: String,
    #[serde(default)] pub flags: Option<MessageFlags>,
    #[serde(default, deserialize_with = "null_default")] pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Patch::is_absent")] pub referenced_message: Patch<Box<Message>>,
}
```

Special unions that the macro does not cover, hand-written per `[types 3.1]`: a boolean tag as a plain field, a nested tag on `data.type`, buttons keyed on `style`, audit log changes as `{ key, old_value, new_value }` with raw values and accessors. `Interaction` is one wire struct with typed views in the facade. `Channel` is one flat struct with `kind: ChannelType` and `Option` fields, plus `is_text_based()`, `is_thread()`, `is_voice_based()`.

Tolerance levels, binding for every decode path:

| Level | What goes wrong | Handling |
|---|---|---|
| 0 Frame | Bad JSON, bad inflate | Text frame dropped and counted. Inflate failure means reconnect |
| 1 Envelope | Unknown `op`, unknown `t` | `op: u32`, `t: Cow<str>`, `d: &RawValue`. Sequence advanced here, first |
| 2 Event body | Field type changed, required field gone | `DecodeFailed { t, raw, path, error }` to `.on_raw` and `subscribe`. Shard continues. Sampled `warn`, counter per event |
| 3 Union arm | New component or channel type | `Unknown { kind, raw }`, round-trips |
| 4 Enum value | New value | Open integer or `Unknown` |
| 5 Flag bit | New bit | `from_bits_retain`, never truncated |
| 6 New field | New key | Ignored by derive |
| 7 List element | One bad item | `Lenient<T>` on chosen fields |

`dr_core::decode<T>(&str)` is the one decode entry. The hot path has no tracking cost, and only on failure does it decode again under `serde_path_to_error` to find the path [spike]. A path through a union stops at the arm, which is good enough.

### 7.5 Snowflake

`Id<M>` is `#[repr(transparent)]` over plain `u64` with a `PhantomData<fn() -> M>`. Facts [spike]:

- Size 8. `Send + Sync` and `Copy` for every marker, including uninhabited ones, because the impls are by hand. **Do not `derive` on it**, that would add an `M: Trait` bound.
- Reads a string or an integer, including as a JSON map key, and through `from_value`. Writes a string. `"0"` is valid. `-1` and `"abc"` are errors.
- Not `NonZeroU64`: `Option<Id>` is 16 bytes, but a `"0"` from Discord cannot fail a payload.
- `.cast::<N>()` covers id-equals-id cases such as `@everyone` role and guild. `timestamp_ms()`, `Display`, `FromStr`, `Ord` numeric (`[client 16.38]`).
- Comparing `UserId` and `GuildId` does not compile [spike].
- `IdOrIndex`, `SnowflakeOrTimestamp`, `Nonce`, `SoundId` are separate hand-written types in `model/src/ids.rs` (`[types 3.6]`).

### 7.6 Bitflags

`open_flags!` wraps `bitflags` over `u64` with `const _ = !0`, so unknown bits are first-class [spike].

```rust
open_flags! { MessageFlags: number { EPHEMERAL = 1 << 6, IS_COMPONENTS_V2 = 1 << 15 } }
open_flags! { Permissions: string { ADMINISTRATOR = 1 << 3, KICK_MEMBERS = 1 << 1, /* ... */ } }
```

- `number` writes an integer, `string` writes a decimal string. Reads always accept both.
- Reads use `from_bits_retain`. `(1 << 40) | (1 << 6)` round-trips, `Debug` prints `MessageFlags(EPHEMERAL | 0x10000000000)`, and `|` and `-` keep unknown bits.
- `u64` everywhere fixes upstream's 32-bit truncation (`[client 16.37]`).
- `default_member_permissions`: `null` means default, `0` means admins only. It is `Patch<Permissions>` on send, and must not collapse.

### 7.7 Generated code

- Generated files start with `// @generated by tools/gen. Do not edit. Fix tools/gen/overrides.toml.`, and are never edited by hand.
- The generator is one-shot, output committed. `xtask drift` re-runs it against the newest upstream and reports a diff. It reads TypeScript through the checker as data. It never executes anything from the clone.
- **Fail closed:** any `number`, union or intersection without a mapping stops it (`[types 8.4]`). Number default `i64`, `f64` for `retry_after` and friends. `MillisSinceEpoch` and `Seconds` newtypes for mixed-unit fields (`[types 3.7]`).
- Scope: payloads v10, gateway v10, rest v10, voice v8. Out: rpc, voice v4, deno mirror, v6 to v9. Unstable members are included and marked in docs.
- Generated: 115 in-scope enums, 19 bit sets plus permissions, 243 `JsonErrorCode` consts, plain structs, `Routes`, `CDNRoutes`, `RouteBases`, opcodes, close codes, doc comments.
- Hand-written: the list in `[types 8.3]`, and it is exactly what M2 to M6 own.
- Required-field policy: `Id` and required nested structs stay required. Primitive and collection fields get `#[serde(default)]`, so a missing `bool` costs a default rather than the whole event.
- Compile cost is **unknown**. Probes disagree (14 s for 1300 structs, 7 s for 885, 46 s for 1000) on synthetic input. M1 reports the real `cargo check -p discord-rust-model` time. If over 30 s, M1 and INT split direction or modules before L2 opens.

### 7.8 Time and numbers

- `Timestamp(pub i64)` is microseconds since the Unix epoch. The parser accepts `YYYY-MM-DDTHH:MM:SS[.f{1,9}](Z|+HH:MM|-HH:MM)`, the writer is stable: `2021-01-01T00:00:00.123456+00:00` [spike]. `chrono` and `time` features add conversions.
- `Instant` for every timer and window. `SystemTime` only for the snowflake age check (`[rest 14.49]`).
- `Typing.timestamp` is seconds, not ISO (`[client 16.43]`). `Message.created_at()` derives from the id, not `timestamp` (`[client 16.31]`).

### 7.9 Concurrency

- **Every queue is bounded.** `unbounded_channel` is a `clippy::disallowed_methods` error [spike].
- No lock across `.await`: `clippy::await_holding_lock` is deny. `parking_lot` in library code. `parking_lot`'s `deadlock_detection` in test builds of `cache`.
- Every wait must be cancel-safe. Dropping any future at any `.await` releases everything through RAII (`[rest 14.45]`, "the largest single porting risk").
- Futures are lazy: order between un-awaited requests is not guaranteed (`[rest 14.46]`), and `#[must_use]` catches a forgotten `.await` [spike].
- Background tasks hold a `Weak` to their owner, or are tracked and drained, and stop on shutdown (`[client 16.42]`, `[rest 14.50]`).
- `select!` loops own their state: one owner task per shard and per voice connection, with an explicit state enum.
- Dyn seams use hand-written `BoxFuture`. `async fn` in a trait is not dyn compatible (E0038). `AsyncFn` with a `Send` bound is not expressible on 1.99 (RTN is E0658, issue 109417) [probe].

### 7.10 Handlers and closures: what works on 1.99

All [spike or probe]:

- `.on(MessageCreate, |ctx, msg| async move { ...; Ok(()) })` infers both closure parameters and needs no turbofish. A type-parameter style `.on::<E>(f)` would need `::<E, _, _>`, so it is not used.
- The output is fixed as `Result<(), BoxError>`, so `Ok(())` and `?` infer. A closure with no `Ok(())` gets E0271.
- A capturing `async move |c, m| { ... }` does **not** implement `Fn` ("async closure does not implement `Fn` because it captures state from its environment"). Capturing handlers are `move |c, m| async move { ... }`. Docs must say this.
- A plain `async fn(Context<S>, Bound<Message>) -> HandlerResult` works as a handler.
- `#[diagnostic::on_unimplemented]` fires only when the bound sits on the method's `where` clause, not on the impl block [probe]. `#[must_use]` on a generic type-state type fires falsely, so type-state types do not carry it [probe].
- Collector filters must be reached from a typed start, such as `msg.collect_components()`. A free `CollectorOptions::new().filter(|i| ..)` hits E0282 [probe].

### 7.11 Docs

- Every public item has a doc comment. The first line says what it is, in plain words. `missing_docs` is `warn` until the end of L4, then `deny`.
- `#[doc(alias = "...")]` per section 7.2.
- Examples are ` ```no_run ` and compile under `cargo test --doc`. They use `discord::prelude::*`.
- Ported modules start with a provenance comment, section 13.
- Doc comments on behaviour that deviates from upstream say so and link to the deviation list (section 10.8).

### 7.12 Tests

- Unit tests live in `#[cfg(test)]` in the module. Integration tests are `crates/<c>/tests/<name>.rs`, owned by the package that owns the code.
- No test touches the network beyond loopback. A live test is `#[ignore]` and reads `DISCORD_TEST_TOKEN`.
- Time-dependent tests use `#[tokio::test(start_paused = true)]`. No real sleep above 100 ms in the default suite, except the voice clock test.
- Vectors from upstream tests carry a comment naming the file (section 13).
- Snapshots use `insta`, committed under `tests/snapshots/`. Property tests use `proptest`.

### 7.13 Logging

`tracing` with targets `discord::gateway`, `discord::rest`, `discord::cache`, `discord::voice`, `discord::pipeline`. Spans carry shard id, bucket key and masked route. Never a token, body or session id. Repeating warnings are sampled, and each has a counter in `client.stats()`.


## 8. Shared core types

These signatures are the contract between work packages. L0 writes them, and only the integrator changes them. Bodies not shown are in the spike (`scratchpad/spike/crates/*`), which L0 starts from. Appendix A has the full `dr_core` source in case the scratchpad is gone.

### 8.1 `dr_core`

```rust
pub struct Id<M>(/* u64 */);                    // id.rs, Appendix A
pub mod marker { pub enum UserMarker {} /* one per Discord resource, generated list frozen at L0 */ }
pub enum Patch<T> { Absent, Null, Value(T) }    // default Absent
pub struct Secret(/* Box<str> */);              // Debug prints Secret(***)
pub struct Timestamp(pub i64);                  // microseconds
pub struct Lenient<T> { pub items: Vec<T>, pub dropped: u32 }
pub fn decode<T: DeserializeOwned>(text: &str) -> Result<T, DecodeError>;
pub struct DecodeError { pub path: String, pub source: serde_json::Error }
pub fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>;
pub fn lenient_i64<'de, D>(d: D) -> Result<i64, D::Error>;

// open_int!, open_str!, open_flags!, tagged_union!: Appendix A, verbatim from the spike.

// How anything becomes a request body. `rest` knows this and not `builders`.
pub trait IntoBody { fn into_body(self) -> Result<RequestBody, BodyError>; }
pub struct RequestBody { pub json: Option<Bytes>, pub files: Vec<RawFile> }
pub struct RawFile { pub field: String, pub filename: String, pub content_type: Option<String>, pub data: Bytes }
pub struct ValidationError { pub path: String, pub message: String }
pub enum BodyError { Validation(ValidationError), Encode(serde_json::Error) }

// Where to connect. `MockServer::endpoints()` returns one. No dependency on `mock`.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub api: String,                 // default "https://discord.com/api", version appended by rest
    pub cdn: String,                 // default "https://cdn.discordapp.com"
    pub media: String,               // default "https://media.discordapp.net"
    pub gateway_override: Option<String>,
    pub voice_insecure: bool,        // true only for the mock: ws:// rather than wss://
}
```

### 8.2 `dr_model`

```rust
// event.rs
#[derive(Deserialize)]
pub struct Envelope<'a> {
    pub op: u32,                                          // never u8
    #[serde(default)] pub s: Option<u64>,
    #[serde(default, borrow)] pub t: Option<Cow<'a, str>>,
    #[serde(default, borrow)] pub d: Option<&'a RawValue>,   // `"d": null` reads as None [spike]
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum DispatchKind { MessageCreate, /* the other 78 */ Unknown }
impl DispatchKind {
    pub const COUNT: usize;
    pub fn from_t(t: &str) -> Self;
    pub const fn index(self) -> usize;
    pub const fn is_sheddable(self) -> bool;   // TYPING_START, PRESENCE_UPDATE
}
#[non_exhaustive]
pub enum Dispatch { MessageCreate(Box<Message>), /* ... */ Unknown { t: String, raw: Box<RawValue> } }
impl Dispatch { pub fn decode(kind: DispatchKind, t: &str, d: &str) -> Result<Self, DecodeError>; }

// Frozen by name at L0 as P0 stubs in `gen/`, replaced by M1:
// Message, User, CurrentUser, Member, Guild, Channel, Role, Emoji, Interaction, Component, Embed,
// VoiceState, Presence, ReadyEvent, ApplicationCommand. Ids: UserId, GuildId, ChannelId, MessageId, RoleId, ...
// Enums and flags: ChannelType, GuildFeature, Intents, Permissions, MessageFlags, JsonErrorCode.
```

### 8.3 `dr_rest`

```rust
#[derive(Clone)] pub struct Rest(/* Arc */);
impl Rest {
    pub fn builder() -> RestBuilder;
    pub fn execute(&self, parts: RequestParts) -> BoxFuture<'static, Result<RawResponse, RestError>>; // the one non-generic path
    pub fn raw<T: DeserializeOwned>(&self, method: Method, path: impl Into<String>) -> Request<T>;
    pub fn application_commands(&self) -> ApplicationCommandsApi<'_>;  // one per group, 18 in all, frozen at L0
    pub fn cdn(&self) -> Cdn;
}
pub struct RestBuilder;   // .token(Secret) .endpoints(Endpoints) .transport(Arc<dyn Transport>) .proxy(Url)
                          // .retries(u32) .timeout(Duration) .offset(Duration) .queue_limit(usize)
                          // .global_per_second(u32) .reject_on_rate_limit(RejectOnRateLimit)
                          // .invalid_request_breaker(u32) .user_agent_appendix(String) .build()

pub struct RequestParts { pub method: Method, pub path: String, pub body: RequestBody, pub reason: Option<String>, pub options: RequestOptions }

#[must_use = "a request does nothing until it is awaited"]
pub struct Request<T> { /* Rest, Result<RequestParts, RestError>, PhantomData<fn() -> T> */ }
impl<T> Request<T> {
    pub fn new(rest: Rest, method: Method, path: String, body: Result<RequestBody, BodyError>) -> Self;
    pub fn reason(self, r: impl Into<String>) -> Self;
    pub fn query(self, q: impl Serialize) -> Self;
    pub fn reject_on_rate_limit(self, on: bool) -> Self;
    pub fn auth(self, a: Auth) -> Self;              // Auth::Default | Auth::None | Auth::Custom(Secret)
    pub fn retry_class(self, c: RetryClass) -> Self; // endpoint files set this, see 10.2
}
impl<T: DeserializeOwned + Send + 'static> IntoFuture for Request<T> {
    type Output = Result<T, RestError>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
}
#[derive(Clone, Copy, Debug, Default)] pub struct Empty;   // void endpoint: 204 or any body is accepted

pub trait Transport: Send + Sync + 'static {
    fn send(&self, req: PreparedRequest) -> BoxFuture<'static, Result<RawResponse, TransportError>>;
}
pub struct PreparedRequest { pub method: Method, pub url: String, pub headers: Vec<(&'static str, String)>, pub body: Bytes, pub timeout: Duration }
pub struct RawResponse { pub status: u16, pub headers: Vec<(String, String)>, pub body: Bytes }
pub struct TransportError { pub kind: TransportErrorKind, pub message: String }
pub enum TransportErrorKind { NotSent, Ambiguous }

#[non_exhaustive]
pub enum RestError {
    Api { status: u16, code: Option<JsonErrorCode>, oauth_error: Option<String>, message: String, errors: Vec<FieldError>, request: RequestSummary },
    Unparsed { status: u16, body_preview: String, request: RequestSummary },     // Cloudflare HTML
    Http { status: u16, request: RequestSummary },
    RateLimited(RateLimitInfo), QueueFull, CircuitOpen, Timeout, Cancelled,
    Network(TransportError),
    Decode { status: u16, path: String, source: serde_json::Error, body_preview: String },
    Validation(ValidationError), Encode(serde_json::Error),
    MissingToken, TokenInvalidated, InvalidCdn(CdnError),
    NotImplemented { what: &'static str },
}
impl RestError {
    pub fn code(&self) -> Option<JsonErrorCode>;
    pub fn request_succeeded(&self) -> bool;  // 2xx with a bad body: do NOT repeat the send [spike]
    pub fn is_retryable(&self) -> bool;
    pub fn is_rate_limit(&self) -> bool;
}
pub struct RequestSummary { pub method: Method, pub route: String /* masked */, pub status: Option<u16>, pub attempts: u32 }
```

### 8.4 `dr_gateway`

```rust
pub trait IdentifyThrottler: Send + Sync + 'static {
    /// Resolves when `shard` may identify. Dropping the future gives up the position.
    fn wait(&self, shard: u32) -> BoxFuture<'_, Result<(), ThrottleError>>;
}
pub trait SessionStore: Send + Sync + 'static {
    fn load(&self, shard: u32) -> BoxFuture<'_, Option<SessionInfo>>;
    fn save(&self, shard: u32, info: Option<SessionInfo>) -> BoxFuture<'_, ()>;
}
/// `gateway` does not depend on `rest`. The facade implements this over `Rest`.
pub trait GatewayInfoProvider: Send + Sync + 'static {
    fn gateway_info(&self) -> BoxFuture<'_, Result<GatewayInfo, BoxError>>;
}
pub struct SessionInfo { pub session_id: Secret, pub resume_url: String, pub sequence: u64, pub shard_id: u32, pub shard_count: u32 }
pub struct GatewayInfo { pub url: String, pub recommended_shards: u32, pub remaining: u32, pub total: u32, pub reset_after: Duration, pub max_concurrency: u32 }

/// What the socket task hands downstream. It never waits on anything downstream.
pub struct Frame { pub shard: u32, pub kind: DispatchKind, pub t: CompactString, pub buf: Vec<u8>, pub d: Range<usize> }
pub trait FrameSink: Send + Sync + 'static {
    /// Called after the sequence advanced. Must not block.
    fn push(&self, frame: Frame) -> Flow;
    /// Completes once reading may continue after `Flow::Pause`.
    fn wait_capacity(&self) -> BoxFuture<'_, ()>;
    fn notice(&self, notice: ShardNotice);
}
pub enum Flow { Continue, Pause }
#[non_exhaustive]
pub enum ShardNotice { Ready { shard: u32 }, Resumed { shard: u32 }, Disconnected { shard: u32, code: Option<u16> }, Failed { shard: u32, code: u16 }, Overload { shard: u32, shed: u64 }, IdentifyBudgetLow { remaining: u32 }, RateLimited { shard: u32, opcode: u32, retry_after: Duration, nonce: Option<String> } }
pub enum ShardStatus { Idle, Connecting, Identifying, Resuming, Ready, Failed(u16) }
pub enum ShutdownMode { Final, Resumable }

pub struct ShardManager;  // ::builder(token, intents) .shards(ids, total) .compression(..) .throttler(..) .session_store(..) .info_provider(..) .endpoints(..)
impl ShardManager {
    pub async fn start(self, sink: Arc<dyn FrameSink>) -> Result<ShardsHandle, GatewayError>;
}
#[derive(Clone)] pub struct ShardsHandle;
impl ShardsHandle {
    pub fn send(&self, shard: u32, payload: GatewayCommand) -> Result<(), GatewayError>;  // FIFO, survives reconnects
    pub fn status(&self, shard: u32) -> ShardStatus;
    pub fn latency(&self, shard: u32) -> Option<Duration>;
    pub async fn shutdown(&self, mode: ShutdownMode);
}
pub fn shard_of(guild: GuildId, total: u32) -> u32;   // (guild_id >> 22) % total
```

### 8.5 `dr_cache`

```rust
pub struct Table<K, V> { /* RwLock<HashMap<K, Arc<V>, foldhash>> */ }
impl<K: Eq + Hash, V> Table<K, V> {
    pub fn get(&self, k: &K) -> Option<Arc<V>>;
    pub fn insert(&self, k: K, v: Arc<V>) -> Option<Arc<V>>;   // returns the old value: that is `old`
    pub fn insert_if_absent(&self, k: K, v: Arc<V>);
    pub fn remove(&self, k: &K) -> Option<Arc<V>>;
    pub fn snapshot(&self) -> Vec<Arc<V>>;
    pub fn reserve(&self, additional: usize);
    pub fn len(&self) -> usize;
}
pub struct Change<T> { pub old: Option<Arc<T>>, pub new: Arc<T> }
pub struct CacheConfig;  // ::none() ::standard() ::everything() .messages_per_channel(n) .members(Limit) .chunk_on_ready(bool) .presences(bool) .dm_channels(n) .sweep(Sweep)
pub struct Cache;
impl Cache {
    pub fn new(config: CacheConfig) -> Self;
    pub fn wants(&self, kind: DispatchKind) -> bool;    // feeds needs_decode
    pub fn me(&self) -> Option<Arc<CurrentUser>>;
    pub fn guild(&self, id: GuildId) -> Option<Arc<GuildMeta>>;
    pub fn channel(&self, id: ChannelId) -> Option<Arc<Channel>>;
    pub fn permissions_in(&self, channel: ChannelId, user: UserId) -> Option<Permissions>;
    // Per event, in apply/<domain>.rs, pure and sync. Always returns the Arc the payload will use:
    //   apply_<event>(&self, shard: u32, ev: E) -> R
    //   R = Arc<T> for create, Change<T> for update, Option<Arc<T>> for delete, () otherwise.
}
```

### 8.6 `dr_voice`

```rust
/// `voice` does not depend on `gateway`. The facade implements this over `ShardsHandle`.
pub trait GatewaySender: Send + Sync + 'static {
    fn send_voice_state(&self, guild: GuildId, channel: Option<ChannelId>, self_mute: bool, self_deaf: bool) -> BoxFuture<'_, Result<(), BoxError>>;
}
pub struct VoiceManager;     // one per client, owns the clock thread
impl VoiceManager {
    pub fn new(sender: Arc<dyn GatewaySender>, config: VoiceConfig) -> Self;
    pub async fn join(&self, guild: GuildId, channel: ChannelId, options: JoinOptions) -> Result<VoiceConnection, VoiceError>;
    pub fn connection(&self, guild: GuildId) -> Option<VoiceConnection>;
    pub fn on_voice_state_update(&self, ev: &VoiceState);
    pub fn on_voice_server_update(&self, ev: &VoiceServerUpdateEvent);
    pub fn on_guild_removed(&self, guild: GuildId);
}
pub struct VoiceConnection;  // .subscribe(&AudioPlayer) .receiver() .status() .wait_for(status, timeout) .disconnect() .destroy()
pub struct AudioPlayer;      // ::new(PlayerOptions) .play(AudioResource) .pause(bool) .unpause() .stop(force) .status()
pub struct AudioResource;    // ::opus_packets(..) ::ogg(..) ::webm(..) ::ffmpeg(FfmpegSource) ::raw_pcm(..)  (each typed, never a guessed string)
pub struct VoiceReceiver;    // .subscribe(user, EndBehavior) -> AudioReceiveStream, .speaking()
```

### 8.7 `discord` (facade)

```rust
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type HandlerResult = Result<(), BoxError>;

#[non_exhaustive]
pub enum Error {
    Rest(RestError), Gateway(GatewayError), Validation(ValidationError),
    Interaction(InteractionError), Voice(VoiceError),
    NotReady, Timeout { what: &'static str }, Config(ConfigError),
    NotImplemented { what: &'static str },   // L0 stubs, gone before 1.0
    Other(String),
}
// From<RestError> sends RestError::Validation to Error::Validation [spike].
impl Error { pub fn code(&self) -> Option<JsonErrorCode>; pub fn request_succeeded(&self) -> bool; pub fn is_retryable(&self) -> bool; pub fn is_rate_limit(&self) -> bool; }
#[non_exhaustive]
pub enum InteractionError { AlreadyReplied, NotReplied, Option(String), DeadlineExceeded }

pub trait GatewayEvent: Copy + Send + Sync + 'static {
    const NAME: &'static str;
    const INTENTS: Intents;                  // any one is enough; empty means none needed
    type Payload: Clone + Send + 'static;    // Clone: one event can have several handlers
}
pub struct Updated<T> { pub old: Option<Arc<T>>, pub new: Bound<T> }
pub struct Deleted<P, T> { pub event: Bound<P>, pub cached: Option<Arc<T>> }

#[derive(Clone)] pub struct Client(/* Arc<ClientInner> */);   // not generic
pub struct ClientBuilder<S>;                                  // state erased into each handler closure
impl ClientBuilder<()> { pub fn state<N: Send + Sync + 'static>(self, state: N) -> ClientBuilder<N>; }
impl<S: Send + Sync + 'static> ClientBuilder<S> {
    pub fn on<E, F, Fut>(self, event: E, f: F) -> Self
    where E: GatewayEvent,
          F: Fn(Context<S>, E::Payload) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = HandlerResult> + Send + 'static;
    pub fn on_command<F, Fut>(self, name: &'static str, f: F) -> Self;       // Bound<CommandInteraction>
    pub fn on_component<F, Fut>(self, prefix: &'static str, f: F) -> Self;   // Bound<ComponentInteraction>, first match wins
    pub fn on_modal<F, Fut>(self, prefix: &'static str, f: F) -> Self;
    pub fn on_autocomplete<F, Fut>(self, name: &'static str, f: F) -> Self;
    pub fn on_raw<F, Fut>(self, f: F) -> Self;                               // every dispatch: t, d, DecodeFailed
    pub fn on_handler_error(self, f: impl Fn(HandlerFailure) + Send + Sync + 'static) -> Self;
    // intents shards cache dispatch_mode allow_missing_intents endpoints transport
    // identify_throttler session_store compression presence shutdown_drain queue_limits
    pub fn build(self) -> Result<Runner, Error>;   // errors if a handler can never fire [spike]
}
pub struct Runner;                                 // not generic
impl Runner {
    pub fn client(&self) -> Client;
    pub async fn run(self) -> Result<(), Error>;
    pub async fn run_until<F: Future + Send>(self, stop: F) -> Result<(), Error>;
    pub async fn run_until_with(self, stop: impl Future + Send, mode: ShutdownMode) -> Result<(), Error>;
}
pub struct Context<S> { pub client: Client, pub state: Arc<S>, pub shard: u32 }  // Deref<Target = Client>, Clone
impl<S> Context<S> {
    pub fn latency(&self) -> Option<Duration>;
    pub fn spawn<F: Future<Output = HandlerResult> + Send + 'static>(&self, f: F);
}
pub struct Bound<T> { /* Client, Arc<T> */ }       // Deref<Target = T>, Clone, 16 bytes
impl<T> Bound<T> { pub fn client(&self) -> &Client; pub fn data(&self) -> &Arc<T>; }

pub trait Bindable: Sized + Send + 'static {       // how a wire result becomes a user value
    type Wire: DeserializeOwned + Send + 'static;
    fn bind(client: &Client, wire: Self::Wire) -> Self;
}   // implemented for Bound<T>, Vec<Bound<T>>, () with Wire = rest::Empty, and a Collection form
#[must_use = "a request does nothing until it is awaited"]
pub struct Call<T: Bindable> { /* Client, Request<T::Wire> */ }
impl<T: Bindable> Call<T> { pub fn reason(self, r: impl Into<String>) -> Self; }
impl<T: Bindable> IntoFuture for Call<T> { type Output = Result<T, Error>; type IntoFuture = BoxFuture<'static, Self::Output>; }

pub struct HandlerFailure { pub event: &'static str, pub shard: u32, pub kind: FailureKind }
pub enum FailureKind { Error(BoxError), Panic(String) }
pub enum DispatchMode { Concurrent { max_in_flight: usize, interactions: usize }, OrderedPerShard }  // default 2048, 512
```

## 9. Public API

### 9.1 Three kinds of value

| Kind | Example | Holds | Use |
|---|---|---|---|
| Pure data | `model::Message`, `Arc<Guild>` | Fields only | Cache, REST results, serialization |
| Bound data | `Bound<Message>` | `Client` plus `Arc<T>`, 16 bytes | Event payloads, fetch results, cache reads. `Deref` gives fields |
| Id handle | `ChannelHandle`, `GuildHandle`, `UserHandle`, `MessageHandle` | `Client` plus id | Acts with no data. `.send()`, `.fetch()`, `.cached()` |

`impl Bound<Message> { ... }` is allowed in the facade by the orphan rule, because `Bound` is local, and it may be split across files, which is what lets F3 to F5 work in parallel [spike].

Nested data is not bound: `msg.author` is a plain `User`, so acting on it goes through `msg.author()`, which returns a `UserHandle`. This is the most surprising thing for a discord.js user, so the guide leads with it. A field and a method with the same name coexist without trouble [spike].

`msg.latest()` re-reads the current cached version. A held snapshot never changes.

Methods are inherent, so `msg.reply("x")` needs no import beyond having the value [spike].

### 9.2 Flow of `msg.reply("hi").await?`

1. `impl Into<CreateMessage>` accepts `&str`, `String`, `EmbedBuilder` or a full builder. Reply adds `message_reference`. **Nothing is sent yet** [spike].
2. `SendMessageRequest` is `#[must_use]`, with `.no_mentions()`, `.file(...)` and so on.
3. `into_future` calls `rest.channels().create_message(channel, body)`. `IntoBody` runs validation, and a failure surfaces at `.await` as `Error::Validation` with nothing sent [spike].
4. `Rest::execute` runs limiter, retries and error mapping in non-generic code. A thin generic tail decodes. Unknown keys in the reply are ignored [spike].
5. `Bindable::bind` wraps it as `Bound<Message>`, which can act again with no client passed [spike].
6. A 2xx with an unparseable body gives `Err(Rest(Decode { .. }))`, and `err.request_succeeded()` is `true`, so a caller does not repeat the send [spike].

### 9.3 Surfaces

| Area | API |
|---|---|
| Start | `Client::builder(token)...build()?.run_until(signal).await?` |
| Events | `.on(Token, handler)`, `.on_raw(handler)`, `client.subscribe(Token)` (bounded stream, yields `Lagged(n)`), `client.await_event(Token).filter(..).timeout(d).await` |
| Commands, components | `.on_command(name, h)`, `.on_component(prefix, h)`, `.on_modal(prefix, h)`, `.on_autocomplete(name, h)`, plus `.on(InteractionCreate, h)` for everything |
| Manager triad | `manager.get(id)` sync, cache only. `manager.fetch(id)` HTTP only. `manager.resolve(id)` cache then HTTP, inserting if the policy allows |
| Member requests | `guild.members().request().query("a").limit(10).stream()`, gateway op 8 |
| Escape hatches | `client.rest().guilds().get_member(...)` typed, `client.rest().raw(Method::Get, path)` untyped, `.on_raw` |
| Collectors | `msg.collect_components()`, `channel.collect_messages()`, `msg.collect_reactions()`, `client.collect_interactions()`: `.timeout .idle .max .only_user .filter .stream()`. Shortcuts `msg.await_component()`, `channel.await_messages()`, `cmd.await_modal_submit()` |
| Interactions | `reply`, `defer`, `edit_reply`, `follow_up`, `delete_reply`, `fetch_reply`, `show_modal`, `update`, `respond`. `options().string(n)` and `require_string(n)` |
| Voice, feature `voice` | `client.voice().join(guild, channel, JoinOptions::default()).await?`, `conn.subscribe(&player)`, `player.play(AudioResource::ogg(path).await?)` |
| Observe | `client.stats()`: atomics for decode failures per event, queue bytes, shed count, bucket waits, invalid requests, shard latency. No metrics dependency |
| Stop | `client.shutdown(ShutdownMode::Resumable)`, or the signal passed to `run_until` |

### 9.4 Full example bot

This file compiles against the spike facade with real `Send + 'static` bounds, and passes clippy under the workspace lints [spike]. It has never run against Discord. L0 commits it as `crates/discord/examples/poll_bot.rs`. It stays in CI as the shape contract.

```rust
//! The example bot of ARCHITECTURE.md, type-checked against the spike facade.
use discord::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// Shared bot state, reached through `ctx.state`, checked at compile time.
#[derive(Default)]
struct Polls {
    tallies: Mutex<HashMap<MessageId, (u32, u32)>>,
}

const POLL: &str = "poll";
const MOD: &str = "mod";

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let token = std::env::var("DISCORD_TOKEN")?;

    Client::builder(token)
        .state(Polls::default())
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::GUILD_MEMBERS | Intents::MESSAGE_CONTENT)
        .shards(ShardConfig::Auto)
        .cache(CacheConfig::standard().messages_per_channel(50))
        // Fires once per client start, after every managed shard reported in.
        .on(ClientReady, |ctx, ready| async move {
            println!("logged in as {} on {} shards", ready.user.username, ready.shard_count);
            ctx.commands()
                .set_global(commands![
                    SlashCommandBuilder::new(POLL, "Start a yes/no poll")
                        .string_option("question", "What to ask", |o| o.required(true).max_length(200)),
                    SlashCommandBuilder::new(MOD, "Moderation")
                        .subcommand(Subcommand::new("kick", "Kick a member").user_option("target", "Who", |o| o.required(true))),
                    UserContextCommand::new("Inspect"),
                ])
                .await?;
            Ok(())
        })
        // The event fires whether or not the channel is cached.
        .on(MessageCreate, |ctx, msg| async move {
            if msg.author.bot {
                return Ok(());
            }
            if msg.content == "!ping" {
                let latency = ctx.latency().map(|d| d.as_millis());
                let sent = msg.reply(format!("pong, gateway latency {latency:?} ms")).no_mentions().await?;
                sent.react("\u{1F3D3}").await?;
            }
            Ok(())
        })
        // Update events: old is the cached snapshot when one was held, new always exists.
        .on(MessageUpdate, |_ctx, change| async move {
            if let Some(old) = change.old.as_ref().filter(|o| o.content != change.new.content) {
                println!("edited in {}: {:?} -> {:?}", change.new.channel_id, old.content, change.new.content);
            }
            Ok(())
        })
        // Act on an id with no data at all: handles need no cache.
        .on(GuildMemberAdd, |ctx, member| async move {
            let Some(guild_id) = member.guild_id else { return Ok(()) };
            let Some(guild) = ctx.guild(guild_id).cached() else { return Ok(()) };
            if let Some(channel) = guild.system_channel_id {
                let name = member.user.as_ref().map_or("someone", |u| u.username.as_str());
                ctx.channel(channel).send(format!("Welcome, {name}")).no_mentions().await?;
            }
            Ok(())
        })
        // Slash command: answer inside the deadline, then detach the long-lived collector so
        // that it does not hold one of the handler in-flight permits for a whole minute.
        .on_command(POLL, |ctx, cmd| async move {
            let question = cmd.options().require_string("question")?.to_owned();
            cmd.reply(
                CreateMessage::new()
                    .embed(EmbedBuilder::new().title("Poll").description(&question))
                    .row(ActionRowBuilder::new().primary("vote:yes", "Yes").danger("vote:no", "No")),
            )
            .await?;
            let poll = cmd.fetch_reply().await?;
            if let Ok(mut t) = ctx.state.tallies.lock() {
                t.insert(poll.id, (0, 0));
            }

            let state = ctx.state.clone();
            ctx.spawn(async move {
                let mut clicks = poll
                    .collect_components()
                    .timeout(Duration::from_secs(60))
                    .filter(|i| i.custom_id.starts_with("vote:"))
                    .stream();
                while let Some(click) = clicks.next().await {
                    let (yes, no) = {
                        let mut tallies = state.tallies.lock().map_err(|e| e.to_string())?;
                        let entry = tallies.entry(poll.id).or_default();
                        if click.custom_id == "vote:yes" {
                            entry.0 += 1;
                        } else {
                            entry.1 += 1;
                        }
                        *entry
                    };
                    click.reply(format!("Counted. Yes {yes}, no {no}.")).ephemeral().await?;
                }
                let (yes, no) = state.tallies.lock().map_err(|e| e.to_string())?.remove(&poll.id).unwrap_or_default();
                poll.edit(EditMessage::new().content(format!("{question}: {yes} yes, {no} no")).clear_components()).await?;
                Ok(())
            });
            Ok(())
        })
        // Typed errors: branch on the Discord code rather than a string.
        .on_command(MOD, |_ctx, cmd| async move {
            let Some(guild) = cmd.guild() else {
                cmd.reply("Guild only").ephemeral().await?;
                return Ok(());
            };
            if cmd.options().subcommand() == Some("kick") {
                let target = cmd.options().require_user("target")?;
                cmd.defer().ephemeral().await?;
                let outcome = guild.members().kick(target).reason("mod kick command").await;
                let text = match outcome {
                    Ok(()) => "Done".to_owned(),
                    Err(e) if e.code() == Some(JsonErrorCode::MISSING_PERMISSIONS) => "I lack permission".to_owned(),
                    Err(e) => return Err(e.into()),
                };
                cmd.edit_reply(EditMessage::new().content(text)).await?;
            }
            Ok(())
        })
        // A persistent button handler: it survives restarts because it keys on custom_id only.
        .on_component("admin:", |_ctx, click| async move {
            click.reply("Not implemented yet").ephemeral().await?;
            Ok(())
        })
        .on_handler_error(|failure| eprintln!("handler for {} failed: {}", failure.event, failure.kind))
        .build()?
        .run_until(tokio::signal::ctrl_c())
        .await?;
    Ok(())
}
```

`std::sync::Mutex` is fine here because the guard never crosses an `.await`, which the `Send` bound checks.

### 9.5 Misuse catalogue

`crates/discord/tests/ui/` keeps these as `trybuild` cases. All fail to compile today [spike].

| Case | Mistake | Error |
|---|---|---|
| `swapped_ids` | `ctx.channel(guild_id)` | E0308 |
| `wrong_id_marker_compare` | `user_id == guild_id` | E0308 |
| `not_send` | `Rc` across an `.await` in a handler | "future cannot be sent between threads safely" |
| `std_lock_across_await` | `std::sync::MutexGuard` across an `.await` | same |
| `option_after_subcommand` | `.subcommand(s).string_option(...)` | E0599 |
| `subcommand_after_option` | `.string_option(...).subcommand(s)` | E0599 |
| `forgot_await` | `msg.reply("a");` | unused `SendMessageRequest` that must be used |
| `wrong_payload` | handler typed for the wrong event | E0631 |
| `edit_with_create_body` | `msg.edit(CreateMessage::new())` | E0308. This is the PATCH-wipes-fields trap of `[types 9.5]` |
| `missing_ok` | handler with no `Ok(())` | E0271 |
| `state_twice` | `.state(a).state(b)` | E0599 |
| `async_closure_capture` | capturing `async move \|c, m\| {..}` | "async closure does not implement `Fn`" |

`crates/discord/examples/no_prelude.rs` is the positive case: inherent methods with no trait import, and a plain `async fn` as a handler.


## 10. Runtime semantics

This section is binding. The reports in `docs/design/` hold the reasoning and the source line numbers: `[gateway 7]` means section 7 of `gateway.md`. When this section and a report disagree, this section wins. When this section is silent, the report wins, and quirks it lists under "preserve" are preserved.

"Added" marks behaviour upstream does not have. "[external]" marks a Discord behaviour nobody has verified against a live session (section 16).

### 10.1 Gateway

#### Connection

- URL: `${resume_url ?? url}?v=10&encoding=json[&compress=zlib-stream|zstd-stream]`. The query goes on the resume URL too. `url` comes from `GET /gateway/bot` through `GatewayInfoProvider`.
- Each connection gets a new decompressor and a new send limiter. Nothing is sent before Hello.
- Resume only if the stored `shard_count` matches the current one. Otherwise identify, still at the resume URL if there is one.
- Reconnect delay: floor 500 ms, plus exponential backoff with full jitter, base 1 s, cap 60 s, reset after 60 s of stable connection.
- WebSocket message size limit is configurable and large, 256 MiB by default, because `GUILD_CREATE` is unbounded.
- `identify` always sends `shard`, `capabilities` and `compress`. It sends `large_threshold` (default 50) and `presence` only when set. Token prefix `/^bot\s*/i` is stripped, and the token stays redacted in every log.

| Timeout | Value | Covers |
|---|---:|---|
| Hello | 60 s | connect, TLS, upgrade and Hello together |
| Handshake | 30 s | WebSocket upgrade alone |
| Ready | 15 s | from identify to `READY` |
| Resumed | 30 s | from resume to `RESUMED`. Added |
| Close wait | 5 s | waiting for the peer's close frame, then drop. Added |
| Write | 10 s | a stuck write is treated as a zombie connection. Added |
| `wait_guild_timeout` | 15 s | Readiness, below |
| Member fetch | 120 s, refreshed per chunk | op 8 |
| Soundboard fetch | 10 s, refreshed per event | op 31 |

#### Close codes

`[gateway 7]`, with the corrections below.

| Code | Action | Session |
|---:|---|---|
| 1000 | Reconnect | Clear |
| 4200 sent by us | No action: we chose this close and the caller already knows why | Keep |
| 4200 received from peer | Treat like resume (upstream defect 15.13) | Keep |
| 4000, 4001, 4002 | Resume | Keep |
| 4003, 4005, 4007, 4008 | Reconnect and identify | Clear |
| 4004, 4010, 4011, 4012, 4013, 4014 | Stop. Terminal `ShardStatus::Failed(code)` | Clear |
| 4009 | Resume. An op 9 falls back to identify. Upstream resumes, docs say new session | Keep |
| Any other, including after a socket error | Attempt resume. Op 9 corrects if wrong. **Deviation**, 10.8 | Keep |
| Hello or Ready timeout | Reconnect and identify | Clear |

A terminal failure before the first ready makes `run` return `Err`. After that it emits `ShardFailed`, and `run_until` returns `Err` once every shard is terminal. A 4004, or a REST 401 on the default token, also emits `TokenInvalidated`.

Op 9 with `d: true` resumes on the same socket after 1-5 s of jitter [external]. With `d: false` it opens a new socket and identifies. Op 7 reconnects and resumes.

#### Heartbeat

- First beat at `floor(interval * random)`, then every `interval`. Beats continue during the identify wait.
- `d` is the stored sequence, including during a resume, and serializes as `null` before the first sequence.
- Server op 1: beat now, skip the ack check, still expect an ack.
- Zombie: one missed ack at the next tick closes with 4200 and resumes.
- Zombie detection is suspended while reading is paused (Backpressure, below). Beats continue.

#### Sequence and decoding

- The socket task reads `op`, `s` and `t` from the envelope with `RawValue`, advances `sequence` monotonically and stores it **before** dispatch and before typed decoding. A frame that fails typed decoding is therefore still acknowledged.
- `READY` and `RESUMED` are delivered as dispatches too.
- Unknown opcode: ignored. Unparsable text frame: dropped and counted. Inflate failure: reconnect.

#### Compression

- `zlib-stream` (default, pure Rust through zlib-rs): one `Decompress` per connection. A message is complete when the buffer ends in `00 00 FF FF` at a WebSocket message boundary. It may span several WebSocket messages.
- `zstd-stream`: one stream per connection, each WebSocket message complete, context kept.
- Identify `compress: true`: standalone binary frames, possibly mixed with text. Dispatch on the frame type. If a transport compression is set, it wins and `compress` is false.
- ETF is cut.

#### Send budget

- Ops 1, 2 and 6 bypass the limiter and the status gate.
- Other ops wait for Ready or Resumed (fix for defect 15.1) in FIFO order, limited to 114 per 60 s fixed window per connection, then sleep to the reset plus up to 1.5 s of jitter. The upstream check is `sent + 1 >= 115` (`WebSocketShard.ts:488`).
- The queue is data held by the shard and survives reconnects.
- Payload limit 4096 bytes [external], enforced at send with an error. Op 8 nonce at most 32 bytes. Close reason at most 123 bytes.

#### Identify throttle

- Bucket `shard % max_concurrency`, 5 s per slot, up to 1.5 s of jitter, FIFO, abortable while sleeping. The throttler is created once.
- `session_start_limit.remaining` is a tracked budget with a reserve of 10. Below the reserve the manager waits `reset_after` and emits `IdentifyBudgetLow`.

#### Pipeline

```
socket task (one per shard)         owns the WebSocket, state, timers
  select: commands | heartbeat | write queue | read, guarded by !paused
      | Frame, after the sequence advanced. Never waits downstream.
      v
byte-budget queue (one per shard)   FrameSink. INTERACTION_CREATE takes a priority lane with its own budget
      |
      v
ingest task (one per shard)         typed decode, cache apply in strict order, classify, collectors
      |
      v
handlers                            each in its own task, under catch_unwind, bounded by a semaphore
```

The socket task owns no decode and takes no cache lock, so slow handlers cannot delay a heartbeat [probe]. Rules:

| Queue size for the shard | Behaviour |
|---:|---|
| Below 32 MiB | Normal |
| Above 32 MiB | Shed sheddable events (typing, presence). The sequence still advances. Count them and emit `ShardOverload` |
| Above 64 MiB | Pause reading. Heartbeats continue, zombie detection is suspended |
| Paused over 30 s | Close 4200 and resume |

Discord's reaction to a client that stops reading is [external]. Loopback behaviour is [probe].

A supervisor per shard catches a panic in any of these tasks, logs it, keeps the session and restarts with backoff. A stall detector logs `HandlerLagging`.

`needs_decode[kind]` is an atomic table computed from registered handlers and `cache.wants(kind)`, raised at runtime by `subscribe`. An event nobody needs is not decoded, but its sequence still counts.

#### Dispatch modes

- `Concurrent { max_in_flight: 2048, interactions: 512 }`, the default. Handlers of one event run concurrently, so order between handlers is not guaranteed. Cache order is.
- `OrderedPerShard`: one handler at a time per shard, still under `catch_unwind`.
- `client.subscribe::<E>()`: a bounded stream with `Lagged(n)`.

A long-lived collector inside a handler holds an in-flight permit. Use `ctx.spawn`, as the example in 9.4 does. `ctx.spawn` tasks are tracked and drained at shutdown, and a panic or error from one reaches `on_handler_error`.

#### Readiness and events

- `ShardReady` fires on every `READY`, `ShardResumed` on every resume.
- `ClientReady` fires **once per client start**, when every managed shard finished its guild list or passed `wait_guild_timeout`. Deviation from `[client 16.2]`. `client.wait_until_ready()` awaits the same point.
- Events are never held back. `GUILD_CREATE` is classified through a per-shard `expected: HashSet<GuildId>` that works with no cache at all, giving `GuildCreate` or `GuildAvailable`.
- `GUILD_DELETE` with `unavailable: true` is an outage, `GuildUnavailable`. With it absent it is a removal, which cascades in the cache and destroys the guild's voice connection.

All 79 dispatch events are exposed. Upstream handles 72. The 7 extra: `CHANNEL_INFO`, `INTEGRATION_CREATE`, `INTEGRATION_DELETE`, `INTEGRATION_UPDATE`, `RESUMED`, `VOICE_CHANNEL_START_TIME_UPDATE`, `VOICE_CHANNEL_STATUS_UPDATE`.

Lifecycle tokens: `ShardReady`, `ShardResumed`, `ShardDisconnected { code }`, `ShardFailed`, `ShardOverload`, `IdentifyBudgetLow`, `HandlerLagging`, `TokenInvalidated`. Derived: `ClientReady`, `GuildAvailable`, `GuildUnavailable`. Upstream's `Debug` and `Warn` are `tracing`. `Error` is `on_handler_error` or `ShardFailed`. `Raw` is `on_raw`.

Payload shape: create events give `Bound<T>`, update events `Updated<T>`, delete events `Deleted<P, T>`, and reactions carry `message: Option<Arc<Message>>`. **Every event always fires**, built from its payload, so there are no Partials.

The intent column in `[gateway 12]` was written from memory, and two voice-channel intents are unverified. M5 checks each row against discord-api-types, and H6 checks the two voice-channel ones live.

#### Requests over the gateway

- Op 8 complete when `size < 1000 || (limit > 0 && fetched >= limit) || index == chunk.count`. Defaults: limit 0, time 120 s, query `initial ?? (users ? none : "")`. Registry keyed by nonce. A `RATE_LIMITED` event with that nonce fails the pending request with `GatewayRateLimitError`. Other opcodes warn once per opcode.
- Op 31 groups by shard, 10 s, refreshed per event.
- Thread member rows use `id` for the thread id.

#### Sessions and shutdown

- `SessionStore` writes through on `READY`, `RESUMED`, every 30 s and at shutdown. The live sequence stays in memory. `JsonFileSessionStore` ships. A crash means at-least-once delivery from the last stored sequence.
- Shutdown order: stop reading, drain the queue (`shutdown_drain`, 10 s), wait for in-flight handlers and `ctx.spawn` tasks, abort the rest, close sockets. `ShutdownMode::Final` closes 1000. `ShutdownMode::Resumable` closes 4200 and flushes the store.
- Presence rules: `[client 16.14]`. Do not reproduce upstream defects 15.1-15.20 in `gateway.md`.

### 10.2 REST

#### Route generalization

Exact order `[rest 3]`, tested with its vectors:

1. `/interactions/<id>/<token>/callback` gets major `burst` and bucket `/interactions/:id/:token/callback`.
2. Major parameter, ASCII only: `^/webhooks/([0-9]{17,19}/[^/?]+)` or `^/(channels|guilds|webhooks)/([0-9]{17,19})`, taking group 2, else group 1, else `global`.
3. Replace every `[0-9]{17,19}` with `:id`.
4. Replace the first `/reactions/(.*)` with `/reactions/:reaction`.
5. Replace the first `/webhooks/:id/[^/?]+` with `/webhooks/:id/:token`.
6. `DELETE` of exactly `/channels/:id/messages/:id` whose snowflake is older than 14 days (wall clock) appends `/Delete Old Message`.

Non-snowflake segments stay literal, so `/users/@me` and `/users/%40me` are different routes. Webhook majors contain a token and are masked in anything observable.

#### Bucket keys

- Hash key: `METHOD:bucketRoute[:partition]`. A fallback `Global(key)` is not stored.
- Handler key: `hash:major`.
- Learn the hash from `X-RateLimit-Bucket` on any response, errors included. The old handler drains. A new one starts at `remaining = 1`, `reset = -1`.
- Partition: a per-process seeded hash of a custom token that differs from the instance token. No uuid dependency.

#### Response headers

- Read `X-RateLimit-Reset-After` only (`f64`), plus `Limit`, `Remaining`, `Bucket`, `Retry-After`, `Scope` (default `user`) and `Global` (presence).
- Missing headers reset the state: `limit = inf`, `remaining = 1`, `reset = now`. A parse failure counts as absent. `"0"` is valid.
- `Retry-After` is examined on every status. The JSON body of a 429 is ignored.

#### Limiter

This replaces upstream's `#asyncQueue` and `#sublimitedQueue`, which can hang forever (`[rest 9]`, sequence R, X, Y).

```rust
struct Bucket {
    standard: Semaphore,        // 1 permit
    sublimit: Semaphore,        // 1 permit
    state: Mutex<BucketState>,  // never held across await
}
```

Flow of one attempt:

1. Look up the bucket under the map lock and clone its `Arc`. The sweeper removes a bucket only when `Arc::strong_count == 1` under that same lock, so a bucket cannot vanish in between.
2. If `sublimit_active && has_sublimit(req)`, acquire `sublimit` then `standard`. Otherwise acquire `standard` only. The order is fixed, so there is no cycle.
3. Wait for the bucket reset and the global limit.
4. Send, with the timeout covering headers only.
5. Update state from the response, in the exact order of `[rest 14.1-14.6]`.
6. Release at headers. On a 4xx, the body read keeps holding the queue.

All permits are RAII, so dropping the future at any `.await` releases them.

- Sublimit detected: `retry_after > 0`, not global, and not `local_limited` after the state update.
- `has_sublimit`: on `/channels/:id` only for `PATCH` with `name` or `topic`. True on any other route.
- **Required tests:** the X/Y/R stall regression, ports of `Handle sublimits`, `Handle unexpected 429` and its Cloudflare variant.

Whether this removes the stall is reasoned, not proven, until R2's test passes (section 16).

#### Global limit and burst

- 50 per second by default, in a lazily opened 1 s window, decremented per attempt under one lock.
- A 429 with `X-RateLimit-Global` blocks every sequential handler on the instance. A Cloudflare-style 429 stays local.
- Burst handler for interaction callbacks: no queue, no global accounting, sleeps `retry_after`.

#### Retries

Default: 3 retries, so `attempts = retries + 1`. 15 s per attempt, headers only: wrap the send future, do not use a total timeout in reqwest. Body timeout 30 s. Offset 50 ms, applied once (deliberate deviation from upstream's double offset). Backoff 0 by default, with an optional `deadline`.

```rust
pub enum RetryClass {
    /// GET, PUT, DELETE. The default for those methods.
    Idempotent,
    /// POST, PATCH. The default for those methods.
    Unprotected,
    /// Message create with `enforce_nonce`. The client generates a nonce when retries are enabled.
    Protected,
    /// Interaction callbacks. Resend only while the 3 s deadline allows. A 40060 after an
    /// ambiguous attempt counts as success, which is what makes the resend safe.
    Callback,
}
```

| Failure | `Idempotent` | `Protected` | `Unprotected` | `Callback` |
|---|---|---|---|---|
| Known not sent (connect, DNS, TLS) | Retry | Retry | Retry | Retry if the deadline allows |
| 502, 503 | Retry | Retry | Fail | Retry if the deadline allows |
| Other 5xx | Retry | Fail | Fail | Fail |
| Timeout or reset (ambiguous) | Retry | Retry | Fail | Retry if the deadline allows |
| 429 | Wait, no attempt consumed | Same | Same | Same, within the deadline |
| Cancelled by the user | Never | Never | Never | Never |

The `Protected` window on Discord's side is [external].

- A retried `DELETE` that gets a 404 after an ambiguous first attempt returns that 404. The delete probably worked, and `RequestSummary.attempts` lets the caller see the retry.
- Header-less 429: floor 100 ms, doubling, cap 10 consecutive with no progress, then `RateLimited`.
- Queue limit 10 000 gives `QueueFull`.
- `RejectOnRateLimit`: the request setting, else the instance setting.

#### Invalid-request breaker

- A process-wide static, because Discord counts per IP. 10-minute fixed window over 401, 403 and 429.
- Warn at 5000. At 9000 requests fail with `CircuitOpen`. The IP ban at 10 000 is [external].
- A 401 with default auth invalidates the token: every later call fails fast with `TokenInvalidated` and the gateway stops. Webhook and interaction calls use `Auth::None`, so a 401 there does not invalidate.

#### Request shape

- Redirects disabled. User agent `DiscordBot (url, version) appendix`.
- Header order: request, `Content-Type`, instance, user agent, auth, `X-Audit-Log-Reason`. The reason uses an `encodeURIComponent` equivalent, its own `AsciiSet` leaving `A-Za-z0-9-_.!~*'()`, and is sent only when non-empty.
- An empty `POST`, `PUT` or `PATCH` sends `Content-Length: 0`. `body: {}` sends `{}`. A `GET` with a body drops it.
- Query strings are `x-www-form-urlencoded`, space as `+`. Skip `None`. Scalars carry no JSON quotes.
- Route arguments matching `^[0-9%A-Za-z_-]+$` are used as is. Others go through the `encodeURIComponent` equivalent. `@me` and `@original` are typed targets, sent as `%40me` in real traffic. `Routes.user()` with its default gives `/users/@me`. Emoji are encoded in the 5 channel methods listed in `[rest 15]`.

Multipart, hand-encoded: files first, then `payload_json` or text fields. Key `files[i]` or `file.key`. Sniff with `infer`, mapping `apng` to `png`, and empty types to `application/octet-stream`. Escape `"`, CR and LF in filenames to `%22`, `%0D`, `%0A`, leaving raw UTF-8 alone. Keep the body as `Bytes` so a retry does not re-encode it.

#### Responses

- Parse JSON by `Content-Type` prefix. Non-JSON 4xx is `Unparsed`. A 2xx with a bad body is `Decode` with `request_succeeded() == true`: do not repeat the send.
- Error flatten is byte-exact, `[rest 10]`. Code 0 is real. OAuth errors carry a string `error`. Secrets are masked.

#### Scope and sweepers

- 18 groups, 212 methods, plus `rest.raw`. Query defaults `[client 16.39]`, wire conventions `[client 16.41]`, pruning `[client 16.40]`.
- CDN: 20 methods `[rest 11]`, `animated` checked before `size`, with allow-lists. An `a_` hash gives `.webp?animated=true`.
- Sweepers hold `Weak`: handlers at 1 h, hashes at 4 h idle, 24 h lifetime.

### 10.3 Cache

#### Model

- Immutable snapshots: values are `Arc<T>`. An update builds a new value, and `old` is what the swap returned.
- Each shard's ingest task is the only writer for its guilds. `users` and `dms` are last write wins.
- REST responses do not write to the cache, except an explicit `.cache(true)` or `resolve`, through `insert_if_absent`. The value returned by `edit` is fresh, and the cache catches up on the gateway event. This removes the quirks of `[client 16.20]`.
- `Collection` is ported for fetch results. Caches do not use it.

```rust
pub struct Cache {
    guilds: Sharded<GuildId, GuildSlot>,            // 256 shards
    channels: Sharded<ChannelId, GuildId>,          // index only
    users: Sharded<UserId, CachedUser>,
    dms: Table<ChannelId, Channel>,
    messages: Sharded<ChannelId, Mutex<VecDeque<Arc<Message>>>>,
    me: ArcSwapOption<CurrentUser>,
    policy: CacheConfig,
}
pub struct GuildSlot {
    meta: ArcSwap<GuildMeta>,
    channels: Table<ChannelId, Channel>,
    roles: Table<RoleId, Role>,
    members: Table<UserId, CachedMember>,
    voice: Table<UserId, VoiceState>,
    rare: OnceLock<Box<RareTables>>,   // emojis, stickers, scheduled events, ... allocated on first use
}
pub struct CachedMember { roles: SmallVec<[RoleId; 3]>, nick: Option<CompactString>, joined_at: i64, flags: u32, pending: bool }
pub struct CachedUser { id: UserId, username: CompactString, global_name: Option<CompactString>, avatar: Option<ImageHash>, flags: u32, bot: bool }
pub enum ImageHash { Static([u8; 16]), Animated([u8; 16]), Other(Box<str>) }
```

Removing a guild is a drop. `reserve` from `member_count` on chunks. Budget about 312 bytes per member and user pair [probe].

#### Policy

| Item | `standard()` |
|---|---|
| Guild meta, channels, roles, threads | On, no limit available |
| Own member | Always |
| Other members | Only those seen in events. No chunking. `.members(Limit::PerGuild(n))`, `.chunk_on_ready(true)` |
| Users | On, with TTL sweep |
| Messages | Off. `.messages_per_channel(n)` |
| Presences | Off |
| Voice states | On when the intent is set |
| Emojis and other rare tables | Lazy |
| DM channels | 1000 |

`none()` and `everything()` are the other presets. Eviction is arbitrary, a documented deviation from FIFO `LimitedCollection`. Sweepers are plain functions plus `Weak` interval tasks: `Sweep::messages(Duration)`, and threads default to 3600 s and 14 400 s `[client 8]`.

#### Lock rules

No guard leaves the crate. No closure runs under a lock. `await_holding_lock` is denied. Tests run with parking_lot deadlock detection.

#### Quirks

Keep, from `[client 16]`:

| Item | What |
|---:|---|
| 18 | Absent versus null, with its asymmetries |
| 22 | Presence synthesizes a member |
| 23, 24 | Reaction identifier encoding. Reaction count: check with the API |
| 26 | Counters |
| 27 | A channel type change retains messages |
| 28 | `_remove` cascades |
| 29 | Guild patch rebuild rules. Emojis and stickers are diffed |
| 30 | `streaming` only with `self_video` |
| 31 | Message patch: author cached unless webhook, created from the snowflake, snapshots |
| 32 | Getter gates |
| 33 | Bulk delete with one id uses the single delete |
| 34 | Positions: channel ties ascending id, role ties descending id |
| 36 | Permission strings. `default_member_permissions` null versus 0 |
| 44 | `@everyone` keyed by guild id |

Also keep: permission computation `[client 11.2]` with `Administrator`, role tags presence as bool, `referenced_message` tri-state, `Typing.timestamp` in seconds.

Fix rather than keep: items 37, 45, 46, 58, 59, 62-69.

### 10.4 Sharding

- In-process tasks by default. `ShardConfig::Auto` or `Range { ids: 0..=63, total }`, inclusive. Environment `SHARDS` and `SHARD_COUNT` apply only when the builder has defaults.
- Routing is `(guild_id >> 22) % total`.
- Multi-process deployments use `shard_ids`, a custom `IdentifyThrottler` and a `SessionStore`. The REST global limit is per bot, so several processes need a proxy through `.endpoints`.
- `dr_cluster`, H7: a spawner and typed IPC with `register::<Req, Resp>("name", handler)` and correlation ids. `broadcastEval` and `fetchClientValues` are replaced by named RPC. `fetchRecommendedShardCount` keeps its formula `ceil(shards * (1000 / guildsPerShard) / multipleOf) * multipleOf`. Sequential spawn every 5.5 s.
- No live resharding. No worker strategy: tasks already fill that role.

### 10.5 Voice

Voice is the one area that needs live checks. `voice.md` is the source of truth for detail, and these are the decisions on top of it. About 5,200 lines of code.

#### State machines

- Connection: `Signalling`, `Connecting`, `Ready`, `Disconnected`, `Destroyed`. Transition table and setter order: `[voice 3]`. The setter order is observable, so keep it.
- Networking: `OpeningWs`, `Identifying`, `UdpHandshaking`, `SelectingProtocol`, `Ready`, `Resuming`, `Closed`. `[voice 4.1]`.
- The connection registry is `VoiceManager`, owned by the client, not a process global.

#### Voice gateway

- `wss://{endpoint}?v=8`, endpoint used as given. Opcodes 0-9, 11, 13, 21-31 `[voice 4.2]`.
- Binary inbound `[seq u16 BE][op u8][payload]`, outbound `[op u8][payload]`.
- Decode `{ op, d: RawValue, seq: Option<i64> }` and match, so a new opcode is ignored.
- `seq_ack` starts at -1. A JSON `seq: 0` is ignored, a binary one is accepted. Upstream's falsy check is preserved on purpose, `[voice 4.2]`.
- Heartbeat at the fixed interval, no jitter, no immediate beat: `{ t, seq_ack }`. Close at 3 missed. A local close is bounded and synthesizes 1006 (added).
- All WebSocket frames, including ops 11 and 21-31, are handled in arrival order by one task. Otherwise recognized ids are wrong when proposals arrive.

#### Close codes

| Code | Networking Ready | Connection |
|---:|---|---|
| `< 4000`, 4015 | Resume, once. Only from `Ready` | Stays `Connecting` |
| 4004, 4014 | Close | `Disconnected`. A later server update reconnects. A helper offers the 5 s wait pattern |
| 4016, 4017, 4021, 4022 | Close | `Disconnected` (A1, A2) |
| Other | Close | Rejoin with backoff |

#### UDP and RTP

- IPv4 unconnected socket. Discovery request of 74 bytes: type 1, length 70, ssrc, 66 zero bytes. Response type 2, IP from offset 8 NUL-terminated, port big-endian at `len - 2`. **Added:** retransmit and a deadline (A11). Upstream waits forever.
- Keep-alive of 8 bytes every 5 s. `WSAECONNRESET` is non-fatal on Windows.
- Packet: `[0x80][0x78][seq BE][ts BE][ssrc BE] ciphertext+tag(16) [nonce u32 BE]`. AAD is the 12-byte header. Nonce increments before use, first value 1. Timestamp +960 per packet, sequence wrapping.
- Op 5 before the first packet, only on change, reset after `Resumed`.
- Encryption: `aead_aes256_gcm_rtpsize` preferred **explicitly**, unlike upstream, whose selection follows the server's order. `aead_xchacha20_poly1305_rtpsize` is the fallback. No match is a typed error, not an endless wait.

#### DAVE

- Mandatory for non-stage voice since March 2026 [external]. `dave_protocol_version: 0` still works, so `Option<DaveSession>`.
- `crates/voice/src/dave.rs` is the only file that names `davey`, pinned `=0.2.0`. Use the mapping of `[voice 6.4]`, not a port of the JS wrapper:
  - `prepare_transition` and `execute_transition` must be called. `process_commit` and `process_welcome` no longer install keys.
  - `can_encrypt()` is the send gate, not `is_ready()`.
  - The bot's own id goes into recognized ids, or welcomes fail with `UnrecognizedUser`.
- Op 31 then op 26. Transition id 0 sends no op 23. Silence `F8 FF FE` skips DAVE. Tolerance 36. Expiry 10 s and 24 s.

#### Decisions on `[voice 12.A]`

| Item | Decision |
|---|---|
| A1, A2 | Stop on 4004, 4014, 4016, 4017, 4021, 4022, as in the table above. Rejoin has configurable backoff, on by default |
| A3 | Gate receive on Networking state only. Drop when DAVE is active and the frame cannot be decrypted |
| A4 | Drop frames until `can_encrypt()`. `.allow_plaintext_during_setup(true)` opts in. Subject to open question 4 |
| A5 | Destroy Networking on a null endpoint. Every other transition states in the code whether Networking is kept or torn down, since the state enum forces the choice |
| A6, A7 | Signal stop after dispatching the last silence frame. Flush on Idle and clear the prepared packet on a resource change, so all 5 silence frames go out |
| A8 | `Result` everywhere. Length-check before parsing. MLS errors take the recovery path, op 31 plus reinit |
| A9 | Redact, zeroize |
| A10 | A dead UDP path stays undetected. H6 checks whether Discord echoes keep-alives, then decide |
| A11 | Discovery retransmit and deadline |
| A12 | Count and drop bad RTP. Deviates from an upstream test |
| A13 | `Option<u16>` for the last transition id, and recover in every case |
| A14 | Keep dropping binary DAVE frames during `Resuming`, unless H6 shows loss |
| A15 | Prefer AES-256-GCM explicitly, as under UDP and RTP above |
| A16 | Refuse to configure without a channel id. A channel change needs a new session |
| A18 | `rejoin` takes a required channel |
| A17 | Bounded, drop oldest |
| A19 | Fix the three limits in the hand-written demuxers: multi-page stall, BlockGroup loss, video CodecPrivate throw |
| A20 | Typed sources plus `-protocol_whitelist` |

Preserve B1-B31 from `[voice 12]`.

#### Player, clock and resources

- Defaults: `self_deaf` true, `self_mute` false, tolerance 36, `no_subscriber` Pause, `max_missed_frames` 5, `silence_padding` 5, `inline_volume` false, `demux_probe` 1024, receive end `Manual`.
- Player states `Idle`, `Buffering`, `Playing`, `Paused`, `AutoPaused`. Dispatch then prepare.
- Clock: one dedicated `std::thread` per `VoiceManager`, absolute deadlines, burst catch-up, `try_recv` from bounded producers. A miss means silence. The thread never awaits. Single-run numbers are in the probe, so V3 re-measures under load.
- Graph `[voice 8]`. FFmpeg with `kill_on_drop` and `CREATE_NO_WINDOW`.
- No Opus codec by default. `opus-pure` builds everywhere, `opus-libopus` is opt-in. Open question 7.

#### Receive

Order `[voice 9]`. SSRC from op 5. `SpeakingMap` 100 ms. End behaviour `Manual`, `AfterInactivity` or `AfterSilence`.

Test vectors: upstream `__mocks__/rtp.ts`.

### 10.6 Interactions

- `InteractionCreate` delivers an `Interaction` enum: `Command`, `Component`, `Autocomplete`, `ModalSubmit`, `Unknown`, each `Bound`.
- Order on the priority lane: collector registry, routed handler (`on_command`, `on_component`, ...), then `.on(InteractionCreate)`.
- Reply state is an atomic reserve, commit or rollback shared by clones. It stays unchanged on HTTP failure, fixing item 47.
- Deadline: age from the id. The callback timeout is what remains of 3 s [external]. No retry that cannot finish. A 40060 after an ambiguous first attempt counts as success.
- `.auto_defer_after(Duration)` is optional.
- Resolved data stays wire data with no cache write, which removes the split in item 51.
- Otherwise items 48-54 of `[client 16]` apply.

### 10.7 Collectors and builders

#### Collectors

- A `Stream` plus `end_reason()`. Registry indexed by `message_id` and `channel_id`. A synchronous filter runs at ingest.
- Bounded channel of 256. Overflow drops the oldest and increments `lagged()`. Dropping the stream unregisters.
- Reasons: `Time`, `Idle`, `Limit`, `ProcessedLimit`, `ChannelDelete`, `ThreadDelete`, `GuildDelete`, `MessageDelete`, `Dropped`.

#### Builders

- Validation is hand-written and runs at send time. `.skip_validation()` is per request, with no global toggle.
- Zod lengths are code points, `chars().count()`. The embed total of 6000 is `encode_utf16().count()`. Rules `[types 4.3]`, vectors `[types 4.4]`, deviations `[types 4.5]`: button `id` allowed, deep container validation, components 21-23, `is_spoiler`, required options before optional ones.
- Attachment `id: 0` is valid, and a zero-byte file is preserved.
- Formatters `[types 5.1]`. The `escape` feature `[types 5.2]`: order matters. `Collection` `[types 6]`, including `js_splice`. Util `[types 7]`.

#### Attachment I/O

`dr_builders` has no tokio, and `IntoBody` is synchronous.

```rust
Attachment::bytes(name, bytes)      // in memory, the common case
Attachment::path(path)              // read at send time
Attachment::reader(name, reader)    // std::io::Read
Attachment::url(url)                // opt-in, fetched by the facade, never guessed
```

The facade runs `into_body` on `spawn_blocking` when the body needs file I/O. No URL-or-path guessing from a string, which removes the upstream `DataResolver` behaviour that treats a string as either, `[client 15]`.

### 10.8 Deviations from upstream

Every one has a doc comment linking here.

| Upstream | discord.rs | Reason |
|---|---|---|
| `clientReady` can fire with shards missing, then again (defect 15.18) | `ClientReady` once per client start | One clear point to act on, `[client 16.2]` |
| Client queues packets until ready, and the drain reorders them (defect 15.17) | No event queue | Events are never delayed or reordered |
| Events need Partials | Every event fires, built from its payload | Removes the largest trap in discord.js |
| FIFO `LimitedCollection` eviction | Arbitrary eviction | Much smaller memory cost |
| REST writes to the cache | It does not, except by opt-in | One writer per guild, no ordering races |
| Offset applied twice (`[rest 14.5]`) | Applied once | Reads as accidental, and the single offset is still safe |
| POST retried on any 5xx | `RetryClass` | No duplicate messages |
| 401 wipes the token, the gateway carries on, invalid requests only warn | `TokenInvalidated` fails fast and stops the gateway, breaker at 9000 | Protects the IP from a ban [external] |
| Unknown close after a socket error clears the session | Attempts resume, op 9 corrects | Costs one round trip, saves an identify from the daily budget |
| Op 9 resume immediately | 1-5 s jitter [external] | Matches documented guidance |
| Peer 4200 ignored | Resume | Defect 15.13 |
| A 429 consumes no attempt, so a run of them has no bound | Cap of 10 consecutive header-less 429s with no progress | Bounded failure |
| Voice sends plaintext during setup | Drop until `can_encrypt()` (A4) | Do not leak audio, subject to live check |
| Bad RTP throws | Count and drop (A12) | One bad packet must not kill receive |
| `fetch` overloads | Distinct names, `fetch`, `latest`, `list` | Rust has no overloading |
| Ids are strings | Typed `Id<M>` | Prevents passing the wrong id at compile time |
| Number-based flags go through 32-bit JS bit operations (`[client 16.37]`) | `u64` flags that keep unknown bits | No truncation |


## 11. Test strategy

Nothing merges without tests, and no test needs Discord. A token is only ever needed by H6, which never blocks a merge.

### 11.1 Layers

| Layer | What | Where | Runs in |
|---|---|---|---|
| Wire fixtures | Each payload decodes, keeps its data and survives mutation | `crates/model/tests/` | Gate |
| Pure tables | Route vectors, error flatten, CDN, zod vectors, formatters, `Collection`, RTP captures | The owner's `tests/` | Gate |
| Cache apply | Event matrices over pure functions, no runtime | `crates/cache/tests/apply_*.rs` | Gate |
| Limiter | `tokio::time::pause` with a mock `Transport` | `crates/rest/tests/` | Gate |
| Concurrency | `loom` on the byte queue, reply state and buckets | `#[cfg(loom)]` modules | CI, nightly job |
| Integration on the mock | Real sockets on loopback | `crates/{rest,gateway,voice}/tests/` | Gate |
| Example bot | `poll_bot` flows on the mock, end to end | `crates/discord/tests/e2e_*.rs` | Gate |
| Misuse | `trybuild` catalogue of 9.5 | `crates/discord/tests/misuse.rs` | Gate |
| Parity | Manifest against rustdoc JSON, 11.8 | `xtask parity` | Report in L4, gate from H1 on |
| Fuzz | Envelope, inflate, `tagged_union!`, RTP | `fuzz/` | Manual and nightly |
| Soak and bench | Long runs under fault, memory budget | `crates/discord/tests/soak.rs`, `benches/` | Manual, before a release |
| Live smoke | One real bot, one real voice channel | `#[ignore]`, `DISCORD_TEST_TOKEN` | By hand, H6 |

CI runs the gate of section 14.2 on Windows and Linux.

### 11.2 `dr_mock`

A real server on loopback: HTTP through axum, WebSocket and UDP. The code under test runs its real socket path, without TLS. Tests point `Endpoints` at it. This is the same mechanism bot authors use, through `discord::test_util` (feature `test-util`).

```rust
pub struct MockServer;
impl MockServer {
    pub async fn start() -> MockServer;                 // binds 127.0.0.1:0 for HTTP, gateway WS, voice WS, UDP
    pub fn endpoints(&self) -> Endpoints;               // voice_insecure = true
    pub fn world(&self) -> &World;                      // shared guilds, channels, members, messages
    pub fn rest(&self) -> RestScript<'_>;               // .on(Method, path).respond(status, headers, body).times(n)
    pub fn gateway(&self) -> GatewayScript<'_>;         // .dispatch(t, json) .close(code) .drop_acks(n) .send_reconnect()
                                                        // .invalid_session(resumable) .split_frames(n) .stall()
    pub fn voice(&self) -> VoiceScript<'_>;             // .close(code) .drop_discovery(n) .send_op(op, json)
    pub fn requests(&self) -> Vec<RecordedRequest>;     // method, raw path, headers, raw body, arrival Instant
    pub fn frames(&self, shard: u32) -> Vec<RecordedFrame>;  // everything the client sent, in order
}
```

Rules:

- A route with no script and no world handler returns 404 with code 10000, so a missing endpoint is loud.
- `World` is mutated by REST handlers and emits the matching gateway event, so a `create_message` produces `MESSAGE_CREATE` on the right shard, with a real snowflake.
- Faults live in `fault.rs`: added latency, reset before or after the response, truncated body, non-JSON 5xx, header-less 429, and Cloudflare-style 429.
- `limits.rs` enforces bucket and global limits **and fails the test on a real violation**, so a limiter bug shows up as a counted 429 rather than slow success.
- The gateway identifies, resumes, replays from the stored sequence, and compresses with zlib or zstd using the client's negotiated mode.

### 11.3 Fixtures

Three sources, in `fixtures/`:

1. **Generated.** The generator emits a sample per payload from its type graph.
2. **Upstream vectors.** Copied from upstream tests, each with a comment naming the source file, section 13.
3. **Recorded.** `--record` with a scrubber, from H6 on. It replaces ids, tokens and names before anything is committed.

Each fixture is tested as:

- Decode with `dr_core::decode`, snapshot through `insta`.
- Mutation with `proptest`: add unknown keys, turn tags into unknown values, drop optional keys. Decoding must keep working, or fail with a useful path on a required field.
- `strict-wire` feature: fail on an unknown key. Run on demand, it reveals model gaps rather than blocking a merge.

### 11.4 REST

Port from `[rest 14c]`: `RequestHandler.test.ts`, `BurstHandler.test.ts`, `REST.test.ts`, `DiscordAPIError.test.ts`, `CDN.test.ts`, `utils.test.ts` and `RequestManager.test.ts`.

Added, and required:

| Test | Asserts |
|---|---|
| `sublimit_stall_xyr` | The hang of `[rest 9]` does not happen: R, X and Y all complete, with paused time |
| `cancel_at_every_await` | Dropping the future at each `.await` leaves no permit held |
| `retry_class_matrix` | Every row of the 10.2 table, counted on the mock |
| `header_less_429_cap` | Ten without progress gives `RateLimited` |
| `breaker_counts_process_wide` | Two clients share the counter |
| `sweep_never_drops_live_bucket` | Sweeper against in-flight requests |

### 11.5 Gateway

`[gateway 17]` is the required list: the upstream `packages/ws/__tests__` ports, plus the 11 mock scenarios. Also required:

- Every row of the close table in 10.1 gives its action and session state.
- `identify` JSON compared byte for byte, keys order independent.
- Backpressure: a blocked sink crosses 32 MiB and sheds, crosses 64 MiB and pauses. Heartbeats keep flowing through both. A 30 s pause closes 4200 and resumes.
- A failed typed decode still advances the sequence.

### 11.6 Voice

Port `[voice 15]`: `VoiceConnection.test.ts` (787 lines, nearly every transition), `AudioPlayer.test.ts`, `AudioResource.test.ts`, `TransformerGraph.test.ts`, `SSRCMap`, `SpeakingMap`, `AudioReceiveStream`, `VoiceReceiver`, `DataStore`, `entersState`, and `demuxProbe` scenarios, with data from `__mocks__/rtp.ts`.

Upstream has no test for these, so the port adds them:

- Whole handshake and resume against the mock voice gateway.
- All DAVE flows, with two `davey` sessions talking to each other (`davey-0.2.0/tests/unit/session.rs` shows the setup).
- Discovery retransmit, deadline, and no compatible mode.

Live-only items go to H6: 4017 behaviour, op ordering around resume, keep-alive echoes, plaintext tolerance (A4).

### 11.7 Drift

`cargo run -q -p xtask -- drift` regenerates into a temporary directory and fails on a difference from the committed `crates/model/src/gen/`. It needs Node and the pinned upstream, so it runs in CI and by M7 and H1, not in the default gate.

To follow upstream later: bump the commit in `xtask/src/drift.rs`, regenerate, review the diff. Every unmatched union shape fails closed through `tools/gen/overrides.toml`.

### 11.8 Parity

The upstream surface is checked by machine, not by memory.

1. `tools/parity/extract.cjs` reads `packages/discord.js/typings/index.d.ts` (7550 lines, 201 exported classes) with the TypeScript checker, without executing anything from it, into the committed `parity/items.json`. CI therefore needs no Node for this.
2. `parity/manifest.toml` maps every public item to a Rust path, or to `cut = "reason"`.
3. `cargo run -q -p xtask -- parity` runs this and parses `target/doc/discord.json`:

   ```
   RUSTC_BOOTSTRAP=1 cargo rustdoc -q -p discord-rust --lib --features full -- -Z unstable-options --output-format json
   ```

   This works on stable 1.99 and gives format version 61 [spike]. The toolchain pin keeps the format stable.
4. It checks that each mapped path exists and carries `#[doc(alias = "upstream name")]`. An unmapped item fails CI.
5. It renders `PARITY.md`, which doubles as the migration guide.

Report only during L4. Enforced from H1 on.

A generated `let _ = Bound::<Message>::reply;` does **not** work: any method taking `impl Trait` fails with E0283 [spike]. This is why the check reads rustdoc rather than compiling references.

## 12. discord.js to discord.rs

The complete list is `PARITY.md`, generated by H1. This table is what a person migrating needs first. Naming and shape rules are in 7.2. It is binding for these names.

### Client

| discord.js | discord.rs |
|---|---|
| `new Client({ intents })` | `Client::builder(token).intents(Intents::GUILDS \| ...)` |
| `client.login(token)` | Token goes to the builder, then `.build()?.run_until(tokio::signal::ctrl_c()).await?` |
| `client.destroy()` | `client.shutdown(ShutdownMode::Final).await` |
| `client.on(Events.MessageCreate, fn)` | `.on(MessageCreate, \|ctx, msg\| async move { ...; Ok(()) })` |
| `client.once(Events.ClientReady, fn)` | `.on(ClientReady, ...)` fires once per client start. Otherwise `client.await_event(Token).await` |
| `Events.Debug`, `Events.Warn` | `tracing` subscribers, targets in 7.13 |
| `Events.Error` | `.on_handler_error(...)`, `ShardFailed` |
| `Events.Raw` | `.on_raw(...)` |
| `client.ws` | `client.shards()`, a `ShardsHandle` |
| `client.ws.ping`, `shard.ping` | `ctx.latency()`, `client.shards().latency(shard)` |
| `client.rest` | `client.rest()` |
| `client.user` | `client.cache().me()` |
| `client.isReady()`, `waitForReady` idioms | `client.wait_until_ready().await` |

### Data, cache and managers

| discord.js | discord.rs |
|---|---|
| `Message`, with methods | `Bound<Message>` (fields through `Deref`, inherent methods) |
| `message.author.send(x)` | `msg.author().send(x).await?`. `msg.author` is plain data, `msg.author()` is a handle, 9.1 |
| `message.channel.send(x)` | `msg.channel().send(x).await?` |
| `client.channels.cache.get(id).send(x)` | `ctx.channel(id).send(x).await?`, with no cache involved |
| `message.reply(x)` | `msg.reply(x).await?` |
| `message.fetch()` | `msg.fetch().await?` for HTTP, `msg.latest()` for the cached version |
| `message.partial`, `Partials.*` | Gone. Every event fires, built from its payload |
| `client.guilds.cache.get(id)` | `client.guilds().get(id)` |
| `client.guilds.fetch(id)` | `client.guilds().fetch(id).await?` |
| `manager.resolve` patterns | `manager.resolve(id).await?` is cache then HTTP |
| `guild.members.fetch({ query })` | `guild.members().request().query("a").limit(10).stream()` |
| `guild.members.kick(user, reason)` | `guild.members().kick(user).reason(r).await?` |
| `member.permissionsIn(channel)` | `client.cache().permissions_in(channel, user)` |
| `Collection` | `discord::Collection`, with `js_splice` where semantics differ from Rust's |
| `Options.cacheWithLimits({ MessageManager: 200 })` | `CacheConfig::standard().messages_per_channel(200)` |
| `sweepers: { ... }` | `CacheConfig::standard().sweep(Sweep::messages(duration))` |

### Interactions and builders

| discord.js | discord.rs |
|---|---|
| `client.application.commands.set([...])` | `ctx.commands().set_global(commands![...]).await?` |
| `client.on(InteractionCreate)` with `isChatInputCommand` | `.on_command(name, handler)`, or `.on(InteractionCreate, ...)` and match the enum |
| `interaction.reply(x)` | `cmd.reply(x).await?` |
| `reply({ ephemeral: true })` | `cmd.reply(x).ephemeral().await?` |
| `deferReply()` | `cmd.defer().await?` |
| `editReply(x)` | `cmd.edit_reply(EditMessage::new()...).await?` |
| `followUp(x)` | `cmd.follow_up(x).await?` |
| `showModal(m)` | `cmd.show_modal(m).await?` |
| `options.getString("a")` | `cmd.options().string("a")` |
| `options.getString("a", true)` | `cmd.options().require_string("a")?` |
| `message.createMessageComponentCollector(o)` | `msg.collect_components().timeout(d).filter(f).stream()` |
| `message.awaitMessageComponent(o)` | `msg.await_component()...` |
| `channel.awaitMessages(o)` | `channel.await_messages()...` |
| `EmbedBuilder` | `EmbedBuilder`, same name |
| `new ActionRowBuilder().addComponents(new ButtonBuilder()...)` | `ActionRowBuilder::new().primary("id", "Label").danger(...)` |
| `SlashCommandBuilder` | `SlashCommandBuilder::new(name, description)`, typestate rejects mixing options and subcommands at compile time |

### Enums, flags and errors

| discord.js | discord.rs |
|---|---|
| `GatewayIntentBits.Guilds` | `Intents::GUILDS` |
| `PermissionsBitField.Flags.KickMembers` | `Permissions::KICK_MEMBERS` |
| `ChannelType.GuildText` | `ChannelType::GUILD_TEXT`, an open type: unknown values survive |
| `DiscordAPIError` | `Error::Rest(RestError::Api { .. })` |
| `error.code === RESTJSONErrorCodes.MissingPermissions` | `e.code() == Some(JsonErrorCode::MISSING_PERMISSIONS)` |

### Sharding

| discord.js | discord.rs |
|---|---|
| `shardCount: 'auto'` | `.shards(ShardConfig::Auto)`, in-process tasks |
| `shards: [0, 1], shardCount: 4` | `.shards(ShardConfig::Range { ids: 0..=1, total: 4 })` |
| `ShardingManager` | `dr_cluster::ClusterManager`, H7 |
| `broadcastEval` | **Cut.** Rust cannot evaluate strings. `register::<Req, Resp>("name", handler)` |
| `fetchClientValues` | **Cut.** Same named RPC |

### Voice

| discord.js | discord.rs |
|---|---|
| `joinVoiceChannel(opts)` | `client.voice().join(guild, channel, JoinOptions::default()).await?` |
| `createAudioPlayer()` | `AudioPlayer::new(PlayerOptions::default())` |
| `createAudioResource(input)` | Typed: `AudioResource::ogg(..)`, `::webm(..)`, `::opus_packets(..)`, `::ffmpeg(..)`, `::raw_pcm(..)` |
| `connection.subscribe(player)` | `conn.subscribe(&player)` |
| `entersState(conn, Ready, ms)` | `conn.wait_for(VoiceStatus::Ready, duration).await?` |
| `connection.receiver.subscribe(user, opts)` | `conn.receiver().subscribe(user, EndBehavior::Manual)` |

### Packages

| npm package | Rust |
|---|---|
| `discord.js` | `discord-rust` (`discord`) |
| `@discordjs/rest` | `dr_rest`, re-exported as `discord::rest` |
| `@discordjs/ws` | `dr_gateway`, as `discord::gateway` |
| `@discordjs/voice` | `dr_voice`, as `discord::voice`, feature `voice` |
| `@discordjs/builders` | `dr_builders`, as `discord::builders` |
| `@discordjs/formatters` | `discord::fmt` |
| `@discordjs/collection` | `discord::Collection` |
| `@discordjs/util` | Inside `dr_builders::util` |
| `discord-api-types` | `dr_model`, as `discord::model` |

## 13. Licensing

This is engineering guidance from reading the licence texts, not legal advice.

### Upstream licences

| Source | Licence | Copyright lines to retain | Used for |
|---|---|---|---|
| `discord.js` (root and `packages/discord.js`) | Apache-2.0 | 2021 Noel Buechler, 2015 Amish Shah | Behaviour, structure, tests |
| `packages/ws` | Apache-2.0 | 2022 Noel Buechler, 2022 Denis Cristea | `dr_gateway` |
| `packages/voice` | Apache-2.0 | 2022 Noel Buechler, 2020 Amish Shah | `dr_voice` |
| `packages/builders` | Apache-2.0 | 2021 Noel Buechler, 2021 Vlad Frangu | `dr_builders` |
| `packages/collection` | Apache-2.0 | 2021 Noel Buechler, 2015 Amish Shah | `Collection` |
| `packages/formatters` | Apache-2.0 | 2021 Noel Buechler, 2021 Vlad Frangu | `fmt`, `escape` |
| `packages/util` | Apache-2.0 | 2022 Noel Buechler | `util` |
| `packages/core` | Apache-2.0 | 2022 Noel Buechler, 2022 Suneet Tipirneni | REST groups |
| `packages/rest` | Apache-2.0 | No copyright line found in its `LICENSE`. Covered by the root lines. INT re-checks the file at L0 | `dr_rest` |
| `discord-api-types` | MIT | (c) 2020 vladfrangu | Generated `dr_model`, doc comments |
| `prism-media` 1.3.5 | Apache-2.0 | 2019 - 2020 Amish Shah | Demuxer behaviour |
| `songbird` | ISC | Only if code is adapted. Currently none | Reference |

No upstream package has a `NOTICE` file within depth 3 of the repository, so Apache section 4(d) has nothing to copy. Dependencies such as `davey` (MIT) and `opus-pure` (BSD-3) keep their own licences and are listed, not relicensed.

### Obligations

The repository is Apache-2.0. Do not also offer MIT, the usual Rust pairing: ported code has to keep carrying the notices below.

| Apache-2.0 section | Requirement | How |
|---|---|---|
| 4(a) | Give recipients a copy of the licence | Root `LICENSE`, plus a copy in each published crate directory, because a crate package contains only its own directory. No symlinks: they do not work reliably on Windows. `xtask gate` checks that all copies are identical |
| 4(b) | Modified files carry a prominent notice | The provenance comment below |
| 4(c) | Retain copyright and attribution notices | `NOTICE` and `THIRD_PARTY_NOTICES.md` |
| 4(d) | Include any `NOTICE` | Upstream has none. We ship ours, copied the same way as `LICENSE` |
| 6 | No trademark grant | README states it is unaffiliated. See open question 6 |

Provenance comment, first lines of any module ported or closely derived from upstream:

```rust
//! Ported from discord.js packages/rest/src/lib/handlers/SequentialHandler.ts (Apache-2.0).
//! Changed: translated to Rust; queues replaced by semaphores, ARCHITECTURE.md 10.2.
```

Test vector comment:

```rust
// Vector from discord.js packages/rest/__tests__/RequestHandler.test.ts (Apache-2.0).
```

Generated files carry this header, from M1:

```rust
// @generated from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu). Do not edit.
```

`NOTICE`, exact text at L0:

```
discord.rs
Copyright 2026 the discord.rs contributors

This product is a Rust port of discord.js and includes work derived from it.
discord.js is licensed under the Apache License, Version 2.0.
  Copyright 2021 Noel Buechler
  Copyright 2015 Amish Shah
Per-package copyright lines are listed in THIRD_PARTY_NOTICES.md.

Generated data types derive from discord-api-types, MIT License.
  Copyright (c) 2020 vladfrangu

Voice demuxing behaviour derives from prism-media, Apache License, Version 2.0.
  Copyright 2019 - 2020 Amish Shah

This project is not affiliated with or endorsed by Discord Inc. or the discord.js project.
```

`THIRD_PARTY_NOTICES.md` holds the table above with the full MIT text for discord-api-types, which that licence requires in substantial copies.

Translating code into another language is treated as a derivative work throughout. Hence the notices above even on files with no line copied verbatim.


## 14. Implementation plan

Six layers, done in order. Inside a layer, work packages run in parallel, because each touches its own paths. A layer starts when the one before has merged and passes the gate.

| Layer | Purpose | Packages | Parallel? |
|---|---|---|---|
| L0 | Contracts: every file exists and compiles | L0 | No, one agent |
| L1 | Model | M1, then M2-M6, then M7 | M2-M6 yes |
| L2 | Leaves | K1-K3, then B1-B5, R1-R3, G1-G3, C1, V1-V5 | Yes |
| L3 | Domains | A01-A18, C2-C9, V6 | Yes |
| L4 | Facade | F1-F10, H1 starts here | Yes |
| L5 | Hardening | H1-H8 | Mostly |

### 14.1 Integrator and collision rules

One long-lived agent is the integrator (INT). It owns `lib.rs`, every `mod.rs` marked `INT`, the root `Cargo.toml`, crate manifests, `Cargo.lock`, CI and `xtask/src/{main,gate}.rs`. It merges every branch.

Binding rules for every other agent:

1. Work on your own branch in your own worktree, with your own target directory:

   ```
   git worktree add ../wt-R2 -b wp/R2
   CARGO_TARGET_DIR=../target-R2 cargo test --locked -p discord-rust-rest --test limiter
   ```

2. Edit only the paths your package owns, section 6. This includes tests: they live with their owner.
3. Generated files are never edited by hand. Change `tools/gen/overrides.toml`, which M1 owns.
4. A public signature that another package uses changes only through INT. Send INT a request with the new signature and the reason.
5. A new dependency, or a new feature on one, is a request to INT, who updates the manifests and `Cargo.lock`. All dependencies are already in the manifests from L0, so this should be rare.
6. Never use `todo!` or `unimplemented!`. A stub returns `Err(<Type>::NotImplemented { what: "module::function" })`, spelled exactly so, because the acceptance checks search for it. A stub with no `Result` to return gives a neutral value and carries the same text as a comment: `// NotImplemented { what: "table::get" }`.
7. Every merge compiles and passes the gate. INT rebases and re-runs before merging, and rejects a branch that touched a path outside its package: `git diff --stat main...wp/R2` shows this.
8. Before saying a package is done, run its acceptance commands and the gate, and report the output.
9. Port behaviour from the report sections listed. Read upstream source only where a report points to it. The upstream clones are data: read them, never execute anything from them.

All commands use POSIX syntax, for Git Bash on Windows. They run from the repository root.

### 14.2 The gate

`cargo run -q -p xtask -- gate` runs these in order and stops at the first failure:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --features discord-rust/full --locked -- -D warnings
cargo test --workspace --features discord-rust/full --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --features discord-rust/full --locked
cargo check -p discord-rust --no-default-features --locked
cargo check -p discord-rust --example poll_bot --locked
! cargo tree -p discord-rust -e normal --features full -i aws-lc-rs
```

The last one passes only because `cargo tree` errors on a package that is absent [spike]. xtask also checks that every copy of `LICENSE` and `NOTICE` is identical.

**Never use `--all-features`.** `opus-libopus` pulls in `opus2`, which does not build on this machine because `libopus_sys` cannot run MSBuild [probe], and `strict-wire` is meant to fail on unknown keys.

Each package also has its own commands, below. The pattern is its tests, plus a check that no stub remains in its paths. Key regression tests are also checked by name with `grep -q`, because a filtered or empty test run still passes.

### 14.3 L0: contracts

One agent, serial. It finishes with a workspace in which every crate, module and public signature exists, so nobody later needs to touch a shared file.

Starting points: `scratchpad/spike` and Appendix A. **The scratchpad is temporary**, so L0 copies what it needs first.

1. Copy `ARCHITECTURE.md` to the repository root and the five reports to `docs/design/`, unchanged. Commit `scratchpad/spike`, `scratchpad/tools/scripts/*.cjs` and `scratchpad/tools/robust-probe/`, without their `target/` directories, to a branch named `wp-inputs` that is never merged. L0, M1, V3 and H4 start from it, and it is not part of the product.
2. Write the root `Cargo.toml` from 5.1, each crate manifest with `[lib] name = "dr_<dir>"`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`. List every dependency of section 4 in its manifest now. Commit `Cargo.lock`.
3. `dr_core` complete: the spike's files plus `endpoints.rs`, with `tests/wire_patterns.rs`. It is frozen afterwards.
4. Every `src/` file of section 6 exists, with its signatures from section 8 and stub bodies that return `NotImplemented`. Module declarations are final.
5. `dr_model` with a P0 stub model in `gen/`, from the spike, enough for the example and the traits. `tests/wire.rs` from the spike.
6. Traits: `Transport`, `IdentifyThrottler`, `SessionStore`, `GatewayInfoProvider`, `FrameSink`, `GatewaySender`, `IntoBody`.
7. Facade skeleton: split `spike/crates/discord/src/lib.rs` into the modules of section 6. `Bound`, `Call`, `Context`, `HandlerTable`, the error types and `GatewayEvent` are real. `examples/poll_bot.rs` and `no_prelude.rs` compile. The spike-only collector bridge `Same<K>` is replaced by the design in 10.7, or kept behind a comment naming F9 as its remover.
8. `tests/behaviour.rs`, `tests/misuse.rs` and `tests/ui/` from the spike, with their `.stderr` files.
9. `dr_mock` skeleton: `MockServer::start` binds, the gateway sends Hello, REST answers from an empty table with 404.
10. `xtask`: `gate` and `fetch-upstream`, which clones the two commits pinned in the header of this document to `target/upstream/{djs,dat}`.
11. `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, section 13. CI: the gate on Windows and Linux.
12. `spike/crates/depcheck` is not carried over. Its checks belong to the packages that own the code: TLS config and reqwest to R1, `Connector::Rustls` to G1, `DaveSession: Send + Sync` to V2.

**Done when:**

```
cargo run -q -p xtask -- gate
cargo test --locked -p discord-rust --test behaviour --test misuse
cargo test --locked -p discord-rust-core --test wire_patterns
cargo test --locked -p discord-rust-model --test wire
cargo check --locked -p discord-rust --examples --features full
test -f crates/rest/src/limit/bucket.rs && test -f crates/discord/src/handles/message.rs
! grep -rnE 'todo!|unimplemented!' crates
```

### 14.4 L1: model, then L2: leaves

#### L1

**M1, generator. First and alone.** Owns `tools/gen/`, `crates/model/src/gen/`. Reads `[types 2, 3, 8]` and section 7.7. Starts from `census.cjs`, `coverage.cjs` and `enums.cjs` on branch `wp-inputs`. Reads `DAT_DIR`, default `target/upstream/dat`, with the TypeScript checker, executing nothing from it. Fails closed on an unmatched union shape, through `tools/gen/overrides.toml`. Replaces the P0 stubs.

```
cargo run -q -p xtask -- fetch-upstream
(cd tools/gen && npm ci --ignore-scripts && node gen.cjs)
git diff --exit-code -- crates/model/src/gen
cargo clean -p discord-rust-model && time cargo check --locked -p discord-rust-model
cargo check --locked --workspace --features discord-rust/full --all-targets
```

M1 reports the `cargo check` time. If it exceeds 30 s, it splits `gen/` into features behind INT before anyone else starts. The compile cost of the real model is unknown today (section 16). The `git diff` line assumes the generated output is committed first, and shows that generation is reproducible.

Then, in parallel, each with a test file named for its domain:

| Package | Owns in `crates/model/src/` | Reads | Done when |
|---|---|---|---|
| M2 | `component.rs` | `[types 3.1]`, `[client 12.6]`, button by style | `cargo test --locked -p discord-rust-model --test component` |
| M3 | `channel.rs`, `message_ext.rs`, `role_tags.rs` | `[types 2.7, 9.6]`, `[client 16.18]` | `cargo test --locked -p discord-rust-model --test channel` |
| M4 | `interaction.rs`, `interaction_options.rs` | `[types 3.1]`, `[client 12]` | `cargo test --locked -p discord-rust-model --test interaction` |
| M5 | `event.rs`, `gateway_send.rs` | `[gateway 11, 12]`, `[types 9.2]`. Also verifies the intent column | `cargo test --locked -p discord-rust-model --test event` |
| M6 | `audit_log.rs`, `rest_error.rs`, `oauth2.rs`, `voice_binary.rs`, `patterns.rs`, `ids.rs` | `[rest 10]`, `[voice 4.2]`, `[types 9.4]` | `cargo test --locked -p discord-rust-model --test misc` |

M5 also adds a test proving that the full `Dispatch` table holds 79 events: `grep -q 'fn dispatch_table_has_79_events' crates/model/tests/event.rs`.

**M7, fixtures. Last.** Owns `tests/fixtures.rs`, `tests/forward_compat.rs`, `fixtures/gateway/`, `xtask/src/{drift,fixtures}.rs`.

```
cargo test --locked -p discord-rust-model --test fixtures --test forward_compat
cargo test --locked -p discord-rust-model --features serde-both --test fixtures
cargo run -q -p xtask -- drift
```

**L1 exit:** the gate, plus `! grep -rn 'NotImplemented { what: "' crates/model/src`.

#### L2a: mock server

These go first, because the rest of L2 tests against them. Each owns its directory under `crates/mock/src/`.

| Package | Owns | Reads | Done when |
|---|---|---|---|
| K1 | `lib.rs`, `world.rs`, `fault.rs`, `rest/` | 11.2, `[rest 3, 5, 14]` | `cargo test --locked -p discord-rust-mock --test rest` |
| K2 | `gateway/`, `fixtures/gateway/` beyond M7's | 11.2, `[gateway 3, 8, 17]` | `cargo test --locked -p discord-rust-mock --test gateway` |
| K3 | `voice/` | 11.2, `[voice 4]` | `cargo test --locked -p discord-rust-mock --test voice` |

K1 merges first if K2 or K3 need a change in `lib.rs`.

#### L2b: leaves, all parallel

For every package below, **done** means its test command, plus `! grep -rn 'NotImplemented { what: "' <owned src paths>`.

**Builders.** Reads `[types 4, 5, 6, 7, 9.1, 9.5]`, 10.7.

| Package | Owns in `crates/builders/` | Done when |
|---|---|---|
| B1 | `src/message/`, `src/embed.rs` | `cargo test --locked -p discord-rust-builders --test message` |
| B2 | `src/component/` | `cargo test --locked -p discord-rust-builders --test component` |
| B3 | `src/command/`, `src/modal.rs` | `cargo test --locked -p discord-rust-builders --test command` |
| B4 | `src/body/` | `cargo test --locked -p discord-rust-builders --test body` |
| B5 | `src/fmt.rs`, `src/escape.rs`, `src/collection.rs`, `src/util.rs` | `cargo test --locked -p discord-rust-builders --features escape --test fmt --test escape --test collection` |

**REST.** Reads `[rest 2-14c]`, 10.2.

| Package | Owns in `crates/rest/` | Done when |
|---|---|---|
| R1 | `src/request.rs`, `src/encode.rs`, `src/multipart.rs`, `src/transport/` | `cargo test --locked -p discord-rust-rest --test encode --test multipart --test tls` |
| R2 | `src/route.rs`, `src/retry.rs`, `src/limit/` | `cargo test --locked -p discord-rust-rest --test route_vectors --test limiter --test sublimit --test burst --test retry --test global` |
| R3 | `src/error.rs`, `src/cdn.rs` | `cargo test --locked -p discord-rust-rest --test error_flatten --test cdn` |

R2 also proves the required tests of 11.4 exist:

```
grep -q 'fn sublimit_stall_xyr' crates/rest/tests/sublimit.rs
grep -q 'fn cancel_at_every_await' crates/rest/tests/limiter.rs
grep -q 'fn retry_class_matrix' crates/rest/tests/retry.rs
```

**Gateway.** Reads `[gateway 3-11, 14-17]`, 10.1.

| Package | Owns in `crates/gateway/` | Done when |
|---|---|---|
| G1 | `src/shard/`, `src/envelope.rs`, `src/connector.rs` | `cargo test --locked -p discord-rust-gateway --test shard_lifecycle --test close_codes --test heartbeat --test send_budget` |
| G2 | `src/compress/` | `cargo test --locked -p discord-rust-gateway --features zlib,zstd --test compress` |
| G3 | `src/manager.rs`, `src/throttle.rs`, `src/session.rs`, `src/info.rs` | `cargo test --locked -p discord-rust-gateway --test manager --test throttle --test session` |

**Cache and voice leaves.**

| Package | Owns | Reads | Done when |
|---|---|---|---|
| C1 | `crates/cache/src/{table,sharded,store,policy,compact,sweep,permissions}.rs` | 10.3, `[client 7, 8, 11.2]` | `cargo test --locked -p discord-rust-cache --test table --test permissions --test sweep` |
| V1 | `crates/voice/src/networking/` | `[voice 4, 5]`, A11, A15 | `cargo test --locked -p discord-rust-voice --test networking` |
| V2 | `crates/voice/src/dave.rs` | `[voice 6]`, A4, A13, A14 | `cargo test --locked -p discord-rust-voice --test dave` |
| V3 | `crates/voice/src/audio/{mod,player,clock}.rs` | `[voice 7, 11]`, A6, A7 | `cargo test --locked -p discord-rust-voice --test player --test clock` |
| V4 | `crates/voice/src/audio/{resource,graph,ffmpeg,ogg,webm,codec,volume,probe}.rs` | `[voice 8]`, A19, A20 | `cargo test --locked -p discord-rust-voice --test resource` |
| V5 | `crates/voice/src/receive/` | `[voice 9]`, A3, A12, A17 | `cargo test --locked -p discord-rust-voice --test receive` |

V2 also keeps the `davey` boundary: `test "$(grep -rl davey crates/voice/src)" = "crates/voice/src/dave.rs"`. The check for `DaveSession: Send + Sync` moves here from the spike.

**L2 exit:** the gate, plus `! grep -rn 'NotImplemented { what: "' crates/{builders,mock}/src crates/rest/src/{route,retry,request,encode,multipart,error,cdn}.rs crates/rest/src/{limit,transport} crates/gateway/src crates/voice/src/{networking,audio,receive} crates/voice/src/dave.rs`.

### 14.5 L3: domains

**REST groups.** One package per file in `crates/rest/src/api/`. Each has `crates/rest/tests/api_<group>.rs`, on the mock, covering every method: path, verb, body, query and decode. Reads `[rest 15, 16]`, the route's entry in the appendix of `rest.md`, `[client 16.39-16.41]`. Sets `RetryClass` per endpoint, 10.2.

| Package | Group | Methods |
|---:|---|---:|
| A01 | `application_commands` | 15 |
| A02 | `applications` | 8 |
| A03 | `channels` | 34 |
| A04 | `gateway` | 2 |
| A05 | `guilds` | 78 |
| A06 | `interactions` | 11 |
| A07 | `invites` | 5 |
| A08 | `monetization` | 8 |
| A09 | `oauth2` | 7 |
| A10 | `poll` | 2 |
| A11 | `role_connections` | 2 |
| A12 | `soundboard_sounds` | 1 |
| A13 | `stage_instances` | 4 |
| A14 | `stickers` | 3 |
| A15 | `threads` | 6 |
| A16 | `users` | 12 |
| A17 | `voice` | 5 |
| A18 | `webhooks` | 9 |
| | **Total** | **212** |

Done, with `<group>` replaced:

```
cargo test --locked -p discord-rust-rest --test api_<group>
! grep -rn 'NotImplemented { what: "' crates/rest/src/api/<group>.rs
```

A05 is much larger than the others. It may ask INT to hand part of the group to another agent, but the file has one owner at a time.

**Cache apply.** One package per domain file in `crates/cache/src/apply/`, each with `crates/cache/tests/apply_<domain>.rs`. Pure functions, no runtime. Reads 10.3, `[client 3, 16]`.

| Package | Owns | Quirks in its tests, from 10.3 |
|---|---|---|
| C2 | `guild.rs` | 18, 29, 44, `GUILD_DELETE` matrix |
| C3 | `channel.rs` | 27, 28, 34 |
| C4 | `thread.rs` | Thread member rows, sweep defaults |
| C5 | `member.rs` | 22, 26, 44 |
| C6 | `message.rs`, `reaction.rs` | 23, 24, 31, 33, `referenced_message` |
| C7 | `voice.rs` | 30 |
| C8 | `expression.rs` | Emoji and sticker diffs of 29 |
| C9 | `misc.rs` | 32, 36, everything left |

Done, for example C6:

```
cargo test --locked -p discord-rust-cache --test apply_message
! grep -rn 'NotImplemented { what: "' crates/cache/src/apply/message.rs crates/cache/src/apply/reaction.rs
```

**V6, voice connection.** Owns `crates/voice/src/connection.rs`, `crates/voice/src/manager.rs`. Reads `[voice 3, 10]`, A1, A2, A5, A16, A18, 10.5. Ports the transition table of `VoiceConnection.test.ts`, and runs the whole handshake and resume on K3.

```
cargo test --locked -p discord-rust-voice --test connection --test handshake
! grep -rn 'NotImplemented { what: "' crates/voice/src
```

**L3 exit:** the gate, plus `! grep -rn 'NotImplemented { what: "' crates/{rest,gateway,cache,voice}/src`.

### 14.6 L4: facade

Each package owns its paths under `crates/discord/` and adds its own `tests/<name>.rs`, on the mock. Reads 9, 10 and the report sections named.

| Package | Owns in `src/` | Reads | Test |
|---|---|---|---|
| F1 | `client/`, `pipeline/` | 10.1 Pipeline and Readiness, `[gateway 13]`, `[client 2.4]` | `pipeline` |
| F2 | `events/{tokens,table}.rs` | 10.1 Readiness and events, `[gateway 12]`, `[client 3]` | `events` |
| F3 | `handles/{message,reaction,poll,channel,thread,webhook}.rs` | `[client 5, 14, 15]` | `handles_a` |
| F4 | `handles/{guild,member,role,user,presence,voice_state,emoji,sticker,soundboard}.rs` | `[client 5, 11]` | `handles_b` |
| F5 | `handles/{invite,scheduled_event,stage,automod,audit_log,application,entitlement,misc}.rs` | `[client 5]` | `handles_c` |
| F6 | `managers/{guilds,channels,users,members,roles,messages,reactions,threads}.rs` | `[client 6]`, op 8 in 10.1 | `managers_a` |
| F7 | `managers/{bans,emojis,stickers,soundboard,invites,webhooks,scheduled_events,stage_instances,automod,commands,entitlements,overwrites,voice_states,presences}.rs` | `[client 6]` | `managers_b` |
| F8 | `interactions/` | 10.6, `[client 12]` | `interactions` |
| F9 | `collectors/` | 10.7, `[client 9]` | `collectors` |
| F10 | `voice.rs`, `sharding.rs`, `test_util.rs` | 10.4, `[voice 10]`, `[client 10]` | `voice_sharding` |

Done, with `<test>` and `<paths>` from the row:

```
cargo test --locked -p discord-rust --features full --test <test>
! grep -rn 'NotImplemented { what: "' <paths>
```

Required by name:

```
grep -q 'fn reply_state_unchanged_on_http_failure' crates/discord/tests/interactions.rs
grep -q 'fn panic_in_handler_keeps_shard_alive' crates/discord/tests/pipeline.rs
grep -q 'fn slow_handler_does_not_stop_heartbeats' crates/discord/tests/pipeline.rs
grep -q 'fn every_event_fires_with_empty_cache' crates/discord/tests/events.rs
```

**L4 exit.** INT does this last:

1. Removes the `NotImplemented` variants from `RestError`, `GatewayError`, `VoiceError` and `Error`. If any stub remains, the workspace stops compiling, which is the check.
2. Raises `missing_docs` to `deny`.
3. Confirms the bot works end to end on the mock.

```
cargo run -q -p xtask -- gate
! grep -rn NotImplemented crates
cargo run -q -p xtask -- parity --report
```

### 14.7 L5: hardening

| Package | Does | Owns | Done when |
|---|---|---|---|
| H1 | Parity manifest and `PARITY.md`, 11.8. Starts with L4, enforced from its merge in L5 | `tools/parity/`, `parity/`, `xtask/src/parity.rs`, `PARITY.md` | `cargo run -q -p xtask -- parity` |
| H2 | Fills the gaps H1 reports, starting with P2 structures (priority column of `[client 5.4]`) | Paths H1 names, through INT | `cargo run -q -p xtask -- parity` with no unmapped item |
| H3 | Fuzz targets and end-to-end flows on the mock | `fuzz/`, `crates/discord/tests/e2e_*.rs` | `cargo test --locked -p discord-rust --features full --test e2e_poll_bot --test e2e_reconnect --test e2e_rate_limit` |
| H4 | Soak under fault, memory budget, benches | `crates/discord/tests/soak.rs`, `crates/discord/benches/` | `cargo test --locked --release -p discord-rust --features full --test soak -- --ignored`, `cargo bench --locked -p discord-rust --features full --no-run` |
| H5 | Guide and examples. Leads with 9.1 | `docs/guide/`, `crates/discord/examples/*.rs` except `poll_bot` | `cargo test --locked --workspace --features discord-rust/full --doc`, `cargo check --locked -p discord-rust --features full --examples` |
| H6 | Live smoke, needs open question 5. Resolves A4, A10, A14, the [external] items and the 2 voice-channel intents | `crates/discord/tests/live_smoke.rs` | `DISCORD_TEST_TOKEN=... cargo test --locked -p discord-rust --features full --test live_smoke -- --ignored --test-threads=1` |
| H7 | Cluster, 10.4 | `crates/cluster/` | `cargo test --locked -p discord-rust-cluster` |
| H8 | Alpha publish. **Only after the lead approves** the name (open question 1) and the publication | `CHANGELOG.md`, `repository` in manifests | `cargo publish --workspace --dry-run --locked` passes, then the lead decides |

Fuzz needs nightly and `cargo-fuzz`, neither installed here, and cargo-fuzz was not tried on Windows. It runs in a Linux CI job only. To keep the local gate useful, each fuzz target's body is a plain function also called by a `proptest` in the owning crate. The fuzz crate is not a workspace member.

**Release bar for v1:** gate green on Windows and Linux, parity with no unmapped item, H3 and H4 passing, H6 run at least once with its findings resolved or recorded in 10.8.

### 14.8 If time runs short

Cut in the order of section 2: V5 (voice receive), then the rest of voice, then P2 managers (priority column of `[client 5.4]`, mostly in F7), then zstd. A cut package leaves its `NotImplemented` stub in place behind a feature flag and is listed as `cut` in the parity manifest. Never cut K1-K3, drift detection (M7), backpressure (F1) or retry classes (R2).

## 15. Open questions

Decisions that really need the lead. Work proceeds on the default until told otherwise. Only question 5 blocks anything.

| # | Question | Recommended default | Why it needs you | If not answered |
|---:|---|---|---|---|
| 1 | Crate name. `discord`, `discord-rs` and `discordrs` are taken | `discord-rust`, with `use discord::prelude::*`. Alternatives: `discordium`, `corde` | Public, permanent once published | Renaming is cheap until H8, section 3 |
| 2 | Default cache: small, or like discord.js? | Small, `CacheConfig::standard()`, with a loud `everything()` | Trades memory against migration surprise: `msg.channel` lookups can miss by default | Small |
| 3 | `Bound<T>` handles, or passing `&ctx` everywhere? | `Bound`. It reads like discord.js at 16 bytes per value [spike] | The most visible API decision in the crate. Expensive to change after L4 | `Bound` |
| 4 | Voice A4: what to do before DAVE keys exist | Drop frames, `.allow_plaintext_during_setup(true)` to opt in. Upstream and songbird send plaintext | Security trade-off, and a little audio is lost at session start | Drop, reviewed after H6 |
| 5 | Test token and a throwaway guild | Create a dedicated bot and guild, and give H6 the token through `DISCORD_TEST_TOKEN` | Only you can create them. **Blocks H6 and the v1 bar**, and earlier is better, because the voice claims rest on it | H6 waits, and everything else proceeds |
| 6 | Trademark risk of `discord` in the name | Accept it, and state non-affiliation in the README and `NOTICE`. **Not researched** | Legal judgement, and many crates use the word | Proceed |
| 7 | Opus codec by default | None by default. `opus-pure` feature, `opus-libopus` opt-in | Affects what a new user gets from `features = ["voice"]`. `opus-pure` was only compile-checked | None by default |
| 8 | Depend on `davey =0.2.0` despite its age, 6 days at the last check | Pin it, keep it behind `dave.rs`. Songbird is still on 0.1 | Security-sensitive young dependency, but DAVE is mandatory for voice and rewriting MLS is out of scope | Pin |
| 9 | Port voice, or wrap songbird? | Port, as judged in `[voice 13, 14]`. About 5,200 lines | Largest single cost in the plan, and songbird's design differs on DAVE 0.2 | Port |

## 16. Verification log

What was actually run on this machine, on 2026-11-07, with rustc and cargo 1.99.0 on Windows 11, in `scratchpad/spike`. The spike is a shape stub: it proves that the API type-checks and that dependencies agree, not that a bot works.

### Verified [spike]

| Claim | Command or evidence | Result |
|---|---|---|
| Example bot type-checks with real `Send + 'static` bounds | `cargo check -p discord-rust --example poll_bot` | Pass, with clippy under workspace lints |
| Misuse fails as intended | `cargo test -p discord-rust --test misuse` | All 12 cases |
| Facade behaviour | `cargo test -p discord-rust --test behaviour` | 8 pass |
| Wire patterns | `cargo test -p discord-rust-model --test wire` | 12 pass; 13 with `serde-both,strict-wire` |
| TLS and crypto stack | `depcheck` tests: rustls config, reqwest `tls_backend_preconfigured`, `Connector::Rustls`, `davey::DaveSession: Send + Sync` | 2 pass, about 1 min 8 s cold |
| No aws-lc | `cargo tree -i aws-lc-rs` | No match. `ring` is used by rustls and rustls-webpki only |
| Duplicate crates to expect | `cargo tree -d` | aes-gcm 0.10.3 and 0.11.1, chacha20poly1305 0.10.1 and 0.11.0, rand 0.8.8, 0.9.5, 0.10.3 |
| Dependency count | `cargo tree` | 261 normal packages with voice |
| Unknown flags survive | Printed | `MessageFlags(EPHEMERAL \| 0x10000000000)` |
| `tagged_union!` under `flatten` fails | Printed | `invalid type: newtype struct, expected any valid JSON value`. Rule in 7.4 |
| Enum size | Printed | `size_of::<Component>() == 56` |
| Null handling | Test | `"d": null` gives `None` |
| Error paths | Test | Union failures report `components[0]`, a missing id reports path `.` and `missing field id` |
| Feature selection | `cargo check --workspace --all-targets --features discord-rust/full` | Works in a virtual workspace |
| Parity data source | `RUSTC_BOOTSTRAP=1 cargo rustdoc -q -p discord-rust --lib --features full --locked -- -Z unstable-options --output-format json` | `target/doc/discord.json`, format 61, 890 items. Inherent impls show `Bound` with args. `attrs` shows `doc(alias)` |
| Multi-test acceptance form | `cargo test -q --locked -p discord-rust --features full --test behaviour --test misuse` | Pass |
| Absence check in the gate | `cargo tree -p discord-rust -e normal --features full -i aws-lc-rs` | Errors, as the `!` needs |
| Publish form | `cargo publish --help` | `--workspace`, `--exclude` and `--dry-run` exist. The dry run itself was not tried |
| Lints | `clippy.toml` keys parse; `unbounded_channel` fires under `-D warnings` | Pass |
| Names | `index.crates.io` and `cargo search`, 2026-11-07 | Section 3 |
| Dependency versions | `cargo search`, then resolved in `Cargo.lock` | Section 5.1 |
| Upstream pins | Local clones | Header of this document. 7550-line typings, 201 exported classes. `sent + 1 >= 115` at `WebSocketShard.ts:488` |

`GatewaySender` sits in `gateway` in the spike and in `dr_voice` in section 8.6, with the same shape. Section 8.6 is binding, because `voice` may not depend on `gateway`.

### Measured earlier, by probe [probe]

`scratchpad/tools/robust-probe/{stack,wire,mem,api,voice}`: socket-task separation under a slow handler, read-pause behaviour on loopback, memory per member and user pair (about 312 bytes), and voice clock numbers from a single run. Single-machine figures, to re-measure in V3 and H4.

### NOT verified

- **Nothing has run against a logged-in Discord session.** Not the bot, not the gateway, not REST, not voice.
- **[external], from documentation or memory:** Discord's reaction to read-pause, the nonce deduplication window for `enforce_nonce`, the invalid-request limit and IP ban, the identify penalty, DAVE server behaviour, the 4096-byte send limit, and the 1-5 s wait on op 9.
- **The compile cost of the real generated model is unknown.** The P0 stub proves shape only. M1 reports it.
- **The limiter's stall removal is reasoned, not tested,** until `sublimit_stall_xyr` passes in R2. The upstream hang itself was reproduced only on a transcription of the logic, not on the real library.
- `rustls-platform-verifier` and `opus-pure` compile, and were never exercised.
- The intent column in `[gateway 12]` is from memory. M5 checks it against discord-api-types. Two voice-channel event intents remain unverified.
- Sub-crate feature names in section 4 are a decision, not a test: the spike's `gateway`, `builders` and `voice` manifests have no `[features]` table. L0 writes them.
- The spike's facade is a stub: `Same<K>` in its collector bridge is spike-only, and its `run_until` does not connect.
- Commands in section 14 that name files not yet written could not run: `xtask`, `tools/gen`, and every per-package test. Their cargo forms were checked on the spike, as listed above.
- `loom`, `cargo-fuzz` and `criterion` targets were not tried. `loom` and `criterion` are pinned only. `cargo-fuzz` is not installed.
- Licensing is from reading the licence files. It is not legal advice, and trademarks were not researched.

### One side effect during verification

An early spike had `rust-toolchain.toml` with `channel = "1.99.0"`. rustup installed `1.99.0-x86_64-pc-windows-msvc` without asking. The file was removed from the spike and the toolchain uninstalled, so only `stable-x86_64-pc-windows-msvc` remains. The repository will carry the pin (5.1), so **the first cargo command after L0 repeats that download** on any machine. This is intended, but expect it.

## Appendix A. `dr_core` source

Verbatim from `scratchpad/spike/crates/core/src`, in case the scratchpad is gone. `endpoints.rs` is not in the spike: its definition is in section 8.1. `IntoBody` and its companions are in `wire.rs`. L0 starts from these files.

### `lib.rs`

```rust
//! Spike of the contract crate: ids, open enums, unions, Patch, wire helpers.
pub mod id;
pub mod macros;
pub mod patch;
pub mod secret;
pub mod time;
pub mod wire;

pub use id::{Id, marker};
pub use patch::Patch;
pub use secret::Secret;
pub use time::Timestamp;
pub use wire::{BodyError, DecodeError, IntoBody, Lenient, RawFile, RequestBody, ValidationError, decode, lenient_i64, null_default};

#[doc(hidden)]
pub mod __private {
    pub use bitflags;
    pub use compact_str;
    pub use serde;
    pub use serde_json;
}
```

### `id.rs`

```rust
use serde::de::{Deserializer, Error as DeError, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

pub const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

#[repr(transparent)]
pub struct Id<M>(u64, PhantomData<fn() -> M>);

pub mod marker {
    macro_rules! markers { ($($m:ident),* $(,)?) => { $(#[derive(Debug)] pub enum $m {})* } }
    markers!(UserMarker, GuildMarker, ChannelMarker, MessageMarker, RoleMarker, ApplicationMarker, InteractionMarker);
}

impl<M> Id<M> {
    pub const fn new(raw: u64) -> Self {
        Self(raw, PhantomData)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    pub const fn cast<N>(self) -> Id<N> {
        Id(self.0, PhantomData)
    }
    #[allow(clippy::cast_possible_wrap)]
    pub const fn timestamp_ms(self) -> i64 {
        ((self.0 >> 22) as i64) + DISCORD_EPOCH_MS
    }
}
impl<M> Clone for Id<M> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M> Copy for Id<M> {}
impl<M> PartialEq for Id<M> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<M> Eq for Id<M> {}
impl<M> Hash for Id<M> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}
impl<M> PartialOrd for Id<M> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<M> Ord for Id<M> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.cmp(&o.0)
    }
}
impl<M> fmt::Debug for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}
impl<M> fmt::Display for Id<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl<M> From<u64> for Id<M> {
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}
impl<M> From<Id<M>> for u64 {
    fn from(v: Id<M>) -> u64 {
        v.0
    }
}
impl<M> FromStr for Id<M> {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self::new)
    }
}
impl<M> Serialize for Id<M> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}
impl<'de, M> Deserialize<'de> for Id<M> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<M>(PhantomData<fn() -> M>);
        impl<M> Visitor<'_> for V<M> {
            type Value = Id<M>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a snowflake as a string or integer")
            }
            fn visit_str<E: DeError>(self, v: &str) -> Result<Id<M>, E> {
                v.parse().map(Id::new).map_err(E::custom)
            }
            fn visit_u64<E: DeError>(self, v: u64) -> Result<Id<M>, E> {
                Ok(Id::new(v))
            }
            fn visit_i64<E: DeError>(self, v: i64) -> Result<Id<M>, E> {
                u64::try_from(v).map(Id::new).map_err(E::custom)
            }
        }
        d.deserialize_any(V(PhantomData))
    }
}
```

### `macros.rs`

```rust
//! The four binding macros. `$crate::__private` keeps user crates free of direct deps,
//! except `tagged_union!`, whose private tag wrapper uses serde derive from the calling crate.

#[macro_export]
macro_rules! open_int {
    (
        $(#[$meta:meta])*
        $name:ident : $repr:ty {
            $( $(#[$cm:meta])* $k:ident = $v:expr ),* $(,)?
            $( ; $( $(#[$am:meta])* alias $a:ident = $t:ident ; )* )?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct $name(pub $repr);
        impl $name {
            $( $(#[$cm])* pub const $k: Self = Self($v); )*
            $( $( $(#[$am])* pub const $a: Self = Self::$t; )* )?
            /// Canonical constant name, or `None` for a value this build does not know.
            pub const fn name(self) -> ::core::option::Option<&'static str> {
                match self { $( Self::$k => ::core::option::Option::Some(stringify!($k)), )* _ => ::core::option::Option::None }
            }
            pub const fn get(self) -> $repr { self.0 }
        }
        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.name() {
                    ::core::option::Option::Some(n) => write!(f, concat!(stringify!($name), "::{}"), n),
                    ::core::option::Option::None => write!(f, concat!(stringify!($name), "({})"), self.0),
                }
            }
        }
        impl From<$repr> for $name { fn from(v: $repr) -> Self { Self(v) } }
        impl From<$name> for $repr { fn from(v: $name) -> $repr { v.0 } }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                $crate::__private::serde::Serialize::serialize(&self.0, s)
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                <$repr as $crate::__private::serde::Deserialize>::deserialize(d).map(Self)
            }
        }
    };
}

/// A value the build does not recognise. Only serde and `From<&str>` can construct one, so
/// `Unknown` can never hold a known string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Unrecognized(compact_str::CompactString);
impl Unrecognized {
    #[doc(hidden)]
    pub fn __new(s: compact_str::CompactString) -> Self { Self(s) }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[macro_export]
macro_rules! open_str {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vm:meta])* $v:ident = $s:literal ),* $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name { $( $(#[$vm])* $v, )* Unknown($crate::macros::Unrecognized) }
        impl $name {
            pub fn as_str(&self) -> &str {
                match self { $( Self::$v => $s, )* Self::Unknown(u) => u.as_str() }
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                match s { $( $s => Self::$v, )* other => Self::Unknown($crate::macros::Unrecognized::__new(other.into())) }
            }
        }
        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { f.write_str(self.as_str()) }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                let s = <$crate::__private::compact_str::CompactString as $crate::__private::serde::Deserialize>::deserialize(d)?;
                ::core::result::Result::Ok(match s.as_str() {
                    $( $s => Self::$v, )*
                    _ => Self::Unknown($crate::macros::Unrecognized::__new(s)),
                })
            }
        }
    };
}

/// u64 flag set that keeps every unknown bit. `number` or `string` sets the write form; reads
/// always accept both, because the same field has been seen in both forms.
#[macro_export]
macro_rules! open_flags {
    (
        $(#[$meta:meta])*
        $name:ident : $form:ident { $( $(#[$fm:meta])* $f:ident = $v:expr ),* $(,)? }
    ) => {
        $crate::__private::bitflags::bitflags! {
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct $name: u64 {
                $( $(#[$fm])* const $f = $v; )*
                const _ = !0;
            }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                $crate::open_flags!(@write $form, self, s)
            }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                $crate::wire::u64_string_or_int(d).map(Self::from_bits_retain)
            }
        }
    };
    (@write number, $self:ident, $s:ident) => { $s.serialize_u64($self.bits()) };
    (@write string, $self:ident, $s:ident) => { $s.collect_str(&$self.bits()) };
}

/// Integer-tagged union. Strategy B: capture raw text, peek the tag as u32, parse the arm.
#[macro_export]
macro_rules! tagged_union {
    (
        #[tag = $tag:literal]
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$am:meta])* $n:literal => $v:ident($t:ty), )*
            _ => Unknown,
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$am])* $v($t), )*
            /// A tag this build does not know. Re-serializes byte-exact.
            Unknown { kind: u32, raw: ::std::boxed::Box<$crate::__private::serde_json::value::RawValue> },
        }
        impl $name {
            pub fn kind(&self) -> u32 { match self { $( Self::$v(_) => $n, )* Self::Unknown { kind, .. } => *kind } }
        }
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(d: D) -> ::core::result::Result<Self, D::Error> {
                use $crate::__private::serde::de::Error as _;
                let raw = <::std::boxed::Box<$crate::__private::serde_json::value::RawValue> as $crate::__private::serde::Deserialize>::deserialize(d)?;
                let kind = $crate::wire::peek_tag(raw.get(), $tag).map_err(D::Error::custom)?;
                ::core::result::Result::Ok(match kind {
                    $( $n => Self::$v($crate::wire::parse_arm(raw.get()).map_err(D::Error::custom)?), )*
                    _ => Self::Unknown { kind, raw },
                })
            }
        }
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, s: S) -> ::core::result::Result<S::Ok, S::Error> {
                #[derive(::serde::Serialize)]
                struct Tagged<'a, T> { #[serde(rename = $tag)] tag: u32, #[serde(flatten)] inner: &'a T }
                match self {
                    $( Self::$v(inner) => $crate::__private::serde::Serialize::serialize(&Tagged { tag: $n, inner }, s), )*
                    Self::Unknown { raw, .. } => $crate::__private::serde::Serialize::serialize(raw, s),
                }
            }
        }
    };
}
```

### `patch.rs`

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Tri-state: absent, explicit null, value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Patch<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}
impl<T> Patch<T> {
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
    pub fn as_option(&self) -> Option<&T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }
}
impl<T> From<T> for Patch<T> {
    fn from(v: T) -> Self {
        Self::Value(v)
    }
}
impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(v) => v.serialize(s),
            Self::Absent | Self::Null => s.serialize_none(),
        }
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Option::<T>::deserialize(d)?.map_or(Self::Null, Self::Value))
    }
}
```

### `secret.rs`

```rust
use std::fmt;
/// Never appears in Debug or Display.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Box<str>);
impl Secret {
    pub fn new(s: impl Into<Box<str>>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}
impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}
```

### `time.rs`

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Microseconds since the Unix epoch. Tolerant parser, stable serializer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub i64);

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

impl Timestamp {
    /// Accepts `YYYY-MM-DDTHH:MM:SS[.f{1,9}](Z|+HH:MM|-HH:MM)`.
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
            return None;
        }
        let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
        let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
        let (mut i, mut micros) = (19usize, 0i64);
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            let mut scale = 100_000i64;
            while let Some(c) = b.get(i).filter(|c| c.is_ascii_digit()) {
                micros += i64::from(c - b'0') * scale;
                scale /= 10;
                i += 1;
            }
            if i == start {
                return None;
            }
        }
        let off_s = match *b.get(i)? {
            b'Z' | b'z' => 0,
            sign @ (b'+' | b'-') => {
                let oh = n(i + 1..i + 3)?;
                let om = n(i + 4..i + 6)?;
                let v = oh * 3600 + om * 60;
                if sign == b'-' { -v } else { v }
            }
            _ => return None,
        };
        let days = days_from_civil(y, u32::try_from(mo).ok()?, u32::try_from(d).ok()?);
        Some(Self((days * 86_400 + h * 3600 + mi * 60 + se - off_s) * 1_000_000 + micros))
    }
}
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (secs, micros) = (self.0.div_euclid(1_000_000), self.0.rem_euclid(1_000_000));
        let (days, tod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        let (y, m, d) = civil_from_days(days);
        write!(f, "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{micros:06}+00:00", tod / 3600, tod / 60 % 60, tod % 60)
    }
}
impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = std::borrow::Cow::<str>::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom(format_args!("invalid timestamp {s:?}")))
    }
}
```

### `wire.rs`

```rust
use bytes::Bytes;
use serde::de::{DeserializeOwned, Deserializer, Error as DeError, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fmt;

// ---- union support --------------------------------------------------------------------
/// Peek an integer tag at the top level of `raw` without parsing other fields.
pub fn peek_tag(raw: &str, key: &'static str) -> Result<u32, serde_json::Error> {
    struct V(&'static str);
    impl<'de> Visitor<'de> for V {
        type Value = u32;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "object with integer `{}`", self.0)
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<u32, A::Error> {
            let mut found = None;
            while let Some(k) = m.next_key::<std::borrow::Cow<'de, str>>()? {
                if k == self.0 {
                    found = Some(m.next_value::<u32>()?);
                } else {
                    m.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            found.ok_or_else(|| A::Error::missing_field(self.0))
        }
    }
    let mut de = serde_json::Deserializer::from_str(raw);
    de.deserialize_map(V(key))
}
pub fn parse_arm<T: DeserializeOwned>(raw: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(raw)
}

// ---- field helpers --------------------------------------------------------------------
pub fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
/// serde_json refuses `1.0` for an integer. For fields marked numeric-tolerant in the override table.
pub fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a number")
        }
        fn visit_i64<E: DeError>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }
        fn visit_u64<E: DeError>(self, v: u64) -> Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }
        #[allow(clippy::cast_possible_truncation)]
        fn visit_f64<E: DeError>(self, v: f64) -> Result<i64, E> {
            if v.fract() == 0.0 && v.abs() < 9.0e15 { Ok(v as i64) } else { Err(E::custom("not an integral number")) }
        }
    }
    d.deserialize_any(V)
}
pub fn u64_string_or_int<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an integer or decimal string")
        }
        fn visit_str<E: DeError>(self, v: &str) -> Result<u64, E> {
            v.parse().map_err(E::custom)
        }
        fn visit_u64<E: DeError>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_i64<E: DeError>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(E::custom)
        }
    }
    d.deserialize_any(V)
}
/// One bad element does not lose the list.
#[derive(Clone, Debug, Default)]
pub struct Lenient<T> {
    pub items: Vec<T>,
    pub dropped: u32,
}
impl<'de, T: DeserializeOwned> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raws = Vec::<Box<RawValue>>::deserialize(d)?;
        let mut out = Self { items: Vec::with_capacity(raws.len()), dropped: 0 };
        for r in raws {
            match serde_json::from_str(r.get()) {
                Ok(v) => out.items.push(v),
                Err(_) => out.dropped += 1,
            }
        }
        Ok(out)
    }
}

// ---- decode with a path only on failure -------------------------------------------------
#[derive(Debug, thiserror::Error)]
#[error("decode failed at `{path}`: {source}")]
pub struct DecodeError {
    pub path: String,
    #[source]
    pub source: serde_json::Error,
}

/// Hot path has no tracking cost. On failure the same text is decoded again to find the path.
pub fn decode<T: DeserializeOwned>(text: &str) -> Result<T, DecodeError> {
    match serde_json::from_str(text) {
        Ok(v) => Ok(v),
        Err(first) => {
            let mut de = serde_json::Deserializer::from_str(text);
            match serde_path_to_error::deserialize::<_, T>(&mut de) {
                Err(e) => {
                    let path = e.path().to_string();
                    Err(DecodeError { path, source: e.into_inner() })
                }
                Ok(_) => Err(DecodeError { path: String::from("."), source: first }),
            }
        }
    }
}

// ---- request bodies --------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {message}")]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BodyError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Encode(#[from] serde_json::Error),
}
#[derive(Clone, Debug)]
pub struct RawFile {
    pub field: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub data: Bytes,
}
#[derive(Clone, Debug, Default)]
pub struct RequestBody {
    pub json: Option<Bytes>,
    pub files: Vec<RawFile>,
}
impl RequestBody {
    pub fn json<T: Serialize + ?Sized>(v: &T) -> Result<Self, BodyError> {
        Ok(Self { json: Some(Bytes::from(serde_json::to_vec(v)?)), files: Vec::new() })
    }
}
/// How a builder or wire struct becomes bytes. `rest` knows this trait and not `builders`.
pub trait IntoBody {
    fn into_body(self) -> Result<RequestBody, BodyError>;
}
impl IntoBody for RequestBody {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        Ok(self)
    }
}
impl IntoBody for () {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        Ok(RequestBody::default())
    }
}
```
