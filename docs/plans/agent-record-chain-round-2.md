# Agent record chain, round 2

Base: Layr `dd1d24c9eba977f1909e01a1b0d252c9`, Git
`4e3ddaad67eaae6d857e38e7607738da9a634b34`.

This round keeps the persisted agent and notification receipt representations.
The notification repository returns its existing decoded `AgentRecord`; remaining
JSON dispatch callers own the consuming projection that previously lived inside
the repository. Account-worker effects borrow a small typed notification view
from the record. JSON dispatch decodes only that view, borrowing strings and
skipping unrelated fields; it never decodes another complete agent for effects.
Cache capture receives only the agent ID. Idle
observation reads its episode from the decoded record's remainder, without
fetching the row a second time. Settings resolution receives the lead flag.

Compatibility witnesses cover repeated legacy row bytes, frozen analytics
projection bytes, shared observation parameters, and idle prefix evidence. The
idle witness also checks that unrelated extension storage is not cloned. These
witnesses run in the fast tier.

## Remaining dispatch migration

Convert the following chain together, retaining the existing stored-byte and
stream-sharing witnesses:

- `dispatch_notification_with_effects`: keep the typed repository result through
  mutation and publication. `AfterAgent::Stored` and `persist_agent` should carry
  `AgentRecord` throughout, eliminating the remaining projection/decode seams.
- `PublicationContext::{validate_actor, scoped_delivery}` and
  `validate_agent{,_write_fields}`: consume checked fields directly, including
  typed execution affinity and notification counters.
- `kinds::apply`, `kinds::{turn, flush_command}`, `shell_policy::observe`, and
  scheduler receipt settlement: mutate the same typed agent.
- `NotificationSnapshot::{from_records, agent}`, `notices::Decision::agent`,
  `notices::{decide, provider_label}`, and `items::{task_transition,
  non_text, advance_reasoning}`: keep agent snapshots typed while
  provider item bodies remain with their existing owners.
- `questions::{completed, resolved, submit_agent_complaint}` and
  `recover_task_in_transaction`: keep the typed agent across question admission
  and task recovery.
- `write_text_notification`, `flush_notification_streams`, `turn_settled`,
  `reasoning_stream_key`, `flush_reasoning_stream`, `flush_selected`, `flush_keys`,
  `flush_text_stream`, `apply_token_usage`, and `update_common_agent_fields`:
  mutate typed activity, counters, status, and usage without reconstructing the
  complete agent. Stream analytics should receive the already-checked snapshot.
- Submission authorization and recovery callers: remove their final agent
  projections only after the team/permission helpers consume typed fields.

Do not replace these boundaries with parallel JSON and typed implementations.
A text delta must not gain a complete-agent decode, an unrelated extension clone,
or a second database read. Receipt replay must continue to skip effects.

## Value inventory

Counts compare the base above with this round, including tests. “All tokens”
counts every whole-word `Value`, including comments and typed `Field::Value`
constructors. “JSON tokens” excludes line comments and `Field::Value` variants,
which are typed presence markers rather than JSON values. Paths are relative to
`runtime-services/src`; `../store` denotes `runtime-store/src`.

| File                                              |    All tokens |   JSON tokens |
| ------------------------------------------------- | ------------: | ------------: |
| `account_worker/preparation/pipeline.rs`          |       43 → 43 |       42 → 42 |
| `account_worker/preparation/snapshot.rs`          |       78 → 78 |       78 → 78 |
| `account_worker/store.rs`                         |       21 → 20 |       19 → 18 |
| `idle_compaction.rs`                              |       19 → 27 |       19 → 19 |
| `native_actions.rs`                               |       54 → 54 |       54 → 54 |
| `notification_dispatch.rs`                        |     151 → 153 |     150 → 150 |
| `notification_dispatch/effects.rs`                |       15 → 25 |       15 → 10 |
| `notification_dispatch/kinds.rs`                  |       18 → 19 |       18 → 18 |
| `notification_dispatch/notices.rs`                |       36 → 35 |       36 → 35 |
| `notification_dispatch/review_tests.rs`           |         0 → 0 |         0 → 0 |
| `notification_dispatch/stream.rs`                 |       34 → 35 |       34 → 34 |
| `notification_dispatch/submission.rs`             |       36 → 36 |       36 → 36 |
| `notification_runtime_effects.rs`                 |         9 → 6 |         9 → 6 |
| `../tests/idle_compaction.rs`                     |         4 → 4 |         4 → 4 |
| `../store/lib.rs`                                 |         6 → 6 |         2 → 2 |
| `../store/repositories/notification_dispatch.rs`  |       15 → 14 |       13 → 12 |
| `../store/repositories/orchestration_settings.rs` |         1 → 0 |         1 → 0 |
| **Total**                                         | **540 → 555** | **530 → 518** |

The raw-token increase comes from typed presence constructors and documented
remaining dispatch boundaries. JSON references decrease; settings-only callers
retain their unrelated JSON code. Every remaining group is accounted for below.

- `account_worker/preparation/pipeline.rs`: Provider request parameters, preparation snapshots, pending metadata, and test fixtures remain with the preparation migration. Settings now consumes a boolean and no longer projects an agent for this lookup.
- `account_worker/preparation/snapshot.rs`: Preparation resources, provider configuration, snapshot validation/authorization, and fixture inputs remain JSON; the settings caller passes only the lead flag.
- `account_worker/store.rs`: Provider results/errors, provider journal frames, account pin configuration, retained stream metadata, and fixtures remain JSON. Agent post-commit snapshots and auth identity use typed fields.
- `idle_compaction.rs`: Episode identity/prefix/attempt/before values are opaque historical evidence. Existing refusal, unload, native admission, and handoff callers remain JSON. The cache report is decoded at ingress; two object matches encode new raw remainders. Test policy fixtures remain JSON.
- `native_actions.rs`: Native request parameters, receipt/result bodies, snapshots, policy ports, and their tests remain JSON. Settings resolution no longer accepts an agent document.
- `notification_dispatch.rs`: The deferred mutable dispatch chain, provider frames, signatures, transcript projections, and test fixtures remain JSON. Effect calls decode only the notification-agent view.
- `notification_dispatch/effects.rs`: SharedJson retains its transparent journal representation (storage, constructor, dereference, and deserializer). Recovered notification bodies, provider frames, and provider parameters remain JSON. All agent effect inputs are typed.
- `notification_dispatch/kinds.rs`: The deferred dispatch mutations, provider parameters, command/item metadata, and task/notice construction remain JSON. Idle observation moves into a typed agent at its single boundary.
- `notification_dispatch/notices.rs`: Provider notice inputs, pure notice decisions, metadata, account documents, and fixtures remain JSON. Analytics affinity consumes the turn ID and in-flight flag.
- `notification_dispatch/review_tests.rs`: No literal Value references; existing JSON fixtures exercise dispatch rollback and replay.
- `notification_dispatch/stream.rs`: Retained provider parameters, stream serialization/metadata, the deferred dispatch agent boundary, and stream fixtures remain JSON. Analytics decodes only the retained notification view when a capture exists.
- `notification_dispatch/submission.rs`: Submission bodies/assets, event metadata, and team/permission authorization remain JSON; existing dispatch authorization entry projections are explicitly retained.
- `notification_runtime_effects.rs`: Only provider parameters, frames, and timestamp extraction remain JSON. No agent input is JSON.
- `../tests/idle_compaction.rs`: Native admission test ports and test evidence remain JSON; observation callers hold AgentRecord.
- `../store/lib.rs`: The shared projection adapter and its tests retain their JSON return type. This change only exports existing idle record types.
- `../store/repositories/notification_dispatch.rs`: Related records, transcript items/text, and event metadata remain JSON. Agent reads and writes are typed.
- `../store/repositories/orchestration_settings.rs`: No remaining JSON Value references.
