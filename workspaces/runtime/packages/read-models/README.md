# studio-read-models

## Change Contract

- Responsibility: pure render-ready projections of typed Studio domain facts.
- Public interface: `sidebar::{project, visible, status_context}` and `chat_status`.
- Contract owner: DTOs remain in `runtime-schema/src/sidebar.rs`; this crate owns
  their projection policy. Existing schema domain enums are reused as inputs.
- Boundaries: no database, HTTP, filesystem, clock, scheduling, or subscriptions.
  Services obtain facts; HTTP delivers the existing entity stream.
- Invariants: full rows, server-owned indicator precedence, opaque sort keys,
  explicit nulls. Nullable read targets reuse `ReadStateDto` and its CAS revision.
- Configuration: none. Compact recency uses the generated Rust shared constant
  at render; a boundary crossing takes effect on the next render.
- Focused checks: `cargo test -j 4 -p studio-read-models`.

Search text follows the existing committed tail. Provider deltas are buffered by
`account_worker::STREAM_CADENCE` (750 ms), then written at its existing flush or
completion boundary. The existing writer journal coalesces changed encoded tails
into at most one durable mark per lead per commit. Equal replay and worker-only
tails create no sidebar mark. Search can lag live streamed text until that
existing flush/completion commit and SSE delivery; the pilot adds no timer.
