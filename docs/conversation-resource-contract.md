# Conversation resource ownership

Rust metadata in `runtime-schema/src/observable_resources.rs` selects existing
wire schemas; `studio-contracts --write` owns the generated output. No endpoint
or persisted projection is added.

| Fact                   | Generated API                                                                 | Identity / acquisition                                                                          |
| ---------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Room summaries / radio | `state().current.data.rooms`                                                  | Entity envelope id; `RoomEntityDtoFields`                                                       |
| Complaint summaries    | `state().current.data.complaints`                                             | Envelope id; `ComplaintEntityDto`                                                               |
| Message receipts       | `state().current.data.messageReceipts`                                        | Envelope id; `MessageReceiptEntityDto`                                                          |
| Queues                 | `state().current.data.queues`                                                 | Agent envelope id; `QueueEntityDto`; nested items by id                                         |
| Room history           | `room(roomId(id), roomLegacy(true)?)` / `createRoomMirror(dto)`               | Existing room subscription; `RoomData` is `ChatReadFields` or legacy `readonly MessageRecord[]` |
| Complaint detail       | `complaint(complaintId(id))` / `createComplaintMirror(dto)`                   | Existing complaint subscription; response rows by id                                            |
| Transcript tail / page | `transcript(agentId(id), transcriptTail(n)?)` / `createTranscriptMirror(dto)` | Existing subscription / caller-owned one-shot page; incoming array order, records by id         |
| Transcript item        | `createTranscriptItemMirror(dto)`                                             | Existing `/api/transcript/item` response, `TranscriptItemResponse`; sourceId stays a fact       |
| Full output            | `createTextBlobMirror(dto)`                                                   | Existing blob GET, `TextBlobResponse` hash/size/text; no eager reads                            |
| Historical turn error  | `createTurnErrorMirror(dto)`                                                  | Existing `/api/analytics` turns, `AnalyticsTurnRecord` agent/thread/turn/status/error           |
| Live text              | `createLiveDeltaMirror(dto)`                                                  | Existing `watchLiveDeltas`, `ResourceLiveDeltaEvent` agentId/itemId/offset/text                 |

Access `.data` only for live/stale resource statuses. The generated aliases are
readonly. Declared domain fields are exported from `observableFieldTypes.ts`.
For example, a room history session observes `store.room(roomId(room)).current`,
then passes the resulting model references to rows. It releases its MobX observer
when selection changes. App/Workspace resolve selection; LiveConversation resolves
canonical agent/lead models, retaining the acknowledged-create overlay only until
the summary arrives. The existing ConversationProvider owns navigation and
draft/send/read/edit/refresh commands; PromptComposer alone subscribes to draft
input. B owns history-session disposal.

`App -> LiveConversation -> useConversationHistory -> TurnHistory -> StreamingText`
is the agent caller path. The history session retains bounded page/model references;
the generated store owns the live tail. `useRoomHistory` retains generated room
message references. Paging, loading/errors, anchors and exact command records stay
in TypeScript. Scope cleanup stops generated interest, connection/live listeners
and reactions; history version changes do not change React keys. The outside-W3
`useMessages` immutable adapter uses those same owners. Live text is bounded by
the history runtime and reconciles on reset or durable replacement.

The store and subscription module keep their established
inactive grace/eviction lifetimes; no additional acquisition owner is created.

Direct App/Workspace W3 mounts run inside their existing server services or
server-view provider. Each uses that owner's store, subscription and API transport.
The outer multi-server sidebar only mirrors its existing state facts; W3 history
is rendered inside the selected server view, so no transcript forwarding channel
is needed. A generated mirror has no runtime/accessor, network interest or teardown;
the receiver discards it with its scope. Equal native ids on different owners
must use distinct mirrors and services. Serialized state contains the selected
facts and reconstructs them through `createStateMirror`.

Transcript input ids are optional on the existing wire. Supplied ids retain
identity through reorder; missing/null ids retain positional identity within
that record, matching legacy fallback. Changing an input's identity replaces that
input, while nested content changes update it in place. Duplicate supplied ids
are rejected. Records retain incoming order, without introducing sorting policy.

Live delta offsets count Unicode scalar values, not UTF-16 units or bytes. The
existing delta transport has no sequence, epoch or replay guarantee; its own
source describes loss as harmless because durable transcript items replace live
text. A delta mirror preserves each incoming value exactly; it does not invent
sequence facts. Bounded ephemeral runs belong to the history session and are
scoped by server owner, agent and item. Connection resets, gaps, durable replacement
and disposal must clear or reconcile those runs in that owner. Subscription
revision/cursor/reset metadata continues through the existing resource transport.

Radio controls retain existing booleans but omit `busyNotice` from the generated
domain projection. Existing Rust parity is `busy = active || status not in
[idle, blocked]`, `stopping = status == stopping`, `inputBlocked = stopping ||
(blocked && active)`, `canPass = busy && status != blocked`, and `speaking =
status == speaking`. The old notice derives solely from busy/status: blocked,
stopping, or the remaining busy states. Its wording remains a client concern.
Names already present on native records remain native facts; display fallbacks,
search, sorting, group labels, composer affordances and drafts remain client-owned.
Queues enter only through `state().queues`; after a mutation use the existing scoped refresh/reconnect, because a QueueView GET has no entity sequence and cannot safely overwrite SSE.

Paging/navigation, pending command identity, confirmed overlays and retry UI retain
their existing local owners. One-shot mirrors do not add caches or read recovery.
