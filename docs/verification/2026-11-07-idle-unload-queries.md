# Idle unload query work

Base: Git `ff501dfe`, Layr `66e00fa88806755593dd01a1b3cd2443`.

The supplied live profile attributed 20.1% inclusive server CPU to
`idle_unload_usage_snapshot` and 14.4% to `sync_unload_deadlines`.
This change uses disposable SQLite fixtures; it does not measure or change the
live server.

## Cause and change

The existing watch wakes account workers after commits to agents, events,
monitors, account transfers, approvals, or tool requests. A process idle recheck
also synchronizes compaction deadlines. These notifications intentionally remain
conservative: their table and row hints do not identify changed eligibility fields.

The snapshot previously searched account agents and source cleanup owners for
each candidate thread. It now starts with the indexed session lookup and finds
each owner by primary key. Account usage and the account's source cleanup thread
set are computed once inside the query. There is no stored cache or new runtime
state.

Deadline synchronization previously transferred and decoded all matching Claude
agent records, then queried the fired receipt for each episode, even when every
deadline was unchanged. A single SQL query now joins the supplied deadline map,
filters unchanged deadlines, and excludes settled episodes before returning any
records. Changed rows must be materialized before the receipt predicate: SQLite
otherwise evaluates the correlated receipt lookup for unchanged episodes too.
The additional two-size VM-step witness detects that work even when the query
returns zero rows. Changed records still use the existing typed episode calculation and
agent repository write. Missing deadlines still clear the stored unload time;
unchanged deadlines cannot move the fire time as wall time advances.

## Count witnesses and sampled timing

Both witnesses failed before production changes. These are individual debug
fixture samples on the shared development machine, not timing thresholds or a
live CPU improvement claim. Setup and migrations are outside the measured call.

| Fixture                                            | Before                                          | After                                          |
| -------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------- |
| 100 idle lead agents, 100 loaded threads           | 97,999 SQLite VM steps; 4.113 ms                | 36,616 VM steps; 3.656 ms                      |
| 100 unchanged Claude episodes, no unload deadlines | 101 SQL statements, 200 returned rows; 5.820 ms | 1 SQL statement, 0 returned rows; 0.63–0.86 ms |

The optimized 50-agent snapshot takes 18,416 VM steps. The witness checks both
the 100-agent operation budget and near-linear growth when doubling agents and
threads. Only operation counts are asserted. The deadline witness requires one
statement and zero returned rows, which also prevents decoding unchanged records
and querying their receipts individually.

Review exposed a second quadratic cost in the first version of the deadline
query: the receipt predicate ran before unchanged-row filtering. Its new witness
failed at 64,627 VM steps for 100 episodes and 249,227 for 200. Materializing the
changed rows first reduces those counts to 3,534 and 7,034, respectively. This
witness asserts near-linear growth, independently of returned-row counts.

Parity witnesses cover durable busy conditions, source cleanup phases and
accounts (including deleted owners), finished-worker unload policy, deadline
changes and clearing, fire-time clamping, unchanged record bytes, and a settled
receipt with the same episode identity on another account. They are selected by
the fast nextest profile. Existing account-worker tests exercise the caller's
unload and process decisions.

## Validation commands

- `cargo test --workspace --exclude studio-native-client -j 4 --lib idle_unload::tests -- --nocapture`
- `cargo test --workspace --exclude studio-native-client -j 4 --lib idle_compaction::tests -- --nocapture`
- `cargo test --workspace --exclude studio-native-client -j 4 --lib account_worker::tests::idle_unload`
- `TMPDIR=/home/alex/.cache/codex-studio/tmp nice -n 10 just test-fast`
- After the fast gate builds `web/dist`: `node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts`
- `cargo clippy --workspace --exclude studio-native-client --all-targets --locked -j 4 -- -D warnings`
- `cargo clippy --workspace --exclude studio-native-client --all-targets --locked -j 4 --features studio-runtime-server/perf-reporting,studio-runtime-services/test-support,studio-runtime-services/bench-support -- -D warnings`

Gate results and the exact submitted state are recorded in the Studio task.
