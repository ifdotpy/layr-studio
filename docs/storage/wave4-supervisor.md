# Supervisor descriptor cutover notes

Wave 4 registers all eight existing tables without consolidating or migrating data.
The independent journal still owns transactions, process proof, output ACKs and WAL
flushing. All original DML remains raw: joins, aggregates, generation predicates and
narrow projections are shorter and retain their original decode/refusal boundaries.
Only installation and state-check registration move to descriptors.

The catalog now includes the same eight tables plus the existing monitor-native
index, a documented addition approved for wave 4. OperationsRow owns its SQL.
Fresh state already calls SupervisorJournal::open; that path now installs the
seven original tables, then the monitor index, then operation_writes in the same
two migration stages. No fresh-state writer, commit, fsync or publication behavior
changes. Every existing object's name, type, table and SQL stays byte-identical;
remaining catalog differences are creation order only.

| Table            | Requirements for later consolidation                                                                                                                                        |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| handles          | Retain nullable legacy key evidence, signature, PID, sequences, original fractional Unix seconds and epoch calculation; preserve SQL NULL versus raw JSON init result.      |
| child_identities | Preserve PID/PGID and exact process-start identity; audit nullable legacy keys and job evidence before STRICT conversion.                                                   |
| degraded_handles | Join prior PID and start identity with the current child before accepting recovery proof; retain fractional recovered-at seconds.                                           |
| operations       | Preserve INTEGER/TEXT/NULL native ID storage classes, original acceptance seconds, optional generation and exact optional response bytes; retain the monitor partial index. |
| operation_writes | Preserve accepted-only versus actually-written evidence and handle/operation identity; absence never authorizes another write.                                              |
| events           | Keep generation-fenced sequence/ACK transactions, original payload bytes and byte-size accounting; do not newly decode unread columns.                                      |
| recovery_events  | Preserve AUTOINCREMENT identity, Unix seconds, open outcome/detail evidence and existing last-16 retention independently of handle deletion.                                |
| supervisor_state | Keep open keys and arbitrary legacy text alongside journal identity and presence-aware recovery JSON; never normalize unknown evidence.                                     |

Descriptors use signed SQLite counters; the existing narrow journal readers retain
unsigned-range validation. Previously unchecked tables receive only count reports,
including malformed or nullable history. Recovery's open fields and extensions use
raw JSON tokens with last-wins duplicates; clean recovery serialization retains its
field order. Init results, operation responses and event payloads have named raw
JSON owners. Open journal state text is deliberately not parsed by its codec.

Migration-order audit: the two Supervisor migrations install the original seven
tables and monitor index, then add operation-write evidence. Neither migration
adds columns to an existing table. All monitor-index columns already exist in the
frozen Python journal. The upgrade witness loads that independent historical SQL
without operation_writes or file-migration markers, removes the native index,
and opens it through SupervisorJournal. It checks all eight tables, the complete
index set and the retained handle signature against a fresh installation.

Init-result and recovery decoding retain their former finite-number and nesting
limits through a streaming admission check, while keeping the original raw tokens.
Event payloads retain their separate raw-syntax-only admission. A failing-first
regression witness covers unrepresentable numbers at the root and inside objects.
