# REST layer map: discord.js to discord.rs

Sources read (read-only, nothing from them was executed):

- discord.js clone at commit `5bb74922` (2026-11-06), `@discordjs/rest` 2.5.0. Paths below are relative to its root.
- discord-api-types clone at commit `00cbe03b` (2026-11-06), version 0.38.57. Paths prefixed `dat/`.

Confidence markers:

- No marker: read directly in the cloned source.
- **[model]**: checked by running my own hand-written transcription of the logic (fake network), not the real library.
- **[memory]**: comes from a dependency that is not in the clone (no `node_modules`) or from general knowledge of Discord, undici or Rust crates. Verify before relying on it.

## 1. Package map

The task brief says "RequestManager". That class no longer exists. Its role is now the `REST` class, and the name survives only in the test file `RequestManager.test.ts`.

| File | Lines | Role |
|---|---:|---|
| `packages/rest/src/lib/REST.ts` | 480 | `REST`: event emitter, options, hash and handler maps, sweepers, route generalisation, request building |
| `packages/rest/src/lib/handlers/SequentialHandler.ts` | 440 | Per-bucket FIFO with rate limit, global and sublimit logic |
| `packages/rest/src/lib/handlers/BurstHandler.ts` | 157 | Unqueued handler used only for interaction callbacks |
| `packages/rest/src/lib/handlers/Shared.ts` | 210 | `makeNetworkRequest`, `handleErrors`, `incrementInvalidCount`, process-wide counters |
| `packages/rest/src/lib/interfaces/Handler.ts` | 27 | `IHandler { id, inactive, queueRequest }` |
| `packages/rest/src/lib/utils/utils.ts` | 241 | `makeURLSearchParams`, `parseResponse`, `hasSublimit`, `shouldRetry`, `onRateLimit`, normalisers, `calculateUserDefaultAvatarIndex` |
| `packages/rest/src/lib/utils/types.ts` | 433 | All option and data types |
| `packages/rest/src/lib/utils/constants.ts` | 69 | Defaults, events, CDN allow-lists, UUID namespace |
| `packages/rest/src/lib/CDN.ts` | 407 | CDN URL builder |
| `packages/rest/src/lib/errors/*.ts` | 192 | `DiscordAPIError`, `HTTPError`, `RateLimitError` |
| `packages/rest/src/strategies/undiciRequest.ts` | 121 | Default Node transport |
| `packages/rest/src/environment.ts`, `index.ts`, `web.ts`, `shared.ts` | 48 | Transport selection and exports |
| `packages/util/src/RawFile.ts` | 33 | `RawFile` type |
| `packages/util/src/functions/userAgentAppendix.ts` | 47 | Runtime appendix for the User-Agent |
| `packages/proxy/src/**` | 172 | HTTP forward proxy built on `REST` |
| `apps/proxy-container/src/index.ts` | 13 | Runnable proxy: `new REST({ rejectOnRateLimit: true, retries: 0 })` |
| `packages/core/src/api/*.ts` | 4839 | 18 typed endpoint classes (section 15) |
| `dat/rest/v10/index.ts` | 1635 | `Routes`, `CDNRoutes`, `RouteBases`, `OAuth2Routes` (section 16) |

Runtime dependencies of `rest` (`packages/rest/package.json`): `@discordjs/collection`, `@discordjs/util`, `@sapphire/async-queue`, `@sapphire/snowflake`, `@vladfrangu/async_event_emitter`, `discord-api-types`, `magic-bytes.js`, `undici`, `uuid`.

## 2. `REST` class (`packages/rest/src/lib/REST.ts`)

### 2.1 Options (`utils/types.ts:23-143`, defaults in `utils/constants.ts:17-39`)

| Option | Type | Default | Notes |
|---|---|---|---|
| `agent` | `Dispatcher \| null` | `null` | Transport agent; the proxy hook (section 12) |
| `api` | string | `https://discord.com/api` | Base without version |
| `authPrefix` | `'Bot' \| 'Bearer'` | `'Bot'` | |
| `cdn` | string | `https://cdn.discordapp.com` | |
| `mediaProxy` | string | `https://media.discordapp.net` | Only animated stickers use it |
| `globalRequestsPerSecond` | number | `50` | `Infinity` allowed |
| `handlerSweepInterval` | ms | `3_600_000` (1 h) | `0` or `Infinity` disables; over 4 h throws |
| `hashSweepInterval` | ms | `14_400_000` (4 h) | Same rules |
| `hashLifetime` | ms | `86_400_000` (24 h) | |
| `headers` | map | `{}` | Added to every request; beats per-request headers |
| `invalidRequestWarningInterval` | number | `0` | `0` means no warnings |
| `makeRequest` | fn | default strategy | Pluggable transport |
| `offset` | number or `fn(route)` | `50` | ms; clamped to `>= 0` |
| `rejectOnRateLimit` | bool or filter | `false` | Filter may be async. No array form in this version |
| `retries` | number | `3` | Compared with `!==` (see 14.21) |
| `retryBackoff` | number or `fn(route, status\|null, retryCount, body)` | `0` | Callback may return `null` to stop retrying |
| `timeout` | number or `fn(route, body)` | `15_000` | ms per attempt, clamped to `>= 0` |
| `userAgentAppendix` | string | `Node.js/<ver>` | |
| `version` | string | `'10'` | `APIVersion` from discord-api-types |

Constructor behaviour (`REST.ts:79-88`): builds `CDN` from raw options, merges with `{ ...Default, ...options }`, sets `globalRemaining = max(1, globalRequestsPerSecond)` and starts the sweepers.

### 2.2 Public surface

- Fields: `agent`, `cdn`, `globalRemaining`, `globalDelay`, `globalReset` (starts at `-1`), `hashes`, `handlers`, `options`. Private `#token`.
- Verbs: `get`, `delete`, `post`, `put`, `patch(fullRoute, RequestData)`, all calling `request`.
- `request(InternalRequest)` is `queueRequest` plus `parseResponse`.
- `queueRequest(InternalRequest)` returns the raw `ResponseLike`. The proxy package uses this.
- Setters: `setAgent`, `setToken`. Sweeper control: `clearHashSweeper`, `clearHandlerSweeper`.
- Exported helpers: `makeURLSearchParams`, `parseResponse`, `calculateUserDefaultAvatarIndex`, all constants, `version`.

### 2.3 Per-request options (`RequestData`, `types.ts:303-382`)

`appendToFormData`, `auth` (`AuthData | boolean`), `body`, `dispatcher`, `files`, `headers`, `passThroughBody`, `query` (`URLSearchParams`), `reason`, `rejectOnRateLimit`, `signal`, `versioned`. `InternalRequest` adds `fullRoute` and `method`. `RequestMethod` is DELETE, GET, PATCH, POST, PUT.

`HandlerRequestData` (`types.ts:414-416`) is `{ body, files, rejectOnRateLimit, signal, auth: boolean | string }`, where a string `auth` is the UUID of a custom token.

### 2.4 Events (`constants.ts:44-51`, `types.ts:8-15`)

| Event | Payload | Emitted from |
|---|---|---|
| `restDebug` | string | Sweepers and handlers. Handler lines are prefixed `[REST <hash>:<major>]` |
| `handlerSweep` | swept map | Every interval, even when empty |
| `hashSweep` | swept map | Every interval, even when empty |
| `invalidRequestWarning` | `{ count, remainingTime }` | `Shared.ts:40` |
| `rateLimited` | `RateLimitData` | Only the pre-emptive `while (limited)` loop, `SequentialHandler.ts:247` |
| `response` | `(APIRequest, ResponseLike)` | `Shared.ts:118`, for every attempt that got a response, only if a listener exists |

`RateLimitData` fields: `global`, `hash`, `limit`, `majorParameter`, `method`, `retryAfter`, `route`, `scope` (`user | global | shared`), `sublimitTimeout`, `timeToReset`, `url`.

### 2.5 Lifecycle of one call (`REST.ts:243-270`)

1. `generateRouteData(fullRoute, method)` gives `{ majorParameter, bucketRoute, original }`.
2. `customAuth = typeof auth === 'object' && auth.token !== #token`. `auth` becomes `uuidV5(token, 'acc82a4c-f887-417b-a69c-f74096ff7e59')` if custom, otherwise the boolean `request.auth !== false`.
3. Hash lookup with key `` `${method}:${bucketRoute}${customAuth ? `:${uuid}` : ''}` ``. On a miss the value is `` `Global(${that same key})` ``. The fallback is not stored in `hashes`.
4. Handler lookup with key `` `${hash}:${majorParameter}` ``, created on a miss. `majorParameter === 'burst'` gives a `BurstHandler`, anything else a `SequentialHandler`.
5. `resolveRequest` builds URL and fetch options. It throws `Expected token to be set for this request, but none was present` here, so the handler already exists by then.
6. `handler.queueRequest(routeId, url, fetchOptions, { body, files, auth, rejectOnRateLimit, signal })`.

### 2.6 Sweepers (`REST.ts:90-155`)

- Hash sweep: removes entries with `now - lastAccess > hashLifetime`. `lastAccess === -1` is exempt, though stored entries always have a real time because the `-1` fallback is never inserted.
- Handler sweep: removes handlers whose `inactive` getter is true. `BurstHandler.inactive` is a constant `false`, so burst handlers are never swept.
- Timers are `unref`'d so they do not hold the process open.

## 3. Route generalisation and bucket mapping

`generateRouteData` (`REST.ts:438-479`), in order:

1. If `endpoint` starts with `/interactions/` and ends with `/callback`, return major `burst` and bucket `/interactions/:id/:token/callback`.
2. Major parameter regex: `/(?:^\/webhooks\/(\d{17,19}\/[^/?]+))|(?:^\/(?:channels|guilds|webhooks)\/(\d{17,19}))/`. Result is `group2 ?? group1 ?? 'global'`.
3. Bucket route, applied in this order:
   - replace all `\d{17,19}` with `:id`
   - first `\/reactions\/(.*)` becomes `/reactions/:reaction`
   - first `\/webhooks\/:id\/[^/?]+` becomes `/webhooks/:id/:token`
4. If `DELETE` and bucket is exactly `/channels/:id/messages/:id`, take the trailing snowflake. If its age is over 14 days, append `/Delete Old Message`.

Test vectors for the port, produced by my transcription **[model]**:

| Method | Endpoint | Bucket route | Major |
|---|---|---|---|
| GET | `/channels/339942739275677727` | `/channels/:id` | `339942739275677727` |
| POST | `/channels/339942739275677727/messages` | `/channels/:id/messages` | `339942739275677727` |
| GET | `/channels/339942739275677727/messages/392063687801700356` | `/channels/:id/messages/:id` | `339942739275677727` |
| DELETE | `/channels/339942739275677727/messages/392063687801700356` (old) | `/channels/:id/messages/:id/Delete Old Message` | `339942739275677727` |
| DELETE | `/channels/339942739275677727/messages/<recent snowflake>` | `/channels/:id/messages/:id` | `339942739275677727` |
| PUT | `/channels/339942739275677727/messages/392063687801700356/reactions/%F0%9F%91%8D/@me` | `/channels/:id/messages/:id/reactions/:reaction` | `339942739275677727` |
| DELETE | `/channels/339942739275677727/messages/392063687801700356/reactions/name%3A180162837826289664/180162837826289664` | `/channels/:id/messages/:id/reactions/:reaction` | `339942739275677727` |
| GET | `/channels/339942739275677727/messages/392063687801700356/reactions/%F0%9F%91%8D` | `/channels/:id/messages/:id/reactions/:reaction` | `339942739275677727` |
| DELETE | `/channels/339942739275677727/messages/392063687801700356/reactions` | `/channels/:id/messages/:id/reactions` | `339942739275677727` |
| GET | `/guilds/222078108977594368/members/180162837826289664` | `/guilds/:id/members/:id` | `222078108977594368` |
| PATCH | `/guilds/222078108977594368/members/%40me` | `/guilds/:id/members/%40me` | `222078108977594368` |
| GET | `/guilds/templates/abcDEF` | `/guilds/templates/abcDEF` | `global` |
| PATCH | `/guilds/222078108977594368/templates/abcDEF` | `/guilds/:id/templates/abcDEF` | `222078108977594368` |
| GET | `/webhooks/1234567890123456789` | `/webhooks/:id` | `1234567890123456789` |
| POST | `/webhooks/1234567890123456789/tok-en_1` | `/webhooks/:id/:token` | `1234567890123456789/tok-en_1` |
| PATCH | `/webhooks/1234567890123456789/tok-en_1/messages/%40original` | `/webhooks/:id/:token/messages/%40original` | `1234567890123456789/tok-en_1` |
| DELETE | `/webhooks/1234567890123456789/tok-en_1/messages/392063687801700356` | `/webhooks/:id/:token/messages/:id` | `1234567890123456789/tok-en_1` |
| POST | `/webhooks/1234567890123456789/tok-en_1/slack` | `/webhooks/:id/:token/slack` | `1234567890123456789/tok-en_1` |
| POST | `/interactions/1234567890123456789/tok-en_1/callback` | `/interactions/:id/:token/callback` | `burst` |
| GET | `/gateway/bot` | `/gateway/bot` | `global` |
| GET | `/users/%40me` | `/users/%40me` | `global` |
| GET | `/users/180162837826289664` | `/users/:id` | `global` |
| GET | `/invites/abcDEF` | `/invites/abcDEF` | `global` |
| PUT | `/applications/1234567890123456789/guilds/222078108977594368/commands` | `/applications/:id/guilds/:id/commands` | `global` |
| GET | `/channels/not-a-snowflake` | `/channels/not-a-snowflake` | `global` |

Two-level mapping:

- `hashes`: `METHOD:bucketRoute[:authUuid]` maps to `{ value, lastAccess }`. No major parameter here.
- `handlers`: `<hash value>:<major>` maps to the handler. Routes that learn the same bucket hash share a handler, and so share limit state, for the same major parameter.

Learning a hash (`SequentialHandler.ts:302-327`), on any response that carries `X-RateLimit-Bucket`, including errors:

- `partitionedHash = hash + (customAuth ? ':' + uuid : '')`.
- If it differs from `this.hash`: emit debug and store it in `hashes` with `lastAccess = now`.
- If equal: refresh `lastAccess` on the existing entry.
- The current handler keeps running what it already has ("eliminated via attrition"). Its limit state is not copied to the new handler, which starts from `remaining = 1`, `reset = -1`.

## 4. Building the HTTP request (`REST.ts:296-415`)

**URL:** `` `${api}${versioned === false ? '' : `/v${version}`}${fullRoute}${query}` ``. `?` is added only if the query string is non-empty.

**Headers,** merged so that later entries win: `request.headers`, then `Content-Type: application/json` (JSON path only), then `options.headers`, then `User-Agent`, then `Authorization` and `X-Audit-Log-Reason` when set.

- User-Agent: `` `DiscordBot (https://discord.js.org, ${version}) ${appendix}`.trim() ``.
- Authorization when `auth !== false`: object form uses `` `${auth.prefix ?? options.authPrefix} ${auth.token}` ``, otherwise `` `${authPrefix} ${token}` ``, or the missing-token error.
- `X-Audit-Log-Reason`: `encodeURIComponent(reason)`, only for a non-empty reason. No length check.

**Body**, first match wins:

1. `files?.length` is non-zero: multipart form.
   - Key is `file.key ?? files[${index}]`.
   - Byte-like data with no `contentType`: sniff with `filetypeinfo(data)` and use the first match only. `image/apng` is rewritten to `image/png`. No match leaves the type empty.
   - Non-byte data (`string | number | boolean`) is stringified. Its type is `file.contentType`, with no sniffing.
   - Each file is appended as a Blob with `file.name` as the filename.
   - After the files, if `body != null`: `appendToFormData` appends each `Object.entries` pair as a text field, otherwise a `payload_json` text field holds `JSON.stringify(body)`.
   - No Content-Type is set by hand; the transport adds the boundary.
2. `body != null` and `passThroughBody`: body sent as is, no Content-Type added.
3. `body != null`: `JSON.stringify(body)` with `Content-Type: application/json`.
4. Otherwise no body and no Content-Type. `files: []` falls through here (test `postFile`, `REST.test.ts:361`).

**Method:** upper-cased. `GET` and `HEAD` force the body to `null`, even after the Content-Type was computed.

**Dispatcher:** `request.dispatcher ?? this.agent ?? undefined`.

File attachment wiring (`packages/util/src/RawFile.ts`): `data: Buffer | Uint8Array | boolean | number | string`, `name`, optional `contentType`, optional `key`. To use a placeholder id other than the array index, the caller must set `key: 'files[<id>]'` themselves.

Non-default keys used by core: `file` for stickers (`guild.ts:1007`) and `target_users_file` with `contentType: 'text/csv'` for invites (`channel.ts:507`, `invite.ts:76`). `appendToFormData` is used only by `createSticker` (`guild.ts:1011`).

## 5. `SequentialHandler` (`handlers/SequentialHandler.ts`)

State: `reset = -1`, `remaining = 1`, `limit = Infinity`, `#asyncQueue`, `#sublimitedQueue = null`, `#sublimitPromise = null`, `#shiftSublimit = false`.

Derived:

- `globalLimited`: `globalRemaining <= 0 && now < globalReset`
- `localLimited`: `remaining <= 0 && now < reset`
- `limited`: either of those
- `inactive`: both queues empty and not `limited`

One request at a time per handler: the next request starts only after the previous one's headers have arrived. Even with `remaining > 1`, requests in a bucket and major are never concurrent.

`runRequest(routeId, url, options, requestData, retries = 0)`:

1. `while (limited)`:
   - Global case: `timeout = globalReset + offset - now`. The first waiter creates the shared `globalDelay` promise, which clears itself when done.
   - Local case: `timeout = reset + offset - now`, with its own sleep.
   - Build `RateLimitData` with `retryAfter = timeToReset = timeout`, `sublimitTimeout = 0`, `scope = 'user'`.
   - Emit `rateLimited`, call `onRateLimit` (may throw), emit debug, then wait.
2. Global accounting: if `!globalReset || globalReset < now`, set `globalReset = now + 1000` and `globalRemaining = globalRequestsPerSecond`. Then `globalRemaining--`.
3. `makeNetworkRequest`. A `null` result means retry with `retries + 1`.
4. Parse rate limit headers:

| Header | Variable | Present | Absent |
|---|---|---|---|
| `X-RateLimit-Limit` | `limit` | `Number` | `Infinity` |
| `X-RateLimit-Remaining` | `remaining` | `Number` | `1` |
| `X-RateLimit-Reset-After` | `reset` | `sec * 1000 + now + offset` | `now` |
| `X-RateLimit-Bucket` | hash | see section 3 | no hash change |
| `Retry-After` | `retryAfter` | `sec * 1000 + offset` | `0` |
| `X-RateLimit-Scope` | `scope` | value | `'user'` |
| `X-RateLimit-Global` | | presence only | |

5. If `retryAfter > 0`: with `X-RateLimit-Global` present, set `globalRemaining = 0` and `globalReset = now + retryAfter`. Otherwise, if `!localLimited` (evaluated with the just-updated state), `sublimitTimeout = retryAfter`.
6. Status 401, 403 or 429: `incrementInvalidCount`.
7. Branch: `res.ok` returns the response. 429: the flow below, then re-enter with the same `retries`. Anything else: `handleErrors`, where `null` means retry with `retries + 1`.

429 branch detail (`SequentialHandler.ts:352-429`):

- `isGlobal = this.globalLimited`, derived from state and not from the header (see 14.11).
- `onRateLimit` is called with `retryAfter` and `sublimitTimeout ?? 0`. It may throw before any sublimit state is created.
- No `rateLimited` event is emitted here. If the request then waits in the `while (limited)` loop on re-entry, that loop emits it. A pure sublimit wait emits none.

## 6. `BurstHandler` (`handlers/BurstHandler.ts`)

- Selected only for interaction callbacks. One handler per auth partition, since the bucket route is a constant.
- No queue, no bucket state, no hash learning. It neither reads nor writes `globalRemaining` or `globalReset`.
- Reads only `Retry-After`, `X-RateLimit-Scope`, `X-RateLimit-Global`.
- Still counts invalid requests and uses the same retry and error paths.
- On 429: `onRateLimit` with `limit = Infinity`, `timeToReset = retryAfter`, `sublimitTimeout = 0` and `global` from the header. Then it sleeps `retryAfter` and retries with the same `retries`. Only that one request sleeps.
- `inactive` is always `false`.

Webhook follow-ups, `editReply` and similar go to `/webhooks/...` and use `SequentialHandler`. They also consume the local global budget, even though they use `auth: false`.

## 7. Network, retries, errors (`handlers/Shared.ts`)

`makeNetworkRequest`:

- A new `AbortController` per attempt, with a timer from `normalizeTimeout(timeout, bucketRoute, body)`.
- The user signal is not reused because it may be shared by several requests. If already aborted, abort at once. Otherwise add a listener, removed in `finally` (test `RequestHandler.test.ts:694`).
- The timer is cleared as soon as `makeRequest` resolves, at response headers. Reading the body has no timeout.
- On a thrown `Error`: if `shouldRetry(error) && retries !== options.retries`, compute the backoff. `null` rethrows, `> 0` sleeps, then return `null`. Otherwise rethrow. Non-`Error` throws are rethrown directly.
- `shouldRetry` (`utils.ts:140-145`): `error.name === 'AbortError'`, or `code === 'ECONNRESET'`, or the message contains `ECONNRESET`. DNS failures and connection refused are not retried.
- If a `response` listener exists, emit `{ method, path, route, options, data, retries }` and `res.clone?.() ?? { ...res }`.

`normalizeRetryBackoff` (`utils.ts:213-226`): number form is `max(0, n) * (1 << retryCount)`. Callback form is passed through without clamping; only `> 0` sleeps.

`handleErrors`:

- 5xx: if `retries !== options.retries`, apply backoff (`null` throws `HTTPError`) and return `null`. Otherwise throw `HTTPError(status, statusText, method, url, requestData)`.
- 4xx: `parseResponse`, `isDiscordError = 'code' in data`. If `status === 401 && requestData.auth === true`:
  - If `isDiscordError && code !== 0` and not yet warned: emit one process-wide warning.
  - `manager.setToken(null)`.
  - Then throw `DiscordAPIError(data, isDiscordError ? data.code : data.error, status, method, url, requestData)`.
- Other statuses (1xx, 3xx): return the response. `request()` then parses it as a success.

Invalid request tracking (`Shared.ts:19-45`):

- Module-level `invalidCount` and `invalidCountResetTime`, deliberately process-wide since the limit is per IP.
- Fixed 10 minute window that starts lazily at the first invalid request and resets lazily.
- Counts 401, 403 and every 429, from both handlers and on every attempt.
- Emits `invalidRequestWarning` when `interval > 0 && count % interval === 0`, on the instance that made the request, using that instance's interval.
- Test `RequestHandler.test.ts:82`: 5 requests with interval 0, then 5 with interval 2, gives 3 warnings (at 6, 8 and 10).

## 8. Global limit

State lives on the `REST` instance: `globalRemaining`, `globalReset`, `globalDelay`.

- Local fixed 1 second window, opened lazily by the first request after expiry. Budget is `globalRequestsPerSecond`. Only `SequentialHandler` uses it.
- Decrement happens on every attempt, including retries.
- On a 429 with `X-RateLimit-Global`: budget set to 0, reset set to `now + retryAfter`. All sequential handlers on the instance then wait on the shared `globalDelay`.
- Custom-token requests share this budget. Only buckets are partitioned per token.
- `rateLimited` fires once per waiting request and per loop iteration, not once per global event.

A 429 with `Retry-After` but without `X-RateLimit-Global` and without bucket headers, such as one from Cloudflare, is not global for this library. Only the affected handler waits (test `RequestHandler.test.ts:380`), and other handlers keep sending.

## 9. Sublimits

`hasSublimit(bucketRoute, body, method)` (`utils.ts:118-132`):

- `/channels/:id`: true only if the method is `PATCH` and the body is a non-null object with `name` or `topic`.
- Any other route: true.
- It is consulted only while a sublimit queue exists, so "true" here means "serialise everything on this route during a sublimit".

A sublimit is detected when a response has `Retry-After > 0`, is not global, and leaves the handler not `localLimited`. That covers a `remaining > 0` 429 and a 429 with no bucket headers.

State machine (names: `Q` = `#asyncQueue`, `SQ` = `#sublimitedQueue`, `SP` = `#sublimitPromise`):

```text
queueRequest(req):
  queue = Q; type = Standard
  if SQ and hasSublimit(req): queue = SQ; type = Sublimit
  await queue.wait(signal)                      # may reject on abort
  if type == Standard:
    if SQ and hasSublimit(req):                 # retroactive move
      queue = SQ; w = SQ.wait(); Q.shift(); await w
    elif SP: await SP.promise                   # stall, while still holding Q's head
  try: return runRequest(...)
  finally:
    queue.shift()
    if shiftSublimit: shiftSublimit = false; SQ?.shift()
    if SQ?.remaining == 0: SP?.resolve(); SQ = null

runRequest, on 429 with sublimitTimeout:
  first = SQ is null
  if first: SQ = new; SQ.wait() (not awaited, takes head); Q.shift()
  SP?.resolve(); SP = null
  sleep(sublimitTimeout)
  SP = new promise
  if first: await Q.wait(); shiftSublimit = true
  return runRequest(same retries)
```

Intended contract:

- Non-sublimited traffic keeps flowing during the sleep, because `SP` is null then.
- Sublimited requests queue behind the first one, both new arrivals and those already in `Q`.
- After the sleep, standard requests stall until `SQ` drains, because the retry may reach another sublimit.
- If a request inside `SQ` hits the sublimit again, the stalled standard requests are released for the new sleep.

**Stall found in this logic [model].** On `/channels/:id` the handler can hang permanently:

1. R (`PATCH name`) gets a sublimit 429 and sleeps, holding the head of `SQ`.
2. X (`PATCH bitrate`) is at the head of `Q`, in flight or waiting on a bucket reset, when R wakes. Y (`PATCH bitrate`) is queued behind X.
3. R wakes, sets `SP`, and queues in `Q` behind Y.
4. X finishes. Y reaches the head of `Q` and awaits `SP`.
5. `SP` resolves only when `SQ` drains, and that needs R, which is behind Y. Nothing can proceed, and later requests on that handler queue forever.

My transcription (`SequentialHandler` queue and 429 logic only, fake network) reproduced it: R and Y never completed, with `Q.remaining = 2` and `SQ.remaining = 1`. The control run with no Y completed normally. The upstream `Handle sublimits` test does not cover this timing. I have not run the real library, so treat it as a strong finding that is unconfirmed upstream. Other routes avoid it because `hasSublimit` is true for every request, so Y moves to `SQ` and frees `Q`. The port should not copy this structure as is (see 14.7) and should include a regression test.

## 10. Error types (`lib/errors/`)

**`DiscordAPIError`:** `rawError`, `code` (`number | string`), `status`, `method`, `url`, `requestBody { files, json }`. `name` is `DiscordAPIError[<code>]`.

Message (`DiscordAPIError.ts:76-89`):

- Discord shape (`code` present): `message + '\n' + flattened`, or whichever is non-empty, or `Unknown Error`.
- OAuth shape: `error_description ?? 'No Description'`.

`flattenDiscordError(obj, key = '')`:

- If `obj.message` is a string: yield `` `${key}[${code}]: ${message}` ``, or `` `${code}: ${message}` `` with an empty key, trimmed.
- Otherwise for each `[otherKey, val]`:
  - `nextKey` is `key` if `otherKey` starts with `_`.
  - With a non-empty `key`: `key.otherKey`, or `key[otherKey]` if numeric.
  - With an empty `key`: `otherKey`.
  - A string `val` yields as is. A `_errors` wrapper recurses over each element with `nextKey`. Anything else recurses.

Expected outputs from `__tests__/DiscordAPIError.test.ts`:

```text
Invalid Form Body\nusername[BASE_TYPE_BAD_LENGTH]: Must be between 2 and 32 in length.
Invalid Form Body\nembed.fields[0].value[BASE_TYPE_REQUIRED]: This field is required
Invalid Form Body\nform_fields.label[0][BASE_TYPE_REQUIRED]: This field is required
```

**`HTTPError`:** `status`, `method`, `url`, `requestBody`. Message is `statusText` and `name` is `HTTPError`. The response body is not read.

**`RateLimitError`:** all `RateLimitData` fields. `name` is `RateLimitError[<route>]` and the message is empty.

**Other failures** surface as raw errors: `AbortError` after retries, non-retryable network errors, JSON parse errors, the missing-token `Error`, and `RangeError` from CDN validation. `AsyncQueue` rejects with `Request aborted manually` for a queued abort (test `RequestHandler.test.ts:749`).

`dat/payloads/common.ts:342`: `RESTRateLimit { code?, global, message, retry_after }` describes the 429 JSON body, but the library never reads it. `RESTJSONErrorCodes` lives in `dat/rest/common.ts`.

## 11. CDN (`lib/CDN.ts`)

Constants (`constants.ts:53-57`):

- `ALLOWED_EXTENSIONS`: `webp, png, jpg, jpeg, gif`
- `ALLOWED_STICKER_EXTENSIONS`: `png, json, gif`
- `ALLOWED_SIZES`: `16` to `4096`, powers of two

`makeURL(route, opts)`:

- Lower-case the extension (default `webp`). Validate against the allow-list; the failure is a `RangeError`.
- If `size` is truthy, validate it.
- Build `new URL(base + route + '.' + ext)`.
- Set `animated` when it is not `undefined`, so `animated=false` is emitted. Then set `size`. Order is always `animated`, then `size`.

`dynamicMakeURL`: `animated = true` if `!forceStatic && hash.startsWith('a_')`. The extension is unchanged, so the result is `.webp?animated=true` rather than `.gif`.

| Method | Path | Kind |
|---|---|---|
| `appAsset(clientId, hash, opts)` | `/app-assets/{id}/{hash}` | static |
| `appIcon(clientId, hash, opts)` | `/app-icons/{id}/{hash}` | static |
| `avatar(id, hash, opts)` | `/avatars/{id}/{hash}` | dynamic |
| `avatarDecoration(asset)` | `/avatar-decoration-presets/{asset}` | forced `.png` |
| `banner(id, hash, opts)` | `/banners/{id}/{hash}` | dynamic |
| `channelIcon(channelId, hash, opts)` | `/channel-icons/{id}/{hash}` | static |
| `defaultAvatar(index)` | `/embed/avatars/{index}` | forced `.png` |
| `discoverySplash(guildId, hash, opts)` | `/discovery-splashes/{id}/{hash}` | static |
| `emoji(emojiId, opts)` | `/emojis/{id}` | static, `animated` allowed with webp |
| `guildMemberAvatar(guildId, userId, hash, opts)` | `/guilds/{g}/users/{u}/avatars/{hash}` | dynamic |
| `guildMemberBanner(guildId, userId, hash, opts)` | `/guilds/{g}/users/{u}/banners/{hash}` | dynamic |
| `icon(id, hash, opts)` | `/icons/{id}/{hash}` | dynamic |
| `roleIcon(roleId, hash, opts)` | `/role-icons/{id}/{hash}` | static |
| `splash(guildId, hash, opts)` | `/splashes/{id}/{hash}` | static |
| `sticker(id, ext = 'png')` | `/stickers/{id}` | sticker allow-list; `gif` uses `mediaProxy` |
| `stickerPackBanner(bannerId, opts)` | `/app-assets/710982414301790216/store/{id}` | static |
| `teamIcon(teamId, hash, opts)` | `/team-icons/{id}/{hash}` | static |
| `guildScheduledEventCover(eventId, hash, opts)` | `/guild-events/{id}/{hash}` | static |
| `soundboardSound(soundId)` | `/soundboard-sounds/{id}` | no extension, bypasses `makeURL` |
| `guildTagBadge(guildId, hash, opts)` | `/guild-tag-badges/{id}/{hash}` | static |

20 public methods. `calculateUserDefaultAvatarIndex(id)` is `(id >> 22) % 6` (`utils.ts:168`).

`dat` `CDNRoutes` has 24 entries (`dat/rest/v10/index.ts:1247`). Those missing from `CDN`: `userAvatarDecoration`, `applicationCover` (same path as `appIcon`), `achievementIcon`, `storePageAsset`. `CDN.channelIcon` has no `CDNRoutes` entry.

## 12. Proxy support

There are three distinct mechanisms.

**Outbound HTTP proxy.** There is no dedicated option. The caller passes `agent: new ProxyAgent(url)`, or a per-request `dispatcher` (`apps/guide/content/docs/legacy/additional-info/proxy.mdx`). A custom `makeRequest` can do the same. The gateway needs its own setup.

**Reverse proxy target.** `api: 'http://host:port/api'` sends calls to a proxy such as `@discordjs/proxy`.

**`@discordjs/proxy`** (`packages/proxy/src/handlers/proxyRequests.ts`):

- `fullRoute` is the path with `^/api(/v\d+)?` removed. Query is forwarded.
- Forwards only `Content-Type`, `Authorization`, `X-Audit-Log-Reason`.
- Calls `rest.queueRequest({ body: req, passThroughBody: true, auth: false, ... })`. The method is passed through unvalidated so Discord can answer 405.
- Success: copies status and headers except `/^x-ratelimit/i`, then pipes the body.
- `DiscordAPIError`: status plus `rawError` as JSON. `HTTPError`: status only.
- `RateLimitError`: 429, `Retry-After: timeToReset / 1000`, `X-RateLimit-Global` if global, `X-RateLimit-Scope`.
- `AbortError`: 504 `Upstream timed out`.
- Unknown errors are rethrown after `res.end()`.
- `apps/proxy-container` runs it with `rejectOnRateLimit: true, retries: 0`, leaving retries to the caller.

Limitations to keep in mind if this is ported:

- `auth: false` means all client tokens share buckets.
- `body: req` is a consumed stream, so a retry cannot resend it. The container's `retries: 0` avoids this.
- For a sublimit, `Retry-After` comes from `timeToReset`, not `sublimitTimeout`, so it can be far too short.

## 13. Utilities and transport

`makeURLSearchParams(params, { arrayFormat = 'repeat' | 'comma' })` (`utils.ts:57-95`):

- Drops `null`, `undefined`, functions, symbols, invalid dates, and objects with no custom `toString`.
- String stays as is. Number, bigint and boolean use `toString`, so `false` is emitted as `false`. `Date` uses `toISOString`. Other objects use a custom `toString`.
- Arrays filter their elements the same way. An empty array emits nothing. `comma` joins into a single key.
- Key order is insertion order. Core never passes `arrayFormat`.

`parseResponse` (`utils.ts:102-108`): JSON if `Content-Type` starts with `application/json`, otherwise an `ArrayBuffer`. A 204 gives an empty buffer. `getWidgetImage` returns bytes this way.

Default transport (`strategies/undiciRequest.ts`):

- Resolves any body type to bytes, string or FormData.
- Lazily creates one shared `Agent` when no dispatcher is given.
- `statusText` comes from `STATUS_CODES`. `ok` means 2xx. Headers are rebuilt manually because of HTTP/2 symbol keys.
- It has no `clone`.

Selection (`index.ts`, `web.ts`): Node uses undici `request`; Deno, Bun and Workers use global `fetch`.

`generateRouteData` in `REST.ts` needs the snowflake timestamp: `(id >> 22) + 1420070400000` **[memory, @sapphire/snowflake]**.

## 14. Subtle behaviour a port must preserve

### Rate limit protocol

1. **`X-RateLimit-Reset-After`, never `X-RateLimit-Reset`.** Using the relative header avoids clock skew. The 429 JSON `retry_after` and `global` are ignored; only headers count.
2. **Bucket state is overwritten on every response,** including errors. Missing headers reset it: `limit = Infinity`, `remaining = 1`, `reset = now`.
3. **Header parsing.** `"0"` is a valid value: JS tests the string, not the number, so `Remaining: 0` means 0 and `Retry-After: 0` gives `retryAfter = offset`, which is `> 0` by default. Unparsable values become `NaN`, and all comparisons then read as "not limited". In Rust, treat a parse failure as absent. Do not use a "non-zero" test.
4. **Times are floats.** `Reset-After` is fractional seconds (`0.5`, `0.25`). Keep `f64` until converted to a `Duration`.
5. **Double offset.** `reset` already includes `offset`, and `getTimeToReset` adds it again. The same applies to global: `retryAfter` includes it and `timeout = globalReset + offset - now` adds it again. Sublimit sleep has one offset. With the default 50 ms the effect is harmless, but it does make waits longer, so keep it or make a deliberate decision.
6. **`!localLimited` is evaluated after the state update.** The order in lines 293-343 defines what counts as a sublimit: a normal 429 has `remaining: 0` plus future reset, so it is local, not sublimit. Reordering breaks it.
7. **Sublimit contract** as in section 9, but do not copy the two-queue shuffle verbatim because of the stall. Port tests: `Handle sublimits`, `Handle unexpected 429` (second queued request finishes after the first, at least 1 s later), the Cloudflare variant, plus a new regression test for the X and Y scenario.
8. **Sublimit-wide serialisation.** On any route other than `/channels/:id`, a bucket-less 429 pushes all later requests on that handler behind the first.
9. **Cloudflare-style 429s stay local.** Without `X-RateLimit-Global`, other handlers keep sending. Keep it for parity, but consider an optional process-wide brake: repeated invalid requests lead to IP bans **[memory]**.
10. **No retry cap on 429.** A 429 never increments `retries`. A 429 with neither `Retry-After` nor bucket headers re-sends immediately, in a tight loop limited only by the global budget. Consider a small floor delay.
11. **`global` in the 429 path** comes from `this.globalLimited`, not the header, so a non-global 429 reports `global: true` if the local 1 s budget happens to be exhausted. Burst uses the header.
12. **`Retry-After` is examined on any status,** not only 429. A non-429 response with `X-RateLimit-Global` and `Retry-After` still blocks globally. `Retry-After` on a 5xx does not affect backoff.
13. **`rateLimited` event timing.** It fires only in the pre-emptive loop, never directly from a 429, and never for a pure sublimit wait. `onRateLimit` can run twice for one incident: once in the 429 branch, then again in the loop. Filters must tolerate that.
14. **`rejectOnRateLimit` precedence:** `request ?? instance`, so a per-request `false` opts out. In the 429 branch it throws before sublimit state is created.
15. **`scope`** defaults to `user` when absent, and the pre-emptive path always reports `user`. Every 429 counts as invalid regardless of scope.

### Buckets and auth

16. **Hash keys are per method and exclude major.** Handler keys are hash plus major. After learning, different routes can share a handler. `routeId` must therefore be per request, not per handler.
17. **Token partitioning.** Only a custom token that differs from the instance token is partitioned: `hash:uuid` and `method:route:uuid`, as a UUID v5 so the raw token is not used in keys. `auth: {token: sameAsInstance}` is not partitioned and counts as `auth === true`. `auth: false` is not partitioned either.
18. **The 401 token wipe** applies only when `requestData.auth === true`. It covers 401s with code 0 too, and only the warning depends on `code !== 0`. Later default-auth requests then fail locally with the missing-token error. This is why webhook and interaction calls use `auth: false`: a bad webhook token must not wipe the bot token.
19. **Generalisation quirks.**
    - `\d` must be ASCII only. In the Rust `regex` crate `\d` is Unicode by default, so use `[0-9]`.
    - Do the substitutions in the listed order.
    - Non-snowflake path segments stay literal (`/invites/abc`, `/guilds/templates/abc`), creating one hash key per code. Sweeping is what bounds memory.
    - `/users/@me` and `/users/%40me` are different buckets.
    - The old-message check reads the wall clock, not a monotonic one.
20. **Route argument encoding (`dat/rest/v10/index.ts:1212-1231`).**
    - Each truthy argument matching `/^[\d%A-Za-z-_]+$/` is left alone. Anything else goes through `encodeURIComponent`.
    - So an explicit `'@me'` becomes `%40me` and `'@original'` becomes `%40original` **[model]**. Core passes these explicitly (`Routes.user('@me')`, `threadMembers(id, '@me')`, `webhookMessage(..., '@original')`), so real traffic uses the encoded form.
    - A default parameter is not encoded: `Routes.user()` gives `/users/@me`.
    - `%` counts as safe, which is why core's `encodeURIComponent(emoji)` is not double-encoded. `.` fails the pattern, but `encodeURIComponent` leaves it unchanged, so `tok.en` stays `tok.en`.
    - Core encodes the emoji in 5 methods in `channel.ts`.

### Retries, timeouts, cancellation

21. **Attempts.** `retries` counts 5xx and retryable network errors together, so total attempts are `retries + 1` (test: 500 `.times(4)` with default 3). The check is `!==`: a negative value or `Infinity` never stops. Use `u32`.
22. **Timeout covers one attempt, up to response headers only.** Rate-limit waits and body reading are outside it. In `reqwest`, a client-level timeout also covers the body **[memory]**, so wrap the send future instead.
23. **Timeouts and resets retry for every method, including POST,** so a timed-out create can run twice. This is existing behaviour; document it, and consider exposing `nonce` guidance.
24. **User abort is treated as a timeout.** `AbortError` retries, so with defaults an aborted request re-enters `makeRequest` up to 3 more times, each aborting at once, with backoff sleeps if configured. A port should separate user cancellation and not retry it. Observable end result stays the same: the request fails.
25. **Abort coverage is partial upstream.** It only covers the queue wait and the network call. Rate limit sleeps, the sublimit stall and the retroactive `SQ.wait()` ignore it until they finish.
26. **Queued abort removes only that entry.** The head entry ignores the signal and fails at the network step **[memory, async-queue]**. Test `abort active and queued requests`: the active request rejects with the signal's reason, the queued one with `Request aborted manually`, and `makeRequest` is not called for it.
27. **The queue is released at headers, not at body end,** because the handler returns before the caller reads the body. On 4xx the error body is read while still holding the queue.

### Request shape

28. **Header priority.** Instance headers beat per-request headers, and UA and auth headers beat everything. JS objects are case-sensitive while `HeaderMap` is not, so apply inserts in the same order and be careful with `editGuildCommandPermissions` and `revokeToken`, which set `Authorization` themselves with `auth: false`.
29. **Body presence.**
    - `body: {}` sends `{}`.
    - `body: null` or `undefined` sends nothing and no Content-Type.
    - `GET` with a body drops it but keeps `Content-Type: application/json`.
    - Check empty `POST`, `PUT`, `PATCH` send `Content-Length: 0`; undici does **[memory]** and Discord has rejected PUTs without it **[memory]**. Test with a mock server.
30. **Multipart.**
    - Files come first, then `payload_json`, or individual text fields for stickers.
    - Default file key is `files[<array index>]`.
    - An empty `files` array means JSON mode.
    - Only the first sniff result is used, and `image/apng` maps to `image/png`.
    - An empty type goes out as `application/octet-stream` per the FormData encoding **[memory]**.
    - Filenames: FormData escapes `"`, CR and LF as `%22`, `%0D`, `%0A` and leaves UTF-8 raw **[memory]**. Names must match `attachment://name` references, so verify the encoding the Rust multipart library applies, as its default may percent-encode **[memory]**.
31. **Retries must resend the body.** JS reuses the FormData or string. Keep the encoded body or its parts as `Bytes` so each attempt can rebuild the request. A `passThroughBody` stream cannot be resent; that is the proxy limitation.
32. **`appendToFormData` stringifies with JS rules.** An `undefined` value in `Object.entries` becomes the text `undefined`. In Rust, skip `None`, and stringify scalars without JSON quotes.
33. **Explicit null must survive.** `JSON.stringify` keeps `null` and drops `undefined`, and Discord PATCH semantics depend on it. Rust body types need a three-state field (absent, null, value).
34. **Query encoding is `application/x-www-form-urlencoded`:** space is `+`, and `~ ! ' ( )` are percent-encoded while `* - . _` are not **[model]**. `form_urlencoded` should match; confirm with a test. `encodeURIComponent` (reason, route args) differs: it leaves `! ' ( ) * ~ - _ .` and encodes space as `%20` **[model]**, so it needs its own `AsciiSet`. Test: `😄` becomes `%F0%9F%98%84` (`REST.test.ts:249`).
35. **`User-Agent` format** must stay `DiscordBot ($url, $version)` followed by the appendix. Discord requires that form **[memory]**.
36. **Redirects.** undici `request` does not follow them **[memory]**, and a 3xx returns as a non-error result. Disable automatic redirects in `reqwest` (it follows by default **[memory]**), since cross-origin redirects could expose the auth header.

### Responses and errors

37. **Non-JSON 4xx is still a `DiscordAPIError`,** with `code` undefined and message `No Description` (test `malformedRequest`, 405). The name reads `DiscordAPIError[undefined]`. Use `Option<Code>` and a raw enum with an `Unparsed(Bytes)` variant. HTML 403 and 429 pages from Cloudflare land here.
38. **`Content-Type` decides parsing, by a case-sensitive prefix.** A body labelled JSON that fails to parse throws raw. 2xx with no JSON type returns bytes. Rust typed calls need a clear rule for 204 (`()`).
39. **Error code is `number | string`:** Discord numeric or OAuth `error` string. Code `0` is real (`401: Unauthorized`).
40. **Error flattening** must match byte for byte (tests in section 10), including first-level numeric keys and `_errors` nested in `_errors`.
41. **Secrets.** `url` in errors, `RateLimitData` and debug output contains webhook and interaction tokens. Handler ids contain them too, because the webhook major parameter is `id/token`. `requestBody.json` on OAuth errors contains `client_secret`, and the `response` event carries `Authorization`. Rust `Debug` and `Display` should redact these, with a `secrecy` wrapper for tokens.
42. **`response` event for the default transport** gets a shallow copy sharing one body stream (`clone` is missing). In Rust, emit a buffered copy only if a listener exists.

### Process-wide and runtime assumptions

43. **`invalidCount` and the 401 warning flag are per process,** not per client. Use `static` atomics or a mutex.
44. **JS atomicity must be made explicit.** No `await` separates these, so in JS they are atomic. In multi-threaded Rust they need one lock:
    - `limited` check, window reset and `globalRemaining--`
    - `handlers.get`, then create, then enqueue
    - the `SP` assignment plus `Q.wait()` after the sublimit sleep
    - Note the sweep race: a handler removed between lookup and enqueue would allow two handlers for one bucket. Check `inactive` and enqueue under the same lock.
45. **Rust futures can be dropped at any `.await`.** JS promises cannot, so upstream never needs this. Every queue slot (both queues, the `shiftSublimit` debt, the sublimit gate) must be released by an RAII guard, or a cancelled request hangs its bucket. This is the largest single porting risk.
46. **Rust futures are lazy.** `rest.get()` in JS joins the queue synchronously at call time, so call order equals send order. A Rust future joins on first poll. If order matters, join in a non-async function that returns the future, or document the difference.
47. **`SP` stays set after resolving** (line 180 does not clear it), and later standard requests pass straight through a resolved promise. A Rust `Notify` is edge-triggered and would hang here, so use a `watch` channel or a level-triggered gate.
48. **Recursion becomes a loop.** `runRequest` recurses on every retry and 429; use a loop to avoid boxed async recursion. Sleep arguments can be slightly negative in JS, where timers clamp, so saturate to zero.
49. **Clock choice.** JS uses `Date.now()` throughout. Prefer `Instant` for bucket and global timing, and `SystemTime` only for the snowflake age check.
50. **Background sweepers must not keep the client alive,** the equivalent of `unref`. Hold a `Weak` in the task, stop it on drop, and keep the 4 h validation or an equivalent `Duration` check.
51. **Handler concurrency is 1,** by design. Any parallel send within a bucket and major changes the observable rate limit behaviour.
52. **Burst has no ordering guarantee** and no shared state. Do not route it through the sequential path, since a 3 second interaction deadline **[memory]** is why it exists.

## 14b. Rust mapping suggestions

Suggestions only. Crate behaviour and versions are **[memory]** and were not checked here.

| JS | Rust |
|---|---|
| `REST` | `Rest(Arc<RestInner>)`, cheap to clone |
| `makeRequest` | `trait Transport`, default `reqwest` with `redirect::Policy::none()` and `rustls`; enables a mock in tests |
| `agent`, `ProxyAgent` | `reqwest::Proxy` in a builder, or a custom `Transport` |
| `AsyncQueue` | Custom FIFO with `VecDeque<oneshot::Sender>` plus RAII guard |
| `#sublimitPromise` | `tokio::sync::watch` |
| `AbortSignal` | Dropping the future, plus an optional `CancellationToken` |
| `AsyncEventEmitter` | Non-blocking callback or `broadcast` channel; `tracing` for debug lines |
| `magic-bytes.js` | `infer` |
| `uuid` | `uuid` with `v5` |
| Errors | `enum RestError { Api, Http, RateLimit, Timeout, Cancelled, Network, MissingToken, Decode, InvalidCdn }` |
| `hasSublimit` body check | Compute a flag from the body before serialising |
| Multipart | Small encoder into `Bytes` for exact filename control and cheap retries |
| `RawFile.data` | `Bytes` or `String`; number and boolean forms can be dropped |
| Sublimit logic | Handler actor or explicit gate, meeting the contract in section 9 without the stall |

## 14c. Upstream tests worth porting

| File | Cases |
|---|---|
| `packages/rest/__tests__/RequestHandler.test.ts` (752 lines) | Significant Invalid Requests; standard rate limits; sublimits; unexpected 429; unexpected 429 Cloudflare; `rejectOnRateLimit` pre-emptive and on 429; per-token isolation; per-call precedence; global; temp and permanent outage; slow server; Unauthorized; Bad Request; malformed; abort listener cleanup; abort active and queued |
| `BurstHandler.test.ts` (222) | Handler selection, burst concurrency, 404, unexpected 429, reject, slow server, outage |
| `REST.test.ts` (450) | Verbs, query, auth variants, reason encoding, urlencoded, echo, 201, old-message delete, `postFile` matrix |
| `DiscordAPIError.test.ts` (143) | Flatten strings, OAuth shape |
| `CDN.test.ts` (154) | Every URL form, invalid size and extension |
| `utils.test.ts` (199) | `makeURLSearchParams`, normalisers |
| `RequestManager.test.ts` (42) | Missing token, negative offset |
| `UndiciRequest.test.ts` (135) | Body resolution, header rebuild |
| `packages/proxy/__tests__/proxyRequests.test.ts` (84) | Header stripping, error passthrough |

## 15. Endpoint coverage: `packages/core/src/api`

`API` (`index.ts:40-97`) holds one instance of each class, all sharing one `REST`. `InteractionsAPI` also takes `WebhooksAPI`. Counts are unique public method names; overload signatures are not counted separately.

212 methods: 211 make HTTP calls (4 of them via `WebhooksAPI`), and 1 builds a URL (`OAuth2API.generateAuthorizationURL`).

Patterns:

- Almost every method takes a trailing `{ auth?, signal?, reason? }` subset. Interaction and webhook-token methods accept only `signal`, since they force `auth: false`.
- Non-JSON or special bodies:
  - `oauth2.ts`: `tokenExchange`, `refreshToken`, `getToken`, `revokeToken` use URL-encoded `passThroughBody`.
  - `revokeToken` sets `Authorization: Basic base64(id:secret)`.
  - `editGuildCommandPermissions` sets `Authorization: Bearer <token>` after removing any `Bearer ` prefix.
  - `getWidgetImage` returns bytes.
- `InteractionsAPI` wraps the body as `{ type, data }`. Types: `reply` ChannelMessageWithSource, `defer` DeferredChannelMessageWithSource, `deferMessageUpdate` DeferredMessageUpdate, `updateMessage` UpdateMessage, `createAutocompleteResponse` ApplicationCommandAutocompleteResult, `createModal` Modal, `launchActivity` LaunchActivity. `with_response` goes in the query, and the result is returned only if it was set. In Rust, use two methods or a generic rather than overloads.
- `followUp` forces `wait: true`. `editReply` and `deleteReply` default to `@original`.
- `WebhooksAPI.execute` returns a result only with `wait: true`.
- File uploads (10 methods plus follow-ups): `ChannelsAPI.createMessage`, `editMessage`, `createInvite`, `createForumThread`; `GuildsAPI.createSticker`; `InteractionsAPI.reply`, `updateMessage`; `InvitesAPI.updateTargetUsers`; `WebhooksAPI.execute`, `editMessage`. `followUp` and `editReply` get files through the webhook methods. `createForumThread` lifts `files` out of `message`.
- `auth: false` is forced on: `GatewayAPI.get`, the 7 interaction callback methods, 4 OAuth2 POSTs, `editGuildCommandPermissions`, and 6 `WebhooksAPI` methods (`execute`, `executeSlack`, `executeGitHub`, `getMessage`, `editMessage`, `deleteMessage`). `WebhooksAPI.get`, `edit` and `delete` depend on whether a token is passed.
- Routes in `dat` not used by core (16 of 129):
  - Deprecated: `channelPins`, `channelPin`, `guilds`, `guildMFA`, `guildCurrentMemberNickname`, `nitroStickerPacks`.
  - Others: `channelPermissionsMember`, `channelPermissionsRole`, `channelVoiceStatus`, `guildPermissionsMember`, `guildPermissionsRole`, `guildMemberVerification`, `applicationUserIdentityProfile`, `userApplicationIdentities`, `applicationIdentities`, `userApplicationIdentityDelete`.

The Options column below comes from a text scan of each method body, so treat it as an index and confirm against the source when implementing.

| Class | File | Methods | Lines |
|---|---|---:|---:|
| ApplicationCommandsAPI | `applicationCommands.ts` | 15 | 347 |
| ApplicationsAPI | `applications.ts` | 8 | 157 |
| ChannelsAPI | `channel.ts` | 34 | 757 |
| GatewayAPI | `gateway.ts` | 2 | 32 |
| GuildsAPI | `guild.ts` | 78 | 1502 |
| InteractionsAPI | `interactions.ts` | 11 | 591 |
| InvitesAPI | `invite.ts` | 5 | 96 |
| MonetizationAPI | `monetization.ts` | 8 | 159 |
| OAuth2API | `oauth2.ts` | 7 | 153 |
| PollAPI | `poll.ts` | 2 | 55 |
| RoleConnectionsAPI | `roleConnections.ts` | 2 | 49 |
| SoundboardSoundsAPI | `soundboardSounds.ts` | 1 | 19 |
| StageInstancesAPI | `stageInstances.ts` | 4 | 79 |
| StickersAPI | `sticker.ts` | 3 | 44 |
| ThreadsAPI | `thread.ts` | 6 | 132 |
| UsersAPI | `user.ts` | 12 | 201 |
| VoiceAPI | `voice.ts` | 5 | 97 |
| WebhooksAPI | `webhook.ts` | 9 | 272 |
| **Total (18 classes)** | | **212** | |

#### ApplicationCommandsAPI (15), `packages/core/src/api/applicationCommands.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getGlobalCommands` | GET applicationCommands | query |
| `createGlobalCommand` | POST applicationCommands | body |
| `getGlobalCommand` | GET applicationCommand |  |
| `editGlobalCommand` | PATCH applicationCommand | body |
| `deleteGlobalCommand` | DELETE applicationCommand |  |
| `bulkOverwriteGlobalCommands` | PUT applicationCommands | body |
| `getGuildCommands` | GET applicationGuildCommands | query |
| `createGuildCommand` | POST applicationGuildCommands | body |
| `getGuildCommand` | GET applicationGuildCommand |  |
| `editGuildCommand` | PATCH applicationGuildCommand | body |
| `deleteGuildCommand` | DELETE applicationGuildCommand |  |
| `bulkOverwriteGuildCommands` | PUT applicationGuildCommands | body |
| `getGuildCommandPermissions` | GET applicationCommandPermissions |  |
| `getGuildCommandsPermissions` | GET guildApplicationCommandsPermissions |  |
| `editGuildCommandPermissions` | PUT applicationCommandPermissions | auth:false, body |

#### ApplicationsAPI (8), `packages/core/src/api/applications.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getCurrent` | GET currentApplication |  |
| `editCurrent` | PATCH currentApplication | body |
| `getEmojis` | GET applicationEmojis |  |
| `getEmoji` | GET applicationEmoji |  |
| `createEmoji` | POST applicationEmojis | body |
| `editEmoji` | PATCH applicationEmoji | body |
| `deleteEmoji` | DELETE applicationEmoji |  |
| `getActivityInstance` | GET applicationActivityInstance |  |

#### ChannelsAPI (34), `packages/core/src/api/channel.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `createMessage` | POST channelMessages | files, body |
| `editMessage` | PATCH channelMessage | files, body |
| `getMessageReactions` | GET channelMessageReaction | query |
| `deleteOwnMessageReaction` | DELETE channelMessageOwnReaction |  |
| `deleteUserMessageReaction` | DELETE channelMessageUserReaction |  |
| `deleteAllMessageReactions` | DELETE channelMessageAllReactions |  |
| `deleteAllMessageReactionsForEmoji` | DELETE channelMessageReaction |  |
| `addMessageReaction` | PUT channelMessageOwnReaction |  |
| `get` | GET channel |  |
| `edit` | PATCH channel | reason, body |
| `delete` | DELETE channel | reason |
| `getMessages` | GET channelMessages | query |
| `showTyping` | POST channelTyping |  |
| `getPins` | GET channelMessagesPins | query |
| `pinMessage` | PUT channelMessagesPin | reason |
| `deleteMessage` | DELETE channelMessage |  |
| `bulkDeleteMessages` | POST channelBulkDelete | reason, body |
| `getMessage` | GET channelMessage |  |
| `crosspostMessage` | POST channelMessageCrosspost |  |
| `unpinMessage` | DELETE channelMessagesPin | reason |
| `followAnnouncements` | POST channelFollowers | reason, body |
| `createInvite` | POST channelInvites | files, reason, body |
| `getInvites` | GET channelInvites |  |
| `createThread` | POST threads | reason, body |
| `createForumThread` | POST threads | files, reason, body |
| `getArchivedThreads` | GET channelThreads | query |
| `getJoinedPrivateArchivedThreads` | GET channelJoinedArchivedThreads | query |
| `createWebhook` | POST channelWebhooks | reason, body |
| `getWebhooks` | GET channelWebhooks |  |
| `editPermissionOverwrite` | PUT channelPermission | reason, body |
| `deletePermissionOverwrite` | DELETE channelPermission | reason |
| `sendSoundboardSound` | POST sendSoundboardSound | body |
| `addGroupDMRecipient` | PUT channelRecipient | body |
| `removeGroupDMRecipient` | DELETE channelRecipient |  |

#### GatewayAPI (2), `packages/core/src/api/gateway.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `get` | GET gateway | auth:false |
| `getBot` | GET gatewayBot |  |

#### GuildsAPI (78), `packages/core/src/api/guild.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `get` | GET guild | query |
| `getPreview` | GET guildPreview |  |
| `edit` | PATCH guild | reason, body |
| `addMember` | PUT guildMember | body |
| `getMembers` | GET guildMembers | query |
| `getChannels` | GET guildChannels |  |
| `createChannel` | POST guildChannels | reason, body |
| `setChannelPositions` | PATCH guildChannels | reason, body |
| `getActiveThreads` | GET guildActiveThreads |  |
| `getMemberBan` | GET guildBan |  |
| `getMemberBans` | GET guildBans | query |
| `banUser` | PUT guildBan | reason, body |
| `unbanUser` | DELETE guildBan | reason |
| `bulkBanUsers` | POST guildBulkBan | reason, body |
| `getRoles` | GET guildRoles |  |
| `getRole` | GET guildRole |  |
| `createRole` | POST guildRoles | reason, body |
| `setRolePositions` | PATCH guildRoles | reason, body |
| `editRole` | PATCH guildRole | reason, body |
| `deleteRole` | DELETE guildRole | reason |
| `getPruneCount` | GET guildPrune | query |
| `beginPrune` | POST guildPrune | reason, body |
| `getVoiceRegions` | GET guildVoiceRegions |  |
| `getInvites` | GET guildInvites |  |
| `getIntegrations` | GET guildIntegrations |  |
| `deleteIntegration` | DELETE guildIntegration | reason |
| `getWidgetSettings` | GET guildWidgetSettings |  |
| `editWidgetSettings` | PATCH guildWidgetSettings | reason, body |
| `getWidget` | GET guildWidgetJSON |  |
| `getVanityURL` | GET guildVanityUrl |  |
| `getWidgetImage` | GET guildWidgetImage | query |
| `getWelcomeScreen` | GET guildWelcomeScreen |  |
| `editWelcomeScreen` | PATCH guildWelcomeScreen | reason, body |
| `getEmojis` | GET guildEmojis |  |
| `getEmoji` | GET guildEmoji |  |
| `createEmoji` | POST guildEmojis | reason, body |
| `editEmoji` | PATCH guildEmoji | reason, body |
| `deleteEmoji` | DELETE guildEmoji | reason |
| `getScheduledEvents` | GET guildScheduledEvents | query |
| `createScheduledEvent` | POST guildScheduledEvents | reason, body |
| `getScheduledEvent` | GET guildScheduledEvent | query |
| `editScheduledEvent` | PATCH guildScheduledEvent | reason, body |
| `deleteScheduledEvent` | DELETE guildScheduledEvent | reason |
| `getScheduledEventUsers` | GET guildScheduledEventUsers | query |
| `getTemplates` | GET guildTemplates |  |
| `syncTemplate` | PUT guildTemplate |  |
| `editTemplate` | PATCH guildTemplate | body |
| `deleteTemplate` | DELETE guildTemplate |  |
| `getStickers` | GET guildStickers |  |
| `getSticker` | GET guildSticker |  |
| `createSticker` | POST guildStickers | files, reason, appendToFormData, body |
| `editSticker` | PATCH guildSticker | reason, body |
| `deleteSticker` | DELETE guildSticker | reason |
| `getAuditLogs` | GET guildAuditLog | query |
| `getAutoModerationRules` | GET guildAutoModerationRules |  |
| `getAutoModerationRule` | GET guildAutoModerationRule |  |
| `createAutoModerationRule` | POST guildAutoModerationRules | reason, body |
| `editAutoModerationRule` | PATCH guildAutoModerationRule | reason, body |
| `deleteAutoModerationRule` | DELETE guildAutoModerationRule | reason |
| `getMember` | GET guildMember |  |
| `searchForMembers` | GET guildMembersSearch | query |
| `searchForMessages` | GET guildMessagesSearch | query |
| `editMember` | PATCH guildMember | reason, body |
| `removeMember` | DELETE guildMember | reason |
| `addRoleToMember` | PUT guildMemberRole | reason |
| `removeRoleFromMember` | DELETE guildMemberRole | reason |
| `getTemplate` | GET template |  |
| `createTemplate` | POST guildTemplates | body |
| `getWebhooks` | GET guildWebhooks |  |
| `getOnboarding` | GET guildOnboarding |  |
| `editOnboarding` | PUT guildOnboarding | reason, body |
| `getSoundboardSounds` | GET guildSoundboardSounds |  |
| `getSoundboardSound` | GET guildSoundboardSound |  |
| `createSoundboardSound` | POST guildSoundboardSounds | reason, body |
| `editSoundboardSound` | PATCH guildSoundboardSound | reason, body |
| `deleteSoundboardSound` | DELETE guildSoundboardSound | reason |
| `editIncidentActions` | PUT guildIncidentActions | body |
| `getRoleMemberCounts` | GET guildRoleMemberCounts |  |

#### InteractionsAPI (11), `packages/core/src/api/interactions.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `reply` (+3 overload sigs) | POST interactionCallback | files, query, auth:false, body |
| `defer` (+3 overload sigs) | POST interactionCallback | query, auth:false, body |
| `deferMessageUpdate` (+3 overload sigs) | POST interactionCallback | query, auth:false, body |
| `followUp` | via webhooks.execute | body |
| `editReply` | via webhooks.editMessage |  |
| `getOriginalReply` | via webhooks.getMessage |  |
| `deleteReply` | via webhooks.deleteMessage |  |
| `updateMessage` (+3 overload sigs) | POST interactionCallback | files, query, auth:false, body |
| `createAutocompleteResponse` (+3 overload sigs) | POST interactionCallback | query, auth:false, body |
| `createModal` (+3 overload sigs) | POST interactionCallback | query, auth:false, body |
| `launchActivity` (+3 overload sigs) | POST interactionCallback | query, auth:false, body |

#### InvitesAPI (5), `packages/core/src/api/invite.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `get` | GET invite | query |
| `delete` | DELETE invite | reason |
| `getTargetUsers` | GET inviteTargetUsers |  |
| `updateTargetUsers` | PUT inviteTargetUsers | files |
| `getTargetUsersJobStatus` | GET inviteTargetUsersJobStatus |  |

#### MonetizationAPI (8), `packages/core/src/api/monetization.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getSKUs` | GET skus |  |
| `getSKUSubscriptions` | GET skuSubscriptions | query |
| `getSKUSubscription` | GET skuSubscription |  |
| `getEntitlements` | GET entitlements | query |
| `getEntitlement` | GET entitlement |  |
| `createTestEntitlement` | POST entitlements | body |
| `deleteTestEntitlement` | DELETE entitlement |  |
| `consumeEntitlement` | POST consumeEntitlement |  |

#### OAuth2API (7), `packages/core/src/api/oauth2.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `generateAuthorizationURL` | (no HTTP, builds URL) |  |
| `tokenExchange` | POST oauth2TokenExchange | auth:false, urlencoded |
| `refreshToken` | POST oauth2TokenExchange | auth:false, urlencoded |
| `getToken` | POST oauth2TokenExchange | auth:false, urlencoded |
| `getCurrentBotApplicationInformation` | GET oauth2CurrentApplication |  |
| `getCurrentAuthorizationInformation` | GET oauth2CurrentAuthorization |  |
| `revokeToken` | POST oauth2TokenRevocation | auth:false, urlencoded |

#### PollAPI (2), `packages/core/src/api/poll.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getAnswerVoters` | GET pollAnswerVoters | query |
| `expirePoll` | POST expirePoll |  |

#### RoleConnectionsAPI (2), `packages/core/src/api/roleConnections.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getMetadataRecords` | GET applicationRoleConnectionMetadata |  |
| `updateMetadataRecords` | PUT applicationRoleConnectionMetadata | body |

#### SoundboardSoundsAPI (1), `packages/core/src/api/soundboardSounds.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getSoundboardDefaultSounds` | GET soundboardDefaultSounds |  |

#### StageInstancesAPI (4), `packages/core/src/api/stageInstances.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `create` | POST stageInstances | reason, body |
| `get` | GET stageInstance |  |
| `edit` | PATCH stageInstance | reason, body |
| `delete` | DELETE stageInstance | reason |

#### StickersAPI (3), `packages/core/src/api/sticker.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getStickerPack` | GET stickerPack |  |
| `getStickers` | GET stickerPacks |  |
| `get` | GET sticker |  |

#### ThreadsAPI (6), `packages/core/src/api/thread.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `join` | PUT threadMembers |  |
| `addMember` | PUT threadMembers |  |
| `leave` | DELETE threadMembers |  |
| `removeMember` | DELETE threadMembers |  |
| `getMember` (+2 overload sigs) | GET threadMembers | query |
| `getMembers` | GET threadMembers | query |

#### UsersAPI (12), `packages/core/src/api/user.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `get` | GET user |  |
| `getCurrent` | GET user |  |
| `getGuilds` | GET userGuilds | query |
| `leaveGuild` | DELETE userGuild |  |
| `edit` | PATCH user | body |
| `getGuildMember` | GET userGuildMember |  |
| `editCurrentGuildMember` | PATCH guildMember | reason, body |
| `createDM` | POST userChannels | body |
| `getConnections` | GET userConnections |  |
| `getApplicationRoleConnection` | GET userApplicationRoleConnection |  |
| `updateApplicationRoleConnection` | PUT userApplicationRoleConnection | body |
| `deleteApplicationRoleConnection` | DELETE userApplicationRoleConnection |  |

#### VoiceAPI (5), `packages/core/src/api/voice.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `getVoiceRegions` | GET voiceRegions |  |
| `getUserVoiceState` | GET guildVoiceState |  |
| `getVoiceState` | GET guildVoiceState |  |
| `editUserVoiceState` | PATCH guildVoiceState | reason, body |
| `editVoiceState` | PATCH guildVoiceState | body |

#### WebhooksAPI (9), `packages/core/src/api/webhook.ts`

| Method | HTTP / Routes key | Options |
|---|---|---|
| `get` | GET webhook |  |
| `edit` | PATCH webhook | reason, body |
| `delete` | DELETE webhook | reason |
| `execute` (+2 overload sigs) | POST webhook | files, query, auth:false, body |
| `executeSlack` | POST webhookPlatform | query, auth:false, body |
| `executeGitHub` | POST webhookPlatform | query, auth:false, body |
| `getMessage` | GET webhookMessage | query, auth:false |
| `editMessage` | PATCH webhookMessage | files, query, auth:false, body |
| `deleteMessage` | DELETE webhookMessage | query, auth:false |

## 16. `Routes` in discord-api-types (`dat/rest/v10/index.ts`)

Contents of the file:

- `APIVersion = '10'` (line 27).
- `Routes` (lines 29-1209): 129 functions. Verified by two independent counts, a parser and a line grep.
- Argument-escaping wrapper over every route (lines 1212-1231), then frozen. See 14.20.
- `StickerPackApplicationId = '710982414301790216'` (line 1235).
- `ImageFormat`: JPEG, PNG, WebP, GIF, Lottie (`json`).
- `CDNRoutes` (lines 1247-1564): 24 functions with the same escaping wrapper.
- `RouteBases` (line 1604): `api`, `cdn`, `media`, `invite`, `template`, `gift`, `scheduledEvent`.
- `OAuth2Routes` (line 1617): `authorizationURL`, `tokenURL`, `tokenRevocationURL`, `authorizationServerMetadataURL`, `openIDConfigurationURL`.

The grouping below is mine; the source file has no groups and is only loosely ordered. "Distinct ops" counts unique `VERB path` lines in the doc comments, which is approximate:

- 232 in total: GET 102, POST 39, PATCH 33, DELETE 37, PUT 21.
- It overcounts real endpoints, because `@me` versus `{user.id}` and `{webhook.id}` versus `{application.id}` variants are listed separately. Webhooks is the most inflated.
- `oauth2Authorization` is a browser page, not an API call.
- "(dep.)" marks the 6 `@deprecated` entries.

| Group | Entries | Distinct ops | Routes keys |
|---|---:|---:|---|
| Auto moderation | 2 | 5 | `guildAutoModerationRules`, `guildAutoModerationRule` |
| Audit log | 1 | 1 | `guildAuditLog` |
| Channel (core, permissions, invites, followers, typing, recipients, voice status) | 9 | 14 | `channel`, `channelPermission`, `channelPermissionsMember`, `channelPermissionsRole`, `channelInvites`, `channelFollowers`, `channelTyping`, `channelRecipient`, `channelVoiceStatus` |
| Messages | 4 | 7 | `channelMessages`, `channelMessage`, `channelMessageCrosspost`, `channelBulkDelete` |
| Reactions | 4 | 6 | `channelMessageOwnReaction`, `channelMessageUserReaction`, `channelMessageReaction`, `channelMessageAllReactions` |
| Pins | 4 | 6 | `channelMessagesPins`, `channelMessagesPin`, `channelPins` (dep.), `channelPin` (dep.) |
| Polls | 2 | 2 | `pollAnswerVoters`, `expirePoll` |
| Threads | 5 | 12 | `threads`, `guildActiveThreads`, `channelThreads`, `channelJoinedArchivedThreads`, `threadMembers` |
| Guild (core, channels, MFA, prune, regions, vanity, onboarding, welcome, verification, incident actions) | 12 | 20 | `guilds` (dep.), `guild`, `guildPreview`, `guildChannels`, `guildMFA` (dep.), `guildPrune`, `guildVoiceRegions`, `guildVanityUrl`, `guildOnboarding`, `guildWelcomeScreen`, `guildMemberVerification`, `guildIncidentActions` |
| Guild members (+ member/message search) | 7 | 12 | `guildMember`, `guildMembers`, `guildMembersSearch`, `guildMessagesSearch`, `guildCurrentMemberNickname` (dep.), `guildMemberRole`, `guildPermissionsMember` |
| Bans | 3 | 5 | `guildBans`, `guildBan`, `guildBulkBan` |
| Roles | 4 | 8 | `guildRoles`, `guildRole`, `guildPermissionsRole`, `guildRoleMemberCounts` |
| Integrations | 2 | 2 | `guildIntegrations`, `guildIntegration` |
| Widget | 3 | 4 | `guildWidgetSettings`, `guildWidgetJSON`, `guildWidgetImage` |
| Invites | 4 | 6 | `guildInvites`, `invite`, `inviteTargetUsers`, `inviteTargetUsersJobStatus` |
| Templates | 3 | 7 | `template`, `guildTemplates`, `guildTemplate` |
| Emojis (guild + application) | 4 | 10 | `guildEmojis`, `guildEmoji`, `applicationEmojis`, `applicationEmoji` |
| Stickers | 6 | 8 | `sticker`, `stickerPacks`, `stickerPack`, `nitroStickerPacks` (dep.), `guildStickers`, `guildSticker` |
| Scheduled events | 3 | 6 | `guildScheduledEvents`, `guildScheduledEvent`, `guildScheduledEventUsers` |
| Soundboard | 4 | 7 | `sendSoundboardSound`, `soundboardDefaultSounds`, `guildSoundboardSounds`, `guildSoundboardSound` |
| Stage instances | 2 | 4 | `stageInstances`, `stageInstance` |
| Voice | 2 | 5 | `voiceRegions`, `guildVoiceState` |
| Users | 7 | 11 | `user`, `userApplicationRoleConnection`, `userGuilds`, `userGuildMember`, `userGuild`, `userChannels`, `userConnections` |
| Webhooks | 5 | 22 | `channelWebhooks`, `guildWebhooks`, `webhook`, `webhookMessage`, `webhookPlatform` |
| Gateway | 2 | 2 | `gateway`, `gatewayBot` |
| OAuth2 | 5 | 5 | `oauth2CurrentApplication`, `oauth2CurrentAuthorization`, `oauth2Authorization`, `oauth2TokenExchange`, `oauth2TokenRevocation` |
| Application commands (+ permissions) | 6 | 16 | `applicationCommands`, `applicationCommand`, `applicationGuildCommands`, `applicationGuildCommand`, `guildApplicationCommandsPermissions`, `applicationCommandPermissions` |
| Interactions | 1 | 1 | `interactionCallback` |
| Application (self, role-connection metadata, activity instances) | 3 | 5 | `currentApplication`, `applicationRoleConnectionMetadata`, `applicationActivityInstance` |
| Monetization (entitlements, SKUs, subscriptions) | 6 | 8 | `entitlements`, `entitlement`, `consumeEntitlement`, `skus`, `skuSubscriptions`, `skuSubscription` |
| Application identities | 4 | 5 | `applicationUserIdentityProfile`, `userApplicationIdentities`, `applicationIdentities`, `userApplicationIdentityDelete` |
| **Total** | **129** | **232** | |

## Appendix: every `Routes` entry with its documented operations

Extracted from the doc comments in `dat/rest/v10/index.ts`.

- `applicationRoleConnectionMetadata`: `GET /applications/{application.id}/role-connections/metadata`; `PUT /applications/{application.id}/role-connections/metadata`
- `guildAutoModerationRules`: `GET /guilds/{guild.id}/auto-moderation/rules`; `POST /guilds/{guild.id}/auto-moderation/rules`
- `guildAutoModerationRule`: `GET /guilds/{guild.id}/auto-moderation/rules/{rule.id}`; `PATCH /guilds/{guild.id}/auto-moderation/rules/{rule.id}`; `DELETE /guilds/{guild.id}/auto-moderation/rules/{rule.id}`
- `guildAuditLog`: `GET /guilds/{guild.id}/audit-logs`
- `channel`: `GET /channels/{channel.id}`; `PATCH /channels/{channel.id}`; `DELETE /channels/{channel.id}`
- `channelMessages`: `GET /channels/{channel.id}/messages`; `POST /channels/{channel.id}/messages`
- `channelMessage`: `GET /channels/{channel.id}/messages/{message.id}`; `PATCH /channels/{channel.id}/messages/{message.id}`; `DELETE /channels/{channel.id}/messages/{message.id}`
- `channelMessageCrosspost`: `POST /channels/{channel.id}/messages/{message.id}/crosspost`
- `channelMessageOwnReaction`: `PUT /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/@me`; `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/@me`
- `channelMessageUserReaction`: `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/{user.id}`
- `channelMessageReaction`: `GET /channels/{channel.id}/messages/{message.id}/reactions/{emoji}`; `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}`
- `channelMessageAllReactions`: `DELETE /channels/{channel.id}/messages/{message.id}/reactions`
- `channelBulkDelete`: `POST /channels/{channel.id}/messages/bulk-delete`
- `channelPermission`: `PUT /channels/{channel.id}/permissions/{overwrite.id}`; `DELETE /channels/{channel.id}/permissions/{overwrite.id}`
- `channelPermissionsMember`: `GET /channels/{channel.id}/permissions/member/{user.id}`
- `channelPermissionsRole`: `GET /channels/{channel.id}/permissions/role/{role.id}`
- `channelInvites`: `GET /channels/{channel.id}/invites`; `POST /channels/{channel.id}/invites`
- `channelFollowers`: `POST /channels/{channel.id}/followers`
- `channelTyping`: `POST /channels/{channel.id}/typing`
- `channelMessagesPins`: `GET /channels/{channel.id}/messages/pins`
- `channelMessagesPin`: `PUT /channels/{channel.id}/messages/pins/{message.id}`; `DELETE /channels/{channel.id}/messages/pins/{message.id}`
- `channelPins` (deprecated): `GET /channels/{channel.id}/pins`
- `channelPin` (deprecated): `PUT /channels/{channel.id}/pins/{message.id}`; `DELETE /channels/{channel.id}/pins/{message.id}`
- `channelRecipient`: `PUT /channels/{channel.id}/recipients/{user.id}`; `DELETE /channels/{channel.id}/recipients/{user.id}`
- `channelVoiceStatus`: `PUT /channels/{channel.id}/voice-status`
- `guildEmojis`: `GET /guilds/{guild.id}/emojis`; `POST /guilds/{guild.id}/emojis`
- `guildEmoji`: `GET /guilds/{guild.id}/emojis/{emoji.id}`; `PATCH /guilds/{guild.id}/emojis/{emoji.id}`; `DELETE /guilds/{guild.id}/emojis/{emoji.id}`
- `guilds` (deprecated): `POST /guilds`
- `guild`: `GET /guilds/{guild.id}`; `PATCH /guilds/{guild.id}`; `DELETE /guilds/{guild.id}`
- `guildPreview`: `GET /guilds/{guild.id}/preview`
- `guildChannels`: `GET /guilds/{guild.id}/channels`; `POST /guilds/{guild.id}/channels`; `PATCH /guilds/{guild.id}/channels`
- `guildMember`: `GET /guilds/{guild.id}/members/{user.id}`; `PUT /guilds/{guild.id}/members/{user.id}`; `PATCH /guilds/{guild.id}/members/@me`; `PATCH /guilds/{guild.id}/members/{user.id}`; `DELETE /guilds/{guild.id}/members/{user.id}`
- `guildPermissionsMember`: `GET /guilds/{guild.id}/permissions/member/{user.id}`
- `guildMembers`: `GET /guilds/{guild.id}/members`
- `guildMembersSearch`: `GET /guilds/{guild.id}/members/search`
- `guildMessagesSearch`: `GET /guilds/{guild.id}/messages/search`
- `guildCurrentMemberNickname` (deprecated): `PATCH /guilds/{guild.id}/members/@me/nick`
- `guildMemberRole`: `PUT /guilds/{guild.id}/members/{user.id}/roles/{role.id}`; `DELETE /guilds/{guild.id}/members/{user.id}/roles/{role.id}`
- `guildMFA` (deprecated): `POST /guilds/{guild.id}/mfa`
- `guildBans`: `GET /guilds/{guild.id}/bans`
- `guildBan`: `GET /guilds/{guild.id}/bans/{user.id}`; `PUT /guilds/{guild.id}/bans/{user.id}`; `DELETE /guilds/{guild.id}/bans/{user.id}`
- `guildRoles`: `GET /guilds/{guild.id}/roles`; `POST /guilds/{guild.id}/roles`; `PATCH /guilds/{guild.id}/roles`
- `guildRole`: `GET /guilds/{guild.id}/roles/{role.id}`; `PATCH /guilds/{guild.id}/roles/{role.id}`; `DELETE /guilds/{guild.id}/roles/{role.id}`
- `guildPermissionsRole`: `GET /guilds/{guild.id}/permissions/role/{role.id}`
- `guildRoleMemberCounts`: `GET /guilds/{guild.id}/roles/member-counts`
- `guildPrune`: `GET /guilds/{guild.id}/prune`; `POST /guilds/{guild.id}/prune`
- `guildVoiceRegions`: `GET /guilds/{guild.id}/regions`
- `guildInvites`: `GET /guilds/{guild.id}/invites`
- `guildIntegrations`: `GET /guilds/{guild.id}/integrations`
- `guildIntegration`: `DELETE /guilds/{guild.id}/integrations/{integration.id}`
- `guildWidgetSettings`: `GET /guilds/{guild.id}/widget`; `PATCH /guilds/{guild.id}/widget`
- `guildWidgetJSON`: `GET /guilds/{guild.id}/widget.json`
- `guildVanityUrl`: `GET /guilds/{guild.id}/vanity-url`
- `guildWidgetImage`: `GET /guilds/{guild.id}/widget.png`
- `invite`: `GET /invites/{invite.code}`; `DELETE /invites/{invite.code}`
- `inviteTargetUsers`: `GET /invites/{invite.code}/target-users`; `PUT /invites/{invite.code}/target-users`
- `inviteTargetUsersJobStatus`: `GET /invites/{invite.code}/target-users/job-status`
- `template`: `GET /guilds/templates/{template.code}`; `POST /guilds/templates/{template.code}`
- `guildTemplates`: `GET /guilds/{guild.id}/templates`; `POST /guilds/{guild.id}/templates`
- `guildTemplate`: `PUT /guilds/{guild.id}/templates/{template.code}`; `PATCH /guilds/{guild.id}/templates/{template.code}`; `DELETE /guilds/{guild.id}/templates/{template.code}`
- `pollAnswerVoters`: `GET /channels/{channel.id}/polls/{message.id}/answers/{answer_id}`
- `expirePoll`: `POST /channels/{channel.id}/polls/{message.id}/expire`
- `threads`: `POST /channels/{channel.id}/threads`; `POST /channels/{channel.id}/messages/{message.id}/threads`
- `guildActiveThreads`: `GET /guilds/{guild.id}/threads/active`
- `channelThreads`: `GET /channels/{channel.id}/threads/archived/public`; `GET /channels/{channel.id}/threads/archived/private`
- `channelJoinedArchivedThreads`: `GET /channels/{channel.id}/users/@me/threads/archived/private`
- `threadMembers`: `GET /channels/{thread.id}/thread-members`; `GET /channels/{thread.id}/thread-members/{user.id}`; `PUT /channels/{thread.id}/thread-members/@me`; `PUT /channels/{thread.id}/thread-members/{user.id}`; `DELETE /channels/{thread.id}/thread-members/@me`; `DELETE /channels/{thread.id}/thread-members/{user.id}`
- `user`: `GET /users/@me`; `GET /users/{user.id}`; `PATCH /users/@me`
- `userApplicationRoleConnection`: `GET /users/@me/applications/{application.id}/role-connection`; `PUT /users/@me/applications/{application.id}/role-connection`; `DELETE /users/@me/applications/{application.id}/role-connection`
- `userGuilds`: `GET /users/@me/guilds`
- `userGuildMember`: `GET /users/@me/guilds/{guild.id}/member`
- `userGuild`: `DELETE /users/@me/guilds/{guild.id}`
- `userChannels`: `POST /users/@me/channels`
- `userConnections`: `GET /users/@me/connections`
- `voiceRegions`: `GET /voice/regions`
- `channelWebhooks`: `GET /channels/{channel.id}/webhooks`; `POST /channels/{channel.id}/webhooks`
- `guildWebhooks`: `GET /guilds/{guild.id}/webhooks`
- `webhook`: `GET /webhooks/{webhook.id}`; `GET /webhooks/{webhook.id}/{webhook.token}`; `PATCH /webhooks/{webhook.id}`; `PATCH /webhooks/{webhook.id}/{webhook.token}`; `DELETE /webhooks/{webhook.id}`; `DELETE /webhooks/{webhook.id}/{webhook.token}`; `POST /webhooks/{webhook.id}/{webhook.token}`; `POST /webhooks/{application.id}/{interaction.token}`
- `webhookMessage`: `GET /webhooks/{webhook.id}/{webhook.token}/messages/@original`; `GET /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`; `PATCH /webhooks/{webhook.id}/{webhook.token}/messages/@original`; `PATCH /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`; `DELETE /webhooks/{webhook.id}/{webhook.token}/messages/@original`; `DELETE /webhooks/{webhook.id}/{webhook.token}/messages/{message.id}`; `PATCH /webhooks/{application.id}/{interaction.token}/messages/@original`; `PATCH /webhooks/{application.id}/{interaction.token}/messages/{message.id}`; `DELETE /webhooks/{application.id}/{interaction.token}/messages/{message.id}`
- `webhookPlatform`: `POST /webhooks/{webhook.id}/{webhook.token}/github`; `POST /webhooks/{webhook.id}/{webhook.token}/slack`
- `gateway`: `GET /gateway`
- `gatewayBot`: `GET /gateway/bot`
- `oauth2CurrentApplication`: `GET /oauth2/applications/@me`
- `oauth2CurrentAuthorization`: `GET /oauth2/@me`
- `oauth2Authorization`: `GET /oauth2/authorize`
- `oauth2TokenExchange`: `POST /oauth2/token`
- `oauth2TokenRevocation`: `POST /oauth2/token/revoke`
- `applicationCommands`: `GET /applications/{application.id}/commands`; `PUT /applications/{application.id}/commands`; `POST /applications/{application.id}/commands`
- `applicationCommand`: `GET /applications/{application.id}/commands/{command.id}`; `PATCH /applications/{application.id}/commands/{command.id}`; `DELETE /applications/{application.id}/commands/{command.id}`
- `applicationGuildCommands`: `GET /applications/{application.id}/guilds/{guild.id}/commands`; `PUT /applications/{application.id}/guilds/{guild.id}/commands`; `POST /applications/{application.id}/guilds/{guild.id}/commands`
- `applicationGuildCommand`: `GET /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`; `PATCH /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`; `DELETE /applications/{application.id}/guilds/{guild.id}/commands/{command.id}`
- `interactionCallback`: `POST /interactions/{interaction.id}/{interaction.token}/callback`
- `guildMemberVerification`: `GET /guilds/{guild.id}/member-verification`; `PATCH /guilds/{guild.id}/member-verification`
- `guildVoiceState`: `GET /guilds/{guild.id}/voice-states/@me`; `GET /guilds/{guild.id}/voice-states/{user.id}`; `PATCH /guilds/{guild.id}/voice-states/@me`; `PATCH /guilds/{guild.id}/voice-states/{user.id}`
- `guildApplicationCommandsPermissions`: `GET /applications/{application.id}/guilds/{guild.id}/commands/permissions`; `PUT /applications/{application.id}/guilds/{guild.id}/commands/permissions`
- `applicationCommandPermissions`: `GET /applications/{application.id}/guilds/{guild.id}/commands/{command.id}/permissions`; `PUT /applications/{application.id}/guilds/{guild.id}/commands/{command.id}/permissions`
- `guildWelcomeScreen`: `GET /guilds/{guild.id}/welcome-screen`; `PATCH /guilds/{guild.id}/welcome-screen`
- `stageInstances`: `POST /stage-instances`
- `stageInstance`: `GET /stage-instances/{channel.id}`; `PATCH /stage-instances/{channel.id}`; `DELETE /stage-instances/{channel.id}`
- `sticker`: `GET /stickers/{sticker.id}`
- `stickerPacks`: `GET /sticker-packs`
- `stickerPack`: `GET /sticker-packs/{pack.id}`
- `nitroStickerPacks` (deprecated): `GET /sticker-packs`
- `guildStickers`: `GET /guilds/{guild.id}/stickers`; `POST /guilds/{guild.id}/stickers`
- `guildSticker`: `GET /guilds/{guild.id}/stickers/{sticker.id}`; `PATCH /guilds/{guild.id}/stickers/{sticker.id}`; `DELETE /guilds/{guild.id}/stickers/{sticker.id}`
- `guildScheduledEvents`: `GET /guilds/{guild.id}/scheduled-events`; `POST /guilds/{guild.id}/scheduled-events`
- `guildScheduledEvent`: `GET /guilds/{guild.id}/scheduled-events/{guildScheduledEvent.id}`; `PATCH /guilds/{guild.id}/scheduled-events/{guildScheduledEvent.id}`; `DELETE /guilds/{guild.id}/scheduled-events/{guildScheduledEvent.id}`
- `guildScheduledEventUsers`: `GET /guilds/{guild.id}/scheduled-events/{guildScheduledEvent.id}/users`
- `guildOnboarding`: `GET /guilds/{guild.id}/onboarding`; `PUT /guilds/{guild.id}/onboarding`
- `guildIncidentActions`: `PUT /guilds/${guild.id}/incident-actions`
- `currentApplication`: `GET /applications/@me`; `PATCH /applications/@me`
- `applicationActivityInstance`: `GET /applications/{application.id}/activity-instances/{instance_id}`
- `entitlements`: `GET /applications/{application.id}/entitlements`; `POST /applications/{application.id}/entitlements`
- `entitlement`: `GET /applications/{application.id}/entitlements/{entitlement.id}`; `DELETE /applications/{application.id}/entitlements/{entitlement.id}`
- `skus`: `GET /applications/{application.id}/skus`
- `guildBulkBan`: `POST /guilds/{guild.id}/bulk-ban`
- `consumeEntitlement`: `POST /applications/{application.id}/entitlements/{entitlement.id}/consume`
- `applicationEmojis`: `GET /applications/{application.id}/emojis`; `POST /applications/{application.id}/emojis`
- `applicationEmoji`: `GET /applications/{application.id}/emojis/{emoji.id}`; `PATCH /applications/{application.id}/emojis/{emoji.id}`; `DELETE /applications/{application.id}/emojis/{emoji.id}`
- `skuSubscriptions`: `GET /skus/{sku.id}/subscriptions`
- `skuSubscription`: `GET /skus/{sku.id}/subscriptions/{subscription.id}`
- `sendSoundboardSound`: `POST /channels/{channel.id}/send-soundboard-sound`
- `soundboardDefaultSounds`: `GET /soundboard-default-sounds`
- `guildSoundboardSounds`: `GET /guilds/{guild.id}/soundboard-sounds`; `POST /guilds/{guild.id}/soundboard-sounds`
- `guildSoundboardSound`: `GET /guilds/{guild.id}/soundboard-sounds/{sound.id}`; `PATCH /guilds/{guild.id}/soundboard-sounds/{sound.id}`; `DELETE /guilds/{guild.id}/soundboard-sounds/{sound.id}`
- `applicationUserIdentityProfile`: `GET /applications/{application.id}/users/{user.id}/identities/{provider_issued_user_id}/profile`; `PATCH /applications/{application.id}/users/{user.id}/identities/{provider_issued_user_id}/profile`
- `userApplicationIdentities`: `GET /users/{user.id}/application-identities/{application.id}`
- `applicationIdentities`: `GET /applications/{application.id}/application-identities/{provider_type}/{provider_issued_user_id}`
- `userApplicationIdentityDelete`: `POST /users/{user.id}/application-identities/{application.id}/{provider_type}/{provider_issued_user_id}/delete`
