# Projections in the page

The page keeps the projection cache of every server in memory. The UI reads only
memory. This replaces the native SQLite projection store and the RxDB and
Dexie caches. See "Frontend cache" in the
[target architecture](../decisions/2026-10-28-target-client-server-architecture.md).

The [sync protocol](../sync-protocol.md) is unchanged. Stream events still carry
the data. The client does not poll.

## Who keeps what

| Frontend                                                             | Projection cache                                                     | Unsent input               |
| -------------------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------- |
| On the same machine as its server (desktop shell, loopback host)     | None. Memory only.                                                   | IndexedDB, written at once |
| Remote from its server (phone, UI-only computer, other server views) | Memory, plus one IndexedDB record per subscription (data and cursor) | IndexedDB, written at once |

The server determines locality from the authenticated
[principal](../../workspaces/runtime/packages/runtime-services/src/principal.rs).
Owner sessions are local; paired device sessions are remote. A browser on the
server machine paired as a remote device is treated as remote.

## Cache records

The [subscription cache](../../workspaces/client/apps/web/src/sync/subscriptionCache.ts)
uses one IndexedDB record per subscription, containing its data and cursor. The
server allows caching only for subscriptions with provable row ownership. The
page writes after display, at most every five seconds and when hidden. A corrupt
or unknown record is discarded and recovered with a snapshot. A loaded row
never replaces a row with a higher server sequence.

## Unsent input

The device keeps unsent input in one IndexedDB store until server confirmation.
Old RxDB unsent messages are intentionally not migrated.

## Native core

The core keeps no projection, draft or queue state. It removed the `projection.*Rows`,
`drafts.*`, `sync.*`, `outbox.*` and `preferences.*` commands and its SQLite store.
At the first server connection after a start it moves the old store's drafts,
unsent uploads and recordings to the server
([`store_move.rs`](../../workspaces/client/packages/client-core/src/store_move.rs)).
The desktop window uses the same web code as the browser: drafts, preferences and
unsent input live in the page storage described above.
