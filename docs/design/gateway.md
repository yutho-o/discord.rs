# Gateway layer map for the discord.rs port

Source of truth for porting `@discordjs/ws` and the discord.js gateway client layer to Rust.

## 0. Scope and provenance

| Item | Value |
| --- | --- |
| discord.js monorepo | commit `5bb74922` (2026-11-06), `packages/ws` version 2.0.2 |
| `packages/discord.js/package.json` version | says 14.16.3, but the code is the main branch: the `Shard*` client events are gone (see 13.6) |
| discord-api-types | commit `00cbe03b` (2026-11-06), version 0.38.57, gateway v10 |
| `node_modules` | not present in the clone, so third-party sources were not read |

Paths are relative to the discord.js repo root, except `dat:` which is relative to the discord-api-types repo root.

Each claim falls into one of these classes:

- Unmarked: read directly in the cited source.
- **[inferred]**: concluded from reading the code. Nothing was executed, as instructed, so races are reasoned rather than reproduced.
- **[external]**: from my memory of Discord's docs or of a third-party library, not in the cloned sources. Check before relying on it.

## 1. File map

### packages/ws/src

| File | Lines | Role |
| --- | --- | --- |
| `ws/WebSocketShard.ts` | 992 | The whole protocol: connection, state machine, heartbeat, identify, resume, send limiter, decompression, close handling |
| `ws/WebSocketManager.ts` | 390 | Public facade: options, shard id and count computation, `session_start_limit` check, delegates to the strategy |
| `utils/constants.ts` | 85 | Defaults, `Encoding`, `CompressionMethod`, `ImportantGatewayOpcodes`, process-wide default session store |
| `throttling/IIdentifyThrottler.ts` | 11 | `waitForIdentify(shardId, signal)` |
| `throttling/SimpleIdentifyThrottler.ts` | 48 | In-memory throttler, one FIFO queue per `shardId % maxConcurrency` |
| `strategies/sharding/IShardingStrategy.ts` | 30 | `spawn`, `connect`, `destroy`, `send`, `fetchStatus` |
| `strategies/sharding/SimpleShardingStrategy.ts` | 85 | All shards in the current thread |
| `strategies/sharding/WorkerShardingStrategy.ts` | 373 | Shards in `worker_threads`, with a message protocol |
| `strategies/sharding/defaultWorker.ts` | 4 | Default worker entry point |
| `strategies/context/IContextFetchingStrategy.ts` | 59 | What a shard needs from its host: options snapshot, session get/set, identify gate |
| `strategies/context/SimpleContextFetchingStrategy.ts` | 40 | Direct calls; one throttler per manager in a `WeakMap` |
| `strategies/context/WorkerContextFetchingStrategy.ts` | 99 | Same calls proxied over `parentPort` with random nonces |
| `utils/WorkerBootstrapper.ts` | 176 | Worker-side shard hosting and event forwarding |

### packages/discord.js/src

| File | Role |
| --- | --- |
| `client/Client.js` | `login`, `_attachEvents`, `_handlePacket`, `_checkReady`, `_triggerClientReady`, `_broadcast`, `destroy` |
| `client/websocket/handlers/index.js` | `PacketHandlers` table, event name to handler |
| `client/websocket/handlers/*.js` | 72 packet handlers, one per event |
| `client/actions/*.js` | Cache mutation shared with REST paths (`ActionsManager.js`, `Action.js`) |
| `client/voice/ClientVoiceManager.js` | Forwards voice packets to adapters |
| `util/Status.js` | `Ready`, `Idle`, `WaitingForGuilds` |
| `util/Events.js` | Client event names |
| `util/Partials.js` | Partial structure opt-ins |
| `util/IntentsBitField.js` | Bitfield over `GatewayIntentBits` |
| `util/Options.js` | Client defaults, including `ws.largeThreshold: 50`, `ws.version: 10`, `waitGuildTimeout: 15_000` |

There is no `WebSocketManager` under `packages/discord.js/src/client/websocket` any more; only `handlers/` remains. `Client.ws` is the `@discordjs/ws` manager directly (`Client.js:218`).

Also relevant:

- `packages/util/src/functions/calculateShardId.ts`
- `packages/util/src/gatewayRateLimitError.ts`
- `packages/core/src/client.ts:240-461`, the non-cached request helpers

## 2. WebSocketManager

Source: `packages/ws/src/ws/WebSocketManager.ts`, defaults in `utils/constants.ts:39-72`.

### 2.1 Options

| Option | Default | Notes |
| --- | --- | --- |
| `intents` | required | Bitfield, `0` allowed |
| `token` | none | May be set once later with `setToken`; a second set throws (`:360-366`) |
| `buildIdentifyThrottler(manager)` | `SimpleIdentifyThrottler(session_start_limit.max_concurrency)` | May be async |
| `buildStrategy(manager)` | `SimpleShardingStrategy` | Called in the constructor (`:270`) |
| `capabilities` | `0` | Always sent in identify |
| `compression` | `null` | Transport compression |
| `encoding` | `'json'` | The only `Encoding` member (`constants.ts:13-15`) |
| `handshakeTimeout` | 30 000 ms | Passed to the socket constructor |
| `helloTimeout` | 60 000 ms | Timer starts right after the socket is created, so it covers connect, TLS and upgrade too |
| `readyTimeout` | 15 000 ms | Starts after the identify is sent |
| `identifyProperties` | `{browser, device: '@discordjs/ws <ver>', os: process.platform}` | |
| `initialPresence` | `null` | Sent only when truthy |
| `largeThreshold` | `null` (Client sets 50) | Sent only when truthy |
| `shardCount` | `null` | `null` means recommended |
| `shardIds` | `null` | `number[]`, or `{start, end}` with **inclusive** end |
| `retrieveSessionInfo(shardId)` | process-wide map | May be async |
| `updateSessionInfo(shardId, info \| null)` | process-wide map | May be async |
| `useIdentifyCompression` | `false` | Mutually exclusive with `compression` |
| `version` | `APIVersion` (`'10'`) | Client passes numeric `10`; it is stringified in the query |

`SessionInfo` is `{resumeURL, sequence, sessionId, shardCount, shardId}` (`:30-51`).

### 2.2 Methods

- **`connect({gatewayInformation})`** (`:338-358`)
  1. Throws `TypeError` when the information is missing.
  2. Stores it.
  3. Computes shard ids.
  4. Throws when `session_start_limit.remaining < shardIds.length`, clearing the stored information. The message computes the reset date from `reset_after` in milliseconds.
  5. Calls `updateShardCount(getShardCount())`, which destroys, pins `options.shardCount` and spawns.
  6. Calls `strategy.connect()`.
- **`getShardIds()`** (`:322-336`): array as is; object as `start..=end`; otherwise `0..(shardCount ?? gatewayInformation.shards)`.
- **`getShardCount()`** (`:307-314`): `options.shardCount`, else `max(ids) + 1`. With `shardIds: [1, 3, 7]` and no count this gives 8.
- **`updateShardCount(n)`** (`:291-299`): `strategy.destroy({reason})`, set, then `spawn`. It does not connect.
- **`destroy(options)`** (`:374-377`): options without `recover`, then clears gateway information. Keep a session resumable with `code: 4200`.
- **`send(shardId, payload)`**, **`fetchStatus()`**, **`getGatewayInformation()`** (throws before `connect`), async dispose.

Events are forwarded from every shard with `shardId` appended as the last argument (`SimpleShardingStrategy.ts:31-34`, `ManagerShardEventsMap` at `:217-230`).

## 3. WebSocketShard lifecycle

Source: `packages/ws/src/ws/WebSocketShard.ts`.

### 3.1 Types

- **Events** (`:35-45`): `closed(code)`, `debug(msg)`, `dispatch(payload)`, `error(err)`, `heartbeat({ackAt, heartbeatAt, latency})`, `hello()`, `ready(data)`, `resumed()`, `socketError(err)`.
- **Status** (`:47-52`): `Idle`, `Connecting`, `Resuming`, `Ready`.
- **Recovery** (`:54-57`): `Reconnect`, `Resume`.
- **Local close codes** (`:77-80`): `Normal = 1000`, `Resuming = 4200`.

### 3.2 State transitions

| From | Trigger | To | Line |
| --- | --- | --- | --- |
| Idle | `internalConnect`, socket created | Connecting | 309 |
| any other than Idle | `internalConnect` called | throws "wasn't idle" | 182 |
| Connecting | Hello received, resumable session, `resume()` | Resuming | 592 |
| Connecting | Hello received, identify sent | stays Connecting until READY | 575 |
| Connecting or Resuming | READY | Ready | 751 |
| Resuming | RESUMED | Ready | 768 |
| Ready | op 9 with `d: true` and a session | Resuming, same socket | 811-812, 592 |
| any non-Idle | `destroy` completes | Idle | 404 |

### 3.3 `connect()` (`:157-179`)

- The first call waits for `race(once(Ready), once(Resumed))`. It resolves on the first of them.
- Later calls, after one has resolved, just start `internalConnect` and return.
- `once` is from `node:events`, which rejects if `error` is emitted during the wait. That is how fatal close codes reject `connect()` and therefore `Client.login()`.

### 3.4 `internalConnect()` (`:181-321`)

1. Require status Idle.
2. Build query params `v`, `encoding`, and `compress` when configured.
3. Create a **new** decompressor for this connection (section 8).
4. `session = retrieveSessionInfo(id)`.
5. URL is `${session?.resumeURL ?? gatewayInformation.url}?${params}` (`:281`). The query string is appended on the resume URL as well.
6. Open the socket with `binaryType = 'arraybuffer'`. `onopen` resets the send limiter (`:303-305`).
7. Status becomes Connecting.
8. Wait for Hello with `helloTimeout`; return if not ok.
9. If `session?.shardCount === options.shardCount`, `resume(session)`. Otherwise `identify()`.

### 3.5 `onMessage` by opcode (`:737-860`)

| Op | Behaviour |
| --- | --- |
| 0 Dispatch | See below |
| 1 Heartbeat | `heartbeat(true)`, immediately, skipping the ack check |
| 7 Reconnect | `destroy({recover: Resume})`, close code becomes 4200 |
| 9 InvalidSession | `d` true and session present: `resume(session)` on the same socket. Otherwise `destroy({recover: Reconnect})` |
| 10 Hello | Emit `hello`, sleep `floor(interval * random())` (abortable), `heartbeat()`, then `setInterval` |
| 11 HeartbeatAck | `isAck = true`, emit `heartbeat` stats |
| other | Ignored, the switch has no default |

Dispatch steps (`:744-792`), in this order:

1. If Resuming, `replayedEvents++`.
2. READY: status Ready, build and **store** the session, emit `ready`.
3. RESUMED: status Ready, emit `resumed`.
4. Retrieve the session; if `payload.s > session.sequence`, store the new sequence. If no session, log only.
5. Emit `dispatch`. READY and RESUMED are also delivered as dispatch, after their dedicated event.

### 3.6 `identify()` (`:517-582`)

1. Wire `closed` to an abort controller and await `strategy.waitForIdentify(id, signal)`.
2. If aborted, return.
3. Build the payload:
   - `token`, `properties`, `intents`, `capabilities` (even 0), `compress`, `shard: [id, shardCount]`
   - `large_threshold` and `presence` only when truthy
4. Send op 2.
5. `waitForEvent(Ready, readyTimeout)`.

### 3.7 `resume()` (`:584-603`)

Sets Resuming, resets `replayedEvents`, sends op 6 `{token, seq, session_id}`. **There is no timeout waiting for RESUMED**; only zombie detection can end a hung resume.

### 3.8 `destroy()` (`:323-412`)

1. If Idle: log and return. Session info is not touched.
2. Default code: `recover === Resume` gives 4200, otherwise 1000.
3. Reset `isAck`, clear interval, abort first-heartbeat sleep, `lastHeartbeatAt = -1`, abort all `waitForEvent` timers, reset the network-error flag.
4. Clear session when `(recover !== Resume && code !== 4200) || code === 1000`.
5. Detach `onmessage` and `onclose`.
6. If the socket is OPEN: `close(code, reason)`, wait for the close, emit `closed(code)`.
7. Detach `onerror` last. The upstream comment notes that doing it earlier could crash on an error fired by `close`.
8. Status Idle.
9. If `recover` is set: sleep 500 ms, then `internalConnect()`.

Session clearing truth table:

| recover | code | Session |
| --- | --- | --- |
| Resume | default 4200 | Kept |
| Resume | 4000, 4001, 4002, 4009, unknown | Kept |
| Resume | explicit 1000 | Cleared |
| Reconnect | default 1000 | Cleared |
| Reconnect | 40xx | Cleared |
| Reconnect | explicit 4200 | Kept |
| none | default 1000 | Cleared |
| none | explicit 4200 | Kept |
| none | fatal 40xx | Cleared |

### 3.9 `waitForEvent()` (`:414-455`)

Races `once(event, {signal: timeout})` against `once(closed)`:

- Event first: `{ok: true}`.
- Closed first: `{ok: false}`; a destroy is already running.
- Rejection (timeout, `abort`, or an `error` event): `destroy({code: 1000, recover: Reconnect})` and `{ok: false}`.

So a Hello or Ready timeout **clears the session** and re-identifies.

## 4. Send path and rate limit

`send()` at `WebSocketShard.ts:457-515`, constants at `utils/constants.ts:74-85`.

1. No connection object: throw.
2. **Important opcodes** 1, 2 and 6: send immediately, with no queue, no counter and no status check.
3. Status not Ready: wait for `ready`. If that rejects because of an `error` event, retry `send` from the top.
4. Wait on the FIFO `sendQueue`.
5. If `now >= resetAt`, reset to `{sent: 0, resetAt: now + 60 000}`.
6. If `sent + 1 >= 115`:
   - sleep `resetAt - now + random() * 1500`
   - race the sleep against `closed`
   - if closed first: release the queue and retry `send`, so the payload moves to the new connection
7. `sent++`, release the queue, serialize and write.

The budget rationale is in the upstream comment (`:462-464`): Discord allows 120 per 60 s; 115 is used and the rest is left for the unthrottled opcodes.

Details:

- The window is per connection, reset in `onopen`.
- At most 114 throttled payloads pass in a window, because the check fires at `sent == 114`.
- The sleeping send is counted on the stale window. The next send resets the state, so one payload per rollover goes uncounted. Harmless given the margin.
- The window starts at `onopen` or the first send after expiry. It is a fixed window, not sliding.
- Worker strategy: `send` posts and returns immediately (`WorkerShardingStrategy.ts:179-191`), so the caller sees no completion or error.

`RATE_LIMITED` dispatch `{opcode, retry_after (seconds), meta}` is defined at `dat:gateway/v10.ts:2411-2473`; only op 8 has metadata today (`{guild_id, nonce?}`). See section 13.5 for handling.

## 5. Heartbeat

`WebSocketShard.ts:605-620`, `:823-845`, `:847-858`.

- First beat waits `floor(heartbeat_interval * jitter)` with `jitter` in `[0, 1)`. The sleep is abortable by `destroy`.
- Then a repeating interval of `heartbeat_interval`.
- On each non-requested beat: if the previous beat is unacknowledged, `destroy({reason: 'Zombie connection', recover: Resume})`, close code 4200.
- Payload `{op: 1, d: session?.sequence ?? null}`. `d` must serialize as JSON `null`, not be omitted.
- The sequence is read from the session store on every beat, through a worker round trip in worker mode.
- `lastHeartbeatAt` is set after the send; latency is `ackAt - lastHeartbeatAt`.
- A requested beat (op 1 from the server) sets `isAck = false` too, so a server-requested beat also needs an ack before the next timer tick.
- The heartbeat runs concurrently with the identify wait, because each `onMessage` call is independent. **A shard waiting in the throttle keeps heartbeating.**
- Client: `pings[shardId] = latency`, `lastPingTimestamps[shardId] = heartbeatAt`, `client.ping` is the average (`Client.js:391-395`, `:475-477`).

## 6. Session and resume rules

- READY stores `{sequence: s, sessionId, shardId, shardCount, resumeURL: resume_gateway_url}` (`:753-761`).
- Sequence only moves forward (`:781`), and is stored **before** `dispatch` is emitted.
- A resume is attempted only when the stored `shardCount` equals the current one (`:316`).
- A stored session with another shard count still contributes its `resumeURL` to the URL, then identifies there (`:281` and `:316`).
- Non-1000 close keeps the session alive on Discord's side, hence 4200 when recovering by resume. **[external]** Closing with 1000 or 1001 invalidates the session.
- Persisting across restarts works through the two callbacks, combined with `destroy({code: 4200})`.
- Default `Client.destroy()` uses code 1000 and clears the session (`Client.js:532`).

## 7. Close codes

`onClose` at `WebSocketShard.ts:867-987`. `closed` is always emitted first (`:868`). Codes come from `dat:gateway/v10.ts:121-198`.

| Code | Name | Action | Session | `error` event |
| --- | --- | --- | --- | --- |
| 1000 | Normal | Reconnect | Cleared | No |
| 4200 | Resuming (local) | Nothing, `break` | Unchanged | No |
| 4000 | UnknownError | Resume | Kept | No |
| 4001 | UnknownOpcode | Resume | Kept | No |
| 4002 | DecodeError | Resume | Kept | No |
| 4003 | NotAuthenticated | Reconnect | Cleared | No |
| 4004 | AuthenticationFailed | Stop | Cleared | `Authentication failed` |
| 4005 | AlreadyAuthenticated | Reconnect | Cleared | No |
| 4006 | not defined in enum | Default branch | See default | No |
| 4007 | InvalidSeq | Reconnect | Cleared | No |
| 4008 | RateLimited | Reconnect | Cleared | No |
| 4009 | SessionTimedOut | **Resume** | Kept | No |
| 4010 | InvalidShard | Stop | Cleared | `Invalid shard` |
| 4011 | ShardingRequired | Stop | Cleared | `Sharding is required` |
| 4012 | InvalidAPIVersion | Stop | Cleared | `Used an invalid API version` |
| 4013 | InvalidIntents | Stop | Cleared | `Used invalid intents` |
| 4014 | DisallowedIntents | Stop | Cleared | `Used disallowed intents` |
| other (1001, 1006, ...) | default | Socket error seen on this connection: Reconnect (cleared). Otherwise: Resume (kept) | | No |

Every recovery goes through the fixed 500 ms sleep in `destroy`. There is no backoff.

## 8. Compression and encoding

### 8.1 Encoding

JSON only. `GatewayURLQuery` allows `etf` (`dat:gateway/common.ts`) but `Encoding` has a single member, and `send` always uses `JSON.stringify`.

### 8.2 Modes

| Option | Query | Behaviour |
| --- | --- | --- |
| none | none | Text frames with JSON |
| `ZlibNative`, `ZlibSync` | `compress=zlib-stream` | Streaming zlib, one context per connection |
| `ZstdNative` | `compress=zstd-stream` | Streaming zstd, one context per connection |
| `useIdentifyCompression` | none, `compress: true` in identify | Some frames arrive binary, each a standalone zlib stream |

- The two zlib options differ only in the Node backend; Rust needs one.
- When a backend is missing upstream warns and removes `compress` from the query (`:219-220`, `:234-235`, `:260-263`). In Rust this becomes a compile-time feature.
- `compression` set and `useIdentifyCompression` true: warn and disable identify compression (`:191-194`). Transport wins.

### 8.3 Frame decoding (`unpackMessage`, `:630-735`)

1. Text frame: `JSON.parse`. On failure return null, silently dropped. The upstream comment cites Bun misreading custom close codes.
2. Binary with identify compression: one-shot inflate of the frame.
3. Binary with transport compression:
   - zstd: every WebSocket message is a complete gateway message, so flush is always true.
   - zlib: flush only when the message ends with `00 00 FF FF` and has at least 4 bytes (`:663-669`). Otherwise feed the data and return null, because a gateway message may span several WebSocket messages.
   - Both modes use `chunkSize: 65_535`.
   - Inflated bytes are decoded as UTF-8 and parsed as JSON. Empty output returns null (`:622-628`).
4. Otherwise: debug log, dropped.

Decompression errors only emit `error`; the shard does not reconnect by itself (`:213-215`, `:254-256`, `:674-677`, `:698-716`). Because `once` rejects on `error`, any pending `waitForEvent` turns this into destroy + Reconnect, and a pending first `connect()` rejects.

## 9. Identify throttling

`throttling/SimpleIdentifyThrottler.ts`, `strategies/context/SimpleContextFetchingStrategy.ts`.

```
key = shardId % maxConcurrency
state = per key { FIFO queue, resetsAt = +inf initially }
await queue.wait({signal})            // abortable while queued
diff = resetsAt - now
if 0 < diff <= 5000: sleep(diff + random() * 1500)   // not abortable
resetsAt = now + 5000
queue.shift()
```

- One identify per 5 s per bucket, plus up to 1.5 s of jitter.
- A slot is consumed when `resetsAt` is written, whether or not the identify is then sent.
- `diff > 5000` cannot happen with this code, except via the `+inf` initial value, which skips the sleep.
- Test at `__tests__/util/SimpleIdentifyThrottler.test.ts` pins that negative `diff` never sleeps and that shards 0 and 1 do not block each other, with 2 and 3 waiting.
- One throttler per manager (`WeakMap`), kept across `updateShardCount`. `maxConcurrency` is fixed by the first `gatewayInformation`.
- Custom throttlers are the extension point for distributed setups. Contract in `IContextFetchingStrategy.ts:32-37`: reject with an abort error on abort; other errors make the shard recover.
- Worker mode: one throttler in the main thread, with `CancelIdentify` for aborts.

Resume does not go through the throttler.

## 10. Sharding strategies

### 10.1 Simple

Single thread. Each shard has a `SimpleContextFetchingStrategy` holding an options snapshot taken at `spawn`. `connect` is `Promise.all`. `send` to an unknown shard throws `RangeError`. `destroy` clears the map.

### 10.2 Worker

One worker per `shardsPerWorker` shards, or one for `'all'`.

Main to worker (`WorkerSendPayloadOp`, `WorkerShardingStrategy.ts:20-35`):

| Op | Payload |
| --- | --- |
| Connect | `shardId` |
| Destroy | `shardId`, `options` |
| Send | `shardId`, `payload` |
| SessionInfoResponse | `nonce`, `session` |
| ShardIdentifyResponse | `nonce`, `ok` |
| FetchStatus | `nonce`, `shardId` |

Worker to main (`WorkerReceivePayloadOp`, `:37-59`):

| Op | Payload |
| --- | --- |
| Connected, Destroyed | `shardId` |
| Event | `event`, `data[]`, `shardId` |
| RetrieveSessionInfo | `nonce`, `shardId` |
| UpdateSessionInfo | `shardId`, `session` |
| WaitForIdentify | `nonce`, `shardId` |
| CancelIdentify | `nonce` |
| FetchStatusResponse | `nonce`, `status` |
| WorkerReady | none |

Notes:

- `WorkerBootstrapper.bootstrap({forwardEvents, shardCallback})` lets a user process dispatch inside the worker.
- `unknownPayloadHandler` receives custom messages.
- `destroy` waits for `Destroyed`, then terminates the worker.
- The token is passed in `workerData`.

In Rust, tasks on a multi-threaded runtime replace this. The useful part is the seam: `ShardContext` (session get/set, identify gate) and `ShardingStrategy`. See section 16.

### 10.3 Multi-process

The same manager options cover it: pass `shardIds` and `shardCount`, plus a shared throttler and session store. discord.js also has `src/sharding/ShardingManager.js`; it passes `SHARDS`, `SHARD_COUNT`, `SHARDING_MANAGER` via env or `workerData` (`Client.js:103-112`, `:225`), and listens to `closed` and `resumed` (`ShardClientUtil.js:36-66`).

### 10.4 Shard routing

`calculateShardId(guildId, shardCount) = Number(BigInt(guildId) >> 22n) % shardCount` (`packages/util/src/functions/calculateShardId.ts`). In Rust: `((guild_id >> 22) % shard_count as u64) as u32`.

## 11. Protocol reference

### 11.1 Opcodes (`dat:gateway/v10.ts:62-116`)

| Op | Name | Direction |
| --- | --- | --- |
| 0 | Dispatch | Receive |
| 1 | Heartbeat | Both |
| 2 | Identify | Send |
| 3 | PresenceUpdate | Send |
| 4 | VoiceStateUpdate | Send |
| 6 | Resume | Send |
| 7 | Reconnect | Receive |
| 8 | RequestGuildMembers | Send |
| 9 | InvalidSession | Receive |
| 10 | Hello | Receive |
| 11 | HeartbeatAck | Receive |
| 31 | RequestSoundboardSounds | Send |
| 43 | RequestChannelInfo | Send |

Op 5 does not exist. Non-dispatch payloads have `t: null, s: null` (`:2798-2801`).

### 11.2 Send payloads (`dat:gateway/v10.ts:2482-2771`)

| Op | Fields |
| --- | --- |
| 1 | `d: number \| null` |
| 2 | `token`, `properties{os, browser, device}`, `compress?`, `large_threshold?` (50 to 250), `shard?`, `presence?`, `intents`, `capabilities?` |
| 3 | `since: number \| null`, `activities[]{name, state?, type, url?}`, `status`, `afk` |
| 4 | `guild_id`, `channel_id \| null`, `self_mute`, `self_deaf` |
| 6 | `token`, `session_id`, `seq` |
| 8 | `guild_id`, `presences?`, `nonce?` (up to 32 bytes; an invalid nonce is ignored and replies have none), then either `user_ids` (one or many) or `query` + `limit` |
| 31 | `guild_ids[]` |
| 43 | `guild_id`, `fields[]` (`status`, `voice_start_time`, or any string) |

**[external]** Discord closes with 4002 for sent payloads over 4096 bytes.

### 11.3 Intents (`dat:gateway/v10.ts:203-233`)

| Name | Bit | Value | Privileged [external] |
| --- | --- | --- | --- |
| Guilds | 0 | 1 | |
| GuildMembers | 1 | 2 | Yes |
| GuildModeration (alias GuildBans) | 2 | 4 | |
| GuildExpressions (alias GuildEmojisAndStickers) | 3 | 8 | |
| GuildIntegrations | 4 | 16 | |
| GuildWebhooks | 5 | 32 | |
| GuildInvites | 6 | 64 | |
| GuildVoiceStates | 7 | 128 | |
| GuildPresences | 8 | 256 | Yes |
| GuildMessages | 9 | 512 | |
| GuildMessageReactions | 10 | 1024 | |
| GuildMessageTyping | 11 | 2048 | |
| DirectMessages | 12 | 4096 | |
| DirectMessageReactions | 13 | 8192 | |
| DirectMessageTyping | 14 | 16384 | |
| MessageContent | 15 | 32768 | Yes |
| GuildScheduledEvents | 16 | 65536 | |
| AutoModerationConfiguration | 20 | 1048576 | |
| AutoModerationExecution | 21 | 2097152 | |
| GuildMessagePolls | 24 | 16777216 | |
| DirectMessagePolls | 25 | 33554432 | |

Bits 17, 18, 19, 22 and 23 are unassigned here. All listed bits together: 53608447. An unknown bit should be allowed through as a raw value, so that new intents do not need a release.

### 11.4 Capabilities (`dat:gateway/v10.ts:238-247`)

Only `ChannelObfuscation = 1 << 15`, marked `@unstable` as a temporary testing opt-in.

## 12. Dispatch events

`GatewayDispatchEvents` has **79** members (`dat:gateway/v10.ts:252-332`). `PacketHandlers` has **72**. The exact difference was checked by script, and is the 7 events marked "no handler" below.

Column notes:

- **Intent** is **[external]**, from Discord's docs; the cloned sources have no such mapping. "none" means not gated.
- **Handler**: `A` delegates to `client/actions/<Name>.js`, `H` is inline in the handler file.
- All client events listed exist in `util/Events.js`; the string name is given where it differs from the obvious camelCase.
- `dat` lines are the `Gateway...Dispatch` types in `gateway/v10.ts`.

| Event | dat | Intent | Handler | Client event(s) | Guards and side effects |
| --- | --- | --- | --- | --- | --- |
| APPLICATION_COMMAND_PERMISSIONS_UPDATE | 671 | none | H | ApplicationCommandPermissionsUpdate | No caching, emits mapped object |
| AUTO_MODERATION_ACTION_EXECUTION | 605 | AutoModerationExecution | H | AutoModerationActionExecution | Guild cached |
| AUTO_MODERATION_RULE_CREATE | 566 | AutoModerationConfiguration | H | AutoModerationRuleCreate | Guild cached |
| AUTO_MODERATION_RULE_DELETE | 592 | AutoModerationConfiguration | H | AutoModerationRuleDelete | Guild and rule cached |
| AUTO_MODERATION_RULE_UPDATE | 579 | AutoModerationConfiguration | H | AutoModerationRuleUpdate(old?, new) | Guild cached, old may be null |
| CHANNEL_CREATE | 779 | Guilds | A | ChannelCreate | Emits only if not already cached |
| CHANNEL_DELETE | 805 | Guilds | A | ChannelDelete | Cached only |
| CHANNEL_INFO | 818 | response to op 43 | no handler | none | |
| CHANNEL_PINS_UPDATE | 858 | Guilds, DirectMessages | H | ChannelPinsUpdate(channel, time) | Channel cached; null timestamp when last pin removed |
| CHANNEL_UPDATE | 792 | Guilds | A + H | ChannelUpdate(old, new) | Uncached: added, no emit. Type change: rebuilt, messages migrated; if not buildable, removed from cache |
| ENTITLEMENT_CREATE | 908 | none | H | EntitlementCreate | Uses `client.application` |
| ENTITLEMENT_DELETE | 934 | none | H | EntitlementDelete | Built uncached, removed |
| ENTITLEMENT_UPDATE | 921 | none | H | EntitlementUpdate(old?, new) | |
| GUILD_AUDIT_LOG_ENTRY_CREATE | 2350 | GuildModeration | H | GuildAuditLogEntryCreate(entry, guild) | Guild cached |
| GUILD_BAN_ADD | 1112 | GuildModeration | H | GuildBanAdd | Guild cached |
| GUILD_BAN_REMOVE | 1125 | GuildModeration | H | GuildBanRemove | Guild cached; ban built if uncached |
| GUILD_CREATE | 955 | Guilds | H | GuildAvailable or GuildCreate | See 13.3 |
| GUILD_DELETE | 1066 | Guilds | H | GuildUnavailable or GuildDelete | See 13.3 |
| GUILD_EMOJIS_UPDATE | 1138 | GuildExpressions | A | GuildEmojiCreate, GuildEmojiUpdate, GuildEmojiDelete (`emojiCreate`, ...) | Diff of full list against cache |
| GUILD_INTEGRATIONS_UPDATE | 1186 | GuildIntegrations | H | GuildIntegrationsUpdate | Guild cached |
| GUILD_MEMBER_ADD | 1204 | GuildMembers | H | GuildMemberAdd | Guild cached, `memberCount++` |
| GUILD_MEMBER_REMOVE | 1222 | GuildMembers | A | GuildMemberRemove | `memberCount--` always; emits when member resolved; removes presence and voice state |
| GUILD_MEMBER_UPDATE | 1246 | GuildMembers | A | UserUpdate, GuildMemberUpdate, GuildMemberAvailable | User sync first; emits only if changed; uncached member: added, GuildMemberAvailable |
| GUILD_MEMBERS_CHUNK | 1271 | response to op 8 | H | GuildMembersChunk(members, guild, {index, count, notFound, nonce}) | Guild cached; adds presences |
| GUILD_ROLE_CREATE | 1345 | Guilds | A | GuildRoleCreate (`roleCreate`) | Emits if new |
| GUILD_ROLE_DELETE | 1371 | Guilds | A | GuildRoleDelete (`roleDelete`) | Cached only |
| GUILD_ROLE_UPDATE | 1358 | Guilds | H | GuildRoleUpdate (`roleUpdate`) | Guild and role cached |
| GUILD_SCHEDULED_EVENT_CREATE | 1393 | GuildScheduledEvents | H | GuildScheduledEventCreate | Guild cached |
| GUILD_SCHEDULED_EVENT_DELETE | 1419 | GuildScheduledEvents | A | GuildScheduledEventDelete | Partial `GuildScheduledEvent` |
| GUILD_SCHEDULED_EVENT_UPDATE | 1406 | GuildScheduledEvents | H | GuildScheduledEventUpdate(old?, new) | Guild cached |
| GUILD_SCHEDULED_EVENT_USER_ADD | 1432 | GuildScheduledEvents | A | GuildScheduledEventUserAdd(event, user) | Partial event and user |
| GUILD_SCHEDULED_EVENT_USER_REMOVE | 1449 | GuildScheduledEvents | A | GuildScheduledEventUserRemove(event, user) | Partial event and user |
| GUILD_SOUNDBOARD_SOUND_CREATE | 1466 | GuildExpressions | H | GuildSoundboardSoundCreate | Guild cached |
| GUILD_SOUNDBOARD_SOUND_DELETE | 1492 | GuildExpressions | A | GuildSoundboardSoundDelete | Partial `SoundboardSound` |
| GUILD_SOUNDBOARD_SOUND_UPDATE | 1479 | GuildExpressions | H | GuildSoundboardSoundUpdate(old?, new) | Guild cached |
| GUILD_SOUNDBOARD_SOUNDS_UPDATE | 1514 | GuildExpressions | H | GuildSoundboardSoundsUpdate(sounds, guild) | Keyed by `sound_id` |
| GUILD_STICKERS_UPDATE | 1162 | GuildExpressions | A | GuildStickerCreate, GuildStickerUpdate, GuildStickerDelete (`stickerCreate`, ...) | Diff of full list against cache |
| GUILD_UPDATE | 1056 | Guilds | A | GuildUpdate(old, new) | Cached only |
| INTEGRATION_CREATE | 1558 | GuildIntegrations | no handler | none | |
| INTEGRATION_DELETE | 1584 | GuildIntegrations | no handler | none | |
| INTEGRATION_UPDATE | 1571 | GuildIntegrations | no handler | none | |
| INTERACTION_CREATE | 1610 | none | A | InteractionCreate | Class by type; unknown type: debug, dropped. Component and message-command in a non-text channel: dropped |
| INVITE_CREATE | 1623 | GuildInvites | H | InviteCreate | Channel cached |
| INVITE_DELETE | 1701 | GuildInvites | H | InviteDelete | Channel cached |
| MESSAGE_CREATE | 1729 | GuildMessages, DirectMessages | A | MessageCreate | Partial channel. Non-text dropped. Already cached message: dropped unless authored by the client user. Thread counters updated first. `lastMessageId` set |
| MESSAGE_DELETE | 1804 | GuildMessages, DirectMessages | A | MessageDelete | Thread `messageCount--`; partial message |
| MESSAGE_DELETE_BULK | 1830 | GuildMessages | A | MessageBulkDelete (`messageDeleteBulk`) | Channel cached; emits only if at least one resolved |
| MESSAGE_POLL_VOTE_ADD | 2368 | GuildMessagePolls, DirectMessagePolls | A | MessagePollVoteAdd(answer, userId) | Partials `Poll`, `PollAnswer`; `voteCount++` |
| MESSAGE_POLL_VOTE_REMOVE | 2376 | GuildMessagePolls, DirectMessagePolls | A | MessagePollVoteRemove(answer, userId) | `voteCount` floors at 0 |
| MESSAGE_REACTION_ADD | 1876 | GuildMessageReactions, DirectMessageReactions | A | MessageReactionAdd(reaction, user, {type, burst}) | Partial user, channel, message, reaction; uses `member` when given |
| MESSAGE_REACTION_REMOVE | 1920 | GuildMessageReactions, DirectMessageReactions | A | MessageReactionRemove(reaction, user, {type, burst}) | No `member` in payload |
| MESSAGE_REACTION_REMOVE_ALL | 1928 | GuildMessageReactions, DirectMessageReactions | A | MessageReactionRemoveAll(message, removed) | |
| MESSAGE_REACTION_REMOVE_EMOJI | 1941 | GuildMessageReactions, DirectMessageReactions | A | MessageReactionRemoveEmoji | |
| MESSAGE_UPDATE | 1742 | GuildMessages, DirectMessages | A + H | MessageUpdate(old, new) | Non-text dropped; partial message |
| PRESENCE_UPDATE | 1959 | GuildPresences | H | UserUpdate, GuildMemberAvailable, PresenceUpdate(old?, new) | `user` may be partial. PresenceUpdate only if it has listeners and changed |
| RATE_LIMITED | 2411 | none | H | none | See 13.5 |
| READY | 493 | none | H | none directly | See 13.2 |
| RESUMED | 542 | none | no handler | none | Handled by the shard; whitelisted in the Client |
| SOUNDBOARD_SOUNDS | 1536 | response to op 31 | H | SoundboardSounds(sounds, guild) | Keyed by `sound_id` |
| STAGE_INSTANCE_CREATE | 1972 | Guilds | A | StageInstanceCreate | Channel resolved |
| STAGE_INSTANCE_DELETE | 1985 | Guilds | A | StageInstanceDelete | Channel resolved |
| STAGE_INSTANCE_UPDATE | 1998 | Guilds | A | StageInstanceUpdate(old?, new) | Channel resolved |
| SUBSCRIPTION_CREATE | 720 | none | H | SubscriptionCreate | |
| SUBSCRIPTION_DELETE | 746 | none | H | SubscriptionDelete | Built uncached, removed |
| SUBSCRIPTION_UPDATE | 733 | none | H | SubscriptionUpdate(old?, new) | |
| THREAD_CREATE | 2064 | Guilds | A | ThreadCreate(thread, newlyCreated) | Emits only if not already cached; `newly_created ?? false` |
| THREAD_DELETE | 2095 | Guilds | H | ThreadDelete | Cached only |
| THREAD_LIST_SYNC | 2011 | Guilds | H | ThreadListSync(threads, guild) | Removes non-archived cached threads in scope first. Member rows use thread id in `id` |
| THREAD_MEMBERS_UPDATE | 2024 | Guilds, GuildMembers for others | A | ThreadMembersUpdate(added, removed, thread) | Skips emit when both are empty |
| THREAD_MEMBER_UPDATE | 2037 | Guilds | H | ThreadMemberUpdate(old, new) | Uncached member: added, **no emit**. `id` is the thread id |
| THREAD_UPDATE | 2082 | Guilds | A + H | ThreadUpdate(old, new) | Uses the ChannelUpdate action |
| TYPING_START | 2127 | GuildMessageTyping, DirectMessageTyping | A | TypingStart | Non-text: Warn event, dropped |
| USER_UPDATE | 2163 | none | A | UserUpdate(old, new) | Emits only if changed |
| VOICE_CHANNEL_EFFECT_SEND | 2173 | GuildVoiceStates | H | VoiceChannelEffectSend | Guild cached |
| VOICE_CHANNEL_START_TIME_UPDATE | 2259 | unverified | no handler | none | |
| VOICE_CHANNEL_STATUS_UPDATE | 2233 | unverified | no handler | none | |
| VOICE_SERVER_UPDATE | 2298 | none | H | Debug, VoiceServerUpdate({guildId, endpoint, token}) | Token masked in Debug. Forwarded to the voice adapter. `endpoint` may be null |
| VOICE_STATE_UPDATE | 2285 | GuildVoiceStates | H | Debug, VoiceStateUpdate(old, new) | Guild cached; self update forwarded to voice adapter |
| WEBHOOKS_UPDATE | 2328 | GuildWebhooks | H | WebhooksUpdate(channel) | Channel cached |

A low-level port must expose all 79, including the 7 discord.js ignores. `@discordjs/core` re-emits every dispatch by `t` (`core/src/client.ts:221`).

Payload quirks in the types:

- `GUILD_CREATE` adds `joined_at`, `large`, `unavailable?`, `member_count`, `voice_states`, `members`, `channels`, and more (`:964`). `voice_states` have no `guild_id`.
- `READY.guilds` are `APIUnavailableGuild` (`:516`). `application` is partial (`:536`).
- `GUILD_DELETE.unavailable` is `true` or absent (`:1074`): absent means removed, not an outage.
- `VOICE_SERVER_UPDATE.endpoint: null` means the server is being reallocated (`:2306-2325`).

## 13. discord.js client layer

### 13.1 Packet flow (`Client.js:405-428`)

```
dispatch -> _handlePacket(packet, shardId)
  if status != Ready and t not in BeforeReadyWhitelist:
      push to incomingPacketQueue
  else:
      if queue non-empty: shift one, setImmediate(_handlePacket(it))
      PacketHandlers[t]?.(client, packet, shardId)
      if t == READY: await _checkReady()
      elif status == WaitingForGuilds and t in {GUILD_CREATE, GUILD_DELETE}:
          expectedGuilds.delete(d.id); await _checkReady()
```

`BeforeReadyWhitelist` (`:38-46`): READY, RESUMED, GUILD_CREATE, GUILD_DELETE, GUILD_MEMBERS_CHUNK, GUILD_MEMBER_ADD, GUILD_MEMBER_REMOVE.

### 13.2 Readiness (`READY.js`, `Client.js:337-378`)

READY handler:

1. Creates or patches `client.user`.
2. For each guild: add to `expectedGuilds`, set `shardId`, add to cache.
3. Creates or patches `client.application`.
4. `status = WaitingForGuilds`.

`_checkReady`:

1. Clear the timer.
2. If `expectedGuilds` is empty and every shard's status is Ready: trigger ready.
3. Otherwise arm a timer: `waitGuildTimeout` (default 15 000 ms) with the Guilds intent, 0 without. When it fires, trigger ready regardless of what is still missing.

`_triggerClientReady`: `status = Ready`, set `readyTimestamp`, emit `clientReady`.

Default `status` starts as `Idle` (`:130`).

### 13.3 Guild availability

| Packet | Cached? | Condition | Result |
| --- | --- | --- | --- |
| GUILD_CREATE | Yes | was unavailable, now available | patch, GuildAvailable |
| GUILD_CREATE | Yes | otherwise | nothing, **no patch** |
| GUILD_CREATE | No | client Ready | add, GuildCreate |
| GUILD_CREATE | No | client not Ready | add, no event |
| GUILD_DELETE | No | | nothing |
| GUILD_DELETE | Yes | `unavailable` set | `available = false`, GuildUnavailable |
| GUILD_DELETE | Yes | not set | remove channels, destroy voice adapter, remove guild, GuildDelete |

### 13.4 Consumers of `ws.send`

| Caller | Op | Details |
| --- | --- | --- |
| `structures/ClientPresence.js:24-37` | 3 | Broadcast to managed shards, or a given shard or list |
| `managers/GuildMemberManager.js:238-310` | 8 | See below |
| `managers/GuildManager.js:167-215` | 31 | Grouped by shard; 10 s inactivity timeout |
| `structures/Guild.js:1515-1525` | 4 | Voice adapter `sendPayload`, routed by `guild.shardId` |

Op 43 has no caller in discord.js.

`_fetchMany`:

- Nonce defaults to a generated snowflake; over 32 characters throws.
- `query = initialQuery ?? (users ? undefined : '')`, `limit` default 0.
- Timeout 120 s, refreshed by each matching chunk.
- Completes when any holds: chunk `size < 1000`; `limit` reached; received chunk count equals `chunk.count`.
- Single non-array user resolves to the member rather than a collection.
- `RATE_LIMITED` with the same nonce rejects with `GatewayRateLimitError`.

Presence `_parse` (`ClientPresence.js:47-85`):

- Defaults: `afk: false`, `since: null`, `status: status ?? current`.
- Activity `type` defaults to Playing.
- A Custom activity with no `state` moves `name` into `state`, and `name` becomes `'Custom Status'`.
- When `activities` is omitted but status, afk or since is set, the current activities are re-sent.

### 13.5 Rate limit reporting

`RATE_LIMITED.js`: op 8 is silent, since callers handle it. Any other opcode warns once per opcode per process. `GatewayRateLimitError` carries `data` and the original `payload`.

### 13.6 What the Client does not do

`_attachEvents` (`Client.js:385-396`) listens only to `debug`, `dispatch` and `heartbeat` on the manager.

- It has no listener for `error`, `closed`, `resumed`, `hello` or `socketError`.
- There are no `shardReady`, `shardDisconnect`, `shardResume` or `shardError` client events.
- `Events.Invalidated` is defined but has no emitter in `src`.
- `options.closeTimeout` is documented and defaulted, but nothing reads it.

Other gateway users of the manager:

- `ClientVoiceManager.js:26-34`: on `closed` with code 1000, destroys adapters for guilds of that shard.
- `ShardClientUtil.js:36-66`: forwards `closed` and `resumed` to the sharding manager.

### 13.7 Login

`Client.js:312-330`:

1. Reject a missing token.
2. Strip `/^bot\s*/i`.
3. Fetch `GET /gateway/bot`.
4. `ws.connect`.
5. On any error: `destroy()` and rethrow.

### 13.8 Partials

`Partials`: User, Channel, GuildMember, Message, Reaction, GuildScheduledEvent, ThreadMember, Poll, PollAnswer, SoundboardSound.

`Action.getPayload` (`Action.js:29-31`) builds a stub with `manager._add` when the partial is enabled, and otherwise looks in the cache, with a miss dropping the event. **DM messages depend on this**: without `Partials.Channel`, an uncached DM channel means `MESSAGE_CREATE` is dropped.

`Action.getChannel` (`:33-55`) synthesizes `recipients` for DM and group DM.

## 14. Subtle behaviour the port must preserve

### Connection and URL

1. **Appending the query to the resume URL.** `v`, `encoding`, `compress` must be on the resume URL as well (`WebSocketShard.ts:281`). Both Discord URLs arrive without a query; concatenate, or parse and set params.
2. **A fresh decompressor per connection, with a fresh limiter.** Reusing a zlib or zstd context across connections corrupts the stream.
3. **Hello before identify or resume.** Nothing is sent before Hello.
4. **The fixed 500 ms pause before each reconnect** (`:409`). Upstream's comment explains it prevents a draining loop when connection fails immediately. Treat it as a floor (see 15.11).
5. **Custom close code 4200 for resumable closes.** The frame must carry a non-1000 code, or the session is lost.

### Sequence and dispatch

6. **Sequence advances for every dispatch, before parsing the body into a typed event.** Unknown `t` values and bodies that fail to deserialize must still advance it (`:779-790`), or resume replays or misses events. Upstream gets this for free by not parsing. A Rust port must split envelope parsing from typed parsing, and keep unknown events as `Unknown { name, raw }`.
7. **Sequence is monotonic** (`:781`).
8. **Session stored before `ready` and `dispatch` emission** (`:761-763`).
9. **`READY` and `RESUMED` delivered as dispatch too**, after their dedicated events.
10. **Heartbeat `d` null before the first sequence**, and the stored sequence even during a resume.
11. **Unknown opcodes ignored**, not errors. Unparseable text frames dropped (`:632-639`).

### Heartbeat

12. **Jittered first beat, in `[0, interval)`**, then fixed interval.
13. **Heartbeat during the identify wait.** The throttle can take many multiples of 5 s, and the socket must stay alive.
14. **Server op 1: immediate beat that skips the ack check**, but still expects an ack.
15. **Zombie: one missed ack, detected at the next tick**, recovers by resume with code 4200.

### Send

16. **Opcodes 1, 2, 6 bypass the limiter and the status gate.** Otherwise a saturated limiter would kill the connection, or identify would deadlock waiting for the Ready it causes.
17. **115 per 60 s per connection**, with the difference to 120 reserved for those opcodes.
18. **Other opcodes wait until the shard is Ready, in FIFO order**, and survive a reconnect: a payload pending on the limiter moves to the next connection instead of failing (`:501-505`).
19. **No opportunistic drop.** A send only fails when the shard has never had a connection.

### Identify and resume

20. **Resume only when `shardCount` matches** (`:316`).
21. **Bucket by `shardId % max_concurrency`, 5 s per bucket, FIFO**, with up to 1.5 s jitter. The test pins bucket independence.
22. **An abort while queued frees the position.** A shard whose socket closes while queued must re-queue on the next connection and must not send on the old one.
23. **Op 9 `d: true` resumes on the same socket; `d: false` opens a new socket and identifies** (`:808-821`).
24. **`session_start_limit.remaining` checked once, at `connect`**, against the number of managed shards.
25. **`shard` always sent, `capabilities` and `compress` always sent, `large_threshold` and `presence` only when set.**

### Close handling

26. **The table in section 7, including its surprises:** 4009 resumes, 4003/4005/4007/4008 clear the session, 4200 from the peer does nothing.
27. **Six fatal codes stop the shard permanently and surface an error.** `connect()` must fail with that error when it happens before the first ready. `Client.login` then destroys everything and rethrows.
28. **`closed` observed before `error`** (`:868`, `:904-908`). `waitForEvent` depends on the order to avoid a reconnect after a fatal code. A single-owner state machine gets this by construction; keep the public event order.
29. **Any socket error on the connection converts an unknown close from resume to reconnect** (`:864`, `:979-984`). Note the misleading name: `failedToConnectDueToNetworkError` is set for any socket error, including one on a long-established session. Kept as behaviour here because it is what upstream does, though 16.3 raises the option of narrowing it.

### Compression

30. **zlib end-of-message is the 4-byte suffix on a WebSocket message boundary**, and multiple WebSocket messages may form one gateway message.
31. **zstd: each WebSocket message is one complete gateway message, in one long stream.** Do not reset the context between messages, and do not expect a frame end per message.
32. **Identify compression: frames are standalone, text frames remain possible.** Dispatch on frame type, not on a mode alone.
33. **Transport compression wins over identify compression.**

### Client layer

34. **Whitelist and queue before ready** (13.1).
35. **`clientReady` can fire repeatedly.** A later re-identify runs the READY handler, which sets `WaitingForGuilds`, so non-whitelisted events queue again until `_checkReady` completes (`READY.js`, `Client.js:421`). A resume does not do this.
36. **The ready timer forces ready.** With the Guilds intent absent it is immediate, even with other shards still connecting.
37. **GUILD_CREATE emits GuildCreate only once the client is Ready.** Initial guilds are silent.
38. **GUILD_DELETE with `unavailable` set is an outage, not a removal.**
39. **Cache-gated emission.** Most events are dropped silently on a cache miss unless the partial is enabled (section 12).
40. **`THREAD_MEMBER_UPDATE` and `THREAD_LIST_SYNC` member rows use `id` for the thread id.** Stated by upstream comments.
41. **Member chunk completion rule** (13.4), and the nonce limit of 32.
42. **Client-level defaults:** `largeThreshold` 50, `waitGuildTimeout` 15 000 ms, member fetch timeout 120 s, soundboard fetch timeout 10 s.
43. **Masked secrets.** The voice token is masked in debug output, and the token is not logged in connection messages. The Rust `Debug` impls must redact.

## 15. Upstream defects: do not reproduce

All are **[inferred]** from reading the code. They are listed so the port avoids them deliberately, and so nobody "fixes" the port back to them.

1. **Send wedged during a resume.** A non-important send while not Ready waits only for `ready` (`:475`). After a resume, `resumed` fires instead, so the payload hangs until the next full READY or an `error` event. Wait for either.
2. **Send queue leak.** In the limiter sleep race, `once(closed)` rejects on `error`, the `await` throws, and `sendQueue.shift()` never runs (`:496-499`). All later throttled sends on that shard hang.
3. **Identify continues after failure.** When `waitForIdentify` throws an unknown error, `destroy` runs but there is no `return`, so execution falls through to send an identify (`:535-549`). Since `destroy` with recovery also reconnects, this can double-identify.
4. **`destroy` during a wait can schedule a reconnect.** `destroy` aborts the `waitForEvent` controllers (`:353-355`). The catch branch then starts a second `destroy` with `recover: Reconnect` (`:436-440`). A user-initiated destroy while waiting for Hello or Ready can therefore come back to life, and clear a session it meant to keep. Two concurrent recoveries can also hit "wasn't idle" as an unhandled rejection.
5. **Close wait without timeout.** `destroy` awaits the close callback with no timer of its own (`:383-394`). On a dead TCP connection, which is exactly the zombie case, it relies on the `ws` library's internal close timer. **[external]** I recall that as 30 s. A port must bound it. `options.closeTimeout` in the Client looks intended for this but is unused.
6. **Socket left open in CONNECTING.** `destroy` only closes when OPEN (`:375`). A timed-out handshake loses its handlers but is not torn down.
7. **Leaked interval.** If `destroy` runs during the first `await this.heartbeat()`, `setInterval` is armed afterwards on a dead connection (`:840-843`).
8. **Out-of-order dispatch.** `onMessage` runs concurrently per message with a differing number of awaits. READY does an extra `await updateSessionInfo`, and identify-compression inflate is off-thread, so a later message can overtake. With an async session store this widens. The Rust per-shard loop must be strictly ordered.
9. **Session store round trip per dispatch and per heartbeat.** Costly with a remote store or in worker mode. Keep the sequence in memory, and write through.
10. **Process-wide default session store** (`constants.ts:28`). Two managers in one process, for example two tokens, share entries keyed only by shard id, and can try to resume each other's sessions. Make it per manager.
11. **No backoff, no identify budget.** A persistent failure retries every 500 ms plus timeouts. **[external]** Discord limits session starts per day (`session_start_limit`), and repeated exhaustion can reset the token. Add jittered exponential backoff above the 500 ms floor, and treat `remaining` as a budget.
12. **Decompression failure does not recover.** Once a stream context is corrupt, every later frame fails, yet only `error` is emitted. Inflate-then-parse errors are also uncaught: `parseInflateResult` in `void onMessage` and `JSON.parse` in the inflate callback (`:649-656`). In Rust: treat as connection failure, destroy with Reconnect.
13. **Peer close 4200 does nothing** (`:879-881`). The shard stays non-Idle with no socket. Treat like Resume.
14. **Non-abortable throttle sleep** (`SimpleIdentifyThrottler.ts:40`). The slot is consumed for a shard that has already closed. Pass the signal through.
15. **Throttler double initialization.** `ensureThrottler` checks then awaits, so a slow async `buildIdentifyThrottler` can create two throttlers and break the limit (`SimpleContextFetchingStrategy.ts:11-21`). The same pattern is in `WorkerShardingStrategy.ts:369-372`. Use a once-cell.
16. **Client cannot log in twice.** `login` calls `ws.setToken`, which throws if the token is already set, and `destroy` does not clear it (`Client.js:321`, `WebSocketManager.ts:360-366`).
17. **Lazy, reordering queue drain.** `_handlePacket` drains the queue one packet at a time, only when another packet arrives, and processes the new packet first (`Client.js:409-418`). So queued packets stay until the next event, and order is inverted. While not ready, shifted packets just re-queue at the tail. Flush in order once ready.
18. **Premature ready with many shards.** When a shard takes over `waitGuildTimeout` to connect, `clientReady` fires with shards missing, then again later (`Client.js:361-376`).
19. **Fatal error delivery is uncertain.** Nothing in `Client` listens to manager `error`, so after login a fatal close has no clear public signal. **[external]** `AsyncEventEmitter` throws on an unhandled `error`, as Node's does. Rust should expose a typed event and a per-shard terminal status.
20. **Unlistened requests.** `client.ws.send` calls for member and soundboard fetches are not awaited, so a rejection is unhandled, and the worker `send` swallows failures entirely.

## 16. Rust mapping

Suggestions for the design phase. Crate names are candidates; no version or API was checked.

### 16.1 Shard task

One task owns the socket and all state, in a `select!` over:

- socket read
- heartbeat timer
- command channel (send, destroy, status)
- identify gate, as a pending future
- the Hello, Ready and close deadlines

This removes defects 4, 7 and 8 from section 15 by construction, and makes `closed` ordering explicit.

```rust
enum ShardStatus { Idle, Connecting, Resuming, Ready }
enum Recovery { Reconnect, Resume }

struct SessionInfo {
    resume_url: String,
    sequence: u64,
    session_id: String,
    shard_count: u32,
    shard_id: u32,
}
```

Keep the limiter queue as data, a `VecDeque` of pending payloads, drained when Ready and within budget. A pending payload then survives reconnects without any special case.

**Backpressure trap.** If the task awaits a bounded event channel inside the read arm, heartbeats stall and Discord drops the shard. Pick one explicitly:

- split the writer and heartbeat into their own task,
- use an unbounded channel,
- or a bounded channel with a documented policy.

Do not block the heartbeat on user code.

### 16.2 Traits

| Upstream | Rust |
| --- | --- |
| `IIdentifyThrottler` | `trait IdentifyThrottler { async fn wait(&self, shard_id: u32) -> Result<(), ...>; }`, cancellation by dropping the future |
| `retrieveSessionInfo`, `updateSessionInfo` | `trait SessionStore`, default in-memory per manager |
| `IContextFetchingStrategy` | Probably merged into the shard's constructor arguments |
| `IShardingStrategy` | One in-process implementation first. The trait is only needed for external hosting |
| `WorkerShardingStrategy` | Not needed: tasks on a multi-threaded runtime. Optionally a dedicated runtime per group |
| Multi-process | `shard_ids` + `shard_count` + custom throttler and store |

Upstream's `AbortSignal` contract (reject with an abort error) becomes future drop. Make the implementations cancel-safe, specifically so that a dropped queue waiter gives up its position and a holder releases the bucket.

### 16.3 Decisions needed

1. **Unknown close after a socket error:** upstream clears the session (item 29). Keeping the session and trying resume seems better, since Discord would answer op 9 if it were invalid. Costs nothing but one round trip.
2. **4009:** upstream resumes. **[external]** Discord's docs say to start a new session. Resume then op 9 reaches the same place with one more round trip.
3. **Op 9 `d: true`:** upstream resumes at once. **[external]** Discord's docs suggest a wait of 1 to 5 s. A short jitter is harmless.
4. **ETF:** skip for the first release. JSON matches upstream, and serde covers it. A feature flag can come later.
5. **Hung resume:** add an equivalent of `readyTimeout` for RESUMED, which upstream lacks.

### 16.4 Rust-specific traps

- **Message size limits.** **[external]** `tungstenite` defaults, as I recall, to a 64 MiB message and 16 MiB frame limit. Large uncompressed payloads should be tested, and the limit made configurable.
- **Monotonic time.** Use `Instant` for latency and windows. `SystemTime` only for display.
- **Close reason length.** A WebSocket close reason must be at most 123 bytes, and a user-provided one must be truncated or rejected.
- **zlib.** Use one `flate2::Decompress` per connection. Feed the accumulated message, loop with `FlushDecompress::Sync` until input is consumed and output stops growing, and reuse the output buffer. Do not reuse the context across connections.
- **zstd.** Use a streaming `DCtx` kept for the whole connection, flushing output after each message. A one-shot decompress per message will fail because later messages reference earlier windows.
- **Serialization.**
  - Heartbeat `d: null` must be emitted.
  - Optional fields use `skip_serializing_if`.
  - Op 11 and op 7 may carry no `d`; op 9 carries a bool; op 1 from the server has null.
  - Parse an envelope `{op, s, t, d: &RawValue}`, then parse `d` by `op` and `t`.
  - Snowflakes arrive as strings.
- **Tokens.** Redacted `Debug`, never in URLs or logs.
- **Feature flags.** `zlib` and `zstd`, so a build can omit a C dependency. This matters on Windows.

## 17. Conformance tests

### Ported from upstream `packages/ws/__tests__`

| File | Case |
| --- | --- |
| `ws/WebSocketManager.test.ts` | `connect` requires gateway information |
| | gateway information unavailable before connecting |
| | shard count with no count or ids; with count; with id array; with id range |
| | update shard count |
| | both `shardIds` and `shardCount` (`{2..3}`, count 4 gives `[2, 3]` and 4) |
| | capabilities default to 0; provided bitfield |
| | strategy delegation; gateway information cleared on destroy |
| `util/SimpleIdentifyThrottler.test.ts` | separate buckets do not wait; same bucket does; negative `diff` does not sleep |
| `strategy/WorkerShardingStrategy.test.ts` | spawn, connect, send, session info, destroy |
| `strategy/WorkerContextFetchingStrategy.test.ts` | session info round trip |

Upstream has **no** direct test of `WebSocketShard`.

### Needed for the port, with a mock gateway

1. Hello, then identify with the exact JSON. Jittered first beat within the interval. Beats continue during a throttle wait.
2. Each close code from section 7 gives the expected action and session state.
3. Op 7, and op 9 with true and false.
4. Missed ack triggers close 4200, then resume on `resume_gateway_url` with `v`, `encoding`, `compress` in the query.
5. Unknown `t` and malformed known bodies advance the sequence. The next heartbeat and resume carry it.
6. Limiter: 114 throttled sends pass, the next waits. Opcodes 1, 2, 6 bypass. Window resets per connection. Pending sends move to the next connection and finish after RESUMED, which is the fix for 15.1.
7. zlib: one gateway message split over several WebSocket messages; several gateway messages sharing a context; new context after reconnect. zstd: the same, with captured vectors.
8. Identify compression with a mix of text and binary frames.
9. Fatal close before the first ready fails `connect()`. After it, a terminal status plus event.
10. `destroy` during Hello wait, Ready wait, throttle wait and limiter sleep never reconnects. Bounded close wait on a silent peer.
11. Client: queue order on ready, re-identify re-entering `WaitingForGuilds`, ready timer behaviour, GUILD_CREATE and GUILD_DELETE matrix, member chunk completion, RATE_LIMITED rejection by nonce.

## 18. Open points

1. `@vladfrangu/async_event_emitter` and `ws` sources were not available. Claims about `emit('error')` with no listeners, `ws`'s close timer and synchronous message delivery are from memory.
2. No intent mapping exists in the cloned sources. The Intent column in section 12 is from memory of Discord's docs, and the intents for `VOICE_CHANNEL_STATUS_UPDATE` and `VOICE_CHANNEL_START_TIME_UPDATE` are unverified.
3. `dat:` documents `CHANNEL_INFO` and the voice channel events, but neither discord.js nor these notes confirm delivery rules beyond the types.
4. The races in section 15 are from code reading and were not reproduced.
