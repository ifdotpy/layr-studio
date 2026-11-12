# Short snapshot readers (#1002)

Base: Git `75cfb12c1bf4fc59c498c80a8b494a598c471731`, Layr
`ec5bb93eeb6e7669a42901a1cd591583`. All checks use disposable state.

## Reader and checkpoint review

`runtime-http/src/routes/entity_transfer.rs::prepare` previously kept one
`RuntimeStore::read_async` transaction around every SQL page, JSON conversion,
validation and temporary-file write. Client network writes already happened
after preparation, through `TransferReader`. Both initial snapshots and scoped
SSE catch-up use this path. REST entity pulls and the global state delta reader
already read one bounded page per `read_async` call.

The first page captures the sequence watermark. Each continuation uses that
same upper bound and the previous page's checkpoint, with a new read transaction.
Serialization and spool writes run on the blocking pool after the transaction
has ended. Mutable source projections may reflect writes after the watermark;
the baseline therefore has at-least-watermark semantics, followed by replay
from its advertised watermark, rather than point-in-time isolation.

Replay uses `entity:<collection>:<id>` keys (`sync/client.ts::entityCacheRow`)
and sequence-guarded replacement (`sync/localCollection.ts::applyBatch`), not
additive updates. Deletes become `_deleted` rows. Snapshot replacement prunes
missing scoped members (`sync/client.ts::persistScopedEntities`, with the same
rule in `persistStagedEntityTransfer`). The server's membership revision forces
a replacement snapshot after team insertion, deletion or restoration; ordinary
content changes and tombstones remain deltas. Window changes likewise emit
keyed upserts or tombstones. Aggregate DTOs are complete keyed values.

Client evidence (paths under `workspaces/client/apps/web/src`):
`sync/client.ts:84` constructs the keyed row and tombstone; `:574` converts
received changes; `:575` prunes absent snapshot members, with staged-transfer
pruning at `:753`. `sync/localCollection.ts:33` compares sequence numbers and
`:158` replaces the keyed value. Server evidence (paths under
`workspaces/runtime/packages`): `runtime-http/src/routes/entity_transfer.rs:192`
checks team-membership revisions before deciding replay versus replacement;
`runtime-services/src/entity_scope.rs:908` emits window-member tombstones;
`runtime-store/src/repositories/sync_documents.rs:502` owns the membership
revision lookup. The existing
`team_insertion_deletion_and_restore_replace_scopes_while_content_stays_delta`
witness exercises removals and reintroduction of older team content.

The existing durability owner, `runtime-store/src/sqlite_durability.rs`, sets
`wal_autocheckpoint=0` and the shared 16 MiB `journal_size_limit`. The account
registry already schedules PASSIVE checkpoints for pending commits every 15
seconds. This limit controls retained WAL size on reuse, not growth while a
reader pins frames. Adding another timer or an idle TRUNCATE would not resolve
the pinned reader. Settings remain unchanged.

Shutdown closes the change hub before waiting for HTTP serving to end
(`runtime-server/src/main.rs`), drains services, then runs the existing final
TRUNCATE. Closed-hub checks stop preparation between pages and discard queued
stream frames. An already-running SQLite statement still finishes; this change
does not interrupt statements or guarantee a deadline for an arbitrarily slow
individual query. The changes-index fix is owned separately.

## Connecting analysis (no client changes)

The server awaits initial content and entity transfer preparation before it
returns the SSE response (`runtime-http/src/routes/sync_stream.rs:527`, `:562`,
`:743` at the base). `SignedEventSource` awaits fetch response headers before
calling `onopen` (`web/src/servers/net/eventSource.ts:101`, `:127`). The schema
handshake timer is armed only inside `onopen`
(`web/src/sync/resourceEvents.ts:1558`), so a slow initial server read leaves a
gap before that timer starts. Startup visibility depends on projection readiness
(`web/src/app/startup/StartupOwner.ts:94`; `web/src/components/shell/App/App.tsx:38`),
and the fixed 800 ms timer can display Connecting while readiness is pending.

Cache startup has two relevant limitations. `sync/client.ts:294` decides whether
cache exists from the saved workspace hint, not IndexedDB. If the hint is absent
or invalid despite existing IndexedDB records, `:318` awaits identity network
I/O before opening the cache. With a valid scope and hint, `startCache` loads
state independently (`:221`, `:378`), but StartupOwner's timer does not wait for
cache hydration. A slow IndexedDB read can therefore show Connecting even when
cache exists. A valid loaded cache hydrates readiness markers
(`sync/subscriptionCache.ts:357`) without waiting for SSE.

The server-view mount path calls `services.areas.start()` before React renders
(`web/src/mount.tsx:61`, `:68`). That invokes StartupOwner and starts credential
and identity network work (`web/src/app/ServerAreasProvider.tsx:217`;
`web/src/app/startup/StartupOwner.ts:299`). This violates the requested startup
ordering. A follow-up must render cached state without a loader when IndexedDB
cache exists and start network work after React. None of these client paths are
changed here. These are source-derived causes, not a reproduced live-browser
trace.

## Deterministic witnesses

- `snapshot_spooling_and_slow_consumer_release_read_transactions`: invokes the
  real multi-page `prepare`, with the established SQL-boundary assertion at
  `spool_page`, then pauses the consumer after one frame, commits a write and
  verifies TRUNCATE leaves zero WAL bytes before consuming the next frame.
  On the base implementation plus the witness/assertion, it failed in 0.803 s
  with `filesystem/process work inside a SQL closure` (nextest exit 100,
  `/tmp/read-pages-red-1002.log`).
- `snapshot_pages_replay_updates_and_tombstones_from_watermark`: commits a
  delete of an emitted row, an update of an unread row and an insertion between
  SQL pages; checkpoints between pages; verifies that continuation retains its
  upper watermark and subsequent replay converges. A closed hub refuses the
  next page.
- `shutdown_discards_queued_snapshot_frames`: closes the hub with pending
  initial frames and requires the stream to end immediately.

All three are in both fast-tier default-filter blocks. The first focused green
run passed all 13 selected transfer/shutdown tests. Detached check commands use
`systemd-run --user` and keep a separate `.exit` file alongside each log.

Full storage/server/HTTP nextest, with workspace feature resolution, `-j 4`
and the `ci` profile: 2,163 run, 2,162 passed, 1 failed, 10 skipped; 159.940 s
test execution, exit 100 (`/tmp/read-pages-packages-1002.log`). All three new
witnesses passed. The unchanged failure is
`entity_scope_queries::tests::entity_scope_target_hot_queries_use_owned_indexes_and_ignore_unrelated_history`
at `runtime-store/src/entity_scope_queries.rs:1011`: the direct query uses
`changes_durable_identity (source_label=?)` and a temporary ordering tree.
The owner confirmed this failure on main `75cfb12c`. That source and its
indexes remain unchanged here; the separate
`fix-changes-label-index` worker owns this failure. It is also selected by the
fast tier.

Clippy ratchet with `--exclude-native`: passed, exit 0, 5,777 resolved SQL call
sites (`/tmp/read-pages-clippy-fixed-1002.log`). The initial ratchet caught two
synchronous metadata reads in the new async tests; they now use `tokio::fs`.
No allowances or baselines changed.

The full `just test-fast` run selected 3,211 Rust tests: 3,210 passed and the
only failed test was the known index witness above. It also caught two JSON
roundtrips used to construct `ResourceRef` in the new tests; those now use the
typed `StateResource` constructor directly. All other gate steps passed. The
run took 1,228.136 s, with 953.550 s in workspace boundary guards. Full log:
`/home/alex/.cache/codex-studio/checks/run-1794449018534-3YCvBi/full.log`.
An earlier detached invocation stopped before any check because its Git bridge
lacked the Layr environment; the completed run passed that environment through.

The owner will land the label-index fix first. This line must then rebase onto
that main state and rerun `just test-fast` before landing; the known failing
base run is not a passing gate.

After replacing the test constructors, `pnpm run lint:all`, the Clippy ratchet
with `--exclude-native`, and all three new witnesses passed (each exit 0).
Logs: `/tmp/read-pages-final-lint-1002.log`,
`/tmp/read-pages-final-clippy-1002.log`, and
`/tmp/read-pages-final-focused-1002.log`. No production behavior changed after
the original full package and fast-tier runs.

## Review correction: retention between delta pages

Astra's review of `743d54c693a2754f88e601a1cd92be4f` identified an omitted
retention check: a tombstone could be pruned after one delta page but before
the next. The new continuation transaction now compares the current maximum
of the entity and durable retention floors against `interval.checkpoint` for
deltas. A floor beyond that checkpoint returns the existing HTTP 409 error
mechanism before an incomplete transfer can publish its cursor. Reconnecting
from the last delivered cursor takes a replacement snapshot. Snapshot paging
does not reject ordinary watermark drift or retention advancement.

`delta_pages_reject_pruned_unread_tombstone_before_watermark` runs the actual
`prepare` path with a test-only page limit and interleaving. It asserts the
exact ordering checkpoint 2 < floor 3 < watermark 4. The real tombstone pruner
runs between pages; only its retained-count fixture is inflated to avoid
creating 10,000 tombstones. Both entity and durable floor variants require a
409 followed by a replacement snapshot without the deleted row. Before the
fix, the witness failed in 0.861 s because the incomplete delta published
watermark 4 (`/tmp/read-pages-prune-red-fixed-1002.log`). The reviewed state has
the same continuation code as that red run; its additional test hooks do not
change the retention logic.

`projected_pages_replay_main_row_updates_deletes_and_membership` now drives
the real transfer path through page interleavings for read-time agent views,
work detail/history, direct task detail, and task windows. It checks keyed
projection updates, work update-then-delete, and replacement of a team scope
after a member disappears. The existing shared-sequence boundary witness is
included in the focused transfer suite.

The web witness `paged snapshot replay preserves sequence guards, scope
pruning and update-then-delete` uses the actual assembler and sync-client
application path. It applies a two-page staged snapshot after another scope
has committed a newer row, verifies that the older version loses, evicts only
missing members of the selected scope, then replays an update and a deletion.
Duplicate replay and a later stale snapshot cannot resurrect the tombstone.
Both added Rust tests are selected in both fast-filter blocks; the client
witness is in the existing fast-tier bootstrap test file.

The earlier full package/fast-tier results above precede this retention fix.
Focused verification is repeated for the corrected state; the agreed rebase
and full fast-gate rerun after the label-index landing remain required.

Corrected-state checks: all 15 focused Rust transfer/shutdown tests passed
in 5.001 s; all 48 client bootstrap tests passed; TypeScript type checking,
repository lint and Clippy with `--exclude-native` passed. No allowances or
ratchet baselines changed. Logs and exit files:
`/tmp/read-pages-review-final-focused-1002.log`,
`/tmp/read-pages-review-final-lint-1002.log`,
`/tmp/read-pages-review-final-clippy-1002.log`,
`/tmp/read-pages-review-types-fixed-1002.log`, and
`/tmp/read-pages-review-web-final-1002.log` (each exit 0).
