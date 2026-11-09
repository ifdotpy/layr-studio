# Sync protocols

This document defines Studio's change notifications and durable SSE delivery.
The renderer and server use protocol 4 for live notifications. Unsupported and
unspecified stream protocol selectors receive HTTP 426. Pull projection schemas
remain separate from the stream transport version.

## Rule: no polling, only SSE events

This section owns the rule; other documents link here. The server pushes all
client data over SSE. Stream subscriptions deliver both the initial data and
subsequent changes.

The client starts an explicit HTTP request only for an explicit user action, a
lagging revision announced by the stream, or a link too slow for SSE. Startup
fetch chains, focus probes, periodic refresh, and automatic mark-read are
forbidden.

Local storage and IndexedDB may contain device keys, credentials and pairing
secrets, the monitor input journal, outbox request identities, unsent user input (drafts,
reply and answer text, form and queue edits, pending uploads, and unsent
recordings) until the server confirms it, and the remote frontend's subscription
cache. Secret answer fields remain in memory only. The cache is never used to decide whether to send or retry a command.

A remote frontend stores one IndexedDB record per cacheable subscription, with
its data and cursor together. It writes at most once per five seconds and when
the page becomes hidden. The server decides locality and cacheability; local
frontends and subscriptions without provable row ownership start with snapshots
after a reload. All subscriptions may resume from an in-memory cursor after a
stream reconnect. A corrupt or unknown record is discarded and recovered with a snapshot. A browser
on the server machine that pairs as a remote device is treated as remote.

Drafts, uploads, recordings, and unconfirmed sends share one IndexedDB input
buffer. Input is written immediately and removed on server confirmation. Old
RxDB unsent messages are intentionally not migrated.

Message send receipts and queue projections use the Rust-owned `MessageSendStatus`
contract generated into the web API. An uncertain result keeps its request identity. An expired receipt retains the
message on the device and cannot be retried with that identity.

### React network boundary

Root `.oxlintrc.json` owns the production boundary in
`workspaces/client/apps/web/src` (TypeScript and JavaScript): React components
and hooks cannot import API/transport capabilities or use browser `fetch`,
`EventSource`, `XMLHttpRequest`, `WebSocket` or `sendBeacon` directly.
Type-only imports, generated facades and named pure API error/storage helpers
remain allowed. Tests, fixtures and stories are outside this production boundary.

Permanent exemptions are `store/`, `sync/`, the generated observable contract,
and the exact plain class, command, credential, routing, stream and telemetry
owners listed in the configuration. Files named `use*` inside `store/` and
`sync/` remain subject to the boundary. New exemptions require review; naming a
file Owner does not grant access. Commands retain explicit action intent and
request identity; observation only acquires existing store/SSE interest.
Exempt infrastructure must not re-export transport capabilities to React callers.

The final configuration override is the sorted migration allowlist, based on
`4b6016b749bf701691bc01a1bb24f87a`. Its single reason manifest is
[network-boundary-exceptions.json](../workspaces/tooling/apps/repository-checks/tests/network-boundary-exceptions.json).
Exceptions disable only import restrictions, so globals and beacon access remain
forbidden. The short-tier `network-boundary.test.ts` checks the manifest/config
sets, committed file-count ceiling and actual restricted-import diagnostics
without the final override. Remove each exception and its manifest reason when
its restricted imports disappear, and lower the ceiling when pruning.

Known limits kept for review:

- Static import/global rules cannot cheaply detect network-starting owner methods
  called inside `useEffect`/`useLayoutEffect`.
- `useServerAreas().scope.services.api` remains reachable; narrowing the
  React-facing ServerAreas context belongs to the next migration wave.
- The pinned Oxlint rules do not restrict dynamic `import()`, so production
  dynamic imports require review for transport access.
- Native bridge capabilities remain allowed and require review for network use:
  `nativeRequest("servers.request", ...)` from `native/client`, `frameRequest`
  from `native/frame`, and `nativeHost().core.invoke` can bypass the import boundary.
- Global aliases and destructuring are not followed; review indirect browser
  network access, while ordinary asset loads through Image/src remain allowed.
- Equal-count exception swaps are not detected by the ceiling; they change both
  the configuration and reason manifest and must be rejected in review.

## Routes and reconnect

The web client keeps an ordered route list for each paired server. The paired
origin is first. The `session` event of every stream lists the origins the server
answers on (`routes`). The client learns them and drops the ones that are no
longer listed. A key paired over HTTPS never goes over plain HTTP.

- **Identity first.** Before a learned route gets a signed request, the client
  sends a fresh nonce to `/api/multi-server/v1/identity?nonce=...` without a key.
  The server signs the nonce with its multi-server identity key. The client
  verifies the signature with the server key stored at pairing. A route with the
  right `serverId` that cannot sign gets nothing. The HTTPS paired origin needs no
  proof (TLS binds it to its host name). The nonce size owner is
  `IDENTITY_PROOF_NONCE_BYTES` in `multi_server_protocol.rs`.
- **Faster route.** While a later route is in use, the client checks the earlier
  routes on network change, on foreground, and every minute. A failed route waits
  for a cooldown. A move replaces the stream.
- **One retry owner.** The stream coordinator retries with jittered exponential
  backoff, capped at 5 minutes. The count resets after the stream stays up for
  one minute. Offline and refused (401, 403) connections wait for a wakeup.
- **Probe, then replace.** On foreground or network change the client sends an
  empty subscription change and waits for its acknowledgement on the stream.
  Only a missing answer within 5 seconds replaces the stream.

Owners: `servers/routes.ts` (route constants and list), `sync/resourceConnection.ts`
(retry and probe constants), `sync/resourceEvents.ts` (the single retry loop).

## Subscriptions follow the screen

This section owns the rule: a mounted view subscribes to the resources it displays.
Identical typed references share one subscription across components and cooperating
tabs. The [subscription module](../workspaces/client/apps/web/src/sync/subscriptions/index.ts)
exposes `useSubscription(ref, { graceMs })` and domain data with loading, live, stale
or error status. It owns the [grace interval in milliseconds](../workspaces/client/apps/web/src/sync/subscriptions/contracts.ts),
reference counting and transport integration. Returning to a recently viewed chat
within grace reuses its interest without a subscription change. A resource with
no mounted reader is stale, including while its grace interval runs. Offscreen
chats retain cached history without opening a subscription to keep it warm.

The [OpenAPI document](../workspaces/runtime/packages/runtime-http/openapi.json)
owns `ResourceRef` and `SubscriptionDataMap`; Rust and TypeScript consume generated
types. Diagnostic keys are derived from references and are never parsed back.
The [server registry](../workspaces/runtime/packages/runtime-http/src/routes/subscriptions.rs)
owns each resource kind's initial snapshot, change filter, encoded byte bound and
diagnostic key. A parity witness requires exactly one handler and data contract
for every generated resource kind. Business services publish domain changes
without tracking sessions or subscribers.

Draft subscriptions include a typed snapshot on open and reopen. Its revision
is the draft sequence read with its rows, so later stream batches continue from
that point. The snapshot is bounded by the shared [draft row and stream byte
limits](../workspaces/runtime/packages/runtime-schema/src/shared_values.rs); its
`complete` flag is false when either limit truncates it.
Legacy drafts above the current text limit are refused visibly while other drafts remain available.
Pre-upgrade command retries retain pre-upgrade semantics, including cross-owner identity behavior, and never convert text to a file.

Large baselines and deltas use `entity-transfer` frames on the same connection.
The required `payloadKind` enum is `entity`, `transcript`, or `view`. An entity
page is a canonical `EntityChangeBatch` within the shared
[document and complete-event byte limits](../workspaces/runtime/packages/runtime-schema/src/shared_values.rs).
The server validates pages and spools the interval to an anonymous temporary file
from one SQLite read snapshot. It splits at record boundaries first; serialization
fragments preserve a single existing oversized record. A complete marker proves
the final interval, without constructing a batch above the canonical document limit.
Transcript and view transfers serialize one complete `ResourceRevisionEntry` as
page zero, split by the same `partIndex` and `partCount` string fragmentation.
Their resource, cursor, and reply kind match the envelope; only the final part
sets `complete=true`. Protocol 4 is unchanged; the schema hash fences the new field.

The fetch stream stages transfer pages in memory before reading more data.
Only a complete interval enters the per-database projection application queue;
all scope checks finish before the first write, snapshot pruning follows complete
staging, and the durable cursor is written last. Interrupted staging leaves the
previous projection and cursor intact. A write failure retains the old cursor so replay
can finish idempotently. Closing the page discards incomplete staging. The normal small event path stays direct,
and per-scope cursors send only changed records after a baseline. Internal pages
retain the initial SQLite snapshot's recovery decision even when their checkpoints
predate the tombstone floor. Source-owned visibility fences in the existing
[store bootstrap](../workspaces/runtime/packages/runtime-store/src/sync_schema.sql)
record predicate-field movements for affected direct selectors and team roots.
Only a relevant movement requests a replacement; status updates, ordinary history
appends and tombstones retain row deltas. The indexed fence lookup does not scan
selected histories.

One queue per server, workspace, and schema orders interval application and cache
hydration. Typed row batches compare source sequences synchronously and publish
after the whole batch; they use no optimistic storage adapters or CAS retries.
A late cache record cannot replace a completed live scope. The subscription cache
captures data and cursor together through that queue and writes one IndexedDB
record. Recovery reopens SSE from the last applied cursor; a missing or expired
cursor receives a snapshot. Entity, transcript, and view delivery never starts
an automatic HTTP recovery read.

Entity subscriptions read only rows after their last delivered sequence through
the `(collection, seq)` index, including sequence-bearing deletion tombstones.
Empty live intervals stay suppressed without advancing the delivered cursor;
reconnect still acknowledges the client's durable cursor with an empty delta.
No selected collection is rehashed. The explicit exception is an ownership or
team-membership movement detected by the indexed visibility fence: it replaces
that affected scope so entities that left it are removed from the client.
Missing or expired cursors likewise require a replacement baseline.

Transcript history uses a typed `transcript` reference with an optional `tail`
window. Its [default item count](../workspaces/runtime/packages/runtime-schema/src/shared_values.rs)
is generated for the renderer and checked against OpenAPI. The subscription
baseline contains the newest window and its older-history cursor. Oversized
windows and individual items use the existing transfer fragmentation. A reconnect delta preserves loaded older history. A snapshot replaces the
window, including after a reset; older rows are restored only by explicit paging.

Scrolling to older history uses the [explicit user action exception](#rule-no-polling-only-sse-events):
one bounded `/api/transcript/page` read starts before the retained boundary.
The renderer merges pages by item identity into the current conversation range,
retaining the scroll anchor and both paging cursors. Live deltas preserve that
boundary even when the shared transcript projection covers a larger range.

Transcript item hashes stay in the server's private change-detection index; sparse
deltas carry changed items, removals and optional order without an item revision
map. The [renderer metadata format](../workspaces/client/apps/web/src/sync/client.ts)
reads the previous persisted format and drops its unused map on the next write.
The [API schema identity](../workspaces/runtime/packages/runtime-schema/src/api_schema.rs)
changes with the wire contract. Draft and outbox storage keep their workspace identity.

The old RxDB migration is removed. A browser that did not open Studio after the
migration was introduced cannot recover unresolved sends left in that old store.

The browser keeps each pending message command and workspace identity in memory
and writes them to the unsent input buffer while dispatching the POST. A reconnect
or explicit retry reuses the exact command; confirmation or definitive refusal
removes the buffered command.
A damaged entry can be discarded without blocking other messages. The retired
`studio-thin-cache` database is deleted on startup. Its unsent messages and drafts
are not migrated.

## Protocol 4: resource changes

User actions, media transport, and recovery of an already accepted write retain
their own request identities and lifecycles.

### Authoritative types

The committed [OpenAPI document](../workspaces/runtime/packages/runtime-http/openapi.json)
owns the closed resource union and named event envelopes.
`pnpm run contracts:generate` emits TypeScript declarations and the SHA-256 identity of
the canonical OpenAPI JSON. `pnpm run contracts:check` rejects stale output. API
responses include the server identity. Hash-bearing protocol-4 connections begin
with an `api-schema` event; mismatches get only that handshake before close.
Hashless non-renderer clients retain the existing event sequence. Mutating
requests with a present mismatching hash receive a marked 426. A mismatch stops
resource and draft synchronization and message delivery until the renderer
updates. Stream payloads are not walked against generated
runtime validators; handlers retain structural preconditions and workspace,
epoch, revision, and ordering checks.

### Durable change source

The [durable source](../workspaces/runtime/packages/runtime-store/src/durable_seq.rs)
gives each row of the main tables a change number. `durable_rows` holds
`(table, id, seq, deleted)`; a trigger on each opted-in table sets `seq` on insert and
update, and a delete turns the entry into a tombstone. The main tables get no new column,
so writers, fixtures and migrations do not change. `DURABLE_TABLES` lists the opted-in
tables (`runtime_agents`, `runtime_requests`, `runtime_work`); a new table opts in with
one line there.

The numbers are the counter of `sync_entities`, `sync_documents` and `sync_generation`:
the next number is one above the highest of those three and `durable_rows`. Every
allocator uses that rule, so there is one cursor space and a cursor from any of these
sources is valid for all of them.

`changes_since(table, cursor)` returns the rows with `seq` above the cursor and the
tombstones above it, or a snapshot when the cursor is absent, in the future, or below
`durable_floor.floor`. The floor is the highest `seq` of a pruned tombstone.
Tombstones live `TOMBSTONE_RETENTION_SECONDS` (90 days); a delete prunes older ones.

Status: every entity reader (stream, scoped subscription, REST pull, mutation reply)
reads through [`entity_scope`](../workspaces/runtime/packages/runtime-services/src/entity_scope.rs).
It builds the work documents (`WORK_COLLECTIONS`: work, workDetail, workResult,
workDecision, workRelease) from `runtime_work`, its history tables and `durable_rows` at
read time. They have no copy in `sync_entities`; migration `2026-11-04-work-read-time`
deleted the old copies. The direct collections (`DIRECT_COLLECTIONS`) have one document
per row of a main table: `plan` (`runtime_plans`), `annotation` (`runtime_annotations`),
`rule` (`runtime_rules`), `request` and `questionHistory` (`approvals`), `complaint`
(`runtime_complaints`, with the current names of its author and lead) and `edge`
(chat rows of `graph_edges`). A `chat` document reads its group, its member edges and
its messages; their writers mark the chat (`durable_rows` table `chat_views`), so a
member change or a new message refreshes it. Migration `2026-11-04-chat-read-time`
numbered these rows, marked every chat and deleted the copies. The task and monitor windows
(`WINDOW_COLLECTIONS`) keep only their members in `durable_rows` (`task_window`,
`monitor_window`); the writer that owns the window rule joins and leaves members, and a
member's document is built from `runtime_tasks` or `runtime_monitors` at read time. A
change of a member row or of its membership is sent at the later number; a member that
left is a delete. `task_window_state` keeps the owner, created time and running state the
task window compares. A task that moves to another agent resets the scoped readers of
both owners. Migration `2026-11-04-window-read-time` moved the members of the copies. They are built
at read time too; migration `2026-11-04-direct-read-time` numbered their rows and deleted
their copies. The agent views (`AGENT_VIEW_COLLECTIONS`: agent, agentSummary,
agentDetail, retainedWorktree) are built at read time from `runtime_agents`. A write that
changes a view without a change of the agent row (team counts, attention, command
progress, account state) writes a mark: a `durable_rows` entry with table `agent_views`
and no payload. A view takes the higher number of the agent row and its mark.
The separate read-time `agentTail` collection contains only lead identities and
bounded committed tails. Its changes use entries for the existing
`runtime_agent_volatile` source table in the same durable cursor. Tail-only flushes
never mark agent views or recompute operational facts. Worker tails are excluded.
Operational reasons in `agentSummary` use four indexed owner reads and a single
bottom-up request-local pass; projects, peer teams and saved order remain their
own entities. There is no sidebar resource, stored projection, extra logical mark
namespace or transaction mark-proof cache. Team
visibility of agent-scoped documents reads `root_id` of `runtime_agents`; the agent row
triggers move the team fence. Migration `2026-11-04-agent-views-read-time` deleted the
copies and the view log. An approval that is not open (or not a question) is a delete in a delta and
absent from a snapshot. An update that leaves a record-only row unchanged takes no new
number. A document built from one main-table change carries that row's number, so
several documents can share a number; a page is cut only between numbers. The other
collections still come from `sync_entities`.

### Transcript pages at read time

`runtime_items` is a durable table: its triggers keep a `durable_rows` entry for each item.
Migration `2026-10-30-transcript-read` installs them. It does not number the old rows,
because the live table has about 1.5 million of them. The online migration
`runtime_items_seq` (`transcript_seq_backfill`, started at every boot, see
[online migrations](online-migrations.md)) numbers them in short batches. When it
completes, it runs `transcript_copy_missing_from_owner`. Only when that returns nothing does
it set `sync_entity_meta` key `transcript_copies` to `retired`. When an item is missing, the
key stays unset and the writer keeps the transcript copies in `sync_entities`. A state
without items retires the copies in the migration itself. No migration deletes the old rows.

With the key set, `pull_scope` writes nothing. A conversation's change number is the
highest `seq` of its agent row, its items, and any deleted item or agent. The page is
built from `runtime_items` for each pull that is newer than the cursor. The server remembers
the item hashes of the pages it sent (memory only, 256 pages). A cursor that matches a
remembered page gets a delta in the existing transcript frame format. Any other cursor gets
the full page. Pages are never cached in `sync_entities`.

The first frame of a stream with a chat open also carries the newest window of the
`SYNC_TRANSCRIPT_RECENT_CHATS` most recently changed other chats, as ordinary transcript
snapshots. They are not subscribed. A client that opens one of them already has its tail.

### Entity publication

The renderer reads chats and cards only from their `sync_entities` rows, and a
pull does not rebuild a complete projection. Every writer transaction therefore
publishes the views of the `runtime_agents` and `approvals` rows it changed before it
commits. A changed `runtime_work` row refreshes only its owner agent's views. The [store journal](../workspaces/runtime/packages/runtime-store/src/entity_publication.rs)
records each changed row, and the services publisher projects it through
`sync_policy::publish_entity_changes`. A write path needs no publish call of its
own. A path that publishes after its last write is not published twice.
Volatile agent fields are outside this rule: their writers leave
`runtime_agents` unchanged on purpose.

Visible Team rows retain an `agentDetail` subscription for command progress,
errors, and result excerpts; their summaries remain authoritative for status and
membership. Collapsed finished rows do not retain detail subscriptions.

The assignment Tasks view subscribes to its team's existing `work` resource
and the root agent's detail for `taskDefaultGate`. Its scoped `workDetail` metadata
and `workResult`, `workDecision`, and `workRelease` history records reconstruct the
canonical OpenAPI `WorkItem`. The global `work` summary excludes histories.
Each history record is a row of its history table with its own change number, so an
unchanged record keeps its number. A change of the owner agent, its execution run or a
dependency gives the affected `workDetail` documents the number of that change.
Visible single-agent detail observers select that entity before decoding rows.
Task actions retain their existing
request identities and receive their resulting view through the stream.

Retained cleanup folders have their own `retainedWorktree` entities, including
folders of deleted workers. Question answers remain in `questionHistory` entities
after their pending request disappears; secret answers are omitted from this
persistent projection. Both collections follow their source transaction and
hard-delete tombstones. A bounded transcript baseline uses `transcriptSnapshot`
in the subscription's initial resource frame, after its acknowledgement; the
renderer persists it before notifying transcript readers. Oversized frames
retain their full payload and cursor through transfer fragmentation. Saved Claude repair evidence is read on opening the
view or explicit refresh, never because an agent's status changed. Viewing a
focused completed result records its read receipt; its confirmed mutation
response does not cause another session or projection read. An empty local
sidebar order never creates a migration write; existing pending requests and
nonempty legacy user preferences retain their original migration lifecycle.

Rename completion uses the agent entity for its title and the transient
`rename-progress` resource for the exact request's receipt, including failure.
The selector includes both the agent and request identity; subscription baselines
replay the durable receipt after reconnect. Terminal receipts release the selector.
Saved server names consume each owner's discovery snapshot through its existing
stream. Startup credentials and workspace initialization share the finite
[read-recovery budget](../workspaces/client/apps/web/src/sync/readRetry.ts);
exhaustion stays visible until reconnect or an explicit retry. The visible transcript
uses stream snapshots and intervals, including fragmented oversized entries.

Open process and work-review panels and complaint threads receive their bounded
records in `viewSnapshot` through typed screen subscriptions. Room baselines
retain a newest-message window and an older-history cursor. Open terminal output
uses Unicode-scalar offsets. Each live subscription sends an initial snapshot,
then only output since its last sent cursor, including status updates with no new
text. Buffer loss, an output reset, or an oversized interval sends a bounded
replacement snapshot. Removing a terminal selector releases its live cursor.
The renderer applies every resource frame through one FIFO per stream source,
including hints, overflow frames, and completed staged intervals.
Transfer completion reserves its queue position on arrival; staging prerequisites
settle before the persistence deadline starts. Each frame is persisted before dispatch;
its reason does not select a persistence path. Terminal deltas declare
`terminalBaseOffset` in the OpenAPI-owned resource entry, in Unicode scalars.
The terminal cursor also carries the OpenAPI-owned `terminalGeneration` reset
counter. `TerminalsRepository` increments it whenever replacement output reduces
the output end offset. An additive migration initializes existing rows to zero.
A missing or mismatched generation on resume always produces a snapshot, even
when output has grown past the old offset after a reset. Under the projection
application queue, persistence compares both generation and base offset with the saved
cursor, then stores the merged output and cursor together. Entity and
transcript deltas use their existing `after` bases. Reconnect and ownership
changes read persisted cursors; they need no in-memory terminal baseline proof.
Persisted terminal revisions prevent late snapshots from restoring pre-reset output.

Entity and transcript data require a matching workspace cursor. The renderer
accepts generated typed payloads without structural DTO validation, retaining
workspace, epoch, revision, sequence, resource, and terminal-generation fences.
Persistence errors, deadlines, and exhausted queue capacity stop delivery and
request recovery for the affected resources. Subscription removal strips only
removed entries; source close silently drops pending frames. Raw server intervals
retain delta semantics.
Saved terminal history and explicit retries remain user actions.

The existing native EventSource transport and the in-memory projection cache
remain in use. Adding a separate query cache is unnecessary for this contract. The Rust HTTP adapter
owns SSE framing; the renderer and server compare the same generated schema
identity before continuing with live updates.

### Live text

Model text reaches the open chat before its durable item does. The account worker
buffers provider deltas and writes them to SQLite at most every 750 ms. After the
journal commit of a delta, the worker also sends it, in memory and without a timer,
to every sync stream. A stream forwards it as a `live-delta` event only when it has
the transcript of that agent subscribed. The event is not a resource frame: it has no
revision, no `Last-Event-ID`, and no replay.

- Fields (`ResourceLiveDeltaEvent` in OpenAPI): `agentId`, `itemId` (the durable
  transcript item id), `offset` (item text length in Unicode scalars before `text`),
  and `text`.
- Each stream has a bounded queue (256 events, `LIVE_DELTA_CAPACITY`). Queued deltas
  that continue each other are sent as one delta. A full queue is merged: the runs of
  each item join and equal progress events become one. The stream stays open. Only a
  queue that is still full after the merge drops its oldest event; the durable item
  carries that text.
- The renderer keeps live text in memory only. It shows `durable.slice(0, offset)` plus
  the live run while the durable item is shorter than the run and at least `offset`
  long. The durable item wins when it is at least as long. A gap in `offset` starts a
  new run.
- Only assistant message text is live. Reasoning text has no durable item, and tool
  output is a bounded tail that has no stable offset. Both stay on the durable path.
- The 200 ms spacing of resource and token-rate frames does not apply to this event.

### Subscription and notification

A same-origin EventSource requests `/api/sync/stream?protocol=4&resources=...`.
`resources` is a URL-encoded JSON array of generated typed references. After the
session handshake, `/api/sync/subscriptions` accepts typed additions and removals;
its generation acknowledgement precedes the added resources' initial data. The
connection stays open when interests change. Only the subscription module knows
about sessions, generations, acknowledgements, cursors or grace timers.

Named `resources` events carry the workspace, process epoch, monotonic stream
revision and bounded domain records. Entity changes use typed upserts and deletes.
Transcript subscriptions receive a bounded visible tail; older history remains an
explicit user action. Durable per-reference cursors describe fully applied local
records. An initial subscription can resume with a delta when its cursor remains
valid; a missing or expired cursor receives a fresh bounded snapshot. Transient
stream revisions never replace durable projection cursors. Snapshot records and
their cursor are read at the same database boundary.

The server installs a subscription and captures its initial state without a gap
that could lose a committed change. Producers notify only after a successful
mutation is visible. Rollbacks, no-op writes and projection-cache maintenance
must not produce a resource change. Completing an asynchronous refresh may
notify waiting consumers even when its data is unchanged, because its loading
or error state has settled. Fetching an unchanged projection must not recursively
invalidate that projection. Queues and pending references are
bounded; overflow requires explicit reconciliation rather than silent loss.

Each tab opens its own stream for its active subscriptions. Unused subscriptions
and their source watchers are released. Each stream retains versions for its
active references and at most 128 inactive references; an evicted reference
requires a fresh server baseline. Stream session identity is scoped to the
server connection and cleared when that connection closes.

### Durable per-ref resume cursors

The [OpenAPI subscription contract](../workspaces/runtime/packages/runtime-http/openapi.json)
defines `SubscriptionAdd`, `SubscriptionCursor`, and `SubscriptionReplyKind`.
Each added ref carries an optional `since: { workspaceId, seq }` cursor. The
stream's initial request carries the same entries in its `since` query parameter.
The server returns `replyKind: delta` and only records newer than the applied
cursor when retained history covers it. A current cursor returns an empty delta.
Missing, foreign, future, or expired cursors return `replyKind: snapshot` and
replace that ref's projection. The durable store sequence and records come from
the same SQLite boundary; stream epochs are independent of these cursors.

The client stores each cursor in its in-memory projection after its records are
applied, through the serialized projection application queue. A failed, cancelled,
or incomplete delivery cannot advance the cursor. Oversized deliveries advance
only after all fragments have been applied. Reconnect resends active refs with
these applied cursors through the same subscription path. A covered reconnect
requires no sync pull. Transient resources retain their existing in-memory
snapshot owners and do not create durable cursor rows.

The state resource resumes from the completed entity projection's existing
checkpoint, advanced by completed state subscription intervals.
Its ready marker identifies a replacement baseline; ordinary deltas preserve
that marker and unchanged agent objects. A delta may overlap already applied
records, but a missing base or forward gap cannot advance the checkpoint.

Subscribed resources own their initial delivery. A cold watcher waits for its
applied SSE baseline instead of racing it with an unconditional HTTP read.
Readiness belongs to the exact subscribed ref: a detail or transcript baseline
does not complete the state resource. The state completion marker proves only
the canonical state subscription scope, not every detail or historical entity.
An applied reconnect baseline can restore readiness even when its revision has
not changed. Identity, authentication, and schema checks retain their existing
boundaries.

Revision-only hints, gaps, overflow, and failed local application reopen the
stream with the last applied cursor, or without one when a reset is required.
A cold watcher waits for its SSE baseline through transport interruption.
Schema mismatch pauses synchronization through the existing mismatch latch.
Neither transport recovery nor subscription recovery starts an automatic entity,
transcript, or view HTTP read.

Cursorless process, complaint, and legacy-room views become ready only after
their current inline value is applied by the view adapter. A failed or discarded inline
entity application reconnects the tab stream from the durable per-ref cursor. Queued deltas
cannot confirm readiness after their base failed, and a cancelled subscription
cannot request recovery for its replacement.

The server may split a reconnect baseline into separate resource frames.
A same-epoch passive notification for another resource must not cancel a
pending entity write. The entity cursor becomes applied only after that write
commits; state resets, epoch changes, and failed application retain their
identity and cancellation fences while reopening SSE.

Project deletion uses the dedicated `POST /api/projects/delete` contract in
[OpenAPI](../workspaces/runtime/packages/runtime-http/openapi.json). The confirmation
captures the project entity's own `seq` and the applied baseline's `workspaceId`;
a stream revision or global cursor is not the project precondition. One canvas
transaction records project and chat deletion, permanent room retirement, managed
stop intents, and the exact-request receipt. The ordinary resource stream delivers
the resulting tombstones. A successful reply does not trigger a refresh.
An identical request recovers the committed deletion receipt even after re-registration;
its stop outcome reflects the current command state. Changed content or a stale
project precondition requires a new confirmation.
Membership follows parent links; a root identifier alone does not assign an
orphan to a project. An associated orphan without explicit ownership, or an
associated malformed ancestry, refuses the whole deletion. Project deletion
scopes failed-work release to its selected owners. Sidebar-only removal keeps its separate registration semantics. Outbound
location changes capture their checked project/workspace version and managed
actor epoch; both outbox reservation writers recheck those values before transport.
Fresh location reservations require that admission; only an exact existing
reservation can retain the legacy marker-less format and replay its saved outcome.
External stops and disposable-workspace cleanup follow commit through existing
durable owners. Project deletion retains transcripts, accounting, native history,
and source folders. It revokes archive restoration while preserving cleanup
already requested by an earlier archive action and its exact operation identity.

### Connection loss and recovery

Connection health, subscription readiness, and message delivery are separate
contracts. The renderer's
[connection state module](../workspaces/client/apps/web/src/sync/connectionState.ts)
owns transitions, activity, retry attempts, deadlines and reconnect reasons; the
[connection module](../workspaces/client/apps/web/src/sync/resourceConnection.ts)
owns lifecycle boundaries. The subscription store owns
whether a resource has an applied baseline; the outbox owns whether a message has
a server receipt. Neither stale resource data nor an unacknowledged message alone
means that the connection is lost.

Health and transport activity are derived views of that state. Failure reasons
describe observed boundaries, not an inferred remote
cause; an unclassified interruption is unknown. A validated server frame clears
the failure. The retry deadline is Unix epoch milliseconds belonging to the
actual retry timer and is cleared when that timer fires or is canceled. UI copy
consumes this detail without maintaining a second recovery state machine.

Only one stream-opening attempt may prepare durable cursors at a time.
Subscription changes coalesce with that attempt rather than
canceling it. Opening uses the current resource union. Late subscriptions read
their available cursors within the same original preparation budget, once per
selector; they never renew that budget. Resources without a prepared cursor
receive a baseline. Cursor preparation for opening and
subscription changes is bounded by the connection module's named preparation
budget. If optional cursor reads do not finish, the stream requests an
authoritative SSE baseline without a cursor. This is not an HTTP polling fallback
and does not bypass the durable application receipt. Closing the attempt cancels
its preparation; late results cannot reopen a superseded stream.

An intentionally hidden tab releases its stream with a `paused` connection state.
Returning to the tab resumes the stream without reporting that pause as a
network failure. A browser `offline` event immediately closes the local stream,
and cancels retry timers. An `online` event starts recovery
immediately, but cannot establish a live connection without fresh server evidence.
Transport errors and expired server liveness still trigger recovery.

Named `heartbeat` events prove liveness and never trigger resource reads.
Comment-only SSE heartbeats are insufficient for a browser silence timeout
because EventSource does not expose them to JavaScript. A silent, failed or
closed stream changes the visible connection state; reconnect attempts use
bounded increasing delay with jitter. Voice capture closes when the connection
becomes unavailable.

There is no periodic `/api/sync/generations` request and no polling fallback.
While the stream is unavailable, automatic state updates are unavailable.
A successful reconnection resumes each current subscription from its last fully
applied durable cursor or receives a bounded snapshot. It does not retry user
commands, messages, credit charges or other writes.

The voice and authentication-progress adapters retain targeted reads when
their SSE entry lacks the needed value. A transient failure may retry that same
outstanding read within the shared [read-recovery attempt budget](../workspaces/client/apps/web/src/sync/readRetry.ts),
with increasing backoff. The budget counts failed attempts, including the initial
attempt, for those targeted reads. Exhaustion reports a visible
read-recovery error and leaves no retry timer. Only a newer resource version or
unversioned notification, a transition back to a live stream, or an explicit user
retry starts a fresh budget. Duplicate or older versions, repeated live status,
and page resume do not replenish it. Retries pause while hidden or offline and
stop when the consumer unsubscribes. Successful reads do not schedule retries.
Draft storage bootstrap permits three delayed retries (1, 3 and 10 seconds) after
its initial attempt, then pauses until explicit activity or resume. Pending-write
recovery keeps its existing journal and operation identities. A replayed chat deletion reports the current stop confirmation from durable agent state.

An unchanged reconnect baseline still reconciles a subscribed consumer once.
Transcript subscriptions apply completed SSE intervals; opening a transcript does
not prefetch it through HTTP. Explicit older-history paging remains a bounded
user-action read and preserves the current streamed tail.

An epoch distinguishes a server process restart from an old event revision.
The client rejects malformed messages, incompatible protocol versions and
another workspace's events. Duplicate notifications within an epoch do not
cause repeated reads. Stream revisions are transient notification sequence
numbers; they are not durable projection cursors. Reconnect/reset uses scoped durable deltas or bounded snapshots, so missed
notifications do not require an unbounded stream event log.

Named `token-rates` events preserve live token-rate telemetry on the same stream.
They use the generated OpenAPI contract and update the existing renderer cache;
they do not cause a periodic limits or costs request.

### Notifications from other processes

Supported CLI writers publish their committed resource changes through the
[authenticated notification relay](../workspaces/runtime/apps/server/src/studio_api/sync/resources/relay/README.md).
`POST /api/sync/notify` accepts the OpenAPI-defined request identity and resource
references; the ordinary API token and request-boundary checks still apply.
The CLI verifies that the API state directory matches the source write's root
before sending a notification.

A bounded process-local receipt cache acknowledges an identical retry without
republishing it; reusing a retained identity with a different body returns 409.
After receipt eviction or server restart, repeating an invalidation is safe.
This is not durable exactly-once delivery. A lost or truncated response retries
only the same notification, never the original command or database write.
If notification delivery remains unconfirmed, the CLI reports that the source
change has committed. Reconnecting the stream obtains a full subscription
baseline. Arbitrary external SQL writes are not supported event sources.

### Worktree measurements

Worktree size notifications report completed measurements. The scanner wakes
on a request, rather than scanning on a timer. A cached size is not evidence
that arbitrary external filesystem writes have been detected: repository trees
are not recursively watched. The response's measurement
time identifies the age of the returned result.

### Read-only request audit

[`audit-live-requests.ts`](../workspaces/client/apps/web/scripts/audit-live-requests.ts)
measures idle and chat-switching traffic in headless Chromium, retaining request
counts, fetch stacks, CDP initiators and stream metadata outside the checkout.
It blocks every write except the audited tab's stream subscription updates:

```sh
node workspaces/client/apps/web/scripts/audit-live-requests.ts \
  --url http://127.0.0.1:4620 --chat FIRST_ID --chat SECOND_ID \
  --output "$HOME/.cache/codex-studio/request-audit.json"
```

Blocked writes remain attempted requests in the report. A blocked read receipt
can cause uncertainty-recovery pulls, which must be classified separately from
reads triggered by a live change.
