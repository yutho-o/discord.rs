# packages/voice: port map for discord.rs

Source: `@discordjs/voice` 0.19.2 (`packages/voice`, Apache-2.0), read from the local clone. Date: 2026-11-07.
All paths are relative to `packages/voice/` unless they start with another package name.

## 0. How to read this report

Evidence tags:

- **[src]** read directly in the cloned source, or in the prism-media 1.3.5 npm tarball (dependency, not in the clone) or a downloaded crate.
- **[test]** asserted by an upstream test.
- **[ext]** taken from outside the repo (Discord docs, daveprotocol.com, web search). Not confirmed against a live gateway.
- **[inf]** my inference from reading the code. Nothing here was run against Discord.

What was and was not verified:

- Nothing from the clone was executed.
- `cargo search` and `cargo info` were run. No crate was compiled, so **no build on Windows or rustc 1.99 is confirmed**.
- The transformer routing table in section 8 comes from a standalone re-implementation of the search (my own script), not from running upstream code.
- The upstream `VoiceWebSocket` tests are all `describe.skip` ("TODO: Fix voice tests"), and no test covers `DAVESession` or `Networking`. Those two areas rest on code reading alone.

## 1. Package map

| File | LOC | Role | Rust target |
|---|---:|---|---|
| `src/VoiceConnection.ts` | 827 | Connection lifecycle state machine, public handle | `connection.rs` |
| `src/networking/Networking.ts` | 812 | Handshake state machine, RTP build, transport encryption, DAVE dispatch | `networking/mod.rs` |
| `src/networking/DAVESession.ts` | 395 | Transition/epoch/failure policy around `@snazzah/davey` | `networking/dave.rs` |
| `src/networking/VoiceWebSocket.ts` | 236 | WS wrapper, heartbeats, seq tracking, binary framing | `networking/ws.rs` |
| `src/networking/VoiceUDPSocket.ts` | 173 | UDP socket, keep-alive, IP discovery | `networking/udp.rs` |
| `src/audio/AudioPlayer.ts` | 651 | Player state machine, prepare/dispatch steps | `audio/player.rs` |
| `src/audio/AudioResource.ts` | 296 | Resource, silence padding, pipeline assembly | `audio/resource.rs` |
| `src/audio/TransformerGraph.ts` | 281 | `StreamType`, cost graph, path search, FFmpeg args | `audio/graph.rs` |
| `src/audio/PlayerSubscription.ts` | 32 | Connection-to-player link | `audio/subscription.rs` |
| `src/audio/AudioPlayerError.ts` | 19 | Error plus the resource | `error.rs` |
| `src/DataStore.ts` | 187 | Connection registry, global 20 ms clock, join payload | `manager.rs`, `audio/clock.rs` |
| `src/joinVoiceChannel.ts` | 78 | Entry point and options | `lib.rs` |
| `src/receive/VoiceReceiver.ts` | 261 | UDP parse/decrypt, subscriptions | `receive/receiver.rs` |
| `src/receive/AudioReceiveStream.ts` | 139 | Per-user packet stream, end behaviours | `receive/stream.rs` |
| `src/receive/SSRCMap.ts` | 108 | SSRC to user map with events | `receive/ssrc.rs` |
| `src/receive/SpeakingMap.ts` | 64 | Packet-driven speaking detection | `receive/speaking.rs` |
| `src/util/demuxProbe.ts` | 129 | Ogg/WebM sniffing | `audio/probe.rs` |
| `src/util/Secretbox.ts` | 127 | Picks an XChaCha library at runtime | Dropped, the crate is a compile-time dependency |
| `src/util/entersState.ts`, `abortAfter.ts` | 66 | Wait for status with timeout or abort | `util.rs` |
| `src/util/adapter.ts` | 52 | Gateway adapter contract | `adapter.rs` |
| `src/util/generateDependencyReport.ts` | 93 | Diagnostics | Small function: features plus FFmpeg probe |
| `src/util/constants.ts` | 1 | `RTP_OPUS_PAYLOAD_TYPE = 0x78` | `constants.rs` |

Integration points outside this package, all [src]:

- `packages/discord.js/src/client/voice/ClientVoiceManager.js`
- `packages/discord.js/src/structures/Guild.js:1514` (`voiceAdapterCreator`)
- `packages/discord.js/src/client/websocket/handlers/VOICE_STATE_UPDATE.js:27` and `VOICE_SERVER_UPDATE.js:11`

External runtime dependencies (`package.json`): `@snazzah/davey ^0.1.12`, `prism-media ^1.3.5`, `ws ^8.21.3`, `discord-api-types ^0.38.54`.

Optional, discovered at runtime: an Opus library (`@discordjs/opus` or `opusscript`), a libsodium-style cipher library, FFmpeg.

Node's own `crypto` provides AES-256-GCM. There is no retry library, no scheduler and no rate limiter.

## 2. Public API to expose

From `src/index.ts`.

**Functions**

- `joinVoiceChannel`, `getVoiceConnection`, `getVoiceConnections`, `getGroups`
- `createAudioPlayer`, `createAudioResource`, `demuxProbe`, `validateDiscordOpusHead`
- `entersState`, `generateDependencyReport`

**Classes**

- `VoiceConnection`, `AudioPlayer`, `AudioResource`, `PlayerSubscription`, `AudioPlayerError`
- `VoiceReceiver`, `AudioReceiveStream`, `AudioPacket`, `SSRCMap`, `SpeakingMap`
- Lower level, also exported: `Networking`, `VoiceUDPSocket`, `VoiceWebSocket`, `DAVESession`

**Enums and constants**

- `VoiceConnectionStatus`, `VoiceConnectionDisconnectReason`, `NetworkingStatusCode`
- `AudioPlayerStatus`, `NoSubscriberBehavior`, `StreamType`, `TransformerType`, `EndBehaviorType`
- `SILENCE_FRAME` (`F8 FF FE`)

**Defaults**

| Option | Default | Note |
|---|---|---|
| `selfDeaf` | `true` | |
| `selfMute` | `false` | The docstring says "defaults to true". The code wins (`src/joinVoiceChannel.ts:53-68`). |
| `group` | `'default'` | |
| `daveEncryption` | `true` | |
| `decryptionFailureTolerance` | `36` | The docstring says 24. The constant wins (`src/networking/DAVESession.ts:45`). |
| `debug` on connection | off | |
| `debug` on player | **on** unless exactly `false` | Inverted compared to everything else (`src/audio/AudioPlayer.ts:250`). |
| `behaviors.noSubscriber` | `Pause` | |
| `behaviors.maxMissedFrames` | `5` | |
| `silencePaddingFrames` | `5` | |
| `inlineVolume` | `false` | |
| `demuxProbe` size | 1024 bytes | |
| Receive `end` | `Manual` | |

## 3. VoiceConnection state machine

States (`src/VoiceConnection.ts:23-48`): `Signalling`, `Connecting`, `Ready`, `Disconnected`, `Destroyed`.

Disconnect reasons: `WebSocketClose` (carries `closeCode`), `AdapterUnavailable`, `EndpointRemoved`, `Manual`.

### Transitions [src]

| From | Trigger | To | Notes |
|---|---|---|---|
| (new) | constructor | Signalling | `adapterCreator` runs before `_state` is assigned |
| (new) | `createVoiceConnection`, `sendPayload` false | Disconnected(AdapterUnavailable) | Already tracked in the registry |
| any but Destroyed | `VOICE_SERVER_UPDATE` with endpoint, and state packet known | Connecting | Replaces Networking. **Also reconnects from Disconnected and Ready.** |
| any but Destroyed | `VOICE_SERVER_UPDATE` with null endpoint | Disconnected(EndpointRemoved) | Networking kept, see 12.A5 |
| Signalling | `VOICE_STATE_UPDATE` with `channel_id`, server packet known | Connecting | Race fix, gh #11553 |
| Connecting or Ready | Networking enters Ready | Ready | Resets `rejoinAttempts` to 0 |
| Connecting or Ready | Networking enters any non-Closed code | Connecting | So a resume shows as Ready to Connecting |
| any but Destroyed | Networking `close` with 4014, 4021, 4022 | Disconnected(WebSocketClose, code) | No automatic rejoin |
| any but Destroyed | Networking `close` with any other code | Signalling | `rejoinAttempts++`, sends join payload |
| Signalling (just entered) | that join send fails | Disconnected(AdapterUnavailable) | |
| not Destroyed or Signalling | `disconnect()`, send OK | Disconnected(Manual) | Sets `joinConfig.channelId = null`, **drops the subscription and Networking**, returns true |
| not Destroyed or Signalling | `disconnect()`, send fails | Disconnected(AdapterUnavailable) | **Keeps the subscription**, drops Networking, returns false |
| not Ready, not Destroyed | `rejoin(cfg)`, send OK | Signalling | `rejoinAttempts++` before sending |
| Ready | `rejoin(cfg)`, send OK | Ready | No state change, no counter change [test] |
| not Destroyed | `rejoin(cfg)`, send fails | Disconnected(AdapterUnavailable) | Keeps subscription, drops Networking |
| any but Destroyed | `destroy(adapterAvailable = true)` | Destroyed | Untracks, sends leave (`channel_id: null`, result ignored), then `adapter.destroy()` from the setter |
| any but Destroyed | adapter calls `destroy()` | Destroyed | Same with `adapterAvailable = false` |
| Destroyed | `destroy()` | throws | `Cannot destroy VoiceConnection - it has already been destroyed` |

### State setter order (`:297-342`)

This order is observable, so keep it:

1. If Networking changed, detach listeners (installing a no-op error handler) and destroy the old one, then rebind the receiver to the new one.
2. If the new state is Ready, reset `rejoinAttempts`. If it is Destroyed, destroy every receive stream.
3. On entry to Destroyed, call `oldState.adapter.destroy()`.
4. Assign `_state`.
5. If the subscription changed, call `oldSubscription.unsubscribe()`.
6. Emit `stateChange(old, new)`.
7. Emit the status-named event, only if the status actually changed.

### Other behaviours

- `addStatePacket` always updates `selfDeaf`, `selfMute`, and `channelId` if non-null. A null `channel_id` deliberately does nothing: the comment says it may be a network failure, handled when the WS dies.
- `createVoiceConnection` with an existing live connection reuses it. It ignores the new `adapterCreator`, `debug`, `daveEncryption` and tolerance. If Disconnected it calls `rejoin`. Otherwise it only sends the join payload, and `joinConfig` updates when the state packet returns.
- `ping` returns `{ ws, udp }`. `udp` is **always undefined** because `VoiceUDPSocket.ping` is deprecated and never written (`src/networking/VoiceUDPSocket.ts:83-85`).
- Registry: `Map<group, Map<guildId, VoiceConnection>>`, global, with `default` pre-created.

**Port design:** make the registry a `VoiceManager` value owned by the client rather than a process global.

## 4. Networking

### 4.1 State machine (`src/networking/Networking.ts:33-41`)

```
OpeningWs -> Identifying -> UdpHandshaking -> SelectingProtocol -> Ready <-> Resuming
any -> Closed
```

| From | Trigger | To | Side effects |
|---|---|---|---|
| (new) | constructor | OpeningWs | Opens `wss://{endpoint}?v=8` |
| OpeningWs | WS open | Identifying | Sends op 0 |
| Identifying | op 2 Ready | UdpHandshaking | Creates UDP, starts discovery, `connectedClients = {}` |
| UdpHandshaking | discovery resolved | SelectingProtocol | Sends op 1 |
| SelectingProtocol | op 4 | Ready | Creates DAVE session, random `sequence` (16 bit) and `timestamp` (32 bit), `nonce = 0`, `speaking = false`, `packetsPlayed = 0` |
| Ready | WS close, code `< 4000` or `4015` | Resuming | New WS with `sequence` carried over. UDP, DAVE and keys kept. |
| Ready | UDP closed | Resuming | Effectively dead code, see 12.A10 |
| Resuming | WS open | (same) | Sends op 7 |
| Resuming | op 9 | Ready | Forces `speaking = false` so op 5 is resent on the next packet |
| any other | WS close | Closed, then emits `close(code)` | Only one resume attempt: a close while Resuming goes straight to Closed |

The setter destroys `ws`, `udp` and `dave` whenever the object reference changes, after detaching listeners.

### 4.2 Voice gateway protocol (v8)

URL: `wss://${endpoint}?v=8` with the endpoint used as given, no port stripping [src]. The docs' example endpoint includes `:2048` [ext].

| Op | Name | Dir | Format | Handling [src] |
|---|---:---|---|---|
| 0 | Identify | C to S | JSON | `server_id, user_id, session_id, token, max_dave_protocol_version` (0 if DAVE disabled) |
| 1 | SelectProtocol | C to S | JSON | `protocol: 'udp', data: { address, port, mode }` |
| 2 | Ready | S to C | JSON | Reads `ip, port, ssrc, modes` only when Identifying. `heartbeat_interval` in this payload is erroneous per docs [ext] and is not read. |
| 3 | Heartbeat | C to S | JSON | `{ t: Date.now(), seq_ack }` |
| 4 | SessionDescription | S to C | JSON | `mode, secret_key (number[]), dave_protocol_version`, only when SelectingProtocol |
| 5 | Speaking | both | JSON | Send `{ speaking: 0 or 1, delay: 0, ssrc }`. On receive, only the receiver reacts. |
| 6 | HeartbeatAck | S to C | JSON | Resets missed count, sets ping. Nonce **not** checked. |
| 7 | Resume | C to S | JSON | `server_id, session_id, token, seq_ack`. No `user_id`, no DAVE version. |
| 8 | Hello | S to C | JSON | `setHeartbeatInterval` in any non-Closed state |
| 9 | Resumed | S to C | JSON | Only when Resuming |
| 11 | ClientsConnect | S to C | JSON | Adds `user_ids` to `connectedClients`. Accepted in UdpHandshaking, SelectingProtocol, Ready, Resuming. **Not in Identifying.** |
| 13 | ClientDisconnect | S to C | JSON | Removes from `connectedClients`. Receiver also deletes the SSRC entry. |
| 21 | DavePrepareTransition | S to C | JSON | Ready or Resuming with a DAVE object |
| 22 | DaveExecuteTransition | S to C | JSON | Same |
| 23 | DaveTransitionReady | C to S | JSON | `{ transition_id }` |
| 24 | DavePrepareEpoch | S to C | JSON | Same as 21 |
| 25 | DaveMlsExternalSender | S to C | binary | **Ready only** |
| 26 | DaveMlsKeyPackage | C to S | binary | Sent only in SelectingProtocol or Ready |
| 27 | DaveMlsProposals | S to C | binary | Ready only. Replies with op 28 if a commit is produced. |
| 28 | DaveMlsCommitWelcome | C to S | binary | `commit` then optional `welcome`, simply concatenated |
| 29 | DaveMlsAnnounceCommitTransition | S to C | binary | Ready only |
| 30 | DaveMlsWelcome | S to C | binary | Ready only |
| 31 | DaveMlsInvalidCommitWelcome | C to S | JSON | `{ transition_id }`, sent only in SelectingProtocol or Ready |

**Binary framing [src]**

- Inbound: `[seq: u16 BE][op: u8][payload]` (`src/networking/VoiceWebSocket.ts:129-131`). Every binary frame sets `sequence`.
- Outbound: `[op: u8][payload]`, no seq (`:190`).
- Op 27 payload: `[optype: u8 (0 append, 1 revoke)][proposals]`.
- Ops 29 and 30 payload: `[transition_id: u16 BE][mls data]`.

**JSON sequence tracking**

- `if (packet.seq) this.sequence = packet.seq` (`:153`): falsy check, so `seq: 0` is ignored for JSON. Binary has no such check.
- `sequence` starts at `-1`, and `-1` is sent as `seq_ack` until something numbered arrives. Docs allow this [ext].

**Unknown ops:** ignored without error. Non-JSON text emits `error` and keeps the socket open.

**Port design:** deserialize into `{ op: u8, d: RawValue, seq: Option<i64> }` then match. A strict tagged enum would break whenever Discord adds an opcode.

### 4.3 Close codes

Enum values from `ref/dat/voice/v8.ts`. Behaviour [src] (`Networking.ts:440-453`, `VoiceConnection.ts:474-499`):

| Code | Name | When Networking is Ready | Then VoiceConnection |
|---:|---|---|---|
| `< 4000` (1000, 1005, 1006...) | transport | Resume | stays Connecting |
| 4001 | UnknownOpcode | Close | Rejoin |
| 4002 | FailedToDecode | Close | Rejoin |
| 4003 | NotAuthenticated | Close | Rejoin |
| 4004 | AuthenticationFailed | Close | Rejoin |
| 4005 | AlreadyAuthenticated | Close | Rejoin |
| 4006 | SessionNoLongerValid | Close | Rejoin |
| 4009 | SessionTimeout | Close | Rejoin |
| 4011 | ServerNotFound | Close | Rejoin |
| 4012 | UnknownProtocol | Close | Rejoin |
| 4014 | Disconnected | Close | **Disconnected** |
| 4015 | VoiceServerCrashed | Resume | stays Connecting |
| 4016 | UnknownEncryptionMode | Close | Rejoin |
| 4017 | E2EE/DAVE required | Close | Rejoin, see 12.A1 |
| 4020 | BadRequest | Close | Rejoin |
| 4021 | RateLimited | Close | **Disconnected** |
| 4022 | CallTerminated | Close | **Disconnected** |

If Networking is not Ready, every code means Close. The VoiceConnection column is the same.

### 4.4 Heartbeats (`src/networking/VoiceWebSocket.ts:202-235`) [src]

- Fixed `setInterval` at `heartbeat_interval`. **No jitter, and no immediate first heartbeat**: the first one goes out after one full interval. Docs mention no jitter [ext].
- On each tick: if `lastHeartbeatSend != 0 && missedHeartbeats >= 3`, close the WS with no code and clear the interval. It then *still* calls `sendHeartbeat()`, whose failure is swallowed by the try/catch.
- `missedHeartbeats++` on send, reset to 0 on op 6.
- So three unacknowledged heartbeats are sent, and the close happens on the fourth tick [the skipped test asserts 3 messages].
- `ping = lastAck - lastSend`, undefined until the first ack.
- The local `ws.close()` starts a close handshake, so the `close` event needs the server's reply or the `ws` library's timeout. The port should bound this and synthesize 1006 [inf].

### 4.5 UDP (`src/networking/VoiceUDPSocket.ts`) [src]

- Socket type `udp4`, **not connected**: `send(buf, port, ip)`, so datagrams from any source are accepted.
- **IP discovery request:** 74 bytes: `u16 BE 1`, `u16 BE 70`, `u32 BE ssrc`, then 66 zero bytes.
- **IP discovery response:** ignored unless the first `u16 BE` is 2. IP is the UTF-8 run from offset 8 to the first NUL, and must pass `isIPv4`. Port is `u16 BE` at `length - 2`. A malformed response is silently ignored and waiting continues [test].
- **No timeout and no retransmit on discovery.** It rejects only if the socket closes. The test comment says it waits "indefinitely".
- **Keep-alive:** first send on the next tick after construction (so after the discovery packet), then every 5000 ms. 8-byte buffer with `u32 LE` counter in the first 4 bytes, the rest zero. Wraps after `2^32 - 1`. Replies are no longer tracked, so **UDP liveness is not detected at all**.

Test vector: `VALID_RESPONSE` in `__tests__/VoiceUDPSocket.test.ts:26-31` decodes to `91.90.123.93:54148`.

### 4.6 Outgoing packet [src] (`Networking.ts:744-811`)

```
[0x80][0x78][seq u16 BE][timestamp u32 BE][ssrc u32 BE] [ciphertext + 16-byte tag] [nonce u32 BE]
```

- AAD is exactly the 12-byte RTP header.
- Inner frame is `dave.encrypt(opus)` when DAVE applies, else the raw Opus.
- The nonce counter increments **before** use, so the first value is 1. Wraps to 0 past `2^32 - 1`. It lives in the first 4 bytes of a zeroed 12 or 24 byte buffer, and those 4 bytes are appended to the packet.
- Nonce increments at prepare. `sequence`, `timestamp` and `packetsPlayed` increment at dispatch.
- `timestamp += 960` per packet, regardless of wall clock. Pauses do not advance it.
- `sequence` wraps at `2^16` and `timestamp` at `2^32`, both by resetting to 0. For `timestamp` that differs from modular arithmetic: `wrapping_add` is correct RTP and the difference should not matter [inf].
- Dispatch order: `setSpeaking(true)` (WS op 5 only if the flag changed), **then** the UDP send. Docs require op 5 before any voice [ext].
- A module-level `nonce` buffer (`:176`, `rtpHeader.copy(nonce...)` at `:755`) is written and never read. Do not port it.

## 5. Encryption modes supported today

`SUPPORTED_ENCRYPTION_MODES` (`Networking.ts:21-26`) [src]:

| Mode | Cipher | Nonce | Tag | Status |
|---|---|---|---|---|
| `aead_aes256_gcm_rtpsize` | AES-256-GCM | 12 bytes (4 + 8 zeros) | 16 | Only if Node exposes it. Docs: preferred when hardware supports it [ext]. |
| `aead_xchacha20_poly1305_rtpsize` | XChaCha20-Poly1305 IETF | 24 bytes (4 + 20 zeros) | 16 | Always listed. Docs: "You must support" it [ext]. |

**Not implemented:** `xsalsa20_poly1305`, `_suffix`, `_lite`, `_lite_rtpsize`, `aead_aes256_gcm`. All marked deprecated in `ref/dat/voice/v8.ts` and removed upstream in 0.18.0 (`CHANGELOG.md:71-72`).

**Selection quirk:** `options.find(o => SUPPORTED.includes(o))` iterates **the server's list**, so the first mutually supported mode in Discord's order wins. The `unshift` that puts AES first in the local array has no effect on the choice [src].

If nothing matches, the throw happens inside the discovery `.then`, becomes an `error` event, and Networking **stays in UdpHandshaking forever**.

## 6. DAVE (E2EE)

### 6.1 Is it mandatory?

**Yes, in practice, for every voice call that is not a stage channel. A port without DAVE will not connect to normal voice channels.**

- [src] `CHANGELOG.md:15`: "Always install Davey as DAVE is becoming required (#11385)", in 0.19.1 (2026-03-09). `@snazzah/davey` is a hard dependency.
- [src] `ref/dat/voice/v8.ts`: close code 4017 `EndToEndEncryptionDAVEProtocolRequired`.
- [ext] Discord docs: "we will only support E2EE calls starting on March 1st, 2026" for DMs, group DMs, voice channels and Go Live.
- [ext] A third-party incident log and a Pycord issue report enforcement from 2026-03-02 with close code 4017 for out-of-date clients. Secondary sources, and they disagree by a day with the docs.
- [ext] Stage channels remain outside E2EE, per secondary sources. So `dave_protocol_version: 0` must stay a working path.

Library option: `daveEncryption` defaults to `true`. Setting it to `false` sends `max_dave_protocol_version: 0` and creates no session.

Negotiation: the client sends its max in Identify and the server picks the lowest shared version [ext]. Max is `Davey.DAVE_PROTOCOL_VERSION`, which is 1 in the crate [src].

### 6.2 Which library?

`@snazzah/davey` only (`README.md:68`), an OpenMLS-based implementation. Rust core published as the **`davey` crate**. Crypto parameters [ext, daveprotocol.com]: MLS ciphersuite 2 (DHKEMP256_AES128GCM_SHA256_P256), media AES128-GCM with an 8-byte truncated tag.

### 6.3 What `DAVESession.ts` adds on top of the library [src]

Constants:

- `TRANSITION_EXPIRY = 10` s
- `TRANSITION_EXPIRY_PENDING_DOWNGRADE = 24` s
- `DEFAULT_DECRYPTION_FAILURE_TOLERANCE = 36` packets

Flows:

| Event | Behaviour |
|---|---|
| Op 4 received | `createDaveSession` runs while computing the Ready state, so state is still SelectingProtocol. `reinit()` emits the key package synchronously, sent as op 26. **Key package leaves before state becomes Ready**, which is why op 26 is allowed in SelectingProtocol. |
| `dave_protocol_version == 0` at op 4 | A wrapper exists with no inner session. A later op 24 can create one. |
| `daveEncryption === false` | No wrapper at all. All DAVE ops ignored. |
| Op 24, `epoch == 1` | `protocolVersion = data.protocol_version`, then `reinit()`: new key package, or `reset()` plus passthrough 10 s if version is 0. Other epochs: logged only. |
| Op 21, `transition_id == 0` | Stored and executed immediately, no op 23. Networking emits `transitioned(0)`. |
| Op 21, other id | Stored. If version is 0, `setPassthroughMode(true, 24)`. Sends op 23. |
| Op 22 | If not pending: debug log and false. Else apply version. Downgrade sets `downgraded`. A later non-zero transition while downgraded enables passthrough 10 s ("upgrade"). Clears `reinitializing`, sets `lastTransitionId`, deletes the pending entry, emits `transitioned`. |
| Op 25 | `setExternalSender`. |
| Op 27 | `processProposals(optype, rest, [...connectedClients])`. If a commit results, send op 28 with commit + welcome. |
| Op 29 or 30, success | id 0: clear `reinitializing`, set `lastTransitionId`, emit `transitioned(0)`. Else: store pending with the current protocol version and send op 23. |
| Op 29 or 30, failure | `recoverFromInvalidTransition(id)`: if not already reinitializing, set `reinitializing`, zero failures, send op 31, then `reinit()` which sends op 26. Order is 31 then 26. |

**Encrypt** (`:349`): plaintext is returned when `protocolVersion == 0`, when the session is not ready, or when the packet is exactly `SILENCE_FRAME`. Otherwise `encryptOpus`.

**Decrypt** (`:361`):

1. `canDecrypt = session.ready && (protocolVersion != 0 || canPassthrough(userId))`.
2. If it is the silence frame or `!canDecrypt`, return the input unchanged.
3. On success, zero `consecutiveFailures`.
4. On failure with no pending transitions and not reinitializing: `consecutiveFailures++`. Past the tolerance, recover if `lastTransitionId` is truthy, else **rethrow**.
5. Failures during a transition or reinit are only logged. All failure paths that do not throw return `null`, and the packet is dropped.

**Verification API:** `voicePrivacyCode` (null if version 0 or no session), `getVerificationCode(userId)`, and the `transitioned` event, all re-exposed by VoiceConnection.

### 6.4 Mapping onto `davey` 0.2.0 (crate, not the npm wrapper)

The JS wrapper was written for the **0.1.x** semantics. `davey` 0.2.0 (dated 2026-11-01 in its changelog, 6 days ago) is breaking, and songbird 0.6.1 is still on `^0.1.2` [src]. A line-by-line port of the wrapper would be wrong on 0.2.

| JS | Rust 0.2.0 | Change to make |
|---|---|---|
| `new DAVESession(v, uid, cid)` | `DaveSession::new(NonZeroU16, u64, u64, None)` | IDs parsed to `u64`. Version 0 cannot be represented, so `Option<DaveSession>`. |
| `getSerializedKeyPackage()` | `create_key_package()` | Creates a new package each call |
| `reinit` | `reinit(v, uid, cid, None)` | **Now keeps current keys**, `is_ready` stays true for same identity |
| `ready` | `is_ready()` and `can_encrypt()` | `is_ready` is set when ratchets update, which can precede having an encryption key. Use `can_encrypt()` for the send gate. |
| `processCommit(buf)` | `process_commit(&[u8]) -> Result<bool, _>` | **No longer installs keys.** `false` means ignored: do not send op 23. |
| `processWelcome(buf)` | `process_welcome(&[u8], Option<&[u64]>)` | **No longer installs keys.** New argument, see below. |
| wrapper-side `pendingTransitions` | `prepare_transition(id, v)`, `execute_transition(id)` | **Must be called**: after a successful commit or welcome, and on ops 21 and 22. Without this, keys never switch. Id 0 applies at once. |
| `processProposals(op, buf, ids)` | `process_proposals(ProposalsOperationType, &[u8], Option<&[u64]>) -> Option<CommitWelcome>` | Same shape |
| `setPassthroughMode(b, s)` | `set_passthrough_mode(bool, Option<u32>)` | Can now only shorten the cipher expiry |
| `encryptOpus` | `encrypt_opus` (`Cow`), `encrypt_into` | `Err(NotReady)` if no key. Prefer `encrypt_into` on the hot path. |
| `decrypt(user, 0, buf)` | `decrypt`, `decrypt_into` | Errors now split into `NoDecryptorForUser`, `MissingKeyRatchet`, `InvalidNonce`, `MissingCryptor`, `DecryptionFailure`, `UnencryptedWhenPassthroughDisabled` |
| async `getVerificationCode` | `get_verification_code(u64)` | Sync |

**Trap:** `process_welcome` checks **every group member, including ourselves**, against `recognized_user_ids` (`davey-0.2.0/src/session.rs:646-659`). `connectedClients` comes from op 11 and may not contain our own id [inf]. Add the bot's own id to that slice, or every welcome could fail with `UnrecognizedUser`.

**Ordering trap (from songbird):** its WS task uses a biased `select!` because op 11 messages queued during the handshake must be handled before DAVE proposals, otherwise recognized ids are wrong (`songbird-0.6.1/src/driver/tasks/ws.rs:119-123`). Process all WS frames in arrival order in one task.

**Reference difference:** songbird runs the DAVE protocol-frame check (trailing `FA FA`, length at least 11) before decrypting and silently drops what cannot be decrypted (`udp_rx/mod.rs:199-243`). It uses a 120 s downgrade passthrough where JS uses 24 s. It also skips DAVE and sends plaintext when `!is_ready()` (`mixer/mod.rs:647-661`), like JS.

**Open check:** `DaveSession` being `Send` on 0.2.0 needs a compile test. Songbird wraps 0.1.x in `Arc<RwLock<Option<_>>>`, which suggests it, but that is a different version.

## 7. AudioPlayer

States (`src/audio/AudioPlayer.ts:35-60`): `Idle`, `Buffering`, `Playing`, `Paused`, `AutoPaused`.

| From | Trigger | To |
|---|---|---|
| any | `play(resource)`, resource already `started` | Playing (`missedFrames = 0`, `playbackDuration = 0`) |
| any | `play(resource)`, not started | Buffering |
| Buffering | stream `readable` | Playing |
| Buffering | stream `end`, `close`, `finish` | Idle |
| Buffering | `stop()` with any `force` | Idle |
| Playing | `pause(interpolate = true)` | Paused, `silencePacketsRemaining` 5 or 0 |
| Paused | `unpause()` | Playing, `missedFrames = 0` |
| Playing | no Ready subscribers, `Pause` behaviour | AutoPaused, 5 silence packets remaining |
| AutoPaused | a subscriber is Ready | Playing |
| Playing | no Ready subscribers, `Stop` behaviour | Idle via `stop(true)` |
| Playing | no Ready subscribers, `Play` behaviour | stays, keeps reading and discarding in real time |
| non-Idle | `stop(force)` with `force` or `silencePaddingFrames == 0` | Idle |
| non-Idle | `stop()` otherwise | Same state; `silenceRemaining = silencePaddingFrames` if it was -1 |
| Playing | `missedFrames >= maxMissedFrames` | `stop()` path |
| non-Idle, non-Buffering | `checkPlayable()` sees `!resource.readable` | Idle |
| non-Idle | stream error | emit `error`, then Idle if it is the current resource |

`play()` errors: throws if `resource.ended`; throws if owned by another player; **silent no-op** if already owned by this player.

Setter order (`:315-363`):

1. Destroy the old resource if it changed: swap in a no-op error handler, clear `audioPlayer`, destroy, drain.
2. Remove Buffering listeners when leaving Buffering.
3. On Idle: stop speaking on all subscribers and remove from the clock.
4. If a resource is present: add to the clock.
5. Assign, emit `stateChange`, emit the status event if the status changed **or** if Playing with a different resource.

**Global clock** (`src/DataStore.ts:101-187`) [src]:

- One timer drives all players in the process. `nextTime += 20` each cycle, then `setTimeout(max(1, nextTime - Date.now()))`.
- It never skips: after a stall it catches up in 1 ms steps, so packets go out in a burst.
- Cycle:
  1. Filter players by `checkPlayable()` (this can move a player to Idle).
  2. `_stepDispatch` for all of them, sending what was encrypted last cycle.
  3. `_stepPrepare` for each one sequentially, with `setImmediate` between players so a long list does not block.
- The separation exists so all UDP sends happen as close together as possible at the start of the frame.
- A consequence is one frame (20 ms) of added latency.

`_stepPrepare` (`:549-615`) reads with no blocking. A missing packet means a silence frame and `missedFrames++`. It increments `state.playbackDuration` by 20 per prepared packet including silence, while `resource.playbackDuration` counts only real packets.

**Subscriptions:** `VoiceConnection.subscribe` asks the player, and a connection holds at most one subscription. The `subscribe` event is deferred to the next tick; `unsubscribe` fires synchronously after `setSpeaking(false)`. Re-subscribing to the same player reuses the same object.

## 8. AudioResource, StreamType and the pipeline

`StreamType` (`src/audio/TransformerGraph.ts:28-49`): `Arbitrary`, `OggOpus`, `Opus` (object mode, ready to play), `Raw` (s16le), `WebmOpus`.

Graph edges [src]:

| From | To | Transformer | Cost | Details |
|---|---|---|---:|---|
| Raw | Opus | Opus encoder | 1.5 | 48 kHz, 2 ch, frame size 960 |
| Opus | Raw | Opus decoder | 1.5 | Same |
| OggOpus | Opus | Ogg demuxer | 1 | |
| WebmOpus | Opus | WebM demuxer | 1 | |
| Arbitrary, OggOpus, WebmOpus | Raw | FFmpeg PCM | 2 | |
| Raw | Raw | Inline volume | 0.5 | s16le |
| Arbitrary, OggOpus, WebmOpus | OggOpus | FFmpeg Ogg | 2 | **Only if** `ffmpeg -h` output contains `--enable-libopus`. Checked once, lazily. |

FFmpeg args, exact:

- PCM: `-i <input or -> -analyzeduration 0 -loglevel 0 -f s16le -ar 48000 -ac 2 pipe:1`
- Ogg: `-i <input or -> -analyzeduration 0 -loglevel 0 -acodec libopus -f opus -ar 48000 -ac 2 pipe:1`

`pipe:1` is appended by prism [src]. Note `-analyzeduration` comes **after** `-i`, so it applies as an output option rather than an input one [inf].

Search: depth-limited DFS (depth 5), skipping an edge whose own cost exceeds the current best total; ties keep the first found. Edge insertion order therefore matters.

Routing, computed from those rules (identical with and without libopus except the first row):

| Input | No volume | With inline volume |
|---|---|---|
| Arbitrary, FFmpeg with libopus | FFmpegOgg > OggDemux (3) | FFmpegPCM > Volume > Encoder (4) |
| Arbitrary, FFmpeg without | FFmpegPCM > Encoder (3.5) | FFmpegPCM > Volume > Encoder (4) |
| OggOpus | OggDemux (1) | FFmpegPCM > Volume > Encoder (4) |
| WebmOpus | WebmDemux (1) | FFmpegPCM > Volume > Encoder (4) |
| Opus | (none, 0) | Decoder > Volume > Encoder (3.5) |
| Raw | Encoder (1.5) | Volume > Encoder (2) |

So OggOpus with volume goes through FFmpeg rather than decoding in-process (4 beats 4.5). Keep it for parity, or adjust the cost as a conscious change.

`createAudioResource` (`src/audio/AudioResource.ts:258-296`):

- String input forces Arbitrary and FFmpeg.
- Otherwise it uses `inputType`, or infers from `instanceof` prism classes (`Encoder` gives Opus, `Decoder` gives Raw, `VolumeTransformer` gives Raw with volume, demuxers give Opus, anything else gives Arbitrary).
- Inference and the `hasVolume` shortcut run only when `inputType` is undefined.
- `volume` and `encoder` handles are exposed for live control.

**Port design:** `instanceof` has no Rust equivalent. Use typed source constructors and return handles (`VolumeHandle`, `EncoderHandle`) from the builder.

Resource lifecycle (`:125-167`):

- `silenceRemaining` starts at -1, meaning not started.
- The `readable` **getter has a side effect**: when the underlying stream stops being readable, it sets `silenceRemaining = silencePaddingFrames`.
- `ended` is `readableEnded || destroyed || silenceRemaining == 0`.
- `read()` returns `null` at 0, `SILENCE_FRAME` when positive (decrementing), otherwise a real packet (adding 20 to `playbackDuration`).
- `started` flips on the first `readable` event.

**prism-media 1.3.5 behaviour to reproduce** [src, tarball, `src/opus/*`, `src/core/*`]:

*OggDemuxer*
- Checks `OggS` and version 0, and throws otherwise. No resync.
- Packets are built from lacing values within one page.
- A packet **continuing across pages stalls the demuxer forever**: a trailing 255 lacing value returns "need more data", and more data never changes that page's table.
- First `OpusHead` sets the head and locks the bitstream serial. `OpusTags` emits an event. Other serials are dropped. Pre-skip and granule position are ignored.

*WebmDemuxer*
- Requires EBML first.
- Understands only EBML, Segment, Cluster, Tracks, TrackEntry, TrackNumber, TrackType, SimpleBlock, CodecPrivate.
- **BlockGroup is skipped**, so such audio is silently lost. No lacing support.
- Track match is `(data[0] & 0x0F) == track.number`, and the payload is `data[4..]`: only 1-byte track vints work.
- The first audio track is used. Any CodecPrivate that is not `OpusHead` throws `Audio codec is not Opus!`, **including a video track's**.
- Container sizes are ignored for parent elements, so unknown-size live streams work.
- Vint expansion overflows at 32 bits. Use `u64` in Rust.

*Encoder*
- Buffers until `frameSize * channels * 2` = 3840 bytes.
- A trailing partial frame is dropped at end of stream.
- `setBitrate` clamps to 16000..128000, `setFEC`, `setPLP` takes 0..1 scaled to percent.

*Decoder*
- Swallows `OpusHead` and `OpusTags` chunks.

*VolumeTransformer*
- `volume == 1` is passthrough.
- Otherwise `floor(volume * sample)`, clamped to the i16 range, keeping a partial-sample remainder.
- Helpers: `setVolumeDecibels` is `10^(db/20)`, `setVolumeLogarithmic` is `value^1.660964`, and the logarithmic getter is `volume^(1/1.660964)`.

*FFmpeg*
- Lookup order: `ffmpeg-static`, `ffmpeg`, `avconv`, `./ffmpeg`, `./avconv`.
- Probed with `-h`, `windowsHide`, cached. Killed with SIGKILL on destroy.
- **Port:** `kill_on_drop(true)`, `CREATE_NO_WINDOW` on Windows, stderr to null. FFmpeg is not on this machine's PATH, so integration tests need it installed.

`demuxProbe` (`src/util/demuxProbe.ts`) [src, test]:

- Feeds bytes to both demuxers, and swallows their errors.
- The first head passing the validator (`channels == 2 && rate == 48000`, offsets 9 and 12 LE) resolves the type.
- It gives up at `probeSize` or stream end with `Arbitrary`.
- An invalid head does not resolve, it keeps going.
- Output must contain every input byte in order [test].
- Rejects object-mode streams, already ended streams, and propagates stream errors.
- **Port:** return `Chain<Cursor<Vec<u8>>, R>`. Node's `stream.push(readBuffer)` puts the bytes at the buffer's tail, so it relies on the buffer being empty.

## 9. Receiver

`onUdpMessage` order (`src/receive/VoiceReceiver.ts:206-240`) [src]. Preserve the order because each step gates the cost of the next:

1. Drop if `length <= 12`.
2. `ssrc` = `u32 BE` at offset 8. Drop if not in the SSRC map (this also filters keep-alive and discovery replies).
3. `speaking.onPacket(userId)`. Runs **before** any validation, so silence and invalid packets count as speaking.
4. Drop if there is no subscription for that user. **No decryption without a subscriber**, so DAVE failure counters only see subscribed users.
5. Require `encryptionMode`, `nonceBuffer`, `secretKey`.
6. Drop unless `(b[1] & 0x7F) == 0x78` and `b[0] >> 6 == 2`.
7. `parsePacket`. If it throws, `stream.destroy(error)` [test]. If it returns null, drop.

`parsePacket`:

1. `X = (b[0] >> 4) & 1`, `CC = b[0] & 0x0F`, `P = b[0] & 0x20`.
2. `headerSize = 12 + 4*CC`, plus 4 if X (extension **preamble only**).
3. Nonce from the last 4 bytes. Tag is the 16 bytes before them. Ciphertext is `[headerSize, len-20)`. AAD is `[0, headerSize)`.
4. Decrypt.
5. If P: `n = last byte`; if `n < payload.len`, strip `n`, otherwise leave as is. (Added in 0.19.2, `CHANGELOG.md:9`.)
6. If X: read `extLen = u16 BE at (12 + 4*CC + 2)` and skip `4 * extLen` bytes of plaintext.
7. If VoiceConnection is Ready and Networking is Ready or Resuming and a DAVE wrapper exists: `dave.decrypt(payload, userId)`. Null means drop.
8. Yield `AudioPacket { payload, sequence, timestamp, ssrc }`.

Order matters: transport decrypt, then padding, then extension, then DAVE.

**SSRCMap** (`src/receive/SSRCMap.ts`) [src, test]:

- Keyed by audio SSRC. Fed only by **op 5 Speaking** (`{ userId, audioSSRC }`), removed by op 13.
- `update` merges and emits `create` (only if new) then `update(old, new)`.
- `get(number)` is a direct lookup; `get(string)` scans and returns the first match.
- `delete(string)` removes the first match only.
- `videoSSRC` exists in the type and nothing sets it.
- A user is unknown until their op 5 arrives, and their earlier packets are dropped.

**SpeakingMap** (`src/receive/SpeakingMap.ts`) [src, test]:

- `DELAY = 100` ms after the last packet.
- First packet: record `Date.now()` in `users`, emit `start`.
- Timer expiry: emit `end`, **then** delete from `users`. So listeners still see the user in the map.

**AudioReceiveStream** (`src/receive/AudioReceiveStream.ts`) [src, test]:

- `Manual`: only ends when destroyed.
- `AfterInactivity(ms)`: every packet renews the timer.
- `AfterSilence(ms)`: a packet renews only if it is not the silence frame, or if no timer exists yet. So the first packet always starts a timer, even if silent.
- End: `push(null)` then destroy on next tick, then `close` removes it from `subscriptions`.
- `subscribe(userId)` returns the existing stream and **ignores new options** [test].
- `_read` is a no-op and the `push` result is ignored: **no backpressure, unbounded buffer**.
- The receiver survives Networking replacement, and its maps are not cleared on reconnect. Streams are destroyed only on `Destroyed`.

Test vectors in `__mocks__/rtp.ts`: `RTP_PACKET_DESKTOP`, `_CHROME`, `_ANDROID`, `XCHACHA20_SAMPLE`, `AES256GCM_SAMPLE`. The expected sequence, timestamp and SSRC values are in `__tests__/VoiceReceiver.test.ts:71-92,179-228`. Port these as Rust unit tests directly.

## 10. Adapter and main-gateway integration

Contract (`src/util/adapter.ts`):

- Library gives the adapter `onVoiceServerUpdate`, `onVoiceStateUpdate`, `destroy`.
- Adapter gives the library `sendPayload(payload) -> bool` (false means **definitely** failed) and `destroy()`.
- The payload is op 4 (`VoiceStateUpdate`): `{ guild_id, channel_id, self_deaf, self_mute }`.

discord.js implementation [src]:

- `Guild.voiceAdapterCreator` stores methods in `client.voice.adapters` keyed by guild id, `sendPayload` always returns `true`, and `destroy` removes the entry.
- `ClientVoiceManager.onVoiceStateUpdate` forwards only if `guild_id && session_id && user_id == client.user.id`.
- `onVoiceServer` forwards by guild id.
- On shard close with **code 1000 only**, it destroys all adapters of that shard's guilds.

**Port design:** define a `GatewaySender` trait for voice and keep voice as its own crate behind a feature, so the core client does not depend on Opus or DAVE.

**Rate limits:** the voice package has none of its own. Join payloads go through the shard's normal send queue, so they count against the main gateway budget (see the gateway report). The only voice-side signal is close code 4021, which stops reconnection.

## 11. Rust design constraints that follow from all this

These are consequences of the JS model not translating directly.

- **Synchronous re-entrancy is gone.** JS listeners run inside the setter and can call back in. Use one actor task per connection and per Networking, and deliver events through channels.
- **Do not model status waiting with `watch` alone.** `watch` coalesces, so a short pass through Signalling could be missed. `entersState` relies on seeing every transition, including transient ones. Use `broadcast` for events plus a current-state read, and define the behaviour on `Lagged`.
- **`entersState` also rejects on `error`.** Node's `events.once` rejects when the emitter emits `error` during the wait, so a transient connection error fails the wait [inf from Node semantics]. Choose explicitly; I suggest not failing on non-fatal errors, and documenting it.
- **The prepare step must not block.** Use a bounded channel from a producer task and `try_recv` in the clock. A miss means a silence frame, as in JS. The bound replaces Node's object-mode high-water mark (16 packets, about 320 ms, plus upstream buffering [inf]).
- **Clock:** one task per `VoiceManager`, absolute deadlines, monotonic `Instant`, burst catch-up like JS (or `MissedTickBehavior::Burst`). Windows timer granularity may be around 15 ms by default [inf, unmeasured], which directly threatens 20 ms pacing on the dev machine, so measure it early. A dedicated thread with a high-resolution waitable timer may be needed there.
- **Windows UDP:** `recv` can return `WSAECONNRESET` after an ICMP port-unreachable [inf, general Windows behaviour]. Do not treat it as fatal.
- **`DaveSession` needs `&mut` for both encrypt and decrypt.** So the UDP receive task and the clock share one lock. Keep the critical section to the single call, and never hold it across `.await`.
- **Nonce buffers:** JS shares `nonceBuffer` between send and receive, which only works because it is single-threaded and the sender rewrites it each time. Use separate stack arrays.
- **Secrets:** see 12.A9.

## 12. Subtle behaviour

### A. JS behaviours I recommend NOT copying (decision needed for each)

These look like upstream bugs or hazards. All come from code reading and none were reproduced live. Each should be an explicit decision.

| # | Behaviour | Evidence | Recommendation |
|---|---|---|---|
| A1 | **4017 triggers an unbounded rejoin.** With `daveEncryption: false` on a non-stage channel, each attempt would identify with version 0 again. | `VoiceConnection.ts:477`, code list | Add 4017 to the no-reconnect list, and also 4004 and 4016 as likely non-recoverable. Report clearly. |
| A2 | **No backoff on automatic rejoin.** `rejoinAttempts` only counts; nothing waits. It also spends main gateway budget. | `:486-497` | Configurable backoff, default on, documented as a divergence |
| A3 | **DAVE ciphertext reaches subscribers during a resume.** Networking Ready to Resuming makes VoiceConnection `Connecting` synchronously, and `parsePacket` requires connection status `Ready`. UDP keeps delivering, so DAVE is skipped and the protocol frame is pushed as if it were Opus. The `Resuming` check in that condition is effectively unreachable. | `VoiceReceiver.ts:183-194`, `VoiceConnection.ts:518-522` | Gate on Networking state only, and drop when DAVE is active and undecryptable |
| A4 | **Plaintext send while DAVE is expected.** `protocolVersion > 0` but session not ready: Opus goes out with transport encryption only. Songbird does the same. | `DAVESession.ts:350` | **Security decision.** Dropping frames until `can_encrypt()` is the conservative choice, and costs a little audio at session start. Confirm that receivers tolerate it. |
| A5 | **Spread keeps `networking` in states whose types forbid it.** Null endpoint gives Disconnected with Networking **alive and undestroyed**. Rejoin from Connecting gives Signalling with Networking still running, while `onNetworkingStateChange` ignores it. | `VoiceConnection.ts:355-359, 486-489, 663-666` | A Rust enum forces a choice. Decide per transition. Destroying on null endpoint looks right. Rejoin should keep or tear down explicitly. |
| A6 | **Speaking flag ends up true while paused.** The last silence packet is prepared and `setSpeaking(false)` runs, but next cycle's dispatch calls `setSpeaking(true)`. With `pause(false)` it is never cleared. | `AudioPlayer.ts:571-576`, `Networking.ts:711` | Signal stop after dispatching the last silence |
| A7 | **Only 4 of 5 trailing silence frames are sent, and the fifth is stale.** After read 5, the next cycle's `checkPlayable()` goes Idle **before** dispatch. The prepared packet stays in Networking and is dispatched as the **first packet of the next resource**. Docs ask for five frames. | `DataStore.ts:120-124`, `AudioResource.ts:126` | Dispatch first or flush on Idle, and clear `preparedPacket` on resource change |
| A8 | **Uncaught throws from protocol input.** Short binary frames, `setExternalSender`, `processProposals` and `encrypt` errors have no try/catch in the handlers, so a bad frame can raise an uncaught exception in Node. | `VoiceWebSocket.ts:129`, `Networking.ts:585-589, 677` | `Result` everywhere. Length-check before parsing. On MLS errors follow the recovery path (op 31 plus reinit). |
| A9 | **Secrets in debug logs.** `stringifyState` serializes `token` and `secretKey`, and WS debug logs print Identify and op 4 verbatim. | `Networking.ts:196-202`, `VoiceWebSocket.ts:142,174` | Custom `Debug`, redact in tracing, zeroize keys |
| A10 | **Stale doc: UDP self-close.** The comment says the socket closes when replies stop. Keep-alive tracking is gone and `close` fires only from `destroy()`, after listeners are removed. A dead UDP path is undetectable; `onUdpClose` is dead code. | `VoiceUDPSocket.ts:83,117-124`, `Networking.ts:456-467` | Detect via keep-alive replies if Discord echoes them. Needs live testing. |
| A11 | **Handshake hangs.** Lost discovery packet, or no compatible mode, leaves Networking stuck with no timeout. `Buffering` also has no timeout. | `VoiceUDPSocket.ts:151-172`, `Networking.ts:506` | Retransmit discovery and add a handshake deadline that ends in `close` |
| A12 | **One bad packet kills the receive stream.** Any transport decrypt error destroys the user's stream, and the socket is unconnected, so anyone who knows an SSRC and the port could send one. Short packets reach `subarray` with negative offsets. | `VoiceReceiver.ts:236-238`, [test] | Bounds-check, count and drop. Keep stream destruction only as an option. Note it diverges from a test. |
| A13 | **Falsy `lastTransitionId`.** `if (this.lastTransitionId)` treats id 0 as absent, so after a transition 0 session, failures past tolerance **throw and destroy the stream**. The counter is not reset, so each later packet does it again. The counter is also session-wide, not per user. | `DAVESession.ts:373-375` | Use `Option<u16>` and recover in all cases. Consider per-user counting. |
| A14 | **Binary DAVE frames dropped during Resuming**, while JSON DAVE ops are accepted, and `sequence` has already advanced. Harmless if the server replays only after op 9 [ext suggests it], which is unconfirmed. | `Networking.ts:584` vs `:558` | Keep unless live tests show loss; otherwise queue until Ready |
| A15 | **Encryption preference ignored** (section 5). | `Networking.ts:210` | Prefer AES-256-GCM explicitly per docs, fall back to XChaCha |
| A16 | **`channel_id!` assertion.** A server packet after a state packet with null channel passes null into Networking and then DAVE. Also, `connectionOptions.channelId` can go stale after a move without a server update, and the DAVE group id derives from it. | `VoiceConnection.ts:439` | Refuse to configure without a channel id. Treat a channel change as needing a new session. |
| A17 | **Receive buffer unbounded** (section 9). | `AudioReceiveStream.ts:138` | Bounded with drop-oldest and a counter |
| A18 | **Rejoin after `disconnect()` with no arguments sends a null channel**, since `channelId` was nulled and only a non-null state packet restores it. | `VoiceConnection.ts:623, 660` | Make the channel required, or document it |
| A19 | **Demuxer limits**: multi-page stall, BlockGroup loss, video CodecPrivate throw (section 8). | prism | Fix all three in the hand-written demuxers, because they are silent failures |
| A20 | **String input reaches FFmpeg as-is.** Args avoid a shell, but `-i <string>` still accepts any FFmpeg protocol, so a user-supplied URL could read local files or reach internal hosts [inf]. | `TransformerGraph.ts:166` | Document it, and offer a `-protocol_whitelist` option |

### B. Behaviours to preserve exactly

**Protocol**

1. Exact byte layouts of sections 4.2, 4.5, 4.6, 9. First nonce is 1, counter in the first 4 bytes big-endian.
2. Send op 5 before the first packet, only on change, and reset the flag after Resumed. Docs say a missing op 5 gets the connection dropped with an invalid SSRC error [ext].
3. `seq_ack` in both Heartbeat and Resume, and -1 before any numbered message. Docs make this mandatory since v8 [ext].
4. The JSON `seq: 0` is ignored but binary `seq: 0` is accepted. Keep it unless proven harmful: `seq_ack` must match what the server expects.
5. Resume is attempted only from Ready and exactly once. Failure falls through to rejoin. If the server's buffer lost messages, it closes and a full reconnect is needed [ext].
6. Resume keeps UDP, secret key, DAVE session, nonce, sequence, timestamp. A new Networking resets all of them.
7. Key package timing: sent as soon as op 4 arrives, before entering Ready.
8. Reply order on bad commit or welcome: op 31, then op 26.
9. Silence frames skip DAVE in both directions, as an exact 3-byte match. Spec-defined [ext].
10. Op 28 is plain concatenation, commit then welcome.
11. `transition_id == 0` means no op 23.
12. A `Date.now()` value is a fine heartbeat nonce and acks are not matched against it.
13. `VOICE_SERVER_UPDATE` always rebuilds Networking, even for the same endpoint: a new token means the session cannot be reused [ext]. Do not cache server packets beyond the current connection's reconnect logic.
14. After 4014 stay Disconnected and let a later `VOICE_SERVER_UPDATE` reconnect automatically. The guide relies on this to tell a channel move from a kick: wait 5 s for Signalling or Connecting after Disconnected (`apps/guide/content/docs/voice/voice-connections.mdx:99-114`). Upstream calls it "imperfect", and it is the documented user pattern, so provide a helper.
15. In Signalling, a state packet with a channel plus any stored server packet triggers Networking, using the possibly stale server token. A newer server packet then replaces it. This fixes gh #11553, so keep it.

**Player and resource**

16. Dispatch-then-prepare order across all players, one global clock, burst catch-up, no frame skipping.
17. `stop()` semantics: Buffering always goes Idle at once; otherwise padding plays unless forced [test].
18. AutoPaused to Playing costs one cycle: the local `state` is still AutoPaused for the rest of that `_stepPrepare`, so reading resumes next cycle.
19. `NoSubscriberBehavior::Play` keeps consuming the resource in real time with no one listening.
20. Silence on read miss, and the player ends after `maxMissedFrames` consecutive misses. Then padding, then Idle [test].
21. Two distinct durations: the player state's (includes silence, resets on each `play`) and the resource's (real packets only).
22. Status event fires for Playing to Playing on a resource change, with no Idle in between.
23. `play(sameResource)` on the same player is a no-op. Resources are single-use and cannot be replayed after ending [test].
24. `playable` means connection status Ready. So a resume in progress triggers the no-subscriber behaviour, and a WS blip auto-pauses playback under the default.
25. `unsubscribe` calls `setSpeaking(false)`.

**Receiver**

26. Pipeline order in section 9, including speaking detection before validation and no decryption without a subscriber.
27. Padding strip guard `n < len`, and the extension skip happening in plaintext.
28. `end` emitted before user removal in SpeakingMap. `create` before `update` in SSRCMap.
29. Default `selfDeaf: true`. Receiving needs `selfDeaf: false`, per the doc comment at `VoiceConnection.ts:233-235`.

**Audio formats**

30. Opus 48 kHz stereo, 960 samples per channel per frame, 3840 PCM bytes per frame, RTP timestamp step 960, payload type 0x78.
31. `validateDiscordOpusHead` strictness: mono or non-48k goes through FFmpeg.

## 13. Crate survey

Search results as returned by `cargo search`, run 2026-11-07.

### Requested searches

| Search | Relevant result | License | MSRV | Notes |
|---|---|---|---|---|
| `davey` | **`davey` 0.2.0** | MIT | unset, edition 2024 | The same project as the npm package. OpenMLS 0.8.1 with `openmls_rust_crypto` 0.5.1. Optional `napi` and `pyo3`, off by default. README says usage docs do not exist yet. Lockfile has 224 packages including optional ones. Declares `openmls` feature `js`, which looks odd for a native build; check its effect. Current `openmls` is 0.9.2. |
| `audiopus` | `audiopus` **0.3.0-rc.0**, `audiopus_sys` 0.2.2 | ISC | unset | Only a release candidate shows as latest. Songbird has moved off it. |
| `opus` | **`opus` 0.4.0** | MIT/Apache-2.0 | unset | Backed by `opusic-sys` 0.7.x, with optional cmake and bindgen build deps |
| `opus` | `opus-pure` 0.2.2 | BSD-3-Clause | 1.88 | Pure Rust, no deps, encoder + decoder + Ogg. Its README claims byte-exactness with libopus 1.6.1 where possible. Not independently verified. |
| `opus` | `opus1` 0.3.0 | BSD-3-Clause | 1.98 | Pure Rust. README claims "Production-ready"; bit-exact only on ARM64, not on x86-64. Hard fork from opus-rs. Not independently verified. |
| `songbird` | **`songbird` 0.6.1** | ISC | 1.83 | Full async voice stack, details below |

### Additional crates I checked

| Crate | Version | License | Use |
|---|---|---|---|
| `opus2` | 0.4.0 | MIT/Apache-2.0 | What songbird uses. Default `backend-libopus` and `static` via `libopus_sys` 0.4.0. Optional `backend-mousiki` is pure Rust. |
| `aes-gcm` | 0.11.1 | Apache-2.0 OR MIT | AES-256-GCM. MSRV 1.85. |
| `chacha20poly1305` | 0.11.0 | Apache-2.0 OR MIT | XChaCha20-Poly1305. MSRV 1.85. |
| `discortp` | 0.6.0 | ISC | RTP and Discord packet types. Optional convenience: the layouts are short enough to hand-write. |
| `serenity-voice-model` | 0.3.0 | not checked | Voice gateway models. Our models should come from the discord-api-types port instead. |
| `symphonia` | 0.6.1 | MPL-2.0 | Containers and decoders. Its feature list shows **no Opus codec**. Not needed for parity since FFmpeg covers it. |
| `ogg` | 0.9.2 | BSD-3-Clause | Page parsing. API not checked for async suitability. |
| `matroska-demuxer` | 0.8.1 | Zlib OR MIT OR Apache-2.0 | Described as a file demuxer. Check whether it needs `Seek`; live streams cannot seek. |
| `rubato` | 5.0.1 | not checked | Only if in-process resampling becomes a goal. FFmpeg does it for parity. |
| `tokio-tungstenite` | 0.30.0 | not checked | Voice WS |
| `tokio-websockets` | 0.13.3 | not checked | Alternative |

Crate names with a dot are not allowed on crates.io, and **`discordrs` 3.2.0 already exists, self-described "discord.rs"**, as does `discord-rs` 0.1.1. The project needs a different published crate name, even if the repo stays `discord.rs`. Flagging for the overall project.

### Songbird assessment

Songbird is the closest existing Rust implementation and a very useful reference, including its DAVE handling. Depending on it instead of porting is a real option, with these costs:

- It is on `davey ^0.1.2`, so it does not have the 0.2.0 fixes (nonce reuse on re-install, welcome and commit validation, and others listed in the changelog).
- Its model is a **per-call mixer driver**, not per-player. `AudioPlayer` shared across connections, the status state machines and the graph would not map 1:1.
- Default features pull in `serenity`. `driver` alone avoids this but still brings `symphonia`, `rubato`, `reqwest`, `rusty_pool`, and more.

License is ISC, compatible. If code is adapted rather than just read, keep its notice.

**Recommendation: write the port, use songbird as the reference, do not depend on it.** The request is a discord.js equivalent, and API parity is what songbird cannot give. That is a judgment call for the project lead.

## 14. Reuse versus hand-written

### Reuse

| Need | Crate | Confidence |
|---|---|---|
| DAVE, MLS, frame crypto, ratchets, verification codes | `davey` 0.2.0 | No realistic alternative. Risk: 6 days old and breaking. Songbird and npm are on 0.1. Pin it exactly and compile-test first. |
| Transport AEADs | `aes-gcm`, `chacha20poly1305` | High |
| Opus codec | `opus2` (libopus) behind a trait, as a cargo feature | Medium: not compiled here. cmake 4.4.4 is present on this machine; older bundled CMake projects can have compatibility trouble with CMake 4, so test. |
| Opus, no C toolchain | `opus-pure` as an alternate feature | Medium-low: young, self-reported quality. Worth evaluating because it removes the C build on Windows. |
| WebSocket and TLS | `tokio-tungstenite` with rustls | High. Same choice as the main gateway crate. |
| Async, UDP, child process, channels | `tokio` | High |
| Randomness, snowflakes | `rand`, shared core types | High |
| FFmpeg | External binary via `tokio::process`, as upstream | High |

Opus can be fully optional: Opus, Ogg and WebM passthrough with no volume need no codec at all, since silence frames are a constant.

### Hand-write

| Component | Basis | Est. Rust LOC |
|---|---|---:|
| `VoiceConnection` actor and state machine | section 3 | 700 |
| `Networking` handshake, RTP, encryption | sections 4 and 5 | 700 |
| `VoiceWebSocket` heartbeat, framing, seq | 4.2, 4.4 | 250 |
| `VoiceUdpSocket` discovery, keep-alive | 4.5 | 180 |
| `DaveSession` policy wrapper, **rewritten for 0.2 semantics** | section 6 | 350 |
| `AudioPlayer` and subscriptions | section 7 | 550 |
| Global clock | section 7 | 120 |
| `AudioResource` and builder | section 8 | 300 |
| Transformer graph and search | section 8 | 200 |
| FFmpeg wrapper and probing | section 8 | 200 |
| Streaming Ogg demuxer | section 8 | 180 |
| Streaming WebM demuxer | section 8 | 250 |
| Encoder and decoder stages, frame buffering | section 8 | 150 |
| Volume stage | section 8 | 80 |
| `demux_probe` | section 8 | 100 |
| `VoiceReceiver`, SSRC, speaking, streams | section 9 | 500 |
| Adapter trait, manager, `join`, `enters_state` | section 10 | 250 |
| Gateway payload models, if not already provided | section 4.2 | 200 |
| **Total** | | **about 5,200**, plus tests |

These are rough estimates from upstream sizes, not measurements.

## 15. Tests to port

Directly reusable data:

- `__mocks__/rtp.ts` (RTP captures and AEAD samples, with keys)
- `VALID_RESPONSE` for discovery
- `__tests__/demuxProbe.test.ts` scenarios

Behaviour suites to translate:

- `__tests__/VoiceConnection.test.ts` (787 lines, nearly every transition)
- `__tests__/AudioPlayer.test.ts` (including exact prepare and dispatch counts)
- `__tests__/AudioResource.test.ts`
- `__tests__/TransformerGraph.test.ts`
- `__tests__/SSRCMap.test.ts`, `SpeakingMap.test.ts`, `AudioReceiveStream.test.ts`, `VoiceReceiver.test.ts`, `DataStore.test.ts`, `entersState.test.ts`

Gaps upstream that the port needs to fill itself:

- A mock voice gateway for the whole handshake and resume
- All DAVE flows, with two `davey` sessions talking to each other (`davey-0.2.0/tests/unit/session.rs` shows setup)
- A live test against a real voice channel for everything marked [ext] or [inf]: 4017 behaviour, op ordering around Resume, keep-alive echoes, and whether receivers tolerate A4

## 16. Decisions needed

1. **A4:** drop or send plaintext when DAVE is negotiated but no key is available. Dropping is more private; upstream sends.
2. **A1/A2:** add close codes and backoff to automatic reconnection. This diverges from upstream behaviour.
3. **A12:** stream destruction on one bad packet. An upstream test asserts destruction.
4. **Songbird:** port as recommended, or wrap its `driver`.
5. **Opus default:** `opus2` (libopus, needs a C build) or `opus-pure` (no toolchain, less proven).
6. **Crate name:** `discordrs` and `discord-rs` are both taken.
7. **`davey` 0.2.0 versus 0.1.x:** 0.2.0 recommended for its nonce-reuse and validation fixes, despite its age.

## Sources

External pages used for [ext] claims:

- [Discord voice connections docs](https://docs.discord.com/developers/topics/voice-connections)
- [DAVE protocol whitepaper](https://daveprotocol.com/)
- [Pingoru incident log for Discord](https://pingoru.io/providers/discord/incidents/70967) (secondary)
- [Pycord issue 3135, via mirror](https://github.yezi957.workers.dev/Pycord-Development/pycord/issues/3135) (secondary, read as a search snippet only)
- [Help Net Security](https://www.helpnetsecurity.com/?p=371276) and [The Next Web](https://thenextweb.com/news/discord-end-to-end-encrypted-voice-video-all-users) on stage channel scope (secondary, search snippets only)
- [prism-media 1.3.5 tarball](https://registry.npmjs.org/prism-media/-/prism-media-1.3.5.tgz)

Local crate sources read: `davey-0.2.0`, `songbird-0.6.1`, and README or manifest files for the Opus crates, under `~/.cargo/registry/src/`.
