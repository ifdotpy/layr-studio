# Queue and execution descriptor cutover

This step changes schema ownership only. Physical SQL, timestamp units, retained
bytes, receipt authority, revision triggers and transaction boundaries stay as they
were. `queue_schema` owns the eleven tables and their indexes; `queue_payloads`
names the typed JSON views. `LazyJson` preserves admitted historical text, including
malformed text, until a caller explicitly requests a typed view. Duplicate object
members keep their last value in that view; encoding never replaces stored bytes.

| Table                      | Requirement for later physical consolidation                                                              |
| -------------------------- | --------------------------------------------------------------------------------------------------------- |
| runtime_events             | Preserve status/error diagnostics, source rowids, team publications and all queue indexes.                |
| runtime_event_meta         | Preserve missing/null fields, legacy delivery spellings, raw assets/timing and metadata key exemption.    |
| runtime_event_revisions    | Keep the event revision clock and trigger increments distinct from publication sequences.                 |
| runtime_native_notices     | Keep account/connection filtering, rowid ordering and retained malformed evidence.                        |
| runtime_execution_runs     | Preserve account/epoch ownership, source thread identity and retained rowid watermarks.                   |
| runtime_execution_nodes    | Preserve managed-worker links and historical status evidence.                                             |
| runtime_execution_effects  | Preserve per-kind identities, reference ownership and exact outcome evidence.                             |
| runtime_execution_attempts | Preserve request tickets, generation fences, original submission evidence and terminal-source uniqueness. |
| runtime_execution_inputs   | Preserve composite run/event uniqueness and the event lookup index.                                       |
| runtime_process_actions    | Preserve admitted request/signature identity, uncertain actions and exact process results.                |
| runtime_tool_requests      | Preserve managed-tool/callback discrimination, origin uniqueness and exact receipts.                      |

SQL cutover: execution attempt/run record lookups use
narrow generated scalar selections. The former select raw storage and leave domain
decoding and error mapping at the existing caller. Event revision lookup retains its shorter raw COALESCE expression and fallback of 1.

Retained SQL sites have these reasons:

- Queue reads retain column-specific malformed-value diagnostics; constructing and
  converting an additional row would grow the existing mapper.
- Queue writes preserve the existing borrowed connection interface and bindings;
  constructing a generated row would grow the call.
- Execution writes use JSON expressions, joins, INSERT SELECT, custom conflict
  assignments or receipt settlement that generic CRUD would not shorten.
- Tool, notice and process-action consumers keep their action-specific validation,
  exact saved results and JSON projections; full-row decoding would read columns
  those repair/history paths deliberately leave unread.
- Historical ALTER and dated migration definitions remain frozen compatibility
  transforms. Current feature installers reference descriptor tables and indexes.
- Revision/publication/summary triggers stay with their existing specialized owners;
  this wave does not introduce a second trigger mechanism.

Feature preparation preserves the existing migration order: base table/index
installation precedes the generated-column indexes installed by receipt-owner
preparation and startup recovery. Fresh-state installation consumes the generated
catalog. Its only permitted catalog changes are `create_order`; object names,
types, parent table names and SQL text must match the previous catalog byte for byte.

Per table, the original relative index creation order and the original relative
trigger creation order are preserved independently. This keeps SQLite planner
tie-breaking stable; index/trigger interleaving is not a constraint.

The upgrade-order audit covers all eleven owners against `migrations/mod.rs`.
Early feature preparation installs only indexes over the original columns. Tool
request current-actor/current-status/origin/startup indexes and the attempt
terminal-source index retain their original later preparation stage. A historical
layout witness runs the real feature installer before receipt preparation, checks
that later columns/indexes remain absent early, and verifies their later creation
without rewriting the retained record.
