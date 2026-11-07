# Generated observable state and computed sidebar views

Status: Accepted, contract v5 (2026-11-07). Supersedes the initial render-ready
sidebar resource pilot for [issue #994](https://github.com/ifdotpy/layr-studio/issues/994).

## Decision

The server publishes domain facts through existing entities. The existing
`agentSummary` projection adds an operational reason code, checked wait counts,
read-receipt CAS proof and mark-unread eligibility. Rust computes precedence;
it sends no presentation labels, compact rules, search documents or sort keys.
Summary projection remains read-time. Operational aggregation uses indexed owner
reads and a bottom-up traversal once per requested batch. This preserves the
existing canonical summary cost; it does not introduce a materialized cache.

A separate `agentTail` state collection reads bounded lead tails directly from
volatile storage at the existing flush cadence. Its Rust-owned maximum is
`AGENT_TAIL_MAX_CHARS` Unicode scalar values. Tail changes do not rebuild summary
operational facts. Workers have no tail entities. Existing project, peer-team
and workspace entities retain their ownership, including saved sidebar order.
The dedicated sidebar resource, row DTO, marker namespace and transaction proof
machinery from the initial pilot are removed.

Rust schema metadata selects five collections from the existing `state` resource:
`agents`, `projects`, `peerTeams`, `workspaces` and `agentTail`.
`studio-observable-codegen` generates private mutable models and public deeply
readonly views. `store.state()` has one resource slot and observation atom;
collection maps delegate to it and do not acquire independent subscriptions.
Other entity variants remain owned by their existing consumers.

Generated apply compares fields, recurses into nested models, preserves omitted,
null and discriminated-union semantics, and mutates keyed membership only when
it changes. Snapshot detection scans the input in O(n); observable mutations are
proportional to changed members. Ordered array insertions and removals still have
native O(n) shift cost. Equal snapshots and scalar-only updates do not splice
ordered arrays. Empty, populated and cleared collections retain their identity.

## Client reference pattern

`src/components/sidebar/viewModel.ts` wraps a generated summary as `.agent`.
Computed getters add UI policy without copying facts into a compatibility Agent:
team/folder membership, compact visibility, display identity, activity ordering
and lazy search text. One list facade serves the main and combined sidebars.
Existing `useSidebarOrder` owns effective optimistic rank and interleaving;
existing combined `chatKey` and routing helpers own source identity.
Each source retains its native generated model references.

`src/status/operationalStatus.ts` is the single reason-to-label formatter used by
sidebar and conversation indicators. It owns English labels and pluralization,
not precedence. Missing operational facts yield a neutral loading value with no
status claim. A pending read may display `read_result` only for the exact current
thread, turn and receipt revision; a changed target retires that overlay.
Capture CAS proofs by value before asynchronous dispatch.

Search reads tail text only for a nonempty query. Structural grouping and order
never depend on tails. Compact recency uses the generated
`COMPACT_RECENT_WINDOW_SECONDS` duration at render time; it changes at the next
render without a new timer. Transient selection, expansion, rename drafts,
search text and pending commands stay in their existing local owners.

Lists observe membership; row shells and shared title, status, progress and menu
leaves observe their own fields. All model readers are observers, including
wrappers around non-observer widgets. Never cache derived values in React
`useMemo` keyed only by stable observable collection identity: use MobX computed
values or read inside the observer. See the
[component rules](../../workspaces/client/apps/web/README.md#observable-component-hierarchy).

This is the template for subsequent screen migrations: select existing generated
collections, add a small computed facade over raw readonly facts, reuse shared
formatters and leaves, and keep commands and transient controls in their current
owners. Do not introduce a screen framework, duplicate DTOs or another apply loop.

## Lifecycle and limits

The handwritten store runtime detects observation and delegates acquisition to
the existing subscription module after 100 ms of continuous observation.
Unobservation releases its reference; the existing module owns stale state,
20-second grace, cache eviction, batching, cursors and recovery. Untracked action
reads open no interest. SSE remains the only server-data transport.

An abandoned suspended render can retain a MobX reaction until pinned
mobx-react-lite finalization (including its 10-second timer backstop), followed
by existing subscription grace. Immediate abandoned-render release is not promised.
Radio rooms retain their existing summary path. Legacy serialized navigation
carries native facts; UI facades are created locally, never serialized.

The benchmark's conversation guard is a memoized sibling, not a real Conversation
mount. Its results establish only that guard's isolation. Measurement requires
an explicitly approved quiet host window; ordinary checks are separate evidence.
