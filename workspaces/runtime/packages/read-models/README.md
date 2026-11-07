# studio-read-models

## Change Contract

- Responsibility: pure domain operational precedence and completed-result read proofs.
- Public interface: `chat_status::{derive_chat_status, mark_read_target}`.
- Contract owner: runtime-schema owns the reason-code DTO and checked nonempty wait.
- Boundaries: no I/O, HTTP, clock, labels, sorting, grouping, or search policy.
- Invariants: Rust computes precedence once; the client formats the reason.
- Focused checks: `cargo test -j 4 -p studio-read-models`.

The existing read-time agent summary receives request-local operational facts from
runtime-services using owner/root indexes and one bottom-up pass. Lead search tails
are a separate `agentTail` collection; worker tails produce no rows or stream wakes.
The existing provider flush/completion cadence owns freshness (750 ms while streaming).
No status aggregation runs on a tail-only read, and no second mark namespace exists.
