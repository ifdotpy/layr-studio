# Architecture completion and acceptance criteria

Snapshot date: 2026-11-07. The target architecture is incomplete.

This file contains the unfinished work, source evidence, owner decisions, and acceptance criteria from the reviewed conversation.
The groups describe scope, not an implementation order. The owner asked for the complete design without feature loss.

**Evidence and limits**

- Conversation: `/Users/igor/.claude/projects/-Users-igor-Projects-codex-agents/ba7074e4-92b1-4491-a001-077d217c7966.jsonl`.
- Tracking issue: [#630](https://github.com/ifdotpy/layr-studio/issues/630).
- Design: [target architecture](../decisions/2026-10-28-target-client-server-architecture.md).
- Additional design: [project peering](../decisions/2026-11-01-p2p-project-peering.md).
- Inspected line: `/Users/igor/Projects/layr-store/projects/studio/lines/arch-events`.
- Inspected saved commit: `a2bcc787c83bc5c4b2a2eea30f3d805ac4f82f15`, plus the 13 unsaved files listed below.
- Main reference during the initial inspection: `b70e3cb07`. Main and `arch-events` contain different changes.
- Source paths below are relative to the inspected line unless another root is explicit.
- The audit read the user messages, question-tool answers, relevant agent reports, source, and saved test logs.
- The audit did not build code, run tests, install a package, or change runtime state.

The issue checkboxes and agent summaries are historical evidence. They do not establish the current implementation or a successful live check.
Unchecked acceptance criteria mean that this file contains no complete acceptance evidence. Some corresponding code already exists.
A completed criterion needs an exact revision, a named check, its result, and any environment limit.

**Owner decisions that govern acceptance**

| Decision             | Accepted outcome                                                                                                     | Conversation evidence                                                                                                    |
| -------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Full scope           | Complete the architecture and preserve features. Do not stop at an isolated component.                               | 2026-10-30 full implementation request; 2026-11-04 request to write all remaining code; 2026-11-06 "все в любом порядке" |
| Ownership            | The server owns state and business logic. Tauri connects to it.                                                      | 2026-10-28 "One process - server, but tauri connects to it"                                                              |
| Frontend core        | Move stateful frontend core behavior to the server. Share pure Rust functions and contracts.                         | 2026-10-28 design discussion; 2026-11-05 clarification "Код front core с backend" and "Go"                               |
| Browser storage      | Keep the thin cache, unsent input, and device key. Do not restore RxDB or Dexie.                                     | Final cache answer on 2026-11-04, JSONL line 24918                                                                       |
| Slow networks        | Merge live deltas for a slow client. Use the compressed 8 KB inline text threshold.                                  | JSONL line 24918                                                                                                         |
| Stream ownership     | Build remaining views at read time. Do not retain copies merely to reduce implementation effort.                     | Stream answer at line 27147; final approach answer at line 55516                                                         |
| Data retention       | Keep all messages and conversation history. Remove only redundant or expired service data under its contract.        | 2026-10-28 "нет, сообщения все храним"                                                                                   |
| State model          | Use typed tables and strict structs/enums. Do not make the conversation or an event store the source of state.       | 2026-10-28 type and event-store discussion                                                                               |
| Federation           | Keep federation. Remove the obsolete v1 signature implementation.                                                    | 2026-10-28 federation decision; answer at line 39656                                                                     |
| Other owners         | Use peer-to-peer project pairing with zero trust. Leads communicate as equal team members.                           | 2026-11-01 "Давай p2p. Пейринг per project и zero trust"                                                                 |
| Owner trust          | Tailscale remains the trusted perimeter for the existing owner access path.                                          | 2026-10-28 explicit trust decision                                                                                       |
| Trusted paired peers | Keep broad reads for the existing trusted peer role. Do not apply this grant to cross-owner project peers.           | 2026-11-04 "Оставить всё открытым"                                                                                       |
| QUIC checks          | Keep per-request signatures and contract checks. The earlier once-per-connection proposal is superseded.             | Answer at line 27147                                                                                                     |
| Local network        | Implement the local-network route and pairing. The owner already approved this work.                                 | Answer at line 27147; 2026-11-04 "нового pairing по LAN без Tailscale - да"                                              |
| Drafts               | One server draft per chat. The last device write wins. Preserve each device's unsent input until confirmation.       | Answer at line 39656                                                                                                     |
| Client migration     | Remove `store_move` and the client SQLite dependency after the accepted move-window closure.                         | Answer at line 39656                                                                                                     |
| Markdown             | Use shared pure Rust Markdown code through WebAssembly for the web frontend and directly for a native client.        | 2026-11-01 Markdown discussion and acceptance                                                                            |
| Maintainer agreement | Sasha agreed to the whole design. This is not an unresolved owner decision.                                          | 2026-10-30 "саша согласен с дизайном"                                                                                    |
| Delivery scope       | The last implementation goal allowed local code and tests, without a pull request. The owner then stopped that goal. | Final goal reports; 2026-11-07 "остановись"                                                                              |

This file records requirements. Its creation does not restart implementation or authorize publication, installation, or live model requests.

**Interrupted change**

The saved `model` change is `83e36ceb1`. The saved `terminal` change is `a2bcc787c`.
The earlier agent report that called `model` unsaved is obsolete.

The unsaved change moves `queue` documents to read-time construction through `durable_rows`.
It also changes the `peerTeam` migration and view hashes so the first subsequent change receives a new sequence number.

All 13 unsaved files are under `workspaces/runtime/`:

| File                                                                       | State          | Purpose                                               |
| -------------------------------------------------------------------------- | -------------- | ----------------------------------------------------- |
| `packages/runtime-services/src/entity_scope.rs`                            | Modified       | Read queue documents from the durable window          |
| `packages/runtime-services/src/queue_write.rs`                             | Modified       | Build queue views and mark changed revisions          |
| `packages/runtime-services/src/sync_policy.rs`                             | Modified       | Check queue seeds and update peer-team view hashes    |
| `packages/runtime-services/tests/test_support/agent_views.rs`              | Modified       | Include terminal and queue views in the test reader   |
| `packages/runtime-services/tests/wave7_p03_message_ingress.rs`             | Modified       | Adjust the comparison of stored entity copies         |
| `packages/runtime-services/tests/wave7_p19_radio.rs`                       | Modified       | Apply the queue migration in the fixture              |
| `packages/runtime-store/src/durable_seq.rs`                                | Modified       | Add the queue window and shared view-hash updates     |
| `packages/runtime-store/src/entity_publication.rs`                         | Modified       | Find queue owners through the durable window          |
| `packages/runtime-store/src/migrations/m2026_11_06_peer_team_read_time.rs` | Modified       | Preserve the old peer-team view hash during migration |
| `packages/runtime-store/src/migrations/mod.rs`                             | Modified       | Register the queue migration                          |
| `packages/runtime-store/src/repositories/projects.rs`                      | Modified       | Remove a deleted project's peer-team window and hash  |
| `packages/runtime-store/src/repositories/sync_documents.rs`                | Modified       | Classify queue documents as direct views              |
| `packages/runtime-store/src/migrations/m2026_11_06_queue_read_time.rs`     | New, untracked | Migrate queues and test sequence changes              |

**Last test result**

Saved log: `/private/tmp/claude-501/-Users-igor-Projects-codex-agents/ba7074e4-92b1-4491-a001-077d217c7966/scratchpad/full.log`.
Nextest run: `9572629c-433f-4093-9966-e079b24b6a47`.
Result: 6310 tests ran, 6306 passed, 4 failed, and 73 were skipped.
The wrapper task reported exit code zero, but the test log reports failure. The wrapper result is not acceptance evidence.

| Failed test                                                                                           | Observed assertion                                                 | Unfinished check                                                                                               |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `entity_publication::tests::message_send_projection_upgrade_rebuilds_receipts_and_queues`             | The expected incomplete seed is already complete.                  | The fixture still changes queue copies in `sync_entities`. Establish the correct old-schema migration fixture. |
| `agent_management_store::incremental_lifecycle_projection_matches_full_rebuild`                       | Pause does not change the compared map.                            | The map reads only `sync_entities`. Compare the documents that readers actually receive.                       |
| `wave7_p10_lifecycle::production_interrupt_is_durable_post_commit_and_retry_coalesces_after_rollback` | The operation succeeds where the test expects a transaction error. | The failure triggers target `sync_entities`. Inject failure at the current publication boundary.               |
| `wave7_p10_lifecycle::lifecycle_store_failpoints_and_missing_consumer_leave_no_writes`                | The operation succeeds where the test expects a transaction error. | Preserve rollback coverage after the queue leaves `sync_entities`.                                             |

These source findings explain stale test assumptions. They do not prove that the implementation is correct.

**Unfinished work inventory**

The item numbers come from the previous agent's issue-line references. They are not separate issue numbers or stable identifiers.
Acceptance identifiers in this file remain independent of those line numbers.

| Items      | Status at inspection                                | Remaining result                                                                                                     | Criteria   |
| ---------- | --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ---------- |
| 127        | Partial; queue unsaved                              | Finish queue, room, studioError, workspace, and transcript hash rows. Preserve migrated project fields.              | S1 to S8   |
| 128        | Incomplete                                          | Remove remaining entity copies and the old copy-publication path.                                                    | S1, S8     |
| 63         | Partial                                             | Finish Chat, Turn, Session ownership. Preserve `turnId` facts outside an active turn before removing the JSON field. | D1         |
| 64         | Incomplete                                          | Implement Delivery variants, required timestamps, storage checks, and all writers.                                   | D2         |
| 65, 159    | Shared draft logic exists; storage incomplete       | Store the draft on Chat and simplify client draft modules.                                                           | D3, C3     |
| 68         | Incomplete                                          | Replace JSON-owned Work state with typed owner columns.                                                              | D4         |
| 72         | Partial                                             | Fold execution tables into turn attempts and the shared outbox.                                                      | D5         |
| 73         | Incomplete                                          | Make tool calls and their results Items.                                                                             | D6         |
| 74         | Incomplete                                          | Relocate radio settings, `project_deleted_at`, and `userHidden` before removing `runtime_rooms`.                     | D7         |
| 75         | Comparison tool exists                              | Resolve unmatched task data before any removal of `runtime_tasks`.                                                   | D8         |
| 95 to 97   | Partial                                             | Route all write actions, agent tools, and peer orchestration through commands.                                       | K1, K2     |
| 98         | Partial; design conflict remains                    | Map remaining result families without losing exact external outcomes.                                                | K3         |
| 103        | Incomplete                                          | Restore edited turns through Layr snapshots instead of Git-only checkpoints.                                         | K7         |
| 106 to 108 | Partial                                             | Complete pure decide, atomic domain/outbox writes, and immediate effects after commit.                               | K4 to K6   |
| 110        | Partial                                             | Complete recovery rules per effect. `AskTarget` is absent from the inspected restart enum.                           | R1 to R4   |
| 113        | Unverified                                          | Prove recovery without a supervisor. The report does not establish completion.                                       | R5         |
| 134        | Incomplete, already authorized in the original task | Implement local-network routes and pairing without Tailscale.                                                        | N4         |
| 139        | Reported incomplete                                 | Measure analytics storage and complete the online rebuild.                                                           | B4         |
| 144        | Partial                                             | Apply branded TypeScript identifiers to all agreed identifier contracts and callers.                                 | T2         |
| 146        | Incomplete                                          | Decode stored content directly into strict types.                                                                    | T3         |
| 148        | Partial                                             | Convert existing untagged internal contracts. A guard against new cases is insufficient.                             | T4         |
| 155        | Incomplete                                          | Move peer-update steps to commands. The two old files contain 2841 lines.                                            | M2         |
| 156        | Incomplete                                          | Move large agent text to blobs and replace the `context_repair_phase` dependency safely.                             | B2         |
| 157        | Incomplete                                          | Replace retained Python helpers and prove the recovery owner before removal.                                         | M3         |
| 165 to 167 | Evidence incomplete                                 | Measure analytics and provider duplicate/probe behavior.                                                             | B4, R2, R3 |
| 168        | Overall agreement already given                     | Record actual overlap or ownership issues without asking again for design approval.                                  | V4         |
| 172        | No complete live evidence                           | Verify the integrated, installed design when delivery is authorized.                                                 | V3         |

Key source evidence:

- `workspaces/runtime/packages/runtime-services/src/entity_scope.rs`: mixed durable and `sync_entities` readers.
- `workspaces/runtime/packages/runtime-store/src/repositories/sync_documents.rs`: direct collection list and remaining entity storage.
- `workspaces/runtime/packages/runtime-store/src/feature_schema.rs`: Work, execution, tool-request, and room storage.
- `workspaces/runtime/packages/runtime-store/src/agent_volatile.rs` and `records.rs`: chat-owned fields and retained turn identifiers.
- `workspaces/runtime/packages/runtime-services/src/commands.rs`: current command coverage and separate execution paths.
- `docs/plans/request-results-v2-mapping.md`: reference candidates and external result bodies that still need an owner.
- `workspaces/runtime/packages/runtime-services/src/checkpoints.rs`: Git `commit-tree`, `update-ref`, and `read-tree` operations.
- `workspaces/runtime/packages/runtime-schema/src/persistence.rs`: current restart rules and provider effects with rule `None`.
- `workspaces/runtime/apps/runtime-server/src/handover.rs`: listener bound to `127.0.0.1`.
- `workspaces/client/apps/web/src/generated/api.ts`: branded types for only part of the identifier contracts.
- `workspaces/runtime/packages/runtime-services/src/peer_updates.rs` and `peer_update_pull.rs`: retained update workflow.
- `workspaces/runtime/packages/runtime-store/src/agent_columns.rs`: `context_repair_phase` reads agent JSON.
- `workspaces/client/apps/desktop/`: retained `recover_backend.py`, `deliver_server.py`, `desktop_delivery.py`, and `install_update.py`.

**Acceptance: ownership and the client**

- [ ] **C1. Server ownership.** Closing every UI leaves agents and commands operational. The server owns chats, queues, drafts, uploads, synchronization, and domain decisions.
- [ ] **C2. Shared frontend.** Browser, progressive web app (PWA), and Tauri use the same frontend and command contract. No client keeps a second domain database or durable outbox.
- [ ] **C3. Unsent input.** Reload, tab closure, network loss, and server restart preserve unconfirmed text and attachments. Confirmation removes only the corresponding input.
- [ ] **C4. Thin cache.** A remote client stores one data-and-cursor record per subscription. Writes occur after display, at most every five seconds, and on hide.
- [ ] **C5. Cache bounds.** The cache obeys the agreed 32 MiB budget and least-recently-used eviction. Oversized entries are skipped. Local-server frontends keep no server-data cache.
- [ ] **C6. Cache recovery.** Offline cached data remains readable and visibly stale. Reconnect applies newer data without replacing it with older cached rows.
- [ ] **C7. Small browser surface.** IndexedDB serves only the accepted cache, unsent input, and device-key roles. RxDB, Dexie, and per-frame persistence do not return.
- [ ] **C8. Native shell.** Microphone, dictation, file dialogs, preview, and notifications still work through native capabilities. Permission checks remain at the native boundary.
- [ ] **C9. Shared pure code.** WebAssembly and native clients use the same Rust Markdown parser. Shared hashes, types, and signing rules have one owner.
- [ ] **C10. Frontend-core audit.** Every remaining client-core module has a documented device or transport purpose. Stateful domain logic, including any in `views.rs`, resides on the server.
- [ ] **C11. Existing clients.** PWA installation and Web Push still work. Two laptop servers, a desktop UI, and a phone can operate together.

**Acceptance: durable views and immediate UI updates**

- [ ] **S1. One source.** Every entity reader builds documents from owner rows and durable marks. No current entity payload depends on a copy in `sync_entities`.
- [ ] **S2. All readers.** Streams, scoped subscriptions, REST pulls, and command responses return equivalent views for the same database state.
- [ ] **S3. Cursor safety.** Pagination never splits one sequence unit. Deletes produce tombstones. An expired cursor produces a snapshot without gaps or duplicate application.
- [ ] **S4. Migration safety.** Each moved collection preserves visible data and keeps the sequence counter above every prior value. Repeated migration does not alter the result.
- [ ] **S5. Queue changes.** Queue insertion, reorder, withdrawal, delivery, uncertainty, and deletion update the view. An unchanged revision does not allocate another sequence.
- [ ] **S6. Peer-team changes.** The first real change after migration reaches the client. Member folder changes, membership changes, and project deletion update the correct team.
- [ ] **S7. Project fields.** Vault, Layr readiness, and folder synchronization remain visible for projects that support them. Their source has an explicit owner and change notification.
- [ ] **S8. Copy removal.** Equality checks cover each removed collection. Remaining workspace, error, room, and transcript hash dependencies no longer require `sync_entities`.
- [ ] **S9. Live deltas.** Model deltas reach only relevant subscriptions from memory. Neither the old 750 ms batch nor the durable 200 ms interval delays them.
- [ ] **S10. Slow clients.** A full client buffer merges compatible deltas without corrupting order or content. The final durable item reconciles any missed transient deltas.
- [ ] **S11. Mobile traffic.** Compressed text up to the shared 8 KB threshold stays inline. Larger text uses a hash and size and loads on demand.
- [ ] **S12. History traffic.** Compressed streams send the visible tail and agreed recent-chat tails. Older history loads on scroll. Valid reconnect cursors receive deltas.
- [ ] **S13. Immediate feedback.** Pending UI state uses the command ID. Returned rows settle it by sequence without waiting for another stream frame.
- [ ] **S14. Push only.** Server data reaches the client through server-sent events (SSE). Network health probes do not become data polling.

**Acceptance: domain model and history**

- [ ] **D1. Chat, Turn, Session.** Typed rows own state and settings. One chat has at most one unfinished turn. No independent agent JSON field can contradict them.
- [ ] **D2. Delivery.** Queued, Submitted, Applied, Withdrawn, and Unknown carry their required fields. Generated checks reject invalid combinations. Uncertain legacy rows migrate without invented facts.
- [ ] **D3. Shared draft.** The Chat owns one draft. Two devices observe the accepted last-write behavior. Device input survives until the server confirms its command.
- [ ] **D4. Work.** Typed columns own Work status and searchable state. Existing assignment, review, blocking, acceptance, and cancellation flows retain their behavior.
- [ ] **D5. Execution history.** Turns and the common outbox retain all execution-attempt facts. Equality checks precede removal of each obsolete execution table.
- [ ] **D6. Tool history.** Items retain tool input, full output, status, and identity. Removing read receipts does not remove tool-call history.
- [ ] **D7. Rooms.** Chat relationships determine derived rooms. Explicit owners retain radio configuration, project deletion state, and hidden flags before `runtime_rooms` is removed.
- [ ] **D8. Tasks.** The task-to-item comparison accounts for every unmatched field and row. No task output or status disappears because a sample looked redundant.
- [ ] **D9. Process.** Process states include typed `Completed { exit }`. Completion without a required exit value fails validation.
- [ ] **D10. Immutable records.** Checkpoint and attachment content cannot change after creation. The attachment file remains the content owner.
- [ ] **D11. Conversation retention.** Messages, items, and rewind-hidden items remain readable in history. A typed `hidden_by` relation replaces JSON-only rewind flags.

**Acceptance: commands and recovery**

- [ ] **K1. One command path.** Every domain mutation uses the common identity, permission, decision, and receipt rules across UI, agent, and peer callers.
- [ ] **K2. Command coverage.** Send, stop, pause, withdraw, delete, edit, answer, retry, mark-delivered, and discard have connected callers and failure behavior.
- [ ] **K3. Exact replay.** The same ID and body never repeat a mutation. A different body conflicts. Old commands below the client watermark never execute again.
- [ ] **K3a. Result ownership.** Each result family has a typed owner and revision, or an explicit retained external outcome. Responses read current rows without storing redundant snapshots.
- [ ] **K3b. Result-contract conflict.** Reconcile the design's no-body rule with the migration plan's `Body` exceptions. Do not delete exact outcomes or claim completion with an unresolved conflict.
- [ ] **K4. Pure decisions.** Decision functions read supplied facts and return row changes and effects. They perform no database, filesystem, provider, or network input/output.
- [ ] **K5. Atomic writes.** State changes, create hashes, receipt references, watermarks, and required outbox entries commit together. Injected failures leave none of them partly applied.
- [ ] **K6. Immediate effects.** Normal effects start immediately after commit. A recovery worker handles crash leftovers, without adding a normal-path polling delay.
- [ ] **K7. Edit and delete safety.** Edit restores the correct Layr snapshot and forks the provider thread. Rewind, restore, and delete preserve the required prior snapshot and history.
- [ ] **K8. Stop semantics.** Stop interrupts the selected turn without pausing the chat or clearing its queue. Pause remains separate. A stale turn ID cannot stop a newer turn.
- [ ] **K9. Stop races.** Stop covers preparation, scheduled resume, remote execution, and lost connection. Cleanup from the old turn cannot affect the next queued turn.
- [ ] **K10. Withdrawal.** A queued message returns to the composer. A held message interrupts its turn as specified. Repeated withdrawal creates no second interrupt or draft copy.
- [ ] **K11. Multi-chat actions.** Delete and descendant actions preserve their documented transaction boundary. No partial tree change survives a failed transaction.
- [ ] **R1. Durable send mark.** The send-intent mark commits before external effects. Lost responses preserve uncertainty rather than trigger a fresh action.
- [ ] **R2. Target deduplication.** Every automatic retry uses the same ID and exact body. A recorded target test proves deduplication before an effect receives `TargetDedupes`.
- [ ] **R3. Target probes.** `AskTarget` distinguishes found, proven absent, and uncertain. Only proven absence permits a new send. Codex and peer execution have recorded boundary evidence.
- [ ] **R4. Unknown outcomes.** Interrupt, rollback, fork, and other unproven effects remain Unknown after ambiguity. Retry, mark-delivered, and discard preserve the accepted user choices.
- [ ] **R5. Restart modes.** Clean restart, unclean stop, and no-supervisor operation cannot repeat a provider input or reset-credit charge. Tests cover committed-intent loss under SQLite NORMAL mode.
- [ ] **R6. One recovery owner.** The supervisor operation ID equals the outbox operation ID. Server and supervisor do not make independent resend decisions.
- [ ] **R7. Provider adapter.** Adapters translate protocol and report capabilities. The server owns scheduling and requeue. An unapplied Claude steer returns to the queue once.
- [ ] **R8. Provider evidence.** Raw frames use a separate writer and rotating files. They support diagnosis and replay but never become the source of domain state.
- [ ] **R9. Replay checks.** Recorded commands and provider frames produce expected typed state. Tests include conflicts, lost responses, restart, and out-of-order observations.

**Acceptance: approvals and permissions**

- [ ] **A1. Typed approvals.** Each approval belongs to a turn. Its typed request determines valid answers, close reasons, and the answering principal.
- [ ] **A2. Answer identity.** The same answer replays. A different answer conflicts, including secret answers. Policy answers create an explicit `Answered { by: Policy }` record.
- [ ] **A3. Approval lifecycle.** Turn end, session loss, and chat deletion close pending requests and preserve uncertainty for submitted answers. Hidden approvals retain their actual state.
- [ ] **A4. Durable timeout.** The server owns the Claude timeout through a durable timer. A surviving supervised process retains its pending approvals after restart.
- [ ] **A5. One permission contract.** Each handler receives its principal. Shared checks cover descendant, own-link, approved-room, device-grant, and project-grant boundaries.
- [ ] **A6. Trusted peers.** Existing trusted paired peers retain the broad reads the owner accepted. Write authority remains restricted to the applicable link and command.
- [ ] **A7. Cross-owner peers.** ProjectPeer and Federated roles cannot inherit owner or trusted-peer access from the network path. Negative permission tests fail before mutation.
- [ ] **A8. Device identity.** One device key works across the shared server trust list. Full and Observe grants behave as selected. Revocation cannot return through a later merge.

**Acceptance: networks and communication between leads**

- [ ] **N1. Direct clients.** A remote frontend connects directly to each selected server. Another server is not a required proxy for its API calls.
- [ ] **N2. Route identity.** The client verifies public identity before credentials. A reused address with another server key receives no credentials.
- [ ] **N3. Reconnect.** Ordered routes, learned addresses, cooldowns, and one retry owner work together. A foreground or network-change probe replaces only a failed stream.
- [ ] **N4. Local-network access.** A separately paired local-network route works without Tailscale. An HTTPS-paired credential never travels over plain HTTP. Identity mismatch is refused.
- [ ] **N5. Contract mismatch.** A frontend mismatch refuses the connection with an update message. Peer compatibility follows the current and previous contract policy.
- [ ] **N6. QUIC integrity.** Per-request signatures, contract checks, and replay protection remain. Reconnection resumes queued delivery without an unnecessary backoff delay.
- [ ] **N7. Transport scope.** Fetch and SSE remain supported. WebTransport is optional future work, not a completion requirement.
- [ ] **P1. Project pairing.** Owners can create and accept a one-use invitation in project settings. Expiry, fingerprint confirmation, and project selection are enforced.
- [ ] **P2. Minimum grant.** The initial project grant permits only LeadMessages. Other projects, files, spawn, stop, and work assignment remain inaccessible.
- [ ] **P3. Equal leads.** Each lead sees the peer lead in its team with a server label. The normal agent message tool sends and receives peer messages.
- [ ] **P4. Wake and delivery.** A peer message wakes the receiving lead like a local report. Stable IDs prevent duplicates across retries, disconnects, and server restarts.
- [ ] **P5. Untrusted content.** Received peer text is agent input, not permission or a server command. Subagents do not receive a new cross-owner management relationship.
- [ ] **P6. Any network.** Direct peer communication works outside a shared tailnet. Relay fallback carries encrypted traffic. The owner can disable public fallback relays.
- [ ] **P7. Revocation.** Revoking a project grant blocks further use and removes its queued outbound messages. Pairing again creates a new grant.
- [ ] **P8. Reachable UI.** The installed UI exposes project pairing and peer-lead messages through visible controls. Two servers can complete invitation, send, reply, and revoke.
- [ ] **P9. Federation retained.** Existing approved federation rooms still work after obsolete v1 signature removal. Project grants do not silently replace or delete their data.

**Acceptance: storage, types, and removal of obsolete code**

- [ ] **B1. SQLite ownership.** SQLite remains the server store. Typed rows own current state. Conversation history does not become an event-sourced domain database.
- [ ] **B2. Blob ownership.** Large item and agent text use content-addressed blobs. Blob write and fsync precede row commit. Reference checks and delayed collection preserve live content.
- [ ] **B3. Full tool output.** Truncated item output moves to a verified blob before any receipt copy or fulltext row disappears.
- [ ] **B4. Analytics rebuild.** A database clone provides `dbstat` measurements. The online rebuild preserves rows, serves commands between batches, and resumes after interruption.
- [ ] **B5. Writer behavior.** Commands take priority over bounded background batches. Analytics, terminal output, and raw provider frames use their separate writers.
- [ ] **B6. Transaction boundary.** Write transactions perform no external input/output. A long or stuck transaction produces the existing duration alert.
- [ ] **B7. Online migration.** Large migrations use a durable progress marker and bounded batches. Restart, low disk space, and retry preserve old and new data consistently.
- [ ] **B8. Backup and space.** Write-ahead-log backup identifies its database generation and passes a restore check. Space reclamation removes no source history or protected build cache.
- [ ] **B9. Service retention.** Receipt, tombstone, nonce, and outbox cleanup obey their replay and retention rules. Conversation content is excluded from service cleanup.
- [ ] **T1. One type owner.** Rust owns shared types and constants. TypeScript and SQLite checks derive from that owner. Production parity checks detect missing consumers.
- [ ] **T2. Identifier safety.** Agreed identifiers use distinct Rust newtypes and TypeScript brands. A compile-time witness rejects mixed identifier kinds.
- [ ] **T3. Strict decode.** Stored typed data decodes directly without an intermediate generic Value. Invalid internal variants fail. Old formats migrate instead of silently defaulting.
- [ ] **T4. Tagged contracts.** Internal enum contracts use explicit tags. Named boundary adapters convert external wire formats. No internal union relies on untagged fallback.
- [ ] **T5. Provider evolution.** Unknown provider item kinds retain their original kind and raw content. The UI can display them without accepting invalid internal state.
- [ ] **T6. Transcript contracts.** Rust generates transcript schemas. No hand-written OpenAPI copy can diverge from the actual response.
- [ ] **M1. One mechanism.** One signed-envelope implementation, nonce owner, and shared outbox replace the duplicate paths. Trust lists remain separate where their permissions differ.
- [ ] **M2. Peer updates.** Commands and the shared outbox own update steps. An installation test covers lost responses, restart, and recovery before the obsolete workflow files disappear.
- [ ] **M3. Helper replacement.** TypeScript or Rust replacements pass the retained installation and recovery checks. A live-proven recovery owner exists before Python helpers are deleted.
- [ ] **M4. Client removal.** Electron, obsolete client stores, outbox, sync, and store-move code are absent. Required drafts, uploads, and recordings remain available on the server.

**Known defects and conflicting status reports**

1. `project_projection::local_project_view` supplies Missing for readiness, sync, and vault. `sidebarServices::sidebarVault` still reads `project.vault` and skips absent values.
   The project migration therefore lacks complete feature-preservation evidence. Removed tests do not establish that the fields were unnecessary. Acceptance: S7.
2. The saved peer-team migration does not preserve the old view hash. The unsaved correction addresses the first-change sequence problem. Acceptance: S6.
3. The agent later treated local-network work as an unanswered decision. The owner already selected implementation and accepted separate pairing. Acceptance: N4.
4. The agent listed maintainer agreement as open despite the owner's earlier confirmation. Actual concurrent edits need coordination, not renewed design approval.
5. The design says result bodies disappear. The result-migration inventory retains exact external outcomes as Body. Acceptance requires one reconciled contract, not deletion by assertion.
6. A restriction against new untagged contracts does not remove existing untagged types. Generated brands for four types do not complete every identifier contract.
7. A source-only audit or a previous agent's PASS does not prove provider behavior, installation, or a live migration.

**Acceptance: integration evidence**

- [ ] **V1. Interrupted change.** All four failed tests exercise the new boundaries and pass. Each regression check fails on the actual broken behavior.
- [ ] **V2. Local integration.** The complete change passes required local checks on an identified revision. The report separates focused checks from full gates and skipped tests.
- [ ] **V3. Authorized delivery.** Before a pull request or merge, `just test-fast` passes. Before installation, `just test-comprehensive` passes on the exact package commit.
- [ ] **V3a. Live acceptance.** After authorized installation, real user flows cover both servers, phone reconnect, peer messages, approvals, edit, withdrawal, and restart recovery.
- [ ] **V3b. Honest limits.** Fixtures do not substitute for real model requests or reset-credit evidence. Unavailable platform checks remain explicitly unverified.
- [ ] **V4. Current main.** Integration accounts for concurrent main changes. Old implementations do not return beside newer ones. The final diff preserves unrelated work.
- [ ] **V5. Final evidence.** Each criterion has a result or an explicitly accepted limitation. No open item disappears through a stale issue checkbox or an unsupported completion claim.

The local-code milestone and the live-acceptance milestone are separate. The last local-only goal did not authorize the latter.
This document records both so a later implementation cannot call the entire design complete after local tests alone.
