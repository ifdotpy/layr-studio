# Observable resources

`index.ts` exports the small runtime used by generated resource facades. A server
creates one runtime with its existing subscription module. Components consume
generated facades and readonly `ResourceSlot.current` values; mutable models and
their `apply` methods stay private to generated code.

Each slot tracks observation through its own atom. `interest.ts` waits for 100 ms
of continuous observation before acquiring a reference. The existing subscription
module owns reference counting, immediate stale status, transport batching,
20-second grace, and its 128-entry inactive cache bound. This module owns none of
the SSE transport, cursor, epoch, tombstone, or server identity policy.

Subscription notifications coalesce in one microtask and apply inside one MobX
action. Actions reading a facade outside an observer acquire no interest. An
abandoned React render can retain a reaction until mobx-react-lite finalization;
the reference is then released through the same subscription grace.

Generated argument facades create one `runtime.keyed({ create, apply })` factory
per resource kind. Generated branded arguments become a `ResourceRef`; the runtime
uses the existing `subscriptionKey` for identity. A plain Map LRU retains at most
`MAX_INACTIVE_RESOURCE_FACADES` unobserved facades across these factories, refreshed
on lookup; observed slots are pinned. This cache owns only model retention: its
eviction resets `current` to `loading`, without releasing transport or changing
the subscription owner's independent FIFO, 20-second grace or inactive bound.
A held facade resolves the canonical slot on reuse, so it can reacquire normally.

Parents observe `slot.current`, narrow its status and pass the live/stale model
reference to leaves. In-place apply keeps that model identity. Reset/eviction
changes the slot status observably, so the parent retires the old model; a detached
model reference alone carries no subscription interest or validity guarantee.
Scalar and union roots can return a replacement from `apply`; equal roots leave
the slot atom unchanged.

A confirmed live subscription with no selected record yields `absent` and drops
the retained model. Loading or an interrupted connection does not establish
absence. Requests and question history have independent generated projections
over the same state subscription; observing the sidebar state creates no approval
models. Request leaves observe their own model fields; request views share computed
question descriptions and stdin classification without combining status and description.
Answer draft identity includes the server scope, owner, request kind and complete fields.
Projection slot keys differ, but acquisition and eviction use the shared
resource identity, so an owner eviction resets every projection of that resource.

Transcript and room facades use the same subscription data observer as handwritten
consumers. `createDataObserver` attaches `subscribeTranscriptProjection` with the
complete ref (including tail), preserving the existing local projection's partial
metadata, contiguous-window checks, durable cursors, epoch handling and oversized
interval recovery. Room views retain their existing inline/recovery adapter and
numeric history cursors. The runtime does not page, merge messages, advance cursors
or issue reads. Wave 3 can pass generated window model references to its history
UI while keeping `transcriptPageBounds.ts` and the current user-driven page-read
path as the history owner; drafts and outbox stay handwritten.

Rust also generates `entityCollections.ts` (payload collection names and typed map
initialization) and each collection facade's serialization boundary, including
`serializeStateCollections`. The compatibility entity projection consumes that
inventory. Work items, results and gate evidence use the existing scoped work
selector through generated field DTOs; terminal metadata and merged output use
their existing keyed subscriptions. `retainedWorktrees(agentId)` selects the lead
and its deleted members by retained root identity, without requiring live agents
or a global state interest.

Retained cleanup delivery relies on the production soft-delete path: the agent
row keeps its root identity while cleanup completes. An arbitrary hard delete of
a retained agent is not delivered incrementally; an authoritative snapshot
reconciles it. Cross-root hard-delete tombstones are never included in this feed.
