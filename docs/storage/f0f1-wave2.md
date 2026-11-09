# F0/F1 descriptor conversion

The Canvas registry owns 23 F0/F1 descriptors in `clock_schema.rs`, `settings_schema.rs`, and the existing peer-update pilot. The snapshot remains the historical parity fixture. Fresh-state table and index creation uses descriptors; feature/bootstrap creation uses their idempotent installer. The existing publication triggers, shared sequence allocator, floors, tombstone retention, conditional membership changes, and migration transforms keep their SQL and transaction boundaries.

The 20 tables with no prior state-check report use the shared count-only checker, preserving their historical acceptance. The state-check report for `sync_entities` reuses its existing checker, including heterogeneous envelope normalization and transcript hash-only checks. Federation settings reuses its existing parser and size check. Those registrations replace the previous reports rather than add duplicate reports. Their historical duplicate-key acceptance remains cutover debt.

`ExactJson` keeps admitted bytes for federation settings, orchestration settings, and optional publication team counts. The orchestration descriptor uses the same object-only `StoredSettings` owner as its reader; publication counts reuse `agent_progress::TeamCounts`. Federation extensions preserve raw unknown members.

Three columns retain raw storage: `sync_documents.payload`, `sync_entities.payload`, and `runtime_sidebar_order.record`. Existing writers/readers admit arbitrary JSON shapes and historical malformed evidence; typed envelope admission cannot preserve that behavior. Their existing `Value` boundaries remain explicitly deferred. Publication comparisons of prior `team_counts` also stay raw: malformed old text triggers republication instead of a decode error.

## Remaining cutover work

No INTEGER enum recoding, time conversion, consolidation, or data migration is included.

| Source table                           | Remaining cutover contract                                                                                                       |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `durable_floor`                        | Preserve singleton floor and shared cursor domain.                                                                               |
| `durable_rows`                         | Preserve sequence allocation, delete tombstones, retention floor, and both membership fences.                                    |
| `runtime_agent_record_generation`      | Preserve atomic trigger increments.                                                                                              |
| `runtime_scheduler_agent_change_state` | Preserve floor, generation, entry count, and total reconciliation.                                                               |
| `runtime_scheduler_agent_changes`      | Preserve nonnegative generation and deleted-agent identity.                                                                      |
| `sync_documents`                       | Type arbitrary payload evidence; retain explicit sequence identity and scope/id uniqueness.                                      |
| `sync_entities`                        | Type heterogeneous/raw payloads; preserve hash-only transcript markers and state-check duplicate-key leniency.                   |
| `sync_entity_meta`                     | Identify each open key's owner and unit before metadata consolidation.                                                           |
| `sync_generation`                      | Preserve the maximum of every existing sequence source before removal.                                                           |
| `sync_identity`                        | Validate historical nullable key rows before strict key cutover.                                                                 |
| `task_window_state`                    | Resolve fractional created time (U1); preserve old/new owner snapshot fences.                                                    |
| `agent_publication_state`              | Preserve optional flags and exact text comparison; resolve malformed prior counts policy.                                        |
| `client_command_watermarks`            | Preserve monotonic MAX admission and WITHOUT ROWID identity.                                                                     |
| `runtime_access_local_alias`           | Preserve singleton and open alias text.                                                                                          |
| `runtime_access_settings`              | Preserve singleton and historical nonzero integer flag semantics.                                                                |
| `runtime_federation_settings`          | Preserve raw extensions and state-check duplicate-key leniency.                                                                  |
| `runtime_orchestration_settings`       | Type the remaining independently updated settings without changing absent-field defaults or JSON patch semantics.                |
| `runtime_payload_migrations`           | Keep pre-upgrade schema, staged column additions, generated enum guard, and migration progress atomics behavior-identical.       |
| `runtime_search_rollout`               | Establish phase vocabulary; resolve fractional updated time (U1); keep partial-schema migration readers.                         |
| `runtime_sidebar_order`                | Establish a typed historical record owner without narrowing arbitrary JSON admission.                                            |
| `runtime_stored_text_batches`          | Preserve aggregate batch allocation, exact SHA evidence, fractional created time, and installation in both Canvas and Analytics. |
| `runtime_peer_update_settings`         | Preserve existing singleton pilot and nonzero integer flag semantics.                                                            |
| `runtime_online_migration_dirty`       | Preserve composite identity, key ordering, transform-before-delete ordering, and source-delete replay.                           |

`runtime_payload_migrations` retains its staged bootstrap/upgrade SQL. Its final descriptor cannot safely replace the earlier schema while migrations still add `cutover_ms` and the virtual enum guard. `runtime_stored_text_batches` is also created in Analytics, so the same descriptor installs its physical schema in both databases; its aggregate batch write remains raw to allocate the next batch atomically. Dirty-key CRUD uses the Canvas descriptor inside the existing migration transaction; trigger recording and progress updates remain unchanged.

## SQL literal ledger

These counts compare direct SQL literal sites in the changed Rust files with the shared API candidate, excluding test-only items and descriptor declarations. A compound Rust SQL literal counts once per referenced table. The SQL trigger file and unchanged historical migrations remain separately inventoried in `map.md`; computed table dispatch is not counted as a literal reference. “Removed” includes DDL now supplied by descriptors and queries/writes replaced by generated operations.

| Table                                  | Before | Raw retained | Removed | Retained SQL reason                                                                                                                                       |
| -------------------------------------- | -----: | -----------: | ------: | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `durable_floor`                        |      3 |            1 |       2 | Trigger retention uses MAX and a tombstone subquery.                                                                                                      |
| `durable_rows`                         |     19 |           18 |       1 | Shared sequence expressions, trigger bodies, joins, and partial window selectors preserve cursor and malformed-history behavior.                          |
| `runtime_agent_record_generation`      |      2 |            0 |       2 | Atomic trigger increments remain in unchanged agent_volatile code.                                                                                        |
| `runtime_scheduler_agent_change_state` |      1 |            0 |       1 | Aggregate journal bookkeeping remains in unchanged trigger code.                                                                                          |
| `runtime_scheduler_agent_changes`      |      1 |            1 |       0 | Joined changed-agent selection; trigger writes remain unchanged.                                                                                          |
| `sync_documents`                       |      8 |            6 |       2 | Shared MAX allocator and bounded projection selectors.                                                                                                    |
| `sync_entities`                        |     27 |           25 |       2 | Narrow hash/deleted no-op projection avoids payload reads; joins, JSON selectors, NOT LIKE exclusions, aggregate watermarks, and legacy state validation. |
| `sync_entity_meta`                     |     36 |            7 |      29 | Scope-fence expressions, aggregate initialization, and three REPLACE statements preserving rowid replacement.                                             |
| `sync_generation`                      |      5 |            5 |       0 | Shared MAX allocation and pre-removal counter preservation.                                                                                               |
| `sync_identity`                        |      2 |            1 |       1 | Fresh identity reset uses randomblob/hex expressions.                                                                                                     |
| `task_window_state`                    |      5 |            3 |       2 | Narrow agent-only refresh repairs legacy created/running fields; window joins and historical membership projection.                                       |
| `agent_publication_state`              |      7 |            6 |       1 | JSON source projection and selective/raw prior-state comparison avoid decoding unrelated malformed counts.                                                |
| `client_command_watermarks`            |      3 |            1 |       2 | Atomic monotonic MAX upsert.                                                                                                                              |
| `runtime_access_local_alias`           |      3 |            0 |       3 | None in changed files.                                                                                                                                    |
| `runtime_access_settings`              |      8 |            0 |       8 | None in changed files.                                                                                                                                    |
| `runtime_federation_settings`          |      3 |            1 |       2 | JSON enabled selector; unchanged generic readers retain last-key-wins admission.                                                                          |
| `runtime_orchestration_settings`       |      9 |            7 |       2 | JSON selectors, json_insert cutover, and json_set field updates.                                                                                          |
| `runtime_payload_migrations`           |     13 |           13 |       0 | Pre-upgrade schema, staged progress updates, and migration-machine queries.                                                                               |
| `runtime_search_rollout`               |      1 |            0 |       1 | Unchanged transcript readers intentionally select phase from partially upgraded schemas.                                                                  |
| `runtime_sidebar_order`                |      3 |            0 |       3 | None in changed files; its arbitrary JSON boundary is deferred.                                                                                           |
| `runtime_stored_text_batches`          |      2 |            1 |       1 | Atomic MAX batch-number allocation.                                                                                                                       |
| `runtime_peer_update_settings`         |      0 |            0 |       0 | Existing pilot already used generated CRUD.                                                                                                               |
| `runtime_online_migration_dirty`       |      5 |            2 |       3 | Dirty-recording trigger expression and progress/count join.                                                                                               |
