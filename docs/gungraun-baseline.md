# Backend profiling baseline

Measured 2026-11-03 on the provisioned integration base Git `9fbccc57`,
with Rust 1.99.1, gungraun 0.17.2 and Valgrind 3.25.1. The benchmarks use
optimized code and disposable in-memory stores. Numbers are one observation,
not wall-clock latency; generated IDs, clock reads, SQLite and toolchain changes
can affect comparisons. DHAT's raw profile also contains fixture setup, so use
its benchmark summary for the measured allocation totals.

| Case | Instructions | Allocated blocks | Allocated bytes |
| --- | ---: | ---: | ---: |
| Stored text encode, warm context, 116,000 repetitive bytes | 242,337 | 1 | 116,461 |
| Stored text decode, warm context | 194,327 | 1 | 116,000 |
| Agent record JSON decode | 9,837 | 4 | 24 |
| Agent record JSON encode | 5,287 | 2 | 384 |
| Generic SSE event JSON | 23,170 | 7 | 523 |
| Create assigned work and commit | 5,704,492 | 237 | 17,096 |
| Submit review work and commit | 7,764,843 | 352 | 24,719 |
| Update agent record | 663,402 | 128 | 47,473 |
| Publish and consume live delta, 32 subscribers | 18,915 | 99 | 660 |

The SQLite cases return their fixture after the measured call, excluding database
close. Create and submit include transaction commit. The SSE case calls the
generic event encoder, not the bounded resource encoder. The services build
uses libc auxiliary-vector lookup under Valgrind; see [testing](testing.md).
Warm contexts reflect the implementation's per-thread reuse. Inputs and output
are returned after the measured call so their destruction is outside it.

Inclusive Callgrind counts overlap; do not add them together:

- Assigned work's `queue_assignment` at
  [work_write.rs:3703](../workspaces/runtime/packages/runtime-services/src/work_write.rs)
  used 3,729,716 instructions; its event enqueue used 3,718,880, including
  3,438,916 in [event insertion](../workspaces/runtime/packages/runtime-store/src/repositories/event_write.rs#L132).
- Submission spent 3,705,394 instructions in event enqueue and 3,670,578 in
  [updating work](../workspaces/runtime/packages/runtime-store/src/repositories/work_items.rs#L177).
- Agent update spent 421,642 instructions in SQLite's VDBE, reached from
  [AgentRepository::put](../workspaces/runtime/packages/runtime-store/src/repositories/agents.rs#L108).
- Generic SSE encoding spent 21,064 instructions in Axum `json_data`, called
  at [sync_stream.rs:2276](../workspaces/runtime/packages/runtime-http/src/routes/sync_stream.rs),
  including 18,482 for JSON value serialization.
- DHAT shows three 32-block allocation sites in the live delta's cloned strings
  at [resource_notify.rs:151](../workspaces/runtime/packages/runtime-services/src/resource_notify.rs),
  one per subscriber for each of agent ID, item ID, and text. The case also
  allocates the three original strings, making 99 blocks total.
- Warm stored-text decoding spent 142,748 instructions in zstd decompression and
  approximately 50,815 validating UTF-8 at
  [stored_text.rs:178](../workspaces/runtime/packages/runtime-store/src/stored_text.rs).
  Both validation and the checksum are part of the stored-data contract.
