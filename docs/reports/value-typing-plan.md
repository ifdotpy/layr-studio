# Rust backend JSON value and typing plan

Scope: local workspace at Git commit `2357e24c61315cb4f418dac7646822fe1b975858` (the facade is present). The vendored `layr` and `layr-desk` workspaces are excluded, as are client crates. This is an inventory and proposed work only; implementation waits for PR8 to merge on GitHub main. Rebase and regenerate line references before each PR.

## Evidence and limits

The appendix enumerates **all lines containing the lexical tokens** `Value`, `RawValue` or `OwnedRawJson` in Rust files under every non-vendored runtime workspace member, including tests. It is an intentionally overinclusive review index: `Field::Value`, other unrelated types, comments and strings are included; aliases without either token at the use site are not individually counted. Line counts count matching lines, not occurrences or runtime allocations. The file-level tags are triage hints, not a claim that every line has the same semantics. `?` means individual review is still required. Thus this index does **not** establish a fully classified semantic inventory of every JSON value use; resolve each candidate in its owner before editing. Tests can legitimately use generic values as wire oracles. Do not remove values merely to reduce the count.

Tags may overlap: **a** creation/realtime/messaging/provider/store hot-path candidate; **b** dynamic-boundary candidate (preserve unknown fields/payloads or raw JSON); **c** candidate for existing typed record/DTO; **d** candidate needing a small owned envelope/struct. A file tag does not classify individual values; especially `b` and `c` can coexist in a single record. Confirm runtime reachability and schema before replacement. For example `runtime-providers/src/codec.rs` owns an open provider envelope and should keep arbitrary params, errors and extension fields raw/dynamic; a stable envelope can be typed without closing its payload. JSON patch and user extensions likewise remain dynamic. `runtime-store/src/records.rs` owns `AgentRecord`, `WorkItemRecord`, and associated round-trip/unknown-field handling. Existing `Field` missing/null/value semantics are part of the contract.

Both `/tmp/studio-profile/base-cg.data` and `load-cg.data` exist. Saved `/tmp/studio-profile/load-children.txt` reports 20K task-clock samples and zero lost samples; its inclusive call-graph output is not per-operation evidence. `base-report.err` records missing/corrupt addr2line symbols, and much of the saved output is unresolved addresses. Attempts at self-only reports were stopped after symbol resolution repeatedly emitted errors and produced no usable report. Accordingly the priorities below use static frequency and direct round trips, not asserted percentages or measured speedups. No load test was run. Compare a future controlled profile before claiming benefit.

## Duplicated decoding and owners

- Agent creation: `managed_child_create.rs:1529` clones actor/root values into `ReviewPlan::from_records` (`native_review.rs:114`), which calls `native_actions.rs:1557` `checked_agent`; that validates and converts cloned values back to `AgentRecord` at line 1570. Make the store's `AgentRecord` the sole record representation and retain one validation at the trust boundary; preserve `bounded`, persisted-schema validation, ID-specific errors and unknown fields. Similar `parent_value.get(...)` lookups at `managed_child_create.rs:42,526,1512` should consume that same owner where semantics match; do not invent another agent DTO.
- Store records: `records.rs:1332,2070,2080,2131` has typed wrappers and `from_value` paths; callers such as `native_actions.rs:522,681,782` convert values back into records. The store record type and repository are the single owners; decode each persisted row once when compatible, while preserving forward-compatible fields and validation. Some generic JSON patch paths require a value and remain so.
- Realtime: `runtime-http/src/routes/sync_stream.rs:840,1216` parses values for stream handling; sync policy/projection lives in `runtime-services/src/sync_policy.rs` and `runtime-schema/src/sync_protocol.rs`. The schema module should own stable wire shapes, and the existing projection owner should populate them once. Keep truly open entity payloads raw. Verify the actual branches before sharing a decoder: similar syntax alone is not evidence of identical contracts.
- Messaging and notifications: `runtime-services/src/work_write.rs:568,608,1165` and `notification_dispatch.rs:1615,1633,1952` parse stored/embedded JSON. Consolidate each repeated persisted shape into its existing store record or its existing schema owner, not a universal message DTO; embedded text and provider-specific content can remain dynamic. Audit call sites and error behavior first.
- Guest frames: `linux_vm_guest.rs:128-135,427-455` exposes `GuestRpc::stream` values, parses a frame, clones it to callback and extracts result. Its guest RPC protocol owner should define one small envelope, retaining dynamic method payloads and legacy error forms. Do not equate guest and provider RPC just because both contain `id` and `result`.
- Provider protocol: `runtime-providers/src/codec.rs:233-272,316` and `supervisor_protocol.rs:205,308,448` contain dynamic fields and record parsing. The existing respective codec/protocol is the owner of each distinct wire shape; share only proven identical decoding, preserving unknown provider fields and provider dispatch.
- `project_deletion.rs` membership parsing is being fixed separately. Recheck that change after merge; do not duplicate it here.

## Prioritized independent batches

Each row is a separate reviewable PR. Each should add focused caller-level equivalence tests and run the repository's required `just test-fast` before a PR. CPU expectations are hypotheses, not measurements. Keep ordering as a priority, not an implicit dependency; rebase each onto the then-current main.

| Rank | Owner and bounded change | Expected CPU | Risk | Equivalence evidence |
| --- | --- | --- | --- | --- |
| 1 | `runtime-services` (+ existing `runtime-store` record API only if needed): pass checked actor/root records into native review creation; eliminate the `ReviewPlan` value round trip without bypassing checks. | Moderate per creation; removes cloning/redecode, frequency workload-dependent. | High: validation, request identity, authorization and unknown-field preservation. | Existing `wave7_p11_native` and creation caller tests plus typed-versus-old fixtures for missing/null/unknown fields, invalid stored records, root mismatch, replay/conflicting IDs and identical persisted child/event output. |
| 2 | `runtime-http`, `runtime-services`, `runtime-schema`: one narrow sync-stream projection or decoder at `sync_stream.rs:840/1216`, selected after tracing its real callers; use existing protocol owner. | Potentially high on frequent SSE frames; unmeasured. | High: selection, cursor/order and compatibility; dynamic entity content. | Compare old/new SSE event bytes when byte contract applies and parsed events otherwise, including initial snapshot, delta, empty, oversized, malformed, unknown fields and reconnect ordering; run sync stream caller tests. |
| 3 | `runtime-services`, `runtime-store`: narrow managed child creation record reads and spawn effects around `managed_child_create.rs:1147-1225`, reuse `AgentRecord`/existing run record. | Moderate per spawn; removes repeated materialization. | High: transactional retry/idempotency and malformed historical records. | Actual creation/replay tests comparing stored rows, emitted events, previous receipt and failures for malformed/missing/null fields and different request content. |
| 4 | `runtime-services` + `runtime-store`: one messaging/work-write record family around `work_write.rs:568,608,1165`; keep its existing record owner and preserve extension data. | Potentially moderate for frequent messages; unmeasured. | High: atomicity, retries, historical shape. | Caller-level send, duplicate/replay and failure fixtures; compare stored JSON semantics and emitted messages including unknown fields, missing/null and invalid records. |
| 5 | `runtime-services`: notification dispatch's repeated agent read/notification decoding (`notification_dispatch.rs:1615,1633`); use `AgentRecord` and the existing notification shape owner. | Moderate on provider event traffic, unmeasured. | High: ordering, provider differences and unknown event payloads. | Dispatch fixtures for duplicate/late/malformed events and multiple providers; compare resulting rows, events and error outcomes semantically. |
| 6 | `runtime-services`: guest RPC frame envelope in `linux_vm_guest.rs`; preserve arbitrary payload as raw/value and callback order. | Moderate per frame; avoids full-tree clone if caller contract permits. | High: transport uncertainty, identity, error precedence, framing and callbacks. | Real Unix socket caller tests for byte-identical requests and semantic frames/results, wrong IDs, error variants, invalid JSON, timeout/EOF, callback failure and uncertain flags. |
| 7 | `runtime-store`: one record family at a time using existing record/repository owner; remove repeated `Value` conversions where direct decoding preserves validation and extensions. | Moderate for repeated store reads/writes, workload-dependent. | High: backward compatibility, null/missing and round-trip fields. | Corpus of persisted records old/new decode, semantic round-trip unknown fields, invalid input and byte equivalence where storage bytes matter; repository read/write and rollback tests. |
| 8 | `runtime-providers`: narrow stable codec envelope in `codec.rs`; retain opaque provider params/result/extensions and existing supervisor protocol ownership. | Low to moderate per provider frame, unmeasured. | High: protocol variation, arbitrary IDs and unknown fields. | Provider fixture corpus and actual codec callers: byte/semantic comparison, unknown fields, malformed envelopes, IDs, errors, multiple providers. |
| 9 | `runtime-services`: one separately scoped lower-frequency DTO family identified in appendix, such as work-history or multi-server orchestration. Reuse its authoritative store/schema type; introduce a struct only when no owner exists. | Low/unknown. | Medium to high: stored history and federation compatibility. | Old/new fixture corpus plus real caller tests for legacy, malformed and unknown fields, replay and output equivalence. Split further rather than making a general cleanup PR. |

No batch assumes a closed schema for a dynamic extension or provider payload. A change that requires additional state or recovery machinery needs the owner's decision under the repository simplicity rule.

## Lexical inventory by crate and file

`lines` lists every matching line; `n` is the number of matching lines. Test modules and fixtures remain included. File-level tags are preliminary as defined above, and `?` is explicitly unclassified, not an instruction to replace.

### studio-diagnostics

1 files, 3 matching lines.

- `workspaces/runtime/apps/diagnostics/src/main.rs` (?; n=3): 4, 75, 79

### studio-json

5 files, 47 matching lines.

- `workspaces/runtime/packages/json/examples/measure.rs` (b; n=6): 3, 15, 27, 33, 50, 56
- `workspaces/runtime/packages/json/src/lib.rs` (b; n=8): 18, 23, 27, 169, 173, 234, 238, 243
- `workspaces/runtime/packages/json/src/raw.rs` (b; n=9): 8, 13, 22, 23, 26, 29, 35, 49, 59
- `workspaces/runtime/packages/json/src/raw_sonic.rs` (b; n=8): 12, 20, 21, 24, 34, 40, 52, 62
- `workspaces/runtime/packages/json/tests/parity.rs` (b; n=16): 3, 13, 33, 117, 118, 124, 132, 133, 173, 174, 182, 201, 212, 214, 221, 227

### studio-peer-link

1 files, 1 matching lines.

- `workspaces/runtime/apps/peer-link/src/outbound.rs` (?; n=1): 53

### studio-process-supervisor

1 files, 2 matching lines.

- `workspaces/runtime/apps/process-supervisor/tests/clients.rs` (?; n=2): 15, 18

### studio-runtime-http

94 files, 1115 matching lines.

- `workspaces/runtime/packages/runtime-http/build.rs` (?; n=2): 119, 685
- `workspaces/runtime/packages/runtime-http/examples/draft-server.rs` (?; n=7): 14, 15, 30, 35, 56, 57, 115
- `workspaces/runtime/packages/runtime-http/src/bounded_entities.rs` (?; n=26): 7, 46, 134, 320, 358, 368, 384, 530, 556, 566, 620, 732, 748, 776, 800, 1174, 1187, 1192, 1356, 1357, 1370, 1371, 1535, 1846, 1869, 2552
- `workspaces/runtime/packages/runtime-http/src/error.rs` (?; n=6): 20, 70, 74, 346, 358, 366
- `workspaces/runtime/packages/runtime-http/src/http_body_limits.rs` (?; n=1): 40
- `workspaces/runtime/packages/runtime-http/src/ingress_parity_tests.rs` (?; n=3): 7, 15, 23
- `workspaces/runtime/packages/runtime-http/src/json_body.rs` (b; n=2): 1, 6
- `workspaces/runtime/packages/runtime-http/src/middleware.rs` (?; n=6): 256, 725, 730, 734, 1141, 1239
- `workspaces/runtime/packages/runtime-http/src/model_entities.rs` (?; n=4): 4, 153, 163, 286
- `workspaces/runtime/packages/runtime-http/src/mutation_tasks.rs` (?; n=1): 315
- `workspaces/runtime/packages/runtime-http/src/router.rs` (?; n=22): 10, 122, 372, 807, 888, 903, 904, 917, 919, 920, 1011, 1276, 1711, 1724, 1845, 1863, 1893, 2097, 2120, 2142, 2212, 2215
- `workspaces/runtime/packages/runtime-http/src/routes/account_login.rs` (?; n=5): 556, 566, 670, 705, 741
- `workspaces/runtime/packages/runtime-http/src/routes/account_transfer.rs` (?; n=8): 9, 130, 131, 139, 140, 267, 433, 448
- `workspaces/runtime/packages/runtime-http/src/routes/agent_management.rs` (ac; n=22): 9, 122, 185, 187, 207, 279, 426, 484, 820, 824, 943, 1010, 1011, 1035, 1044, 1063, 1067, 1072, 1257, 1349, 1477, 1565
- `workspaces/runtime/packages/runtime-http/src/routes/agents_create.rs` (ac; n=27): 7, 28, 46, 47, 305, 313, 558, 776, 1162, 1166, 1169, 1172, 1175, 1501, 1502, 1991, 2029, 2031, 2148, 2174, 2185, 2205, 2249, 2264, 2300, 2330, 2471
- `workspaces/runtime/packages/runtime-http/src/routes/branches.rs` (?; n=48): 16, 50, 56, 62, 64, 70, 72, 77, 78, 96, 99, 100, 127, 139, 214, 216, 217, 292, 305, 307, 330, 334, 398, 400, 438, 540, 618, 619, 644, 652, 712, 713, 813, 820, 1000, 1008, 1030, 1040, 1173, 1239, 1558, 1752, 2061, 2076, 2139, 2168, 2256, 2278
- `workspaces/runtime/packages/runtime-http/src/routes/browser.rs` (?; n=8): 15, 38, 88, 142, 213, 295, 348, 396
- `workspaces/runtime/packages/runtime-http/src/routes/catalog.rs` (?; n=5): 191, 195, 220, 290, 349
- `workspaces/runtime/packages/runtime-http/src/routes/catalog_agent.rs` (ac; n=18): 13, 115, 186, 232, 274, 309, 354, 435, 466, 494, 561, 588, 593, 684, 687, 847, 904, 995
- `workspaces/runtime/packages/runtime-http/src/routes/checkpoints.rs` (?; n=33): 10, 91, 107, 142, 162, 187, 207, 209, 262, 263, 297, 397, 425, 570, 582, 586, 590, 591, 595, 603, 625, 651, 673, 709, 735, 740, 773, 777, 785, 795, 796, 804, 818
- `workspaces/runtime/packages/runtime-http/src/routes/claude_controls.rs` (?; n=6): 9, 99, 113, 225, 457, 469
- `workspaces/runtime/packages/runtime-http/src/routes/claude_login.rs` (?; n=8): 7, 69, 80, 95, 110, 123, 137, 182
- `workspaces/runtime/packages/runtime-http/src/routes/commands.rs` (?; n=13): 15, 122, 123, 299, 324, 352, 519, 903, 907, 1029, 1079, 1289, 1333
- `workspaces/runtime/packages/runtime-http/src/routes/compose.rs` (?; n=2): 117, 121
- `workspaces/runtime/packages/runtime-http/src/routes/desktop.rs` (?; n=10): 5, 22, 26, 134, 185, 317, 528, 594, 596, 617
- `workspaces/runtime/packages/runtime-http/src/routes/entity_transfer.rs` (?; n=6): 7, 209, 871, 886, 954, 959
- `workspaces/runtime/packages/runtime-http/src/routes/extractors.rs` (?; n=21): 6, 31, 32, 34, 35, 36, 75, 155, 169, 170, 177, 178, 200, 205, 209, 243, 260, 266, 328, 332, 336
- `workspaces/runtime/packages/runtime-http/src/routes/federation.rs` (?; n=8): 9, 45, 57, 64, 71, 79, 215, 401
- `workspaces/runtime/packages/runtime-http/src/routes/host_secrets.rs` (?; n=47): 12, 67, 75, 80, 81, 86, 329, 330, 351, 362, 376, 389, 412, 421, 425, 435, 480, 530, 570, 600, 611, 629, 630, 649, 664, 1015, 1021, 1022, 1107, 1170, 1171, 1370, 1673, 1680, 1683, 1813, 1817, 1858, 1978, 2025, 2033, 2084, 2143, 2322, 2348, 2383, 2409
- `workspaces/runtime/packages/runtime-http/src/routes/incoming_multi_server.rs` (?; n=2): 353, 809
- `workspaces/runtime/packages/runtime-http/src/routes/insights.rs` (?; n=5): 10, 37, 290, 315, 368
- `workspaces/runtime/packages/runtime-http/src/routes/linux_vm_settings.rs` (?; n=12): 4, 29, 63, 83, 91, 129, 207, 237, 255, 259, 286, 314
- `workspaces/runtime/packages/runtime-http/src/routes/mac_sync.rs` (ac; n=15): 5, 79, 104, 124, 167, 321, 338, 341, 342, 356, 440, 575, 632, 675, 722
- `workspaces/runtime/packages/runtime-http/src/routes/mcp_reconnect.rs` (?; n=2): 4, 40
- `workspaces/runtime/packages/runtime-http/src/routes/messages.rs` (ad; n=1): 446
- `workspaces/runtime/packages/runtime-http/src/routes/mod.rs` (?; n=2): 74, 150
- `workspaces/runtime/packages/runtime-http/src/routes/models.rs` (?; n=17): 6, 93, 115, 128, 204, 208, 239, 278, 392, 397, 418, 449, 485, 757, 890, 919, 1093
- `workspaces/runtime/packages/runtime-http/src/routes/monitor_runtime.rs` (?; n=4): 14, 101, 195, 207
- `workspaces/runtime/packages/runtime-http/src/routes/multi_server.rs` (?; n=23): 11, 402, 423, 557, 565, 567, 610, 909, 932, 941, 948, 955, 1118, 1174, 1179, 1190, 1253, 1269, 1308, 1396, 1404, 1443, 1496
- `workspaces/runtime/packages/runtime-http/src/routes/panels.rs` (?; n=3): 5, 143, 144
- `workspaces/runtime/packages/runtime-http/src/routes/process_controls.rs` (?; n=7): 9, 56, 62, 76, 77, 468, 582
- `workspaces/runtime/packages/runtime-http/src/routes/projects.rs` (?; n=25): 11, 456, 610, 614, 670, 689, 694, 695, 834, 835, 836, 937, 938, 939, 944, 945, 957, 1029, 1045, 1306, 1441, 1855, 1856, 1863, 1864
- `workspaces/runtime/packages/runtime-http/src/routes/questions_reads.rs` (?; n=4): 10, 41, 46, 78
- `workspaces/runtime/packages/runtime-http/src/routes/questions_write.rs` (?; n=6): 10, 45, 478, 500, 646, 647
- `workspaces/runtime/packages/runtime-http/src/routes/queue_write.rs` (?; n=8): 59, 63, 67, 124, 182, 184, 214, 225
- `workspaces/runtime/packages/runtime-http/src/routes/recovery.rs` (?; n=2): 10, 21
- `workspaces/runtime/packages/runtime-http/src/routes/retained_worktrees.rs` (?; n=3): 3, 29, 44
- `workspaces/runtime/packages/runtime-http/src/routes/rules.rs` (?; n=10): 14, 218, 314, 420, 421, 426, 427, 505, 507, 538
- `workspaces/runtime/packages/runtime-http/src/routes/subscriptions.rs` (?; n=9): 10, 17, 42, 43, 196, 1284, 1313, 1346, 1368
- `workspaces/runtime/packages/runtime-http/src/routes/sync.rs` (ac; n=1): 71
- `workspaces/runtime/packages/runtime-http/src/routes/sync_documents.rs` (ac; n=37): 12, 137, 138, 584, 633, 654, 684, 696, 702, 703, 721, 909, 912, 933, 945, 983, 984, 985, 986, 987, 1020, 1038, 1100, 1133, 1155, 1159, 1176, 1189, 1200, 1206, 1228, 1239, 1355, 1515, 1735, 1758, 1908
- `workspaces/runtime/packages/runtime-http/src/routes/sync_stream.rs` (acd; n=57): 25, 517, 518, 832, 840, 888, 1136, 1137, 1216, 1218, 1258, 1496, 2271, 2288, 2291, 2686, 2700, 2807, 2861, 2972, 3009, 3010, 3054, 3057, 3060, 3065, 3069, 3075, 3077, 3437, 3716, 3724, 4015, 4095, 4376, 4425, 4438, 4946, 4992, 5032, 5051, 5606, 6196, 6198, 6206, 6412, 6457, 6642, 6646, 6725, 6889, 6927, 6952, 7032, 7234, 7401, 7441
- `workspaces/runtime/packages/runtime-http/src/routes/system_reads.rs` (?; n=16): 6, 152, 361, 396, 555, 556, 558, 561, 566, 570, 580, 585, 632, 853, 989, 1066
- `workspaces/runtime/packages/runtime-http/src/routes/terminals.rs` (?; n=8): 12, 340, 358, 361, 362, 403, 426, 502
- `workspaces/runtime/packages/runtime-http/src/routes/tool_requests.rs` (?; n=12): 6, 70, 71, 82, 86, 87, 91, 92, 93, 94, 95, 163
- `workspaces/runtime/packages/runtime-http/src/routes/transcript.rs` (?; n=1): 378
- `workspaces/runtime/packages/runtime-http/src/routes/usage.rs` (?; n=11): 9, 118, 158, 378, 388, 404, 462, 528, 572, 622, 631
- `workspaces/runtime/packages/runtime-http/src/routes/usage_reconciliation.rs` (?; n=4): 81, 143, 209, 280
- `workspaces/runtime/packages/runtime-http/src/routes/vm_native_provider.rs` (ab; n=24): 8, 133, 137, 154, 156, 178, 185, 218, 260, 265, 266, 280, 295, 355, 358, 361, 427, 437, 441, 446, 673, 682, 685, 686
- `workspaces/runtime/packages/runtime-http/src/routes/web_push.rs` (?; n=5): 116, 123, 150, 151, 163
- `workspaces/runtime/packages/runtime-http/src/routes/work_items.rs` (?; n=10): 9, 51, 58, 69, 95, 102, 239, 292, 296, 325
- `workspaces/runtime/packages/runtime-http/src/routes/work_write.rs` (ac; n=23): 10, 73, 97, 149, 242, 268, 270, 513, 725, 776, 835, 895, 936, 970, 1014, 1016, 1133, 1179, 1247, 1313, 1415, 1461, 1633
- `workspaces/runtime/packages/runtime-http/src/routes/workspace_files.rs` (?; n=3): 309, 351, 451
- `workspaces/runtime/packages/runtime-http/src/routes/workspace_views.rs` (?; n=21): 4, 173, 185, 188, 225, 427, 450, 476, 585, 616, 628, 731, 742, 748, 749, 751, 757, 771, 812, 831, 869
- `workspaces/runtime/packages/runtime-http/src/schema.rs` (b; n=4): 6, 54, 58, 81
- `workspaces/runtime/packages/runtime-http/src/validation.rs` (?; n=126): 12, 17, 22, 32, 34, 41, 42, 230, 310, 339, 375, 384, 385, 395, 401, 402, 414, 420, 445, 453, 454, 462, 471, 472, 476, 525, 531, 541, 555, 561, 568, 573, 580, 582, 585, 593, 603, 605, 624, 632, 639, 640, 654, 656, 682, 683, 693, 694, 728, 743, 744, 765, 779, 780, 781, 787, 932, 933, 937, 946, 951, 957, 960, 978, 990, 993, 1008, 1009, 1012, 1032, 1039, 1084, 1098, 1124, 1145, 1147, 1148, 1150, 1151, 1168, 1172, 1189, 1194, 1196, 1214, 1224, 1239, 1240, 1246, 1257, 1286, 1288, 1289, 1297, 1339, 1340, 1341, 1344, 1356, 1357, 1364, 1373, 1388, 1389, 1390, 1392, 1401, 1411, 1415, 1419, 1423, 1435, 1439, 1449, 1452, 1454, 1476, 1500, 1548, 1550, 1558, 1688, 1707, 1730, 2123, 2200
- `workspaces/runtime/packages/runtime-http/tests/checkpoint_rewind.rs` (?; n=10): 17, 83, 109, 195, 298, 308, 319, 329, 335, 365
- `workspaces/runtime/packages/runtime-http/tests/child_permissions.rs` (?; n=2): 11, 66
- `workspaces/runtime/packages/runtime-http/tests/claude_file_backups.rs` (?; n=7): 12, 32, 33, 37, 147, 255, 289
- `workspaces/runtime/packages/runtime-http/tests/claude_login.rs` (?; n=2): 12, 62
- `workspaces/runtime/packages/runtime-http/tests/claude_repair_report.rs` (?; n=2): 8, 11
- `workspaces/runtime/packages/runtime-http/tests/draft_replication.rs` (?; n=5): 9, 14, 60, 70, 78
- `workspaces/runtime/packages/runtime-http/tests/future_worker_account.rs` (?; n=6): 13, 212, 214, 220, 249, 306
- `workspaces/runtime/packages/runtime-http/tests/future_worker_catalog_race.rs` (?; n=5): 13, 220, 222, 228, 257
- `workspaces/runtime/packages/runtime-http/tests/mcp_reconnect.rs` (?; n=6): 10, 18, 164, 168, 174, 177
- `workspaces/runtime/packages/runtime-http/tests/multi_server_incoming.rs` (?; n=28): 9, 25, 99, 106, 133, 137, 168, 283, 297, 379, 402, 460, 499, 552, 671, 732, 791, 814, 828, 860, 869, 908, 962, 1143, 1164, 1296, 1346, 1373
- `workspaces/runtime/packages/runtime-http/tests/orchestration_settings.rs` (?; n=3): 9, 21, 43
- `workspaces/runtime/packages/runtime-http/tests/process_controls.rs` (?; n=28): 11, 27, 188, 447, 460, 545, 577, 598, 601, 708, 770, 812, 830, 895, 931, 947, 976, 1002, 1021, 1030, 1058, 1075, 1129, 1151, 1169, 1177, 1178, 1211
- `workspaces/runtime/packages/runtime-http/tests/project_deletion.rs` (?; n=12): 14, 61, 65, 120, 123, 157, 164, 401, 688, 707, 734, 862
- `workspaces/runtime/packages/runtime-http/tests/project_preparation.rs` (?; n=3): 8, 17, 148
- `workspaces/runtime/packages/runtime-http/tests/queue_recovery.rs` (?; n=4): 9, 171, 199, 436
- `workspaces/runtime/packages/runtime-http/tests/recovery_http.rs` (?; n=7): 13, 27, 43, 58, 69, 120, 150
- `workspaces/runtime/packages/runtime-http/tests/remote_access_origin.rs` (?; n=3): 11, 45, 51
- `workspaces/runtime/packages/runtime-http/tests/retained_worktrees.rs` (?; n=5): 12, 22, 24, 118, 146
- `workspaces/runtime/packages/runtime-http/tests/rules_owner_hold.rs` (?; n=4): 10, 17, 39, 94
- `workspaces/runtime/packages/runtime-http/tests/studio_errors.rs` (?; n=22): 11, 36, 45, 73, 81, 83, 108, 123, 126, 158, 167, 225, 233, 254, 488, 631, 752, 888, 1002, 1020, 1384, 1392
- `workspaces/runtime/packages/runtime-http/tests/task_output.rs` (?; n=3): 9, 16, 149
- `workspaces/runtime/packages/runtime-http/tests/task_review.rs` (?; n=8): 10, 15, 41, 178, 186, 261, 273, 295
- `workspaces/runtime/packages/runtime-http/tests/ui_limits_org.rs` (?; n=4): 6, 81, 111, 123
- `workspaces/runtime/packages/runtime-http/tests/voice_provider.rs` (ab; n=7): 8, 24, 54, 169, 438, 526, 554
- `workspaces/runtime/packages/runtime-http/tests/wave7_p04_callback_http.rs` (?; n=8): 11, 53, 291, 494, 496, 708, 715, 721
- `workspaces/runtime/packages/runtime-http/tests/wave7_p10_lifecycle_http.rs` (?; n=5): 12, 55, 87, 221, 339
- `workspaces/runtime/packages/runtime-http/tests/wave7_p15_http.rs` (?; n=15): 10, 16, 42, 55, 71, 74, 77, 80, 89, 91, 189, 230, 231, 380, 411
- `workspaces/runtime/packages/runtime-http/tests/wave7_p18_account_session.rs` (?; n=3): 13, 52, 100

### studio-runtime-process

15 files, 122 matching lines.

- `workspaces/runtime/packages/runtime-process/src/protocol.rs` (?; n=28): 7, 143, 148, 155, 162, 173, 182, 184, 192, 199, 206, 218, 226, 234, 249, 292, 315, 318, 322, 328, 340, 349, 355, 398, 433, 443, 455, 461
- `workspaces/runtime/packages/runtime-process/src/provider_seam.rs` (ab; n=7): 5, 56, 57, 61, 69, 73, 182
- `workspaces/runtime/packages/runtime-process/src/provider_shared.rs` (ab; n=5): 15, 284, 287, 289, 387
- `workspaces/runtime/packages/runtime-process/src/provider_stdio.rs` (ab; n=4): 48, 1323, 1350, 1359
- `workspaces/runtime/packages/runtime-process/src/provider_task.rs` (ab; n=6): 8, 306, 486, 1012, 1169, 1174
- `workspaces/runtime/packages/runtime-process/src/provider_task_tests.rs` (ab; n=11): 149, 316, 535, 550, 691, 843, 885, 1002, 1010, 1013, 1032
- `workspaces/runtime/packages/runtime-process/src/supervisor_child.rs` (?; n=5): 18, 277, 304, 306, 318
- `workspaces/runtime/packages/runtime-process/src/supervisor_daemon.rs` (?; n=9): 28, 154, 481, 488, 513, 554, 715, 836, 1197
- `workspaces/runtime/packages/runtime-process/src/tests.rs` (?; n=7): 8, 73, 77, 85, 86, 87, 101
- `workspaces/runtime/packages/runtime-process/tests/provider_task.rs` (ab; n=16): 5, 25, 43, 46, 49, 59, 66, 69, 81, 93, 98, 143, 158, 172, 178, 668
- `workspaces/runtime/packages/runtime-process/tests/supervisor/deadlines.rs` (?; n=8): 201, 235, 321, 348, 433, 621, 627, 651
- `workspaces/runtime/packages/runtime-process/tests/supervisor/legacy_start.rs` (?; n=3): 62, 75, 79
- `workspaces/runtime/packages/runtime-process/tests/supervisor_receipt_loss.rs` (?; n=3): 10, 41, 54
- `workspaces/runtime/packages/runtime-process/tests/supervisor_v2.rs` (?; n=7): 11, 496, 501, 592, 823, 937, 942
- `workspaces/runtime/packages/runtime-process/tests/wave7_p01_factory.rs` (?; n=3): 4, 43, 48

### studio-runtime-providers

6 files, 69 matching lines.

- `workspaces/runtime/packages/runtime-providers/src/codec.rs` (abd; n=27): 3, 233, 234, 243, 244, 251, 252, 260, 272, 316, 317, 348, 350, 352, 354, 357, 359, 360, 373, 401, 402, 433, 535, 559, 621, 632, 639
- `workspaces/runtime/packages/runtime-providers/src/connection.rs` (ab; n=15): 4, 94, 128, 145, 166, 268, 316, 339, 361, 382, 428, 752, 814, 818, 829
- `workspaces/runtime/packages/runtime-providers/src/initialize.rs` (ab; n=2): 3, 35
- `workspaces/runtime/packages/runtime-providers/src/supervisor_protocol.rs` (ab; n=11): 4, 205, 233, 308, 383, 434, 442, 448, 488, 498, 510
- `workspaces/runtime/packages/runtime-providers/tests/codec.rs` (abd; n=8): 1, 8, 99, 214, 300, 302, 367, 392
- `workspaces/runtime/packages/runtime-providers/tests/connection.rs` (ab; n=6): 1, 40, 748, 775, 795, 846

### studio-runtime-schema

89 files, 830 matching lines.

- `workspaces/runtime/packages/runtime-schema/src/account_registry.rs` (b; n=2): 311, 322
- `workspaces/runtime/packages/runtime-schema/src/agent_entity_native.rs` (abc; n=7): 100, 140, 162, 164, 168, 173, 222
- `workspaces/runtime/packages/runtime-schema/src/agent_entity_nested.rs` (abc; n=22): 230, 231, 238, 240, 243, 248, 255, 265, 274, 285, 291, 292, 301, 304, 310, 326, 338, 343, 447, 575, 609, 619
- `workspaces/runtime/packages/runtime-schema/src/agent_entity_nested_extra.rs` (abc; n=15): 55, 73, 209, 248, 254, 262, 266, 320, 326, 332, 338, 340, 341, 439, 489
- `workspaces/runtime/packages/runtime-schema/src/agent_entity_remote.rs` (abc; n=10): 37, 38, 44, 50, 52, 55, 60, 65, 76, 130
- `workspaces/runtime/packages/runtime-schema/src/agent_entity_transfer.rs` (abc; n=12): 89, 98, 104, 112, 116, 121, 134, 145, 149, 156, 166, 275
- `workspaces/runtime/packages/runtime-schema/src/agent_limits.rs` (abc; n=1): 71
- `workspaces/runtime/packages/runtime-schema/src/agent_progress.rs` (abc; n=8): 85, 243, 250, 271, 300, 378, 386, 441
- `workspaces/runtime/packages/runtime-schema/src/annotation_record.rs` (abc; n=8): 5, 49, 54, 70, 76, 80, 85, 89
- `workspaces/runtime/packages/runtime-schema/src/api_schema.rs` (b; n=15): 6, 59, 61, 67, 73, 74, 77, 92, 94, 95, 108, 116, 118, 124, 130
- `workspaces/runtime/packages/runtime-schema/src/approvals.rs` (b; n=6): 114, 122, 274, 350, 403, 413
- `workspaces/runtime/packages/runtime-schema/src/chat_operational_status.rs` (b; n=1): 22
- `workspaces/runtime/packages/runtime-schema/src/checkpoint_record.rs` (abc; n=6): 6, 18, 22, 31, 37, 41
- `workspaces/runtime/packages/runtime-schema/src/claude_controls.rs` (b; n=9): 6, 208, 235, 253, 289, 318, 378, 429, 445
- `workspaces/runtime/packages/runtime-schema/src/claude_failure.rs` (b; n=8): 4, 39, 112, 127, 141, 148, 201, 217
- `workspaces/runtime/packages/runtime-schema/src/claude_login.rs` (b; n=6): 3, 10, 43, 73, 92, 158
- `workspaces/runtime/packages/runtime-schema/src/claude_options.rs` (b; n=4): 3, 15, 23, 46
- `workspaces/runtime/packages/runtime-schema/src/command.rs` (b; n=2): 253, 372
- `workspaces/runtime/packages/runtime-schema/src/complaint_record.rs` (abc; n=5): 4, 105, 120, 126, 130
- `workspaces/runtime/packages/runtime-schema/src/complaints.rs` (b; n=4): 5, 15, 22, 30
- `workspaces/runtime/packages/runtime-schema/src/continuation_snapshot.rs` (b; n=9): 9, 346, 407, 411, 444, 449, 451, 471, 472
- `workspaces/runtime/packages/runtime-schema/src/contracts.rs` (b; n=28): 15, 39, 378, 394, 571, 577, 578, 586, 587, 596, 623, 752, 928, 945, 964, 968, 974, 1027, 1033, 1177, 1187, 1189, 1190, 1201, 1206, 1230, 1383, 2124
- `workspaces/runtime/packages/runtime-schema/src/creation_intent.rs` (abc; n=2): 229, 292
- `workspaces/runtime/packages/runtime-schema/src/delivery.rs` (b; n=5): 231, 270, 284, 288, 301
- `workspaces/runtime/packages/runtime-schema/src/desktop.rs` (b; n=1): 41
- `workspaces/runtime/packages/runtime-schema/src/event_metadata.rs` (abd; n=23): 5, 43, 44, 52, 53, 58, 64, 67, 97, 132, 137, 148, 151, 157, 159, 164, 173, 177, 180, 185, 187, 192, 196
- `workspaces/runtime/packages/runtime-schema/src/federation_ingress.rs` (b; n=1): 161
- `workspaces/runtime/packages/runtime-schema/src/http_bounded_ingress.rs` (b; n=1): 60
- `workspaces/runtime/packages/runtime-schema/src/http_ingress.rs` (b; n=2): 31, 35
- `workspaces/runtime/packages/runtime-schema/src/json_compat/serializer.rs` (b; n=2): 191, 232
- `workspaces/runtime/packages/runtime-schema/src/json_compat.rs` (b; n=14): 2, 7, 19, 24, 36, 148, 150, 152, 153, 157, 158, 159, 167, 233
- `workspaces/runtime/packages/runtime-schema/src/layr_evidence.rs` (b; n=7): 3, 23, 31, 55, 59, 60, 61
- `workspaces/runtime/packages/runtime-schema/src/lib.rs` (b; n=27): 114, 474, 483, 509, 563, 577, 585, 592, 598, 616, 650, 664, 676, 680, 681, 682, 812, 814, 816, 906, 1056, 1058, 1076, 1081, 1093, 1122, 1214
- `workspaces/runtime/packages/runtime-schema/src/limit_reset_request.rs` (b; n=1): 68
- `workspaces/runtime/packages/runtime-schema/src/model_catalog.rs` (b; n=9): 6, 91, 109, 122, 135, 150, 154, 187, 197
- `workspaces/runtime/packages/runtime-schema/src/monitor_record.rs` (abc; n=3): 5, 579, 583
- `workspaces/runtime/packages/runtime-schema/src/multi_server_access.rs` (b; n=1): 231
- `workspaces/runtime/packages/runtime-schema/src/multi_server_ingress.rs` (b; n=3): 304, 330, 459
- `workspaces/runtime/packages/runtime-schema/src/multi_server_protocol.rs` (b; n=5): 186, 233, 261, 285, 302
- `workspaces/runtime/packages/runtime-schema/src/notification_text.rs` (abd; n=8): 6, 12, 23, 32, 127, 167, 185, 197
- `workspaces/runtime/packages/runtime-schema/src/persistence.rs` (b; n=2): 95, 519
- `workspaces/runtime/packages/runtime-schema/src/plan_record.rs` (abc; n=20): 5, 53, 83, 103, 110, 113, 117, 124, 137, 143, 156, 163, 167, 169, 170, 174, 181, 185, 248, 250
- `workspaces/runtime/packages/runtime-schema/src/pricing.rs` (b; n=2): 4, 12
- `workspaces/runtime/packages/runtime-schema/src/process_controls.rs` (b; n=14): 5, 429, 442, 466, 496, 568, 584, 602, 615, 616, 671, 691, 692, 802
- `workspaces/runtime/packages/runtime-schema/src/project_ingress.rs` (b; n=1): 132
- `workspaces/runtime/packages/runtime-schema/src/project_record.rs` (abc; n=5): 214, 218, 241, 260, 270
- `workspaces/runtime/packages/runtime-schema/src/provider_call_contract.rs` (ab; n=4): 4, 32, 33, 48
- `workspaces/runtime/packages/runtime-schema/src/provider_errors.rs` (ab; n=40): 7, 23, 26, 27, 29, 30, 32, 63, 74, 77, 101, 102, 121, 122, 123, 142, 169, 174, 182, 184, 215, 225, 251, 271, 282, 293, 298, 302, 304, 316, 317, 323, 324, 347, 348, 384, 640, 654, 655, 686
- `workspaces/runtime/packages/runtime-schema/src/provider_input/items.rs` (ab; n=9): 7, 46, 62, 63, 77, 79, 91, 135, 168
- `workspaces/runtime/packages/runtime-schema/src/provider_input.rs` (ab; n=5): 20, 44, 46, 79, 102
- `workspaces/runtime/packages/runtime-schema/src/provider_kind.rs` (ab; n=5): 11, 56, 65, 70, 72
- `workspaces/runtime/packages/runtime-schema/src/provider_lifecycle.rs` (ab; n=12): 5, 43, 69, 88, 111, 139, 198, 206, 207, 209, 228, 234
- `workspaces/runtime/packages/runtime-schema/src/provider_method_policy.rs` (ab; n=7): 3, 67, 70, 77, 97, 114, 118
- `workspaces/runtime/packages/runtime-schema/src/question_answers.rs` (b; n=5): 8, 37, 63, 72, 86
- `workspaces/runtime/packages/runtime-schema/src/queue_command.rs` (b; n=10): 6, 14, 38, 45, 51, 58, 66, 74, 243, 269
- `workspaces/runtime/packages/runtime-schema/src/radio_controls.rs` (b; n=1): 42
- `workspaces/runtime/packages/runtime-schema/src/raw_json.rs` (b; n=2): 4, 8
- `workspaces/runtime/packages/runtime-schema/src/raw_object.rs` (b; n=7): 5, 11, 22, 26, 31, 50, 68
- `workspaces/runtime/packages/runtime-schema/src/records.rs` (abc; n=5): 6, 54, 58, 68, 78
- `workspaces/runtime/packages/runtime-schema/src/recovery_record.rs` (abc; n=14): 7, 145, 168, 198, 210, 212, 216, 222, 225, 236, 264, 271, 326, 331
- `workspaces/runtime/packages/runtime-schema/src/request_projection.rs` (b; n=12): 5, 25, 27, 35, 48, 72, 79, 112, 119, 169, 189, 196
- `workspaces/runtime/packages/runtime-schema/src/request_record.rs` (abc; n=14): 4, 109, 110, 128, 129, 172, 173, 176, 177, 185, 186, 204, 238, 242
- `workspaces/runtime/packages/runtime-schema/src/resource_notification.rs` (abd; n=2): 50, 54
- `workspaces/runtime/packages/runtime-schema/src/resource_ref.rs` (b; n=1): 341
- `workspaces/runtime/packages/runtime-schema/src/room_record.rs` (abc; n=11): 132, 146, 154, 163, 170, 243, 339, 355, 407, 408, 432
- `workspaces/runtime/packages/runtime-schema/src/rule_record.rs` (abc; n=3): 5, 260, 264
- `workspaces/runtime/packages/runtime-schema/src/start_attempt.rs` (b; n=3): 31, 416, 471
- `workspaces/runtime/packages/runtime-schema/src/stored_record.rs` (abc; n=4): 113, 135, 137, 139
- `workspaces/runtime/packages/runtime-schema/src/studio_errors.rs` (b; n=5): 4, 375, 388, 426, 473
- `workspaces/runtime/packages/runtime-schema/src/supervisor_contract.rs` (b; n=4): 4, 28, 52, 67
- `workspaces/runtime/packages/runtime-schema/src/sync_entity_payload.rs` (abc; n=4): 65, 80, 88, 93
- `workspaces/runtime/packages/runtime-schema/src/sync_protocol.rs` (abc; n=2): 49, 56
- `workspaces/runtime/packages/runtime-schema/src/task_command.rs` (b; n=32): 8, 113, 114, 142, 143, 173, 210, 211, 217, 218, 231, 232, 246, 279, 283, 290, 299, 311, 350, 368, 371, 389, 390, 425, 435, 445, 459, 488, 532, 543, 598, 659
- `workspaces/runtime/packages/runtime-schema/src/task_gate.rs` (b; n=8): 3, 61, 67, 75, 93, 151, 159, 179
- `workspaces/runtime/packages/runtime-schema/src/task_record.rs` (abc; n=7): 4, 96, 229, 341, 344, 387, 391
- `workspaces/runtime/packages/runtime-schema/src/task_results.rs` (b; n=15): 3, 23, 41, 101, 128, 133, 144, 149, 153, 158, 182, 226, 244, 245, 247
- `workspaces/runtime/packages/runtime-schema/src/terminal_ingress.rs` (b; n=2): 95, 99
- `workspaces/runtime/packages/runtime-schema/src/terminal_record.rs` (abc; n=13): 5, 26, 30, 40, 44, 47, 50, 52, 56, 59, 69, 76, 89
- `workspaces/runtime/packages/runtime-schema/src/tool_result.rs` (b; n=34): 5, 17, 40, 44, 49, 50, 57, 65, 70, 72, 74, 80, 82, 86, 88, 89, 90, 95, 96, 103, 105, 106, 107, 109, 113, 118, 128, 130, 136, 139, 199, 202, 207, 209
- `workspaces/runtime/packages/runtime-schema/src/transcript_asset.rs` (b; n=11): 7, 8, 42, 43, 79, 120, 181, 211, 258, 263, 267
- `workspaces/runtime/packages/runtime-schema/src/transcript_record.rs` (abc; n=28): 5, 6, 102, 103, 115, 116, 162, 163, 179, 180, 208, 237, 271, 294, 332, 338, 358, 369, 378, 381, 412, 422, 437, 446, 451, 455, 494, 508
- `workspaces/runtime/packages/runtime-schema/src/usage/ledger.rs` (b; n=18): 8, 96, 97, 117, 136, 317, 323, 328, 346, 378, 382, 435, 501, 512, 514, 524, 572, 612
- `workspaces/runtime/packages/runtime-schema/src/usage/persisted.rs` (b; n=19): 5, 11, 12, 40, 85, 109, 120, 130, 137, 138, 169, 171, 177, 180, 183, 195, 264, 274, 317
- `workspaces/runtime/packages/runtime-schema/src/usage.rs` (b; n=36): 9, 251, 252, 269, 272, 282, 288, 302, 304, 305, 352, 354, 355, 359, 361, 368, 400, 405, 422, 434, 520, 522, 582, 583, 590, 591, 598, 607, 611, 626, 789, 791, 795, 802, 807, 819
- `workspaces/runtime/packages/runtime-schema/src/usage_limits.rs` (b; n=5): 1003, 1137, 1169, 1204, 1235
- `workspaces/runtime/packages/runtime-schema/src/usage_resume_record.rs` (abc; n=6): 6, 24, 25, 27, 30, 35
- `workspaces/runtime/packages/runtime-schema/src/work_item_record.rs` (abc; n=41): 8, 54, 81, 82, 88, 89, 95, 96, 98, 141, 144, 147, 150, 153, 159, 178, 181, 183, 190, 194, 195, 196, 197, 201, 204, 205, 211, 213, 214, 216, 221, 227, 231, 233, 237, 240, 245, 247, 252, 256, 297
- `workspaces/runtime/packages/runtime-schema/src/workspace_modes.rs` (b; n=5): 2, 78, 101, 109, 172
- `workspaces/runtime/packages/runtime-schema/src/workspace_operation.rs` (b; n=12): 5, 58, 59, 61, 62, 64, 65, 70, 74, 77, 96, 100

### studio-runtime-server

16 files, 297 matching lines.

- `workspaces/runtime/apps/runtime-server/src/account_registry.rs` (?; n=7): 1484, 2071, 2076, 2162, 2227, 2343, 2406
- `workspaces/runtime/apps/runtime-server/src/history_runtime.rs` (?; n=13): 10, 58, 406, 903, 923, 932, 1094, 1097, 1263, 1670, 1674, 1686, 1973
- `workspaces/runtime/apps/runtime-server/src/host_runtime.rs` (?; n=56): 20, 173, 370, 917, 922, 1031, 1042, 1077, 1210, 1249, 1322, 1323, 1326, 1470, 1471, 1473, 1481, 1483, 1485, 1487, 1489, 1491, 1492, 1499, 1500, 1501, 1502, 1503, 1504, 1505, 1508, 1509, 1510, 1511, 1512, 1513, 1514, 1517, 1518, 1519, 1520, 1521, 1522, 1742, 2276, 2307, 2365, 2511, 2610, 3065, 3134, 3416, 3759, 3782, 3840, 4038
- `workspaces/runtime/apps/runtime-server/src/main.rs` (?; n=7): 363, 1905, 1947, 2290, 2292, 2313, 2374
- `workspaces/runtime/apps/runtime-server/src/monitor_port.rs` (?; n=31): 20, 83, 87, 104, 110, 118, 121, 124, 126, 127, 130, 631, 772, 1838, 1859, 1975, 2081, 2082, 2103, 2117, 2120, 2151, 2176, 2181, 2340, 2346, 3081, 3187, 3241, 4442, 4451
- `workspaces/runtime/apps/runtime-server/src/multi_server_discovery.rs` (?; n=30): 15, 43, 45, 47, 49, 51, 53, 55, 59, 63, 95, 116, 118, 168, 187, 314, 425, 447, 454, 473, 477, 518, 520, 521, 611, 612, 717, 841, 983, 1147
- `workspaces/runtime/apps/runtime-server/src/multi_server_owner.rs` (?; n=12): 15, 27, 51, 55, 76, 96, 107, 206, 211, 223, 226, 233
- `workspaces/runtime/apps/runtime-server/src/native_probe.rs` (?; n=24): 15, 113, 201, 202, 209, 219, 225, 228, 250, 328, 332, 346, 550, 551, 590, 604, 606, 607, 608, 612, 617, 702, 703, 728
- `workspaces/runtime/apps/runtime-server/src/operator_cli/linux_vm.rs` (?; n=1): 60
- `workspaces/runtime/apps/runtime-server/src/provider_ports.rs` (ab; n=30): 27, 55, 79, 84, 95, 105, 119, 167, 189, 307, 312, 336, 339, 379, 381, 562, 708, 779, 783, 877, 939, 940, 1279, 1282, 1297, 1298, 1300, 1360, 1361, 1367
- `workspaces/runtime/apps/runtime-server/tests/browser_launcher.rs` (?; n=1): 80
- `workspaces/runtime/apps/runtime-server/tests/fixture_support.rs` (?; n=4): 4, 26, 36, 55
- `workspaces/runtime/apps/runtime-server/tests/host_exec_service.rs` (?; n=7): 16, 107, 116, 231, 273, 289, 516
- `workspaces/runtime/apps/runtime-server/tests/linux_vm_start.rs` (?; n=1): 61
- `workspaces/runtime/apps/runtime-server/tests/state_check.rs` (?; n=1): 30
- `workspaces/runtime/apps/runtime-server/tests/wave7_p01_registry.rs` (?; n=72): 18, 59, 118, 308, 424, 678, 683, 689, 692, 701, 702, 717, 743, 752, 756, 761, 816, 831, 841, 853, 854, 855, 856, 858, 859, 861, 1039, 1222, 1351, 1366, 1731, 1829, 1967, 1984, 2507, 2513, 2535, 2928, 3046, 3312, 3338, 3364, 3403, 3428, 3492, 3678, 3719, 3728, 3740, 3742, 3745, 3747, 3748, 3749, 3756, 3758, 3764, 3774, 3912, 4093, 4129, 4163, 4465, 4626, 4644, 4704, 4729, 4791, 4803, 4821, 4904, 4919

### studio-runtime-services

459 files, 8819 matching lines.

- `workspaces/runtime/packages/runtime-services/src/account_auth_gate.rs` (?; n=9): 16, 40, 41, 72, 74, 75, 93, 113, 187
- `workspaces/runtime/packages/runtime-services/src/account_driver/auth_cache.rs` (?; n=9): 83, 84, 85, 86, 95, 99, 109, 111, 113
- `workspaces/runtime/packages/runtime-services/src/account_driver/claude.rs` (?; n=20): 48, 68, 75, 128, 149, 178, 193, 201, 239, 254, 261, 269, 271, 281, 284, 306, 344, 362, 367, 394
- `workspaces/runtime/packages/runtime-services/src/account_driver/claude_identity.rs` (?; n=3): 109, 195, 199
- `workspaces/runtime/packages/runtime-services/src/account_driver.rs` (?; n=259): 27, 69, 138, 254, 443, 446, 453, 463, 607, 701, 718, 830, 837, 882, 888, 1036, 1151, 1156, 1164, 1185, 1188, 1192, 1203, 1214, 1220, 1274, 1315, 1328, 1339, 1350, 1369, 1411, 1428, 1451, 1455, 1484, 1497, 1501, 1572, 1612, 1650, 1686, 1700, 1755, 1756, 1760, 1762, 1774, 1779, 1840, 1872, 1875, 1881, 1904, 1905, 1910, 1911, 1932, 2067, 2093, 2100, 2112, 2118, 2119, 2124, 2125, 2133, 2203, 2231, 2234, 2239, 2264, 2334, 2360, 2368, 2378, 2380, 2385, 2406, 2407, 2411, 2414, 2427, 2525, 2537, 2545, 2561, 2563, 2577, 2626, 2639, 2644, 2645, 2669, 2674, 2675, 2683, 2692, 2702, 2719, 2725, 2728, 2730, 2734, 2746, 2750, 2757, 2776, 2779, 2781, 2782, 2783, 2787, 2789, 2795, 2806, 2810, 2816, 2818, 2821, 2828, 2830, 2833, 2842, 2844, 2851, 2853, 2854, 2857, 2863, 2874, 2876, 2886, 2893, 2894, 2901, 2904, 2914, 2918, 2929, 2938, 2948, 2964, 2997, 3006, 3018, 3033, 3036, 3042, 3044, 3047, 3053, 3060, 3061, 3062, 3219, 3230, 3236, 3256, 3263, 3264, 3266, 3269, 3302, 3303, 3313, 3316, 3330, 3335, 3345, 3351, 3352, 3362, 3410, 3411, 3414, 3422, 3430, 3434, 3438, 3443, 3472, 3629, 3675, 3687, 3688, 3708, 3710, 3720, 4081, 4082, 4083, 4084, 4085, 4087, 4089, 4090, 4115, 4131, 4133, 4134, 4135, 4136, 4137, 4138, 4140, 4141, 4205, 4227, 4228, 4229, 4231, 4232, 4244, 4276, 4277, 4315, 4319, 4456, 4461, 4464, 4503, 4532, 4593, 4607, 4608, 4623, 4668, 4687, 4820, 5029, 5038, 5157, 5174, 5175, 5176, 5178, 5211, 5215, 5254, 5255, 5256, 5269, 5288, 5306, 5311, 5314, 5316, 5331, 5349, 5350, 5351, 5352, 5362, 5363, 5364, 5366, 5367, 5371
- `workspaces/runtime/packages/runtime-services/src/account_policy.rs` (?; n=54): 6, 13, 18, 49, 58, 66, 83, 84, 85, 86, 87, 88, 89, 100, 131, 132, 133, 134, 135, 136, 137, 142, 145, 149, 152, 161, 182, 193, 219, 226, 250, 251, 252, 253, 254, 255, 256, 264, 274, 280, 348, 391, 452, 458, 473, 528, 576, 587, 593, 600, 700, 770, 808, 846
- `workspaces/runtime/packages/runtime-services/src/account_preference.rs` (?; n=8): 12, 81, 87, 194, 196, 200, 313, 354
- `workspaces/runtime/packages/runtime-services/src/account_selection.rs` (?; n=15): 3, 12, 23, 26, 44, 49, 51, 60, 83, 134, 220, 223, 273, 284, 287
- `workspaces/runtime/packages/runtime-services/src/account_session_runtime.rs` (?; n=83): 15, 58, 59, 79, 96, 105, 195, 238, 255, 261, 281, 295, 337, 380, 412, 431, 432, 444, 445, 459, 460, 461, 470, 471, 472, 482, 487, 491, 498, 499, 500, 512, 513, 514, 515, 516, 536, 537, 753, 755, 894, 910, 1008, 1188, 1237, 1242, 1250, 1279, 1430, 1609, 1616, 1672, 1676, 2037, 2207, 2246, 2248, 2253, 2258, 2289, 2314, 2337, 2443, 2446, 2471, 2509, 2604, 2610, 2611, 2856, 2927, 2955, 2988, 3035, 3036, 3078, 3094, 3117, 3150, 3154, 3237, 3274, 3298
- `workspaces/runtime/packages/runtime-services/src/account_start_gate.rs` (?; n=7): 31, 101, 104, 131, 490, 1076, 1080
- `workspaces/runtime/packages/runtime-services/src/account_transfer.rs` (?; n=173): 7, 44, 56, 61, 68, 81, 125, 174, 184, 185, 354, 366, 370, 553, 557, 561, 562, 574, 604, 607, 640, 732, 780, 917, 918, 919, 925, 926, 928, 936, 943, 959, 968, 992, 1019, 1020, 1022, 1076, 1078, 1141, 1152, 1157, 1182, 1217, 1219, 1257, 1284, 1286, 1293, 1304, 1338, 1354, 1400, 1402, 1411, 1415, 1427, 1433, 1460, 1464, 1514, 1590, 1595, 1600, 1601, 1607, 1608, 1609, 1635, 1656, 1677, 1678, 1679, 1680, 1699, 1701, 1702, 1713, 1724, 1743, 1769, 1773, 1781, 1837, 1838, 1840, 1846, 1860, 1882, 1960, 1963, 1964, 1967, 2010, 2018, 2025, 2030, 2035, 2036, 2037, 2040, 2041, 2057, 2076, 2080, 2081, 2085, 2088, 2091, 2100, 2107, 2130, 2167, 2168, 2169, 2172, 2173, 2197, 2222, 2223, 2230, 2231, 2234, 2309, 2311, 2338, 2360, 2383, 2443, 2521, 2539, 2546, 2547, 2548, 2549, 2550, 2551, 2552, 2553, 2554, 2555, 2557, 2558, 2559, 2573, 2574, 2577, 2578, 2672, 2964, 2966, 2973, 2975, 2983, 2985, 3011, 3013, 3021, 3023, 3035, 3036, 3077, 3078, 3386, 3396, 3410, 3419, 3425, 3426, 3434, 3671, 3879, 3915
- `workspaces/runtime/packages/runtime-services/src/account_transfer_driver.rs` (?; n=33): 11, 195, 211, 235, 274, 295, 454, 481, 498, 507, 622, 638, 689, 725, 747, 771, 841, 911, 958, 1011, 1034, 1152, 1276, 1419, 1471, 1517, 1723, 1833, 1922, 1942, 2002, 2051, 2082
- `workspaces/runtime/packages/runtime-services/src/account_transfer_executor.rs` (?; n=67): 36, 95, 175, 191, 203, 230, 231, 232, 233, 236, 280, 306, 343, 354, 393, 533, 556, 567, 647, 683, 686, 829, 835, 950, 976, 979, 980, 981, 982, 1025, 1026, 1027, 1028, 1029, 1030, 1031, 1032, 1057, 1066, 1067, 1069, 1071, 1072, 1263, 1348, 1446, 1534, 1589, 1915, 1920, 1945, 1972, 2013, 2058, 2111, 2173, 2218, 2500, 2704, 2732, 2856, 2966, 3005, 3123, 3319, 3372, 3442
- `workspaces/runtime/packages/runtime-services/src/account_worker/agent_fault.rs` (ac; n=2): 387, 499
- `workspaces/runtime/packages/runtime-services/src/account_worker/callback.rs` (?; n=15): 3, 75, 100, 103, 141, 152, 274, 284, 292, 379, 507, 512, 538, 549, 946
- `workspaces/runtime/packages/runtime-services/src/account_worker/callback_record.rs` (ac; n=2): 616, 620
- `workspaces/runtime/packages/runtime-services/src/account_worker/connection_recovery.rs` (?; n=15): 17, 63, 69, 377, 379, 390, 412, 424, 433, 447, 455, 487, 488, 495, 554
- `workspaces/runtime/packages/runtime-services/src/account_worker/history.rs` (?; n=5): 3, 10, 64, 65, 90
- `workspaces/runtime/packages/runtime-services/src/account_worker/idle_unload.rs` (?; n=3): 846, 850, 1126
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation/compaction_tests.rs` (?; n=1): 200
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation/pipeline.rs` (?; n=43): 14, 31, 108, 238, 506, 515, 611, 642, 763, 893, 929, 1147, 1221, 1289, 1304, 1306, 1307, 1387, 1413, 1417, 1418, 1429, 1448, 1449, 1451, 1459, 1463, 1500, 1502, 1503, 1504, 1505, 1506, 1507, 1533, 1536, 1658, 1749, 1750, 1971, 2126, 2142, 2213
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation/receipts.rs` (?; n=4): 49, 122, 130, 212
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation/snapshot.rs` (?; n=76): 62, 107, 115, 127, 153, 155, 177, 208, 257, 295, 347, 382, 383, 405, 421, 514, 801, 809, 812, 817, 819, 820, 825, 836, 848, 857, 873, 907, 908, 932, 950, 1015, 1029, 1049, 1059, 1069, 1070, 1084, 1087, 1118, 1128, 1145, 1163, 1165, 1166, 1167, 1168, 1169, 1170, 1211, 1212, 1254, 1278, 1280, 1281, 1302, 1311, 1449, 1457, 1588, 1675, 1688, 1860, 1911, 1912, 1954, 2045, 2091, 2098, 2216, 2217, 2562, 2602, 2684, 3124, 3230
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation/team_restore.rs` (a; n=3): 12, 25, 100
- `workspaces/runtime/packages/runtime-services/src/account_worker/preparation.rs` (?; n=9): 41, 48, 60, 61, 78, 89, 96, 97, 197
- `workspaces/runtime/packages/runtime-services/src/account_worker/restart_recovery.rs` (?; n=6): 20, 576, 786, 816, 817, 824
- `workspaces/runtime/packages/runtime-services/src/account_worker/retained/consumers.rs` (?; n=4): 123, 129, 186, 242
- `workspaces/runtime/packages/runtime-services/src/account_worker/retained/runtime.rs` (?; n=7): 60, 664, 683, 684, 695, 696, 717
- `workspaces/runtime/packages/runtime-services/src/account_worker/retained/storage.rs` (?; n=10): 130, 205, 209, 237, 238, 302, 350, 399, 507, 1613
- `workspaces/runtime/packages/runtime-services/src/account_worker/retained.rs` (?; n=56): 24, 40, 42, 165, 173, 324, 329, 334, 335, 373, 580, 615, 628, 631, 650, 668, 676, 682, 696, 703, 707, 712, 717, 723, 731, 756, 760, 776, 782, 788, 797, 814, 817, 836, 844, 857, 870, 874, 907, 928, 932, 937, 1073, 1097, 1103, 1107, 1114, 1128, 1143, 1145, 1237, 1455, 1517, 1619, 1638, 1689
- `workspaces/runtime/packages/runtime-services/src/account_worker/steer.rs` (?; n=2): 105, 263
- `workspaces/runtime/packages/runtime-services/src/account_worker/stopped_turns.rs` (?; n=13): 17, 57, 58, 276, 277, 420, 431, 432, 434, 435, 463, 601, 766
- `workspaces/runtime/packages/runtime-services/src/account_worker/store.rs` (a; n=18): 512, 513, 1089, 2114, 2118, 2238, 2252, 2515, 2528, 2613, 2648, 2652, 2659, 3408, 4103, 4534, 4761, 4785
- `workspaces/runtime/packages/runtime-services/src/account_worker/submission.rs` (?; n=2): 22, 232
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/account_transfer.rs` (?; n=5): 105, 304, 319, 328, 420
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/agent_isolation.rs` (ac; n=7): 39, 81, 97, 620, 636, 718, 922
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/auth_hold.rs` (?; n=6): 133, 137, 138, 162, 223, 542
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/browser_skill.rs` (?; n=2): 25, 82
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/callback_handoff.rs` (?; n=11): 18, 22, 26, 48, 75, 95, 96, 172, 204, 1280, 1444
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/callback_head_of_line.rs` (?; n=12): 51, 84, 102, 104, 122, 136, 220, 228, 237, 268, 317, 387
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/capacity_retry.rs` (?; n=1): 103
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/claude_preparation.rs` (?; n=4): 68, 72, 73, 106
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/closure.rs` (?; n=4): 964, 1059, 1188, 1308
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/continuation_start.rs` (?; n=10): 65, 105, 132, 173, 178, 219, 436, 491, 636, 644
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/crash_matrix.rs` (?; n=5): 11, 12, 122, 376, 694
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/exited_generation_callback.rs` (?; n=2): 19, 38
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/idle_unload.rs` (?; n=4): 50, 180, 231, 1031
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/journal_cost.rs` (?; n=4): 11, 75, 123, 139
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/journal_publication.rs` (?; n=4): 109, 173, 445, 549
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/late_thread_reply.rs` (?; n=1): 18
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/live_deltas.rs` (?; n=1): 48
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/login_completion.rs` (?; n=2): 69, 70
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/manual_release_closure.rs` (?; n=1): 21
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/many_agents.rs` (ac; n=8): 71, 78, 84, 148, 264, 322, 334, 395
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/native_requests.rs` (?; n=6): 17, 26, 57, 63, 122, 123
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/native_review.rs` (c; n=1): 18
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/preparation_review.rs` (?; n=3): 4, 413, 634
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/provider_started_turn.rs` (ab; n=2): 14, 20
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/publication_drain.rs` (?; n=11): 10, 34, 41, 47, 52, 55, 236, 262, 292, 634, 787
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/publication_off_path.rs` (?; n=3): 5, 85, 363
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/retained.rs` (?; n=26): 22, 48, 170, 171, 257, 636, 642, 644, 655, 785, 1187, 1240, 1668, 1930, 2078, 2271, 2355, 2451, 2652, 2918, 2946, 3159, 3932, 3954, 3955, 3962
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/settled_streams.rs` (ad; n=1): 17
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/settled_turn_tasks.rs` (?; n=3): 7, 17, 36
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/snapshot_changed.rs` (?; n=1): 127
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/stale_disconnect_recovery.rs` (?; n=3): 37, 53, 87
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/start_throttle.rs` (?; n=1): 37
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/steer.rs` (?; n=15): 57, 139, 153, 190, 216, 227, 233, 273, 353, 664, 680, 769, 875, 978, 1210
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/stop_after_reattach.rs` (?; n=1): 472
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/tool_reply_retirement.rs` (?; n=4): 24, 26, 34, 48
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/unsent_start.rs` (?; n=1): 30
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/vm_attachments.rs` (?; n=3): 14, 17, 18
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/vm_notifications.rs` (ad; n=7): 12, 15, 16, 209, 210, 274, 328
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests/vm_reattach.rs` (?; n=9): 5, 11, 29, 32, 33, 95, 142, 146, 375
- `workspaces/runtime/packages/runtime-services/src/account_worker/tests.rs` (?; n=76): 389, 465, 676, 685, 750, 769, 781, 798, 973, 988, 1025, 1200, 1265, 1279, 2042, 2044, 2061, 2075, 2088, 2090, 2148, 2162, 2260, 2375, 2820, 2824, 2825, 2918, 2931, 2976, 3439, 3450, 3456, 3577, 3578, 3593, 3730, 4265, 4268, 4271, 4274, 4277, 4405, 4408, 4417, 4421, 4422, 4432, 4442, 4473, 4477, 4480, 4619, 4854, 5109, 5112, 5115, 5291, 5295, 5299, 5303, 5355, 5362, 5454, 5541, 5578, 6349, 6353, 6356, 6474, 6543, 6600, 6666, 6728, 6798, 6903
- `workspaces/runtime/packages/runtime-services/src/account_worker/vm_reattach.rs` (?; n=3): 56, 107, 323
- `workspaces/runtime/packages/runtime-services/src/account_worker.rs` (?; n=13): 19, 591, 1224, 1226, 2194, 3139, 3178, 3188, 3237, 3293, 3330, 4359, 4364
- `workspaces/runtime/packages/runtime-services/src/agent_lifecycle_runtime.rs` (ac; n=10): 7, 45, 46, 52, 67, 70, 143, 165, 188, 200
- `workspaces/runtime/packages/runtime-services/src/agent_management.rs` (ac; n=83): 8, 147, 207, 219, 224, 230, 236, 241, 262, 271, 287, 327, 370, 392, 508, 511, 516, 518, 538, 543, 550, 583, 585, 605, 611, 616, 642, 692, 694, 711, 714, 728, 736, 740, 741, 742, 743, 748, 752, 753, 757, 788, 829, 833, 839, 847, 850, 854, 859, 863, 865, 883, 884, 890, 905, 972, 1015, 1023, 1101, 1135, 1140, 1243, 1246, 1259, 1261, 1278, 1323, 1405, 1433, 1449, 1483, 1484, 1497, 1505, 1543, 1545, 1576, 1586, 1958, 1962, 1979, 1990, 2082
- `workspaces/runtime/packages/runtime-services/src/agent_management_maintenance.rs` (ac; n=16): 13, 18, 19, 20, 26, 30, 35, 54, 55, 64, 73, 82, 110, 178, 190, 201
- `workspaces/runtime/packages/runtime-services/src/agent_management_recovery.rs` (ac; n=22): 12, 42, 43, 53, 112, 124, 193, 247, 382, 394, 441, 695, 699, 700, 763, 836, 896, 942, 1358, 1361, 1460, 1787
- `workspaces/runtime/packages/runtime-services/src/agent_management_store.rs` (ac; n=26): 8, 21, 55, 61, 82, 93, 288, 301, 310, 319, 336, 342, 343, 346, 367, 402, 475, 530, 531, 615, 642, 1046, 1048, 1076, 1111, 1237
- `workspaces/runtime/packages/runtime-services/src/agent_move.rs` (ac; n=21): 7, 53, 57, 58, 62, 63, 71, 77, 94, 111, 141, 155, 158, 179, 192, 229, 236, 248, 266, 269, 326
- `workspaces/runtime/packages/runtime-services/src/agent_progress.rs` (ac; n=3): 77, 391, 488
- `workspaces/runtime/packages/runtime-services/src/agents_create/operations.rs` (ac; n=157): 31, 121, 199, 207, 221, 259, 327, 338, 350, 507, 546, 549, 642, 668, 675, 698, 734, 754, 770, 779, 1100, 1117, 1120, 1123, 1125, 1133, 1144, 1150, 1186, 1188, 1198, 1202, 1212, 1220, 1286, 1293, 1310, 1329, 1338, 1348, 1354, 1364, 1369, 1432, 1434, 1437, 1445, 1513, 1515, 1517, 1537, 1591, 1598, 1647, 1648, 1653, 1662, 1663, 1664, 1667, 1683, 1684, 1694, 1742, 1840, 1858, 1897, 1910, 1922, 1929, 1976, 2080, 2327, 2352, 2354, 2355, 2356, 2357, 2358, 2359, 2364, 2392, 2457, 2474, 2476, 2480, 2489, 2490, 2494, 2497, 2534, 2676, 2925, 2926, 2982, 2983, 2984, 2985, 2986, 3035, 3064, 3065, 3066, 3067, 3068, 3069, 3071, 3196, 3197, 3200, 3203, 3204, 3207, 3210, 3213, 3214, 3223, 3227, 3241, 3374, 3375, 3379, 3381, 3389, 3392, 3403, 3409, 3414, 3418, 3425, 3427, 3428, 3429, 3430, 3431, 3452, 3453, 3546, 4108, 4109, 4110, 4133, 4160, 4161, 4325, 4408, 4420, 4459, 4513, 4691, 4777, 4870, 4882, 5027, 5121, 5390, 5391
- `workspaces/runtime/packages/runtime-services/src/agents_create.rs` (ac; n=61): 16, 89, 100, 102, 192, 196, 203, 224, 225, 226, 237, 240, 262, 263, 291, 294, 309, 310, 337, 338, 353, 354, 373, 374, 379, 381, 389, 390, 391, 405, 409, 421, 426, 440, 441, 442, 443, 444, 445, 454, 455, 493, 507, 508, 526, 527, 538, 539, 596, 597, 609, 618, 627, 632, 646, 647, 722, 790, 1082, 1093, 1169
- `workspaces/runtime/packages/runtime-services/src/analytics.rs` (?; n=17): 7, 14, 24, 57, 58, 70, 91, 93, 96, 104, 139, 165, 167, 168, 170, 177, 211
- `workspaces/runtime/packages/runtime-services/src/analytics_capture.rs` (?; n=67): 9, 57, 59, 63, 64, 65, 75, 77, 96, 98, 115, 117, 128, 130, 186, 187, 208, 215, 222, 265, 268, 292, 293, 329, 369, 370, 371, 372, 373, 374, 375, 379, 526, 528, 870, 880, 1010, 1082, 1083, 1126, 1227, 1228, 1237, 1297, 1311, 1332, 1344, 1346, 1358, 1365, 1366, 1368, 1375, 1383, 1418, 1440, 1445, 1455, 1478, 1677, 1723, 1749, 1750, 1844, 1972, 2063, 2069
- `workspaces/runtime/packages/runtime-services/src/analytics_history.rs` (?; n=58): 13, 67, 243, 244, 245, 250, 251, 262, 286, 287, 288, 294, 341, 353, 379, 425, 426, 439, 440, 445, 458, 460, 464, 490, 492, 493, 494, 495, 496, 497, 502, 507, 510, 533, 742, 762, 764, 766, 768, 772, 775, 785, 813, 817, 821, 830, 1018, 1088, 1114, 1159, 1164, 1165, 1167, 1206, 1247, 1250, 1347, 1483
- `workspaces/runtime/packages/runtime-services/src/analytics_history_schedule.rs` (?; n=53): 6, 11, 13, 14, 19, 36, 38, 40, 42, 54, 56, 81, 194, 346, 347, 381, 445, 454, 465, 486, 487, 488, 489, 537, 539, 553, 691, 692, 699, 895, 913, 917, 956, 989, 991, 1049, 1050, 1088, 1136, 1153, 1171, 1172, 1173, 1174, 1178, 1193, 1210, 1214, 1217, 1224, 1225, 1244, 1246
- `workspaces/runtime/packages/runtime-services/src/analytics_measurements.rs` (?; n=12): 3, 5, 7, 20, 22, 27, 35, 70, 73, 91, 96, 98
- `workspaces/runtime/packages/runtime-services/src/analytics_projection.rs` (?; n=6): 244, 257, 264, 372, 389, 583
- `workspaces/runtime/packages/runtime-services/src/analytics_rollout.rs` (?; n=25): 4, 13, 17, 24, 27, 38, 46, 54, 239, 242, 288, 297, 303, 310, 318, 320, 330, 338, 345, 365, 369, 412, 413, 420, 534
- `workspaces/runtime/packages/runtime-services/src/approval_sync.rs` (ac; n=1): 36
- `workspaces/runtime/packages/runtime-services/src/browser.rs` (?; n=62): 25, 100, 111, 267, 268, 272, 276, 280, 380, 401, 402, 423, 424, 439, 1286, 1289, 1304, 1309, 1321, 1360, 1378, 1381, 1396, 1397, 1414, 1415, 1437, 1449, 1456, 1477, 1479, 1519, 1540, 1544, 1550, 1571, 1588, 1595, 1596, 1612, 1636, 1637, 1671, 1680, 1691, 1699, 1701, 1742, 1743, 1752, 1757, 1773, 1951, 1983, 2185, 2259, 2374, 2492, 2594, 2675, 2753, 2906
- `workspaces/runtime/packages/runtime-services/src/browser_answer.rs` (?; n=16): 6, 84, 154, 175, 178, 200, 250, 325, 352, 467, 468, 479, 715, 727, 730, 731
- `workspaces/runtime/packages/runtime-services/src/browser_answer_paste_tests.rs` (?; n=9): 8, 14, 17, 23, 45, 48, 49, 64, 76
- `workspaces/runtime/packages/runtime-services/src/browser_cards.rs` (?; n=40): 15, 83, 85, 86, 87, 88, 93, 95, 96, 97, 98, 99, 100, 261, 262, 321, 325, 337, 342, 354, 366, 367, 368, 404, 410, 460, 461, 470, 471, 475, 484, 536, 557, 558, 564, 583, 596, 599, 657, 662
- `workspaces/runtime/packages/runtime-services/src/browser_cards_tests.rs` (?; n=14): 8, 15, 16, 23, 26, 27, 49, 93, 107, 135, 136, 188, 227, 681
- `workspaces/runtime/packages/runtime-services/src/browser_host.rs` (?; n=14): 21, 447, 455, 458, 463, 465, 466, 467, 468, 469, 470, 1470, 2111, 2124
- `workspaces/runtime/packages/runtime-services/src/browser_host_lifetime_tests.rs` (?; n=2): 25, 47
- `workspaces/runtime/packages/runtime-services/src/browser_host_local_open_tests.rs` (?; n=2): 269, 289
- `workspaces/runtime/packages/runtime-services/src/browser_policy.rs` (?; n=1): 726
- `workspaces/runtime/packages/runtime-services/src/callback_runtime.rs` (?; n=8): 6, 217, 234, 237, 249, 251, 261, 395
- `workspaces/runtime/packages/runtime-services/src/capacity_retry.rs` (?; n=4): 9, 217, 270, 350
- `workspaces/runtime/packages/runtime-services/src/catalog_reads.rs` (?; n=40): 9, 19, 25, 40, 45, 48, 53, 59, 73, 141, 146, 165, 168, 221, 224, 230, 240, 254, 259, 262, 272, 289, 290, 295, 302, 304, 322, 335, 361, 366, 368, 386, 395, 398, 423, 433, 446, 447, 448, 468
- `workspaces/runtime/packages/runtime-services/src/checkpoints.rs` (?; n=28): 12, 149, 168, 217, 226, 307, 403, 448, 525, 582, 592, 598, 722, 750, 768, 773, 786, 788, 802, 808, 820, 824, 846, 861, 873, 877, 1007, 1012
- `workspaces/runtime/packages/runtime-services/src/child_permissions.rs` (?; n=4): 45, 46, 48, 55
- `workspaces/runtime/packages/runtime-services/src/claude_bridge.rs` (?; n=19): 6, 44, 66, 74, 75, 79, 113, 121, 142, 301, 317, 319, 327, 334, 336, 339, 340, 345, 355
- `workspaces/runtime/packages/runtime-services/src/claude_controls/service.rs` (?; n=25): 91, 120, 122, 132, 142, 144, 152, 165, 166, 168, 169, 172, 216, 241, 257, 272, 304, 314, 422, 424, 425, 434, 451, 461, 502
- `workspaces/runtime/packages/runtime-services/src/claude_controls/store.rs` (a; n=6): 45, 124, 126, 197, 239, 273
- `workspaces/runtime/packages/runtime-services/src/claude_controls.rs` (?; n=32): 6, 27, 42, 46, 62, 70, 72, 76, 77, 78, 84, 88, 109, 127, 154, 179, 182, 199, 225, 230, 238, 252, 259, 267, 294, 340, 342, 349, 357, 358, 367, 387
- `workspaces/runtime/packages/runtime-services/src/claude_input_recovery/runtime/submission.rs` (?; n=3): 8, 21, 33
- `workspaces/runtime/packages/runtime-services/src/claude_input_recovery/runtime.rs` (?; n=20): 5, 51, 62, 70, 73, 86, 91, 95, 123, 135, 172, 175, 178, 182, 184, 185, 186, 187, 190, 207
- `workspaces/runtime/packages/runtime-services/src/claude_input_recovery.rs` (?; n=15): 8, 71, 72, 75, 88, 98, 103, 109, 118, 119, 120, 122, 124, 130, 131
- `workspaces/runtime/packages/runtime-services/src/claude_login/chats.rs` (?; n=4): 4, 16, 38, 41
- `workspaces/runtime/packages/runtime-services/src/claude_login/ports.rs` (?; n=15): 12, 56, 57, 58, 61, 65, 69, 71, 74, 78, 82, 88, 92, 101, 103
- `workspaces/runtime/packages/runtime-services/src/claude_login/service.rs` (?; n=12): 181, 273, 281, 291, 303, 331, 339, 431, 481, 930, 933, 1044
- `workspaces/runtime/packages/runtime-services/src/claude_login.rs` (?; n=13): 12, 69, 75, 80, 100, 103, 145, 179, 185, 247, 255, 323, 330
- `workspaces/runtime/packages/runtime-services/src/claude_runtime.rs` (?; n=2): 12, 33
- `workspaces/runtime/packages/runtime-services/src/commands.rs` (?; n=4): 149, 500, 560, 600
- `workspaces/runtime/packages/runtime-services/src/context_repair.rs` (?; n=59): 17, 60, 111, 151, 205, 265, 270, 271, 275, 281, 282, 284, 356, 410, 434, 489, 500, 508, 521, 528, 578, 914, 925, 946, 959, 980, 988, 994, 1002, 1018, 1034, 1045, 1073, 1104, 1217, 1228, 1580, 1649, 1678, 1685, 1787, 1817, 2076, 2105, 2128, 2140, 2193, 2208, 2217, 2228, 2237, 2249, 2255, 2256, 2262, 2263, 2318, 2355, 2381
- `workspaces/runtime/packages/runtime-services/src/context_repair_files.rs` (?; n=51): 19, 110, 116, 127, 138, 141, 144, 147, 150, 153, 156, 159, 162, 175, 254, 277, 344, 366, 441, 443, 726, 788, 790, 809, 868, 870, 871, 872, 873, 874, 875, 880, 915, 917, 923, 924, 930, 972, 974, 1088, 1137, 1151, 1156, 1187, 1208, 1210, 1530, 1549, 1559, 1570, 1596
- `workspaces/runtime/packages/runtime-services/src/continuation.rs` (?; n=46): 5, 51, 52, 54, 59, 65, 119, 123, 131, 137, 208, 209, 210, 214, 215, 216, 254, 264, 307, 327, 347, 434, 449, 455, 465, 508, 509, 529, 542, 601, 611, 617, 627, 656, 672, 706, 707, 709, 710, 712, 715, 720, 722, 733, 760, 761
- `workspaces/runtime/packages/runtime-services/src/continuation_runtime.rs` (?; n=23): 16, 25, 33, 34, 42, 43, 53, 71, 179, 252, 284, 345, 368, 374, 386, 480, 523, 608, 820, 912, 949, 983, 1006
- `workspaces/runtime/packages/runtime-services/src/coordination.rs` (?; n=44): 5, 47, 92, 153, 186, 191, 218, 219, 222, 228, 398, 399, 400, 403, 407, 626, 627, 628, 629, 632, 636, 639, 677, 681, 683, 684, 713, 718, 728, 730, 751, 891, 895, 897, 898, 1260, 1262, 1495, 1568, 1582, 1589, 1592, 1593, 2119
- `workspaces/runtime/packages/runtime-services/src/coordination_reads.rs` (?; n=21): 8, 142, 143, 145, 174, 180, 201, 332, 392, 435, 436, 441, 442, 512, 513, 514, 517, 565, 575, 621, 625
- `workspaces/runtime/packages/runtime-services/src/costs.rs` (?; n=13): 13, 546, 568, 619, 623, 976, 2002, 2012, 2129, 2238, 2416, 2593, 3549
- `workspaces/runtime/packages/runtime-services/src/device_trust.rs` (?; n=12): 25, 156, 166, 177, 182, 183, 213, 296, 421, 429, 508, 509
- `workspaces/runtime/packages/runtime-services/src/diagnostics/host.rs` (?; n=4): 4, 66, 133, 193
- `workspaces/runtime/packages/runtime-services/src/diagnostics/http.rs` (?; n=2): 5, 108
- `workspaces/runtime/packages/runtime-services/src/diagnostics/mod.rs` (?; n=5): 219, 221, 223, 237, 279
- `workspaces/runtime/packages/runtime-services/src/diagnostics/sqlite.rs` (?; n=21): 7, 66, 67, 72, 214, 222, 232, 256, 264, 271, 441, 500, 515, 533, 554, 623, 631, 638, 660, 669, 672
- `workspaces/runtime/packages/runtime-services/src/diagnostics/stages.rs` (?; n=2): 4, 87
- `workspaces/runtime/packages/runtime-services/src/diagnostics/startup.rs` (?; n=4): 8, 26, 49, 59
- `workspaces/runtime/packages/runtime-services/src/diagnostics/store.rs` (a; n=13): 4, 11, 23, 33, 59, 66, 72, 84, 110, 169, 190, 205, 225
- `workspaces/runtime/packages/runtime-services/src/diagnostics/sync.rs` (ac; n=2): 2, 47
- `workspaces/runtime/packages/runtime-services/src/disk_space.rs` (?; n=10): 16, 56, 58, 59, 60, 64, 68, 71, 523, 862
- `workspaces/runtime/packages/runtime-services/src/edit_turn.rs` (?; n=6): 18, 35, 47, 51, 92, 112
- `workspaces/runtime/packages/runtime-services/src/entity_publication.rs` (?; n=5): 66, 82, 104, 118, 124
- `workspaces/runtime/packages/runtime-services/src/entity_publication_tests.rs` (?; n=7): 8, 56, 899, 1560, 1829, 1920, 2147
- `workspaces/runtime/packages/runtime-services/src/event_write.rs` (ad; n=28): 9, 27, 367, 431, 435, 436, 439, 459, 464, 469, 472, 474, 475, 496, 500, 501, 507, 531, 532, 552, 595, 599, 740, 764, 811, 1344, 1371, 1397
- `workspaces/runtime/packages/runtime-services/src/federation.rs` (?; n=19): 8, 28, 40, 48, 49, 150, 151, 195, 200, 256, 356, 410, 476, 493, 627, 658, 670, 672, 816
- `workspaces/runtime/packages/runtime-services/src/federation_credentials.rs` (?; n=3): 25, 175, 568
- `workspaces/runtime/packages/runtime-services/src/federation_delivery.rs` (?; n=20): 12, 206, 251, 297, 299, 301, 304, 324, 326, 353, 362, 614, 637, 650, 653, 654, 679, 731, 744, 747
- `workspaces/runtime/packages/runtime-services/src/federation_effects.rs` (?; n=32): 9, 198, 213, 250, 286, 351, 407, 480, 507, 528, 560, 573, 585, 675, 685, 691, 699, 715, 723, 740, 743, 746, 780, 856, 861, 866, 875, 880, 885, 889, 893, 925
- `workspaces/runtime/packages/runtime-services/src/federation_lead.rs` (?; n=8): 13, 33, 42, 44, 164, 391, 417, 453
- `workspaces/runtime/packages/runtime-services/src/federation_messaging.rs` (ad; n=19): 8, 34, 42, 133, 145, 150, 170, 180, 181, 182, 217, 219, 228, 232, 238, 275, 353, 354, 415
- `workspaces/runtime/packages/runtime-services/src/federation_project_tests.rs` (?; n=4): 12, 99, 101, 138
- `workspaces/runtime/packages/runtime-services/src/host_exec/tests.rs` (?; n=15): 17, 29, 61, 62, 72, 87, 115, 130, 133, 134, 237, 292, 296, 316, 345
- `workspaces/runtime/packages/runtime-services/src/host_exec.rs` (?; n=22): 15, 71, 169, 175, 182, 196, 201, 205, 206, 207, 223, 230, 249, 259, 271, 281, 406, 409, 410, 413, 434, 443
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/command.rs` (?; n=1): 605
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/lifecycle.rs` (?; n=14): 15, 57, 62, 92, 93, 114, 116, 132, 171, 175, 183, 266, 270, 303
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/mod.rs` (?; n=16): 20, 109, 110, 111, 112, 363, 368, 471, 473, 609, 621, 633, 737, 972, 996, 1045
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/protocol.rs` (?; n=12): 16, 116, 121, 122, 140, 164, 165, 205, 342, 461, 477, 479
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/slots.rs` (?; n=24): 9, 60, 62, 77, 78, 79, 80, 88, 92, 93, 238, 260, 296, 551, 588, 704, 776, 780, 793, 847, 848, 849, 864, 907
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/tests.rs` (?; n=14): 41, 121, 128, 140, 150, 156, 161, 174, 198, 309, 528, 1158, 1283, 1340
- `workspaces/runtime/packages/runtime-services/src/host_exec_service/transfer.rs` (?; n=9): 11, 207, 298, 321, 341, 427, 520, 623, 655
- `workspaces/runtime/packages/runtime-services/src/host_project_runtime.rs` (?; n=13): 5, 13, 21, 22, 23, 24, 25, 50, 51, 62, 95, 145, 155
- `workspaces/runtime/packages/runtime-services/src/host_worker_create.rs` (ac; n=39): 15, 204, 205, 216, 227, 228, 229, 269, 295, 298, 312, 322, 327, 335, 341, 342, 343, 374, 415, 416, 421, 426, 431, 436, 451, 536, 596, 599, 605, 615, 619, 620, 712, 757, 950, 967, 1031, 1038, 1050
- `workspaces/runtime/packages/runtime-services/src/idle_compaction.rs` (?; n=17): 7, 34, 35, 36, 44, 75, 78, 99, 100, 161, 236, 315, 363, 380, 421, 728, 893
- `workspaces/runtime/packages/runtime-services/src/layr_native/git_guard.rs` (?; n=1): 40
- `workspaces/runtime/packages/runtime-services/src/layr_native/import/tests.rs` (?; n=1): 647
- `workspaces/runtime/packages/runtime-services/src/layr_native/import.rs` (?; n=2): 396, 1075
- `workspaces/runtime/packages/runtime-services/src/layr_native/inventory.rs` (?; n=6): 4, 36, 49, 80, 119, 174
- `workspaces/runtime/packages/runtime-services/src/layr_native/mapping/response.rs` (?; n=1): 143
- `workspaces/runtime/packages/runtime-services/src/layr_native/mapping.rs` (?; n=4): 143, 187, 269, 297
- `workspaces/runtime/packages/runtime-services/src/layr_native/migration.rs` (?; n=9): 9, 62, 64, 156, 166, 296, 368, 410, 527
- `workspaces/runtime/packages/runtime-services/src/layr_native/runtime.rs` (?; n=61): 53, 65, 67, 69, 71, 78, 87, 93, 97, 100, 108, 224, 225, 226, 234, 235, 236, 243, 245, 246, 255, 257, 258, 266, 268, 269, 270, 282, 297, 326, 335, 417, 419, 434, 459, 499, 516, 518, 552, 558, 560, 696, 763, 820, 888, 904, 930, 952, 992, 1011, 1012, 1015, 1055, 1087, 1378, 1460, 1480, 1486, 1491, 1527, 1797
- `workspaces/runtime/packages/runtime-services/src/layr_native/settings.rs` (?; n=3): 57, 147, 174
- `workspaces/runtime/packages/runtime-services/src/layr_native/switch/tests.rs` (?; n=6): 5, 25, 29, 35, 50, 213
- `workspaces/runtime/packages/runtime-services/src/layr_native/switch.rs` (?; n=6): 15, 18, 26, 137, 155, 162
- `workspaces/runtime/packages/runtime-services/src/layr_native/warm_store_tests.rs` (a; n=4): 26, 52, 172, 206
- `workspaces/runtime/packages/runtime-services/src/layr_native.rs` (?; n=26): 3, 36, 48, 96, 106, 154, 157, 201, 207, 208, 218, 234, 246, 254, 496, 529, 648, 683, 728, 805, 946, 956, 1071, 1102, 1165, 1180
- `workspaces/runtime/packages/runtime-services/src/layr_read/tests.rs` (?; n=15): 17, 31, 43, 48, 60, 63, 64, 135, 190, 199, 224, 274, 327, 338, 355
- `workspaces/runtime/packages/runtime-services/src/layr_read.rs` (?; n=12): 9, 76, 78, 89, 118, 121, 151, 182, 184, 214, 258, 280
- `workspaces/runtime/packages/runtime-services/src/legacy_image_workspace.rs` (?; n=9): 3, 30, 77, 84, 91, 95, 132, 230, 241
- `workspaces/runtime/packages/runtime-services/src/lifecycle_effects.rs` (?; n=85): 22, 56, 73, 74, 108, 109, 208, 234, 239, 242, 243, 245, 246, 247, 256, 257, 259, 270, 271, 277, 278, 283, 284, 303, 307, 419, 580, 737, 764, 769, 776, 916, 930, 933, 945, 946, 948, 950, 996, 1025, 1026, 1033, 1082, 1083, 1089, 1092, 1120, 1121, 1134, 1149, 1157, 1158, 1161, 1166, 1171, 1187, 1199, 1203, 1211, 1217, 1264, 1270, 1296, 1298, 1299, 1302, 1303, 1389, 1393, 1428, 1437, 1469, 1497, 1498, 1502, 1503, 1504, 1505, 1510, 1518, 1562, 1568, 1604, 1605, 1709
- `workspaces/runtime/packages/runtime-services/src/limit_reset.rs` (?; n=16): 8, 77, 270, 274, 294, 311, 524, 537, 540, 590, 673, 674, 720, 868, 871, 891
- `workspaces/runtime/packages/runtime-services/src/linux_vm_guest.rs` (d; n=34): 12, 119, 122, 131, 134, 135, 198, 201, 202, 301, 304, 305, 338, 341, 342, 358, 361, 362, 369, 370, 374, 380, 427, 437, 580, 582, 583, 586, 590, 593, 604, 663, 677, 758
- `workspaces/runtime/packages/runtime-services/src/linux_vm_provider.rs` (ab; n=91): 24, 188, 197, 207, 343, 359, 410, 431, 443, 444, 445, 449, 512, 654, 771, 772, 773, 776, 812, 831, 862, 869, 873, 895, 902, 906, 933, 939, 940, 941, 942, 943, 946, 953, 992, 1096, 1122, 1212, 1283, 1593, 1596, 1617, 1618, 1644, 1645, 1653, 1668, 1672, 1677, 1681, 1685, 1693, 1697, 1706, 1708, 1714, 1719, 1723, 1752, 1753, 1794, 1805, 1808, 1809, 2582, 2583, 2617, 2620, 2621, 2644, 2647, 2648, 2772, 2804, 2838, 2841, 2842, 2871, 2957, 3101, 3104, 3105, 3430, 3431, 3432, 3436, 3449, 3452, 3453, 3463, 3503
- `workspaces/runtime/packages/runtime-services/src/linux_vm_runtime/notices.rs` (?; n=10): 8, 19, 23, 24, 28, 96, 100, 104, 111, 118
- `workspaces/runtime/packages/runtime-services/src/linux_vm_runtime/probes.rs` (?; n=34): 40, 41, 44, 45, 48, 55, 66, 67, 69, 75, 94, 95, 102, 110, 150, 167, 172, 188, 203, 207, 221, 231, 237, 250, 252, 254, 259, 285, 290, 294, 300, 329, 335, 348
- `workspaces/runtime/packages/runtime-services/src/linux_vm_runtime/tests.rs` (?; n=13): 44, 46, 98, 110, 266, 270, 287, 395, 646, 658, 679, 793, 803
- `workspaces/runtime/packages/runtime-services/src/linux_vm_runtime.rs` (?; n=80): 28, 262, 327, 495, 498, 504, 552, 561, 569, 591, 602, 605, 615, 626, 629, 633, 654, 682, 699, 707, 710, 716, 733, 751, 761, 836, 849, 857, 873, 916, 925, 927, 929, 932, 937, 941, 943, 948, 953, 955, 960, 961, 967, 982, 985, 1017, 1024, 1027, 1038, 1086, 1093, 1096, 1098, 1102, 1109, 1128, 1132, 1142, 1190, 1261, 1264, 1267, 1269, 1280, 1287, 1304, 1305, 1316, 1323, 1361, 1394, 1402, 1405, 1428, 1444, 1504, 1507, 1510, 1513, 1518
- `workspaces/runtime/packages/runtime-services/src/linux_vm_settings.rs` (?; n=11): 12, 197, 206, 208, 209, 338, 341, 342, 450, 453, 454
- `workspaces/runtime/packages/runtime-services/src/linux_vm_share/tests.rs` (?; n=4): 128, 136, 139, 140
- `workspaces/runtime/packages/runtime-services/src/linux_vm_share.rs` (?; n=18): 17, 289, 293, 300, 310, 369, 392, 401, 410, 469, 485, 492, 506, 515, 535, 625, 644, 667
- `workspaces/runtime/packages/runtime-services/src/linux_vm_update/tests.rs` (?; n=4): 17, 50, 53, 54
- `workspaces/runtime/packages/runtime-services/src/linux_vm_update.rs` (?; n=7): 18, 93, 117, 118, 152, 167, 176
- `workspaces/runtime/packages/runtime-services/src/liveness.rs` (?; n=16): 22, 30, 35, 45, 51, 82, 94, 116, 151, 267, 282, 330, 334, 396, 724, 753
- `workspaces/runtime/packages/runtime-services/src/mac_sync/round.rs` (ac; n=32): 29, 223, 703, 1158, 1163, 1180, 1245, 1251, 1266, 1279, 1295, 1297, 1298, 1299, 1300, 1301, 1302, 1337, 1375, 1454, 1468, 1489, 1598, 1634, 1734, 1913, 1948, 2000, 2091, 2105, 2108, 2730
- `workspaces/runtime/packages/runtime-services/src/mac_sync/schedule.rs` (ac; n=7): 13, 49, 88, 99, 127, 435, 448
- `workspaces/runtime/packages/runtime-services/src/mac_sync.rs` (ac; n=34): 20, 88, 109, 151, 163, 184, 189, 198, 204, 226, 227, 261, 264, 279, 356, 458, 464, 469, 503, 513, 514, 515, 516, 517, 518, 600, 601, 641, 751, 758, 778, 786, 789, 790
- `workspaces/runtime/packages/runtime-services/src/maintenance.rs` (?; n=12): 15, 113, 114, 122, 133, 143, 166, 239, 304, 314, 326, 419
- `workspaces/runtime/packages/runtime-services/src/managed_child_create.rs` (ac; n=61): 12, 42, 50, 65, 69, 307, 314, 321, 328, 335, 380, 491, 515, 526, 531, 628, 748, 788, 805, 840, 880, 892, 893, 895, 1042, 1061, 1065, 1079, 1147, 1162, 1164, 1165, 1189, 1194, 1200, 1211, 1215, 1222, 1225, 1232, 1271, 1299, 1314, 1338, 1345, 1366, 1452, 1463, 1512, 1517, 1556, 1589, 1596, 1626, 1639, 1640, 1655, 1706, 1708, 1709, 1731
- `workspaces/runtime/packages/runtime-services/src/managed_host_tools/ingress_tests/claim_settlement.rs` (?; n=5): 49, 111, 188, 199, 208
- `workspaces/runtime/packages/runtime-services/src/managed_host_tools/ingress_tests/claude_permissions.rs` (?; n=4): 7, 119, 130, 133
- `workspaces/runtime/packages/runtime-services/src/managed_host_tools/ingress_tests.rs` (?; n=51): 22, 35, 45, 64, 95, 158, 180, 201, 222, 254, 266, 277, 288, 301, 312, 325, 339, 424, 476, 477, 828, 1103, 1130, 1141, 1232, 1233, 1234, 1329, 1387, 1404, 1475, 2010, 2130, 2133, 2134, 2497, 2551, 2568, 3050, 3056, 3120, 3155, 3691, 3699, 3743, 3861, 3936, 3958, 4262, 4350, 4412
- `workspaces/runtime/packages/runtime-services/src/managed_host_tools.rs` (?; n=55): 45, 99, 143, 148, 204, 219, 233, 274, 283, 292, 301, 314, 322, 334, 361, 374, 385, 422, 487, 510, 525, 539, 564, 572, 580, 590, 603, 614, 627, 641, 660, 715, 818, 832, 868, 899, 915, 967, 970, 1071, 1074, 1079, 1082, 1098, 1100, 1101, 1102, 1103, 1104, 1105, 1141, 1190, 1200, 1201, 1202
- `workspaces/runtime/packages/runtime-services/src/managed_status_watch.rs` (?; n=34): 17, 236, 238, 295, 332, 411, 424, 433, 478, 480, 487, 488, 489, 556, 560, 561, 570, 573, 581, 692, 713, 725, 738, 744, 747, 750, 754, 762, 822, 873, 992, 1485, 1740, 1792
- `workspaces/runtime/packages/runtime-services/src/message_admission.rs` (ad; n=9): 9, 23, 80, 151, 153, 160, 217, 235, 262
- `workspaces/runtime/packages/runtime-services/src/model_catalog.rs` (?; n=15): 11, 93, 102, 148, 755, 835, 858, 983, 1016, 1017, 1023, 1026, 1029, 1055, 1414
- `workspaces/runtime/packages/runtime-services/src/monitor_driver.rs` (?; n=5): 6, 281, 292, 397, 520
- `workspaces/runtime/packages/runtime-services/src/monitor_event_queue.rs` (ad; n=2): 10, 461
- `workspaces/runtime/packages/runtime-services/src/monitor_http_input.rs` (?; n=3): 5, 117, 508
- `workspaces/runtime/packages/runtime-services/src/monitor_lifecycle.rs` (?; n=2): 6, 63
- `workspaces/runtime/packages/runtime-services/src/monitor_observation.rs` (?; n=4): 4, 84, 154, 156
- `workspaces/runtime/packages/runtime-services/src/monitor_policy.rs` (?; n=20): 6, 238, 241, 255, 262, 391, 414, 425, 434, 705, 939, 950, 961, 962, 963, 964, 967, 1164, 1280, 1282
- `workspaces/runtime/packages/runtime-services/src/monitor_runtime.rs` (?; n=17): 8, 183, 191, 199, 324, 337, 351, 360, 511, 608, 666, 673, 1019, 1062, 1064, 1154, 1613
- `workspaces/runtime/packages/runtime-services/src/monitor_runtime_wiring.rs` (?; n=9): 16, 100, 768, 1033, 1044, 1067, 1108, 1161, 3485
- `workspaces/runtime/packages/runtime-services/src/multi_server.rs` (?; n=30): 8, 39, 49, 62, 178, 181, 182, 190, 321, 323, 334, 346, 379, 403, 497, 499, 500, 501, 502, 510, 522, 649, 780, 781, 793, 794, 809, 828, 834, 936
- `workspaces/runtime/packages/runtime-services/src/multi_server_auth.rs` (?; n=34): 7, 32, 34, 35, 37, 44, 46, 48, 53, 55, 57, 59, 61, 66, 68, 70, 72, 74, 79, 81, 157, 167, 198, 199, 358, 537, 538, 585, 595, 602, 634, 812, 830, 888
- `workspaces/runtime/packages/runtime-services/src/multi_server_discovery.rs` (?; n=12): 10, 114, 130, 148, 169, 256, 314, 317, 323, 328, 329, 350
- `workspaces/runtime/packages/runtime-services/src/multi_server_orchestration.rs` (?; n=63): 9, 225, 227, 263, 329, 335, 665, 772, 799, 991, 997, 998, 1005, 1016, 1038, 1043, 1045, 1049, 1136, 1159, 1316, 1326, 1327, 1329, 1338, 1378, 1388, 1389, 1390, 1391, 1429, 1462, 1466, 1467, 1468, 1472, 1559, 1598, 1607, 1672, 2038, 2126, 2151, 2154, 2195, 2215, 2291, 2294, 2295, 2304, 2309, 2329, 2336, 2374, 2485, 2695, 2698, 4077, 4478, 4763, 4809, 4825, 4849
- `workspaces/runtime/packages/runtime-services/src/multi_server_pairing.rs` (?; n=18): 10, 44, 54, 69, 105, 110, 136, 471, 506, 571, 582, 805, 824, 825, 922, 933, 942, 946
- `workspaces/runtime/packages/runtime-services/src/multi_server_tools/tests.rs` (?; n=53): 24, 26, 29, 57, 61, 91, 93, 162, 193, 265, 290, 296, 350, 363, 403, 406, 408, 425, 429, 432, 447, 451, 453, 482, 491, 494, 818, 820, 823, 831, 832, 834, 836, 842, 843, 845, 850, 1007, 1011, 1124, 1211, 1213, 1400, 1435, 1464, 1717, 1721, 1758, 1970, 2007, 2035, 2060, 2076
- `workspaces/runtime/packages/runtime-services/src/multi_server_tools.rs` (?; n=79): 19, 62, 73, 247, 266, 267, 277, 324, 340, 341, 349, 350, 354, 357, 358, 414, 445, 449, 465, 466, 467, 469, 498, 499, 500, 515, 525, 532, 555, 558, 572, 595, 596, 607, 610, 640, 651, 692, 748, 758, 759, 782, 783, 786, 787, 790, 791, 815, 833, 870, 874, 875, 894, 976, 979, 997, 1002, 1008, 1016, 1062, 1066, 1078, 1369, 1380, 1394, 1396, 1400, 1403, 1427, 1431, 1480, 1482, 1493, 1512, 1518, 1532, 1533, 1553, 1563
- `workspaces/runtime/packages/runtime-services/src/native_action_ports.rs` (?; n=12): 8, 22, 30, 39, 42, 46, 65, 74, 76, 82, 162, 169
- `workspaces/runtime/packages/runtime-services/src/native_actions.rs` (c; n=53): 19, 111, 155, 177, 200, 255, 261, 266, 273, 321, 374, 391, 427, 442, 573, 574, 826, 838, 843, 877, 991, 993, 1157, 1191, 1215, 1227, 1386, 1445, 1543, 1550, 1557, 1564, 1595, 1596, 1600, 1614, 1615, 1616, 1617, 1618, 1619, 1620, 1621, 1639, 1657, 1678, 1681, 1716, 1764, 1767, 1789, 1815, 1886
- `workspaces/runtime/packages/runtime-services/src/native_review.rs` (c; n=34): 5, 16, 104, 105, 107, 115, 116, 117, 144, 145, 148, 155, 164, 169, 171, 172, 187, 195, 200, 201, 202, 209, 211, 215, 219, 226, 250, 252, 253, 258, 278, 311, 312, 322
- `workspaces/runtime/packages/runtime-services/src/native_review_turn.rs` (c; n=15): 33, 53, 54, 69, 78, 101, 132, 154, 219, 247, 553, 701, 727, 754, 1024
- `workspaces/runtime/packages/runtime-services/src/native_runtime/state.rs` (?; n=7): 4, 21, 26, 31, 36, 37, 43
- `workspaces/runtime/packages/runtime-services/src/native_runtime/tests.rs` (?; n=2): 5, 345
- `workspaces/runtime/packages/runtime-services/src/native_runtime.rs` (?; n=41): 24, 120, 121, 122, 272, 476, 485, 486, 489, 508, 513, 523, 527, 533, 554, 607, 609, 695, 697, 712, 857, 876, 888, 897, 1174, 1181, 1184, 1187, 1199, 1212, 1225, 1227, 1228, 1234, 1237, 1263, 1275, 1280, 1289, 1295, 1315
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/effects.rs` (abd; n=15): 6, 12, 15, 19, 46, 55, 82, 211, 213, 258, 259, 260, 272, 290, 291
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/items.rs` (abd; n=14): 5, 34, 44, 49, 51, 112, 245, 280, 282, 304, 325, 381, 395, 404
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/kinds.rs` (abd; n=16): 10, 19, 21, 76, 414, 427, 484, 506, 539, 541, 605, 607, 618, 630, 639, 664
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/notices.rs` (abd; n=31): 2, 73, 77, 82, 85, 89, 98, 143, 350, 365, 367, 386, 414, 423, 435, 439, 463, 479, 487, 491, 496, 498, 523, 527, 544, 567, 609, 612, 613, 614, 634
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/questions.rs` (abd; n=22): 6, 21, 27, 30, 31, 32, 40, 145, 163, 164, 167, 364, 367, 375, 425, 426, 458, 495, 496, 509, 510, 513
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/snapshot.rs` (abd; n=8): 3, 7, 9, 13, 15, 23, 29, 35
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/stream.rs` (abd; n=33): 16, 72, 523, 524, 525, 526, 527, 528, 553, 595, 634, 751, 872, 881, 900, 1016, 1086, 1095, 1117, 1140, 1154, 1159, 1165, 1271, 1275, 1283, 1384, 1395, 1399, 1408, 1450, 1474, 1517
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch/submission.rs` (abd; n=36): 4, 96, 238, 281, 285, 300, 324, 331, 334, 337, 343, 346, 354, 358, 384, 392, 398, 400, 406, 422, 425, 430, 441, 480, 485, 486, 487, 488, 489, 490, 496, 527, 531, 540, 547, 553
- `workspaces/runtime/packages/runtime-services/src/notification_dispatch.rs` (abd; n=151): 6, 117, 126, 177, 187, 374, 375, 378, 385, 551, 695, 699, 715, 716, 748, 751, 758, 771, 772, 943, 959, 961, 1024, 1027, 1028, 1087, 1097, 1126, 1128, 1135, 1144, 1146, 1152, 1157, 1158, 1175, 1189, 1191, 1195, 1200, 1302, 1306, 1311, 1314, 1318, 1332, 1345, 1348, 1351, 1355, 1357, 1372, 1375, 1382, 1394, 1410, 1414, 1466, 1494, 1503, 1522, 1523, 1564, 1565, 1614, 1617, 1632, 1635, 1676, 1722, 1726, 1739, 1755, 1762, 1775, 1795, 1803, 1806, 1812, 1822, 1869, 1897, 1901, 1914, 1921, 1952, 1969, 1970, 1995, 2029, 2047, 2049, 2055, 2057, 2058, 2069, 2074, 2078, 2087, 2088, 2090, 2091, 2098, 2110, 2111, 2112, 2130, 2131, 2132, 2149, 2152, 2154, 2155, 2189, 2246, 2413, 2444, 2494, 2519, 2693, 2716, 2792, 3002, 3011, 3034, 3038, 3072, 3083, 3129, 3133, 3143, 3198, 3210, 3426, 3433, 3436, 3437, 3503, 3706, 3894, 3898, 3899, 4259, 4348, 4414, 4527, 4545, 4664, 4681, 4756, 4782
- `workspaces/runtime/packages/runtime-services/src/notification_runtime_effects.rs` (ad; n=9): 9, 18, 19, 34, 47, 48, 49, 284, 292
- `workspaces/runtime/packages/runtime-services/src/orchestration_exec.rs` (?; n=11): 18, 22, 25, 28, 181, 201, 205, 209, 213, 218, 223
- `workspaces/runtime/packages/runtime-services/src/orchestration_wire.rs` (?; n=16): 9, 35, 82, 84, 118, 120, 122, 124, 126, 135, 144, 145, 146, 249, 251, 324
- `workspaces/runtime/packages/runtime-services/src/organization_effects.rs` (?; n=21): 8, 54, 58, 92, 100, 104, 145, 165, 168, 186, 200, 220, 224, 225, 226, 245, 250, 273, 298, 308, 349
- `workspaces/runtime/packages/runtime-services/src/outbox_resolve.rs` (?; n=5): 28, 36, 44, 101, 255
- `workspaces/runtime/packages/runtime-services/src/payload_blobs.rs` (?; n=59): 11, 33, 35, 36, 37, 38, 39, 40, 49, 51, 52, 53, 57, 59, 60, 115, 121, 125, 129, 133, 137, 141, 150, 154, 158, 169, 207, 212, 223, 239, 247, 324, 340, 345, 351, 372, 378, 386, 403, 413, 415, 464, 533, 541, 542, 543, 549, 554, 557, 562, 565, 566, 567, 570, 623, 701, 784, 941, 942
- `workspaces/runtime/packages/runtime-services/src/payload_store.rs` (a; n=4): 11, 52, 68, 365
- `workspaces/runtime/packages/runtime-services/src/peer_backup/tests.rs` (?; n=3): 13, 17, 33
- `workspaces/runtime/packages/runtime-services/src/peer_backup.rs` (?; n=4): 21, 147, 516, 533
- `workspaces/runtime/packages/runtime-services/src/peer_conversion.rs` (?; n=6): 12, 112, 168, 169, 210, 236
- `workspaces/runtime/packages/runtime-services/src/peer_projects.rs` (?; n=8): 411, 427, 467, 502, 505, 573, 578, 592
- `workspaces/runtime/packages/runtime-services/src/peer_update_pull.rs` (?; n=29): 18, 132, 330, 350, 373, 383, 575, 749, 839, 878, 911, 977, 988, 989, 1004, 1009, 1013, 1021, 1025, 1034, 1037, 1040, 1086, 1100, 1110, 1114, 1272, 1366, 1374
- `workspaces/runtime/packages/runtime-services/src/peer_updates.rs` (?; n=33): 20, 139, 140, 164, 165, 196, 202, 257, 262, 305, 307, 317, 434, 436, 447, 448, 465, 466, 480, 493, 510, 588, 600, 603, 607, 608, 609, 631, 657, 662, 720, 728, 729
- `workspaces/runtime/packages/runtime-services/src/portable_history.rs` (?; n=90): 11, 103, 107, 127, 133, 136, 144, 220, 229, 232, 235, 236, 239, 240, 251, 253, 477, 486, 490, 496, 506, 847, 851, 855, 858, 869, 891, 956, 958, 964, 972, 974, 975, 976, 998, 1002, 1023, 1069, 1084, 1136, 1142, 1147, 1157, 1161, 1174, 1185, 1186, 1190, 1193, 1203, 1208, 1209, 1221, 1231, 1240, 1244, 1254, 1257, 1266, 1270, 1271, 1274, 1278, 1282, 1286, 1290, 1298, 1303, 1304, 1308, 1309, 1313, 1315, 1319, 1321, 1327, 1330, 1503, 1520, 1529, 1541, 1545, 1546, 1571, 1585, 1764, 1776, 1788, 1800, 1824
- `workspaces/runtime/packages/runtime-services/src/pricing.rs` (?; n=3): 8, 113, 358
- `workspaces/runtime/packages/runtime-services/src/principal.rs` (?; n=2): 28, 29
- `workspaces/runtime/packages/runtime-services/src/process_controls.rs` (?; n=26): 6, 189, 219, 223, 231, 239, 273, 291, 319, 452, 468, 476, 512, 513, 564, 570, 580, 793, 847, 854, 865, 881, 882, 958, 965, 1455
- `workspaces/runtime/packages/runtime-services/src/project_deletion.rs` (?; n=42): 13, 122, 149, 185, 190, 201, 204, 207, 218, 225, 232, 236, 251, 254, 257, 267, 270, 275, 281, 284, 289, 299, 302, 344, 348, 352, 360, 379, 389, 394, 402, 406, 423, 441, 446, 451, 459, 462, 469, 580, 583, 589
- `workspaces/runtime/packages/runtime-services/src/project_locations.rs` (?; n=26): 7, 57, 66, 124, 155, 156, 181, 187, 227, 242, 368, 371, 375, 465, 488, 546, 561, 573, 592, 608, 618, 805, 873, 923, 1039, 1041
- `workspaces/runtime/packages/runtime-services/src/project_path.rs` (?; n=4): 4, 41, 44, 45
- `workspaces/runtime/packages/runtime-services/src/project_projection.rs` (?; n=12): 69, 74, 136, 148, 158, 166, 174, 178, 182, 196, 246, 250
- `workspaces/runtime/packages/runtime-services/src/project_secrets.rs` (?; n=17): 5, 17, 20, 28, 31, 104, 122, 150, 165, 167, 170, 171, 185, 187, 192, 200, 203
- `workspaces/runtime/packages/runtime-services/src/projects/operations.rs` (?; n=36): 11, 60, 71, 73, 90, 91, 95, 97, 101, 102, 111, 118, 120, 124, 138, 148, 149, 150, 151, 152, 155, 160, 161, 164, 171, 175, 179, 183, 201, 205, 206, 207, 250, 280, 283, 301
- `workspaces/runtime/packages/runtime-services/src/projects/radio_lifecycle.rs` (?; n=10): 57, 262, 270, 287, 429, 431, 729, 809, 825, 834
- `workspaces/runtime/packages/runtime-services/src/projects/radio_state.rs` (?; n=19): 36, 88, 167, 173, 181, 182, 186, 187, 188, 192, 193, 198, 199, 203, 215, 324, 371, 407, 417
- `workspaces/runtime/packages/runtime-services/src/projects.rs` (?; n=57): 9, 71, 377, 397, 398, 404, 560, 627, 650, 654, 666, 686, 698, 703, 719, 721, 752, 756, 765, 767, 797, 819, 829, 838, 854, 900, 905, 907, 908, 909, 940, 960, 969, 974, 1008, 1011, 1024, 1032, 1035, 1053, 1107, 1127, 1142, 1180, 1227, 1228, 1229, 1232, 1233, 1236, 1246, 1250, 1253, 1257, 1265, 1269, 1428
- `workspaces/runtime/packages/runtime-services/src/provider_adapter.rs` (ab; n=39): 20, 90, 98, 103, 110, 113, 128, 131, 160, 167, 168, 174, 175, 180, 191, 192, 207, 260, 296, 298, 307, 311, 366, 371, 423, 436, 451, 470, 482, 488, 493, 550, 566, 572, 740, 911, 946, 949, 968
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/cards.rs` (ab; n=41): 8, 78, 83, 84, 108, 305, 306, 318, 319, 320, 327, 331, 333, 342, 343, 349, 356, 361, 370, 371, 380, 381, 386, 403, 410, 421, 426, 582, 593, 594, 596, 633, 635, 640, 665, 704, 737, 1000, 1069, 1085, 1137
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/legacy_view.rs` (ab; n=13): 5, 13, 41, 50, 57, 58, 83, 90, 143, 167, 180, 186, 216
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/model.rs` (ab; n=7): 2, 106, 117, 129, 133, 143, 265
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/projection.rs` (ab; n=9): 4, 39, 54, 56, 57, 89, 123, 136, 158
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/recovery.rs` (ab; n=13): 5, 16, 17, 18, 34, 57, 87, 152, 182, 210, 224, 327, 330
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/storage.rs` (ab; n=5): 6, 35, 36, 173, 860
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/runtime_dispatch.rs` (ab; n=28): 8, 209, 211, 217, 229, 238, 249, 252, 270, 288, 295, 310, 317, 323, 326, 341, 343, 408, 439, 505, 596, 631, 820, 953, 958, 1153, 1160, 1496
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/runtime_ports.rs` (ab; n=12): 6, 192, 214, 243, 246, 405, 431, 452, 454, 472, 508, 535
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/context.rs` (ab; n=2): 14, 18
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/manage.rs` (ab; n=24): 40, 47, 59, 71, 72, 73, 90, 102, 126, 128, 176, 191, 193, 220, 236, 281, 302, 303, 305, 364, 463, 471, 488, 489
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/messages.rs` (abd; n=22): 122, 254, 380, 383, 404, 408, 418, 424, 573, 680, 682, 792, 797, 900, 901, 922, 1002, 1064, 1147, 1148, 1155, 1156
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/reads.rs` (ab; n=28): 13, 14, 30, 36, 41, 63, 64, 67, 115, 118, 209, 211, 228, 242, 288, 299, 300, 341, 342, 439, 440, 463, 498, 499, 500, 501, 529, 554
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/sends_tests.rs` (ab; n=27): 24, 87, 158, 161, 167, 439, 446, 457, 465, 469, 481, 485, 492, 493, 497, 742, 959, 971, 1004, 1025, 1069, 1225, 1240, 1267, 1395, 1458, 1572
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services/work.rs` (ab; n=6): 6, 8, 56, 65, 74, 78
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools/services.rs` (ab; n=33): 59, 266, 287, 342, 354, 384, 433, 496, 508, 567, 584, 627, 629, 638, 651, 865, 1020, 1222, 1227, 1229, 1236, 1259, 1260, 1261, 1263, 1269, 1270, 1271, 1272, 1274, 1330, 1380, 1407
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks/tools.rs` (ab; n=21): 6, 11, 150, 166, 179, 180, 252, 280, 368, 404, 438, 439, 585, 594, 611, 756, 845, 849, 937, 940, 1069
- `workspaces/runtime/packages/runtime-services/src/provider_callbacks.rs` (ab; n=3): 39, 290, 291
- `workspaces/runtime/packages/runtime-services/src/provider_pagination.rs` (ab; n=8): 38, 52, 53, 56, 57, 58, 59, 60
- `workspaces/runtime/packages/runtime-services/src/provider_session_task/tests.rs` (ab; n=1): 53
- `workspaces/runtime/packages/runtime-services/src/provider_session_task.rs` (ab; n=5): 11, 43, 69, 179, 221
- `workspaces/runtime/packages/runtime-services/src/provider_versions/tests.rs` (ab; n=7): 52, 81, 112, 390, 469, 553, 803
- `workspaces/runtime/packages/runtime-services/src/provider_versions.rs` (ab; n=42): 30, 61, 65, 229, 244, 248, 252, 278, 291, 299, 300, 304, 305, 306, 315, 321, 328, 697, 777, 984, 1116, 1132, 1141, 1160, 1242, 1243, 1246, 1249, 1252, 1257, 1288, 1290, 1291, 1295, 1296, 1297, 1299, 1300, 1301, 1302, 1312, 1321
- `workspaces/runtime/packages/runtime-services/src/questions_reads.rs` (?; n=5): 5, 21, 30, 53, 59
- `workspaces/runtime/packages/runtime-services/src/questions_write.rs` (?; n=15): 5, 76, 82, 83, 85, 88, 90, 105, 111, 120, 318, 345, 380, 398, 466
- `workspaces/runtime/packages/runtime-services/src/queue_write.rs` (?; n=16): 6, 51, 145, 153, 181, 189, 212, 214, 266, 267, 271, 277, 288, 293, 405, 428
- `workspaces/runtime/packages/runtime-services/src/radio.rs` (?; n=28): 13, 41, 42, 50, 56, 63, 68, 69, 75, 81, 82, 88, 93, 94, 103, 107, 274, 275, 276, 290, 291, 292, 297, 298, 302, 303, 304, 657
- `workspaces/runtime/packages/runtime-services/src/radio_runtime.rs` (?; n=13): 19, 44, 179, 183, 184, 189, 312, 509, 510, 512, 597, 598, 605
- `workspaces/runtime/packages/runtime-services/src/recovery.rs` (?; n=22): 9, 30, 33, 39, 42, 47, 48, 55, 56, 57, 61, 66, 71, 189, 227, 389, 529, 550, 553, 558, 569, 580
- `workspaces/runtime/packages/runtime-services/src/recovery_absence.rs` (?; n=3): 6, 127, 133
- `workspaces/runtime/packages/runtime-services/src/recovery_history.rs` (?; n=25): 5, 55, 59, 71, 87, 126, 172, 173, 196, 199, 217, 223, 241, 300, 337, 340, 393, 466, 486, 491, 493, 515, 535, 536, 537
- `workspaces/runtime/packages/runtime-services/src/recovery_policy.rs` (?; n=34): 5, 15, 33, 39, 76, 86, 97, 212, 216, 228, 229, 233, 234, 235, 236, 237, 241, 259, 262, 265, 268, 271, 275, 288, 299, 322, 408, 481, 742, 774, 854, 860, 1007, 1011
- `workspaces/runtime/packages/runtime-services/src/recovery_runtime.rs` (?; n=5): 23, 35, 99, 273, 300
- `workspaces/runtime/packages/runtime-services/src/recovery_store.rs` (a; n=36): 14, 28, 107, 116, 210, 211, 225, 304, 331, 382, 409, 416, 417, 426, 429, 430, 449, 527, 698, 709, 730, 731, 753, 805, 883, 894, 903, 925, 926, 984, 990, 1000, 1010, 1011, 1035, 1057
- `workspaces/runtime/packages/runtime-services/src/relocation.rs` (?; n=3): 3, 39, 48
- `workspaces/runtime/packages/runtime-services/src/remote_access.rs` (?; n=3): 11, 50, 51
- `workspaces/runtime/packages/runtime-services/src/remote_transport.rs` (?; n=38): 17, 457, 466, 471, 497, 521, 523, 524, 528, 529, 606, 616, 620, 624, 633, 640, 649, 666, 754, 771, 775, 885, 936, 953, 957, 1039, 1449, 1452, 1463, 1468, 1475, 1479, 1755, 2652, 2793, 2814, 2878, 2897
- `workspaces/runtime/packages/runtime-services/src/resource_notify.rs` (?; n=4): 13, 80, 135, 634
- `workspaces/runtime/packages/runtime-services/src/room_projection.rs` (?; n=23): 6, 104, 153, 154, 159, 190, 198, 199, 200, 206, 247, 248, 269, 326, 332, 359, 364, 387, 400, 421, 452, 459, 460
- `workspaces/runtime/packages/runtime-services/src/rules.rs` (?; n=2): 100, 121
- `workspaces/runtime/packages/runtime-services/src/rules_engine.rs` (?; n=18): 4, 108, 110, 159, 213, 220, 271, 791, 801, 805, 844, 881, 883, 896, 915, 930, 945, 972
- `workspaces/runtime/packages/runtime-services/src/safety_buffering/executor.rs` (?; n=48): 24, 61, 67, 69, 126, 153, 187, 208, 213, 236, 239, 273, 275, 277, 305, 323, 325, 336, 396, 406, 621, 695, 758, 771, 782, 788, 790, 793, 796, 797, 803, 816, 817, 818, 820, 833, 939, 944, 1121, 1199, 1201, 1238, 1365, 1394, 1436, 1441, 1485, 1486
- `workspaces/runtime/packages/runtime-services/src/safety_buffering.rs` (?; n=71): 13, 17, 32, 95, 102, 109, 113, 116, 125, 131, 137, 141, 142, 146, 148, 150, 151, 156, 179, 180, 181, 189, 203, 204, 205, 206, 207, 212, 221, 223, 226, 253, 254, 257, 261, 264, 267, 269, 270, 273, 275, 279, 284, 298, 301, 302, 303, 308, 309, 311, 322, 326, 330, 333, 340, 342, 348, 349, 350, 351, 355, 381, 383, 384, 386, 389, 408, 413, 427, 615, 675
- `workspaces/runtime/packages/runtime-services/src/saved_mount.rs` (?; n=9): 3, 6, 9, 33, 39, 41, 43, 46, 55
- `workspaces/runtime/packages/runtime-services/src/scheduler.rs` (?; n=46): 3, 22, 64, 71, 82, 83, 88, 97, 102, 107, 110, 111, 112, 113, 114, 260, 264, 265, 266, 272, 364, 370, 406, 407, 408, 411, 427, 428, 438, 487, 517, 530, 599, 659, 660, 661, 967, 1060, 1073, 1157, 1335, 1389, 1422, 1453, 1482, 1519
- `workspaces/runtime/packages/runtime-services/src/server_exec/tests.rs` (?; n=5): 25, 29, 101, 291, 362
- `workspaces/runtime/packages/runtime-services/src/server_exec.rs` (?; n=20): 20, 95, 142, 155, 159, 168, 171, 177, 312, 376, 385, 419, 422, 438, 502, 516, 595, 744, 801, 1084
- `workspaces/runtime/packages/runtime-services/src/server_git.rs` (?; n=3): 18, 852, 915
- `workspaces/runtime/packages/runtime-services/src/shell_command.rs` (?; n=3): 8, 13, 115
- `workspaces/runtime/packages/runtime-services/src/shell_policy.rs` (?; n=18): 14, 46, 47, 48, 49, 50, 56, 59, 62, 78, 112, 259, 260, 398, 430, 615, 744, 761
- `workspaces/runtime/packages/runtime-services/src/sign_in.rs` (?; n=24): 20, 35, 36, 37, 55, 91, 94, 95, 105, 122, 135, 136, 143, 147, 157, 160, 211, 231, 232, 238, 245, 248, 331, 334
- `workspaces/runtime/packages/runtime-services/src/studio_errors/causes.rs` (?; n=1): 265
- `workspaces/runtime/packages/runtime-services/src/studio_errors.rs` (?; n=14): 17, 303, 688, 700, 709, 710, 711, 712, 855, 983, 984, 992, 1141, 1171
- `workspaces/runtime/packages/runtime-services/src/submit_gates.rs` (?; n=36): 9, 342, 344, 346, 355, 356, 362, 371, 376, 431, 434, 437, 440, 517, 518, 524, 526, 527, 581, 681, 691, 697, 703, 738, 753, 764, 848, 851, 898, 900, 908, 914, 1042, 1055, 1056, 1125
- `workspaces/runtime/packages/runtime-services/src/sync_documents.rs` (ac; n=13): 3, 10, 12, 52, 138, 139, 144, 156, 157, 172, 173, 181, 183
- `workspaces/runtime/packages/runtime-services/src/sync_entity_context.rs` (ac; n=27): 6, 21, 22, 26, 31, 38, 41, 42, 109, 120, 224, 229, 233, 238, 245, 266, 302, 305, 309, 338, 341, 349, 380, 657, 664, 896, 972
- `workspaces/runtime/packages/runtime-services/src/sync_policy.rs` (ac; n=149): 13, 141, 142, 144, 174, 192, 254, 257, 279, 289, 307, 325, 345, 405, 421, 514, 520, 527, 616, 617, 619, 620, 632, 646, 688, 693, 694, 710, 718, 725, 750, 776, 777, 792, 917, 1128, 1131, 1169, 1171, 1199, 1244, 1272, 1273, 1282, 1283, 1326, 1343, 1345, 1486, 1534, 1561, 1562, 1566, 1572, 1578, 1584, 1589, 1593, 1602, 1604, 1717, 1720, 1722, 1743, 1758, 1834, 1836, 1844, 1863, 1864, 2017, 2048, 2153, 2155, 2189, 2310, 2327, 2330, 2331, 2332, 2333, 2354, 2362, 2404, 2444, 2601, 2603, 2616, 2617, 2619, 2625, 2643, 2655, 2728, 2752, 2756, 2760, 2763, 2767, 2781, 2808, 2809, 2815, 2816, 2824, 2827, 2828, 2834, 2841, 2869, 2870, 2872, 2998, 3017, 3031, 3036, 3136, 3142, 3292, 3490, 3676, 3717, 3838, 3951, 3996, 4043, 4045, 4214, 4220, 4250, 4256, 4259, 4278, 4283, 4293, 4301, 4319, 4382, 4393, 4555, 4627, 4647, 4667, 4678, 4727, 4753, 4779, 4792, 4818
- `workspaces/runtime/packages/runtime-services/src/sync_views.rs` (ac; n=108): 5, 28, 30, 34, 35, 45, 50, 55, 58, 59, 80, 81, 94, 97, 104, 105, 106, 113, 117, 125, 131, 132, 142, 183, 185, 186, 188, 195, 196, 198, 202, 204, 208, 209, 211, 212, 214, 222, 234, 261, 262, 268, 286, 372, 383, 396, 406, 411, 412, 415, 444, 447, 453, 466, 467, 491, 501, 514, 516, 535, 536, 584, 587, 588, 602, 682, 683, 714, 720, 722, 723, 724, 753, 815, 818, 820, 821, 830, 848, 852, 857, 861, 873, 878, 881, 882, 886, 889, 892, 894, 912, 990, 1042, 1074, 1109, 1259, 1290, 1296, 1300, 1303, 1333, 1379, 1395, 1396, 1409, 1436, 1524, 1529
- `workspaces/runtime/packages/runtime-services/src/system_reads.rs` (?; n=11): 8, 146, 172, 218, 228, 366, 376, 383, 389, 395, 397
- `workspaces/runtime/packages/runtime-services/src/task_gate.rs` (?; n=13): 8, 20, 32, 107, 112, 188, 189, 358, 391, 425, 448, 464, 590
- `workspaces/runtime/packages/runtime-services/src/task_output.rs` (?; n=2): 6, 154
- `workspaces/runtime/packages/runtime-services/src/task_recovery.rs` (?; n=16): 5, 15, 21, 25, 28, 37, 40, 84, 112, 120, 121, 223, 224, 229, 244, 395
- `workspaces/runtime/packages/runtime-services/src/task_recovery_store.rs` (a; n=21): 12, 20, 21, 26, 38, 39, 46, 75, 88, 160, 188, 222, 224, 229, 240, 241, 262, 280, 299, 325, 344
- `workspaces/runtime/packages/runtime-services/src/task_settlement.rs` (?; n=17): 19, 40, 53, 57, 76, 87, 199, 248, 263, 287, 305, 355, 411, 425, 437, 438, 490
- `workspaces/runtime/packages/runtime-services/src/terminal_driver.rs` (?; n=65): 12, 36, 37, 41, 56, 118, 126, 142, 159, 163, 170, 180, 190, 226, 297, 301, 306, 311, 345, 504, 661, 826, 959, 1045, 1104, 1197, 1222, 1246, 1252, 1261, 1262, 1279, 1300, 1354, 1384, 2320, 2395, 2611, 2634, 2638, 2838, 2840, 2844, 2850, 2855, 2856, 2861, 2865, 2866, 2875, 2877, 2878, 2879, 2880, 2881, 2882, 2889, 2891, 2909, 2911, 2914, 2985, 3077, 3094, 3182
- `workspaces/runtime/packages/runtime-services/src/terminal_pty.rs` (?; n=13): 13, 75, 79, 90, 94, 312, 313, 320, 386, 506, 509, 513, 516
- `workspaces/runtime/packages/runtime-services/src/text_blob_sweep.rs` (?; n=2): 9, 60
- `workspaces/runtime/packages/runtime-services/src/thread_fork_runtime.rs` (?; n=13): 13, 23, 36, 60, 83, 99, 112, 123, 148, 161, 164, 239, 255
- `workspaces/runtime/packages/runtime-services/src/thread_prepare.rs` (?; n=49): 25, 68, 92, 97, 153, 279, 280, 281, 285, 288, 301, 319, 322, 432, 433, 439, 494, 500, 505, 658, 785, 794, 839, 920, 990, 1000, 1007, 1011, 1031, 1034, 1051, 1059, 1069, 1077, 1085, 1086, 1087, 1088, 1089, 1097, 1105, 1106, 1112, 1114, 1123, 1124, 1129, 1136, 1147
- `workspaces/runtime/packages/runtime-services/src/title_generation.rs` (?; n=49): 15, 179, 188, 189, 330, 331, 335, 338, 460, 481, 488, 513, 535, 536, 537, 541, 553, 557, 560, 565, 574, 578, 583, 586, 589, 591, 592, 837, 838, 883, 884, 893, 897, 898, 904, 909, 925, 944, 967, 982, 985, 988, 994, 999, 1002, 1045, 1046, 1048, 1087
- `workspaces/runtime/packages/runtime-services/src/token_rate.rs` (?; n=9): 247, 278, 398, 720, 1016, 1022, 1171, 1175, 1697
- `workspaces/runtime/packages/runtime-services/src/tool_requests/admission.rs` (?; n=6): 4, 10, 14, 106, 134, 162
- `workspaces/runtime/packages/runtime-services/src/tool_requests/lifecycle.rs` (?; n=11): 3, 8, 45, 57, 91, 108, 128, 153, 154, 168, 247
- `workspaces/runtime/packages/runtime-services/src/tool_requests/prepared.rs` (?; n=13): 18, 19, 20, 53, 124, 128, 157, 164, 178, 197, 198, 199, 206
- `workspaces/runtime/packages/runtime-services/src/tool_requests.rs` (?; n=44): 10, 51, 59, 63, 67, 77, 83, 114, 143, 155, 165, 237, 255, 275, 280, 289, 305, 373, 374, 464, 467, 478, 490, 503, 508, 519, 523, 530, 534, 570, 571, 606, 607, 614, 621, 699, 706, 710, 724, 730, 733, 740, 741, 743
- `workspaces/runtime/packages/runtime-services/src/transcript.rs` (?; n=55): 7, 156, 252, 255, 266, 281, 306, 307, 331, 365, 366, 371, 381, 386, 431, 471, 493, 531, 535, 537, 552, 560, 595, 608, 632, 675, 688, 713, 714, 715, 716, 733, 764, 777, 799, 800, 801, 830, 854, 931, 942, 1089, 1092, 1111, 1112, 1113, 1115, 1116, 1117, 1127, 1154, 1214, 1261, 1375, 1459
- `workspaces/runtime/packages/runtime-services/src/transcript_runtime.rs` (?; n=30): 6, 27, 37, 52, 91, 114, 178, 216, 326, 330, 337, 362, 468, 469, 477, 478, 490, 536, 546, 547, 553, 554, 577, 579, 589, 592, 644, 646, 648, 717
- `workspaces/runtime/packages/runtime-services/src/turn_prepare.rs` (?; n=40): 16, 174, 193, 223, 266, 276, 300, 345, 543, 549, 555, 562, 580, 587, 589, 600, 604, 627, 638, 673, 695, 714, 719, 734, 819, 858, 892, 911, 917, 948, 950, 951, 952, 953, 954, 957, 960, 963, 1177, 1621
- `workspaces/runtime/packages/runtime-services/src/usage.rs` (?; n=88): 11, 63, 89, 91, 95, 96, 100, 105, 107, 110, 112, 115, 116, 117, 120, 122, 225, 249, 319, 334, 358, 361, 362, 383, 398, 402, 421, 425, 437, 440, 500, 511, 548, 551, 559, 566, 578, 601, 663, 665, 668, 670, 712, 723, 769, 775, 777, 900, 901, 986, 1005, 1027, 1077, 1088, 1093, 1095, 1120, 1140, 1150, 1159, 1170, 1177, 1190, 1221, 1222, 1225, 1226, 1233, 1240, 1423, 1424, 1466, 1467, 1512, 1513, 1523, 1524, 1546, 1619, 1755, 1756, 1758, 1759, 1789, 1790, 1917, 1998, 2063
- `workspaces/runtime/packages/runtime-services/src/usage_resume.rs` (?; n=3): 7, 480, 885
- `workspaces/runtime/packages/runtime-services/src/vault.rs` (?; n=48): 10, 350, 364, 369, 376, 379, 382, 384, 392, 395, 396, 399, 400, 402, 403, 405, 417, 419, 421, 445, 453, 460, 482, 483, 488, 492, 494, 497, 516, 524, 550, 558, 630, 631, 632, 729, 732, 744, 747, 756, 774, 941, 979, 985, 988, 989, 1031, 1081
- `workspaces/runtime/packages/runtime-services/src/vm_attachments.rs` (?; n=17): 6, 88, 110, 111, 139, 169, 171, 216, 434, 535, 537, 544, 578, 581, 649, 652, 653
- `workspaces/runtime/packages/runtime-services/src/vm_preparation.rs` (?; n=48): 20, 707, 710, 739, 742, 747, 765, 776, 784, 785, 789, 793, 813, 818, 823, 824, 849, 858, 864, 865, 870, 895, 943, 952, 961, 962, 967, 970, 979, 981, 993, 1110, 1188, 1245, 1248, 1291, 1293, 1299, 1332, 1368, 1369, 1374, 1435, 1496, 1682, 1685, 1686, 1726
- `workspaces/runtime/packages/runtime-services/src/vm_to_host/tests.rs` (?; n=8): 34, 37, 38, 47, 119, 211, 290, 354
- `workspaces/runtime/packages/runtime-services/src/vm_to_host.rs` (?; n=11): 23, 179, 293, 302, 318, 334, 335, 364, 392, 439, 552
- `workspaces/runtime/packages/runtime-services/src/voice/account_session.rs` (?; n=3): 9, 118, 142
- `workspaces/runtime/packages/runtime-services/src/voice/audio.rs` (?; n=2): 9, 41
- `workspaces/runtime/packages/runtime-services/src/voice/effects.rs` (?; n=2): 4, 521
- `workspaces/runtime/packages/runtime-services/src/voice/provider.rs` (ab; n=54): 19, 164, 259, 264, 275, 284, 287, 297, 315, 319, 357, 515, 556, 593, 617, 621, 623, 625, 641, 651, 672, 750, 798, 815, 877, 963, 1071, 1073, 1209, 1404, 1409, 1420, 1443, 1448, 1457, 1465, 1467, 1468, 1478, 1487, 1494, 1519, 1547, 1602, 1603, 1618, 1647, 1661, 1997, 2010, 2219, 2678, 2724, 2774
- `workspaces/runtime/packages/runtime-services/src/voice.rs` (?; n=25): 5, 92, 96, 106, 147, 162, 206, 309, 330, 350, 387, 488, 512, 570, 574, 578, 584, 608, 648, 666, 685, 688, 693, 704, 708
- `workspaces/runtime/packages/runtime-services/src/voice_runtime.rs` (?; n=3): 8, 74, 103
- `workspaces/runtime/packages/runtime-services/src/wakeups.rs` (?; n=3): 3, 6, 19
- `workspaces/runtime/packages/runtime-services/src/web_push.rs` (?; n=2): 19, 666
- `workspaces/runtime/packages/runtime-services/src/web_push_tests.rs` (?; n=3): 173, 175, 345
- `workspaces/runtime/packages/runtime-services/src/work_assignment_runtime.rs` (?; n=4): 3, 89, 90, 92
- `workspaces/runtime/packages/runtime-services/src/work_policy.rs` (?; n=9): 5, 25, 47, 84, 119, 120, 144, 149, 200
- `workspaces/runtime/packages/runtime-services/src/work_scope.rs` (?; n=3): 11, 276, 386
- `workspaces/runtime/packages/runtime-services/src/work_write.rs` (ac; n=100): 23, 132, 133, 294, 299, 305, 568, 574, 579, 607, 609, 612, 615, 618, 619, 640, 645, 681, 689, 709, 718, 794, 850, 899, 904, 948, 1040, 1058, 1078, 1473, 1478, 1572, 1574, 1575, 1616, 1689, 1695, 1731, 1747, 1777, 2159, 2392, 2427, 2432, 2436, 2439, 2441, 2442, 2446, 2450, 2608, 2685, 2715, 2768, 2771, 2873, 2874, 2897, 2944, 3211, 3237, 3242, 3250, 3251, 3254, 3255, 3256, 3288, 3289, 3293, 3314, 3317, 3344, 3348, 3428, 3459, 3591, 3603, 3608, 3689, 3706, 3746, 3984, 4209, 4310, 4329, 4476, 4513, 4670, 4684, 4749, 4761, 4801, 5108, 5943, 5963, 6024, 6033, 6036, 6045
- `workspaces/runtime/packages/runtime-services/src/workspace_cleanup_runtime.rs` (?; n=95): 12, 21, 44, 56, 57, 88, 89, 101, 107, 127, 130, 159, 186, 190, 216, 231, 244, 262, 277, 280, 281, 282, 289, 324, 329, 341, 386, 397, 410, 428, 432, 436, 456, 463, 511, 519, 537, 543, 546, 552, 554, 562, 569, 585, 606, 684, 686, 823, 831, 852, 853, 868, 870, 872, 892, 913, 929, 942, 967, 1006, 1024, 1057, 1085, 1095, 1097, 1115, 1116, 1153, 1156, 1184, 1242, 1245, 1247, 1248, 1249, 1258, 1358, 1378, 1379, 1392, 1413, 1458, 1474, 1484, 1489, 1500, 1503, 1518, 1546, 1669, 1776, 1779, 1946, 2554, 2736
- `workspaces/runtime/packages/runtime-services/src/workspace_files.rs` (?; n=6): 8, 57, 59, 64, 68, 69
- `workspaces/runtime/packages/runtime-services/src/workspace_operations.rs` (?; n=20): 5, 28, 33, 59, 70, 73, 109, 113, 137, 154, 158, 160, 166, 170, 199, 237, 243, 278, 279, 286
- `workspaces/runtime/packages/runtime-services/src/workspace_operations_effects.rs` (?; n=8): 7, 28, 151, 152, 170, 174, 177, 197
- `workspaces/runtime/packages/runtime-services/src/workspace_ready.rs` (?; n=3): 4, 17, 166
- `workspaces/runtime/packages/runtime-services/src/workspace_ready_git.rs` (?; n=11): 25, 204, 207, 211, 220, 391, 431, 559, 571, 640, 898
- `workspaces/runtime/packages/runtime-services/src/workspace_views.rs` (?; n=25): 6, 33, 34, 35, 36, 46, 61, 67, 91, 98, 113, 119, 135, 191, 399, 400, 508, 509, 511, 559, 564, 583, 596, 620, 660
- `workspaces/runtime/packages/runtime-services/tests/account_login_registry_cleanup.rs` (?; n=3): 4, 16, 22
- `workspaces/runtime/packages/runtime-services/tests/account_login_session_start.rs` (?; n=2): 10, 246
- `workspaces/runtime/packages/runtime-services/tests/account_transfer_corpus.rs` (?; n=11): 2, 5, 12, 16, 29, 34, 46, 99, 174, 219, 230
- `workspaces/runtime/packages/runtime-services/tests/account_transfer_driver.rs` (?; n=6): 2, 16, 17, 76, 84, 92
- `workspaces/runtime/packages/runtime-services/tests/account_transfer_progress.rs` (?; n=8): 4, 41, 51, 57, 105, 139, 364, 636
- `workspaces/runtime/packages/runtime-services/tests/agent_management_corpus.rs` (ac; n=13): 2, 14, 22, 36, 40, 44, 51, 158, 161, 186, 220, 501, 503
- `workspaces/runtime/packages/runtime-services/tests/agent_management_store.rs` (ac; n=9): 5, 19, 27, 40, 89, 104, 159, 371, 373
- `workspaces/runtime/packages/runtime-services/tests/analytics_capture/common.rs` (?; n=10): 2, 4, 19, 20, 28, 33, 51, 80, 93, 114
- `workspaces/runtime/packages/runtime-services/tests/analytics_capture_corpus.rs` (?; n=12): 4, 11, 13, 34, 68, 82, 118, 138, 178, 269, 319, 348
- `workspaces/runtime/packages/runtime-services/tests/analytics_history_checkpoint.rs` (?; n=6): 5, 19, 20, 55, 66, 73
- `workspaces/runtime/packages/runtime-services/tests/browser_host_flow.rs` (?; n=2): 13, 370
- `workspaces/runtime/packages/runtime-services/tests/claude_accounts.rs` (?; n=4): 105, 305, 308, 313
- `workspaces/runtime/packages/runtime-services/tests/claude_controls_actor.rs` (?; n=15): 2, 106, 182, 210, 415, 482, 506, 551, 655, 663, 679, 784, 1008, 1074, 1109
- `workspaces/runtime/packages/runtime-services/tests/claude_runtime/login_fixture.rs` (?; n=5): 10, 233, 241, 242, 267
- `workspaces/runtime/packages/runtime-services/tests/claude_usage_limits.rs` (?; n=8): 1, 11, 22, 29, 359, 432, 438, 446
- `workspaces/runtime/packages/runtime-services/tests/codex_login_completion.rs` (?; n=3): 6, 76, 77
- `workspaces/runtime/packages/runtime-services/tests/command_replay.rs` (?; n=5): 7, 25, 30, 38, 84
- `workspaces/runtime/packages/runtime-services/tests/context_repair_corpus.rs` (?; n=11): 2, 114, 220, 228, 233, 238, 260, 272, 477, 494, 676
- `workspaces/runtime/packages/runtime-services/tests/continuation_corpus.rs` (?; n=9): 1, 57, 111, 261, 262, 297, 321, 393, 430
- `workspaces/runtime/packages/runtime-services/tests/continuation_recovery.rs` (?; n=13): 1, 53, 58, 64, 172, 320, 409, 426, 438, 561, 581, 704, 732
- `workspaces/runtime/packages/runtime-services/tests/federation/corpus.rs` (?; n=21): 10, 37, 55, 76, 109, 126, 217, 231, 252, 259, 269, 276, 380, 391, 1148, 1409, 1410, 1851, 1927, 1942, 1956
- `workspaces/runtime/packages/runtime-services/tests/idle_compaction.rs` (?; n=4): 7, 23, 87, 128
- `workspaces/runtime/packages/runtime-services/tests/integration.rs` (?; n=3): 2, 138, 139
- `workspaces/runtime/packages/runtime-services/tests/liveness.rs` (?; n=1): 230
- `workspaces/runtime/packages/runtime-services/tests/monitor_deadline_answer.rs` (?; n=4): 3, 7, 28, 222
- `workspaces/runtime/packages/runtime-services/tests/monitor_flood_and_missed_rules.rs` (?; n=8): 3, 9, 20, 25, 195, 212, 504, 523
- `workspaces/runtime/packages/runtime-services/tests/monitor_output_ui.rs` (?; n=2): 2, 37
- `workspaces/runtime/packages/runtime-services/tests/monitor_recovery.rs` (?; n=7): 2, 12, 19, 35, 55, 379, 907
- `workspaces/runtime/packages/runtime-services/tests/monitor_start_preparation.rs` (?; n=2): 4, 65
- `workspaces/runtime/packages/runtime-services/tests/monitor_wake_guarantees.rs` (?; n=7): 3, 237, 239, 241, 242, 259, 264
- `workspaces/runtime/packages/runtime-services/tests/monitors/fixtures.rs` (?; n=8): 2, 18, 60, 80, 85, 87, 102, 103
- `workspaces/runtime/packages/runtime-services/tests/multi_server/common.rs` (?; n=10): 5, 14, 15, 110, 138, 160, 190, 197, 200, 214
- `workspaces/runtime/packages/runtime-services/tests/multi_server_corpus.rs` (?; n=34): 7, 16, 31, 32, 33, 70, 502, 672, 686, 699, 749, 778, 811, 904, 931, 1011, 1039, 1233, 1369, 1396, 1590, 1728, 1737, 1915, 1916, 2092, 2224, 2267, 2313, 2314, 2589, 2590, 3372, 3460
- `workspaces/runtime/packages/runtime-services/tests/native_runtime_state_corpus.rs` (?; n=2): 5, 79
- `workspaces/runtime/packages/runtime-services/tests/payload_safety_oracle.rs` (?; n=5): 4, 12, 191, 209, 211
- `workspaces/runtime/packages/runtime-services/tests/portable_history_python_parity.rs` (?; n=11): 5, 84, 221, 297, 299, 403, 406, 445, 471, 504, 534
- `workspaces/runtime/packages/runtime-services/tests/process_controls.rs` (?; n=10): 3, 7, 71, 88, 89, 121, 181, 183, 198, 203
- `workspaces/runtime/packages/runtime-services/tests/project_deletion.rs` (?; n=13): 3, 14, 106, 124, 146, 181, 183, 188, 329, 562, 624, 815, 839
- `workspaces/runtime/packages/runtime-services/tests/project_preparation.rs` (?; n=2): 3, 50
- `workspaces/runtime/packages/runtime-services/tests/provider_callbacks/fixture.rs` (ab; n=19): 2, 5, 22, 23, 27, 39, 48, 50, 57, 76, 93, 95, 98, 105, 110, 115, 184, 191, 199
- `workspaces/runtime/packages/runtime-services/tests/provider_callbacks/review.rs` (ab; n=11): 3, 10, 37, 64, 69, 198, 233, 249, 399, 529, 551
- `workspaces/runtime/packages/runtime-services/tests/provider_callbacks.rs` (ab; n=18): 4, 10, 34, 52, 54, 55, 56, 66, 85, 93, 108, 114, 576, 764, 827, 859, 925, 1175
- `workspaces/runtime/packages/runtime-services/tests/radio_policy_corpus.rs` (?; n=12): 3, 9, 20, 25, 26, 30, 37, 81, 192, 204, 205, 219
- `workspaces/runtime/packages/runtime-services/tests/recovery_history.rs` (?; n=12): 10, 25, 27, 55, 84, 87, 90, 326, 356, 417, 497, 521
- `workspaces/runtime/packages/runtime-services/tests/recovery_store.rs` (a; n=28): 62, 101, 110, 111, 389, 400, 498, 513, 519, 528, 541, 556, 558, 574, 611, 617, 700, 996, 997, 1004, 1009, 1030, 1049, 1081, 1113, 1159, 1193, 1513
- `workspaces/runtime/packages/runtime-services/tests/relocation_corpus.rs` (?; n=27): 6, 16, 33, 38, 47, 110, 171, 178, 184, 191, 214, 232, 238, 266, 267, 270, 286, 287, 290, 320, 402, 425, 711, 803, 815, 817, 1056
- `workspaces/runtime/packages/runtime-services/tests/removed_budget_fields.rs` (?; n=5): 4, 14, 16, 22, 23
- `workspaces/runtime/packages/runtime-services/tests/rules_engine_corpus.rs` (?; n=5): 2, 14, 17, 26, 287
- `workspaces/runtime/packages/runtime-services/tests/rules_tick_atomic.rs` (?; n=5): 4, 30, 894, 948, 950
- `workspaces/runtime/packages/runtime-services/tests/scheduler_python_parity.rs` (?; n=2): 5, 258
- `workspaces/runtime/packages/runtime-services/tests/session_cost_golden.rs` (?; n=7): 12, 25, 47, 48, 84, 489, 504
- `workspaces/runtime/packages/runtime-services/tests/shell_policy.rs` (?; n=3): 8, 58, 154
- `workspaces/runtime/packages/runtime-services/tests/startup_recovery.rs` (?; n=5): 3, 14, 22, 26, 40
- `workspaces/runtime/packages/runtime-services/tests/startup_recovery_corpus.rs` (?; n=15): 2, 17, 20, 28, 59, 99, 124, 125, 213, 236, 242, 257, 305, 322, 331
- `workspaces/runtime/packages/runtime-services/tests/submit_gates_corpus.rs` (?; n=13): 3, 38, 42, 55, 85, 89, 96, 99, 206, 263, 294, 322, 373
- `workspaces/runtime/packages/runtime-services/tests/supervisor_journal.rs` (?; n=7): 3, 67, 108, 118, 510, 658, 675
- `workspaces/runtime/packages/runtime-services/tests/support/token_rate_difference.rs` (?; n=7): 1, 7, 8, 40, 52, 53, 57
- `workspaces/runtime/packages/runtime-services/tests/task_recovery.rs` (?; n=24): 3, 8, 17, 39, 40, 79, 106, 118, 129, 208, 223, 238, 239, 246, 254, 309, 337, 380, 381, 388, 397, 436, 447, 454
- `workspaces/runtime/packages/runtime-services/tests/terminal_corpus.rs` (?; n=17): 3, 21, 29, 35, 40, 42, 44, 46, 87, 103, 105, 106, 107, 108, 109, 110, 207
- `workspaces/runtime/packages/runtime-services/tests/test_support/agent_views.rs` (ac; n=11): 4, 7, 8, 21, 31, 39, 47, 99, 101, 103, 141
- `workspaces/runtime/packages/runtime-services/tests/thread_prepare_corpus.rs` (?; n=7): 2, 5, 8, 26, 346, 369, 419
- `workspaces/runtime/packages/runtime-services/tests/title_generation_effects.rs` (?; n=7): 7, 39, 240, 259, 320, 392, 664
- `workspaces/runtime/packages/runtime-services/tests/turn_fixtures.rs` (?; n=25): 5, 9, 11, 12, 17, 22, 79, 86, 96, 98, 99, 103, 104, 105, 106, 107, 108, 112, 113, 118, 119, 120, 121, 206, 211
- `workspaces/runtime/packages/runtime-services/tests/turn_prepare_corpus.rs` (?; n=11): 2, 10, 11, 20, 36, 38, 40, 51, 68, 76, 183
- `workspaces/runtime/packages/runtime-services/tests/vm_macsync_corpus.rs` (ac; n=24): 7, 16, 21, 24, 25, 51, 81, 82, 83, 84, 86, 88, 114, 122, 123, 129, 173, 194, 296, 334, 340, 399, 442, 451
- `workspaces/runtime/packages/runtime-services/tests/vm_macsync_round.rs` (ac; n=24): 12, 32, 34, 36, 45, 68, 71, 81, 99, 102, 103, 128, 167, 185, 221, 225, 243, 251, 267, 432, 556, 717, 1120, 1681
- `workspaces/runtime/packages/runtime-services/tests/voice_provider_corpus.rs` (ab; n=20): 276, 425, 857, 871, 881, 896, 1181, 1383, 1504, 1517, 1595, 1717, 1766, 1819, 1880, 1941, 2207, 2265, 2595, 2829
- `workspaces/runtime/packages/runtime-services/tests/wave7_p03_gate_tests/http.rs` (?; n=11): 4, 10, 19, 134, 294, 487, 505, 591, 593, 617, 681
- `workspaces/runtime/packages/runtime-services/tests/wave7_p03_gate_tests/support.rs` (?; n=2): 58, 104
- `workspaces/runtime/packages/runtime-services/tests/wave7_p03_message_ingress.rs` (ad; n=40): 8, 20, 22, 23, 24, 32, 39, 40, 41, 42, 157, 172, 175, 182, 193, 212, 218, 224, 228, 247, 277, 365, 366, 375, 376, 405, 408, 416, 500, 502, 510, 520, 524, 540, 583, 910, 1008, 1032, 1138, 1165
- `workspaces/runtime/packages/runtime-services/tests/wave7_p04_callback/fixture.rs` (?; n=48): 6, 10, 11, 57, 67, 77, 79, 133, 148, 158, 165, 177, 182, 191, 199, 202, 205, 251, 267, 269, 306, 310, 347, 363, 371, 402, 411, 453, 469, 470, 511, 535, 537, 542, 543, 545, 547, 550, 551, 552, 572, 574, 581, 588, 589, 590, 591, 592
- `workspaces/runtime/packages/runtime-services/tests/wave7_p04_callback_runtime.rs` (?; n=2): 250, 421
- `workspaces/runtime/packages/runtime-services/tests/wave7_p04_review.rs` (?; n=3): 233, 326, 772
- `workspaces/runtime/packages/runtime-services/tests/wave7_p07_runtime.rs` (?; n=36): 9, 47, 48, 50, 57, 59, 89, 96, 97, 100, 113, 128, 141, 142, 157, 160, 161, 177, 201, 208, 226, 444, 698, 700, 753, 762, 822, 1119, 1137, 1139, 1310, 1390, 1400, 1455, 1458, 1459
- `workspaces/runtime/packages/runtime-services/tests/wave7_p08_control_replay.rs` (?; n=2): 2, 289
- `workspaces/runtime/packages/runtime-services/tests/wave7_p08_frozen_input.rs` (?; n=10): 2, 6, 9, 29, 37, 55, 74, 274, 278, 354
- `workspaces/runtime/packages/runtime-services/tests/wave7_p08_retained_controls.rs` (?; n=3): 2, 276, 306
- `workspaces/runtime/packages/runtime-services/tests/wave7_p08_submission.rs` (?; n=28): 1, 16, 17, 34, 70, 81, 87, 110, 114, 146, 183, 202, 214, 223, 225, 237, 252, 274, 278, 286, 291, 294, 298, 300, 305, 309, 378, 434
- `workspaces/runtime/packages/runtime-services/tests/wave7_p10_lifecycle.rs` (?; n=48): 9, 44, 91, 92, 137, 149, 153, 192, 262, 518, 529, 530, 533, 539, 708, 720, 727, 744, 748, 755, 777, 782, 784, 807, 897, 899, 947, 965, 971, 1009, 1032, 1052, 1062, 1070, 1139, 1160, 1184, 1192, 1213, 1265, 1434, 1560, 1681, 1698, 1767, 1768, 1803, 2116
- `workspaces/runtime/packages/runtime-services/tests/wave7_p11_native.rs` (?; n=33): 3, 27, 28, 35, 53, 55, 58, 59, 60, 64, 66, 93, 100, 101, 149, 158, 209, 230, 247, 248, 253, 276, 374, 666, 668, 831, 836, 838, 843, 891, 1131, 1134, 1397
- `workspaces/runtime/packages/runtime-services/tests/wave7_p12_completion.rs` (?; n=10): 8, 60, 71, 81, 94, 293, 419, 456, 464, 516
- `workspaces/runtime/packages/runtime-services/tests/wave7_p13_transcript.rs` (?; n=7): 4, 22, 513, 514, 520, 591, 612
- `workspaces/runtime/packages/runtime-services/tests/wave7_p14_resources.rs` (?; n=5): 3, 13, 23, 28, 153
- `workspaces/runtime/packages/runtime-services/tests/wave7_p15_runtime.rs` (?; n=3): 6, 137, 147
- `workspaces/runtime/packages/runtime-services/tests/wave7_p16_assignment.rs` (?; n=3): 2, 35, 41
- `workspaces/runtime/packages/runtime-services/tests/wave7_p18_account_session.rs` (?; n=7): 2, 9, 10, 11, 14, 26, 95
- `workspaces/runtime/packages/runtime-services/tests/wave7_p18_registry.rs` (?; n=2): 2, 14
- `workspaces/runtime/packages/runtime-services/tests/wave7_p18_transfer_executor.rs` (?; n=4): 2, 16, 31, 59
- `workspaces/runtime/packages/runtime-services/tests/wave7_p19_radio.rs` (?; n=47): 1, 3, 45, 100, 105, 110, 111, 112, 113, 128, 138, 155, 216, 242, 258, 292, 294, 299, 302, 305, 315, 354, 368, 393, 394, 395, 398, 399, 427, 442, 517, 525, 532, 534, 536, 539, 556, 605, 606, 620, 641, 646, 649, 725, 838, 852, 1360
- `workspaces/runtime/packages/runtime-services/tests/wave9_account_session_not_sent.rs` (?; n=4): 5, 27, 205, 235
- `workspaces/runtime/packages/runtime-services/tests/wave9_login_parity.rs` (?; n=18): 12, 220, 234, 236, 247, 252, 259, 276, 357, 383, 385, 388, 391, 392, 396, 507, 514, 521
- `workspaces/runtime/packages/runtime-services/tests/wave9_safety_buffering_executor.rs` (?; n=21): 12, 38, 39, 54, 83, 393, 580, 776, 814, 823, 991, 1156, 1242, 1253, 1413, 1479, 1518, 1685, 1866, 1886, 1955
- `workspaces/runtime/packages/runtime-services/tests/workspace_operations_corpus.rs` (?; n=10): 4, 30, 32, 56, 99, 168, 198, 225, 239, 312
- `workspaces/runtime/packages/runtime-services/tests/workspace_ready/decisions.rs` (?; n=2): 8, 15
- `workspaces/runtime/packages/runtime-services/tests/workspace_ready/effects.rs` (?; n=8): 143, 149, 621, 655, 1567, 1964, 2061, 2362

### studio-runtime-store

114 files, 1263 matching lines.

- `workspaces/runtime/packages/runtime-store/src/account_registry.rs` (a; n=17): 9, 11, 22, 30, 46, 72, 78, 81, 109, 142, 146, 173, 174, 180, 184, 283, 295
- `workspaces/runtime/packages/runtime-store/src/agent_columns.rs` (ac; n=4): 355, 357, 409, 456
- `workspaces/runtime/packages/runtime-store/src/agent_volatile.rs` (ac; n=30): 22, 256, 339, 357, 359, 374, 395, 420, 447, 462, 467, 481, 484, 497, 515, 527, 540, 562, 572, 629, 667, 730, 733, 742, 744, 745, 746, 747, 748, 749
- `workspaces/runtime/packages/runtime-store/src/approvals.rs` (a; n=4): 166, 186, 277, 353
- `workspaces/runtime/packages/runtime-store/src/domain.rs` (a; n=22): 23, 388, 406, 407, 413, 415, 416, 417, 418, 419, 420, 426, 453, 456, 457, 770, 775, 904, 913, 1196, 1294, 1631
- `workspaces/runtime/packages/runtime-store/src/fresh_state.rs` (a; n=4): 12, 232, 269, 345
- `workspaces/runtime/packages/runtime-store/src/lib.rs` (a; n=5): 419, 2435, 5183, 5185, 5192
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_22_analytics.rs` (a; n=1): 55
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_26_canvas.rs` (a; n=3): 4, 27, 42
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_26_provider_identity.rs` (ab; n=23): 7, 29, 31, 39, 41, 48, 60, 61, 65, 83, 98, 105, 126, 157, 178, 185, 245, 303, 480, 505, 549, 551, 562
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_29_model_choices.rs` (a; n=4): 3, 12, 16, 57
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_30_approvals.rs` (a; n=28): 9, 66, 75, 104, 111, 113, 125, 129, 130, 137, 141, 142, 147, 178, 197, 215, 228, 239, 269, 275, 294, 298, 307, 312, 538, 544, 551, 748
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_30_complaint_record.rs` (ac; n=2): 12, 61
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_30_one_outbox.rs` (a; n=11): 10, 65, 126, 197, 240, 245, 297, 324, 373, 382, 387
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_30_outbox_reopen.rs` (a; n=1): 56
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_30_start_wait.rs` (a; n=3): 19, 52, 54
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_access_clients.rs` (a; n=3): 5, 23, 34
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_agent_batch_55.rs` (ac; n=13): 46, 61, 69, 85, 110, 157, 158, 176, 217, 245, 269, 285, 320
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_agent_core.rs` (ac; n=1): 39
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_agent_permissions.rs` (ac; n=5): 12, 18, 22, 27, 32
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_claude_home_account.rs` (a; n=20): 16, 25, 31, 34, 39, 44, 49, 50, 51, 79, 96, 97, 110, 138, 141, 144, 151, 272, 279, 295
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_room_record.rs` (ac; n=2): 2, 86
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_sync_project.rs` (ac; n=3): 3, 18, 21
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_transfer_history_pages.rs` (a; n=4): 6, 25, 90, 107
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_work_projection.rs` (a; n=2): 13, 76
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_10_31_work_records.rs` (ac; n=8): 5, 38, 48, 59, 64, 91, 101, 166
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_agent_capacity.rs` (ac; n=5): 4, 14, 46, 141, 199
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_agent_naming.rs` (ac; n=5): 4, 14, 25, 35, 114
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_agent_null_parity.rs` (ac; n=4): 75, 162, 195, 196
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_agent_turn_null.rs` (ac; n=1): 11
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_annotation_record.rs` (ac; n=2): 3, 14
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_approval_secrets.rs` (a; n=3): 75, 78, 82
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_legacy_question_history.rs` (a; n=3): 7, 26, 29
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_plan_record.rs` (ac; n=2): 3, 14
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_project_context.rs` (a; n=4): 4, 17, 20, 77
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_project_revisions.rs` (a; n=2): 13, 27
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_radio_controls.rs` (a; n=2): 4, 16
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_rate_limit_view.rs` (a; n=5): 96, 112, 129, 145, 257
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_request_reference_outbox.rs` (a; n=1): 73
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_room_leads.rs` (a; n=3): 4, 14, 35
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_transfer_delegation_depth.rs` (a; n=3): 87, 91, 95
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_transfer_pointers.rs` (a; n=4): 102, 120, 147, 153
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_01_transfer_summary.rs` (a; n=3): 91, 118, 139
- `workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_02_legacy_vm_transfers.rs` (a; n=4): 12, 30, 36, 58
- `workspaces/runtime/packages/runtime-store/src/migrations/mod.rs` (a; n=1): 402
- `workspaces/runtime/packages/runtime-store/src/multi_server_state_check.rs` (a; n=2): 5, 67
- `workspaces/runtime/packages/runtime-store/src/records.rs` (ac; n=105): 4, 8, 28, 215, 216, 217, 747, 755, 763, 771, 783, 794, 804, 813, 820, 838, 913, 933, 978, 998, 1005, 1024, 1030, 1036, 1040, 1047, 1054, 1123, 1128, 1332, 1340, 1345, 1377, 1733, 1777, 1817, 1884, 2050, 2070, 2077, 2080, 2084, 2087, 2090, 2122, 2131, 2134, 2153, 2170, 2283, 2286, 2289, 2292, 2297, 2338, 2341, 2344, 2383, 2386, 2389, 2392, 2402, 2456, 2953, 2955, 2968, 3027, 3031, 3032, 3033, 3034, 3035, 3036, 3058, 3069, 3070, 3077, 3151, 3256, 3276, 3284, 3285, 3289, 3293, 3388, 3460, 3554, 3667, 3671, 3725, 3730, 3733, 3842, 3848, 3861, 3865, 3871, 4156, 4207, 4216, 4286, 4293, 4358, 4541, 4574
- `workspaces/runtime/packages/runtime-store/src/repositories/account_state.rs` (a; n=5): 4, 557, 567, 705, 711
- `workspaces/runtime/packages/runtime-store/src/repositories/account_transfer.rs` (a; n=3): 9, 36, 46
- `workspaces/runtime/packages/runtime-store/src/repositories/agent_indexed_reads.rs` (ac; n=2): 295, 319
- `workspaces/runtime/packages/runtime-store/src/repositories/agent_management.rs` (ac; n=13): 6, 16, 42, 74, 228, 229, 234, 241, 250, 263, 335, 336, 339
- `workspaces/runtime/packages/runtime-store/src/repositories/agent_move.rs` (ac; n=4): 7, 29, 47, 53
- `workspaces/runtime/packages/runtime-store/src/repositories/analytics.rs` (a; n=47): 8, 10, 322, 663, 707, 714, 716, 717, 718, 720, 721, 722, 725, 726, 745, 755, 884, 1004, 1088, 1100, 1131, 1202, 1238, 1264, 1293, 1310, 1341, 1364, 1377, 1379, 1383, 1385, 1393, 1406, 1442, 1443, 1468, 1469, 1490, 1510, 1519, 1532, 1546, 1572, 1675, 1683, 1684
- `workspaces/runtime/packages/runtime-store/src/repositories/analytics_capture.rs` (a; n=16): 8, 59, 78, 84, 92, 177, 202, 227, 257, 293, 297, 305, 316, 692, 759, 832
- `workspaces/runtime/packages/runtime-store/src/repositories/checkpoints.rs` (a; n=72): 4, 81, 170, 176, 254, 256, 319, 323, 351, 353, 367, 369, 370, 377, 384, 401, 406, 423, 424, 427, 429, 432, 437, 440, 443, 444, 445, 448, 461, 463, 471, 472, 478, 480, 532, 539, 551, 562, 568, 576, 583, 584, 602, 616, 637, 640, 649, 661, 665, 683, 688, 697, 704, 705, 713, 720, 752, 759, 760, 769, 786, 865, 953, 982, 983, 998, 1014, 1016, 1036, 1038, 1066, 1067
- `workspaces/runtime/packages/runtime-store/src/repositories/claude_controls.rs` (a; n=10): 5, 17, 29, 33, 40, 43, 49, 75, 80, 90
- `workspaces/runtime/packages/runtime-store/src/repositories/claude_records.rs` (ac; n=19): 4, 8, 42, 57, 58, 79, 91, 97, 99, 102, 103, 113, 114, 123, 126, 174, 189, 202, 204
- `workspaces/runtime/packages/runtime-store/src/repositories/claude_retry.rs` (a; n=5): 9, 13, 15, 39, 58
- `workspaces/runtime/packages/runtime-store/src/repositories/context_repair.rs` (a; n=11): 5, 57, 67, 82, 101, 108, 115, 131, 156, 158, 178
- `workspaces/runtime/packages/runtime-store/src/repositories/continuation.rs` (a; n=13): 4, 20, 45, 52, 61, 74, 80, 86, 106, 113, 123, 127, 165
- `workspaces/runtime/packages/runtime-store/src/repositories/coordination.rs` (a; n=1): 102
- `workspaces/runtime/packages/runtime-store/src/repositories/cost_aggregate.rs` (a; n=10): 133, 502, 524, 601, 859, 1128, 1881, 1882, 2266, 2305
- `workspaces/runtime/packages/runtime-store/src/repositories/event_write.rs` (ad; n=36): 5, 81, 86, 95, 96, 97, 105, 160, 163, 164, 217, 220, 223, 227, 228, 231, 233, 234, 235, 238, 268, 273, 274, 275, 303, 332, 346, 347, 348, 356, 358, 418, 475, 538, 561, 593
- `workspaces/runtime/packages/runtime-store/src/repositories/execution_history.rs` (a; n=47): 9, 10, 18, 42, 59, 60, 61, 62, 63, 72, 73, 74, 75, 114, 284, 285, 301, 302, 304, 311, 312, 313, 346, 358, 359, 383, 526, 603, 605, 608, 610, 612, 628, 630, 745, 762, 772, 774, 793, 888, 899, 938, 990, 1005, 1007, 1008, 1013
- `workspaces/runtime/packages/runtime-store/src/repositories/federation.rs` (a; n=33): 8, 59, 72, 77, 93, 118, 134, 139, 150, 180, 188, 237, 255, 336, 337, 345, 362, 363, 364, 372, 400, 407, 411, 419, 466, 472, 588, 592, 621, 623, 633, 640, 641
- `workspaces/runtime/packages/runtime-store/src/repositories/federation_messaging.rs` (ad; n=13): 5, 39, 48, 66, 94, 115, 134, 160, 202, 206, 208, 209, 210
- `workspaces/runtime/packages/runtime-store/src/repositories/mod.rs` (a; n=2): 88, 237
- `workspaces/runtime/packages/runtime-store/src/repositories/monitors.rs` (a; n=1): 274
- `workspaces/runtime/packages/runtime-store/src/repositories/multi_server.rs` (a; n=2): 4, 28
- `workspaces/runtime/packages/runtime-store/src/repositories/multi_server_delivery.rs` (a; n=4): 16, 45, 51, 60
- `workspaces/runtime/packages/runtime-store/src/repositories/notification_dispatch.rs` (abd; n=18): 7, 62, 83, 120, 130, 147, 149, 160, 176, 185, 194, 217, 232, 246, 342, 361, 378, 384
- `workspaces/runtime/packages/runtime-store/src/repositories/orchestration_settings.rs` (a; n=1): 232
- `workspaces/runtime/packages/runtime-store/src/repositories/outbox.rs` (a; n=1): 349
- `workspaces/runtime/packages/runtime-store/src/repositories/process_actions.rs` (a; n=5): 3, 38, 47, 55, 56
- `workspaces/runtime/packages/runtime-store/src/repositories/project_locations.rs` (a; n=6): 6, 30, 49, 63, 64, 99
- `workspaces/runtime/packages/runtime-store/src/repositories/projects.rs` (a; n=28): 6, 99, 103, 109, 129, 257, 328, 339, 342, 355, 372, 381, 445, 451, 465, 468, 475, 486, 488, 492, 494, 504, 524, 542, 599, 600, 624, 701
- `workspaces/runtime/packages/runtime-store/src/repositories/queue_write.rs` (a; n=3): 4, 103, 125
- `workspaces/runtime/packages/runtime-store/src/repositories/remote_outbox.rs` (a; n=7): 396, 407, 412, 438, 469, 524, 540
- `workspaces/runtime/packages/runtime-store/src/repositories/remote_transport.rs` (a; n=6): 10, 17, 79, 273, 278, 817
- `workspaces/runtime/packages/runtime-store/src/repositories/request_results.rs` (a; n=2): 363, 374
- `workspaces/runtime/packages/runtime-store/src/repositories/reviewer_archive.rs` (a; n=4): 174, 202, 221, 255
- `workspaces/runtime/packages/runtime-store/src/repositories/rules_tick.rs` (a; n=4): 4, 18, 113, 131
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/idle_unload.rs` (a; n=2): 281, 307
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/lifecycle/tests.rs` (a; n=13): 94, 107, 125, 129, 172, 456, 559, 755, 861, 918, 1146, 1219, 1263
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/lifecycle.rs` (a; n=23): 537, 557, 565, 744, 1119, 1396, 1429, 1452, 1516, 1698, 1700, 1822, 1824, 1826, 1935, 2014, 2030, 2095, 2136, 2443, 2444, 2625, 2851
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/selection.rs` (a; n=52): 15, 21, 62, 87, 90, 91, 325, 327, 328, 333, 334, 335, 855, 860, 996, 1168, 1360, 1367, 1466, 1467, 1485, 1487, 1492, 1585, 1595, 1597, 1598, 1599, 1602, 1605, 1627, 1628, 1631, 1635, 1639, 1642, 1662, 1695, 1722, 1723, 1762, 1781, 1782, 1831, 1835, 1843, 1966, 1985, 2035, 2044, 2079, 2464
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/steer/tests.rs` (a; n=11): 48, 53, 56, 104, 108, 109, 116, 236, 371, 805, 1264
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler/steer.rs` (a; n=25): 48, 64, 65, 66, 67, 68, 69, 127, 132, 168, 225, 238, 257, 280, 317, 355, 367, 376, 378, 389, 758, 759, 917, 925, 1101
- `workspaces/runtime/packages/runtime-store/src/repositories/scheduler.rs` (a; n=4): 5, 115, 283, 796
- `workspaces/runtime/packages/runtime-store/src/repositories/sync_documents.rs` (ac; n=7): 5, 125, 163, 172, 181, 182, 191
- `workspaces/runtime/packages/runtime-store/src/repositories/system_reads.rs` (a; n=10): 5, 17, 28, 34, 40, 46, 54, 70, 93, 115
- `workspaces/runtime/packages/runtime-store/src/repositories/terminals.rs` (a; n=3): 3, 145, 482
- `workspaces/runtime/packages/runtime-store/src/repositories/transcript.rs` (a; n=49): 12, 114, 117, 299, 308, 341, 347, 372, 378, 400, 423, 428, 429, 433, 436, 441, 442, 445, 447, 452, 502, 505, 608, 610, 611, 612, 613, 614, 615, 696, 699, 701, 703, 708, 758, 773, 901, 906, 907, 908, 923, 924, 988, 1320, 1358, 1369, 1384, 1411, 1415
- `workspaces/runtime/packages/runtime-store/src/repositories/usage.rs` (a; n=44): 6, 101, 114, 115, 124, 141, 150, 153, 154, 166, 167, 168, 192, 209, 210, 220, 248, 261, 278, 279, 289, 292, 329, 330, 335, 364, 368, 375, 381, 408, 430, 432, 438, 455, 472, 475, 512, 568, 573, 577, 589, 595, 597, 606
- `workspaces/runtime/packages/runtime-store/src/repositories/voice.rs` (a; n=5): 5, 107, 189, 209, 228
- `workspaces/runtime/packages/runtime-store/src/repositories/work_items.rs` (a; n=21): 4, 111, 112, 116, 157, 172, 199, 217, 223, 224, 233, 241, 263, 271, 272, 280, 281, 288, 394, 571, 743
- `workspaces/runtime/packages/runtime-store/src/repositories/workspace_operations.rs` (a; n=4): 6, 11, 89, 126
- `workspaces/runtime/packages/runtime-store/src/repositories/workspace_ready.rs` (a; n=17): 7, 18, 22, 32, 57, 75, 88, 223, 308, 429, 442, 470, 477, 492, 513, 531, 536
- `workspaces/runtime/packages/runtime-store/src/repositories/workspace_views.rs` (a; n=3): 1, 2, 246
- `workspaces/runtime/packages/runtime-store/src/schema.rs` (ab; n=10): 6, 361, 363, 364, 365, 366, 367, 368, 382, 605
- `workspaces/runtime/packages/runtime-store/src/state_check.rs` (a; n=4): 6, 123, 219, 326
- `workspaces/runtime/packages/runtime-store/src/state_owner.rs` (a; n=4): 46, 52, 156, 164
- `workspaces/runtime/packages/runtime-store/src/stored_text_compaction.rs` (a; n=12): 16, 477, 530, 534, 536, 592, 593, 632, 676, 707, 738, 739
- `workspaces/runtime/packages/runtime-store/src/stored_text_dictionary.rs` (a; n=1): 404
- `workspaces/runtime/packages/runtime-store/src/supervisor_journal.rs` (a; n=29): 6, 14, 27, 34, 36, 38, 40, 42, 47, 61, 62, 75, 76, 82, 90, 99, 171, 186, 248, 249, 253, 528, 560, 615, 631, 634, 663, 909, 921
- `workspaces/runtime/packages/runtime-store/src/transfer_record.rs` (ac; n=32): 31, 35, 108, 208, 216, 319, 331, 447, 769, 772, 780, 782, 802, 804, 806, 825, 838, 842, 860, 922, 931, 932, 933, 981, 1089, 1154, 1165, 1356, 1358, 1371, 1396, 1398
- `workspaces/runtime/packages/runtime-store/src/work_history.rs` (a; n=15): 10, 57, 60, 62, 74, 100, 114, 125, 133, 138, 140, 142, 173, 176, 192
- `workspaces/runtime/packages/runtime-store/tests/analytics_state_check.rs` (a; n=2): 2, 43
- `workspaces/runtime/packages/runtime-store/tests/cross_runtime.rs` (a; n=4): 4, 110, 237, 253
- `workspaces/runtime/packages/runtime-store/tests/fresh_state.rs` (a; n=2): 6, 141
- `workspaces/runtime/packages/runtime-store/tests/migration_startup.rs` (a; n=4): 2, 179, 391, 410
- `workspaces/runtime/packages/runtime-store/tests/schema_compat.rs` (ab; n=5): 4, 23, 34, 67, 575
- `workspaces/runtime/packages/runtime-store/tests/stored_text_analytics.rs` (a; n=3): 3, 40, 41
- `workspaces/runtime/packages/runtime-store/tests/support/scheduler_repository.rs` (a; n=2): 45, 602

