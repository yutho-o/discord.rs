# discord.rs design input: types, builders, formatters, collection, util

Sources: discord-api-types 0.38.57 (`ref/dat`, commit 00cbe03) and discord.js monorepo (`ref/djs`, commit 5bb7492), both dated 2026-11-06. Paths starting `payloads/`, `gateway/`, `rest/`, `rpc/`, `voice/`, `utils/`, `globals.ts` are relative to `ref/dat`. Paths starting `packages/` are relative to `ref/djs`.

## 1. Conclusions

1. **Types: generate once, then own the output.** Use a throwaway Node script on the TypeScript type checker to produce the bulk of the Rust types. Commit the output as normal Rust. Hand-write the roughly 20 hard areas listed in section 8.3. About 87% of non-RPC object declarations (885 of 1017) map mechanically.
2. **Builders, Collection, formatters and util: port by hand.** They hold behaviour, not data. Port their vitest vectors as data-driven tests.
3. **RPC: leave out of v1.** `rpc/` is the desktop-client IPC protocol (290 interfaces, 9 enums), which bots do not use. Voice v8 is needed; voice v4 is not.
4. **Three things surprised me and contradict the obvious port:**
   - zod 4.5.4 measures string lengths in Unicode code points, so builder limits use `chars().count()`. `embedLength` still uses UTF-16 units, so the 6000 embed total uses `encode_utf16().count()`.
   - `fancy-regex` 0.20.0 handles the variable-length lookbehinds in `escapeItalic` and `escapeUnderline`, and passed 23 of 23 upstream test vectors. No hand-written scanner is needed.
   - `#[serde(tag = "type")]` cannot read integer tags, and almost every Discord union uses one. Custom tagged deserialization is mandatory.
5. **Upstream has at least two builder bugs that should not be ported** (section 4.5): any button with `id` set fails validation, and action rows inside containers are not deep-validated. There is also a coverage gap: no builders exist for component types 21, 22 and 23.

### Evidence labels

- **[probed]**: I ran it. Zod probes used real zod 4.5.4 from npm with predicates I transcribed. Rust probes used regex 1.13.1, fancy-regex 0.20.0 and serde_json in my own crate. No cloned source was executed.
- **[checker]**: result from the TypeScript 5.9.3 type checker, reading the sources as data (0 diagnostics).
- **[read]**: read from source, not run. This is the default for unlabelled statements.

## 2. discord-api-types layout

### 2.1 Entry points

- `v10.ts` re-exports `gateway/v10`, `globals`, `payloads/v10/index`, `rest/v10/index`, `rpc/v10`, `utils/internals`, and `Utils` (`utils/v10`).
- Voice is **not** in `v10.ts`. `voice/index.ts` points at `voice/v8.ts`. `voice/v4.ts` has only 2 enums and a version.
- `deno/` is a mirror and is ignored. API versions other than v10 are ignored.

### 2.2 Totals

Totals for the files in the census (section 2.8): LOC 25287, interfaces 806 (46 generic), enums 126 with 1322 members (78 integer, 29 string, 19 bit), type aliases 751 (81 unions, 30 intersections, 35 utility), fields 2309 (921 optional, 286 nullable, 170 both, 229 explicit `| undefined`), 14 functions, 18 consts.

| Group | LOC | Interfaces | Enums (members) | Aliases (unions) | Fields | Optional | Nullable | Both |
|---|---|---|---|---|---|---|---|---|
| `payloads/v10` | 10940 | 276 | 98 (666) | 232 (49) | 1125 | 510 | 173 | 78 |
| `rest/v10` | 6305 | 129 | 4 (14) | 300 (15) | 464 | 335 | 93 | 85 |
| `gateway/v10.ts` | 2830 | 78 | 7 (134) | 139 (5) | 230 | 52 | 15 | 6 |
| `rpc/v10.ts` | 2672 | 276 | 2 (79) | 43 (9) | 372 | 18 | 5 | 1 |
| `voice/v8.ts` | 710 | 28 | 4 (49) | 19 (2) | 62 | 3 | 0 | 0 |

Caveat: the census counts top-level declarations. Fields inside `& { ... }` intersection arms are not counted, so true field totals are slightly higher.

Utility type uses: Pick 55, Record 42, Exclude 31, Omit 29, Partial 19, Required 19, `_StrictPartial` 10, `_AddUndefined...` 6, Extract 4, `_Pick` 3, `_Nullable` 3, `_NonNullableFields` 3, `_Omit` 2, `_DistributiveOmit` 1, `_DistributivePick` 1, `_StrictRequired` 1, NonNullable 1. Generic declarations: 67 (43 in payloads). Inline object field types: 1 (`APIGuildWidgetMember.activity`). Interface index signatures: 0.

`@unstable` tags: application 7, channel 10, gateway 5, guild 8, message 6, sticker 1, user 8, rest/guild 2, rest/index 1. At enum-member level [checker]: 85 unstable and 25 deprecated.

### 2.3 Top-level modules

- **`globals.ts`**: `Snowflake = string`, `Permissions = string`, and `FormattingPatterns`: User, UserWithNickname, UserWithOptionalNickname, Channel, Role, GameProfile (`<@$id>`), SlashCommand, Emoji, AnimatedEmoji, StaticEmoji, Timestamp, DefaultStyledTimestamp, StyledTimestamp, GuildNavigation, LinkedRole. Ids are `\d{17,20}`, emoji names `\w{2,32}`, timestamps `-?\d{1,13}`, styles `DFRSTdfst`.
- **`payloads/common.ts`**: `PermissionFlagsBits` (bigint const object, highest is `BypassSlowmode = 1n << 52n`, bit 47 unused, `ManageEmojisAndStickers` and `ManageGuildExpressions` both `1n << 30n`), `LocalizationMap`, `RESTError`, `RESTErrorFieldInformation`, `RESTErrorGroupWrapper { _errors }`, recursive `RESTErrorData`, `RESTRateLimit { code?, global, message, retry_after }`.
- **`gateway/common.ts`**: `GatewayURLQuery` (`encoding: 'etf'|'json'`, `compress: 'zlib-stream'|'zstd-stream'`).
- **`gateway/v10.ts`**: `GatewayVersion = '10'`, opcodes, close codes, intents, capabilities, 79 dispatch events.
- **`rest/common.ts`**: `RESTJSONErrorCodes` (246 members, 243 distinct), `Locale` (32), `CannotSendMessagesToThisUserErrorCodes`.
- **`rest/v10/index.ts`**: `APIVersion = '10'`, `Routes` (129 functions covering 233 documented method and path lines: 103 GET, 39 POST, 33 PATCH, 21 PUT, 37 DELETE), `CDNRoutes` (24 functions), `StickerPackApplicationId = '710982414301790216'`, `ImageFormat`, `ImageSize`, per-asset format aliases, `DefaultUserAvatarAssets` 0..5, `CDNQuery`, `RouteBases`, `OAuth2Routes`.
- **`rest/v10/*.ts`**: per-resource JSON bodies, queries and results. 300 aliases, most `Result` aliases being renamed payloads.
- **`voice/v8.ts`**: `VoiceGatewayVersion = '8'`, opcodes, close codes, encryption modes, speaking flags, DAVE payloads.
- **`rpc/`**: desktop IPC, out of scope.
- **`utils/internals.ts`**: `_Nullable`, `_NonNullableFields`, `_AddUndefined...`, `_StrictPartial`, `_StrictRequired`, `_UnionToIntersection`, `_Distributive*`, `urlSafeCharacters`.
- **`utils/v10.ts`**: 14 type guards: `isDMInteraction` (has `user`), `isGuildInteraction` (has `guild_id`), `isApplicationCommandDM/Guild`, `isMessageComponentDM/Guild`, `isLinkButton`, `isInteractionButton` (not Link and not Premium), `isModalSubmitInteraction`, `isMessageComponentInteraction`, `isMessageComponentButtonInteraction`, `isMessageComponentSelectMenuInteraction`, `isChatInputApplicationCommandInteraction`, `isContextMenuApplicationCommandInteraction`. In Rust these become accessors on the interaction type.

`RouteBases`: api `https://discord.com/api/v10`, cdn `https://cdn.discordapp.com`, media `https://media.discordapp.net`, invite `https://discord.gg`, template `https://discord.new`, gift `https://discord.gift`, scheduledEvent `https://discord.com/events`.

### 2.4 Enums by kind

**Bit enums (19):** GatewayIntentBits 23, GatewayCapabilityBits 1, ApplicationFlags 16, ThreadMemberFlags 4, ChannelFlags 11, ActivityFlags 9, GuildSystemChannelFlags 6, GuildMemberFlags 11, InviteFlags 1, MessageFlags 14, EmbedFlags 4, EmbedMediaFlags 1, AttachmentFlags 8, UnfurledMediaItemContentScanMetadataFlags 3, UnfurledMediaItemFlags 1, SKUFlags 3, RoleFlags 1, UserFlags 23, VoiceSpeakingFlags 3. `PermissionFlagsBits` is a const object, not an enum, and is a 20th bit set.

**String enums (29):** GatewayDispatchEvents 79, GatewayRequestChannelInfoField 2, ActivityLocationKind 2, AuditLogOptionsType 2, PresenceUpdateStatus 5, ActivityPlatform 8, GuildFeature 40, GuildWidgetStyle 5, MembershipScreeningFieldType 1, EmbedType 9, AllowedMentionsTypes 3, SelectMenuDefaultValueType 3, MessageSearchAuthorType 6, MessageSearchHasType 18, MessageSearchEmbedType 5, MessageSearchSortMode 2, OAuth2Scopes 29, TeamMemberRole 3, ConnectionService 27, NameplatePalette 11, ApplicationWebhookEventType 6, Locale 32, ImageFormat 5, RPCDeviceType 3, RPCVoiceSettingsModeType 2, VoiceConnectionStates 10, RPCCommands 49, RPCEvents 30, VoiceEncryptionMode 7.

**Enums with duplicate values** [checker], which cannot be Rust enum variants: RESTJSONErrorCodes 3, ChannelType 4, GatewayIntentBits 2, ConnectionService 1, ComponentType 1, ReactionType 1. They become associated consts.

**Only `UserFlags` has values above 2^31** [checker] (2^44 Quarantined, 2^50 Collaborator, 2^51 RestrictedCollaborator). Integer flag sets should use `u64` uniformly.

Key values [checker]:

| Enum | Values |
|---|---|
| GatewayOpcodes | Dispatch 0, Heartbeat 1, Identify 2, PresenceUpdate 3, VoiceStateUpdate 4, Resume 6, Reconnect 7, RequestGuildMembers 8, InvalidSession 9, Hello 10, HeartbeatAck 11, RequestSoundboardSounds 31, RequestChannelInfo 43 |
| GatewayCloseCodes | 4000 Unknown, 4001 UnknownOpcode, 4002 Decode, 4003 NotAuthenticated, 4004 AuthenticationFailed, 4005 AlreadyAuthenticated, 4007 InvalidSeq, 4008 RateLimited, 4009 SessionTimedOut, 4010 InvalidShard, 4011 ShardingRequired, 4012 InvalidAPIVersion, 4013 InvalidIntents, 4014 DisallowedIntents (no 4006) |
| ComponentType | ActionRow 1, Button 2, StringSelect 3, TextInput 4, UserSelect 5, RoleSelect 6, MentionableSelect 7, ChannelSelect 8, Section 9, TextDisplay 10, Thumbnail 11, MediaGallery 12, File 13, Separator 14, ContentInventoryEntry 16, Container 17, Label 18, FileUpload 19, RadioGroup 21, CheckboxGroup 22, Checkbox 23; deprecated alias SelectMenu 3 |
| InteractionType | Ping 1, ApplicationCommand 2, MessageComponent 3, Autocomplete 4, ModalSubmit 5 |
| InteractionResponseType | Pong 1, ChannelMessageWithSource 4, DeferredChannelMessageWithSource 5, DeferredMessageUpdate 6, UpdateMessage 7, AutocompleteResult 8, Modal 9, PremiumRequired 10 (deprecated), LaunchActivity 12 |
| ApplicationCommandType | ChatInput 1, User 2, Message 3, PrimaryEntryPoint 4 |
| ApplicationCommandOptionType | Subcommand 1, SubcommandGroup 2, String 3, Integer 4, Boolean 5, User 6, Channel 7, Role 8, Mentionable 9, Number 10, Attachment 11 |
| ApplicationCommandPermissionType | Role 1, User 2, Channel 3 |
| ApplicationIntegrationType | GuildInstall 0, UserInstall 1 |
| InteractionContextType | Guild 0, BotDM 1, PrivateChannel 2 |
| EntryPointCommandHandlerType | AppHandler 1, DiscordLaunchActivity 2 |
| ChannelType | 0-6, 10-16, with aliases GuildNews 5, GuildNewsThread 10, GuildPublicThread 11, GuildPrivateThread 12 |
| ButtonStyle | 1-6 |
| VoiceOpcodes (v8) | 0-9, 11, 13, DAVE 21-31 (23 members; v4 has 24) |
| VoiceCloseCodes | 4001-4006, 4009, 4011, 4012, 4014-4017, 4020-4022 |
| VoiceEncryptionMode | aead_aes256_gcm_rtpsize, aead_xchacha20_poly1305_rtpsize, xsalsa20_poly1305_lite_rtpsize, aead_aes256_gcm, xsalsa20_poly1305, xsalsa20_poly1305_suffix, xsalsa20_poly1305_lite |

Other large integer enums: AuditLogEvent 71, MessageType 38, RPCErrorCodes 28, RPCCloseEventCodes 9.

### 2.5 Biggest unions

Arms: APIAuditLogChange 91, GatewayDispatchPayload 79, RPCCommandsResult 48, RPCMessagePayload 48, RPCEventsDispatch 30, RPCSubscribeArgs 28, RPCCommandSubscribePayload 28, APIChannel 12, VoiceReceivePayload 11, ModalSubmitComponent 10, APIMessageComponent 10, ImageSize 10, APIApplicationCommandBasicOption 9, APIInteractionResponse 9, TextChannelType 9, GatewaySendPayload 8, APIMessageTopLevelComponent 7, VoiceSendPayload 7, GatewayReceivePayload 6, APIComponentInLabel 6, APIComponentInContainer 6, APIWebhookEventBody 6.

### 2.6 Discriminants [checker]

107 unions have object arms. Discriminants found:

| Field | Unions |
|---|---|
| `type` | APIChannel (12 flattened), APIComponent*, APIInteractionResponse, APIMessageInteractionMetadata, APIApplicationCommandOption (14), APIApplicationCommandInteractionDataOption (11), APIApplicationCommandInteractionData (4), ModalSubmitComponent, APIModalSubmissionComponent, APIInteraction (8), APIWebhookEvent, APIWebhookEventBody, APIApplicationIdentityProfileDynamicField, RESTPostAPIApplicationCommandsJSONBody, RESTPostAPIInteractionCallbackJSONBody |
| `style` | APIButtonComponent |
| `component_type` | APIMessageComponentInteractionData (6), APIMessageSelectMenuInteractionData (5) |
| `entity_type` | APIGuildScheduledEvent (3) |
| `key` | APIAuditLogChange (91) |
| `op` | GatewaySendPayload, GatewayReceivePayload, voice payloads |
| `t` | GatewayDispatchPayload |
| `autocomplete` (boolean) | command option wrappers |

**No discriminant:** RESTErrorData, APIComponentEmbedButton, channel-modify intersections, APIContextMenuInteraction and APIApplicationCommandInteraction (discriminated through nested `data.type`), GatewayRequestGuildMembersData (untagged by shape), FormData body unions, OAuth2 URL-encoded unions, RESTGetAPIGuildMessagesSearchResult (result or 202 not-ready). One conditional type outside the option files: RPCCertifiedDevice.

### 2.7 Field shapes that need a decision

**Snowflake fields by name [checker]:** id 244, guild_id 188, channel_id 125, application_id 92, user_id 52, webhook_id 23, last_message_id 20, sku_id 19, parent_id 18, owner_id 18, creator_id 18, emoji_id 15, message_id 12, guild_scheduled_event_id 12, afk_channel_id 11, system_channel_id 11, after 11, rules_channel_id 9, public_updates_channel_id 9, safety_alerts_channel_id 9, sound_id 9, widget_channel_id 8, before 8, version 7, source_guild_id 7, thread_id 6, pack_id 5, client_id 5, role_id 4, long tail about 30 more.

**`number` fields [checker]:** TypeScript has no integer/float distinction. payloads 296, rest 190, gateway 153, rpc 40, voice 40. By name: position 40, rate_limit_per_user 26, attachment_size_limit 26, volume 17, limit 16, seq 13, color 10, min/max_values 9, bitrate 5, height/width 5, max_age/max_uses 5, retry_after 4, min/max_value 4, size 2, duration_secs 2, heartbeat_interval 2, voice_start_time 2.

**String ids that are not snowflakes:** `APIGuildWidgetMember.id`, `APIIntegrationAccount.id`, `GatewayActivity.id`, `APIVoiceRegion.id`.

**Anonymous unions in fields:**
- `GatewayVoiceChannelEffectSendDispatchData.sound_id`: `Snowflake | number`
- `GatewayRequestGuildMembersDataWithUserIds.user_ids`: `Snowflake | Snowflake[]`
- `GatewayActivity.buttons`: `GatewayActivityButton[] | string[]`
- `nonce`: `number | string`
- `RESTAPIAttachment.id`: `Snowflake | number`
- `RESTAPIGuildCreateOverwrite.id`, `RESTAPIGuildCreateRole.id`: `number | string`
- `RESTPostAPIGuildsJSONBody.afk_channel_id`, `.system_channel_id`: `Snowflake | number | null`
- `APITemplateSerializedSourceGuild.afk_channel_id` and related
- `RESTGetAPIChannelThreadsArchivedQuery.before`: `Snowflake | string` (ISO 8601)
- `RESTOAuth2BotAuthorizationQuery.scope`: template literals containing `bot`

**Open string unions:** `GatewayRequestChannelInfoData.fields: (Field | (string & {}))[]`, `ImageSize = ... | (number & {})`, `FileUploadType = 'audio'|'image'|'video'|`.${string}``.

**Literal sets:** `APIGuild.afk_timeout` 60/300/900/1800/3600, `RecurrenceRuleNWeekday.n` 1..5, search `sort_order` asc/desc, OAuth `prompt` consent/none, `token_type_hint`.

**Tuples:** `shard: [id, count]`, `GatewayActivityParty.size: [cur, max]`, `APIApplication.tags` (1 to 5 strings), `APITeamMember.permissions: ['*']` (deprecated), `sticker_ids` (1 to 3 tuple union).

**Literal fields:** `GatewayGuildDeleteDispatchData.unavailable?: true`, `InviteCreate.uses: 0`, `ThreadCreate.newly_created?: true`, `APIBaseInteraction.version: 1`, `APIApplication.summary: ''`, `APISticker.asset: ''`, `APIWebhookEventBase.version: 1`, OAuth `response_type`, `grant_type`, `iss`. `APIApplicationEmoji`: `roles: []`, `require_colons: true`, `managed: false`, `available: true`. `RPCGetGuild.members: []`.

### 2.8 Census per file

Columns: **L** LOC, **I** interfaces, **gI** generic interfaces, **E** enums, **M** enum members, **Bit** bit enums, **Str** string enums, **A** aliases, **uA** union aliases, **iA** intersection aliases, **tA** utility aliases, **F** fields, **O** optional, **N** nullable, **ON** optional and nullable, **U** explicit `| undefined`. `CMD/` abbreviates `payloads/v10/_interactions/_applicationCommands/`, and `CMD/ci/` abbreviates `CMD/_chatInput/`.

| File | L | I | gI | E | M | Bit | Str | A | uA | iA | tA | F | O | N | ON | U |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| gateway/common.ts | 9 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 1 | 0 | 0 | 0 |
| gateway/v10.ts | 2830 | 78 | 2 | 7 | 134 | 2 | 2 | 139 | 5 | 4 | 1 | 230 | 52 | 15 | 6 | 3 |
| globals.ts | 121 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| payloads/common.ts | 362 | 4 | 0 | 0 | 0 | 0 | 0 | 2 | 1 | 0 | 1 | 10 | 2 | 0 | 0 | 0 |
| CMD/ci/attachment.ts | 23 | 2 | 0 | 0 | 0 | 0 | 0 | 2 | 1 | 0 | 0 | 2 | 2 | 0 | 0 | 0 |
| CMD/ci/base.ts | 38 | 3 | 3 | 0 | 0 | 0 | 0 | 1 | 1 | 0 | 0 | 11 | 5 | 2 | 2 | 0 |
| CMD/ci/boolean.ts | 17 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| CMD/ci/channel.ts | 22 | 2 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 2 | 2 | 0 | 0 | 0 |
| CMD/ci/integer.ts | 54 | 3 | 1 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 5 | 5 | 0 | 0 | 0 |
| CMD/ci/mentionable.ts | 19 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| CMD/ci/number.ts | 54 | 3 | 1 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 5 | 5 | 0 | 0 | 0 |
| CMD/ci/role.ts | 18 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| CMD/ci/shared.ts | 37 | 2 | 2 | 1 | 11 | 0 | 0 | 0 | 0 | 0 | 0 | 4 | 2 | 1 | 1 | 0 |
| CMD/ci/string.ts | 51 | 3 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 5 | 5 | 0 | 0 | 0 |
| CMD/ci/subcommand.ts | 23 | 3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 5 | 3 | 0 | 0 | 0 |
| CMD/ci/subcommandGroup.ts | 25 | 3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 5 | 2 | 0 | 0 | 0 |
| CMD/ci/user.ts | 18 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| CMD/chatInput.ts | 171 | 2 | 0 | 0 | 0 | 0 | 0 | 9 | 6 | 0 | 0 | 4 | 4 | 0 | 0 | 0 |
| CMD/contextMenu.ts | 87 | 3 | 0 | 0 | 0 | 0 | 0 | 10 | 4 | 0 | 0 | 5 | 0 | 0 | 0 | 0 |
| CMD/entryPoint.ts | 27 | 0 | 0 | 0 | 0 | 0 | 0 | 4 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| CMD/internals.ts | 10 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 4 | 1 | 0 | 0 | 0 |
| CMD/permissions.ts | 60 | 2 | 0 | 1 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 7 | 0 | 0 | 0 | 0 |
| payloads/v10/_interactions/applicationCommands.ts | 238 | 1 | 0 | 4 | 11 | 0 | 0 | 5 | 4 | 1 | 0 | 19 | 12 | 4 | 3 | 0 |
| payloads/v10/_interactions/autocomplete.ts | 34 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| payloads/v10/_interactions/base.ts | 286 | 10 | 3 | 0 | 0 | 0 | 0 | 9 | 2 | 3 | 1 | 45 | 20 | 0 | 0 | 0 |
| payloads/v10/_interactions/messageComponents.ts | 93 | 6 | 1 | 0 | 0 | 0 | 0 | 8 | 2 | 3 | 0 | 11 | 0 | 0 | 0 | 0 |
| payloads/v10/_interactions/modalSubmit.ts | 115 | 15 | 1 | 0 | 0 | 0 | 0 | 5 | 2 | 1 | 0 | 16 | 1 | 1 | 0 | 0 |
| payloads/v10/_interactions/ping.ts | 5 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| payloads/v10/_interactions/responses.ts | 162 | 11 | 0 | 2 | 14 | 0 | 0 | 3 | 2 | 0 | 1 | 18 | 3 | 0 | 0 | 0 |
| payloads/v10/application.ts | 405 | 6 | 0 | 4 | 29 | 1 | 1 | 1 | 0 | 0 | 0 | 52 | 27 | 6 | 4 | 0 |
| payloads/v10/applicationIdentityProfile.ts | 235 | 7 | 1 | 1 | 3 | 0 | 0 | 4 | 1 | 0 | 0 | 31 | 21 | 3 | 0 | 0 |
| payloads/v10/auditLog.ts | 981 | 4 | 1 | 2 | 73 | 0 | 1 | 94 | 1 | 0 | 0 | 31 | 18 | 2 | 0 | 0 |
| payloads/v10/autoModeration.ts | 231 | 4 | 0 | 4 | 14 | 0 | 0 | 0 | 0 | 0 | 0 | 22 | 10 | 1 | 1 | 0 |
| payloads/v10/channel.ts | 737 | 26 | 10 | 8 | 45 | 2 | 0 | 15 | 3 | 0 | 1 | 76 | 44 | 21 | 13 | 2 |
| payloads/v10/emoji.ts | 83 | 2 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 8 | 6 | 2 | 0 | 0 |
| payloads/v10/gateway.ts | 482 | 14 | 0 | 5 | 31 | 1 | 2 | 2 | 0 | 1 | 0 | 63 | 42 | 6 | 6 | 0 |
| payloads/v10/guild.ts | 1278 | 29 | 0 | 15 | 92 | 2 | 3 | 2 | 1 | 0 | 1 | 155 | 44 | 47 | 16 | 0 |
| payloads/v10/guildScheduledEvent.ts | 261 | 8 | 1 | 6 | 31 | 0 | 0 | 2 | 1 | 0 | 0 | 39 | 7 | 18 | 3 | 0 |
| payloads/v10/index.ts | 26 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| payloads/v10/interactions.ts | 58 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| payloads/v10/invite.ts | 161 | 2 | 0 | 3 | 6 | 1 | 0 | 2 | 0 | 0 | 2 | 20 | 11 | 2 | 0 | 1 |
| payloads/v10/message.ts | 2489 | 61 | 7 | 22 | 163 | 6 | 7 | 19 | 13 | 1 | 1 | 244 | 149 | 15 | 14 | 0 |
| payloads/v10/monetization.ts | 223 | 3 | 0 | 4 | 18 | 1 | 0 | 0 | 0 | 0 | 0 | 26 | 4 | 4 | 0 | 0 |
| payloads/v10/oauth2.ts | 152 | 0 | 0 | 1 | 29 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| payloads/v10/permissions.ts | 129 | 3 | 0 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 22 | 9 | 7 | 5 | 0 |
| payloads/v10/poll.ts | 121 | 8 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 15 | 3 | 1 | 0 | 0 |
| payloads/v10/soundboard.ts | 45 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 8 | 2 | 2 | 0 | 0 |
| payloads/v10/stageInstance.ts | 85 | 2 | 0 | 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 11 | 1 | 0 | 0 | 0 |
| payloads/v10/sticker.ts | 131 | 2 | 0 | 2 | 6 | 0 | 0 | 1 | 0 | 0 | 1 | 19 | 8 | 1 | 0 | 0 |
| payloads/v10/teams.ts | 84 | 2 | 0 | 2 | 5 | 0 | 1 | 0 | 0 | 0 | 0 | 10 | 0 | 1 | 0 | 0 |
| payloads/v10/template.ts | 67 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 14 | 0 | 4 | 0 | 0 |
| payloads/v10/user.ts | 456 | 7 | 0 | 5 | 67 | 1 | 2 | 0 | 0 | 0 | 0 | 44 | 21 | 15 | 9 | 0 |
| payloads/v10/voice.ts | 106 | 3 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 18 | 3 | 2 | 0 | 0 |
| payloads/v10/webhook.ts | 227 | 5 | 2 | 3 | 11 | 0 | 1 | 6 | 2 | 0 | 0 | 24 | 8 | 5 | 1 | 0 |
| rest/common.ts | 427 | 0 | 0 | 2 | 278 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| rest/v10/application.ts | 57 | 0 | 0 | 0 | 0 | 0 | 0 | 7 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| rest/v10/applicationIdentityProfile.ts | 82 | 5 | 0 | 0 | 0 | 0 | 0 | 3 | 1 | 0 | 0 | 6 | 4 | 0 | 0 | 4 |
| rest/v10/auditLog.ts | 33 | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 5 | 5 | 0 | 0 | 0 |
| rest/v10/autoModeration.ts | 84 | 1 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 | 1 | 8 | 4 | 0 | 0 | 4 |
| rest/v10/channel.ts | 1011 | 25 | 0 | 1 | 3 | 0 | 0 | 49 | 3 | 6 | 0 | 94 | 81 | 22 | 21 | 64 |
| rest/v10/emoji.ts | 99 | 3 | 0 | 0 | 0 | 0 | 0 | 11 | 0 | 0 | 2 | 6 | 3 | 1 | 1 | 3 |
| rest/v10/gateway.ts | 12 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| rest/v10/guild.ts | 1256 | 31 | 0 | 0 | 0 | 0 | 0 | 61 | 2 | 3 | 3 | 129 | 113 | 50 | 48 | 79 |
| rest/v10/guildScheduledEvent.ts | 151 | 5 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 | 0 | 18 | 14 | 2 | 2 | 8 |
| rest/v10/index.ts | 1636 | 1 | 0 | 1 | 5 | 0 | 1 | 24 | 2 | 0 | 0 | 1 | 1 | 0 | 0 | 0 |
| rest/v10/interactions.ts | 424 | 11 | 0 | 0 | 0 | 0 | 0 | 38 | 4 | 0 | 5 | 22 | 13 | 0 | 0 | 4 |
| rest/v10/invite.ts | 103 | 3 | 0 | 1 | 4 | 0 | 0 | 4 | 0 | 0 | 0 | 10 | 3 | 3 | 0 | 0 |
| rest/v10/monetization.ts | 143 | 3 | 0 | 1 | 2 | 0 | 0 | 9 | 0 | 0 | 1 | 15 | 12 | 0 | 0 | 12 |
| rest/v10/oauth2.ts | 266 | 13 | 0 | 0 | 0 | 0 | 0 | 8 | 1 | 3 | 1 | 66 | 22 | 0 | 0 | 0 |
| rest/v10/poll.ts | 55 | 3 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 5 | 3 | 0 | 0 | 0 |
| rest/v10/soundboard.ts | 108 | 4 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 | 0 | 12 | 8 | 6 | 6 | 8 |
| rest/v10/stageInstance.ts | 65 | 2 | 0 | 0 | 0 | 0 | 0 | 4 | 0 | 0 | 0 | 7 | 5 | 0 | 0 | 5 |
| rest/v10/sticker.ts | 97 | 3 | 0 | 0 | 0 | 0 | 0 | 9 | 0 | 0 | 0 | 8 | 3 | 1 | 1 | 3 |
| rest/v10/template.ts | 75 | 2 | 0 | 0 | 0 | 0 | 0 | 8 | 0 | 0 | 1 | 4 | 2 | 1 | 1 | 2 |
| rest/v10/user.ts | 160 | 5 | 0 | 0 | 0 | 0 | 0 | 11 | 0 | 0 | 0 | 21 | 13 | 4 | 2 | 6 |
| rest/v10/voice.ts | 65 | 2 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 | 0 | 5 | 5 | 1 | 1 | 5 |
| rest/v10/webhook.ts | 323 | 6 | 0 | 0 | 0 | 0 | 0 | 25 | 2 | 1 | 5 | 22 | 21 | 2 | 2 | 16 |
| rpc/common.ts | 478 | 14 | 1 | 7 | 62 | 0 | 3 | 1 | 0 | 0 | 0 | 43 | 0 | 0 | 0 | 0 |
| rpc/v10.ts | 2672 | 276 | 5 | 2 | 79 | 0 | 2 | 43 | 9 | 1 | 1 | 372 | 18 | 5 | 1 | 0 |
| utils/internals.ts | 55 | 0 | 0 | 0 | 0 | 0 | 0 | 12 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 |
| utils/v10.ts | 202 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| voice/v4.ts | 176 | 0 | 0 | 2 | 40 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| voice/v8.ts | 710 | 28 | 1 | 4 | 49 | 1 | 1 | 19 | 2 | 0 | 0 | 62 | 3 | 0 | 0 | 0 |

## 3. Patterns that are hard in Rust

### 3.1 Discriminated unions with integer tags

Facts [probed]: `#[serde(tag = "type")]` on `{"type":1}` fails with `invalid type: integer 1, expected variant identifier`. A string tag works. So derive only handles the string-tagged unions.

Recommendation: a `tagged_union!` macro that generates `Deserialize` by peeking at the tag and dispatching, plus `Serialize` that writes the tag. Every generated enum has an `Unknown` arm holding the tag and the raw JSON, so a new component type does not fail a whole message.

```rust
tagged_union! {
    #[tag = "type", repr = u8]
    pub enum Component {
        1 => ActionRow(ActionRow),
        2 => Button(Button),
        // ...
        _ => Unknown { kind: u8, raw: serde_json::Value },
    }
}
```

Implementation options, in order of preference:
1. Deserialize into `serde_json::Value`, read the tag, then `from_value` into the arm. Simple and works at any nesting depth. Costs an allocation per node.
2. Write a `Visitor` that buffers keys, for hot paths only, after benchmarks justify it.

Do not rely on `RawValue` nested under `#[serde(flatten)]` or derive-tagged enums; serde buffers those through its internal content type. I did not probe this interaction, so test it before using it.

Special cases:
- **Boolean tag:** `autocomplete` on options. Model as a plain `autocomplete: bool` on one struct, not as an enum.
- **Nested tag:** application command interactions switch on `data.type`. Model as one struct with `data: InteractionData`, tagged inside `data`.
- **Two-level:** buttons switch on `type`, then `style`. Use one `Button` enum keyed by `style`, not 6 component arms.
- **Shape-based:** `GatewayRequestGuildMembersData` is query-or-user-ids. Use an explicit Rust enum, serialize-only, since the client only sends it.
- **Key-based:** `APIAuditLogChange` has 91 arms on a string `key`. Use `{ key: AuditLogChangeKey, new_value: Option<Value>, old_value: Option<Value> }` plus typed accessors, rather than 91 variants. The arms are mechanically generable if typed access proves worthwhile, but the typed values are not worth the compile time at v1.

### 3.2 Forward compatibility with unknown enum values

Discord adds values without a version bump. TypeScript does not care; a closed Rust enum fails to deserialize and would drop the event or kill the shard.

Rules:
- Every integer and string enum has an `Unknown(T)` arm and `#[non_exhaustive]`. Conversions go through `From<T>`, so `Unknown(0)` can never hold a known value. A public `Unknown` constructor would break `PartialEq`; hide it or normalize it in `From`.
- Duplicated values (ChannelType, ComponentType, GatewayIntentBits, ConnectionService, ReactionType) become associated consts: `impl ChannelType { pub const GUILD_NEWS: Self = Self::GuildAnnouncement; }`.
- `RESTJSONErrorCodes` (243 distinct values) becomes a newtype `JsonErrorCode(pub u32)` with associated consts, because code matches on a handful of values, never exhaustively.
- Never use `deny_unknown_fields` on received types.
- Bit sets keep unknown bits: use `bitflags` with `from_bits_retain`, and never `from_bits_truncate` on received data. A truncate followed by a PATCH would silently clear flags.
- A dispatch payload that fails to parse must surface as an event, not kill the shard. See 3.9.
- Builders parity: `z.enum` rejects unknown values, so validators reject `Unknown` by default. Disabling validation is the escape hatch.

### 3.3 Optional, nullable and both

Counts: 921 optional, 286 nullable, 170 both (85 of them in `rest/v10`), plus 229 explicit `| undefined`.

Facts [probed]: `Option<Option<T>>` with plain `#[serde(default)]` gives `None` for both `null` and absent. Telling them apart requires a custom deserializer (the `serde_with::rust::double_option` shape).

| TypeScript shape | Direction | Rust |
|---|---|---|
| `x: T` | any | `T` |
| `x?: T` | receive | `Option<T>` with `default` |
| `x: T \| null` | receive | `Option<T>` (must accept `null`; require presence is not worth enforcing) |
| `x?: T \| null` | receive | `Option<T>`, unless the semantic differs (below) |
| `x?: T \| null` | send (PATCH) | `Patch<T>`: `Absent`, `Null`, `Value(T)`, skip on `Absent` |
| `x?: T \| undefined` | send | `Option<T>`, skip on `None`. `undefined` means absent only, so all 229 are ordinary optionals. |

Receive-side fields where null and absent differ, and which therefore need a tri-state:
- `APIMessage.referenced_message`: absent means not fetched; `null` means deleted (`payloads/v10/message.ts`, about lines 177-187).
- `APIRoleTags.premium_subscriber`, `available_for_purchase`, `guild_connections`: typed `?: null`, and presence means true. Use a deserializer that maps presence to `bool`.

Serialize-side fields that must emit `null` rather than be omitted:
- Gateway heartbeat `d`: `number | null`.
- Every `Patch::Null` in REST bodies, for example `RESTPatchAPIChannelMessageJSONBody` content, embeds, flags, allowed_mentions, components.
- `_NonDispatchPayload` has `t: null, s: null` on receive, which is simply ignored.

### 3.4 Partial payloads

`Partial<APIUser>`, `Pick<...>`, `Omit<...>` are everywhere: `GatewayPresenceUpdate.user` is `Partial<APIUser> & Pick<'id'>`, `GatewayReadyDispatchData.application` is `Pick<'flags'|'flags_new'|'id'>`, `GatewayGuildMemberUpdateDispatchData` mixes Partial and Required.

Recommendation:
- The checker flattens these (`getPropertiesOfType`) [checker], so the generator emits flat named structs with no `serde(flatten)`. That also avoids the buffering cost of flatten.
- Do not emit one Rust type per TypeScript alias. Merge identical property sets through an override table, and pick a canonical name (`PartialUser`, `PartialChannel`, `PartialApplication`).
- `APIDMInteractionWrapper` and `APIGuildInteractionWrapper` use `Omit` and `Required` and produce a combinatorial set of types. Replace with a single `Interaction` struct where `guild_id`, `member`, `user` are `Option`, plus context accessors (`is_guild()`, `is_dm()`, `invoker()`), matching `utils/v10.ts`.

### 3.5 Bitfields as strings

- `Permissions` is a decimal string on the wire. Rust: `bitflags` over `u64`, custom serde to and from decimal string. Bit 52 is the highest today, so `u64` fits. Do not use `u128` until Discord passes bit 63.
- `APIApplication.flags` is a number and `flags_new` is a string for the same concept.
- Integer flag sets use `u64`, because `UserFlags` reaches 2^51.
- Accept both string and integer when reading flag fields: a tolerant deserializer costs little.
- `MessageFlags` on PATCH must include previously set bits (see section 9.3).

### 3.6 Snowflake

`Snowflake(u64)` newtype, serialized as a string, deserialized from string or integer (ETF sends integers). Provide `FromStr`, `Display`, `timestamp()`, and `Hash`/`Ord`.

Typed markers versus a single newtype is open (section 10). The evidence: field names resolve most markers mechanically, but `id` (244 uses), `before`, `after`, `around`, `version`, `target_id`, `entity_id` need owner-based overrides.

Non-snowflake unions get their own types, not `Snowflake`:
- `IdOrIndex`: `Snowflake | number` on `RESTAPIAttachment.id`, placeholder ids in guild creation.
- `SnowflakeOrTimestamp`: thread archive `before`.
- `Nonce`: `Int(i64) | Str(String)`.
- `SoundId`: `Snowflake | number` on voice channel effects.

Arithmetic on ids exists: `APIApplicationCommandPermissionsConstant.AllChannels(guildId) = guildId - 1` (`payloads/v10/_interactions/_applicationCommands/permissions.ts:58`). `Everyone(guildId) = guildId`.

### 3.7 Numbers

TypeScript `number` hides int versus float. Facts [probed]: `i64` from `1.0` errors in serde_json (`invalid type: floating point`).

The generator needs a type table per field:
- Default `u64` is risky, since a negative or float value fails the whole payload. Use `i64` for counts unless known non-negative, and `u64` only for known unsigned.
- Floats: `retry_after`, `volume`, `heartbeat_interval` (tolerant), `reset_after`, `voice_start_time`.
- A tolerant numeric deserializer that reads `30.0` as an integer belongs in shared helpers, for fields where Discord might emit either.
- `secret_key: number[]` in voice is `Vec<u8>`.

Mixed time units, which a `Duration` wrapper must not confuse:
- `TypingStart.timestamp`: seconds.
- `GatewayActivity.created_at`: milliseconds.
- `presence.since`: milliseconds.
- session `reset_after`: milliseconds.
- `retry_after`: seconds, may be fractional.
- `heartbeat_interval`: milliseconds.
- Most REST timestamps: ISO 8601 strings, wrapped in a `Timestamp` newtype.
- Audit log option counts (`delete_member_days`, `members_removed`, `count`) are numeric strings.

### 3.8 Other mechanics

- **Int-keyed maps:** `APIAuthorizingIntegrationOwnersMap` and `APIApplicationIntegrationTypesConfigMap` are keyed by a stringified integer enum. `HashMap<u8, _>` from string keys works [probed]; open-enum keys need a `Deserialize` that accepts map-key strings.
- **Recursive `RESTErrorData`:** `RESTErrorFieldInformation | RESTErrorGroupWrapper | string | { [k]: RESTErrorData }`. Hand-write as an enum over `Value` and provide a flattening helper that yields `(path, code, message)`.
- **Literal fields:** emit as plain primitives, not validated. Discord may start sending other values, and failing parse over `version: 1` buys nothing.
- **Autocomplete option values:** `APIApplicationCommandInteractionDataIntegerOption.value` and the number variant are `Type extends Autocomplete ? string : number` (`.../_chatInput/integer.ts`, `number.ts`). While a user is typing, the value is a raw string. Model as `OptionValue::Focused(String)` or a focused flag with a string.
- **`APIModalSubmitRadioGroupComponent.value`:** `string | null`. `CheckboxComponent.value`: `boolean`.
- **FormData bodies:** `Record<`files[${bigint}]`, unknown> & { payload_json? }` or inline fields. Hand-write a `Multipart` encoder.
- **Voice binary:** `VoiceBinaryPayload = ArrayBuffer` for DaveMlsExternalSender, KeyPackage, Proposals, CommitWelcome, AnnounceCommitTransition, Welcome. These are binary WebSocket frames, not JSON. Hand-write.
- **`RESTGetAPIGuildMessagesSearchResult`:** a result, or a 202 with `{ message, code: 110000, documents_indexed, retry_after }` (`rest/v10/guild.ts`, about line 571). Dispatch on HTTP status, not body shape.

### 3.9 Gateway decoding

Use two-phase decode: first `{ op, s, t, d: Box<RawValue> }`, then decode `d` by `(op, t)`. `GatewayEvent` has an `Unknown { t: String, raw: Box<RawValue> }` arm, and a typed decode failure yields `DecodeFailed { t, raw, error }` rather than an error on the shard loop. Sequence number `s` must be updated from the envelope **before** typed decode, so that a bad payload does not break resume.

## 4. Builders (`packages/builders/src`, v1.11.1, 8099 LOC)

Dependencies: zod ^4.5.4, ts-mixer, @discordjs/util.

### 4.1 Exported classes (71 plus ValidationError)

ActionRowBuilder, AllowedMentionsBuilder, ApplicationCommandNumericOptionMinMaxValueMixin, ApplicationCommandOptionBase, ApplicationCommandOptionChannelTypesMixin, ApplicationCommandOptionWithAutocompleteMixin, ApplicationCommandOptionWithChoicesMixin, AttachmentBuilder, BaseButtonBuilder, BaseSelectMenuBuilder, ChannelSelectMenuBuilder, ChatInputCommand{Attachment,Boolean,Channel,Integer,Mentionable,Number,Role,String,User}Option, ChatInputCommandBuilder, ChatInputCommandSubcommandBuilder, ChatInputCommandSubcommandGroupBuilder, CommandBuilder, ComponentBuilder, ContainerBuilder, ContextMenuCommandBuilder, CustomIdButtonBuilder, DangerButtonBuilder, EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder, EmojiOrLabelButtonMixin, FileBuilder, FileUploadBuilder, LabelBuilder, LinkButtonBuilder, MediaGalleryBuilder, MediaGalleryItemBuilder, MentionableSelectMenuBuilder, MessageBuilder, MessageContextCommandBuilder, MessageReferenceBuilder, ModalBuilder, PollAnswerBuilder, PollAnswerMediaBuilder, PollBuilder, PollMediaBuilder, PollQuestionBuilder, PremiumButtonBuilder, PrimaryButtonBuilder, RoleSelectMenuBuilder, SecondaryButtonBuilder, SectionBuilder, SeparatorBuilder, SharedChatInputCommandOptions, SharedChatInputCommandSubcommands, SharedClientThemeBuilder, SharedName, SharedNameAndDescription, StringSelectMenuBuilder, StringSelectMenuOptionBuilder, SuccessButtonBuilder, TextDisplayBuilder, TextInputBuilder, ThumbnailBuilder, UserContextCommandBuilder, UserSelectMenuBuilder, ValidationError.

Functions: `createComponentBuilder`, `resolveAccessoryComponent`, `normalizeArray`, `resolveBuilder`, `enableValidators`, `disableValidators`, `isValidationEnabled`, and internal `validate`.

### 4.2 Methods by builder

**Embeds** (`messages/embed/`)
- EmbedBuilder: addFields, spliceFields, setFields, setAuthor, updateAuthor, clearAuthor, setColor, clearColor, setDescription, clearDescription, setFooter, updateFooter, clearFooter, setImage, clearImage, setThumbnail, clearThumbnail, setTimestamp (default now, `new Date(x).toISOString()`), clearTimestamp, setTitle, clearTitle, setURL, clearURL, toJSON.
- EmbedAuthorBuilder: setName, setURL, clearURL, setIconURL, clearIconURL.
- EmbedFieldBuilder: setName, setValue, setInline.
- EmbedFooterBuilder: setText, setIconURL, clearIconURL.

**Messages** (`messages/`)
- MessageBuilder: setContent, clearContent, setNonce, clearNonce, setTTS, addEmbeds, spliceEmbeds, setEmbeds, setAllowedMentions (default new builder), updateAllowedMentions, clearAllowedMentions, setMessageReference, updateMessageReference, clearMessageReference, add{ActionRow,Container,File,MediaGallery,Section,Separator,TextDisplay}Components, spliceComponents, setStickerIds, addStickerIds, spliceStickerIds, setAttachments, addAttachments, spliceAttachments, setFlags, clearFlags, setEnforceNonce, setPoll, updatePoll, clearPoll, setSharedClientTheme, updateSharedClientTheme, clearSharedClientTheme, toJSON, toFileBody.
- AttachmentBuilder: setId (Snowflake or number), setDescription, clearDescription, setDuration, clearDuration, setFilename, clearFilename, setFileData, clearFileData, setFileContentType, clearFileContentType, getRawFile, setTitle, clearTitle, setWaveform, clearWaveform. There is no setter for `is_spoiler`, though the API has it.
- AllowedMentionsBuilder: setParse, clearParse, setRoles, addRoles, spliceRoles, clearRoles, setUsers, addUsers, spliceUsers, clearUsers, setRepliedUser.
- MessageReferenceBuilder: setType, clearType, setMessageId, setChannelId, clearChannelId, setGuildId, clearGuildId, setFailIfNotExists.
- SharedClientThemeBuilder: setColors, setGradientAngle, setBaseMix, setBaseTheme, clearBaseTheme.

**Polls** (`messages/poll/`)
- PollBuilder: addAnswers, spliceAnswers, setAnswers, setQuestion, updateQuestion, setLayoutType, clearLayoutType, setMultiSelect, setDuration, clearDuration.
- PollAnswerBuilder: setMedia, updateMedia. PollMediaBuilder: setText. PollAnswerMediaBuilder: setEmoji, clearEmoji. PollQuestionBuilder: inherits media.

**Components** (`components/`)
- ComponentBuilder: setId, clearId, toJSON.
- ActionRowBuilder: add{Primary,Secondary,Success,Danger,Premium,Link}ButtonComponents, addComponents, add{Channel,Mentionable,Role,String,User}SelectMenuComponent, addTextInputComponent, spliceComponents.
- Buttons: setDisabled; the mixin adds setEmoji, clearEmoji, setLabel, clearLabel (not on Premium); CustomIdButtonBuilder setCustomId; LinkButtonBuilder setURL; PremiumButtonBuilder setSKUId.
- BaseSelectMenuBuilder: setPlaceholder, clearPlaceholder, setMinValues, setMaxValues, setCustomId, setDisabled, setRequired.
- StringSelectMenuBuilder: addOptions, setOptions, spliceOptions. Option builder: setLabel, setValue, setDescription, clearDescription, setDefault, setEmoji, clearEmoji.
- ChannelSelectMenuBuilder: addChannelTypes, setChannelTypes, addDefaultChannels, setDefaultChannels.
- MentionableSelectMenuBuilder: addDefaultRoles, addDefaultUsers, addDefaultValues, setDefaultValues.
- UserSelectMenuBuilder: addDefaultUsers, setDefaultUsers. RoleSelectMenuBuilder: analogous for roles.

**Modals** (`interactions/modals/`, `components/label/`, `components/textInput/`, `components/fileUpload/`)
- ModalBuilder: setTitle, setCustomId, addLabelComponents, addTextDisplayComponents, spliceComponents.
- LabelBuilder: setLabel, setDescription, clearDescription, set{StringSelectMenu,UserSelectMenu,RoleSelectMenu,MentionableSelectMenu,ChannelSelectMenu,TextInput,FileUpload}Component.
- TextInputBuilder: setCustomId, setStyle, setMinLength, clearMinLength, setMaxLength, clearMaxLength, setPlaceholder, clearPlaceholder, setValue, clearValue, setRequired. There is no setLabel: labels moved to LabelBuilder.
- FileUploadBuilder: setCustomId, setMinValues, clearMinValues, setMaxValues, clearMaxValues, addFileTypes, setFileTypes, clearFileTypes, setRequired.

**Components V2** (`components/v2/`)
- ContainerBuilder: setAccentColor, clearAccentColor, setSpoiler, add{ActionRow,File,MediaGallery,Section,Separator,TextDisplay}Components, spliceComponents.
- SectionBuilder: addTextDisplayComponents, set{Primary,Secondary,Success,Danger,Premium,Link}ButtonAccessory, setThumbnailAccessory, spliceTextDisplayComponents.
- ThumbnailBuilder: setDescription, clearDescription, setSpoiler, setURL.
- FileBuilder: setSpoiler, setURL.
- MediaGalleryBuilder: addItems, spliceItems. MediaGalleryItemBuilder: setURL, setDescription, clearDescription, setSpoiler.
- SeparatorBuilder: setDivider, setSpacing, clearSpacing.
- TextDisplayBuilder: setContent.

**Commands** (`interactions/commands/`)
- CommandBuilder: setContexts, setIntegrationTypes, setDefaultMemberPermissions (Permissions, bigint or number, converted to string), clearDefaultMemberPermissions, setNSFW.
- SharedName: setName, setNameLocalization, clearNameLocalization, setNameLocalizations, clearNameLocalizations. SharedNameAndDescription adds the same for description.
- SharedChatInputCommandOptions: add{Boolean,User,Channel,Role,Attachment,Mentionable,String,Integer,Number}Options, spliceOptions. Accepts a builder or a closure, not raw data.
- SharedChatInputCommandSubcommands: addSubcommandGroups, addSubcommands.
- ApplicationCommandOptionBase: setRequired.
- Mixins: setMaxValue, clearMaxValue, setMinValue, clearMinValue; addChannelTypes, setChannelTypes; setAutocomplete; addChoices, setChoices.
- String option: setMaxLength, clearMaxLength, setMinLength, clearMinLength. Attachment option: addFileTypes, setFileTypes, clearFileTypes.
- Context menu: UserContextCommandBuilder, MessageContextCommandBuilder.

### 4.3 Validation rules

Unless noted, lengths are Unicode code points [probed].

**Shared (`Assertions.ts`)**
- `id`: integer 0..2147483647, optional.
- `custom_id`: string 1..100.
- snowflake: regex `^(?:0|[1-9]\d*)$`.
- file upload types: union of enum audio/image/video or string with min 2 chars starting `.`, array max 10.
- member permissions: `z.coerce.bigint`.
- locale map: strict object over all Locale values, each an optional string.

**Components (`components/Assertions.ts`)**
- Emoji: strict `{ id? snowflake, name? 1..32, animated? }`, refine id or name present.
- Button base: strict `{ type, disabled? }`, **with no `id` key**.
  - Custom-id arms: `custom_id`, `emoji?`, `label?` 1..80, refine emoji or label.
  - Link: `url` protocol `http`, `https` or `discord`, max 512, same refine.
  - Premium: `sku_id` snowflake.
  - Discriminated by `style`.
- Select base: `{ id, placeholder? max 150, min_values? 0..25, max_values? 1..25, custom_id, disabled? }`. `min_values` is `z.number` with no integer check.
- `channel_types`: enum array. `default_values`: max 25, id plus type; mentionable allows role or user.
- String select option: `label` 1..100, `value` 1..100, `description?` 1..100, `emoji?`, `default?`.
- String select: options 1..25, plus check `options.length >= min_values` and `min_values <= max_values`.
- Action row: `id`, `type`, components either buttons 1..5 (by type only) or exactly 1 of channel, mentionable, string, role, text input, user select.

**File upload:** `id`, `custom_id`, `min_values` integer 0..10, `max_values` integer 1..10, `file_types`, `required`.

**Label:** `id`, `label` 1..45, `description?` 1..100, component union of string select, text input, user, role, mentionable, channel, file upload.

**Text input:** `id`, `custom_id`, `style` enum, `min_length?` 0..4000, `max_length?` 1..4000, `placeholder?` max 100, `value?` 0..4000, `required?`.

**Components V2 (`components/v2/Assertions.ts`)**
- Unfurled media URL: protocol `http`, `https` or `attachment`.
- Thumbnail: `media`, `description` nullish 1..1024, `spoiler`.
- File: URL `attachment` only.
- Separator: `divider?`, `spacing?` enum.
- Text display: `content` 1..4000.
- Media gallery item: thumbnail shape plus id. Gallery: items 1..10.
- Section: components 1..3 text displays, accessory is button (deep) or thumbnail.
- Container: components min 1 of action row (shallow), file, gallery, section, separator, text display. `spoiler`. `accent_color` integer 0..0xFFFFFF, nullish.

**Chat input commands (`interactions/commands/chatInput/Assertions.ts`)**
- Name: 1..32, regex (needs the `u` flag):
  ```
  ^[\p{Ll}\p{Lm}\p{Lo}\p{N}\p{sc=Devanagari}\p{sc=Thai}_-]+$
  ```
- Description 1..100. Localizations through the locale map.
- Number options: `float32` min and max. Integer options: `int` min and max.
- `channel_types` literal of 12: GuildText, GuildVoice, GuildCategory, GuildAnnouncement, AnnouncementThread, PublicThread, PrivateThread, GuildStageVoice, GuildForum, GuildMedia, DM, GroupDM (all but GuildDirectory).
- Autocomplete: `true` with choices of length 0 or absent, or `false`/absent with choices max 25.
- Choice: name 1..100, string value 1..100, number value any.
- String option: `max_length` 1..6000, `min_length` 0..6000. Attachment: `file_types`.
- Subcommand: options max 25, optional. Group: options 1..25, subcommands only.
- Command: options are either basic options max 25, or subcommands and groups max 25, which prevents mixing. Also `contexts`, `integration_types` enum arrays, `default_member_permissions`, `nsfw`, `type` literal ChatInput optional.
- **Not checked upstream:** required options before optional ones, duplicate option names. Discord enforces these server-side. See section 10.

**Context menu:** name 1..32, not whitespace-only (after trim). No description. `type` literal User or Message.

**Modals:** `title` 1..45, `custom_id`, components 1..5 of action row with exactly 1 text input by type, label (deep), or text display.

**Messages (`messages/Assertions.ts`)**
- File key regex: `^files\[(?<placeholder>\d+?)]$`.
- Raw file: `data` Buffer, Uint8Array or string, `name` min 1, `contentType?`, `key?` regex.
- Attachment: `id` snowflake or number, `description?` max 1024, `duration_secs?` max 2^31-1, `filename?` max 1024, `title?` max 1024, `waveform?` max 400.
- Allowed mentions: `parse` enum array, `roles` and `users` string arrays max 100, `replied_user`. Refine: `parse` containing `users` excludes a non-empty `users` list; same for roles.
- Message reference: `channel_id?`, `fail_if_not_exists?`, `guild_id?`, `message_id` required, `type?` enum.
- Base: `nonce?` string max 25 or number, `tts?`, `allowed_mentions?`, `message_reference?`, `attachments?` max 10, `enforce_nonce?`.
- Shared client theme: `colors` 1..5 of 6-hex (case-insensitive), `gradient_angle` integer 0..360, `base_mix` integer 0..100, `base_theme` enum nullish.
- Non-V2 message:
  - `content?` max 2000, `embeds` max 10, `sticker_ids` max 3, `poll`, theme, components (action rows, shallow, type-only, no text input) max 5.
  - Flags refine: must not include IsComponentsV2.
  - Refine, at least one of: `content !== undefined`, non-empty embeds, poll, non-empty attachments, non-empty components, non-empty stickers, theme. Empty string content passes.
- V2 message:
  - components 1..10, by type, of action row, Container, File, MediaGallery, Section, Separator, TextDisplay, Thumbnail.
  - Flags must include IsComponentsV2.
  - `content` length 0 nullish, `embeds` empty nullish, stickers empty nullish, `poll` null optional, theme null optional.
- `messagePredicate`: union of the two.
- File body: `{ body, files: rawFile[] max 10 }`, no minimum, so edits work. A comment says it does not assert key and id coherence.

**Embeds (`messages/embed/Assertions.ts`)**
- Field `name` max 256 and `value` max 1024, **no minimum**, so empty strings pass here, even though the API rejects them. `inline?`.
- URL protocols: `http`/`https`; icon and image also accept `attachment`.
- Author: `name` 1..256, `icon_url`, `url`.
- Footer: `text` 1..2048, `icon_url`.
- Embed: `title` 1..256 optional, `description` 1..4096 optional, `url`, `timestamp` string optional, `color` integer 0..0xFFFFFF, footer, `image{url}`, `thumbnail{url}`, author, fields max 25.
- Refine: at least one of title, description, non-empty fields, footer, author, image, thumbnail. So `url`, `color` or `timestamp` alone is invalid.
- Refine: `embedLength <= 6000`, in UTF-16 units.

**Polls (`messages/poll/Assertions.ts`):** question text 1..300; answer text 1..55; answer emoji through the emoji predicate; answers 1..10; `duration` number 1..768; `allow_multiselect?`; `layout_type` enum.

### 4.4 Validator mechanics

`validate(validator, value, override)`:
- `override === false`: skip.
- `override === undefined` and global disabled: skip.
- Otherwise `safeParse`, and throw `ValidationError(prettifyError, cause)` on failure.

Global flag defaults to true. Every `toJSON` takes an optional override.

**`toJSON` returns the cloned raw data, not zod's output.** So unknown keys passed through constructors survive, and coercions are discarded [probed].

Rust:
- `AtomicBool` for the global flag.
- `to_json(&self, validate: Option<bool>) -> Result<T, ValidationError>`.
- `ValidationError` holds `Vec<Issue { path, code, message }>`.
- Validation should be hand-written functions, not a zod clone.

Zod semantics observed [probed]:

| Input | Result |
|---|---|
| Strict button with `id: 1` | fails (`unrecognized_keys`) |
| Strict button with `id: undefined` (after `clearId()`) | fails |
| Emoji `id: null` | fails, though `APIPartialEmoji` allows null |
| `z.number` `min_values: 1.5` | passes; NaN and Infinity fail |
| `z.int` 2^53 | fails (safe-int range) |
| `float32` 1e39, Infinity | fails |
| `'😀'` against `max(1)` | passes (code points) |
| `attachment://a.png` | ok |
| `attachment://a b.png` | fails |
| `https://a`, `HTTP://A.com`, `https:/x`, `attachment:foo`, `discord://-/channels/1/2` | ok |
| ` https://x.y` (leading space) | ok |
| `ftp://a`, `not a url` | fails |
| `coerce.bigint` `'8'`, `8`, `undefined`, `''` (→0), `'0x10'` (→16), `' 5 '` (→5), `'-1'` (→-1) | ok |
| `coerce.bigint` `'abc'`, `8.5`, `null` | fails |
| Locale map with `null` value or unknown locale | fails; `undefined` value ok |
| V2 message `content: ''`, `embeds: []` | ok; `content: 'x'` fails |
| Autocomplete `true` with 1 choice | fails; absent with choices ok; `true` with `[]` ok |

The URL checks use a WHATWG parser. The Rust `url` crate is the closest analogue, and I have not differential-tested it. Port the table above as test vectors first.

### 4.5 Upstream defects and gaps: do not copy blindly

1. **Button `id` always fails validation** [probed against a transcribed predicate; source read at `packages/builders/src/components/Assertions.ts:15`]. `buttonPredicateBase` is a `strictObject` without `id`, yet `ComponentBuilder.setId` is available and the API accepts `id`. Even `clearId()` fails, because the key is present with value undefined. Rust should allow `id` on buttons and document the deviation.
2. **Action rows in containers are not deep-validated** [read at `packages/builders/src/components/v2/Container.ts` `toJSON` and `components/Assertions.ts:159`]. `ContainerBuilder.toJSON` calls `component.toJSON(false)` and `containerPredicate` uses the type-only `actionRowPredicate`. A select with an invalid `custom_id` inside a container's action row passes. Rust should validate deep everywhere.
3. **No builders for RadioGroup, CheckboxGroup, Checkbox** (21, 22, 23), though `APIComponentInLabel` and modal-submit types include them. `createComponentBuilder` throws on these, and LabelBuilder has no setter. Rust should add them, or leave a documented gap with raw-payload escape.
4. **No `is_spoiler` setter on AttachmentBuilder.** Add it.
5. **Embed field name and value have no minimum**, though the API requires non-empty. Decide whether to tighten (section 10).
6. **`coerce.bigint` is sloppy** on default member permissions. `''` becomes 0, which means "admin only" in Discord's semantics. A typed `Permissions` argument in Rust removes this entire class.

### 4.6 Rust shape

- **Mixins** (`ts-mixer`) become traits with default methods over an accessor, or macros. Macros give an inherent builder API with no `use` line, and I would start there.
- **`resolveBuilder`** accepts an instance (kept by reference), a function (called with a fresh builder), or raw data (`new Ctor(data)`). Rust: `IntoBuilder<B, Marker>` with marker types, implemented for `B`, `FnOnce(B) -> B`, and raw data types.
- **`RestOrArray`**: `normalizeArray` flattens only if `arr[0]` is an array. Rust: `impl IntoIterator<Item = impl Into<T>>`.
- **Button family**: one `ButtonBuilder` type per style, or a typestate. Keep the separate names for parity.
- **Option kinds:** a typed enum of basic versus subcommand options makes mixing a compile error, replacing the union predicate.
- **`splice`**: needs a `js_splice` helper, because `Vec::splice` panics out of range where JS clamps. A negative start counts from the end.

## 5. Formatters (`packages/formatters/src`, v0.6.1)

### 5.1 Output table

| Function | Output |
|---|---|
| `codeBlock(c)` | ` ```\n{c}\n``` ` |
| `codeBlock(lang, c)` | ` ```{lang}\n{c}\n``` ` |
| `inlineCode(c)` | `` `c` `` |
| `italic(c)` | `_c_` |
| `bold(c)` | `**c**` |
| `underline(c)` | `__c__` |
| `strikethrough(c)` | `~~c~~` |
| `spoiler(c)` | `\|\|c\|\|` |
| `quote(c)` | `> c` |
| `blockQuote(c)` | `>>> c` |
| `hideLinkEmbed(u)` | `<u>` |
| `hyperlink(c, u)` | `[c](u)` |
| `hyperlink(c, u, t)` | `[c](u "t")`; empty title behaves as none |
| `userMention(id)` | `<@id>` |
| `channelMention(id)` | `<#id>` |
| `roleMention(id)` | `<@&id>` |
| `linkedRoleMention(id)` | `<id:linked-roles:id>` |
| `gameProfileMention(id)` | `<@$id>` |
| `chatInputApplicationCommandMention(cid, name)` | `</name:cid>` |
| `(cid, name, sub)` | `</name sub:cid>` |
| `(cid, name, sub, group)` | `</name group sub:cid>` |
| group without sub | falls to `</name:cid>` |
| `formatEmoji(id, animated?)` or `{id, name?, animated?}` | `<a?:{name ?? 'emoji'}:{id}>` |
| `channelLink(c, g?)` | `https://discord.com/channels/{g ?? '@me'}/{c}` |
| `messageLink(c, m, g?)` | `https://discord.com/channels/{g ?? '@me'}/{c}/{m}` |
| `heading(c, level?)` | default `# c`; 2 `## c`; 3 `### c` |
| `orderedList(items, start?)` | every item uses the same number: `1. a\n1. b\n  1. c`; start is `max(start, 1)` |
| `unorderedList(items)` | `- a`; indent `'  '.repeat(depth - 1)` |
| `subtext(c)` | `-# c` |
| `time(date?)` | `<t:{floor(ms / 1000)}>`; absent means now |
| `time(seconds, style?)` | `<t:{seconds}:{style}>`; a number is used as is, no flooring |
| `applicationDirectory(app, sku?)` | `https://discord.com/application-directory/{app}/store[/{sku}]` |
| `email(addr, headers?)` | `<addr>` or `<addr?a=1&b=2>` |
| `phoneNumber(n)` | `<n>`; panics or errors unless it starts with `+`: `Phone number must start with a "+" sign.` |

Enums and consts:
- `HeadingLevel`.
- `Faces`: Shrug `¯\_(ツ)_/¯`, Tableflip `(╯°□°)╯︵ ┻━┻`, Unflip `┬─┬ノ( º _ ºノ)`.
- `GuildNavigationMentions`: `<id:browse>`, `<id:customize>`, `<id:guide>`, `<id:linked-roles>`.
- `TimestampStyles`: t, T, d, D, f, F, s, S, R.
- Types: `FormatEmojiOptions`, `RecursiveArray`, `TimestampStylesString`.

`email` headers: truthy headers have keys lowercased, then `new URLSearchParams(obj)` form-encoding, so spaces become `+`, array values are comma-joined and encoded as `%2C`. An empty object yields `<addr?>`. Test vector: `<test@example.com?subject=Hello&body=World>`.

### 5.2 Escapers (`packages/formatters/src/escapers.ts`)

Exports: `escapeMarkdown` with `EscapeMarkdownOptions` (blockQuote, bold, bulletedList, codeBlock, codeBlockContent, escape, heading, inlineCode, inlineCodeContent, italic, maskedLink, numberedList, quote, spoiler, strikethrough, underline, all defaulting to true), plus escapeCodeBlock, escapeInlineCode, escapeItalic, escapeBold, escapeUnderline, escapeStrikethrough, escapeSpoiler, escapeEscape, escapeHeading, escapeBulletedList, escapeNumberedList, escapeMaskedLink, escapeQuote, escapeBlockQuote.

**Order matters.** `escapeMarkdown` applies: escape, inlineCode, codeBlock, italic, bold, underline, strikethrough, spoiler, heading, bulletedList, numberedList, maskedLink, quote, blockQuote.

Content protection:
- `codeBlockContent: false`: split by ` ``` `. An odd-index segment that is not last is returned raw. Others recurse with options, without `codeBlock` and `codeBlockContent`. Rejoin with ` \`\`\` ` when `codeBlock` is true, plain ` ``` ` otherwise.
- `inlineCodeContent: false`: split by ``(?<=^|[^`])`(?=[^`]|$)`` and handle in the same way.

Regexes:

```
inlineCode     (?<=^|[^`])``?(?=[^`]|$)
italic *       (?<=^|[^*])(?<!(?<!<)https?:\/\/\S*|<[^\s:]+:\/[^\s>]*)\*([^*]|\*\*|$)
italic _       (?<=^|[^_])(?<!<a?:.+|(?<!<)https?:\/\/\S*|<[^\s:]:\/[^\s>]*)_(?!:\d+>)([^_]|__|$)
bold           \*\*(\*)?
underline      (?<!<a?:.+|https?:\/\/\S+)__(_)?(?!:\d+>)
strike         ~~
spoiler        \|\|
escape         \\
heading (gm)   ^( {0,2})([*-] )?( *)(#{1,3} )       -> $1$2$3\$4
bulleted (gm)  ^( *)([*-])( +)
numbered (gm)  ^( *\d+)\.
maskedLink(gm) \[.+]\(.+\)                          -> \$&
quote (gm)     ^(\s*)>(\s+)
blockQuote(gm) ^(\s*)>>>(\s+)
```

Stateful callbacks:
- Italic `*`: when the capture is `**`, a counter toggles the side the backslash goes on: odd `\***`, even `**\*`.
- Italic `_`: the same with `__`, and the counter is reset between the two passes.
- Bold and underline: the same toggle on the optional third char.
- Counters are per call.
- The `$` alternative in `([^*]|\*\*|$)` captures empty at the end of input, and the callback handles that.
- `\s` in `^(\s*)` spans newlines.

Rust regex differences [probed]:

| Construct | JavaScript | Rust `regex` / `fancy-regex` default |
|---|---|---|
| `\d` | ASCII | Unicode (matches Arabic-Indic digits) |
| `\w` | ASCII | Unicode (matches `é`) |
| `\s` | includes U+FEFF | excludes U+FEFF |
| `.` | excludes `\r`, `\n`, U+2028, U+2029 | matches `\r`; excludes only `\n` |
| multiline `^`/`$` | also break on `\r`, U+2028, U+2029 | break on `\n` only |

Fixes: use `[0-9]` or `(?-u:\d)`, `[A-Za-z0-9_]`, an explicit class for `\s` where the difference could surface, `[^\r\n\u{2028}\u{2029}]` for `.`, and a custom multiline form for `^` where `\r` matters.

`regex` rejects all look-around [probed]. `fancy-regex` 0.20.0 compiles all 4 lookbehind patterns [probed]. The italic and underline logic passed 23 of 23 vectors copied from `packages/formatters/__tests__/escapers.test.ts` [probed], even with default classes, so those vectors do not exercise the class differences above. Add dedicated vectors for them.

Recommendation: use `fancy-regex` only in the escaper module. Compile through `LazyLock`. Fancy-regex's backtracking has a step limit that returns an error rather than a panic, so decide a policy for that case: on error, return the input with a less precise escape, never panic.

## 6. Collection (`packages/collection/src/collection.ts`, v2.1.1)

`Collection<K, V>` extends `Map`. Rust: wrap `indexmap::IndexMap<K, V>`, always `shift_remove` to keep order (`swap_remove` breaks it). Delete is O(n), a real trade-off versus JS's Map. Types: `ReadonlyCollection`, `Keep`, `Comparator`.

Inherited Map API: size, get, set, has, delete, clear, keys, values, entries, forEach, iterator.

| Method | Semantics to preserve |
|---|---|
| `ensure(key, gen)` | If `has`, `get`; else `gen(key, coll)`, then `set`. |
| `hasAll(...keys)` | Empty is true. |
| `hasAny(...keys)` | Empty is false. |
| `first()` / `first(n)` | No arg: first or undefined. `n < 0`: `last(-n)`. `n >= size`: all. `0`: empty. |
| `firstKey()` / `firstKey(n)` | The same on keys. |
| `last()` / `last(n)` | `0`: empty. `n < 0`: `first(-n)`. Else `slice(-n)`. |
| `lastKey()` / `lastKey(n)` | The same on keys. |
| `at(i)` / `keyAt(i)` | Truncates. Negative from end. Out of range: undefined. |
| `random()` / `randomKey()` | `at(floor(rand * size))`. |
| `random(n)` / `randomKey(n)` | `min(size, n)`. `!n` gives empty. Partial Fisher-Yates. Negative `n` gives all but the last `|n|` unshuffled in JS; Rust takes `usize`, removing the quirk. |
| `reverse()` | In place via clear and re-set. Returns self. |
| `find(fn)` / `findKey(fn)` | First match in order. |
| `findLast(fn)` / `findLastKey(fn)` | Iterates a snapshot in reverse. |
| `sweep(fn)` | Deletes during iteration. Returns removed count. |
| `filter(fn)` | New collection, order kept. |
| `partition(fn)` | Two new collections, `[passed, failed]`. |
| `flatMap(fn)` | Map then `concat`. Later value overrides; first-seen position kept. |
| `map(fn)` | Array, using initial size. |
| `mapValues(fn)` | New collection, keys kept. |
| `some(fn)`, `every(fn)` | Standard. |
| `reduce(fn, init?)` | `init === undefined` treated as absent: first value seeds. Empty without init: error `Reduce of empty collection with no initial value`. |
| `reduceRight(fn, init?)` | The same, from the last. |
| `each(fn)` | Returns self. |
| `tap(fn)` | `fn(coll)`, returns self. |
| `clone()` | Shallow. |
| `concat(...colls)` | New. Later wins; position retained. |
| `equals(other)` | Falsy arg false. Identity shortcut. Size check. `!==` per value, handling undefined-with-missing-key. |
| `sort(cmp?)` | In place. Stable. Comparator is `(v1, v2, k1, k2)`. Default: undefined last, `String(x)` UTF-16 lexicographic, so numbers compare as strings. |
| `toSorted(cmp?)`, `toReversed()` | Copies. |
| `intersection(other)` | Self's values and order. |
| `union(other)` | Self then other's new keys; self's value wins. |
| `difference(other)` | Self minus other, one-directional. |
| `symmetricDifference(other)` | Self-only then other-only. |
| `merge(other, whenInSelf, whenInOther, whenInBoth)` | Each returns `Keep { keep, value }`. Key order is self then other. |
| `toJSON()` | Array of `[k, v]` entries. Serialize as a sequence of pairs, not as a map. |
| static `combineEntries(entries, combine)` | `combine(first, second, key)` on collision. |
| static `groupBy(items, keySelector)` | `keySelector(item, index)`. |

Rust notes:
- Overloads split: `first()` and `first_n(i)`, `last()` and `last_n(i)`, `random()` and `random_n(n)`, and so on.
- `at` takes `isize`.
- `equals`: Rust `PartialEq` is structural, while JS is identity. For `Arc<V>` values, offer `ptr_eq`.
- `Reduce` with an absent seed: `reduce(fn) -> Option<V>` and `fold(init, fn)`. Rust cannot have `undefined`-valued seeds, so the quirk disappears. Returning `Option` replaces the TypeError.
- `sort` default: requires `V: Display`, or drop the default and require a comparator. `String(x)` semantics on arbitrary values have no Rust equivalent. Provide `sort_by` and `sort_by_key`, and offer `sort_default` only for `V: ToString`, comparing UTF-16 units if parity matters.
- `thisArg` overloads become closures. Type-guard overloads and `Symbol.species` do not apply.
- `set` on an existing key keeps position; delete then set moves to end. `IndexMap::insert` matches.

## 7. Util (`packages/util/src`, v1.1.1)

| Item | Port |
|---|---|
| `JSONEncodable`, `isJSONEncodable` | Trait `JsonEncodable` or `Serialize`. |
| `FileBodyEncodable`, `FileBodyEncodableResult`, `isFileBodyEncodable` | Trait with body and files. |
| `RawFileEncodable`, `isRawFileEncodable` | Trait. |
| `Equatable`, `isEquatable` | `PartialEq`. |
| `RawFile { contentType?, data, key?, name }` | `data` becomes `Bytes`. Upstream allows `boolean` and `number` data, which Rust need not. |
| `Awaitable` | Not needed. |
| `calculateShardId(guildId, shardCount)` | `Number(BigInt(guildId) >> 22n) % shardCount`. Rust: `((id >> 22) % shard_count as u64) as u32`. |
| `embedLength(data)` | Sum of title, description, field names and values, `footer.text`, `author.name`. UTF-16 `.length`, no trimming. Other fields are excluded. |
| `lazy(cb)` | `called` is set before the callback runs, so a throwing callback later returns undefined instead of retrying. Rust: `OnceLock`/`LazyLock`, which differ on panic. Document. |
| `range(n)` or `range({start, end, step = 1})` | End-exclusive. `step <= 0` loops forever in JS. Rust uses `step_by(usize)`, so a zero step should be rejected. |
| `shouldUseGlobalFetchAndWebSocket`, `getUserAgentAppendix` | Runtime detection. Drop, or a static string with the `rustc` version. |
| `GatewayRateLimitError(data, payload)` | Message `Request with opcode ${opcode} was rate limited. Retry after ${retry_after} seconds.` Keep the fields. |

## 8. Recommendation: generate or hand-port

### 8.1 Decision

**Hybrid: one-shot mechanical generation from the TypeScript checker, hand-written core.** The generated code is committed and then owned like any other Rust. The generator stays in a `tools/` directory or is discarded, and can be re-run against a new discord-api-types release to produce a reviewable diff.

### 8.2 Evidence

Mechanical cleanliness [checker]: an object declaration counts as clean when it is non-generic and has no raw union, intersection or template field.

| Scope | Decls | Object | Union | Other alias | Enums | Generic | Clean objects | Clean % | Utility-resolved |
|---|---|---|---|---|---|---|---|---|---|
| payloads | 514 | 451 | 53 | 10 | 98 | 43 | 369 | 82% | 171 |
| rest | 430 | 317 | 32 | 81 | 6 | 0 | 289 | 91% | 188 |
| gateway | 218 | 204 | 12 | 2 | 7 | 4 | 183 | 90% | 125 |
| voice | 47 | 45 | 2 | 0 | 4 | 1 | 44 | 98% | 17 |
| rpc | 334 | 321 | 8 | 4 | 9 | 7 | 312 | 97% | 31 |

Non-RPC: 885 of 1017 object declarations clean (87%). The estimate is conservative, since many unclean fields only reference a named union that the generator can name.

The "Utility-resolved" column counts type aliases that resolve to an object type: 501 outside RPC. It includes plain renames as well as Pick, Omit, Partial and similar, so it is an upper bound on real utility types. The checker reduces all of them to plain properties, which is the most error-prone part of a hand port.

Rejected alternatives:
- **Pure hand port:** 1300+ non-RPC declarations and 115 enums. High risk of typos and drift, and low value in the typing itself.
- **Pure mechanical:** fails on integer-tagged unions, wrapper combinatorics, `number` ambiguity, conditional types, tri-state semantics.
- **Regex or text parsing of the `.ts`:** cannot resolve Pick, Omit and Required. The checker gave 0 diagnostics.

### 8.3 Split

**Generate:**
- All in-scope enums (115 without RPC and v4) through an `open_enum!` macro, keeping deprecated and unstable docs.
- 19 bit sets plus `PermissionFlagsBits`.
- `RESTJSONErrorCodes`, as a newtype with consts.
- Plain object structs, flattened by the checker.
- `Result` aliases as `pub type`.
- `Routes` and `CDNRoutes` function bodies (see section 9.4 on encoding).
- Doc comments, keeping `@see`, `@deprecated`, `@unstable`.
- `RouteBases`, `OAuth2Routes`, opcodes, close codes.

**Hand-write:**
1. `Snowflake` and `IdOrIndex`, `Nonce`, `SnowflakeOrTimestamp`.
2. `Permissions` string serde and bitflag helpers.
3. `Patch<T>` tri-state.
4. `open_enum!` and `tagged_union!` macros.
5. Component tree.
6. Channel union.
7. `Interaction` as a single struct with accessors.
8. Interaction option data, with autocomplete string values.
9. `GatewayEvent` two-phase decode with `Unknown`.
10. Audit log change.
11. `RESTErrorData`.
12. FormData and multipart bodies.
13. OAuth2 URL-encoded unions.
14. Voice binary payloads.
15. Role tags (null means true).
16. `referenced_message` tri-state.
17. `FormattingPatterns`, using `[0-9]` rather than `\d`.
18. `GatewayRequestGuildMembersData`.
19. Message search result, by HTTP status.
20. `APIApplicationCommandPermissionsConstant`.
21. Builders, Collection, formatters, util.

### 8.4 Pipeline

1. **Load:** `ts.createProgram` on `v10.ts` and `voice/v8.ts` with `strict`, `exactOptionalPropertyTypes`, `skipLibCheck`, `types: []`. Assert 0 diagnostics. This reads sources as data and does not execute them. `scratchpad/tools/scripts/coverage.cjs` and `enums.cjs` are working starting points.
2. **Extract** to an intermediate JSON:
   - Enums: `checker.getConstantValue` for resolved values, including bit shifts.
   - Objects: `getPropertiesOfType` with optionality, nullability, declaration text, JSDoc.
   - Unions: arms plus discriminant, found by searching for a property with a unit type in every arm.
3. **Apply an override table** (TOML or JSON, committed next to the generator):
   ```toml
   [numbers]                       # TS number -> Rust
   default = "i64"
   "*.retry_after" = "f64"
   "*.volume" = "f64"
   "APIGuild.afk_timeout" = "u32"

   [ids]                           # string fields -> Snowflake, or remain String
   not_snowflake = ["APIGuildWidgetMember.id", "APIIntegrationAccount.id", "GatewayActivity.id", "APIVoiceRegion.id"]

   [rename]
   strip_prefix = ["API", "RESTAPI"]

   [skip]                          # hand-written: reference it, do not emit it
   types = ["APIInteraction", "APIMessageComponent", "APIChannel"]

   [canonical]                     # merge identical property sets under one name
   "Partial<APIUser>" = "PartialUser"
   ```
4. **Fail closed.** The generator errors on any `number`, union, or intersection without an explicit mapping, rather than guessing. An unmapped item is a to-do list for review.
5. **Emit** one Rust module per source file. Run `rustfmt` and `cargo check`.
6. **Test** with wire fixtures: real captured payloads, round-tripped through deserialize and serialize. Combine with property checks that unknown keys and values do not fail parsing.
7. **Update:** re-run against a new release and review the diff. Since output is committed, later hand edits go through the override table or move the type to the hand-written list.

### 8.5 Scope

- In: payloads v10, gateway v10, rest v10, voice v8.
- Out for v1: rpc, voice v4, deno mirror, v6/8/9.
- Unstable members: include, marked in docs, behind no feature flag. They are real wire values, and `Unknown` covers the rest.

## 9. Subtle behaviour a port must preserve

### 9.1 String length: three different measures

| Rule | Measure | Rust |
|---|---|---|
| All zod `.min` and `.max` on strings | Unicode code points (`zod/v4/core/checks.js`: "Strings are measured in Unicode code points, not UTF-16 units") [probed] | `chars().count()` |
| `embedLength` total, 6000 | UTF-16 units, JS `.length` | `encode_utf16().count()` |
| Bytes | Never used for these limits | Never use `len()` |

Combining sequences count separately. Gateway `nonce` on request guild members: "up to 32 bytes", and an invalid one is ignored with the reply lacking a nonce. That one is genuinely bytes.

### 9.2 Gateway protocol

- **Heartbeat `d`** is `number | null`, and `null` must be serialized, not omitted.
- **`HeartbeatRequest`, `HeartbeatAck`, `Resumed`:** `d` is undefined or absent. `InvalidSession.d` is a boolean. Non-dispatch payloads have `t: null, s: null`.
- **`GUILD_DELETE`**: `unavailable: true` means an outage and not removal. A later `GUILD_CREATE` follows. Absent means the bot was removed.
- **`VOICE_SERVER_UPDATE.endpoint`** may be null: disconnect and wait for a new allocation, do not reconnect.
- **`APIThreadMember`:** `id` and `user_id` are omitted on `GUILD_CREATE` threads. `member` is present only with `with_member`.
- **`GatewayPresenceUpdate.user`:** only `id` is guaranteed.
- **`PresenceUpdateStatus`:** includes `invisible` on send. Received statuses exclude it, so a receive type must not assume otherwise.
- **`APIGuild.unavailable`** and `GUILD_CREATE`-only fields such as `threads`, `members`, `channels`: absent elsewhere.
- **Message content intent:** without `MESSAGE_CONTENT`, `content`, `embeds`, `attachments` and `components` arrive empty. Empty does not mean there is no content (`payloads/v10/message.ts`).
- **`GatewayMessageCreateDispatchData.mentions`** are `APIUserWithMember[]`. `channel_type` appears on message events.
- **`GUILD_MEMBERS_CHUNK`:** `not_found` is `unknown[]`, and `nonce` is echoed.
- **Identify:** `large_threshold` is 50..250, default 50. `shard` is a `[id, count]` tuple. `compress` defaults to false. Optional `capabilities`.
- **`GatewayRequestGuildMembersData`:** `user_ids` accepts one snowflake or an array. `query` empty with `limit: 0` means all members.
- **`RequestChannelInfo` (op 43)** takes open string `fields`.
- **Sequence:** update from the envelope before typed decode (section 3.9).
- **Close codes:** there is no 4006 and 4007/4009 say "start a new session" in their docs. Reconnect policy belongs to the ws design.

### 9.3 Rate limits

- REST 429 body: `RESTRateLimit { code?, global, message, retry_after }` (`payloads/common.ts:342`). `retry_after` is in seconds and can be fractional, so it needs `f64`.
- REST error codes tied to limits [checker]: 20016 slowmode (`ActionCannotBePerformedDueToSlowmodeRateLimit`), 20028 channel write (`ChannelWriteRateLimit`, alias `ChannelSendRateLimit`), 20029 server write (`ServerWriteRateLimit`, alias `ServerSendRateLimit`), 40062 (`ServiceResourceIsBeingRateLimited`).
- Gateway `RATE_LIMITED` dispatch: `{ opcode, retry_after (seconds), meta { guild_id, nonce? } }`, opcode 8 only (`GatewayRateLimitedDispatchData`).
- Gateway close 4008 `RateLimited`: send too fast and you are disconnected.
- `GET /gateway/bot`: `session_start_limit { total, remaining, reset_after (ms), max_concurrency }`.
- `rate_limit_per_user` also throttles thread creation. On thread objects it is returned only when non-zero.
- Search not-ready is a 202 with its own `retry_after` and code 110000, not a 429.
- `MessageBuilder` nonce with `enforce_nonce`: nonce is a string max 25 or a number.
- Bucket algorithm and headers are in `packages/rest`, covered by the REST design.

### 9.4 Routes

`Routes` and `CDNRoutes` are wrapped in a loop (`rest/v10/index.ts:1212` and about `:1544`):

```js
args.map(arg => arg
  ? (urlSafeCharacters.test(String(arg)) ? arg : encodeURIComponent(arg))
  : arg)
```

where `urlSafeCharacters` tests `^[\d%A-Za-z-_]+$` (`utils/internals.ts:43`).

Consequences:
- `%` counts as safe, so an already encoded emoji is not double-encoded. But the test is per-argument: if any other unsafe char is present, `encodeURIComponent` encodes the `%` too, giving `%25`.
- Falsy args pass through untouched: `0`, `''`.
- **Defaults bypass encoding.** The wrapper only sees passed args, and `userId = '@me'` defaults are applied inside the function. So `Routes.user()` gives `/users/@me`, while `Routes.user('@me')` gives `/users/%40me` [read, not run]. The same applies to `@original`. In Rust, model these as `UserTarget::Me` and emit a literal `@me`, without inheriting the encoded variant.
- `encodeURIComponent` leaves unreserved `A-Za-z0-9 - _ . ! ~ * ' ( )`. Rust: a `percent_encoding::AsciiSet` must match that exact set, not `NON_ALPHANUMERIC`.
- Reaction routes pass the emoji through this path.
- Optional-arg routes: `threads(parentId, messageId?)`, `threadMembers(threadId, userId?)`, `webhook(id, token?)`. Defaults: `guildMember` `@me`, `user` `@me`, `webhookMessage` `@original`, `guildVoiceState` `@me`.
- CDN: `storePageAsset` defaults to PNG. `defaultUserAvatar(index)` is always `.png`. `applicationCover` uses `/app-icons/`. Sticker format is GIF, Lottie or PNG. Most assets exclude Lottie, and guild splash and similar exclude GIF too. `ImageFormat.Lottie` serializes as `json`.
- 8 routes are `@deprecated`, such as channel pins and guild create. Keep them marked.

### 9.5 Builder serialization

- **`MessageBuilder.toJSON` always emits `attachments: []`, `embeds: []`, `components: []`.** Sent as a PATCH, those wipe existing content. Rust's builder should either keep parity with a documented warning or distinguish create from edit. I recommend distinguishing them.
- `EmbedBuilder.toJSON` always emits `fields: []`.
- Subcommand and group builders always emit `options: []`.
- `ChatInputCommandBuilder`: `options` is undefined if empty; `type: 1` forced.
- Button `type` and `style` forced by the constructor.
- Thumbnail and File constructors keep only `media.url` and `file.url`, since other fields are response-only ("ignored and provided by the API"). `MediaGalleryItem` constructor does not strip, keeping a structured clone of the whole. Port as is, or normalize; response-only fields are ignored either way.
- `createComponentBuilder`: `ComponentBuilder` instances pass through. It throws `Cannot properly serialize component type: ${type}` on unknown types and on unknown button style. It maps ActionRow, Button, StringSelect, TextInput, UserSelect, RoleSelect, MentionableSelect, ChannelSelect, Thumbnail, File, Separator, TextDisplay, MediaGallery, Section, Container, Label, FileUpload.
- Child validation: most parents call children with `toJSON(false)` and validate deep. `ActionRowBuilder`, `ModalBuilder` and `MessageBuilder` pass `validationOverride` down, since their predicates are shallow.
- `PollBuilder` always creates a question builder. `setPoll` accepts `APIPoll`.
- `clearNameLocalization` sets the key to undefined, so it is dropped from JSON. `null` cannot be set through validators, even though the type allows it.

**Files:**
- `toFileBody`: `body = toJSON(false)`, and `files` are attachments with `fileData`.
- `rawFile = { ...fileData, name: filename, key: id === undefined ? undefined : 'files[' + id + ']' }`.
- **`id: 0` is valid.** Zero-byte string data must be preserved (upstream test `AttachmentBuilder preserves zero-byte string files`). Check for `None`, never emptiness or falsiness.
- `getRawFile` returns undefined with no data.
- REST fallback key: `file.key ?? files[${index}]` (`packages/rest/src/lib/REST.ts:347`).
- `RESTAPIAttachment.id` must match `n` in `files[n]`. Nothing validates that coherence upstream.

### 9.6 API semantics encoded in the types

- **`referenced_message`:** absent is not fetched, `null` is deleted.
- **`APIMessageReference`:** `message_id` optional on receive, `channel_id` required. `RESTAPIMessageReference`: `message_id` required, others strict partial, `fail_if_not_exists` default true.
- **Edit message (`RESTPatchAPIChannelMessageJSONBody`):** content, embeds, flags, allowed_mentions, components are nullable. `flags` must include prior bits. `attachments` must list all retained plus new, so a missing attachment is deleted.
- **Audit log:** `new_value` absent while `old_value` present means reset to null. `target_id` is `string | null`.
- **`video_quality_mode`:** "1 when not present".
- **Search pagination:** do not rely on the length of `messages` to paginate.
- **`APIPollMedia.text`:** optional. `APIFileComponent.name`, `size`: response-only.
- **Interaction responses:** `DeferredMessageUpdate` and `UpdateMessage` are only valid for component and modal submit interactions. `LaunchActivity` requires Activities. Deferred response `data?` holds just `flags`. Action rows in modals are deprecated (`payloads/v10/_interactions/responses.ts:158`).
- **`APIPingInteraction`:** omits `locale`.
- **Role tags:** presence as `null` means true.
- **`EmbedType`:** includes `components`, and `APIEmbed.components` exists.
- **Emoji:** `APIEmoji.id` and `name` are nullable. `APIPartialEmoji.id` is `Snowflake | null`.
- **Application emoji:** fixed literal fields, ignored on parse.
- **Application flags:** `flags` and `flags_new` carry the same concept.
- **Snowflake:** `Everyone(guildId) = guildId`, `AllChannels(guildId) = guildId - 1`.
- **Shard:** `(guild_id >> 22) % shard_count`.
- **`CannotSendMessagesToThisUserErrorCodes`:** a const list of error codes for DM failures. Port it.

### 9.7 JS semantics that disappear or change

- **`splice`:** negative index from end, out-of-range clamps. `Vec::splice` panics.
- **`structuredClone`:** builders deep-copy on input and `toJSON`. Rust's ownership replaces it, but `Clone` on an inner `Arc` would share.
- **`resolveBuilder`** keeps instances by reference. In JS, a later mutation of the same instance shows through. Rust moves or clones, which is safer. Do not try to replicate it.
- **`new Date(x).toISOString()`** in `setTimestamp`: millisecond precision, always UTC with `Z`. Use `chrono` or `time` with the same format, and check the wire format in fixtures.
- **`reduce` with `undefined`**, `sort` by `String(x)`, `random(-n)`: see section 6.
- **Hash order:** Rust `HashMap` iteration differs from JS insertion order. `Collection` must use `IndexMap`. I did not audit payload map fields for order significance; `Record` is used 42 times, so check any that are displayed or round-tripped.
- **Number precision:** JS loses precision above 2^53, which is why ids are strings. Rust `u64` does not, so accept ints from ETF.

## 10. Open decisions

1. **Typed ids versus one `Snowflake`.**
   - Typed `Id<UserMarker>` catches swapped ids at compile time, and the field-name census shows the generator can infer most markers. Cost: `id` (244 uses) needs owner-based mapping, and generics spread through user code.
   - One newtype matches discord.js parity and is simpler.
   - Suggestion: typed markers, since the checker makes the inference cheap, and this is the kind of guarantee a Rust port should add. This belongs to the architecture decision.
2. **Open enum representation.** `enum` with `Unknown(T)` (suggested for matched enums) versus newtype with consts (suggested for error codes). Consider using one form everywhere for macro simplicity.
3. **Create versus edit messages.** Distinct types avoid the wipe-on-PATCH problem and depart from upstream's single `MessageBuilder`. Recommended.
4. **ETF.** JSON-only for v1 is simpler. Keep `Snowflake`'s integer tolerance so ETF can be added later.
5. **Validation deviations from upstream.** Recommended: allow button `id`; deep-validate container action rows; add components 21/22/23; add `is_spoiler`. To decide: tighten embed field minimum; add required-before-optional and duplicate-name checks on commands (Discord rejects these, so catching them locally helps, but it departs from upstream).
6. **Delete cost in `Collection`.** `shift_remove` is O(n). Large caches may want a different structure, with `Collection` kept as the API for smaller sets. This should be decided in the cache design.
7. **Tagged union decoding cost.** `Value` buffering is simple but allocates per node. Benchmark on message-heavy traffic before optimizing.

## 11. Verification status

**Run:**
- Census (TS AST) and coverage (TS checker): 0 diagnostics.
- Enum values and duplicate detection through `checker.getConstantValue`.
- Zod predicates: real zod 4.5.4 with my transcriptions, for all rows in the table in 4.4. Not the upstream predicate objects themselves.
- Rust: lookbehind compile tests, 23 of 23 escaper vectors for `escapeItalic` and `escapeUnderline` only, regex class differences, serde tag and `Option<Option>` behaviour, flatten with `u64`, integer map keys.

**Read, not run:**
- All method inventories, all validation rules other than those in 4.4's table, formatter outputs, Collection semantics, route encoding including the `%40me` quirk, serialization quirks.
- A few results were compared with upstream test expectations by reading them, not running them: Collection `first(-2)` and `last(-2)`, the `email` vector, zero-byte attachments.

**Not tested:**
- The remaining escapers on `fancy-regex`, including the JS line-terminator divergences.
- `url` crate parity with the WHATWG cases in section 4.4.
- `RawValue` under flatten or tagged serde.
- A full generator run. The probes prove extraction works, not the emitted Rust.
- Behaviour against live Discord. Everything here is from sources, not the wire. Statements about what the API accepts are from upstream docs in the types.

**Limits on the numbers:**
- Census excludes fields in `& { ... }` intersection arms.
- Clean percentages treat any field that references a raw union as unclean, so they are a lower bound.
- The 233 route count comes from documented method lines and could differ slightly from true endpoint count.

**Tools left in the scratchpad** (nothing was written to the target repo):
- `scratchpad/tools/scripts/census.cjs`, `coverage.cjs`, `enums.cjs`, `zodprobe.cjs`, `zodlen.cjs`
- `scratchpad/tools/regexprobe/` (Rust crate)
- `scratchpad/tools/census.json`, `coverage.json`, `enums.json`
