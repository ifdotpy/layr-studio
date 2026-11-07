# Agent publication decode cost

Base: live main `9fa8e7af154715ddd62c6f0bb114d79b90aaf5bf`.
Measurements taken on 2026-11-07. No server was started, restarted, or delivered.

## Live evidence

Read-only 30-second profiles used `perf record -F199 -g` against the installed
server process. Artifacts are in `/home/alex/.cache/codex-studio/tmp`:

| Capture                                   | Samples | `publish_agent_projection`, inclusive |
| ----------------------------------------- | ------: | ------------------------------------: |
| Prior `studio-826a.perf.data`             |   1,236 |                                13.77% |
| `studio-9fa8-projection.perf.data`        |   1,185 |                                 2.02% |
| `studio-9fa8-projection-second.perf.data` |   1,165 |                                 0.79% |

These captures have different live workloads and are not before/after measurements
of this change. Neither new capture sampled the reset-credit vector visitor.
The prior capture identifies its concrete element type as
`JsonObject<RateLimitResetCredit>` (11.71% self time), reached through
`RateLimitView` decoding in `normalized_limits_view`, `decoded_account_views`,
`publish_agent_change`, and `publish_agent_projection` inside the scheduler's
journal lifecycle transaction. The symbol mentioning `seed_work_details` is a
shared generic `publish_agents` instantiation, not evidence of repeated startup
seeding.

The read-only `GET /api/perf-reporting` snapshot is
`studio-9fa8-projection-report.json`. It records 10,974 `RateLimitView` decodes
(43,960,534 input bytes), including 7,098 decodes in the account-worker journal
write at `account_worker/store.rs:2311`. These are cumulative observations,
not rates. The JSON facade's largest measured decode type is `AgentRecord`;
source writes keep their existing record validation. Limits publication now uses
the existing borrowed `LimitAgent` view and validates its consumed fields.

## Deterministic witness and change

`agent_projections_decode_accounts_once_with_identical_output` publishes eight
agents and two accounts, with 128 reset credits per account. It first runs
individual publication in a savepoint, then rolls back and publishes the same
batch through `publish_entity_changes`. It compares persisted rate-limit
payloads, their sequence numbers, and agent-view durable marks. The fixture
includes an account move and checking status.

| Work per publication batch                | Before | After |
| ----------------------------------------- | -----: | ----: |
| Account-view decodes                      |     16 |     2 |
| Reset-credit elements decoded             |  2,048 |   256 |
| Full `AgentRecord` decodes                |      8 |     0 |
| Unused owned-work decodes, two owned rows |      2 |     0 |

The account-view and owned-work witnesses failed on the base implementation before
the production change. A further witness failed at eight full-record decodes on
the reviewed batch-only revision before replacing those reads with `LimitAgent`.
The account-view count comes from a test-only thread-local counter at
the existing decode boundary. Credit element counts follow from the fixed
128-element fixture. The existing owned-work probe counts each decoded work row;
the existing scoped agent-publication probe separately counts full-record decodes
at `parse_agent_record`, excluding JSON `Value` decodes.

Each `publish_agents` call now decodes its account views once and lends the mutable
slice to each per-agent publication. Each publication updates that local value
after writing, preserving publication order and subsequent agents' input. An
unreadable account is repaired once; a separate witness counts the repair delete,
checks that both agents remain projected, and compares batch and individual
sequence numbers for both invalid JSON shape and SQL-NULL payloads. Nothing is
retained after the call.
The incremental publisher also stops loading owned work that its callee does not
consume. Single-agent callers, source validation, entity contracts, and durable
marks keep their existing owners. The shared narrow decoder checks row identity,
the safe epoch range, and the types of consumed fields. Full rebuild work loading is outside this
incremental-path change.

## Validation

The first revision passed 111 focused publication, limits-runtime, and sync-policy
tests and all 27 fast-gate steps. Final review-fix results are recorded with the
submitted task state.
Strict Clippy passed in both default and perf-reporting configurations, with only
the existing `large_futures` and `wildcard_enum_match_arm` diagnostics allowed.
The bare-main ratchet reports those existing diagnostic classes; the comparison
and fast-gate results are recorded with the submitted task state.
