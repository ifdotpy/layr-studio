# Wave 2 F2: work-history descriptors

This slice converts `runtime_work_results`, `runtime_work_decisions` and
`runtime_work_releases`. It does not change their physical format or move rows.
`work_history.rs` owns each descriptor's exact existing DDL and work/index index;
`canvas_schema.rs` registers them in table-name order. These lazy tables are not
in the cold catalog, so the parity witness creates the previous code-created
schema independently and compares SQLite's stored table and index SQL.

The three tables retain the `{work}:{index}` identity, ordinal ordering and the
parent deletion cascade. Reconciliation uses the caller's transaction, generated
get/upsert and `db_delete!`; reads use `Read<Canvas>` and `db_query!`. Equal entries
retain their exact original text and durable sequence. Shortened arrays delete
only the tail and keep the existing durable tombstones. Transaction rollback
restores the parent, children and durable stream together.

Each JSON column has a named payload owner (`WorkResultRecord`,
`WorkDecisionRecord`, `WorkReleaseRecord`) with typed `HistoryEvidence` and the
shared exact JSON codec. Object member names are genuinely open evidence here:
the previous child writer validated JSON syntax, without a closed field schema.
Historical arrays, scalars and JSON null remain accepted too. Token dispatch
avoids serde's untagged buffering of raw evidence. The existing parent projection
and semantic-equality boundary still use `Value`; the descriptor fields and codec
owners do not. Narrowing that parent boundary belongs to the work-item slice.

Five hand-written child CRUD/query SQL sites are replaced: reconciliation read,
upsert and tail delete, ordered entries read and single-entry read. Two SQL sites
remain in `work_history.rs`: the cross-table cascade trigger and the migration's
raw parent-record input (which deliberately tolerates invalid historical work
records). `entity_scope.rs` keeps its durable-row/child/parent join and JSON team
selector, with a reason at the query. Generic durable numbering, retention,
triggers and snapshots remain with `durable_seq.rs`; its three child table names
now consume the descriptor owner. Receipt/request APIs are unchanged.

## Cutover work still required

| Source table             | Later cutover requirements                                                                                                                                |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime_work_results`   | Copy original IDs, work IDs, ordinals and raw JSON to the result variant of `history`; prove the rowid/delete mapping and preserve publications/cascades. |
| `runtime_work_decisions` | Copy original IDs, work IDs, ordinals and raw decision evidence; preserve ordering, publications and cascades.                                            |
| `runtime_work_releases`  | Copy original IDs, work IDs, ordinals and every admitted JSON shape; preserve ordering, publications and cascades.                                        |

State-check leniency applies to all three history tables: they had no validator
before descriptors, so their registry reports use the shared `count_legacy_rows`
checker. NULL keys and malformed JSON remain accepted by that report, while typed
reads continue to reject malformed records. Validate this historical evidence
before any stricter cutover; do not silently discard it. Duplicate object keys
also retain their original bytes and existing last-key-wins projection.
No timestamp, enum-code, retention or online-copy encoding decision is made here.

## Candidate validation and size

The focused workspace nextest selection ran all six `work_history::tests`
witnesses and passed, and strict workspace clippy passed with `-D warnings`.
This includes the independently created legacy schema for each of the three
tables, syntax/shape admission, deletion cascades and transactional history writes.
The final slice is based on main `fb26645d6f367dd9235efc4b8958b386d0f2032d`;
shared API and codec files are unchanged from that base.

The six affected Rust files (including the transaction signature prerequisite)
were split into production and `#[cfg(test)]` modules
using ast-grep; benchmark code counts with tests. Tokei reports production code
2046 → 2212 lines (+166), and test/benchmark code 1109 → 1258 (+149). There is
no production size reduction in this slice: replacing one dynamic table path
with three exact schema owners, named evidence types and typed transaction/read
dispatch costs code before the later table consolidation.

Jscpd, with five lines and thirty tokens as thresholds over those same six
files, reports 14 → 14 clones and 101 → 100 duplicated lines: no new clones.
The exact DDL/index strings replace the previous runtime schema generator; no
additional catalog copy was introduced.

## Other F2 tables

The lead split the remaining tables into separate interaction, planning/work and
context slices. Their omission here keeps each descriptor with its actual caller
and typed owner; adding unused descriptors would not complete their conversion.

| Table                          | Reason for a separate slice                                   |
| ------------------------------ | ------------------------------------------------------------- |
| `graph_edges`                  | Graph membership, publication and deletion consumers.         |
| `groups`                       | Group membership and graph readers.                           |
| `messages`                     | Legacy message shape and admission behavior.                  |
| `runtime_chat_messages`        | Autoincrement sequence, delivery JSON and message admission.  |
| `runtime_requests`             | Request identity, property order and exact reply handling.    |
| `runtime_rooms`                | Room membership and radio lifecycle.                          |
| `approvals`                    | Approval admission, secret ownership and request obligations. |
| `runtime_complaints`           | Complaint mutations and typed response history.               |
| `runtime_annotations`          | Typed annotation owner and JSON selectors.                    |
| `runtime_checkpoints`          | Workspace checkpoint capture and reads.                       |
| `runtime_plans`                | Plan schema and work-item callers.                            |
| `runtime_rules`                | Rule tick selectors and action ownership.                     |
| `runtime_tasks`                | Task status/window selectors and expression indexes.          |
| `runtime_work`                 | Typed parent arrays, historical repair and work mutations.    |
| `runtime_workspace_operations` | Workspace operation identity and obligations.                 |
| `runtime_projects`             | Project permission and federation owner overlap.              |
| `runtime_assets`               | Transcript asset encoding and text ownership.                 |
| `runtime_budget`               | Budget value/number owner and settings consumers.             |
| `runtime_compactions`          | Context compaction selectors and evidence.                    |
| `runtime_context_manifests`    | Context manifest schema and producer ownership.               |
| `runtime_context_reminders`    | Reminder shape and lifecycle consumers.                       |
| `runtime_model_status`         | Model status producer and readers.                            |
| `runtime_profiles`             | Profile typed owner and settings readers.                     |
| `runtime_transcript_revisions` | Transcript revision/publication semantics.                    |
