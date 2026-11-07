# discord.js `packages/discord.js/src` map, for the discord.rs port

All paths are relative to `packages/discord.js/src/` unless stated otherwise.

## 0. Provenance and reading limits

- **Source:** the `main` branch at `5bb7492` (2026-11-06). There are 333 `.js` files.
- **Version trap:** `package.json` says `14.16.3`, but the code is not the published 14.x API. Examples:
  - The ready event is `clientReady` (`util/Events.js`). There is no `ready`.
  - `Events` has no `raw` event and no `shard*` events. Raw packets are reached through `client.ws.on(WebSocketShardEvents.Dispatch)`.
  - `Invite` is split into `BaseInvite`, `GuildInvite` and `GroupDMInvite`.
  - `ShardClientUtil` has no `ids` or `count` getters.
  - `Role` has `colors`, not `color`.
  - Do not port from published 14.x documentation.
- **How this was produced:** by reading source only. Nothing was executed, so every behaviour below is a code-reading conclusion, not an observed runtime result.
- **Files read in full (minus JSDoc):**
  - `client/**`, `managers/**`, `sharding/**`, `errors/**`
  - `util/{Options,LimitedCollection,Partials,Sweepers,BitField,PermissionsBitField,IntentsBitField,Channels,Components,DataResolver,Transformers,Constants,Util,Enums,Invites,Status,Events,Symbols}.js`
  - `structures/`: `Base`, `BaseChannel`, `GuildChannel`, all channel classes, `Message`, `MessagePayload`, `MessageMentions`, `MessageReaction`, `Guild`, `BaseGuild`, `AnonymousGuild`, `GuildMember`, `User`, `ClientUser`, `ClientPresence`, `BaseRole`, `Role`, `VoiceState`, `Poll`, `PollAnswer`, `PermissionOverwrites`, `ThreadMember`, `Webhook`, `InteractionWebhook`, `InteractionCallback*`, `Emoji`, `BaseGuildEmoji`, `GuildEmoji`, `ReactionEmoji`, `BaseInvite`, `GuildInvite`, `GuildBan`, `GuildAuditLogs`, `Typing`, all interaction classes, `CommandInteractionOptionResolver`, `ModalComponentResolver`, all collectors and all four `interfaces/*`.
- **Files read partly:**
  - `GuildAuditLogsEntry` (constructor and `targetType`; `actionType` only partly)
  - `Presence` (`Presence` fully, `Activity` constructor start)
  - `ApplicationCommand` (`_patch`, simple methods, `transformOption`; `equals`, `optionsEqual` and `_optionEquals` skimmed)
  - `ClientApplication` (head, plus the list of REST calls)
- **Files inventoried only** (class line, getters and methods by grep; bodies not read). A port must read these before implementing them:
  `ActionRow`, `ActivityInstance`, `ActivityLocation`, `ApplicationEmoji`, `ApplicationRoleConnectionMetadata`, `Attachment` (start of `_patch` only), `AutoModerationActionExecution`, `AutoModerationRule`, `BaseGuildEmoji`-derived `GuildPreviewEmoji`, `BaseSelectMenuComponent`, `ButtonComponent`, `ChannelSelectMenuComponent`, `Component`, `ContainerComponent`, `Embed`, `Entitlement`, `FileComponent`, `GroupDMInvite`, `GuildOnboarding*`, `GuildPreview`, `GuildScheduledEvent`, `GuildTemplate`, `Integration`, `IntegrationApplication`, `InviteGuild`, `LabelComponent`, `MediaGallery*`, `OAuth2Guild`, `SectionComponent`, `SeparatorComponent`, `SKU`, `SoundboardSound`, `StageInstance`, `Sticker`, `StickerPack`, `StringSelectMenuComponent`, `Subscription`, `Team`, `TeamMember`, `TextDisplayComponent`, `TextInputComponent`, `ThumbnailComponent`, `UnfurledMediaItem`, `VoiceChannelEffect`, `VoiceRegion`, `WelcomeChannel`, `WelcomeScreen`, `Widget`, `WidgetMember`, `interfaces/Application`; and `util/APITypes.js` (doc-only re-export typedefs) and the one-line `*FlagsBitField` subclasses.
- **Outside this report's scope:** the internals of `@discordjs/rest` (REST buckets) and `@discordjs/ws` (gateway shards). Only the facts `discord.js` depends on are quoted here, with `packages/...` paths.

## 1. Layering

```
user code
  |  events (AsyncEventEmitter), managers, structures
Client (client/Client.js)
  |-- rest: REST (@discordjs/rest)               <- all HTTP, buckets, retries, files
  |-- ws:   WebSocketManager (@discordjs/ws)     <- shards, heartbeat, resume, identify queue
  |-- actions: ActionsManager (client/actions)   <- shared cache-mutating logic (WS + REST)
  |-- PacketHandlers (client/websocket/handlers) <- one function per dispatch event (72)
  |-- users/guilds/channels: top managers        <- caches
  |-- sweepers, presence, voice, shard, application, user
```

Runtime dependencies (`package.json`):

| Dependency | Use |
|---|---|
| `@discordjs/collection` | Ordered Map plus helpers |
| `@discordjs/formatters` | `channelMention`, `userMention`, `roleMention`, `formatEmoji`, `channelLink`, `messageLink`, and so on |
| `@discordjs/rest`, `@discordjs/ws`, `@discordjs/util` | HTTP, gateway, `calculateShardId`, `lazy`, `isJSONEncodable`, `GatewayRateLimitError` |
| `@sapphire/snowflake` | `DiscordSnowflake.timestampFrom`, `.generate`, `Snowflake.compare` |
| `@vladfrangu/async_event_emitter` | Emitter base |
| `discord-api-types` | Enums, routes, flags |
| `fast-deep-equal` | `Embed`, `Component`, `ApplicationCommand` equality |
| `magic-bytes.js` | Soundboard mime sniffing |
| `undici` | `fetchRecommendedShardCount` |

## 2. Client (`client/Client.js`, `util/Options.js`)

### 2.1 `ClientOptions`

Merging is shallow, except that `presence`, `sweepers`, `ws` and `rest` are each merged one level deep.

| Option | Default | Notes |
|---|---|---|
| `intents` | required | `new IntentsBitField(options.intents ?? options.ws.intents).freeze()`. Missing gives `ClientMissingIntents`. |
| `partials` | `[]` | Array of `Partials` values; must be an array. |
| `makeCache` | `cacheWithLimits(DefaultMakeCacheSettings)` (`MessageManager: 200`) | Called once per manager instance. |
| `sweepers` | `DefaultSweeperSettings` (`threads: {interval: 3600, lifetime: 14400}`) | Seconds. |
| `waitGuildTimeout` | `15_000` | ms; must be a non-NaN number. |
| `failIfNotExists` | `true` | Default for reply `message_reference.fail_if_not_exists`. |
| `enforceNonce` | `false` | Auto-generates a nonce and sets `enforce_nonce`. |
| `allowedMentions` | undefined | Default for every payload. |
| `presence` | undefined | Parsed by `ClientPresence._parse` into `ws.initialPresence` unless that is already set. |
| `jsonTransformer` | `toSnakeCase` | Applied to plain-object embeds, components and modals, and to autocomplete choices. |
| `closeTimeout` | `5_000` | **Dead option.** Only `util/Options.js` mentions it; nothing under `discord.js/src`, `ws/src`, `rest/src` or `util/src` reads it. Do not port. |
| `ws` | `DefaultWebSocketManagerOptions` plus `largeThreshold: 50`, `version: 10` | See below. |
| `rest` | `DefaultRestOptions`; `userAgentAppendix` = `discord.js/<version> <default>` plus the user's appendix | See below. |

Defaults that matter, from `packages/ws/src/utils/constants.ts`:
`capabilities: 0`, `shardCount: null`, `shardIds: null`, `identifyProperties {browser, device: DefaultDeviceProperty, os: process.platform}`, `encoding: json`, `compression: null`, `useIdentifyCompression: false`, `SimpleIdentifyThrottler(session_start_limit.max_concurrency)`, `SimpleShardingStrategy`.

Defaults that matter, from `packages/rest/src/lib/utils/constants.ts`:
`api https://discord.com/api`, `cdn https://cdn.discordapp.com`, `mediaProxy https://media.discordapp.net`, `authPrefix Bot`, `globalRequestsPerSecond 50`, `offset 50`, `retries 3`, `retryBackoff 0`, `timeout 15000`, `rejectOnRateLimit false`, `hashSweepInterval 4h`, `hashLifetime 24h`, `handlerSweepInterval 1h`.

Environment inputs:

- `DISCORD_TOKEN` is used in preference to `options.ws.token`.
- `SHARDS` (JSON; a scalar is wrapped into an array) and `SHARD_COUNT` apply only if `ws.shardIds` and `ws.shardCount` are identity-equal to the defaults.
- `SHARDING_MANAGER` and `SHARDING_MANAGER_MODE` control `client.shard`.
- In worker mode the `SHARDS` and `SHARD_COUNT` values come from `workerData`, not `process.env`.

### 2.2 Fields

`options`, `rest`, `presence: ClientPresence`, `status: Status`, `actions`, `users`, `guilds`, `channels`, `sweepers`, `token` (non-enumerable), `ws`, `shard: ?ShardClientUtil`, `voice: ClientVoiceManager`, `user: ?ClientUser`, `application: ?ClientApplication`, `pings: Collection<shardId, ms>`, `lastPingTimestamps`, `readyTimestamp`.

Getters: `readyAt`, `uptime`, `ping` (mean of all shards, or null).

Hidden: `expectedGuilds: Set`, `readyTimeout`, `incomingPacketQueue: []`.

`Status` (`util/Status.js`) is `Ready=0, Idle=1, WaitingForGuilds=2`.

### 2.3 Public methods

| Method | Behaviour |
|---|---|
| `login(token = this.token)` | Strips `/^bot\s*/i`; sets the token on REST and WS; `GET /gateway/bot`; `ws.connect({gatewayInformation})`. On failure calls `destroy()` and rethrows. Non-string or empty token gives `TokenInvalid`. Resolves as soon as `ws.connect` resolves, **not** at `clientReady`. |
| `destroy()` | Clears REST sweepers, `sweepers.destroy()`, `await ws.destroy()`, nulls the token. Does not clear caches or reset `status`. |
| `isReady()` | `status === Ready`. |
| `deleteWebhook(id, {token, reason})` | `DELETE /webhooks/{id}[/{token}]`, `auth: !token`. |
| `fetchInvite(invite, {withCounts, guildScheduledEventId})` | Via `resolveInviteCode`; `createInvite` picks `GuildInvite`, `GroupDMInvite` or `BaseInvite` by `data.type`. |
| `fetchGuildTemplate(tpl)` | `resolveGuildTemplateCode`. |
| `fetchWebhook(id, token)` | `auth: token === undefined`; `new Webhook(this, {token, ...data})`. |
| `fetchVoiceRegions()`, `fetchSticker(id)`, `fetchStickerPacks({packId})`, `fetchDefaultSoundboardSounds()` | Uncached fetches. |
| `fetchGuildPreview(guild)`, `fetchGuildWidget(guild)` | Resolve the guild id; otherwise `InvalidType`. |
| `generateInvite(options)` | See §2.6. |
| `toJSON()` | `flatten` without `actions` and `presence`. |
| `_eval(script)` | Used by sharding; not portable (§10.3). |
| `_broadcast(packet)` | Sends to every shard id in `ws.getShardIds()`. |
| `incrementMaxListeners` / `decrementMaxListeners` | Node emitter housekeeping; not applicable to Rust. |
| `[Symbol.asyncDispose]` | `destroy()`. |

### 2.4 Ready state machine and packet pipeline

`_handlePacket(packet, shardId)` (`Client.js:405`) receives every dispatch from `ws.on(Dispatch)`:

1. If `status !== Ready` and `t` is not in `BeforeReadyWhitelist` (`READY, RESUMED, GUILD_CREATE, GUILD_DELETE, GUILD_MEMBERS_CHUNK, GUILD_MEMBER_ADD, GUILD_MEMBER_REMOVE`), push to `incomingPacketQueue` and stop.
2. Otherwise, if the queue is non-empty, shift **one** item and schedule `_handlePacket(item)` on `setImmediate`.
3. Run `PacketHandlers[t]` if one exists. Unknown events are silently ignored.
4. If `t === READY`, call `_checkReady()`. Else if `status === WaitingForGuilds` and `t` is `GUILD_CREATE` or `GUILD_DELETE`, remove `d.id` from `expectedGuilds` and call `_checkReady()`.

`_checkReady()`:

1. Clear `readyTimeout`.
2. If `expectedGuilds` is empty **and** `await ws.fetchStatus()` shows every shard `Ready`, call `_triggerClientReady()`.
3. Otherwise arm a timer: `hasGuildsIntent ? waitGuildTimeout : 0`. When it fires, mark ready regardless of how many guilds are still unavailable.

`_triggerClientReady()` sets `status = Ready`, sets `readyTimestamp = now`, and emits `clientReady` with the client.

`READY` handler (`handlers/READY.js`):

- Patch or create `client.user` (a `ClientUser`, also inserted into `users.cache`).
- For each guild in `d.guilds`: add it to `expectedGuilds`, set `shardId`, and call `guilds._add`.
- Patch or create `client.application`.
- Set `status = WaitingForGuilds`. This runs on every `READY`, including re-identifies after a session loss.

Heartbeat events update `pings` and `lastPingTimestamps`. WS debug messages are re-emitted on `debug` with prefix `[WS => Shard N]`.

`ClientVoiceManager` (`client/voice/ClientVoiceManager.js`):

- `adapters: Map<guildId, adapter>`, filled by `Guild.voiceAdapterCreator`.
- Listens to `ws Closed`. On `CloseCodes.Normal` it destroys the adapters of guilds on that shard.
- `onVoiceServer` and `onVoiceStateUpdate` forward payloads. State updates are forwarded only when `user_id` is the client and both `guild_id` and `session_id` are present.

### 2.5 `Events` (`util/Events.js`)

Note the wire names that differ from the keys: `GuildEmojiCreate → 'emojiCreate'`, `GuildRoleCreate → 'roleCreate'`, `GuildStickerCreate → 'stickerCreate'`, `MessageBulkDelete → 'messageDeleteBulk'`, and so on.

- **Declared but never emitted by `src/`:** `Invalidated`.
- **`Error`:** emitted only from `ShardClientUtil._respond`.
- **`Warn`:** emitted from `TypingStart` and `ShardClientUtil.singleton`.

Client-level events, by group:

- ApplicationCommandPermissionsUpdate
- AutoModeration: ActionExecution, RuleCreate, RuleDelete, RuleUpdate
- CacheSweep
- Channel: Create, Delete, PinsUpdate, Update
- ClientReady, Debug
- Entitlement: Create, Delete, Update
- Error
- GuildAuditLogEntryCreate
- Guild: Available, BanAdd, BanRemove, Create, Delete
- GuildEmoji: Create, Delete, Update
- GuildIntegrationsUpdate
- GuildMember: Add, Available, Remove, Update; GuildMembersChunk
- GuildRole: Create, Delete, Update
- GuildScheduledEvent: Create, Delete, Update, UserAdd, UserRemove
- GuildSoundboardSound: Create, Delete, Update; GuildSoundboardSoundsUpdate
- GuildSticker: Create, Delete, Update
- GuildUnavailable, GuildUpdate, InteractionCreate, Invalidated
- Invite: Create, Delete
- Message: BulkDelete, Create, Delete, PollVoteAdd, PollVoteRemove, ReactionAdd, ReactionRemove, ReactionRemoveAll, ReactionRemoveEmoji, Update
- PresenceUpdate, SoundboardSounds
- StageInstance: Create, Delete, Update
- Subscription: Create, Delete, Update
- Thread: Create, Delete, ListSync, MembersUpdate, MemberUpdate, Update
- TypingStart, UserUpdate, VoiceChannelEffectSend, VoiceServerUpdate, VoiceStateUpdate, Warn, WebhooksUpdate

### 2.6 `generateInvite` rules

The checks run in this order:

1. Options must be an object.
2. `application` must be set, otherwise `ClientNotReady`.
3. `scopes` undefined gives `InvalidMissingScopes`; a non-array gives `InvalidType`.
4. At least one scope must be `bot` or `applications.commands` (`InvalidMissingScopes`).
5. `permissions` without `bot` gives `InvalidScopesWithPermissions`.
6. Each scope must be a known `OAuth2Scopes` value (`InvalidElement`).

The result is `${rest.api}/oauth2/authorize?client_id&scope=<space-joined>&disable_guild_select[&permissions][&guild_id]`. `permissions` is omitted when it resolves to `0n`.

## 3. Actions and packet handlers: how the cache is mutated

### 3.1 The mechanism

`client/actions/Action.js` explains why actions exist: packet handlers and REST methods must share logic, so the two paths do not clash. An action's `handle(data)` mutates the cache, may emit, and returns the affected entities. REST methods call the same action with the response body, so the later gateway echo finds the entity already present and stays silent.

Helpers on `Action`:

| Helper | Semantics |
|---|---|
| `getPayload(data, manager, id, partialType, cache)` | **The partial gate.** If `partials.includes(partialType)`, returns `manager._add(data, cache)`; else `manager.cache.get(id)`. |
| `getChannel(data)` | `id = channel_id ?? id`. If `recipients` is present, appends the author, user or `{id: user_id}` when not already there. If `type` is DM or GroupDM, sets `recipients: [author/user/{id:user_id}]`. Injected channel wins; else gate on `Partials.Channel`. |
| `getMessage(data, channel, cache)` | Injected message wins; else gate on `Partials.Message`, with a skeleton `{id, channel_id, guild_id}` via `channel.messages`. |
| `getPoll(data, message, channel)` | Null if the message is partial and either poll partial is off. Creates a `Poll` if `Partials.Poll`; creates a `PollAnswer` if `Partials.PollAnswer`. |
| `getReaction(data, message, user)` | `id = emoji.id ?? decodeURIComponent(emoji.name)`; skeleton `{emoji, count: message.partial ? null : 0, me}`; gate on `Partials.Reaction`. |
| `getMember(data, guild)` | Gate on `Partials.GuildMember`. |
| `getUser(data)` | Injected user wins; skeleton `{id: user_id}`; gate on `Partials.User`. |
| `getUserFromMember(data)` | If `guild_id` and `member.user` are present: `guild.members._add(member).user` when the guild is cached, else `users._add(member.user)`. Otherwise `getUser`. |
| `getScheduledEvent`, `getThreadMember(id, manager)`, `getSoundboardSound` | Gates on their partial. `getThreadMember` passes `cache = false`. |
| `spreadInjectedData(data)` | Copies symbol keys. |

`ActionsManager` declares three `Symbol`s, `injectedUser`, `injectedChannel` and `injectedMessage`. They carry already-built structures into an action. Only `Message.react` uses them (with `fromStructure = true`, which suppresses the event). In Rust, use an explicit context argument.

### 3.2 Action table

"REST caller" lists the methods that reuse the action.

| Action (`client/actions/*.js`) | Cache effect | Event | REST caller / dedupe |
|---|---|---|---|
| `ChannelCreate` | `channels._add` | `channelCreate` only if not already cached | `GuildChannelManager.create` |
| `ChannelDelete` | `channels._remove` | `channelDelete` if it was cached | `GuildChannelManager.delete`; gateway echo finds nothing |
| `ChannelUpdate` | `_update`, returning `{old, updated}`. On type change rebuilds via `createChannel`, copies messages, re-inserts. Uncached: `_add` and returns `{}`. | Emitted by the handler (`channelUpdate` or `threadUpdate`) | `GuildChannelManager.edit`, `ThreadChannel.edit` (no event from REST) |
| `GuildChannelsPositionUpdate` | Sets `rawPosition` | none | `setPosition`, `setPositions` |
| `GuildEmojiCreate`, `GuildStickerCreate` | `_add` | Only if not `already` | `create` on the manager |
| `GuildEmojiUpdate`, `GuildStickerUpdate` | `_update` | Always | internal |
| `GuildEmojiDelete`, `GuildStickerDelete` | `cache.delete` | Always | internal |
| `GuildEmojisUpdate`, `GuildStickersUpdate` | Diffs the full list against the cache, calling create/update/delete | Via sub-actions | also from `Guild._patch` on re-patch |
| `GuildMemberRemove` | `memberCount--` always; removes the member, presence and voice state | `guildMemberRemove` if the member is known | |
| `GuildMemberUpdate` | Syncs the user. Member known: `_update`. Unknown: `_add`. | Known: `guildMemberUpdate` only if `!equals(old)`. Unknown: `guildMemberAvailable`. | |
| `GuildRoleCreate` | `_add` | Only if not `already` | `RoleManager.create` |
| `GuildRoleDelete` | `cache.delete` | If cached | `RoleManager.delete` |
| `GuildRolesPositionUpdate` | Sets `rawPosition` | none | role position setters |
| `GuildScheduledEventDelete` | `cache.delete` (partial gate) | Yes | |
| `GuildScheduledEventUserAdd` / `UserRemove` | none | If event and user are resolved | |
| `GuildSoundboardSoundDelete` | `cache.delete(soundId)` | If resolved | |
| `GuildUpdate` | `_update` | **Always** `guildUpdate` | `Guild.edit` (so REST edits emit, then the gateway echo emits again) |
| `InteractionCreate` | Builds the class (§12); may cache the channel | `interactionCreate` | |
| `MessageCreate` | See §3.4 | `messageCreate` | |
| `MessageDelete` | `messageCount--` on threads, even if the message is unknown; `cache.delete` | If resolved | |
| `MessageDeleteBulk` | Channel from cache only; `messageCount -= ids.length` on threads; partial messages created with `cache = false` | `messageDeleteBulk` if any resolved | |
| `MessagePollVoteAdd` / `Remove` | `answer.voters` add/delete, `voteCount` ±1 (floor 0 on remove) | `(answer, userId)` | |
| `MessageReactionAdd` | `message.reactions._add`, `reaction._add` | Unless `fromStructure` | `Message.react` |
| `MessageReactionRemove` | `reaction._remove` | Always if resolved | |
| `MessageReactionRemoveAll` | Clones then clears | `(message, removed)` | |
| `MessageReactionRemoveEmoji` | `cache.delete` unless the message is partial | `(reaction)` | |
| `MessageUpdate` | `_update` | By the handler | |
| `StageInstanceCreate` / `Update` / `Delete` | `_add`, clone-and-add, add-then-delete | Yes | |
| `ThreadCreate` | `channels._add` | Only if not `existing`; second argument `newly_created ?? false` | `GuildTextThreadManager.create`, `GuildForumThreadManager.create` |
| `ThreadMembersUpdate` | `memberCount`; adds and removes | Skipped if both sets are empty | |
| `TypingStart` | none; builds `Typing` | `typingStart` (or `warn` on a non-text channel) | |
| `UserUpdate` | `_update`; targets `client.user` when the id matches | Only if `!equals` | `ClientUser.edit` |

### 3.3 Packet handler table (`client/websocket/handlers/`, 72 entries)

Handlers that only delegate (no logic in the handler):

| Packet | Action |
|---|---|
| `CHANNEL_CREATE`, `CHANNEL_DELETE` | `ChannelCreate`, `ChannelDelete` |
| `GUILD_EMOJIS_UPDATE`, `GUILD_STICKERS_UPDATE` | `GuildEmojisUpdate`, `GuildStickersUpdate` |
| `GUILD_MEMBER_REMOVE`, `GUILD_MEMBER_UPDATE` | `GuildMemberRemove`, `GuildMemberUpdate` |
| `GUILD_ROLE_CREATE`, `GUILD_ROLE_DELETE` | `GuildRoleCreate`, `GuildRoleDelete` |
| `GUILD_SCHEDULED_EVENT_DELETE`, `_USER_ADD`, `_USER_REMOVE` | `GuildScheduledEventDelete`, `...UserAdd`, `...UserRemove` |
| `GUILD_SOUNDBOARD_SOUND_DELETE` | `GuildSoundboardSoundDelete` |
| `GUILD_UPDATE`, `INTERACTION_CREATE` | `GuildUpdate`, `InteractionCreate` |
| `MESSAGE_CREATE`, `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK` | `MessageCreate`, `MessageDelete`, `MessageDeleteBulk` |
| `MESSAGE_POLL_VOTE_ADD`, `MESSAGE_POLL_VOTE_REMOVE` | `MessagePollVoteAdd`, `MessagePollVoteRemove` |
| `MESSAGE_REACTION_ADD`, `_REMOVE`, `_REMOVE_ALL`, `_REMOVE_EMOJI` | Same-named actions |
| `STAGE_INSTANCE_CREATE`, `_UPDATE`, `_DELETE` | Same-named actions |
| `THREAD_CREATE`, `THREAD_MEMBERS_UPDATE` | `ThreadCreate`, `ThreadMembersUpdate` |
| `TYPING_START`, `USER_UPDATE` | `TypingStart`, `UserUpdate` |

Handlers with their own logic. All "guild-scoped" handlers return silently if the guild is not cached.

| Packet | Behaviour |
|---|---|
| `READY` | §2.4. |
| `RATE_LIMITED` | `RequestGuildMembers` opcode: ignored here (the manager handles it). Other opcodes: `process.emitWarning` once per opcode, in a module-global set. |
| `APPLICATION_COMMAND_PERMISSIONS_UPDATE` | Emits a plain object `{permissions, id, guildId, applicationId}`; no cache. |
| `AUTO_MODERATION_ACTION_EXECUTION` | `new AutoModerationActionExecution(data, guild)`; emit. |
| `AUTO_MODERATION_RULE_CREATE` / `UPDATE` / `DELETE` | `_add` / clone (or null) then `_add` / `cache.delete` (silent if uncached). |
| `CHANNEL_PINS_UPDATE` | Channel from cache; sets `lastPinTimestamp` (null when the last pin was removed); `channelPinsUpdate(channel, time)`. |
| `CHANNEL_UPDATE`, `THREAD_UPDATE` | Via `ChannelUpdate`; emit only if `old && updated`. |
| `MESSAGE_UPDATE` | Via `MessageUpdate`; emit only if `old && updated`. |
| `ENTITLEMENT_CREATE` / `UPDATE` / `DELETE` | On `client.application.entitlements`. Delete does `_add(data, false)` then `cache.delete`. |
| `SUBSCRIPTION_CREATE` / `UPDATE` / `DELETE` | Same pattern, on `client.application.subscriptions`. |
| `GUILD_AUDIT_LOG_ENTRY_CREATE` | `new GuildAuditLogsEntry(guild, data)` (no `logs`); emit `(entry, guild)`. |
| `GUILD_BAN_ADD` | `guild.bans._add(data)`; emit. |
| `GUILD_BAN_REMOVE` | Cached ban or a new `GuildBan`; `cache.delete`; emit. |
| `GUILD_CREATE` | Known guild: patch and emit `guildAvailable` **only if** `!available && !data.unavailable`; otherwise the packet is dropped. New guild: inject `data.shardId`, `_add`, and emit `guildCreate` **only if `status === Ready`**. |
| `GUILD_DELETE` | `data.unavailable`: `available = false`, emit `guildUnavailable`, stop. Otherwise `channels._remove` for each guild channel, destroy the voice adapter, `guilds.cache.delete`, emit `guildDelete`. |
| `GUILD_INTEGRATIONS_UPDATE` | Emit `(guild)`. |
| `GUILD_MEMBERS_CHUNK` | `_add` all members; `presences._add` with `guild` injected; emit `(members, guild, {index, count, notFound, nonce})`. |
| `GUILD_MEMBER_ADD` | `memberCount++`, `_add`, emit. |
| `GUILD_ROLE_UPDATE` | Silent if role uncached; `_update`; **always** emit `(old, role)`. |
| `GUILD_SCHEDULED_EVENT_CREATE` / `UPDATE` | `_add` / clone (or null) then `_add`. |
| `GUILD_SOUNDBOARD_SOUND_CREATE` / `UPDATE` | `_add` / clone then `_add`, keyed by `sound_id`. |
| `GUILD_SOUNDBOARD_SOUNDS_UPDATE`, `SOUNDBOARD_SOUNDS` | `_add` all; emit `(sounds, guild)` under their own event names. |
| `INVITE_CREATE` / `INVITE_DELETE` | Return if the channel is uncached. Inject `channel` and `guild` into the raw object. Create: `guild.invites._add`. Delete: `new GuildInvite`, `cache.delete(code)`. |
| `PRESENCE_UPDATE` | §16 item 22. |
| `THREAD_DELETE` | `channels._remove`; emit. |
| `THREAD_LIST_SYNC` | Removes **non-archived** cached threads of the listed channels (or all guild channels if `channel_ids` is absent), re-adds `threads`, patches `members` (each raw member's `id` is the thread id), emits `(synced, guild)`. |
| `THREAD_MEMBER_UPDATE` | `data.id` is the thread id. Unknown member: `_add` with no event. Known: `_update`, emit. |
| `VOICE_CHANNEL_EFFECT_SEND` | `new VoiceChannelEffect(data, guild)`; emit. |
| `VOICE_SERVER_UPDATE` | Debug log with the token masked; `voice.onVoiceServer`; emit `{guildId, endpoint, token}`. |
| `VOICE_STATE_UPDATE` | Old state is a clone or a synthetic `new VoiceState(guild, {user_id})`. `voiceStates._add`. Member: `_patch` if cached and `data.member` is present; else `_add` if `member.user && member.joined_at`. Self: `voice.onVoiceStateUpdate`. Emit `(old, new)`. The state is **not** removed on leave; `channelId` becomes null. |
| `WEBHOOKS_UPDATE` | Emit `(channel)` if the channel is cached. |

### 3.4 `MessageCreate` in detail

1. `getChannel({id, author, guild_id?, type: channel_type?})`. Without `Partials.Channel`, an uncached channel means the message is dropped silently. This is why DMs need the partial.
2. Non-text channel: return `{}`.
3. Threads: `messageCount++`, `totalMessageSent++`, incremented before the duplicate check.
4. `existing = channel.messages.cache.get(id)`. If it exists **and** its author is not the client, return with no event.
5. `message = existing ?? messages._add(data)`. `channel.lastMessageId = data.id`. Emit `messageCreate`.

Consequence: the bot's own REST send caches the message first, and the gateway echo still emits `messageCreate` (once), using the cached object.

## 4. Partials (`util/Partials.js`)

Values are `createEnum` indices: `User 0, Channel 1, GuildMember 2, Message 3, Reaction 4, GuildScheduledEvent 5, ThreadMember 6, Poll 7, PollAnswer 8, SoundboardSound 9`.

A partial is a structure built from an id-only skeleton. `_patch` tolerates this because every field is guarded by `'x' in data`.

| Structure | `partial` getter | Upgrade |
|---|---|---|
| `User` | `typeof username !== 'string'` | `fetch()` |
| `BaseChannel` | `false` | n/a |
| `DMChannel` | `lastMessageId === undefined` | `fetch()` calls `createDM` |
| `GuildMember` | `joinedTimestamp === null` | `fetch()` |
| `Message` | `typeof content !== 'string' \|\| !author` | `fetch()` |
| `MessageReaction` | `count === null` | `fetch()` (re-fetches the message; absent reaction means `count: 0`) |
| `GuildScheduledEvent` | `name === null` | `fetch()` |
| `ThreadMember` | `flags === null` | manager `fetch` |
| `Poll` | `allowMultiselect === null` | `fetch()` |
| `PollAnswer` | `poll.partial \|\| (text === null && emoji === null)` | via poll |
| `GuildBan` | `!('reason' in this)` | `fetch()` |
| `Sticker` | `!type` | `fetch()` |
| `ClientApplication` | `!name` | `fetch()` |

Sites gated by `Partials` outside `Action.js`:

- `GuildMemberManager.me`: synthesizes `{user: {id}}` when `Partials.GuildMember` is on.
- `DMChannel._patch`: caches recipients without `username` only with `Partials.User`.
- `PRESENCE_UPDATE`: same for users.
- `GuildAuditLogsEntry`: `executor` and `target`, with `Partials.User`.

`Partials.Channel` is documented as "required to receive direct messages".

**Uncached DM reaction, typing, delete and update packets.** `MessageReactionAdd`, `MessageReactionRemove`, `MessageReactionRemoveAll`, `MessageReactionRemoveEmoji`, `MessageDelete`, `MessageUpdate`, `TypingStart` and the poll actions pass `getChannel` no `type` and no `recipients`. So `createChannel` builds nothing for an uncached DM channel, even with `Partials.Channel` on, and returns null with a debug line. Only `MessageCreate` has `channel_type`.

## 5. Structures

### 5.1 Conventions a port must model

- **Constructor plus `_patch(data)`:** `_patch` is idempotent. Every optional field follows `if ('x' in data) this.x = data.x; else this.x ??= null`.
- **`Base`:**
  - `_clone()` is `Object.assign(Object.create(this), this)`: shallow, with the original as prototype.
  - `_update(data)` clones, patches `this`, and returns the clone. So **the cached object keeps its identity and the returned one is the old state**.
  - `valueOf()` returns the id.
  - `toJSON` is `flatten`.
- **Deep-clone overrides:** `GuildChannel` (overwrites), `GuildMember` and `GuildEmoji` (`_roles`), `Presence` and `Activity`. `Message._patch` re-copies its collections even when no new data is present, so that the shallow clone keeps the old ones.
- **Back-reference:** every structure holds a non-enumerable `client`.
- **Derived data via getters:** `channel`, `guild`, `member`, `parent`, `position`, `members` and `roles` are computed on read from the caches, so a removed entity yields null.
- **Mixins (`applyToClass`):** `TextBasedChannel`, `InteractionResponses`, `Webhook`. Each copies property descriptors, with an ignore list.
- **Timestamps** are numbers (ms) with paired `...At` Date getters. Snowflake timestamps come from `DiscordSnowflake.timestampFrom`.

### 5.2 Inheritance tree

Classes marked `[T]` have the `TextBasedChannel` mixin; `[R]` have `InteractionResponses`.

```
Base
├─ User ─ ClientUser
├─ BaseChannel
│  ├─ GuildChannel
│  │  ├─ BaseGuildTextChannel [T] ─ TextChannel, AnnouncementChannel
│  │  ├─ BaseGuildVoiceChannel [T minus lastPinAt] ─ VoiceChannel, StageChannel
│  │  ├─ CategoryChannel
│  │  └─ ThreadOnlyChannel [T keeps only fetchWebhooks, createWebhook, setRateLimitPerUser, setNSFW] ─ ForumChannel, MediaChannel
│  ├─ DirectoryChannel            (guild-scoped but NOT a GuildChannel)
│  ├─ ThreadChannel [T minus fetchWebhooks, setRateLimitPerUser, setNSFW]  (NOT a GuildChannel)
│  ├─ DMChannel [T minus bulkDelete, fetchWebhooks, createWebhook, setRateLimitPerUser, setNSFW]
│  └─ PartialGroupDMChannel [T minus bulkDelete, send, sendTyping, createMessageCollector, awaitMessages, fetchWebhooks, createWebhook, setRateLimitPerUser, setNSFW]
├─ BaseGuild
│  ├─ AnonymousGuild ─ Guild, InviteGuild
│  └─ OAuth2Guild
├─ BaseRole ─ Role, InviteRole
├─ Emoji
│  ├─ BaseGuildEmoji ─ GuildEmoji, GuildPreviewEmoji
│  ├─ ApplicationEmoji, ReactionEmoji
├─ BaseInvite ─ GuildInvite, GroupDMInvite
├─ BaseInteraction
│  ├─ CommandInteraction [R minus deferUpdate, update]
│  │  ├─ ChatInputCommandInteraction
│  │  ├─ ContextMenuCommandInteraction ─ UserContextMenuCommandInteraction, MessageContextMenuCommandInteraction
│  │  └─ PrimaryEntryPointCommandInteraction
│  ├─ MessageComponentInteraction [R]
│  │  └─ ButtonInteraction, StringSelectMenuInteraction, UserSelectMenuInteraction,
│  │     RoleSelectMenuInteraction, MentionableSelectMenuInteraction, ChannelSelectMenuInteraction
│  ├─ ModalSubmitInteraction [R minus showModal]
│  └─ AutocompleteInteraction     (own respond(); no mixin)
├─ interfaces/Application ─ ClientApplication, IntegrationApplication
├─ Message, GuildMember, ThreadMember, GuildBan, PermissionOverwrites, Presence ─ ClientPresence
├─ Poll, PollAnswer, Typing, VoiceState, StageInstance, SoundboardSound, Sticker, StickerPack
├─ ApplicationCommand, AutoModerationRule, GuildScheduledEvent, GuildTemplate, GuildPreview, Integration
├─ Entitlement, Subscription, SKU, Team, TeamMember, Widget, WidgetMember, WelcomeChannel, WelcomeScreen
├─ GuildOnboarding, GuildOnboardingPrompt, GuildOnboardingPromptOption
└─ ActivityInstance, ActivityLocation, AuthorizingIntegrationOwners

Component  (no Base; wraps API data)
├─ ActionRow, ButtonComponent, TextInputComponent, TextDisplayComponent, FileComponent, ThumbnailComponent
├─ SeparatorComponent, SectionComponent, ContainerComponent, MediaGalleryComponent, LabelComponent
└─ BaseSelectMenuComponent ─ StringSelectMenuComponent, UserSelectMenuComponent, RoleSelectMenuComponent,
                             MentionableSelectMenuComponent, ChannelSelectMenuComponent

interfaces/Collector (AsyncEventEmitter) ─ MessageCollector, ReactionCollector, InteractionCollector

No base class: Embed, Attachment, MessageMentions, MessagePayload, MessageReaction, Webhook, InteractionWebhook,
  InteractionCallback, InteractionCallbackResource, InteractionCallbackResponse, CommandInteractionOptionResolver,
  ModalComponentResolver, GuildAuditLogs, GuildAuditLogsEntry, AutoModerationActionExecution, VoiceRegion,
  VoiceChannelEffect, ApplicationRoleConnectionMetadata, MediaGalleryItem, UnfurledMediaItem,
  Activity and RichPresenceAssets (both in Presence.js)
```

`MediaChannel` is an empty subclass of `ThreadOnlyChannel`. `ForumChannel` adds `defaultForumLayout`.

`channel.isTextBased()` is `'messages' in this`, `isVoiceBased()` is `'bitrate' in this`, `isThreadOnly()` is `'availableTags' in this`, `isSendable()` is `'send' in this`. These duck-typed checks must become explicit capability traits or methods in Rust, and `'bitrate' in this` depends on the payload having included `bitrate`.

### 5.3 Key methods by structure

**`Client`-adjacent identity**

- `User`: `partial`, `avatarURL`, `avatarDecorationURL`, `defaultAvatarURL`, `displayAvatarURL`, `bannerURL`, `guildTagBadgeURL`, `tag`, `displayName`, `hexAccentColor`, `dmChannel`, `createDM(force)`, `deleteDM`, `send`, `equals`, `_equals(raw)`, `fetch(force=true)`.
- `ClientUser`: `edit({username, avatar, banner})`, `setUsername`, `setAvatar`, `setBanner`, `presence`, `setPresence`, `setStatus(status, shardId)`, `setActivity(name, opts)`, `setAFK`.
- `GuildMember`: `roles` (fresh manager per read), `voice` (synthetic if none), `presence`, `permissions`, `permissionsIn`, `manageable`, `kickable`, `bannable`, `moderatable`, `displayName`, `displayColor`, `displayHexColor`, `isCommunicationDisabled`, `edit`, `setNickname`, `setFlags`, `kick`, `ban`, `timeout(ms, reason)`, `disableCommunicationUntil`, `createDM`, `deleteDM`, `send`, `fetch`, `equals`, avatar, banner and decoration URLs.

**Channels**

- `BaseChannel`: `createdTimestamp`, `url`, `partial`, `delete`, `fetch(force=true)`, the `is*` guards, `flags`, `permissions`, `appPermissions`.
- `GuildChannel`: `parent`, `permissionsLocked`, `position`, `permissionsFor(memberOrRole, checkAdmin=true)`, `overwritesFor`, `memberPermissions`, `rolePermissions`, `lockPermissions`, `members`, `edit`, `setName`, `setParent`, `setPosition`, `clone`, `equals`, `deletable`, `manageable`, `viewable`, `delete(reason)`.
- `TextBasedChannel` mixin: `lastMessage`, `lastPinAt`, `send`, `sendTyping`, `createMessageCollector`, `awaitMessages`, `createMessageComponentCollector`, `awaitMessageComponent`, `bulkDelete`, `fetchWebhooks`, `createWebhook`, `setRateLimitPerUser`, `setNSFW`.
- `BaseGuildTextChannel`: `setDefaultAutoArchiveDuration`, `setType`, `setTopic`, `createInvite`, `fetchInvites`, plus `threads`. `TextChannel`: `rateLimitPerUser`. `AnnouncementChannel`: `addFollower`.
- `BaseGuildVoiceChannel`: `members` (from voice states), `full`, `joinable`, `createInvite`, `fetchInvites`, `setBitrate`, `setRTCRegion`, `setUserLimit`, `setVideoQualityMode`.
- `VoiceChannel`: `joinable`, `speakable`, `sendSoundboardSound`. `StageChannel`: `stageInstance`, `createStageInstance`, `setTopic`. `CategoryChannel`: `children`.
- `ThreadChannel`: `members`, `guildMembers`, `parent`, `join`, `leave`, `permissionsFor` (delegates to parent), `fetchOwner`, `fetchStarterMessage`, `edit`, `setArchived`, `setAutoArchiveDuration`, `setInvitable`, `setLocked`, `setName`, `setRateLimitPerUser`, `setAppliedTags`, `pin`, `unpin`, `joined`, `editable`, `joinable`, `manageable`, `viewable`, `sendable`, `unarchivable`, `delete`.
- `ThreadOnlyChannel`: `threads`, `setAvailableTags`, `setDefaultReactionEmoji`, `setDefaultThreadRateLimitPerUser`, `createInvite`, `fetchInvites`, `setDefaultAutoArchiveDuration`, `setTopic`, `setDefaultSortOrder`.
- `DMChannel`: `recipientIds`, `recipientId`, `recipient`, `fetch`. `PartialGroupDMChannel`: `iconURL`, `fetchOwner`; `delete` and `fetch` throw.

**Messages**

- `Message`: `channel`, `partial`, `member`, `guild`, `hasThread`, `thread`, `url`, `cleanContent`, `createReactionCollector`, `awaitReactions`, `createMessageComponentCollector`, `awaitMessageComponent`, `editable`, `deletable`, `bulkDeletable`, `pinnable`, `crosspostable`, `fetchReference`, `edit`, `crosspost`, `pin`, `unpin`, `react`, `delete`, `reply`, `forward(channel)`, `startThread`, `fetch`, `fetchWebhook`, `suppressEmbeds`, `removeAttachments`, `resolveComponent`, `equals(message, rawData)`, `inGuild`.
- `MessageMentions`: `everyone`, `users`, `roles`, `crosspostedChannels`, `repliedUser`, lazily computed `members`, `channels`, `parsedUsers`, `has(data, {ignoreDirect, ignoreRoles, ignoreRepliedUser, ignoreEveryone})`, static patterns.
- `MessageReaction`: `me`, `meBurst`, `users`, `emoji` (upgrades itself to a known emoji), `count`, `countDetails`, `burstColors`, `react`, `remove`, `fetch`, `_add`, `_remove`.
- `Poll`: `answers`, `resultsFinalized`, `allowMultiselect`, `layoutType`, `expiresTimestamp`, `question`, `fetch`, `end`. `PollAnswer`: `voters`, `voteCount`, `text`, `emoji`.

**Guild**

- `Guild` managers: `commands`, `members`, `channels`, `bans`, `roles`, `presences`, `voiceStates`, `stageInstances`, `invites`, `scheduledEvents`, `autoModerationRules`, `soundboardSounds`, and lazily created in `_patch`: `emojis`, `stickers`.
- `Guild` methods: `fetchOwner`, `fetchIntegrations`, `fetchTemplates`, `fetchWelcomeScreen`, `createTemplate`, `fetchPreview`, `fetchVanityData`, `fetchWebhooks`, `fetchWidget`, `fetchWidgetSettings`, `widgetImageURL`, `fetchAuditLogs`, `fetchOnboarding`, `edit`, `editOnboarding`, `editWelcomeScreen`, all `setX`, `setWidgetSettings`, `leave`, `disableInvites`, `setIncidentActions`, `equals`, `voiceAdapterCreator`, `maximumBitrate` (96k, 128k, 256k or 384k by tier; `VIPRegions` gives 384k), `maximumStageBitrate` (64k).
- `Role`: `members`, `editable`, `position`, `comparePositionTo`, `edit`, `permissionsIn`, setters, `delete`, `equals`. `BaseRole`: `colors {primaryColor, secondaryColor, tertiaryColor}`, `hexColor`, `iconURL`.
- `PermissionOverwrites`: `edit`, `delete`, static `resolveOverwriteOptions`, static `resolve(overwrite, guild)`.
- `VoiceState`: `member`, `channel`, `deaf`, `mute`, `setMute`, `setDeaf`, `disconnect`, `setChannel`, `edit` (stage only), `setRequestToSpeak`, `setSuppressed`, `fetch`.

**Application and misc**

- `ClientApplication`: managers `commands`, `emojis`, `entitlements`, `subscriptions`; `edit`, `fetch`, `fetchRoleConnectionMetadataRecords`, `editRoleConnectionMetadataRecords`, `fetchSKUs`, `fetchActivityInstance`.
- `ApplicationCommand`: `edit`, setters, `delete`, `equals(command, enforceOptionOrder)`, statics `optionsEqual`, `transformOption(option, received)`; `permissions` manager.
- `Entitlement`: `isActive`, `isTest`, `isUserSubscription`, `isGuildSubscription`, `fetchUser`, `consume`.
- `GuildScheduledEvent`: `coverImageURL`, `createInviteURL`, `edit`, `setX`, `fetchSubscribers`, `isActive`, `isCanceled`, `isCompleted`, `isScheduled`.
- `GuildAuditLogsEntry`: `targetType`, `actionType`, `action`, `reason`, `executor`, `changes`, `extra`, `target`; statics `Targets`, `targetType(n)`, `actionType(n)`.

### 5.4 Complete structure table

Priority: **P0** essential for bots, **P1** commonly used, **P2** niche.

| Structure (`structures/`) | Purpose | Pri |
|---|---|---|
| `Base` | Root: client reference, `_clone`, `_patch`, `_update`, `toJSON`. | P0 |
| `BaseChannel` | Root of all channels: id, type, flags, `is*` guards. | P0 |
| `GuildChannel` | Guild channel: overwrites, parent, position, permission computation. | P0 |
| `BaseGuildTextChannel` | Shared text and announcement channel logic, threads manager. | P0 |
| `TextChannel` | Guild text channel (slowmode). | P0 |
| `AnnouncementChannel` | Guild news channel; followers. | P1 |
| `BaseGuildVoiceChannel` | Voice and stage shared logic, in-channel text chat. | P1 |
| `VoiceChannel` | Guild voice channel; soundboard send. | P1 |
| `StageChannel` | Stage channel; stage instance access. | P2 |
| `CategoryChannel` | Category; children manager. | P1 |
| `DirectoryChannel` | Hub directory stub. | P2 |
| `DMChannel` | Direct message channel, recipient tracking. | P0 |
| `PartialGroupDMChannel` | Read-only group DM stub for user-installed apps. | P2 |
| `ThreadChannel` | Thread (public, private, announcement). | P0 |
| `ThreadOnlyChannel` | Abstract forum and media parent. | P1 |
| `ForumChannel` | Forum channel, layout. | P1 |
| `MediaChannel` | Media channel; empty subclass. | P2 |
| `ThreadMember` | Thread membership record. | P1 |
| `PermissionOverwrites` | Channel allow/deny for a role or member; resolve helpers. | P0 |
| `Message` | Message: fields, permissions getters, reply, edit, react. | P0 |
| `MessagePayload` | Builds request body and files from send options. | P0 |
| `MessageMentions` | Parsed mentions of a message. | P0 |
| `MessageReaction` | Reaction aggregate on a message; count bookkeeping. | P1 |
| `ReactionEmoji` | Emoji attached to a reaction. | P1 |
| `Attachment` | Uploaded file metadata. | P0 |
| `Embed` | Read-only received embed. | P0 |
| `Poll` | Message poll state. | P1 |
| `PollAnswer` | One poll answer, votes, voters. | P1 |
| `Typing` | Typing-indicator event payload. | P2 |
| `Guild` | Full guild with all managers; REST helpers. | P0 |
| `BaseGuild` | Id, name, icon, features. | P0 |
| `AnonymousGuild` | Public guild fields shared with `InviteGuild`. | P0 |
| `InviteGuild` | Guild as seen on an invite. | P2 |
| `OAuth2Guild` | Guild from `GET /users/@me/guilds`; owner and permissions. | P1 |
| `GuildPreview` | Preview of a discoverable guild. | P2 |
| `GuildPreviewEmoji` | Emoji inside a preview. | P2 |
| `GuildMember` | User in a guild: roles, nick, timeout, moderation helpers. | P0 |
| `GuildBan` | Ban record. | P1 |
| `BaseRole` | Id, name, colors, icon. | P0 |
| `Role` | Guild role: permissions, position, tags. | P0 |
| `InviteRole` | Role carried on an invite for an uncached guild. | P2 |
| `Emoji` | Base emoji; identifier and image URL. | P0 |
| `BaseGuildEmoji` | Guild emoji flags (`managed`, `available`). | P1 |
| `GuildEmoji` | Guild-owned custom emoji; roles manager. | P1 |
| `ApplicationEmoji` | Application-owned emoji. | P2 |
| `Sticker` | Sticker (guild or standard). | P1 |
| `StickerPack` | Nitro sticker pack. | P2 |
| `SoundboardSound` | Soundboard sound (guild or default). | P2 |
| `BaseInvite` | Code, inviter, expiry, URL. | P1 |
| `GuildInvite` | Guild invite: uses, target, roles, target-users file. | P1 |
| `GroupDMInvite` | Group DM invite. | P2 |
| `User` | Discord user. | P0 |
| `ClientUser` | The bot user; edit and presence helpers. | P0 |
| `Presence` | User status and activities in a guild (`Activity` and `RichPresenceAssets` in the same file). | P1 |
| `ClientPresence` | The bot's presence and its gateway packet. | P0 |
| `VoiceState` | User voice connection state in a guild. | P1 |
| `VoiceRegion` | Voice region descriptor. | P2 |
| `VoiceChannelEffect` | Voice channel emoji or soundboard effect event. | P2 |
| `StageInstance` | Live stage. | P2 |
| `Webhook` | Webhook management and execution. | P1 |
| `InteractionWebhook` | Token-based webhook for interaction follow-ups. | P0 |
| `Integration` | Guild integration record. | P2 |
| `IntegrationApplication` | Application summary inside an integration or invite. | P2 |
| `interfaces/Application` | Base application fields. | P1 |
| `ClientApplication` | The bot's application; command, emoji, entitlement and subscription managers. | P0 |
| `ApplicationCommand` | Registered slash or context command. | P0 |
| `ApplicationRoleConnectionMetadata` | Linked-role metadata record. | P2 |
| `AuthorizingIntegrationOwners` | Which install context authorized an interaction. | P1 |
| `Team` | Developer team of an application. | P2 |
| `TeamMember` | Member of a team. | P2 |
| `SKU` | Monetization SKU. | P2 |
| `Entitlement` | Monetization entitlement. | P2 |
| `Subscription` | Monetization subscription. | P2 |
| `ActivityInstance` | Running embedded activity instance. | P2 |
| `ActivityLocation` | Where an activity instance runs. | P2 |
| `GuildScheduledEvent` | Guild scheduled event. | P1 |
| `GuildTemplate` | Guild template. | P2 |
| `GuildOnboarding` | Onboarding configuration. | P2 |
| `GuildOnboardingPrompt` | Onboarding prompt. | P2 |
| `GuildOnboardingPromptOption` | Option of a prompt. | P2 |
| `WelcomeScreen` | Guild welcome screen. | P2 |
| `WelcomeChannel` | Entry in a welcome screen. | P2 |
| `Widget` | Public guild widget JSON. | P2 |
| `WidgetMember` | Member listed in a widget. | P2 |
| `GuildAuditLogs` | Audit log page: entries plus included entities. | P1 |
| `GuildAuditLogsEntry` | Single audit log entry with resolved target. | P1 |
| `AutoModerationRule` | Automod rule. | P1 |
| `AutoModerationActionExecution` | Automod action event payload. | P1 |
| `BaseInteraction` | Interaction common fields and type guards. | P0 |
| `CommandInteraction` | Application command interaction base; option resolving. | P0 |
| `ChatInputCommandInteraction` | Slash command invocation. | P0 |
| `ContextMenuCommandInteraction` | Context menu base; target id. | P0 |
| `UserContextMenuCommandInteraction` | User context menu. | P0 |
| `MessageContextMenuCommandInteraction` | Message context menu. | P0 |
| `PrimaryEntryPointCommandInteraction` | Activity entry point command. | P2 |
| `AutocompleteInteraction` | Option autocomplete request. | P0 |
| `MessageComponentInteraction` | Button or select interaction base. | P0 |
| `ButtonInteraction` | Button click. | P0 |
| `StringSelectMenuInteraction` | String select. | P0 |
| `UserSelectMenuInteraction` | User select with resolved users and members. | P1 |
| `RoleSelectMenuInteraction` | Role select. | P1 |
| `MentionableSelectMenuInteraction` | Mentionable select. | P1 |
| `ChannelSelectMenuInteraction` | Channel select. | P1 |
| `ModalSubmitInteraction` | Modal submission. | P0 |
| `ModalComponentResolver` | Typed accessors for submitted modal fields. | P0 |
| `CommandInteractionOptionResolver` | Typed accessors for command options. | P0 |
| `interfaces/InteractionResponses` | Mixin: reply, defer, edit, follow up, modal. | P0 |
| `InteractionCallback` | `with_response` interaction summary. | P1 |
| `InteractionCallbackResource` | `with_response` created resource. | P1 |
| `InteractionCallbackResponse` | `with_response` wrapper. | P1 |
| `interfaces/TextBasedChannel` | Mixin: send, typing, collectors, bulk delete, webhooks. | P0 |
| `interfaces/Collector` | Abstract collector. | P1 |
| `MessageCollector` | Collects messages in a channel. | P1 |
| `ReactionCollector` | Collects reactions on a message. | P1 |
| `InteractionCollector` | Collects interactions by filter. | P0 |
| `Component` | Base received component; wraps API JSON. | P0 |
| `ActionRow` | Row of components. | P0 |
| `ButtonComponent` | Received button. | P0 |
| `BaseSelectMenuComponent` | Received select base. | P0 |
| `StringSelectMenuComponent` | Received string select. | P0 |
| `UserSelectMenuComponent` | Received user select. | P1 |
| `RoleSelectMenuComponent` | Received role select. | P1 |
| `MentionableSelectMenuComponent` | Received mentionable select. | P1 |
| `ChannelSelectMenuComponent` | Received channel select. | P1 |
| `TextInputComponent` | Received text input (modal). | P0 |
| `LabelComponent` | Modal label wrapper around one component. | P1 |
| `ContainerComponent` | Components V2 container. | P1 |
| `SectionComponent` | Components V2 section with accessory. | P1 |
| `SeparatorComponent` | Components V2 separator. | P1 |
| `TextDisplayComponent` | Components V2 text. | P1 |
| `ThumbnailComponent` | Components V2 thumbnail. | P1 |
| `MediaGalleryComponent` | Components V2 media gallery. | P1 |
| `MediaGalleryItem` | Item of a media gallery. | P1 |
| `FileComponent` | Components V2 file. | P1 |
| `UnfurledMediaItem` | Media reference used by V2 components. | P1 |

## 6. Managers

### 6.1 Base classes

- **`BaseManager`** (`managers/BaseManager.js`): holds a non-enumerable `client`.
- **`DataManager`**: holds `holds` (the class). `cache` getter throws `NotImplemented`. `resolve(x)`: an instance returns itself; a string does a cache lookup; otherwise null. `resolveId(x)`: an instance returns `.id`; a string returns itself **without checking that it exists**; otherwise null. `valueOf()` returns `cache`.
- **`CachedManager`**: `_cache = client.options.makeCache({holds, manager: this.constructor, managerType: this.constructor[MakeCacheOverrideSymbol] ?? this.constructor})`. `iterable` entries are `_add`ed.

`CachedManager._add(data, cache = true, {id, extras = []})`:

```
existing = cache.get(id ?? data.id)
if existing:
    if cache: existing._patch(data); return existing          # mutate in place
    clone = existing._clone(); clone._patch(data); return clone  # cache untouched
entry = holds ? new holds(client, data, ...extras) : data
if cache: cache.set(id ?? entry.id, entry)
return entry
```

Managers whose `_add` differs from this:

| Manager | Difference |
|---|---|
| `ChannelManager` | `_add(data, guild, {cache, allowUnknownGuild})`. Existing: patch only if `cache`, and **returns the original either way** (no clone), also registering it in `guild.channels` and `parent.threads`. New: `createChannel`, which may return null. `cache.set` only if `cache && !allowUnknownGuild`. |
| `GuildChannelManager`, `ThreadManager` | `_add(channelObject)`: stores an already-built structure, no patching. |
| `ThreadMemberManager` | Keyed by `user_id`. Existing: patched only if `cache`, returned as is (no clone). |
| `VoiceStateManager` | Keyed by `user_id`. Existing is **always** patched, even with `cache = false`. Constructor is `(guild, data)`. |
| `GuildMemberManager` | Key `data.user.id`, extras `[guild]`. |
| `GuildBanManager` | Key `data.user.id`. |
| `PresenceManager` | Key `data.user.id`. |
| `ReactionManager` | Key `emoji.id ?? emoji.name`. |
| `GuildInviteManager` | Key `code`. |
| `GuildSoundboardSoundManager` | Key `sound_id`; `resolveId` uses `soundId`. |

### 6.2 `fetch` patterns

Common shape: `fetch()` lists; `fetch(id or resolvable)` gets one; `fetch({…, cache, force})`. The cache check is skipped with `force`. `cache` defaults to true, by way of `_add(data, undefined)`.

| Manager | Single, unforced: returns cached if | Many | Notes |
|---|---|---|---|
| `ChannelManager` | exists and `!partial` | n/a | `allowUnknownGuild` path does not cache. |
| `GuildChannelManager` | exists | `GET /guilds/{id}/channels` | Throws `GuildChannelUnowned` for another guild. |
| `UserManager` | exists and `!partial` | n/a | `resolveId` handles `GuildMember`, `ThreadMember`, `Message`. |
| `GuildManager` | exists | `GET /users/@me/guilds` gives `OAuth2Guild`s (uncached) | `with_counts` defaults to true; injects a computed `shardId`. |
| `MessageManager` | exists and `!partial` | query passthrough (`limit`, `before`, `after`, `around`) | `fetchPins` returns `{items: [{pinnedTimestamp, pinnedAt, message}], hasMore}`. |
| `GuildMemberManager` | exists and `!partial`, only if no `limit` and no `withPresences` | **via gateway op 8** | §16 item 15. `search` and `list` use REST. |
| `GuildBanManager` | exists and `!partial` | paginated query | `fetch({})` without `before`, `after`, `limit` or `cache` throws `FetchBanResolveId`. |
| `RoleManager` | exists | `GET roles` | |
| `GuildEmojiManager`, `GuildStickerManager`, `ApplicationEmojiManager` | exists | list | Application list is `{items}`. |
| `GuildInviteManager` | exists | Guild list or channel list | Single fetch lists the **whole guild** and picks one; a string argument forces `cache: true`. |
| `GuildScheduledEventManager` | exists | list | `with_user_count` defaults to true. |
| `AutoModerationRuleManager`, `EntitlementManager`, `ApplicationCommandManager`, `GuildSoundboardSoundManager`, `ThreadMemberManager` | exists | list | `ThreadMember`: `with_member`, `after`, `limit`. Soundboard list is `{items}`. |
| `StageInstanceManager` | Found by `channelId` | n/a | |
| `VoiceStateManager` | exists | n/a | `'@me'` is accepted. |
| `ThreadManager` | delegated to `channels.fetch` | Active or archived | §16 item 19. |
| `SubscriptionManager` | never checks the cache | list | `sku` is required. |
| `ReactionUserManager`, `PollAnswerVoterManager` | n/a | Single page | Caches into `users` as well as the local cache. |

### 6.3 Create, edit and delete: what happens to the cache

| Operation | Effect |
|---|---|
| Channels: create | Via `ChannelCreate` (cached, event). |
| Channels: edit | Via `ChannelUpdate` (**patches the cached object**, no event). Multiple REST calls when `position` is set. |
| Channels: delete | Via `ChannelDelete` (removed, event). |
| Threads: create | Via `ThreadCreate`. |
| Threads: edit | Via `ChannelUpdate`. |
| Roles: create | Via `GuildRoleCreate`, then `setPosition` if `position` is set. |
| Roles: edit | **Returns a patched clone; cache not touched.** Gateway updates it. |
| Roles: delete | Via `GuildRoleDelete`. |
| Members: edit | Returns a clone, or `_add(data, false)` if uncached. |
| Members: `add` | Returns the cached member unless `force`. An empty ArrayBuffer means already a member: `fetch` unless `fetchWhenExisting === false` (then null). |
| Messages: send | `resolvedChannel?.messages._add(data) ?? new Message`; cached. |
| Messages: edit | Returns a clone if cached, else `_add`. |
| Messages: delete, pin, unpin | No cache change; the gateway handles it. `react` runs the action with no event. |
| Emojis: create / edit / delete | Action (cached, event) / clone / nothing. |
| Stickers: create / edit / delete | Action / clone / nothing. |
| Application emojis: create / edit | `_add` / **patches in place** (returns the original). |
| Soundboard: create / edit / delete | `_add` / clone / nothing. |
| Scheduled events: create / edit | `_add` (mutates the cache); delete does nothing. |
| Automod: create / edit | `_add`; delete does nothing. |
| Stage instances: create / edit / delete | `_add` / clone if cached / nothing. |
| Application commands: create / set / edit | `_add(…, true, guildId)`. `delete` removes and returns the cached one. `set` does **not** evict old entries. |
| Bans: create / remove | Nothing. |
| Invites: create | `new GuildInvite`, uncached. Delete does nothing. |
| Entitlements: `createTest` | `new Entitlement`, uncached. |
| Permission overwrites: upsert / delete | Nothing; returns the channel. Gateway `CHANNEL_UPDATE` updates. `set` calls `channel.edit`. |

### 6.4 Complete manager table

| Manager (`managers/`) | Purpose | Holds | Key | Pri |
|---|---|---|---|---|
| `BaseManager` | Holds `client`. | | | P0 |
| `DataManager` | `holds`, `resolve`, `resolveId`. | | | P0 |
| `CachedManager` | Cache plus `_add`. | | | P0 |
| `GuildManager` | All guilds; `fetch`, soundboard request, incident actions, widget URL. | `Guild` | id | P0 |
| `UserManager` | All users; `fetch`, DM open/delete, `send`. | `User` | id | P0 |
| `ChannelManager` | All channels; `fetch`, `createMessage`, `_remove`. | `BaseChannel` | id | P0 |
| `GuildChannelManager` | Guild channels and threads; CRUD, webhooks, positions, followers. | `GuildChannel` (structures may be threads) | id | P0 |
| `CategoryChannelChildManager` | Computed children of a category; `create`. | `GuildChannel` | id (filtered view) | P1 |
| `MessageManager` | Messages of a channel; fetch, pins, edit, delete, react, polls. | `Message` | id | P0 |
| `GuildMessageManager` | Adds `crosspost`. | `Message` | id | P0 |
| `DMMessageManager` | Empty DM subclass. | `Message` | id | P0 |
| `PartialGroupDMMessageManager` | Empty group DM subclass. | `Message` | id | P2 |
| `ThreadManager` | Threads of a channel; fetch active or archived. | `ThreadChannel` | id | P0 |
| `GuildTextThreadManager` | Create threads in text or announcement channels. | `ThreadChannel` | id | P0 |
| `GuildForumThreadManager` | Create forum posts. | `ThreadChannel` | id | P1 |
| `ThreadMemberManager` | Thread members; join, leave, fetch. | `ThreadMember` | user id | P1 |
| `GuildMemberManager` | Members; gateway chunk fetch, search, edit, prune, kick, ban. | `GuildMember` | user id | P0 |
| `GuildMemberRoleManager` | Computed roles of a member; add, remove, set. | `Role` | id (computed) | P0 |
| `GuildBanManager` | Bans; fetch, create, remove, bulk. | `GuildBan` | user id | P1 |
| `RoleManager` | Roles; CRUD, positions, member counts. | `Role` | id | P0 |
| `PermissionOverwriteManager` | Overwrites of a channel; set, upsert, delete. | `PermissionOverwrites` | target id | P0 |
| `PresenceManager` | Presences of a guild. | `Presence` | user id | P1 |
| `VoiceStateManager` | Voice states of a guild. | `VoiceState` | user id | P1 |
| `ReactionManager` | Reactions of a message; `removeAll`. | `MessageReaction` | emoji id or name | P1 |
| `ReactionUserManager` | Users of a reaction; fetch, remove. | `User` | id | P1 |
| `PollAnswerVoterManager` | Voters of a poll answer. | `User` | id | P1 |
| `GuildEmojiManager` | Guild emojis; CRUD, `fetchAuthor`, `resolveIdentifier`. | `GuildEmoji` | id | P1 |
| `GuildEmojiRoleManager` | Computed roles allowed on an emoji. | `Role` | id (computed) | P2 |
| `ApplicationEmojiManager` | Application emojis; CRUD. | `ApplicationEmoji` | id | P2 |
| `GuildStickerManager` | Guild stickers; CRUD. | `Sticker` | id | P2 |
| `GuildSoundboardSoundManager` | Guild soundboard sounds; CRUD. | `SoundboardSound` | sound id | P2 |
| `GuildInviteManager` | Guild invites; fetch, create, delete, target users. | `GuildInvite` | code | P1 |
| `GuildScheduledEventManager` | Scheduled events; CRUD, subscribers. | `GuildScheduledEvent` | id | P1 |
| `StageInstanceManager` | Stage instances. | `StageInstance` | id | P2 |
| `AutoModerationRuleManager` | Automod rules; CRUD. | `AutoModerationRule` | id | P1 |
| `ApplicationCommandManager` | Global (or by `guildId`) commands; CRUD, bulk `set`. | `ApplicationCommand` | id | P0 |
| `GuildApplicationCommandManager` | Guild commands; sets `guild`. | `ApplicationCommand` | id | P0 |
| `ApplicationCommandPermissionsManager` | Command permission overwrites (Bearer token). | n/a | | P2 |
| `EntitlementManager` | Entitlements; fetch, test create/delete, consume. | `Entitlement` | id | P2 |
| `SubscriptionManager` | Subscriptions by SKU. | `Subscription` | id | P2 |

`GuildMemberRoleManager`, `GuildEmojiRoleManager` and `CategoryChannelChildManager` are `DataManager`s whose `cache` is recomputed per read. They have no storage of their own, and `GuildMember.roles` and `CategoryChannel.children` build a new manager on each access.

`ApplicationCommandPermissionsManager` writes require an OAuth2 token (`Authorization: Bearer`, `auth: false`). `commandId ??= client.user.id` means "all commands" (the application id equals the bot user id here). `add` and `remove` are read-modify-write; `UnknownApplicationCommandPermissions` on the read counts as an empty list.

## 7. Cache limits (`util/Options.js`, `util/LimitedCollection.js`)

`cacheWithLimits(settings)` returns `({managerType, manager}) => Collection`:

1. `setting = settings[manager.name] ?? settings[managerType.name]`. Exact class wins; then the base class named by `MakeCacheOverrideSymbol`.
2. Null: `Collection`. Number: `Infinity` gives `Collection`, otherwise `LimitedCollection({maxSize})`.
3. Object: `maxSize` null or `Infinity` gives `Collection`, otherwise `LimitedCollection(setting)`.

`MakeCacheOverrideSymbol` is set on `MessageManager` and `ThreadManager` only. So `MessageManager: n` applies to `GuildMessageManager`, `DMMessageManager` and `PartialGroupDMMessageManager`, and `ThreadManager: n` applies to the text and forum subclasses.

`Options.cacheEverything()` returns plain `Collection`s.

`LimitedCollection.set(key, value)`:

```
if maxSize == 0 and not keepOverLimit(value, key, coll): return   # never stored
if size >= maxSize and key not present:
    for (k, v) in insertion order:
        if not keepOverLimit(v, k, coll): delete k; break     # evict exactly one
insert (k, v)    # may exceed maxSize if every entry is kept
```

Semantics: FIFO by first insertion, not LRU. Overwriting a key does not change its position (JS `Map`). `Symbol.species` returns `Collection`, so derived collections are unlimited. Construction validates `maxSize` is a number and `keepOverLimit` a function or null (`InvalidType`).

Overriding the cache is **unsupported** and warns once per process (`UnsupportedCacheOverwriteWarning`) for: `GuildManager`, `RoleManager`, `PermissionOverwriteManager` (anything but plain `Collection`), and `ChannelManager`, `GuildChannelManager` (allowed if `maxSize` is undefined or `Infinity`). The doc comment in `Options.js` gives the reason: these caches are load-bearing for functionality.

`Collection` itself is an insertion-ordered `Map` subclass, so `first()`, reductions over `cache` and `LimitedCollection` eviction all rely on order.

## 8. Sweepers (`util/Sweepers.js`, `util/Constants.js`)

`SweeperKeys`: `autoModerationRules, applicationCommands, bans, emojis, entitlements, invites, guildMembers, messages, presences, reactions, stageInstances, stickers, threadMembers, threads, users, voiceStates`.

Configuration `{[key]: {interval, lifetime?, filter?}}`:

- `interval` is in **seconds**, must be a number. `<= 0` or `Infinity` means no timer. Timers are `.unref()`ed.
- `filter` is a `GlobalSweepFilter`: a zero-arg function, **called on each tick**, returning either null (skip this tick) or the per-entry predicate. Any other return throws `SweepFilterReturn` inside the timer callback.
- `lifetime` is allowed only for `invites`, `messages`, `threads`, and is used only when `filter` is absent. For other keys `filter` is required.

Lifetime helpers:

| Helper | Timestamp | Excludes | Default lifetime |
|---|---|---|---|
| `filterByLifetime({lifetime, getComparisonTimestamp, excludeFromSweep})` | `createdTimestamp` | none | 14400 s |
| `archivedThreadSweepFilter` | `archiveTimestamp` | `!archived` | 14400 s |
| `expiredInviteSweepFilter` | `expiresTimestamp` | none | 14400 s |
| `outdatedMessageSweepFilter` | `editedTimestamp ?? createdTimestamp` | none | 3600 s |

Filter semantics: `lifetime <= 0` gives a null predicate. Missing or non-number timestamp: keep. Sweep when `now - ts > lifetime * 1000` (strict).

Sweep methods, each returning a count and emitting `cacheSweep`:

| Method | Scope |
|---|---|
| `sweepApplicationCommands` | All guilds' `commands` plus global (`application?.commands`). |
| `sweepAutoModerationRules`, `sweepBans`, `sweepEmojis`, `sweepInvites`, `sweepGuildMembers`, `sweepPresences`, `sweepStageInstances`, `sweepStickers`, `sweepVoiceStates` | `_sweepGuildDirectProp`: every `guild.available` guild. |
| `sweepEntitlements` | `client.application.entitlements` (no null check). |
| `sweepMessages` | Every text-based cached channel. |
| `sweepReactions` | Every message of every text-based channel. |
| `sweepThreadMembers` | Every thread. |
| `sweepThreads` | Client channel cache filtered by thread type; removed through `channels._remove`. |
| `sweepUsers` | `client.users`. |

`destroy()` clears all timers. The `Options.js` example for limited member caches keeps the client's own member through `keepOverLimit: member => member.id === client.user.id`. A port should make this easy, since `guild.members.me` and many permission getters depend on that entry.

## 9. Collectors (`structures/interfaces/Collector.js` and subclasses)

### 9.1 `Collector`

Options: `filter` (default always true; must be a function; may be async), `time`, `idle`, `dispose`.

- `collected: Collection`, `ended`, `lastCollectedTimestamp`, `endReason`.
- `handleCollect(...args)`:
  1. `id = await collect(...args)`. If truthy:
  2. `await filter(...args, collected)`. Pass: `collected.set(id, args[0])`, emit `collect`, set `lastCollectedTimestamp`, restart the idle timer. Fail: emit `ignore`.
  3. **Always** `checkEnd()`.
- `handleDispose(...args)`: no-op unless `options.dispose`. `id = dispose(...args)`; requires `await filter(...args)` (no `collected` argument here) and `collected.has(id)`; delete, emit `dispose`, `checkEnd`.
- `next`: a promise for the next `collect`; rejects with `collected` on `end`, or immediately if already ended.
- `stop(reason = 'user')`: idempotent; clears timers; sets `_endReason`, `ended`; emits `end(collected, reason)`.
- `resetTimer({time, idle})`: stores new values and restarts **only the timers already running**.
- `checkEnd`: if `endReason` is truthy, `stop(reason)`.
- `Symbol.asyncIterator`: queued iteration, ends after `end` once drained; yields argument arrays.
- Timers are `.unref()`ed.

### 9.2 Subclasses

| Class | Listens to | `collect` key | End reasons from `endReason` | Other end reasons |
|---|---|---|---|---|
| `MessageCollector` | `messageCreate`, `messageDelete`, `messageDeleteBulk`, channel, thread and guild delete | `message.id` if channel matches. `received++` before the filter. | `limit` (`max`, `collected.size >=`), `processedLimit` (`maxProcessed`, `received ===`) | `channelDelete` (also parent), `threadDelete`, `guildDelete` |
| `ReactionCollector` | `messageReactionAdd`, `messageReactionRemove`, `messageReactionRemoveAll`, message delete, bulk delete, channel, thread, guild delete | `emoji.id ?? emoji.name` | `limit` (`max`, `total >=`), `emojiLimit` (`maxEmojis`), `userLimit` (`maxUsers`) | `messageDelete`, `channelDelete`, `threadDelete`, `guildDelete` |
| `InteractionCollector` | `interactionCreate`; message and bulk delete only if `messageId`; channel and thread delete only if `channelId`; guild delete only if `guildId` | `interaction.id` after matching `interactionType`, `componentType`, `messageId`, `channelId`, `guildId` | `limit` (`max`, `total >=`), `componentLimit` (`maxComponents`), `userLimit` (`maxUsers`) | `messageDelete`, `channelDelete`, `threadDelete`, `guildDelete` |

`ReactionCollector` details:

- Internal `collect` listener: if `reaction.count === 1`, emit `create`. `total++`, add the user.
- `dispose(reaction, user)` has a side effect: emits `remove` if collected and the user is known. It returns the key only when `reaction.count` is 0, so the `dispose` event fires only for an emptied reaction.
- `remove` listener: `total--`; drop the user if they reacted to no other collected reaction.
- `empty()` on remove-all.
- It does not listen to `messageReactionRemoveEmoji`.

`InteractionCollector` details:

- `channelId` from `message.channelId ?? message.channel_id ?? channels.resolveId(channel)`; `guildId` similarly. `message` may be a raw object.
- `handleDispose` is never subscribed, so `dispose` never fires; the `empty` method is never called from this file.

Convenience wrappers, all creating a collector with one `end` handler:

- `channel.awaitMessages`, `message.awaitReactions`: resolve with the collection; reject with the collection if `options.errors` includes the reason.
- `channel.awaitMessageComponent`, `message.awaitMessageComponent`: `max: 1`; reject with `InteractionCollectorError(reason)`.
- `interaction.awaitModalSubmit`: `max: 1`, `interactionType: ModalSubmit`; `time` must be a number, otherwise `InvalidType`.

Collectors depend on cached state only via the event stream. Without `Partials`, events for uncached messages and channels do not reach them.

## 10. Sharding (`sharding/`)

There are two independent mechanisms:

- **Internal:** a single `Client` with `ws.shardIds: null`, `shardCount: null`. `@discordjs/ws` runs every shard in one process. `client.shard` is null. This is the default.
- **`ShardingManager`:** one process or worker per shard id, each with its own `Client`. Caches are per process.

### 10.1 `ShardingManager`

Options: `totalShards: 'auto'`, `shardList: 'auto'`, `mode: 'process' | 'worker'`, `respawn: true`, `silent: false`, `shardArgs: []`, `execArgv: []`, `token: env`, `api`, `version`. The constructor validates (`ClientInvalidOption`), requires an existing file, and sets the env variables `SHARDING_MANAGER`, `SHARDING_MANAGER_MODE`, `DISCORD_TOKEN`.

- `spawn({amount = totalShards, delay = 5500, timeout = 30000})`:
  - `'auto'` calls `fetchRecommendedShardCount`.
  - `shards.size >= amount` gives `ShardingAlreadySpawned`.
  - If `shardList` is `'auto'`, `totalShards` is `'auto'`, or `totalShards !== amount`: `shardList = 0..amount`.
  - A shard id `>= amount` gives `ClientInvalidOption`.
  - Shards spawn **sequentially**; each step waits for `max(shard ready, delay)`. No delay after the last shard.
- `createShard(id)`, `broadcast(message)`.
- `broadcastEval(fn, {shard, context})`: `(${fn})(this, ${JSON.stringify(context)})`.
- `fetchClientValues(prop, shard)`, `respawnAll({shardDelay = 5000, respawnDelay = 500, timeout = 30000})`.
- `_performOnShards`: no shards gives `ShardingNoShards`; a numeric unknown shard gives `ShardingShardNotFound`; `shards.size !== shardList.length` gives `ShardingInProcess`.
- Event: `shardCreate`.

`fetchRecommendedShardCount(token, {guildsPerShard = 1000, multipleOf = 1, api, version})`: `GET /gateway/bot`; 401 gives `TokenInvalid`; other non-OK throws the response. Result: `ceil(shards * (1000 / guildsPerShard) / multipleOf) * multipleOf`.

### 10.2 `Shard` and `ShardClientUtil`

`Shard` env: `{...process.env, SHARDING_MANAGER: true, SHARDS: id, SHARD_COUNT: total, DISCORD_TOKEN}`. Process mode uses `fork`; worker mode `new Worker(file, {workerData: env, env: SHARE_ENV, argv})`.

- `spawn(timeout = 30000)`: existing child gives `ShardingProcessExists` / `ShardingWorkerExists`. `timeout` of `-1` or `Infinity` returns immediately. Otherwise waits for `ready`; rejects on `disconnect` (`ShardingReadyDisconnected`), `death` (`ShardingReadyDied`), timeout (`ShardingReadyTimeout`).
- `kill()`: removes the exit listener, kills, `_handleExit(false)`.
- `respawn({delay = 500, timeout = 30000})`.
- `send`, `fetchClientValue(prop)`, `eval(script, context)`: identical concurrent requests share one promise (`_fetches`, `_evals`). A dead shard gives `ShardingNoChildExists`.
- `_handleExit(respawn = manager.respawn, timeout)`: emits `death`, clears state, respawns when enabled; respawn failure emits `error`.
- `ShardEvents`: `death, disconnect, error, message, ready, resume, spawn`.

`ShardClientUtil`: `singleton(client, mode)` (a second client warns). `send`, `fetchClientValues`, `broadcastEval`, `respawnAll`, static `shardIdForGuildId(guildId, shardCount)`: `Number(BigInt(guildId) >> 22n) % shardCount`; negative gives `ShardingShardMiscalculation`.

### 10.3 IPC protocol

Messages are plain JSON objects over the child's IPC channel.

| Direction | Keys | Meaning |
|---|---|---|
| child to parent | `_ready: true` | On `clientReady`. |
| child to parent | `_disconnect: true` | On **any** ws shard `Closed`. |
| child to parent | `_resume: true` | On **any** ws shard `Resumed`. |
| child to parent | `_sFetchProp`, `_sFetchPropShard` | Ask the manager for `fetchClientValues`. |
| child to parent | `_sEval`, `_sEvalShard` | Ask the manager for an eval broadcast. |
| child to parent | `_sRespawnAll: {shardDelay, respawnDelay, timeout}` | Respawn request. Errors swallowed. |
| parent to child | `_fetchProp` | Walk a `.`-separated path from the client. |
| parent to child | `_eval` | `client._eval(script)`. |
| both | `_result` or `_error: {name, message, stack}` | Response, echoing the request keys. Errors via `makePlainError` and `makeError`. |
| both | other | Delivered as a `message` event on `Shard`. |

Replies are correlated **by the script or property string and shard id**, not by a request id.

`broadcastEval` sends function source to be evaluated in the child. **Rust cannot do this.** A port needs a typed replacement, for example named handlers registered at startup with serde-encoded arguments and results plus a real correlation id. `fetchClientValues` (dynamic property path) has the same problem.

## 11. Bitfields

### 11.1 `util/BitField.js`

`Flags` (static), `DefaultBit` (`0`, or `0n` for `PermissionsBitField`).

| Member | Behaviour |
|---|---|
| `any(bit)` | `(self & resolve(bit)) != 0`. |
| `equals(bit)` | Exact. |
| `has(bit)` | `(self & b) == b`. So `has(0)` is true. |
| `missing(bits, ...)` | `new(bits).remove(self).toArray(...)`. |
| `freeze()` | Immutable; `add` and `remove` on a frozen instance return **a new instance**. |
| `add(...bits)`, `remove(...bits)` | Mutate unless frozen. |
| `serialize(...)` | `{flagName: bool}` for all positive named flags. |
| `toArray(...)`, iterator | Names of set flags. Filter `isNaN(name) && bit > 0`. Aliases appear under every name. |
| `toJSON()` | Number, or **string** for BigInt. |
| `resolve(bit)` static | Same-typed non-negative value passes through; `BitField` instance; array (OR of each); numeric string (`isNaN` check); flag name. Otherwise `BitFieldInvalid`. |

Most structures store `.freeze()`d fields.

### 11.2 `PermissionsBitField`

BigInt-backed. Statics: `All` (OR of all known flags), `Default = 104_324_673n`, `StageModerator = ManageChannels | MuteMembers | MoveMembers`.

`has(p, checkAdmin = true)` and `any(p, checkAdmin = true)` return true if `Administrator` is set and `checkAdmin`. `missing(bits, checkAdmin = true)` returns `[]` for admins. `toArray()` uses `has(bit, false)`, so admin does not imply every name.

Flags (`discord-api-types/payloads/common.ts`), bit numbers:

```
CreateInstantInvite 0, KickMembers 1, BanMembers 2, Administrator 3, ManageChannels 4, ManageGuild 5, AddReactions 6,
ViewAuditLog 7, PrioritySpeaker 8, Stream 9, ViewChannel 10, SendMessages 11, SendTTSMessages 12, ManageMessages 13,
EmbedLinks 14, AttachFiles 15, ReadMessageHistory 16, MentionEveryone 17, UseExternalEmojis 18, ViewGuildInsights 19,
Connect 20, Speak 21, MuteMembers 22, DeafenMembers 23, MoveMembers 24, UseVAD 25, ChangeNickname 26, ManageNicknames 27,
ManageRoles 28, ManageWebhooks 29, ManageGuildExpressions 30 (alias ManageEmojisAndStickers), UseApplicationCommands 31,
RequestToSpeak 32, ManageEvents 33, ManageThreads 34, CreatePublicThreads 35, CreatePrivateThreads 36, UseExternalStickers 37,
SendMessagesInThreads 38, UseEmbeddedActivities 39, ModerateMembers 40, ViewCreatorMonetizationAnalytics 41, UseSoundboard 42,
CreateGuildExpressions 43, CreateEvents 44, UseExternalSounds 45, SendVoiceMessages 46, SetVoiceChannelStatus 48, SendPolls 49,
UseExternalApps 50, PinMessages 51, BypassSlowmode 52
```

Permission computation (`GuildChannel.memberPermissions`):

1. With `checkAdmin`: owner gives `All`.
2. Base is the OR of all role permissions (including `@everyone`, via `GuildMemberRoleManager.cache`).
3. With `checkAdmin`: `Administrator` in base gives `All`.
4. Apply: remove everyone-deny, add everyone-allow, remove role-denies (union), add role-allows (union), remove member-deny, add member-allow.

Not modelled by this code: timeouts and implicit permission rules. Timeouts are checked ad hoc in getters such as `manageable`, `joinable`, `sendable`, `deletable`.

`rolePermissions`: admin gives `All`; base = role plus `@everyone`; then the everyone overwrite, then this role's overwrite.

`GuildMember.permissions`: owner gives `All`; else the OR of roles, with no channel overwrites.

`ThreadChannel.permissionsFor` delegates to the parent, and is null with no cached parent.

### 11.3 Other flag sets

Number-backed unless noted, one-line subclasses in `util/*BitField.js`. Bits below are from `discord-api-types`.

| Class | Enum | Bits |
|---|---|---|
| `IntentsBitField` | `GatewayIntentBits` | Guilds 0, GuildMembers 1, GuildModeration 2 (alias GuildBans), GuildExpressions 3 (alias GuildEmojisAndStickers), GuildIntegrations 4, GuildWebhooks 5, GuildInvites 6, GuildVoiceStates 7, GuildPresences 8, GuildMessages 9, GuildMessageReactions 10, GuildMessageTyping 11, DirectMessages 12, DirectMessageReactions 13, DirectMessageTyping 14, MessageContent 15, GuildScheduledEvents 16, AutoModerationConfiguration 20, AutoModerationExecution 21, GuildMessagePolls 24, DirectMessagePolls 25 |
| `MessageFlagsBitField` | `MessageFlags` | Crossposted 0, IsCrosspost 1, SuppressEmbeds 2, SourceMessageDeleted 3, Urgent 4, HasThread 5, Ephemeral 6, Loading 7, FailedToMentionSomeRolesInThread 8, ShouldShowLinkNotDiscordWarning 10, SuppressNotifications 12, IsVoiceMessage 13, HasSnapshot 14, IsComponentsV2 15 |
| `ChannelFlagsBitField` | `ChannelFlags` | GuildFeedRemoved 0, Pinned 1, ActiveChannelsRemoved 2, RequireTag 4, IsSpam 5, IsGuildResourceChannel 7, ClydeAI 8, IsScheduledForDeletion 9, HideMediaDownloadOptions 15, ChannelObfuscated 17, IsSpoilerChannel 21 |
| `ThreadMemberFlagsBitField` | `ThreadMemberFlags` | HasInteracted 0, AllMessages 1, OnlyMentions 2, NoMessages 3 |
| `ApplicationFlagsBitField` | `ApplicationFlags` | EmbeddedReleased 1, ManagedEmoji 2, EmbeddedIAP 3, GroupDMCreate 4, ApplicationAutoModerationRuleCreateBadge 6, RPCHasConnected 11, GatewayPresence 12, GatewayPresenceLimited 13, GatewayGuildMembers 14, GatewayGuildMembersLimited 15, VerificationPendingGuildLimit 16, Embedded 17, GatewayMessageContent 18, GatewayMessageContentLimited 19, EmbeddedFirstParty 20, ApplicationCommandBadge 23 |
| `UserFlagsBitField` | `UserFlags` | Staff 0, Partner 1, Hypesquad 2, BugHunterLevel1 3, MFASMS 4, PremiumPromoDismissed 5, HypeSquadOnlineHouse1 6, HypeSquadOnlineHouse2 7, HypeSquadOnlineHouse3 8, PremiumEarlySupporter 9, TeamPseudoUser 10, HasUnreadUrgentMessages 13, BugHunterLevel2 14, VerifiedBot 16, VerifiedDeveloper 17, CertifiedModerator 18, BotHTTPInteractions 19, Spammer 20, DisablePremium 21, ActiveDeveloper 22, Quarantined 2^44, Collaborator 2^50, RestrictedCollaborator 2^51 |
| `SystemChannelFlagsBitField` | `GuildSystemChannelFlags` | SuppressJoinNotifications 0, SuppressPremiumSubscriptions 1, SuppressGuildReminderNotifications 2, SuppressJoinNotificationReplies 3, SuppressRoleSubscriptionPurchaseNotifications 4, SuppressRoleSubscriptionPurchaseNotificationReplies 5 |
| `GuildMemberFlagsBitField` | `GuildMemberFlags` | DidRejoin 0, CompletedOnboarding 1, BypassesVerification 2, StartedOnboarding 3, IsGuest 4, StartedHomeActions 5, CompletedHomeActions 6, AutomodQuarantinedUsernameOrGuildNickname 7, AutomodQuarantinedBio 8, DmSettingsUpsellAcknowledged 9, AutoModQuarantinedGuildTag 10 |
| `InviteFlagsBitField` | `InviteFlags` | IsGuestInvite 0 |
| `AttachmentFlagsBitField` | `AttachmentFlags` | IsClip 0, IsThumbnail 1, IsRemix 2, IsSpoiler 3, ContainsExplicitContent 4, IsAnimated 5, ContainsGoreContent 6, ContainsSelfHarmContent 7 |
| `SKUFlagsBitField` | `SKUFlags` | Available 2, GuildSubscription 7, UserSubscription 8 |
| `RoleFlagsBitField` | `RoleFlags` | InPrompt 0 |
| `ActivityFlagsBitField` | `ActivityFlags` | Instance 0, Join 1, Spectate 2, JoinRequest 3, Sync 4, Play 5, PartyPrivacyFriends 6, PartyPrivacyVoiceChannel 7, Embedded 8 |

Note the gaps (for example permission bit 47). A Rust bitflags type must retain unknown bits on receive (`from_bits_retain`), since `new BitField(n)` keeps whatever the API sends.

## 12. Interactions

### 12.1 Dispatch (`client/actions/InteractionCreate.js`)

Channel resolving first: `data.channel && getChannel({...data.channel, ...('recipients' in data.channel ? {user: data.user} : undefined)})`.

| `type` | Sub-discriminant | Class | Dropped when |
|---|---|---|---|
| ApplicationCommand | `data.type` ChatInput | `ChatInputCommandInteraction` | |
| | User | `UserContextMenuCommandInteraction` | |
| | Message | `MessageContextMenuCommandInteraction` | Channel cached and not text-based |
| | PrimaryEntryPoint | `PrimaryEntryPointCommandInteraction` | |
| | other | none | Debug log |
| MessageComponent | `component_type` Button | `ButtonInteraction` | Channel cached and not text-based (all components) |
| | StringSelect, UserSelect, RoleSelect, MentionableSelect, ChannelSelect | the matching `*SelectMenuInteraction` | |
| | other | none | Debug log |
| ApplicationCommandAutocomplete | | `AutocompleteInteraction` | |
| ModalSubmit | | `ModalSubmitInteraction` | |
| other (including Ping) | | none | Debug log |

### 12.2 `BaseInteraction`

Fields: `type`, `id`, hidden `token`, `applicationId`, `channelId` (from `data.channel?.id`, **not** `channel_id`), `guildId`, `user` (`users._add(user ?? member.user)`), `member` (`guild?.members._add(member) ?? rawMember`, or null), `version`, `appPermissions`, `memberPermissions`, `locale`, `guildLocale`, `entitlements`, `authorizingIntegrationOwners`, `context`, `attachmentSizeLimit`.

Guards: `inGuild` (`guildId && member`), `inCachedGuild`, `inRawGuild`, `isAutocomplete`, `isCommand`, `isChatInputCommand`, `isContextMenuCommand`, `isPrimaryEntryPointCommand`, `isMessageComponent`, `isModalSubmit`, `isUserContextMenuCommand`, `isMessageContextMenuCommand`, `isButton`, `isSelectMenu`, `isStringSelectMenu`, `isUserSelectMenu`, `isRoleSelectMenu`, `isChannelSelectMenu`, `isMentionableSelectMenu`, `isRepliable` (not Ping, not Autocomplete).

`member` is either a `GuildMember` or the **raw API object**, depending on whether the guild is cached. Resolved members, roles and channels behave the same way.

### 12.3 Responses (`structures/interfaces/InteractionResponses.js`)

All callbacks are `POST /interactions/{id}/{token}/callback?with_response=<bool>`, `auth: false`.

| Method | Callback type | Precondition | State after success |
|---|---|---|---|
| `deferReply({flags, withResponse})` | `DeferredChannelMessageWithSource` | not deferred, not replied | `deferred = true`, `ephemeral` from flags |
| `reply(options)` | `ChannelMessageWithSource` | not deferred, not replied | `replied = true`, `ephemeral` from flags |
| `deferUpdate({withResponse})` | `DeferredMessageUpdate` | same | `deferred = true` |
| `update(options)` | `UpdateMessage` | same | `replied = true` |
| `launchActivity({withResponse})` | `LaunchActivity` | same | `replied = true` |
| `showModal(modal, {withResponse})` | `Modal` | same | `replied = true` |
| `AutocompleteInteraction.respond(choices)` | `ApplicationCommandAutocompleteResult` | `!responded` | `responded = true` |

Precondition failure throws `InteractionAlreadyReplied`.

Via `InteractionWebhook` (`new InteractionWebhook(client, applicationId, token)`):

| Method | Route | Precondition | State |
|---|---|---|---|
| `fetchReply(message = '@original')` | `GET webhook message` | none | |
| `editReply(options)` | `PATCH webhook message`, `options.message ?? '@original'` | deferred or replied | `replied = true` |
| `deleteReply(message = '@original')` | `DELETE` | deferred or replied | |
| `followUp(options)` | `POST webhook?wait=true` | deferred or replied | `replied = true` |

Failure throws `InteractionNotReplied`.

Availability:

- `CommandInteraction`: all except `deferUpdate`, `update`.
- `MessageComponentInteraction`: all.
- `ModalSubmitInteraction`: all except `showModal`, including `awaitModalSubmit`.
- `AutocompleteInteraction`: only `respond`.

`withResponse` returns `InteractionCallbackResponse {interaction: InteractionCallback, resource: ?InteractionCallbackResource}`. `resource.message` is `channel.messages._add` or a new `Message`; `resource.activityInstance` is raw.

### 12.4 `CommandInteractionOptionResolver`

The constructor hoists a leading `SubcommandGroup` then `Subcommand` into `_group`, `_subcommand`, and `_hoistedOptions`. `data` keeps the full frozen tree; `resolved` is the transformed resolved map.

| Getter | Accepted option types | Required check property |
|---|---|---|
| `get(name, required)` | any | `CommandInteractionOptionNotFound` |
| `getSubcommand(required = true)`, `getSubcommandGroup(required = false)` | | `...NoSubcommand`, `...NoSubcommandGroup` |
| `getBoolean`, `getString`, `getInteger`, `getNumber` | exact type | `value` |
| `getChannel(name, required, channelTypes = [])` | Channel | `channel`; type mismatch gives `CommandInteractionOptionInvalidChannelType` |
| `getUser` | User, Mentionable | `user` |
| `getMember(name)` | User, Mentionable | never required; note no `required` param |
| `getRole` | Role, Mentionable | `role` |
| `getAttachment` | Attachment | `attachment` |
| `getMentionable` | Mentionable | any of `user`, `member`, `role`; returns `member ?? user ?? role` |
| `getMessage` | pseudo-type `'_MESSAGE'` | `message` |
| `getFocused()` | | Full focused option, or `AutocompleteInteractionOptionNoFocusedOption` |

Wrong type gives `CommandInteractionOptionType`; a required-but-empty value gives `CommandInteractionOptionEmpty`.

`CommandInteraction.transformOption` looks resolved data up **by `option.value`**: `resolved.users[value]`, `.members[value]` (merged as `{user, ...member}`), `.channels[value]`, `.roles[value]`, `.attachments[value]`. It adds them to caches.

`ContextMenuCommandInteraction.resolveContextMenuOptions`: synthesizes option `user` (`type: User`) and/or `message` (`type: '_MESSAGE'`) from `target_id`. `targetId` is stored.

`AutocompleteInteraction` passes raw `data.data.options` (no transform, no resolved).

`util/Util.js transformResolved({client, guild, channel}, resolved)` builds `{members, users, roles, channels, messages, attachments}` Collections, also caching. `members` entries assume `users[id]` exists.

### 12.5 Modals

`ModalSubmitInteraction`: `customId`, `message` (when from a component), `components: ModalComponentResolver`, `deferred`, `replied`, `ephemeral`, `webhook`, `isFromMessage()`.

`transformComponent` recurses through `components` (action rows) and `component` (labels). Leaves get `type`, `id`, `customId` if present, `value` if present, `values` if truthy, and, when `resolved` is present, per-kind collections filtered to the submitted `values`.

`ModalComponentResolver`: `hoistedComponents` from both `components` and `component`, keyed by `customId`. Top-level text displays are ignored.

Getters: `getComponent` (throws `ModalSubmitInteractionComponentNotFound`), `getTextInputValue`, `getStringSelectValues`, `getSelectedUsers`, `getSelectedRoles`, `getSelectedChannels(customId, required, channelTypes)`, `getSelectedMembers`, `getSelectedMentionables` (returns `{users, members, roles}` or null), `getUploadedFiles`, `getRadioGroup`, `getCheckboxGroup`, `getCheckbox`.

Errors: `...ComponentType`, `...ComponentEmpty`, `...ComponentInvalidChannelType`.

Accepted types: `TextInput`, `StringSelect`, `UserSelect` and `MentionableSelect` for users and members, `RoleSelect` and `MentionableSelect` for roles, `ChannelSelect`, `FileUpload`, `RadioGroup`, `CheckboxGroup`, `Checkbox`.

### 12.6 Components (`util/Components.js`)

`createComponent(data)`: already a `Component` passes through; else lookup by type, **falling back to the `Component` base class** for unknown types. Unknown component types must survive round-trips in Rust.

`findComponentByCustomId` flattens action rows, sections (including the accessory) and containers, recursively. Other component types are checked directly.

## 13. Errors (`errors/`)

`DJSError.js`: `DiscordjsError`, `DiscordjsTypeError`, `DiscordjsRangeError`. Each is built by `makeDiscordjsError(Base)`, with `code` and a name `<Class> [<code>]`. A message comes from `Messages[code]` (string or function of args).

REST errors (`DiscordAPIError`, `HTTPError`, `RateLimitError`) come from `@discordjs/rest` and pass straight through. `GatewayRateLimitError` comes from `@discordjs/util`: `{data: {opcode, retry_after, meta}, payload}`.

Codes (`errors/ErrorCodes.js`), grouped. All have a message in `errors/Messages.js`.

| Group | Codes |
|---|---|
| Client | `ClientInvalidOption`, `ClientInvalidProvidedShards` (**no use site in `src/`**), `ClientMissingIntents`, `ClientNotReady`, `TokenInvalid`, `TokenMissing`, `ApplicationCommandPermissionsTokenMissing`, `BitFieldInvalid` |
| Sharding | `ShardingNoShards`, `ShardingInProcess`, `ShardingInvalidEvalBroadcast`, `ShardingShardNotFound`, `ShardingAlreadySpawned`, `ShardingProcessExists`, `ShardingWorkerExists`, `ShardingReadyTimeout`, `ShardingReadyDisconnected`, `ShardingReadyDied`, `ShardingNoChildExists`, `ShardingShardMiscalculation` |
| Values | `ColorRange`, `ColorConvert`, `InviteOptionsMissingChannel`, `InteractionCollectorError`, `FileNotFound`, `ReqResourceType`, `InvalidType`, `InvalidElement`, `NotImplemented`, `SweepFilterReturn` |
| Users, voice | `UserNoDMChannel`, `VoiceNotStageChannel`, `VoiceStateNotOwn`, `VoiceStateInvalidType` |
| Messages | `MessageBulkDeleteType`, `MessageContentType`, `MessageNonceRequired`, `MessageNonceType`, `MessageThreadParent`, `MessageExistingThread`, `MessageReferenceMissing` |
| Guild | `BanResolveId`, `FetchBanResolveId`, `BulkBanUsersOptionEmpty`, `PruneDaysType`, `GuildChannelResolve`, `GuildVoiceChannelResolve`, `GuildChannelOrphan`, `GuildChannelUnowned`, `GuildMembersTimeout`, `GuildSoundboardSoundsTimeout`, `GuildUncachedMe`, `GuildUncachedEntityResolve`, `GuildScheduledEventResolve`, `GuildForumMessageRequired`, `ChannelNotCached`, `StageChannelResolve`, `FetchOwnerId`, `MemberFetchNonceLength` |
| Threads | `ThreadInvitableType`, `NotAThreadOfParent` |
| Webhooks | `WebhookMessage`, `WebhookTokenUnavailable`, `WebhookApplication` |
| Emoji, sticker, sound | `EmojiType`, `EmojiManaged`, `MissingGuildExpressionsPermission`, `NotGuildSoundboardSound`, `NotGuildSticker` |
| Reactions, invites | `ReactionResolveUser`, `InviteResolveCode`, `InviteNotFound` |
| Group DM | `DeleteGroupDMChannel`, `FetchGroupDMChannel` |
| Commands | `GlobalCommandPermissions` |
| Interactions | `InteractionAlreadyReplied`, `InteractionNotReplied`, `CommandInteractionOptionNotFound`, `CommandInteractionOptionType`, `CommandInteractionOptionEmpty`, `CommandInteractionOptionNoSubcommand`, `CommandInteractionOptionNoSubcommandGroup`, `CommandInteractionOptionInvalidChannelType`, `AutocompleteInteractionOptionNoFocusedOption`, `ModalSubmitInteractionComponentNotFound`, `ModalSubmitInteractionComponentType`, `ModalSubmitInteractionComponentEmpty`, `ModalSubmitInteractionComponentInvalidChannelType` |
| Invite URLs | `InvalidMissingScopes`, `InvalidScopesWithPermissions` |
| Other | `EntitlementCreateInvalidOwner`, `PollAlreadyExpired`, `PermissionOverwritesTypeMandatory`, `PermissionOverwritesTypeMismatch` |

Many `InvalidType` and `InvalidElement` sites exist only because JS has dynamic argument types. Rust's type system removes most of them. Runtime states that remain include: not ready, not replied or already replied, missing webhook token, uncached guild or channel, option missing or wrong type, timeouts, sharding state.

## 14. Webhooks (`structures/Webhook.js`, `InteractionWebhook.js`)

`Webhook` fields: `name`, hidden `token`, `avatar`, `id`, `type`, `guildId`, `channelId`, `owner` (cached user), `applicationId`, `sourceGuild` and `sourceChannel` (cached entity or **raw object**).

| Method | Route / behaviour |
|---|---|
| `send(options)` | `POST /webhooks/{id}/{token}?wait=true&thread_id&with_components`, `auth: false`. Requires a token. Returns `messages._add(data, false)` or `new Message`. |
| `sendSlackMessage(body)` | `POST .../slack?wait=true`, `auth: false`; true if the response string is `ok`. |
| `edit({name = this.name, avatar, channel, reason})` | `PATCH /webhooks/{id}[/{token}]`. **Token is dropped from the path when moving channel** (`channel ? undefined : token`), and `auth: !token \|\| Boolean(channel)`. `avatar` as a `data:` string is kept; others go through `resolveImage`. Updates `name`, `avatar`, `channelId` in place. |
| `fetchMessage(message, {threadId})` | `GET .../messages/{id}`; `message` is a string here. Same return rule as `send`. |
| `editMessage(message, options)` | `PATCH .../messages/{id}?thread_id&with_components`. Message may be a string or object. Returns a clone if cached, else `_add`, else `new Message`. |
| `deleteMessage(message, threadId)` | `DELETE .../messages/{id}?thread_id`. |
| `delete(reason)` | `client.deleteWebhook(id, {token, reason})`. |
| `avatarURL`, `url`, `createdTimestamp`, `createdAt`, `channel` | |
| `isUserCreated`, `isApplicationCreated`, `isChannelFollower`, `isIncoming` | Type checks. |

`InteractionWebhook`: `{id, token}`. It receives `send`, `fetchMessage`, `editMessage`, `deleteMessage` and `url` from `Webhook` via `applyToClass`, but **is not `instanceof Webhook`**. Thus `MessagePayload.isWebhook` is false for it, and no `username`, `avatar_url`, `thread_name` or `applied_tags` are sent.

`parseWebhookURL(url)` (`util/Util.js`): regex over `discord.com`, `ptb.` and `canary.`, optional `/api/vN`, id `\d{17,19}`, token `[\w-]{68}`.

## 15. The send path (`structures/MessagePayload.js`, `util/DataResolver.js`)

`MessagePayload.create(target, options, extra)`: a non-object option becomes `{content: options, ...extra}`.

Targets: `TextBasedChannel`, `User`, `GuildMember`, `Webhook`, `WebhookClient`-like, `Message`, `MessageManager`, `BaseInteraction`, `InteractionWebhook`.

`resolveBody()` is memoized and produces:

| Field | Rule |
|---|---|
| `content` | `null` gives `''`; undefined is omitted; otherwise must be a string (`MessageContentType`). |
| `tts` | `Boolean(options.tts)`; **always sent**. |
| `nonce`, `enforce_nonce` | Integer or string, otherwise `MessageNonceType`. No nonce: if `enforceNonce !== false` and the client's `enforceNonce` is on, generates a snowflake and sets `enforce_nonce: true`; else `options.enforceNonce: true` throws `MessageNonceRequired`. `enforce_nonce` is **always sent** (false by default). |
| `components`, `embeds` | `toJSON()` if encodable, else `jsonTransformer`. |
| `username`, `avatar_url`, `thread_name`, `applied_tags` | Only `isWebhook`. `username` defaults to the webhook's `name`. |
| `flags` | `MessageFlagsBitField` resolved, unless null. |
| `allowed_mentions` | Option, or client default. `repliedUser` renamed `replied_user`. **Omitted when editing someone else's message** (`isMessage && no message_reference && author != client`). |
| `message_reference` | Only with `messageId`: `{message_id, channel_id, guild_id, type, fail_if_not_exists}`, last default from the client option. |
| `attachments` | From `files`: `{id: index, description, title, waveform, duration_secs}`. Then options `attachments` appended (edit only). `[]` is meaningful. |
| `sticker_ids` | `sticker.id ?? sticker`. |
| `poll` | `{question: {text}, answers: [{poll_media: {text, emoji}}], duration, allow_multiselect, layout_type}`. |
| `shared_client_theme` | `{colors, gradient_angle, base_mix, base_theme}`. |

`resolveFiles()` is memoized.

- String, `Buffer` or stream directly: `findName` (`basename` without query) or `'file.jpg'`.
- `RawFile`-encodable: `getRawFile()`.
- `{attachment, name}`.

Then `resolveFile` gives `{data, contentType?}`.

`util/DataResolver.js`:

- `resolveFile(resource)`: `Buffer`; async iterable (concatenated); `http(s)://` string (`fetch`, **no status check**); any other string is a **local path** (must be a file, otherwise `FileNotFound`). Else `ReqResourceType`.
- `resolveBase64(data, contentType = 'image/jpg')`: `data:` URL.
- `resolveImage(image)`: falsy gives null; `data:` string passes through; otherwise always labelled `image/jpg`.
- `resolveCode`, `resolveInviteCode`, `resolveGuildTemplateCode`: regex-capture or the original string.
- `resolveInviteTargetUsersFile`: array of users to `\n`-joined ids, or file content.

Message creation (`ChannelManager.createMessage`) accepts `MessagePayload`, a file-body encodable, a JSON encodable, or raw options. `User.send` and `GuildMember.send` open a DM first.

Other helpers in `util/Util.js`:

- `flatten` (`toJSON` engine)
- `parseEmoji`, `resolvePartialEmoji`, `resolveGuildEmoji`
- `moveElementInArray`, `setPosition`, `discordSort`, `getSortableGroupTypes`
- `verifyString`
- `resolveColor`: names from `Colors`, `'Random'`, `'Default'`, `rrggbb` with an optional `#`, `[r, g, b]`, integer in range
- `cleanContent`, `cleanCodeBlockContent` (inserts U+200B between backticks)
- `makeError`, `makePlainError`

`util/Transformers.js`: `toSnakeCase`; `_transformAPIAutoModerationAction`, `_transformAPIMessageInteractionMetadata` (recursive), `_transformGuildScheduledEventRecurrenceRule`, `_transformAPIIncidentsData`, `_transformCollectibles`, `_transformAPIInviteTargetUsersJobStatus`.

## 16. Subtle behaviour a port must preserve or consciously change

Items are marked **[keep]** (protocol or contract), **[decide]** (a design choice is needed) or **[fix]** (an upstream defect not to reproduce).

### Gateway and readiness

1. **[fix]** `client.login` resolves before readiness. Users must wait for `clientReady`. Keep the separation, and offer a `wait_until_ready`.
2. **[keep]** `clientReady` can fire more than once. `READY` sets `WaitingForGuilds` on every re-identify, and `_triggerClientReady` runs again. `readyTimestamp` is documented as overwritten on reconnect (`Client.js:284`). `RESUMED` does not touch `status`.
3. **[decide]** During that window **all** shards' non-whitelisted events are queued in one unbounded `incomingPacketQueue`. A single shard re-identifying delays the whole client. A per-shard readiness model would be a behaviour change; state it if adopted.
4. **[fix]** Queue ordering. `_handlePacket` shifts only one item per incoming packet and runs it on `setImmediate`, after the current packet's handler. So a new packet is handled **before** older queued ones. Pre-ready, a whitelisted packet also shifts an item that is immediately re-pushed to the tail, rotating the queue. Queued packets also wait for a further packet to arrive after ready. A Rust port should drain FIFO at the ready transition.
5. **[keep]** Ready timeout: with `Guilds` intent, `waitGuildTimeout` is restarted by each guild packet; without it the timer is 0 ms. `expectedGuilds` may still be non-empty at ready, and guilds can stay `available = false` afterwards.
6. **[keep]** `guildCreate` is emitted only when `status === Ready`; guilds arriving during startup emit nothing for new entries, and `guildAvailable` for known ones. Known, available guild plus `GUILD_CREATE`: the whole packet is dropped.
7. **[keep]** `READY`'s stubs call `guilds._add({id, unavailable: true, shardId})`, which patches existing guilds to `available = false`. This is what makes a re-identify produce `guildAvailable`. `Guild`'s constructor also marks a guild with no `channels` in its payload as `available = false`, so a REST-fetched uncached guild is "unavailable" until its `GUILD_CREATE`. `Sweepers._sweepGuildDirectProp` skips unavailable guilds.
8. **[keep]** `GUILD_DELETE` with `unavailable: true` is an outage, not removal. Removal must cascade through `channels._remove`, and destroy the guild's voice adapter.
9. **[keep]** `shardId` is injected into raw guild data by `READY`, `GUILD_CREATE`, and `GuildManager.fetch` (computed). `Guild.shardId` is only set in the constructor. `GuildMemberManager._fetchMany` and `voiceAdapterCreator` send on `guild.shardId`. Shard formula: `(guild_id >> 22) % shard_count`.
10. **[keep]** `SHARDS` and `SHARD_COUNT` apply only when `ws.shardIds` and `ws.shardCount` are identity-equal to defaults.
11. **[keep]** Token handling: `/^bot\s*/i` is stripped. The token is a non-enumerable property, and only `_censoredToken` (everything after the second `.` starred) is logged. `VOICE_SERVER_UPDATE` debug masks its token. Every `Debug` impl in Rust must redact tokens, including interaction and webhook tokens (both hidden from enumeration here).
12. **[keep]** Handlers for guild-scoped events return silently when the guild is not cached, and unknown dispatch names are ignored. New gateway events must not crash deserialization.
13. **[keep]** `RATE_LIMITED`: `RequestGuildMembers` is routed to the pending request by `d.meta.nonce`, rejecting with `GatewayRateLimitError(d, requestData)`, with `retry_after` inside `d`. Other opcodes warn once per opcode per process.
14. **[keep]** Presence: `options.presence` is parsed into `ws.initialPresence` for `IDENTIFY`. `ClientPresence._parse`: `afk` non-boolean gives false, `since` non-number gives null, `activity.type ??= Playing`. A `Custom` activity with no `state` moves `name` to `state` and sets `name = 'Custom Status'`. If `activities` is not given and any of `status`, `afk`, `since` is truthy, current activities are re-sent. Only `type`, `name`, `state`, `url` go on the wire. `shardId` may be one id, an array, or omitted for all shards (`_broadcast`).

### Member and soundboard requests over the gateway

15. **[keep]** `GuildMemberManager.fetch`:
    - `fetch()` and `fetch({query, limit, withPresences, users, time, nonce})` go over **op 8**, not REST.
    - A single resolvable with no `limit` and no `withPresences` goes over REST.
    - Default `limit = 0`, `time = 120_000`. `query` is `initialQuery ?? (users ? undefined : '')`.
    - `nonce` defaults to a generated snowflake and must be `<= 32` characters (`MemberFetchNonceLength`).
    - Completion: `members.size < 1000 || (limit && fetched >= limit) || index === chunk.count`. Each chunk refreshes the timeout. Timeout gives `GuildMembersTimeout`.
    - Return type varies: `users` given as a non-array with at least one result returns the single member; otherwise a Collection.
16. **[keep]** `GuildManager.fetchSoundboardSounds({guildIds, time = 10000})`: groups guild ids by shard and sends op 31. Each `soundboardSounds` event refreshes the timeout, even for unrelated guilds. Timeout gives `GuildSoundboardSoundsTimeout`.

### Cache identity and update semantics

17. **[decide]** Cached structures are shared mutable objects: `_patch` mutates in place, and every holder sees the change. `_update` returns the *old* state as a shallow clone. In Rust this conflicts with plain `Arc<T>`. The design must pick between interior mutability and copy-on-write snapshots (`ArcSwap`), and must still deliver `(old, new)` pairs on update events. Getters that resolve by id at read time (`message.channel`, `member.roles`) have to keep working, which argues for holding ids and a client handle, not references. `client` back-references make cycles if strong; use `Weak` or a cloneable client handle.
18. **[keep]** **Absent vs null.** `'x' in data` distinguishes "absent, keep" from "null, clear", everywhere in `_patch`. Partial payloads (`resolved.channels`, `MESSAGE_UPDATE` embeds-only, `member` without `user`) rely on this. Serde models need a three-state field. Asymmetries to carry over:
    - `GuildMember.avatar_decoration_data` absent **resets to null** (`if (data.avatar_decoration_data) … else = null`).
    - `Message.edited_timestamp` is only ever set when truthy, never cleared.
    - `User.banner` and `User.accentColor`: `undefined` = not fetched, `null` = none.
    - `GuildMember.pending`: `??= false` only when not partial, and depends on `joined_at` being patched first.
    - `Webhook.token` is redefined on every `_patch` as `data.token ?? null`.
    - `Role.tags` is rebuilt on every patch, and presence-style booleans map `null` values in the payload to `true` (`'premium_subscriber' in data.tags`).
19. **[keep]** `ThreadManager.fetchArchived({type = 'public', fetchAll = false, before, limit})`: `type: 'private'` with `!fetchAll` uses the *joined* route, whose `before` is a **thread id**; others use a timestamp. `before` accepts a thread or a date (regex `^\d{17,19}$` for ids). `_mapThreads` result has `hasMore` only if `has_more` is present (`/guilds/{id}/threads/active` lacks it). `fetchActive` always fetches the whole guild's active threads, then filters by parent. Raw thread members' `id` field is the thread id.
20. **[keep]** REST-then-gateway echo:
    - `channel.edit()` patches the cache through `ChannelUpdate` first, so the gateway's `channelUpdate` has `old` equal to `new` for changed fields.
    - `Guild.edit` emits `guildUpdate` immediately and again on the echo.
    - `role.edit` returns a patched clone and leaves the cache for the gateway, so old and new differ properly.
    - `message.edit` returns a clone; `GUILD_ROLE_UPDATE` and `GUILD_SCHEDULED_EVENT_UPDATE` always emit, with no diff.
21. **[keep]** `createChannel`:
    - `{cache: false}` does **not** prevent insertion into `guild.channels` or `parent.threads`; only `allowUnknownGuild` does. `ChannelManager._add` returns the existing channel regardless of `cache`.
    - Dispatch: no `guild_id` and no guild: `recipients` (not GroupDM) or `type === DM` gives `DMChannel`; `GroupDM` gives `PartialGroupDMChannel`. `guild_id` set with no cached guild: nothing, null, debug line.
    - Unknown channel types yield null.
    - `DMChannel` forces `type = DM`, so partials have a known type.
22. **[keep]** `PRESENCE_UPDATE`:
    - Caches the user if it has `username` or with `Partials.User`; syncs via `UserUpdate` if `!_equals`.
    - Requires a cached guild.
    - **Synthesizes a member** `{user, deaf: false, mute: false}` with `guildMemberAvailable` if missing and `status !== 'offline'`. This is a partial member.
    - `presenceUpdate` is emitted only if a listener exists and `!equals(old)`. A Rust dispatcher may not be able to count listeners; skipping the clone is the point of that check.
23. **[keep]** `getReaction` decodes `emoji.name` with `decodeURIComponent`, and `ReactionManager._add` keys on `emoji.id ?? emoji.name`. Routes use `Emoji.identifier`: `a:name:id` or `name:id` for custom, `encodeURIComponent(name)` for unicode. `MessageManager.react` builds the same. `resolveIdentifier` avoids double-encoding by checking for `%`. `parseEmoji` decodes first when `%` is present.
24. **[keep]** Reaction count: `_patch` does `count ??= data.count`, so only the first value sticks. `_add`: increments unless it is the client re-adding its own already-counted reaction (`!me || user != client || count == 0`). `_remove`: decrements only if `!me || user != client`, so **the client removing its own reaction does not lower `count`** while `me` is still true (`me` is cleared afterwards); it deletes the reaction when `count <= 0 && users empty`. Check this against the API before keeping it; it may be a defect. Partial reactions (`count === null`) ignore `_add` and `_remove`. `Message.react` calls the action with `fromStructure = true`, so the gateway echo does not double count.
25. **[keep]** `MessageReactionRemoveEmoji` deletes from the cache only for non-partial messages. `getUserFromMember` uses `member.user` in guild reaction and typing packets, which also populates the member cache. Reaction remove has no `member` and uses `getUser`.
26. **[keep]** Counters: `memberCount` and `messageCount` are maintained by increments even for uncached entities. `MessageCreate` increments thread counters before the duplicate check.
27. **[keep]** `ChannelUpdate` type change (text to announcement and back): a new structure replaces the old, message cache copied, `old` is the pre-patch clone of the old class. In Rust: an enum-variant swap, with messages retained.
28. **[keep]** `_remove` (`ChannelManager`) also clears invites pointing at the channel, removes the thread from `parent.threads`, removes the channel's threads from client and guild caches. `sweepThreads` goes through it too.
29. **[keep]** `Guild._patch` clears and rebuilds `channels`, `roles`, `members`, `stageInstances`, `scheduledEvents`, `voiceStates`, `soundboardSounds` when present, but **emojis and stickers are diffed** via actions on re-patch, emitting events. `this.emojis` and `this.stickers` are created inside `_patch`, not in the constructor. `presences` are merged, never cleared. Note that `channels.cache.clear()` does not clear the client channel cache, whose entries are then re-added.
30. **[keep]** `VoiceState`: `streaming` is only assigned when `self_video` is present, since `self_stream` is omitted when false. States are kept after leaving (`channelId` null). `GuildMember.voice` returns a synthetic empty state when none exists.
31. **[keep]** `Message._patch`:
    - The `author` user is cached only if not from a webhook (`users._add(author, !webhook_id)`).
    - `createdTimestamp` is derived from the snowflake, not `timestamp`.
    - `data.member` is merged with `{user: author}`; `thread` goes to the channel cache; `referenced_message` is added to the channel's messages; `MessageMentions` caches users and members from `mentions[].member`.
    - `message_snapshots`: each snapshot is `_add`ed **under the reference's message id** in the referenced channel, when cached.
    - `reactions` present rebuilds the manager.
    - `guildId` falls back to `channel.guild.id`.
32. **[keep]** Getter gates on `Message`:
    - `deletable`: never for `UndeletableMessageTypes`; DMs only own; `Administrator` overrides a timeout; automod messages are not "own".
    - `bulkDeletable`: under 14 days (`MaxBulkDeletableMessageAge = 1_209_600_000`).
    - `crosspostable`: announcement channel, not crossposted, not forward, `Default` type, no poll, guild has `News` feature, `SendMessages` plus `ManageMessages` unless own.
    - `editable`: thread archive and lock rules; forwards not editable.
    - `pinnable`: `ReadMessageHistory | PinMessages`.
    - `system`: `!NonSystemMessageTypes.includes(type)` (`Default, Reply, ChatInputCommand, ContextMenuCommand`).
33. **[keep]** `TextBasedChannel.bulkDelete(messages, filterOld = false)`: array or Collection, or number (fetches last N). Empty returns `[]` with no call. Exactly **one id uses single `DELETE`**, because the bulk route needs at least two. `filterOld` drops messages older than 14 days by snowflake.
34. **[keep]** Positions:
    - `rawPosition` is API-sent; `position` is computed among sortable groups.
    - Sortable groups: Text, Announcement, Forum, Media / Voice, Stage / Category.
    - Channels: same parent unless a category. **Ties:** channels by ascending id, roles by descending id (`discordSort`, `Role.position`, `comparePositions`).
    - `setPosition` sends the **full reindexed list** of the group. Role comparisons use `position`; `RoleManager.highest` reduces over `comparePositionTo`.
    - `GuildChannelManager.edit` with `position` does a separate PATCH first, non-atomically. It passes `{position, reason}` to `setPosition`, so `relative` is undefined.
35. **[keep]** `GuildChannelManager.edit` with `lockPermissions` computes overwrites client-side from the new or current parent and still sends `lock_permissions`. `PermissionOverwrites.resolve`: a string `id` requires `type`; a structure infers it, with mismatch rejected. `allow` and `deny` are strings on the wire. `resolveOverwriteOptions` maps `true` to allow, `false` to deny, `null` to neither, starting from the existing overwrite in `edit`, and from nothing in `create`.
36. **[keep]** Permission values travel as decimal strings. `PermissionsBitField.toJSON` does this; `Role.permissions`, `install_params.permissions`, `default_member_permissions` and `app_permissions` are parsed with `BigInt`. `defaultMemberPermissions: null` means "default"; `0` is "admins only" and must not be treated as null. In `Client.generateInvite` a `0n` value is omitted.
37. **[fix]** `BitField` uses JS `number` for all but permissions, and JS bitwise operators truncate to 32 bits. `UserFlags.Quarantined`, `Collaborator` and `RestrictedCollaborator` are above bit 31, so `has` is wrong for them upstream. Use `u64` everywhere (`u64` also covers permissions at bit 52 and leaves headroom; check for future bits).
38. **[keep]** Snowflakes are strings in JS. Regexes assume `\d{17,19}` (`resolvePartialEmoji`, `parseEmoji`, `parseWebhookURL`, `cleanContent`, `fetchArchived`). A `u64` snowflake type needs string serde and a `Display` that matches. `Snowflake.compare` must be numeric, not lexical.
39. **[keep]** Query defaults that differ from API defaults: `with_counts` true in `GuildManager.fetch` and `BaseGuild.fetch`; `with_user_count` true for scheduled events; reaction users `limit = 100`, `type = Normal`.
40. **[keep]** `X-Audit-Log-Reason` is URI-encoded by REST (`packages/rest/src/lib/REST.ts`). It is sent only when `reason.length` is non-zero. `prune` with `dry` passes roles as a comma-joined string in the query and uses GET, otherwise an array in the JSON body and POST. `compute_prune_count` is from `count`. `prune` requires numeric `days` (`PruneDaysType`).
41. **[keep]** Other wire-format conventions:
    - Stickers upload as multipart with `appendToFormData: true`, file key `file`, `description ?? ''`.
    - Soundboard sounds as base64 data URLs with mime from option, HTTP response or `magic-bytes`.
    - Invite target users as `target_users_file`, `users.csv`, `text/csv`.
    - Forum posts nest the message body under `message`, with files at top level.
    - `X-Discord-Locale` header on command list with `locale`.
    - Application command `set` replaces all commands remotely (bulk PUT).
    - `editOnboarding` must generate ids for new prompts (comment in `Guild.js`).
    - `ApplicationCommand.transformOption` defaults `required: false` on non-subcommand options, which affects `equals` comparisons with API data.
42. **[keep]** Timers are `.unref()`ed: sweepers, collectors, ready timeout, member and soundboard waits. They never keep a process alive. The Rust equivalent is tasks that are cancelled on `destroy`, and do not block shutdown.
43. **[keep]** `Typing.timestamp` is in seconds (`* 1000`), unlike the ISO strings used elsewhere. `Activity.timestamps.start` and `end` are stored as `Date`s built from `Number(value)` (numeric ms on the wire); `Activity.createdTimestamp` is `created_at` as is.
44. **[keep]** `GuildMemberRoleManager.cache` always contains `@everyone`, keyed by guild id, and silently drops unknown role ids. `add` with a single id appends locally to a clone without relying on the response.
45. **[keep]** `resolveId` returns any string unverified. `GuildMemberManager.resolveId`, `ThreadMemberManager.resolveId` and `PresenceManager.resolveId` differ: for non-member input they return an id only if the entry is cached. `GuildMemberManager.addRole` and `removeRole` therefore pass `null` into the route when the member is not cached. Prefer typed ids in Rust; do not reproduce the null route.
46. **[fix]** `UserManager.createDM` appends `{id: client.user.id}` to `recipients`, so `DMChannel.recipientId` works. `DMChannel.recipientId` is null when the client id is not among `recipientIds`, which is the case for a partial DM created from an incoming message, since `getChannel` only adds the author. Then `users.dmChannel(userId)` will not find it and `DMChannel.fetch()` calls `createDM(null)`.

### Interactions

47. **[keep]** `reply`, `deferReply` and the like set their flags only after the HTTP call resolves. Two concurrent calls both pass the guard, and the second gets an API error. A Rust port can make this atomic, but must leave the state unchanged on HTTP failure so that a retry is possible.
48. **[keep]** `editReply` and `followUp` set `replied = true` after a defer. `deferred` stays true. `ephemeral` is set by `reply` and `deferReply` only; it stays null for `update`. `InteractionResponses.reply`'s `flags` and `ephemeral` come from the resolved body.
49. **[keep]** Interaction and webhook requests use `auth: false`, so the bot token is never sent. These requests run with the interaction token in the URL; do not log it.
50. **[keep]** Discord's 3-second initial response window is not enforced client-side anywhere in `src/`, and neither is the token's lifetime. Autocomplete has no defer. `isRepliable()` is false for Ping and Autocomplete.
51. **[keep]** Interaction resolved data **writes to caches**, for example resolved channels are partial yet get `client.channels._add(channel, guild)`. If the guild is uncached (user-install or no `Guilds` intent), the field holds the raw API object. In Rust use a two-variant enum (`Cached` vs `Raw`), and the same for `BaseInteraction.member`.
52. **[keep]** `channelId` comes from `data.channel?.id`. `interaction.channel` is a cache lookup and null without `Partials.Channel` in DMs, yet `channelId` still works for sending.
53. **[keep]** `MessageComponentInteraction.message`: `channel.messages._add(message)`, or a new detached `Message`. `ModalSubmitInteraction.message` is only set when from a component.
54. **[keep]** `AutocompleteInteraction.respond` applies `jsonTransformer` per choice and passes `nameLocalizations` as `name_localizations`.

### Sharding

55. **[keep]** Sequential spawn with a 5.5 s default delay, ahead of `session_start_limit`. Identify concurrency in-process is handled by `SimpleIdentifyThrottler(max_concurrency)`, which is per process; multi-process sharding relies on this delay.
56. **[fix]** IPC correlation is by string: two concurrent identical `broadcastEval`s from a child resolve on the same reply. `Shard` deliberately de-duplicates identical pending evals. Use request ids.
57. **[keep]** `_disconnect` and `_resume` fire per ws shard, so a process hosting several shards reports state ambiguously. `Shard.ready` only covers one bit. `Shard.kill` removes the exit listener first, so that a deliberate kill does not auto-respawn, and `_handleExit(false)` still emits `death`.

### Data resolving and safety

58. **[fix]** `resolveFile` treats any non-URL string as a **local file path**, and `http(s)` strings as URLs to fetch without a status check. With user-supplied input this is a local-file-read and SSRF risk. Use an explicit `enum Attachment { Bytes, Path, Url, Reader }` in Rust.
59. **[fix]** `resolveImage` labels all buffers `image/jpg`. `resolveColor` accepts `[r, g, b]` without per-channel range checks; only the total is checked.
60. **[keep]** `MessageMentions.channels` and `parsedUsers` are lazily parsed from content, and return only cached entities. `has()` has an exact order: everyone (unless `ignoreEveryone`), replied user (unless `ignoreRepliedUser`), then direct user, role and channel (all three gated by `ignoreDirect`), then the member's roles (unless `ignoreRoles`). With `ignoreRepliedUser`, a direct match also requires the id to be in `parsedUsers`, meaning actually written in the content.
61. **[keep]** `cleanContent` replaces mentions using caches, leaving unknown ones intact. In DMs `@&` is left as is. Slash mentions become `/name`, emoji `:name:`.

### Upstream defects seen while reading (do not reproduce)

62. **[fix]** `InteractionCollector.dispose` checks `this.type`, which is never set (it should be `interactionType`). This is unreachable anyway, as `handleDispose` is never subscribed.
63. **[decide]** Not a defect, but do not copy the shape: `InteractionCollector`'s `end` handler calls `removeListener` for every handler, including prototype methods that were never bound or subscribed, which is a harmless no-op in JS. In Rust, tie subscriptions to the collector's lifetime (a drop guard).
64. **[fix]** `InteractionResponses.applyToClass(ModalSubmitInteraction, 'showModal')` passes a **string** where an array is expected. `.includes(prop)` becomes a substring test, which works only by coincidence.
65. **[fix]** `INVITE_CREATE` and `INVITE_DELETE` dereference `guild.invites` when only the *channel* was checked.
66. **[fix]** `GuildMemberRoleManager.clone()` builds its clone on the **same** member and overwrites that member's `_roles` (`clone.member._roles = [...this.cache.keys()]`), dropping unknown ids and adding the everyone id. `.cache` inserts `guild.roles.everyone` even when it is undefined (`cache.set(guild.id, roles.everyone)`), so a member with an uncached everyone role yields an `undefined` value. `TextBasedChannel`'s constructor is never used, as only the prototype is copied.
67. **[decide]** `GuildAuditLogsEntry`, `Targets.Message`: `MessageBulkDelete` resolves `target` from the channel cache (source comment: "Discord sends a channel id for the MessageBulkDelete action type"), or `{id}` if uncached. All other message actions resolve `target` from the **user** cache, or null. This looks deliberate rather than a bug, but the source does not explain it. Verify what `target_id` holds in the API documentation before naming the Rust field.
68. **[fix]** `Sweepers.sweepEntitlements` has no null check on `client.application`, unlike `sweepApplicationCommands`, so an interval firing before `READY` throws.
69. **[fix]** `Events.Invalidated`, `ClientInvalidProvidedShards` and `closeTimeout` are dead. Do not port.

## 17. Porting notes

These are suggestions for the design phase, not decisions.

### Rust mapping

| JS concept | Rust direction |
|---|---|
| `Collection` (ordered `Map`) | An insertion-ordered map; plain `HashMap` breaks `first()` and eviction. Evicting the oldest entry must be cheap (`hashlink`-style, not `IndexMap::shift_remove`, which is O(n)). Concurrent access needs a design (shard locks, or a single-writer actor). |
| `makeCache` factory, keyed by class name | Per-manager policy config: `CachePolicy {Unbounded, Limited {max, keep}, Disabled}`. Enforce "unsupported" managers in the type (no limit available for guilds, roles, overwrites). |
| Event emitter | `enum Event` plus trait handlers or a stream. Async listeners must not block packet handling; document ordering. |
| Class hierarchy | Channels: `enum Channel { Text, Announcement, Voice, Stage, Category, Forum, Media, Directory, Thread, Dm, PartialGroupDm, Unknown }` with traits (`TextBased`, `GuildChannelLike`, `VoiceBased`). Interactions: enum plus shared struct. Components: enum with `Unknown(serde_json::Value)`. |
| Mixins | Traits with default methods, or methods on the enum. |
| `*Resolvable` | `impl Into<Id<T>>` where possible; dedicated enums for emoji, attachment, colour. |
| `(old, new)` updates | Clone before patch only if somebody listens; the cost matters for presence and member updates. |
| `broadcastEval` | Typed RPC (§10.3). In-process sharding as the primary mode. |
| JSON transformer | Typed builders with `serde(skip_serializing_if)`. Note `tts`, `enforce_nonce` are always sent, and `content: ""` differs from absent. |

### Suggested order, by the priorities above

1. Snowflake, bitfields, `Collection`, errors, `Options`.
2. `Base` conventions and raw models. `Client` shell with REST and WS. `READY`, `GUILD_CREATE`, ready state machine.
3. `User`, `Guild`, `GuildMember`, `Role`, channels plus `PermissionOverwrites` and permission computation.
4. `Message`, `MessagePayload`, `MessageMentions`, `Attachment`, `Embed`, components (received).
5. Interactions, resolvers, responses, `InteractionWebhook`, `ApplicationCommand` and managers.
6. Collectors, partial handling, cache limits, sweepers.
7. Everything P1, then sharding (internal first, then process), then P2.

### Open questions for the design owner

- Mutable-shared vs snapshot cache (item 17) is the largest decision. It affects every public signature.
- Whether to keep client-level `status` queueing (items 3 and 4) or per-shard readiness.
- Whether `partials` stays a runtime option, or the API always yields partial-aware types. A static type (`User` vs `PartialUser`) would be stricter than JS, but `_add` patches partials into full objects in place, so one runtime type with optional fields tracks upstream more closely.
- Parity with the voice adapter (`Guild.voiceAdapterCreator`) depends on whether a voice crate is planned. Only the send-payload and forward-events contract in `ClientVoiceManager` lives in this package.
