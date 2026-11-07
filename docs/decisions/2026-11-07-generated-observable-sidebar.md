# Generated observable sidebar resources

Status: Accepted. Source: [issue #994](https://github.com/ifdotpy/layr-studio/issues/994),
with the owner's pilot simplifications recorded below.

## Context

The sidebar reconstructed status, eligibility, search documents and chat ordering
from broad Agent snapshots. Every update passed through renderer selectors even
when a visible row did not change. The server already owns those facts and the
reference-counted SSE subscription protocol.

## Decision

Rust serde and schemars types own the sidebar read model. `studio-read-models`
owns the DTOs and pure projections. Server services project each visible chat,
including the existing operational status projector, and publish full rows
through the existing entity protocol. A row includes its server-computed sort
key, group key, search document, action eligibility and read-receipt CAS target.
Nullable fields are required on the wire and serialize explicit null.

`studio-contracts` refreshes OpenAPI from these owners, then calls the separate
`studio-observable-codegen` crate. Typed Rust metadata identifies model identity,
opaque fields, resource arguments and sorting. Generation validates metadata
against the schema and rejects unsupported shapes with their schema path.
`contracts:check` checks deterministic committed output.

Generated data classes initialize fields before explicit `makeObservable` calls.
They compose nested models without inheritance. Generated `apply(dto)` methods
assign only changed values, recurse through objects, reconcile identified arrays,
replace changed union variants, clear omitted optional fields, and preserve null.
Opaque JSON uses reference observation. Mutable classes and their apply methods
remain private; public interfaces are deep-readonly generated contract views.

The pilot generates only `store.sidebar()`, for `{kind: "sidebar"}`. Its slot has
a strict loading/error/live/stale union; only live and stale contain data.
Generated facades cache typed slots. Keys come from the existing subscription
key owner. Each server creates its own runtime and facades through its existing
services context. No global entity graph is introduced.

The handwritten `src/store/` runtime adds observation detection. Its per-slot atom
starts the shared helper on observation; after 100 ms of continuous observation
the helper acquires the existing subscription module's reference. Unobservation
releases that reference immediately. The existing module remains the sole owner
of stale status, reference counting, 20-second grace, its inactive cache bound,
and batching through `sync/resourceEvents.ts`. Eviction clears the slot's model
graph. Subscription notifications apply in one MobX action per queued batch.
Untracked reads, including action handlers, open no subscriptions.

The sidebar follows the [observable component hierarchy](../../workspaces/client/apps/web/README.md#observable-component-hierarchy):
list membership, row shells and independently changing leaves form separate
observer boundaries. Shared title, indicator, progress and action-menu leaves
are reused across the main and combined sidebars. Selection, expansion, drag state,
rename drafts and pending command state remain device-local. The server owns the
final indicator including unread precedence. Only a pending mark-read for the
exact current CAS target may temporarily display it as read; changing that
target retires the overlay.

Compact visibility combines server `compactPinned` with the Rust-owned
`COMPACT_RECENT_WINDOW_SECONDS` constant and `activityAt` at render time. Crossing
the recency boundary takes effect on the next render. This deliberately avoids
a new timer or server scheduler. Selected rows, search, thresholds and expansion
remain local presentation choices. Existing folder/team/chat interleaving stays
with its current owner; the pilot does not introduce a tree framework.

## Consequences and limitations

React, Mantine and the visual design stay unchanged. Existing SSE transport,
cursors, epochs, tombstones, baseline replacement, atomic persistence and
multi-server identity remain authoritative. There is no polling or second
transport/recovery path. `sidebarChatRow` is excluded from the broad state
projection; sidebar consumers use the resource directly.

An abandoned render, such as a component suspending before commit, can retain a
MobX reaction and acquire interest after 100 ms. Pinned mobx-react-lite 5.1.2 uses
native finalization with a 10-second timer backstop; finalization releases the
reference, followed by the existing subscription grace. This is a documented
render-lifecycle limitation, not a promise of immediate abandoned-render cleanup.

Conversation and other screens still use their shared status helpers during the
pilot. This temporarily duplicates the projection across those screens and Rust;
sidebar callers are migrated without an Agent compatibility projection. Future
migration removes a helper only when its final caller has moved.

Radio rooms retain their existing sidebar summary path. They are the next
read-model candidate and are not represented as `SidebarChatRow` values.

The legacy project summary navigation crosses serialized frame messages and
retains plain DTOs. Its reused observer leaves do not isolate field updates
across a whole message replacement. The combined sidebar reconciles mapped rows
through the generated mirror, preserving model identity and field isolation.

## Alternatives

A handwritten mutable renderer model would create a second contract owner.
MST or mobx-keystone would add another model system. Subscription hooks would
preserve per-component wiring, while a new subscription coordinator would
duplicate the existing lifecycle. A generic resource graph, tree framework and
deadline scheduler add no necessary behavior to this pilot.
