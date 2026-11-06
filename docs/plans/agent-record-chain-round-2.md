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

## Round 3, slice 1: typed dispatch and persistence

Base: Git `99c3cc7177da9af84694afd7b970a9a4dbb6e0e2`, Layr
`97591106a61b77c0b5de01a1b222d880`.

Dispatch, publication validation, `AfterAgent::Stored`, persistence, and
`kinds::apply` now carry `AgentRecord`. Persistence normalizes the mutable record
in place. The stream writers and usage updates also retain that record so their
new signatures introduce no conversion on a buffered text delta. Stream and
post-commit analytics borrow `NotificationAgent` directly.

Static complete-agent decode counts (excluding the existing test-only row
assertion): buffered text delta remains one repository decode; a text flush or
usage write goes from repository decode plus persistence decode to repository
decode alone (2 → 1). Idle observation removes its consuming decode (3 → 2, including the unchanged
post-dispatch row read).
`persist_agent` itself goes from one full decode to zero. Buffered text delta
also removes the notification-view deserializations; no additional agent read
or extension clone is introduced.

Temporary seams retained for the next slices, marked
`TODO(agent-record-chain r3)` at their boundaries:

- Notice/item snapshots, notice decisions and provider labels remain JSON.
- Question resolution and completed-question admission remain JSON.
- Shell-policy observation and scheduler receipt settlement keep their existing
  input conversions; receipt settlement still reloads its committed agent.
- Unbuffered reasoning and non-text item decisions still mutate JSON; their
  return conversion replaces the previous persistence decode.
- Recovery completion and submission authorization retain their external JSON
  callers. Task recovery and complaint authorization remain unchanged.
- Unhandled-notification counters remain in the existing agent remainder.

Value inventory for this slice (same counting method as above):

| File                                    |    All tokens |   JSON tokens |
| --------------------------------------- | ------------: | ------------: |
| `notification_dispatch.rs`              |      153 → 94 |      150 → 94 |
| `notification_dispatch/kinds.rs`        |       19 → 17 |       18 → 15 |
| `notification_dispatch/review_tests.rs` |         0 → 0 |         0 → 0 |
| `notification_dispatch/stream.rs`       |       35 → 33 |       34 → 33 |
| `notification_dispatch/submission.rs`   |       36 → 36 |       36 → 36 |
| `../store/lib.rs`                       |         6 → 6 |         2 → 2 |
| **Slice total**                         | **249 → 186** | **240 → 180** |

Provider payloads, transcript documents, raw evidence, and JSON test fixtures
retain their existing owners. `runtime-store/src/records.rs` is unchanged.

The store facade only re-exports the existing `AgentActivity` and
`AgentContextUsage` owners, allowing direct construction in stream writers.
The new typed persistence allocation witness shares the existing fast-tier
`notification_agent_write_preserves_legacy_row_bytes` selector.

## Round 3, slice 2: typed notification decisions

Base: Layr `5876b7fa8d1f700ca13701a1b23f4191` (slice 1).

Notice and item snapshots now own `AgentRecord`; decisions, task transitions,
reasoning activity, and question admission/resolution consume that checked
record. Notice status, safety, error, activity, and event fields are constructed
directly using their existing owners. Provider labels consume the checked
provider enum. Task recovery gives its already-read typed record to the snapshot.
The store facade re-exports five existing constructor types; `records.rs` remains
unchanged.

This removes the complete-agent return decode from notice decisions, non-text
items, unbuffered reasoning, and question resolution. Including initial and
post-dispatch repository reads, each of those paths goes from 3 full decodes to 2. Buffered text deltas are unchanged. The typed snapshot allocation witness
checks that moving the record into a snapshot retains unrelated extension storage.

Remaining seams: shell observation, scheduler receipt settlement, recovery
completion entry, and submission authorization. Two infrequent JSON entry points
(`questions::submit_user_message` and `agent_complaint_action`) each have one
explicit full-agent decode to call the typed complaint writer. These are marked
`TODO(agent-record-chain r3: authorization slice)` and must disappear with their
authorization callers. The lead approved these two temporary conversions to keep
one complaint implementation.

Value inventory for this slice:

| File                                 |    All tokens |   JSON tokens |
| ------------------------------------ | ------------: | ------------: |
| `notification_dispatch.rs`           |       94 → 94 |       94 → 94 |
| `notification_dispatch/kinds.rs`     |       17 → 17 |       15 → 15 |
| `notification_dispatch/items.rs`     |       19 → 15 |       19 → 15 |
| `notification_dispatch/notices.rs`   |       35 → 37 |       35 → 32 |
| `notification_dispatch/questions.rs` |       31 → 29 |       31 → 29 |
| `notification_dispatch/snapshot.rs`  |         8 → 5 |         8 → 5 |
| `../store/lib.rs`                    |         6 → 6 |         2 → 2 |
| **Slice total**                      | **210 → 203** | **204 → 192** |

Remaining JSON belongs to provider payloads, item/transcript metadata, notice
and complaint evidence, the deferred authorization callers, and test fixtures.
Typed `Field::Value` constructors account for the notice raw-token increase.

## Round 3, slice 3: shell observation and receipt settlement

Base: Layr `476bc3e8130e746fb51d01a1b24ec945` (slice 2).

Shell observation borrows `AgentRecord` from dispatch and the monitor callers,
removing its complete-agent decode and the preceding projection. The scheduler
receipt check borrows the existing typed start attempt, including its recovery
fallback, event list, account/thread affinity, and generation. The durable
settlement repository and confirmed-event checks are unchanged.

Shell notification dispatch goes from 3 full-agent decodes to 2 (initial and
post-dispatch repository reads remain); each monitor observation removes one
full decode. Receipt checking removes the agent projection and event-list clone;
its existing repository reads remain. Buffered text deltas are unchanged.
The typed shell observation witness first failed at the old JSON signature and
checks that a real item is written without changing the agent.

Value inventory for this slice:

| File                             |    All tokens |   JSON tokens |
| -------------------------------- | ------------: | ------------: |
| `notification_dispatch.rs`       |       94 → 94 |       94 → 94 |
| `notification_dispatch/kinds.rs` |       17 → 17 |       15 → 15 |
| `scheduler.rs`                   |       46 → 33 |       46 → 33 |
| `shell_policy.rs`                |       19 → 18 |       19 → 18 |
| `monitor_driver.rs`              |         5 → 5 |         5 → 5 |
| **Slice total**                  | **181 → 167** | **179 → 165** |

Scheduler lifecycle bodies, shell configuration/reports, monitor payloads, and
test fixtures retain their existing JSON owners. Recovery and authorization
caller integration follows as slice 4. Store sources are unchanged in this slice.

## Round 3, slice 4: recovery and local authorization callers

Base: Layr `2aca6c4ba393741197f701a1b255c784` (slice 3).

Task recovery, restart recovery, complaint actions, and local submission policy
retain their checked `AgentRecord`. Recovery still stages a clone before its
savepoint, so failed writes leave both the input agent and stream buffer intact.
Submission team and peer checks read typed records; provider, room, admission,
and event bodies keep their existing owners. Explicit null in the open
`activeExecution.attemptId` field still refuses current-attempt admission instead
of falling back to the start attempt.

For a restart completion containing N agent messages, this removes N entry
decodes plus the final persistence decode (N + 1 to zero at those seams).
Complaint submission removes its single projection/decode pair (1 to 0).
Task recovery and local submission remove full-agent projections; their existing
repository reads remain. The notification text-delta path is unchanged. The
recovery atomicity witness first failed against the old JSON signature, then
runs directly with the typed caller; complaint read/respond coverage also uses
typed records.

Value inventory for this slice:

| File                                                |    All tokens |   JSON tokens |
| --------------------------------------------------- | ------------: | ------------: |
| `notification_dispatch.rs`                          |       94 → 91 |       94 → 91 |
| `notification_dispatch/questions.rs`                |       29 → 28 |       29 → 28 |
| `notification_dispatch/review_tests.rs`              |         0 → 0 |         0 → 0 |
| `notification_dispatch/submission.rs`               |       36 → 21 |       36 → 21 |
| `account_worker/store.rs`                           |       20 → 20 |       18 → 18 |
| `account_worker/restart_recovery.rs`                 |         7 → 7 |         7 → 7 |
| `account_worker/connection_recovery.rs`              |       15 → 15 |       15 → 15 |
| `provider_callbacks/tools/services.rs`              |       37 → 37 |       37 → 37 |
| `provider_callbacks/tools/services/sends_tests.rs`   |       28 → 28 |       28 → 28 |
| **Slice total**                                     | **266 → 247** | **264 → 245** |

Two adapters remain by the lead's explicit scope decision, each marked
`TODO(agent-record-chain r3)` with its owner:

- `account_worker/connection_recovery.rs` converts once around the typed recovery
  writer. Follow-up owner: `recovery_store.rs::RecoveryMessageWriter`, whose
  callback still exposes a mutable JSON agent. Its API is not widened here.
- `notification_dispatch/questions.rs::submit_user_message` converts its JSON
  author once. Follow-up owner:
  `provider_callbacks/tools/services/messages.rs::send_as_actor`, whose
  authorization API remains unchanged.

Restart recovery still serializes the agent into its persisted recovered-notice
journal: that is evidence serialization, with no return decode. Provider bodies,
metadata, complaint views, the two deferred adapters, and test fixtures account
for remaining JSON. Unhandled notification counters retain their existing open
agent-extension owner. `runtime-store/src/records.rs` remains untouched throughout
all four slices.

## Round 3, slice 4b: use the existing moved-agent predicate

Base: Layr `c3da3755068277c6a27501a1b262a12b` (slice 4).

The CI clippy ratchet caught a new JSON conversion in the token-usage move-cache
condition. Dispatch now calls the existing `AgentRecord::moved_from_active`
predicate, which applies legacy truthiness directly to the stored raw field.
This removes the per-usage field conversion without changing complete-agent
decode counts. The existing legacy-shape parity witness covers the predicate.
No store source changes or lint exceptions are added.

The ratchet baseline shrinks for the removed `kinds::apply` redundant clone and
the complaint caller's removed JSON conversion. The ratchet was run failing
before this correction. The final fast gate covers slice 4 and this delta on
the exact combined state.

Value inventory: `notification_dispatch.rs` remains 91 → 91 for both all tokens
and JSON tokens. The change removes a conversion call rather than a type-name
reference. The two approved follow-up adapters listed in slice 4 are unchanged.
