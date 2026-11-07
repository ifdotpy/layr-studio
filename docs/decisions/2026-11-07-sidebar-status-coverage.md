# Operational status coverage migration

Contract v5 moves precedence from `chatStatusModel.ts` to Rust. This inventory
maps the former TypeScript assertions (source state
`f21f47d196917071b05501a1b40f6644`) to their domain-owner witnesses.
Formatter, pending-read overlay, activity, wait display, provider CSS and urgency
remain renderer tests. Rust test names below are in `chat_status` in
`studio-read-models` or `agent_operational::owner_matrix_tests` in
`studio-runtime-services`, unless qualified otherwise.

| Former precedence cases                                                                                                                                                                                   | Rust witness                                                                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Completed-result thread/turn/status validity; matching, mismatching and missing receipt; read flag and saved revision                                                                                     | `chat_status::receipt_matrix::v5_receipt_identity_read_revision_and_completion_matrix`; `chat_status::receipt_and_deferral_tests::v5_read_target_reuses_saved_revision_but_requires_matching_completed_result` |
| Running, starting and queued; in-flight work with auto-wake disabled; empty and nonempty stopped agents                                                                                                   | `v5_root_activity_autowake_and_inflight_matrix`; `chat_status::receipt_and_deferral_tests::v5_deferral_and_empty_stop_rules_do_not_mask_current_attention`                                                     |
| Current question/approval overrides active, failed and unread; absent/null/current/stale epochs; answered/deferred inputs; summary attention with empty panels; stale approval cannot resurrect attention | `v5_request_epoch_state_and_empty_panel_attention_matrix`; `entity_publication::tests::v5_child_input_epoch_and_resolution_invalidate_the_parent_summary`                                                      |
| Failed, interrupted, paused, auto-wake disabled; deferred approval; unread cannot override a stopped/failed turn                                                                                          | `chat_status::tests::v5_persisted_unread_does_not_override_a_stopped_or_failed_turn`; `chat_status::receipt_and_deferral_tests::v5_deferral_and_empty_stop_rules_do_not_mask_current_attention`                |
| Disk-full hold versus failure, with current thread/epoch/account/cause and scheduled, unclaimed retry                                                                                                     | `v5_disk_retry_requires_current_thread_epoch_account_and_unclaimed_schedule`; `chat_status::tests::v5_persisted_unread_does_not_override_a_stopped_or_failed_turn`                                             |
| Complaints, inbox/user tasks and work records do not replace chat status, including child inbox records                                                                                                   | `v5_unrelated_complaints_and_work_are_not_operational_inputs`                                                                                                                                                  |
| Immediate-child queued/starting/running/approval and completed-in-flight count; archived, unmanaged, wrong-parent and stopped children excluded                                                           | `v5_child_status_and_parked_event_matrix`; `v5_children_prune_unmanaged_and_stopped_branches_but_root_activity_uses_root_owner`                                                                                |
| Task/monitor status and owner filtering; commandless command; non-command generic tool; foreground current-turn versus background command; no invented process epoch filter                               | `v5_process_owner_status_turn_and_kind_matrix`                                                                                                                                                                 |
| Waiting child with live command/monitor; stopped branch pruning; active command on a stopped child still affects its root                                                                                 | `v5_children_prune_unmanaged_and_stopped_branches_but_root_activity_uses_root_owner`; `v5_stopped_child_process_and_input_keep_root_attention_without_child_wait`                                              |
| Parked child event and auto-wake disabled; event absent/empty; current input counts and exclusions                                                                                                        | `v5_child_status_and_parked_event_matrix`; `v5_request_epoch_state_and_empty_panel_attention_matrix`                                                                                                           |
| Nested waits propagate bottom-up without sibling leakage or repeated roster reads                                                                                                                         | `agent_operational::tests::v5_operational_batch_has_four_indexed_reads_and_bottom_up_waits`; stopped-branch and process matrices above                                                                         |
| Wide roster of 1,024 children has one indexed batch                                                                                                                                                       | `v5_wide_roster_1024_uses_one_batch`; four-indexed-read witness above                                                                                                                                          |
| In-flight active work stays working; active without an in-flight turn and live command/monitor becomes waiting; waiting background command precedence                                                     | `chat_status::tests::v5_active_wait_and_input_precedence_use_the_same_checked_reasons`; process matrix above                                                                                                   |
| Child input propagates to root; root without local wait remains needs-input with null wait; unmanaged source excluded                                                                                     | `v5_stopped_child_process_and_input_keep_root_attention_without_child_wait`; child-input publication witness above                                                                                             |
| Orphan input stays local; missing and empty root IDs never transfer attention to an unrelated identity                                                                                                    | `v5_orphan_input_is_local_and_empty_root_never_propagates_to_an_empty_identity`                                                                                                                                |

The old tests named “maps live runtime records”, “uses the runtime command, turn,
epoch and fallback rules”, “builds live wait labels”, and their independent
activity/wait assertions in mixed tests remain TypeScript coverage. Receipt
helper assertions remain beside the renderer receipt helper. Only assertions
that invoked the removed precedence implementation are retired after the Rust
matrix passes.

`src/status/operationalStatus.test.ts` covers every reason's label and kind,
wait count pluralization/event text, explicit missing-summary neutrality and
exact pending-read proof retirement. It also supplies contradictory raw status
facts to verify that both sidebar and conversation trust the reason code.
`src/components/sidebar/viewModel.test.ts` covers lazy search, tail-independent
order/grouping, observed membership, compact visibility and raw model identity.

The migration audit compares assertion syntax against the source state above:
all 106 independent assertions remain. The urgency assertion adds the explicit
loading variant. The separate 1,024-roster precedence witness moves to Rust.

## Wave 1: team, worker and conversation status

The reference for this addendum is `chatStatusModel.test.ts` at main
`e98e724e` / Layr `f1b929e1`. An assertion site is an `assert(...)` or
`assert.method(...)` call in its syntax tree; loops do not multiply the count.
The file had 106 sites. Its replacement, `chatActivity.test.ts`, keeps all 66
independent renderer sites: provider/CSS (4), visible activities (40), explicit
wait labels (13), receipt identity (8), and urgency (1). The other 40 sites
selected or counted domain wake triggers. Their behavior moves to the Rust
witnesses below. Thus all 106 original sites are accounted for, with 66/66
independent client assertions retained and 40/40 precedence sites mapped.
No assertion is removed merely to meet the production-line target.

The ordinal ranges below count assertion sites within each original test,
including calls inside loops. Rust names are in
`agent_operational::owner_matrix_tests` unless qualified. The focused workspace
run of all existing `v5_` tests passed 24/24; the added retirement matrix and
extended child-status matrix subsequently passed 2/2 before the final gate.

| Original test and sites                                                                        | Retained client behavior or Rust owner witness                                                                                                                                                                                                                                                                                                                                                                             |
| ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `activity CSS selectors consume every generated provider identity` (1)                         | Same CSS/provider parity assertion in `chatActivity.test.ts`.                                                                                                                                                                                                                                                                                                                                                              |
| `chat status preserves unknown and missing identities without a provider default` (1–3)        | Same three assertions and provider cases, using a model-aware status leaf.                                                                                                                                                                                                                                                                                                                                                 |
| `preserves visible activity and owned wait rules beside the server reason formatter` (1, 3–16) | All 15 receipt/activity assertions remain, including stopped-worker command visibility, root ownership, current/prior-turn background classification and fallback activity.                                                                                                                                                                                                                                                |
| Same test (18, 21, 26–27, 35–36, 40–43)                                                        | All ten exact label assertions remain with explicit server wait counts and the shared formatter. Domain trigger selection is separately covered below.                                                                                                                                                                                                                                                                     |
| Same test (2, 22–25)                                                                           | `v5_child_status_and_parked_event_matrix`, `v5_children_prune_unmanaged_and_stopped_branches_but_root_activity_uses_root_owner`, and `v5_wait_detail_retirement_matrix`: child activity, completed-in-flight, idle/terminal/unmanaged/wrong-parent exclusion, and waiting children with live commands/monitors.                                                                                                            |
| Same test (17, 19–20, 28–34)                                                                   | `v5_process_owner_status_turn_and_kind_matrix` and `v5_wait_detail_retirement_matrix`: empty waits, completed/failed and wrong-owner processes, generic tools, commandless commands, current-turn foreground work, and stopped-turn commands.                                                                                                                                                                              |
| Same test (37–39)                                                                              | `v5_request_epoch_state_and_empty_panel_attention_matrix` and `v5_orphan_input_is_local_and_empty_root_never_propagates_to_an_empty_identity`: absent collections mean no pending rows; absent/null request epoch, current/stale requests, answered/deferred and other-owner exclusions.                                                                                                                                   |
| `keeps a parent waiting for a child with current command or monitor work` (1–6)                | `v5_wait_detail_retirement_matrix` checks command and monitor starting/running/approval states through waiting children, stopped/archived/unmanaged/auto-wake-disabled branches and generic-tool exclusion.                                                                                                                                                                                                                |
| `counts active work through waiting descendants without crossing stopped branches` (1–5)       | `agent_operational::tests::v5_operational_batch_has_four_indexed_reads_and_bottom_up_waits`, `v5_wait_detail_retirement_matrix`, and the stopped-branch matrix cover nested propagation, branch pruning, no live process, sibling separation and terminating parent cycles. `WorkerCard.render.test.ts` additionally checks that the displayed child names retain the exact generated models and direct parent identities. |
| `keeps the parent waiting for a parked child's saved event` (1–2)                              | `v5_wait_detail_retirement_matrix` checks the saved event with a live monitor and all null/empty-event, waiting/stopped/failed/interrupted/archived exclusions while auto-wake is disabled.                                                                                                                                                                                                                                |
| `rejects malformed request epochs while preserving absent, null and numeric rules` (1)         | `v5_request_epoch_state_and_empty_panel_attention_matrix` covers absent/null/current/stale request epochs. The source fixture actually contains those four valid cases, not a malformed epoch.                                                                                                                                                                                                                             |
| `maps live runtime records to sorted caller-visible activities` (1–13)                         | All 13 independent assertions remain on the same activity formatter. Its input now comes from generated agent/process models; the legacy Snapshot adapter exists only in the test fixture.                                                                                                                                                                                                                                 |
| `uses the runtime command, turn, epoch and fallback rules for activities` (1–12)               | All 12 independent assertions remain, including command/query/name/kind fallback, unknown root, missing process epoch, background classification and synthetic activity. P1's `process_controls::tests::activity_snapshot_prefers_process_then_restores_task_after_exit` regression also checks the serialized process turn identity used by the caller.                                                                   |
| `builds live wait labels only from current owned triggers` (2, 9, 11)                          | All three exact label assertions remain on explicit server facts.                                                                                                                                                                                                                                                                                                                                                          |
| Same test (1, 3–8, 10)                                                                         | `v5_child_status_and_parked_event_matrix`, `v5_process_owner_status_turn_and_kind_matrix`, `v5_request_epoch_state_and_empty_panel_attention_matrix`, and `v5_wait_detail_retirement_matrix` own live membership, process identity, parked events and input counts. Generated-model detail-name assertions remain separately in the worker render witness.                                                                 |
| `keeps local completion receipt identity checks` (1–8)                                         | All eight assertions retain the legacy oracle in test-only `readReceiptFixture.ts`; production read eligibility comes exclusively from `markRead`/`markUnread`. Rust `studio_read_models::chat_status::tests::v5_receipt_identity_read_revision_and_completion_matrix` owns the independent receipt conditions.                                                                                                            |
| `orders every indicator kind once by operator urgency` (1)                                     | Same assertion against the formatter-owned ordering.                                                                                                                                                                                                                                                                                                                                                                       |

The old `noEpochAgent` fixture in the mixed wait-label test had a null **agent**
epoch. It is not a valid stored agent and does not justify a compatibility
branch. `runtime-store/src/records.rs` requires `AgentRecord.epoch: i64`
(lines 351–352), its constructor requires an epoch (1330–1345), and
`deserialize_agent_epoch` rejects null/noninteger/out-of-range input
(1047–1056). The persisted-agent schema requires the nonnullable integer epoch
(schema lines 304–309 and 511). SQLite stores `record TEXT NOT NULL`, not an
independent epoch column (`agent_columns.rs:305`); malformed raw SQL can bypass
the contract, but typed business decoding rejects it (`records.rs:3247–3254`).
Normal inserts and replacements use the checked repository path
(`records.rs:3031–3038`, `3296`); volatile writes preserve that checked record,
idle-compaction SQL changes only its own episode fields, and migrations do not
remove the required epoch. The old label assertion remains as a formatter
assertion for an explicit one-input server wait. Absent or null **request**
epochs remain valid and covered by the owner matrix.

`chatStatusModel` and `ChatWaitSource` are deleted. Operational labels and live
waits use only `agentSummary.operationalStatus`; a small computed direct-child
view and generated `processes(agentId)` provide expandable names. Activity
mapping and receipt helpers retain their independent responsibilities. The
process view's server-published `turnId` preserves the current-turn/background
split without a Snapshot selector in App. Radio-room unread aggregation also reads the shared formatter and pending-read overlay over generated summary models. `useChatReadState` preserves the pending-read/CAS lifecycle while taking its completed-result target from generated `markRead`; the menu reads `markUnread`.

### Retired raw status labels and approved visible label alignment

The owner explicitly approved one label source even where the old worker line
used different copy. The worker activity text and its title now use the shared
operational formatter. Error details, command progress stages, phase icons and
transport notices keep their independent presentation.

| Old worker activity text/title                                                                                           | Checked reason → shared text                                                                        |
| ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `Finished`                                                                                                               | `unread_result` → `Unread result`; `read_result` → `Read`                                           |
| `Thinking`, `Writing`, `Using tools`, `Codex is retrying`, `Restoring sign-in`, `Codex reported an error`                | `working` → `Working`; the phase row still presents phase detail separately                         |
| `Starting`, `Queued`, `Preparing folder`, `Waiting to prepare folder`, `Waiting for Codex`, or a client start-wait label | The server's `working` → `Working`, or its checked wait reason → exact shared wait label            |
| `Waiting on command` / client command-progress wait title                                                                | `waiting_background_command` → `Waiting for background command`, or `waiting` → checked wait counts |
| `Turn ended`                                                                                                             | `turn_ended` → `Turn ended. Send a message to continue.`                                            |
| `Needs an answer` (dead raw-status helper only)                                                                          | `needs_input` → `Needs your answer`, or checked wait counts                                         |
| `Complete` (dead raw-status helper only)                                                                                 | `unread_result` / `read_result` as above                                                            |
| A native-release suffix appended to the worker title                                                                     | The shared reason label alone; native release detail remains in its detail surface                  |

`types.statusLabel` has no remaining production caller. Its raw-status test
matrix is retired with the function: idle maps to `ready`, running/starting/
queued to `working` or a checked wait, approval to `needs_input` or
`question_deferred`, waiting/parked to `waiting` or `turn_ended`, completed to
`unread_result`/`read_result`, failed to `failed`/`disk_space`, paused to
`stopped`, and interrupted to `interrupted`. Unsupported raw statuses do not
create a client fallback claim. The shared formatter's exhaustive reason tests
remain the label contract. The 106-site chat-status accounting above is separate
from this explicitly approved retirement of an unused raw-label API.

### Separate summary/detail caller audit

Removing the old full-agent spread exposed callers that still expected detail
facts on a summary. The regression fixtures now construct separate generated
summary and detail mirrors and exercise the actual caller, including loading to
live and in-place detail changes. Summary models are never enriched with detail
fields.

| Caller                                                        | Owning facts and correction                                                                                                                                                 | Retained regression                                                                                 |
| ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| ChatSettingsDialog / UnifiedAgentSettings / ExecutionSettings | Settings, concurrency/version, permissions, effort and worker/reviewer defaults come from observed `agentDetail`; receipt proofs capture values, not mutable model identity | Settings values, permission availability, role exclusions, canonical receipts and loading readiness |
| AgentPhase / request cards / App native actions               | Thread identity comes from summary; native block/error comes from detail; actions wait for detail readiness                                                                 | Block visible, answer/approval guards, native action disabled state and later slot arrival          |
| Usage / execution summary trigger                             | Context usage, compactions and role defaults come from observed detail                                                                                                      | Context percentage, compaction count, worker/reviewer labels and live mutation                      |
| ClaudeSettings / TurnHistory                                  | Native error/status and turn error come from observed detail                                                                                                                | Native sign-in error and failed-turn detail                                                         |
| Worker error/menu leaves                                      | Error reads exclude unrelated activity/progress computations                                                                                                                | Activity-only mutation produces zero error/menu commits; error mutation updates its leaf            |

`generatedAgentConsumers.render.test.ts` preserves these caller boundaries with
14 probes. Existing settings, receipt, request, transcript and usage assertions
remain. Standalone browser and story fixtures explicitly provide private
generated models; controlled fixture updates apply to those same mirrors.

The settings follow-up uses one explicit input contract: a server agent carries
its summary and observed detail model; a new-chat draft carries its local draft.
The settings controller opens no detail resource. Capability reads (including
next-turn support and legacy agent mode) belong to detail or draft. Status effects
track account and pending-account values, and asynchronous settings requests
capture their native id/account before awaiting. The receipt suite now uses
separate, stable generated summary/detail mirrors, including in-place replay;
all 67 receipt assertions remain. Real-caller probes additionally cover draft
model/effort/role edits with zero detail subscriptions, supported running-chat
queueing, account mismatch during a save and legacy single-agent display.
