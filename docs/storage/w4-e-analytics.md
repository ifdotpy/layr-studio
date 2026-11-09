# Wave 4: Analytics descriptors

All sixteen Analytics tables have one `DbTable` owner in `analytics_schema.rs`.
The registry supplies catalog installation and state-check registration. Fresh
state revision preparation uses the same descriptors and trigger source.
`analytics_payloads.rs` names the JSON structures and preserves exact extensions;
`ExactJson` retains original text. Duplicate-member interpretation reuses the
existing budget-history normalizer, now owned by
`json_record_codec::last_json_members`; the shared strict raw-extension helper
keeps its existing policy. Item encoding retains the original TEXT/BLOB
and dictionary reference, with explicit typed decoding. Existing capture and
reporting adapters retain their historical schema checks and JSON API contracts.

Legacy SQL stays unchanged. Relative index creation order within each table is
retained because SQLite planner ties can otherwise select a broader cost index;
the existing bounded-cost-scope witness covers that behavior. Original trigger
order is retained as well; the parity witness compares ordered index and trigger
names within each table in addition to every object's SQL bytes. Catalog object
order may change; object names, types,
table names and SQL bytes do not. The parity witness independently installs the
pre-wave schema and compares every object, ignoring only creation order. Revision
and immutable dictionary triggers remain explicit SQL because `DbTable` has no
trigger declaration. There are no db-api changes or physical migrations.

SQL conversion is limited to DDL installation and revision preparation. Capture,
reporting, cost aggregation, dictionary lookup and metadata DML remain raw: their
joins, JSON expressions, replay boundaries, dynamic table selection, rowid access
or short projections are shorter than the generated equivalent. No writer
changes JSON serialization. Previously checked facts retain their diagnostics;
previously unchecked tables count rows without new rejection. The importer
retains its separate logical report within `analytics_history`.

A later physical consolidation must preserve each table's following contract:

| Source table                            | Later cutover requirement                                                                      |
| --------------------------------------- | ---------------------------------------------------------------------------------------------- |
| analytics_agents                        | Retain nullable historical keys, root/account history and exact JSON extensions.               |
| analytics_turns                         | Preserve selector/record disagreements; conflict updates historically replace only the record. |
| analytics_items                         | Preserve TEXT/BLOB encoding, dictionary identities and tool/time selectors.                    |
| analytics_history                       | Preserve importer reserved-key/empty-agent rules and both logical diagnostics.                 |
| analytics_limits                        | Preserve account/time ordering, AUTOINCREMENT high-water and unvalidated old JSON.             |
| analytics_notifications                 | Preserve nullable grouping selectors and SQL NULL arithmetic.                                  |
| analytics_usage                         | Preserve seq high-water, original receipts/usage evidence and every response/scope index.      |
| analytics_cost_rows                     | Preserve fractional/NULL token amounts, counted-model distinctions and eligibility semantics.  |
| analytics_cost_groups                   | Preserve implicit rowid identity, NULL/float matching and covering-index order.                |
| analytics_usage_revisions               | Preserve string source identities and deletion evidence.                                       |
| analytics_cost_group_revisions          | Preserve integer source rowids and deletion evidence.                                          |
| analytics_usage_revision_clock          | Preserve the shared clock and admitted cardinality; rowid does not impose singleton semantics. |
| analytics_usage_roots                   | Preserve missing-row generation zero and original root identities.                             |
| analytics_meta                          | Preserve key-scoped counter/time/marker parsing and exact open evidence.                       |
| analytics_stored_text_dictionaries      | Preserve content-derived ids, hashes, bytes and all three immutability triggers.               |
| analytics_publication_overflow_receipts | Preserve per-receipt monotonic overflow maxima and transactional accounting.                   |

## Historical preparation audit

The complete Analytics registry installer is test-only; startup never installs
all current descriptor indexes onto an old database. Agents, turns, items,
history, notifications, usage, roots and metadata therefore acquire no new early
DDL. Limits retain their original index preparation after the Analytics catalog.
Cost-row scope/response indexes remain in the original backfill owner; the
eligibility indexes and cost-group covering index remain in `cost_aggregate::install`
after dated migrations. The preflight expected schema is a separate in-memory
database, never the source database. No new early index references `eligible`,
`final_evidence` or `cache_write_1h`.

The three revision descriptors/triggers run at the existing revision-schema
migration; all referenced source identities and clock/revision columns already
exist there. Overflow receipts run at their original dated step. Dictionary
preparation remains after `migrations::run`, with unchanged id/name/created
columns and immutability triggers. The capture-errors migration changes metadata
values and does not invoke a descriptor installer. No additional objects need
deferral.

Existing historical witnesses cover these paths: cost aggregation's
`typed_usage_legacy_migration_keeps_totals_without_reading_usage_rows` starts
without the later eligibility/final-evidence/lifetime columns; startup's
`missing_analytics_revision_schema_is_restored_on_open` and
`current_settings_and_overflow_receipts_exist_on_fresh_and_completed_stores`
exercise missing historical objects through `RuntimeStore::open`; capture-errors'
`startup_converts_capture_error_and_refuses_invalid_legacy_time` exercises the
dated conversion and rollback boundary. The latter startup/migration witnesses
are selected by the fast tier.
