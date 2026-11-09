# Wave 4 A: agent and domain descriptors

All eight tables retain their physical columns, indexes, JSON bytes and units.
The Canvas registry owns their current catalog DDL. Frozen migrations remain
historical witnesses. Existing agent writes retain their conditional updates,
volatile merge and publication behavior; domain writes retain coupled state/session
checks and project all rows in the same transaction.

| Table                     | Later consolidation requirement                                                                                                                          |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime_agents`          | Keep generated-column affinity, nullable historical fields, hold defaults and exact record evidence; preserve conditional writes and publication scopes. |
| `runtime_agent_volatile`  | Preserve each independently nullable JSON field, malformed retained evidence, and unchanged-write suppression.                                           |
| `runtime_agent_moves`     | Preserve exact raw extension evidence, payload/key matching and uncertain move ownership.                                                                |
| `sessions`                | Preserve chat/thread/account identity, retained ended sessions and delayed thread index installation.                                                    |
| `turns`                   | Preserve paired state/session interpretation, exact attempt evidence, reservation uniqueness and callback indexes.                                       |
| `chats`                   | Preserve independent chat presence, nullable scheduling fields and deletion projection.                                                                  |
| `runtime_lead_requests`   | Preserve original signature and admitted agent identity atomically.                                                                                      |
| `command_send_admissions` | Preserve the signature and original millisecond timestamp, replay boundary and bounded garbage collection.                                               |

The cold-state catalog has always omitted four indexes that `domain::install`
creates later: `chats_vm`, `chats_capacity_status`, `chats_active_status`, and
`sessions_thread`. Their raw delayed installer remains in `domain.rs` to preserve
installation timing for fresh and upgraded states. The focused test compares the
cold domain index set with the installed set minus exactly these four indexes.

Generated agent and attempt columns remain SQLite-owned. Descriptor reads select
only their declared fields; the retained hot and repair SQL keeps narrow
projections so an unread malformed column cannot change admission. Agent and
volatile merge/write SQL, compound domain upserts and send admission/GC SQL stay
raw because their conditional updates, expressions and coupled behavior are
shorter than an equivalent generated operation. Existing dynamic agent index
installers retain their migration/repair roles; catalog literals moved to the
physical descriptor without changing their SQL. `agent_columns::table_sql` stays
with its migration/fixture owner: it describes the evolved table after obsolete
generated columns are dropped, whereas the unchanged cold catalog precedes those
migrations.

Follow-up outside this wave: `agent_volatile.rs:{merge,merge_row,merged_text,put,update}`,
`domain.rs:{project,project_turn}`, `records.rs:AgentRecord` and the agent repository
Value-based caller APIs retain their existing JSON mutation boundary. Descriptor
owners introduce no new `serde_json::Value` types or JSON-reencoding writes.

Index attributes retain the original creation order within each table; trigger
order is separately unchanged. SQLite can use index creation order to choose
between equally priced plans. An independent pre-wave ordering witness protects
the agent and turn index sequences.
