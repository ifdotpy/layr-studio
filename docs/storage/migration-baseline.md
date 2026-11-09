# Supported storage baseline

Startup supports completed states from release
`f832d09308e4ac1ae1c46e7a6882077ec644acea` and later. The retired catalog's
ordered IDs, targets and receipt-family classification are frozen in
`runtime-store/src/migrations/baseline_manifest.rs`. They are evidence inputs,
not runnable migrations. The 99 migration modules dated before 2026-11-08 are
removed. The three existing November 9 transitions remain.

Under `runtime.lock`, before ownership publication, fresh bootstrap, local-server
identity, writer connections or recovery, an existing Canvas/Analytics pair must
prove the baseline. Refusal names `state too old: missing/incomplete TARGET/ID`.
A partial database pair is refused by the existing read-only directory check.

The gate uses the actual retired catalog fingerprints, the five individual SQL
checkpoints, all 17 receipt-family finish checkpoints, the analytics provider
format, the complete supervisor schema when present, and the current account
registry format when present. A registry retaining `claude-local` is incomplete.
Every existing checkpoint requires both `complete=1` and `status='complete'`.
The review-model checkpoint may be absent: ff02c9d ran the step before its
ledger existed, and f832d09 retained the same catalog fingerprint. A verified
covering catalog is its completion evidence, matching f832d09's adoption rule.
The baseline does not synthesize that missing historical row. Catalog
markers require a recognized fingerprint and a nonfuture positive decimal schema
version. They need not equal today's schema version: the supported release
writes Canvas marker version 405 and normally finishes startup at version 412.

Fresh state installs the current catalog, then seeds baseline evidence in the
same creation transaction. It no longer runs retired data converters. The agent
descriptor describes the current physical columns and indexes directly. Frozen
differential corpora still need historical input constructors; those live only
behind `test-support` or `cfg(test)` in `test_fixtures`, with no startup authority.

The dated `2026-11-09-retired-objects` cleanup has Canvas and Analytics targets.
Each target commits separately; a failed target rolls back, while a completed
earlier target remains committed. Startup can continue after interruption.

| Database / object                                                                                                            | Selection                                        | Visible effect and safety                                                                                                                                                                                   |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Canvas `runtime_budget`                                                                                                      | Entire obsolete table                            | Removes historical budget state and usage-reconciliation replay-conflict diagnostics. Current usage accounting and receipts are retained. The unused response field and generated API property are removed. |
| Canvas `runtime_server_outbox`, `runtime_federation_outbox`, `runtime_access_outbound`, `runtime_server_fetch`               | Drop only when `NOT EXISTS(SELECT 1 FROM TABLE)` | Removes empty old delivery tables and their dependent schema objects. Nonempty leftovers remain because they may contain unknown-effect evidence. Current delivery uses `runtime_outbox`.                   |
| Canvas `summary_plan_delete`, `summary_annotation_delete`, `summary_questionHistory_delete`                                  | Retired trigger names only                       | Removes triggers belonging to the retired summary format. The current summary trigger owner supplies the retained triggers.                                                                                 |
| Analytics `analytics_capture_effect_receipts`, `analytics_capture_legacy_watermarks`, `analytics_capture_receipt_migrations` | Entire retired tables                            | Removes old capture caches/metadata already retired by the supported release. No current readers remain. Current capture receipts, legacy-key mappings, accounting and request idempotency owners remain.   |

Current `schema::prepare_item_turn_scope` continues to remove the obsolete
`runtime_request_native_approval` index while installing the current transcript
turn index. The retained immutable-scope migration removes its obsolete columns,
indexes and visibility triggers using its existing precise selections.

No transcript, checkpoint, work-history or account-transfer rows are deleted.
Both plain SQLite TEXT and stored-format-1 zstd BLOB are current StoredText forms.
Inline transfer reply values also have current readers. Unreadable transfers can
hold pending effects, so their readers and evidence remain. No age predicate is
used. No live state was read or modified.

The compressed supported-release fixture in `runtime-store/tests/fixtures/f832d09`
was generated by the installed release binary in a new temporary state directory
on a free port, then stopped. It retains the actual original SQLite files. Tests
exercise upgrade/reopen and current agent access, exhaustive checkpoint refusal,
future/invalid catalog refusal, supervisor/provider refusal, cross-file partial
upgrade and cleanup rollback/restart. Current fresh-state, encoding, receipt and
schema checks remain.

A separate task-acceptance correction prevents an event identity collision exposed
by immutable event owners: the immediate root notice is emitted only for
root-owned work. Worker-owned work keeps its one worker notice when its archive
settles. The existing API caller witnesses cover the failure; notice assertions
cover both recipients.

## Measured code reduction

Tokei counts for the 168 changed Rust files against GitHub main `13beb84`:

| Rust code                 | Before |  After | Change |
| ------------------------- | -----: | -----: | -----: |
| Production                | 44,981 | 37,727 | -7,254 |
| Tests and fixture support | 44,640 | 38,282 | -6,358 |

The split classifies integration-test paths, `test_fixtures.rs`, and items behind
`cfg(test)` or `cfg(any(test, feature = "test-support"))` as test support. Counts
exclude comments and blank lines; generated JSON, SQL fixtures and documentation
are outside this Rust count. The fixture helper's duplication check remains at
12 clones / 119 duplicated lines before and after.

## Checkpoint provenance audit

All 22 fresh-release checkpoints were audited against the Git history and the
f832d09 startup runner. Receipt finish rows use the encoded key
`receipts:{length}:{migration-id}-finish:`.

| Checkpoint migration ID                  | Completion evidence and history                                                                                                                                                   |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `2026-10-28-receipts-execution`          | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-actions`            | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-context-repair`     | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-voice`              | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-work`               | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-title`              | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-owner-state`        | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-tools`              | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-requests`           | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-remote`             | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-provider-delivery`  | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-provider-callbacks` | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-vm-provider`        | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-account-worker`     | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-incidents`          | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-checks`             | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-28-receipts-drop-old-tables`    | Its finish row was already required by f832d09 `verify_receipt_finish` before catalog processing; no missing-row adoption.                                                        |
| `2026-10-31-work-records`                | The checkpoint and transition were introduced together in `efbab4467`; keep the row requirement.                                                                                  |
| `2026-10-30-task-records`                | The checkpoint and transition were introduced together in `2d82f2c8b`; keep the row requirement.                                                                                  |
| `2026-11-01-room-lead-projection`        | Ledger added in `e38be69df`, which also adds four Canvas catalog IDs and changes the fingerprint; reaching f832d09 requires that catalog application and its checkpoint.          |
| `2026-11-01-radio-controls-projection`   | Same `e38be69df` catalog change and completion argument as the room-lead projection.                                                                                              |
| `2026-11-07-explicit-review-model`       | Ran without a ledger from `9c5ebefc2`, including ff02c9d. f832d09 adds the ledger without changing the fingerprint; accept the covering completed catalog when the row is absent. |

For the room transitions, `e38be69df` adds `2026-11-01-access-invite-created`,
`2026-11-01-chat-busy-deleted-index`, `2026-11-01-status-turn-indexes` and
`2026-11-01-work-history-rows`. Its Canvas SQL fingerprint changes from
`e5b734ee5d56d82abbd7d93e7c9e03278745216e91a1a6b08f94102c1a1d13ec` to
`c7fc1e21e56d55056f7178fd81a214e9eac47ff985818d75615cd447969a7cdf`.
The ff02c9d and f832d09 completed Canvas fingerprint is unchanged at
`f4d36d4581a8dc90b00d69e865a56ecabd26032d758abd1c9aa0f5596c811e5d`.

The populated `ff02c9d-f832d09` fixture reproduces that real binary upgrade and
retains the missing review-model ledger, explicit review choice and work item.
It failed before the baseline correction. Exhaustive refusal tests still cover
all mandatory missing rows and all explicitly incomplete rows, including a
present but incomplete review-model checkpoint.
