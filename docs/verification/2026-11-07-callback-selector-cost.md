# Callback selector cost

Base: Git `18465ae52e9ed14dd4f71b36bbf4177f41912458`, the live release.
No migration, published contract change, push, delivery, restart, or live SQL
write was performed.

## Live evidence

A single `GET /api/perf-reporting` at approximately 18:47 UTC reported:

| Statement and job                                            | Calls | Cumulative SQL time |
| ------------------------------------------------------------ | ----: | ------------------: |
| Unsent callback selection, `runtime-services/callback_tasks` | 1,168 |             4.694 s |
| Reply-demand existence, `runtime-server/account_registry`    | 2,062 |             5.682 s |

These statements scan current tool requests by status before applying several
JSON predicates. A read-only live count found 890 current callback response
owners with no `payload.accepted`, all with a non-null generated agent.
There were zero rows with an accepted object and a non-null generated agent.
These counts support the diagnosis; the production-writer test enforces the
invariant used by the fix.

The supplied `studio-1846.perf.data` call graphs show JSON text conversion below
SQLite's JSON type/extract functions and B-tree seeks below its VM. A fresh
30-second read-only capture at approximately 18:51 UTC collected 1,311 samples
with no lost samples. Sampling identifies SQLite costs; the endpoint's labelled
statements identify the selectors. The profile alone does not attribute all
SQLite time to these two selectors.

Artifacts are under `/home/alex/.cache/codex-studio/tmp`: `sql-hot-before.json`,
`sql-hot-before.data`, `sql-hot-graphs.txt`, and `sql-hot-fresh-graphs.txt`.
The first `perf record` invocation using process discovery exited 255 without
output; the explicit, verified server PID capture succeeded.

## Change

`CallbackObligation` has no `identity` field. The generated callback `agent`
column, owned by `runtime-store/src/schema.rs::receipt_owner_columns`, reads
`payload.identity.context.agent`. Account-worker obligations therefore always
have a null generated agent, including after their accepted actor is bound.
Both selector predicates now add `agent IS NULL`, allowing the existing
`runtime_tool_request_current_status(status,agent,id)` index to exclude ordinary
callback response owners before fetching and parsing their records.

The accepted account/actor, status, generation, submission and recovery fences,
ordering and limits remain intact. No extra state or index is introduced.
Read-only `EXPLAIN QUERY PLAN` confirms a search on both status and agent.

## Deterministic witness

The new short-tier test writes a real obligation through `accept` and its
production writer, asserts `agent IS NULL`, and recovers it through `unsent`.
It also writes 1,000 unrelated current callback owners with 2 KiB bodies through
the callback writer. Missing-account queries must use fewer than 500 VM steps.
The unchanged selectors failed first at 15,083 steps for `any_unsent` and 5,020
for `has_reply_demand`. The same test verifies that a submitted running
obligation still appears in reply-demand existence and account enumeration.

| Selector           | Before VM steps | After VM steps | Reduction |
| ------------------ | --------------: | -------------: | --------: |
| `any_unsent`       |          15,083 |             72 |    99.52% |
| `has_reply_demand` |           5,020 |             26 |    99.48% |

`any_unsent` shares `TASKS` and `UNSENT` with the ordered recovery query; the
actual `unsent` caller is separately checked for successful recovery in the same
test. Focused validation passed all 35 callback selector, handoff, head-of-line,
replay, and reply-retirement tests.

Measured improvements are fixture results, not a measured post-delivery live
CPU reduction; the live server was left unchanged.

## Validation commands

All direct Cargo commands used `-j 4` and
`TMPDIR=/home/alex/.cache/codex-studio/tmp`, with `CARGO_TARGET_DIR` unset.
The ratchets used `CARGO_BUILD_JOBS=4` and the same temporary directory.
The fast gate is run once on the final state with its own job budget. Check
results and the submitted Layr state are recorded in the work item.

```sh
cargo nextest run --locked --workspace --exclude studio-native-client --lib -j 4 --profile ci -E 'test(callback_recovery_selectors_skip_unrelated_callback_owners)' --success-output immediate
cargo nextest run --locked --workspace --exclude studio-native-client --lib -j 4 --profile ci -E 'test(account_worker::callback_record::tests::) | test(account_worker::tests::callback_) | test(account_worker::tests::tool_reply_retirement::) | test(account_worker::tests::managed_reply_recovery::)' --success-output immediate
node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts --exclude-native
STUDIO_PERF_REPORTING=1 node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts --exclude-native
cargo clippy --locked --workspace --exclude studio-native-client --all-targets -j 4 -- -D warnings
cargo clippy --locked --workspace --exclude studio-native-client --all-targets -j 4 --features studio-runtime-server/perf-reporting -- -D warnings
just test-fast
```
