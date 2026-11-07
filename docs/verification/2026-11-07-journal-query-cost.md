# Journal query cost

Base: `origin/main`, Git `1b179de49`, Layr
`9023fdc418cb77aaabd301a1b41f6e9b`. The live server ran `ff501dfe` with
`perf-reporting`. No delivery, restart, live SQL write, or push was performed.

## Live evidence

On 2026-11-07 at 03:56 UTC, a single read of `GET /api/perf-reporting`
reported the deleted-command task query at 146.818 seconds over 637 calls,
62,932,847 VM steps. Task-window eviction reported 34.288 seconds over 2,529
calls. These are cumulative instrumented SQL durations, not wall-clock CPU time.

A 30-second read-only profile collected 5,206 samples, with no lost samples.
Inclusive sampled CPU was 34.82% in `journal_one`, 10.15% in
`complete_deleted_command`, and 6.85% in the task-window eviction query call.
The profile and endpoint snapshot are under
`/home/alex/.cache/codex-studio/tmp/perf-journal-before.*`; the readable reports
are `perf-journal-children.txt` and `perf-journal-stack.txt` in the same folder.

Before relying on the task key, this single read-only live statement returned
**0** noncanonical rows at 04:00 UTC:

```sh
sqlite3 'file:/home/alex/.local/state/codex-agents/canvas.sqlite3?mode=ro' \
  "SELECT COUNT(*) AS noncanonical_task_keys FROM runtime_tasks WHERE id IS NOT (json_extract(record,'$.agent') || ':' || json_extract(record,'$.itemId'));"
```

## Changes and deterministic results

`TaskRecord::new` owns the task key, `{agent}:{itemId}`. Deleted-command
resolution now probes that primary key for each deleted actor in the account,
retaining the payload identity checks and every later recovery fence. The key's
uniqueness prevents two tasks for the same actor/item; ambiguity across actors
still returns two rows and is refused by the caller. Noncanonical legacy keys
are outside this production invariant and are not searched.

Task-window eviction now walks `task_window_state_history` first and probes
membership in `durable_rows` by its composite primary key. It stops at the
history boundary instead of starting with all retained window tombstones and
sorting. The existing offset is the fixed 100-row history boundary, not a
growing page offset. Newer tombstones or missing members can still require
extra probes; no new state or index was introduced.

The tests execute both the original SQL and the production query on the same
fixture and check equal results. Both new cost bounds failed before the SQL
changes. They measure VM steps rather than elapsed time:

| Query, fixture                                                              | Before VM steps | After VM steps | Reduction |
| --------------------------------------------------------------------------- | --------------: | -------------: | --------: |
| Deleted-command missing item, 10,000 historical tasks                       |          60,023 |             29 |    99.95% |
| Deleted-command existing item, either end of that history                   |          60,029 |             43 |    99.93% |
| Task-window eviction, 10,000 finished state rows, 102 live finished members |          51,559 |          1,230 |    97.61% |

The window fixture also includes a newer running member and a newer state row
without membership. Existing full-rebuild equivalence and deletion/replay tests
exercise the actual callers. The new witnesses are selected by the short tier.
These results do not claim a measured live CPU reduction after deployment.

## Commands

All Cargo commands used four jobs, left `CARGO_TARGET_DIR` unset, and set
`TMPDIR=/home/alex/.cache/codex-studio/tmp`. The first command was run before
the fixes and failed both cost witnesses (the window fixture expectation was
corrected and its failing cost bound was then rerun separately).

```sh
perf record -F199 -g -p 1058116 -o /home/alex/.cache/codex-studio/tmp/perf-journal-before.data -- sleep 30
curl -fsS http://127.0.0.1:4620/api/perf-reporting -o /home/alex/.cache/codex-studio/tmp/perf-journal-before.json
cargo nextest run --locked --workspace --exclude studio-native-client --lib -j 4 --profile ci -E 'test(deleted_command_query_cost_is_independent_of_task_history) | test(task_window_eviction_cost_stops_at_history_boundary)' --no-fail-fast
cargo nextest run --locked --workspace --exclude studio-native-client --lib -j 4 --profile ci -E 'test(task_window_eviction_cost_stops_at_history_boundary)'
cargo nextest run --locked --workspace --exclude studio-native-client --lib -j 4 --profile ci -E 'test(deleted_command_query_) | test(task_window) | test(account_worker::tests::stop_after_reattach::)' --no-fail-fast --success-output immediate
cargo clippy --locked --workspace --exclude studio-native-client --all-targets -j 4 -- -D warnings
cargo clippy --locked --workspace --exclude studio-native-client --all-targets -j 4 --features studio-runtime-server/perf-reporting -- -D warnings
node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts --exclude-native
nice -n 10 just test-fast
```

Focused validation passed all 27 selected tests. Strict Clippy passed in both
configurations. The initial ratchet invocation omitted `--exclude-native` and
failed because the native client's generated renderer bundle was absent; the
corrected command matches the gate's supported selection. Ratchet and fast-gate
results are recorded in the task submission after they complete.
