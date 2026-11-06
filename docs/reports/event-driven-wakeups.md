# Event-driven database wakeups

Status: option (a) implemented. Historical baseline:
GitHub `main` at `515fcf14d3ab1878fd04fdc66d900ca11358905b`.

## Finding

The named loops are already event driven on this baseline. Removing a supposed
periodic poll from them would be a regression or a no-op. They have independent
waiters, but their durable-change signals already originate in the same Canvas
writer's post-commit table observer mechanism:

| Consumer                                 | Current wake and deadline                                                                                                                         | Source                                                                                                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server request outbox due                | `RuntimeOutbox`, `AccessClients`, and `ServerLinks` commits wake the orchestration scheduler; it pumps due rows and reads the next due time.      | `runtime-server/src/main.rs` (`ORCHESTRATION_WAKE_TABLES`, orchestration task); `runtime-services/src/multi_server_orchestration.rs` (`pump_due`, `next_due_at`) |
| Federation outbox and multi-server state | Outbox and federation settings, identity, peers, invites, and rooms commits wake the federation task; it computes delivery deadlines.             | `runtime-server/src/main.rs` (`FEDERATION_WAKE_TABLES`, federation task)                                                                                         |
| Monitor deadlines                        | Monitor and rule commits, relevant agent changes, receipt and filesystem events wake the monitor; it also sleeps to the next actual deadline.     | `runtime-services/src/monitor_runtime_wiring.rs`                                                                                                                 |
| Idle unload deadlines                    | Account workers observe changes to eligibility tables and orchestration settings, plus session events; each sleeps to its computed idle deadline. | `runtime-services/src/account_worker.rs` (`IDLE_STATE_OBSERVED_TABLES`, `StoreBridge`) and `account_worker/idle_unload.rs`                                       |
| Analytics projection                     | The projection catches up at startup and observes `AnalyticsProjection` commits; only a failed pass has a retry delay.                            | `runtime-services/src/analytics_projection.rs`                                                                                                                   |

The actual deadlines and bounded failure retries are necessary timers, not
periodic database polling. Network, provider, filesystem, shutdown, and session
events are independent inputs and must remain in their respective wait sets.

## One post-commit mechanism

Use the existing `RuntimeStore` writer observer registry as the single source
of durable invalidation, and reuse its table and optional row hints for all five
consumers. Existing registrations in the table above are the initial mapping;
keep each mapping next to its consumer and extend it only when a query gains a
new dependency. The shared `RuntimeOutbox` notification must reach each relevant outbox
consumer, with conservative invalidation when its scope is unknown. Do not introduce a second bus, a database change log for wakeups, or
one broad callback per writer. Existing `Notify` and `watch` handles are local
waiter adapters, not independent sources of database truth.

The writer already collects SQLite update-hook table flags and bounded row
hints, publishes pending entity views inside the transaction, and invokes table
and resource observers after its outcome. `ResourceNotifications` consumes the
existing resource observer and fans out SSE invalidations; it is part of this
same writer lifecycle, not a replacement for table hints. Resource invalidations
are intentionally coarser and may suppress receipt-only changes, so routing
scheduler wakeups solely through SSE resource notifications would miss inputs
or cause unnecessary scans. Keep entity publication before commit and resource
fanout after it; do not add publication calls to individual writers. See
`runtime-store/src/lib.rs`, `entity_publication.rs`, and
`runtime-services/src/resource_notify.rs`, as well as
[the sync protocol](../sync-protocol.md#entity-publication).

A successful changed transaction emits its relevant hints after commit, including
when the caller abandons its response. Rollback emits none. An ambiguous commit
must conservatively invalidate possible changed tables with a full hint, as the
writer already does; a hint never asserts that a write succeeded. Callbacks stay
short, nonblocking, and free of database or provider I/O because they can run on
the actor thread. Row-hint overflow or uncertainty means reread the full relevant
scope. No retry of a command or charge follows from an invalidation.

## Consumer contract

Register the waiter before the initial authoritative read. Catch up from durable
state at startup, then wait for a retained/coalesced invalidation, the earliest
actual deadline, shutdown, or another relevant external event. After a wake,
reread and recompute both work and deadline; never treat the notification as the
work itself. A commit between the read and wait must leave a retained permit or
advance a watched generation. Multiple consumers need independent subscriptions:
`notify_one` on a shared `Notify` is not broadcast. Coalescing is safe only
because each awakened consumer drains or rereads authoritative state. If a
consumer can fall behind a bounded hint stream, lag means a full reread.

For outboxes, preserve durable claim, exact request identity, delivery and
uncertain-outcome rules; reconstruct due work on restart and re-arm for a newly
earlier deadline. For monitors, retain receipt recovery and watcher events. For
idle unload, recheck eligibility before unloading and preserve disabled/empty
and setting-change behavior. For analytics, the durable projection cursor remains
the replay boundary and pruning still requires a successful analytics flush.
Neither notifications nor in-memory deadlines survive restart; startup catch-up
is the recovery path. Maintain bounded failure backoff so frequent commits do
not turn a persistent error into a busy loop. Convert wall-clock due values to
monotonic waits as currently done and recompute after wake; timers are not a
substitute for invalidation.

## Proposed phase 2 and evidence

First audit all writers and query dependencies against the table mappings and
existing tests. Address duplicate durable-notification sources, missing dependencies, and
over-broad invalidation found by that audit, using the existing writer hooks and
resource lifecycle; retain already-correct wakeups. In particular, journal and
publication outbox writes should not wake the remote schedulers when their
scope is known to be unrelated. Any expansion beyond this small design needs owner
approval under the repository simplicity rule.

For each real defect, write a failing focused test first. Exercise the actual
consumer: startup with existing due work; insert, update, delete and earlier
reschedule while asleep; two outbox consumers observing one commit; unrelated
commits remaining quiet; rollback, ambiguous commit, abandoned response, and
row-hint overflow; a commit in the read-to-wait gap; coalesced bursts; restart
catch-up; deadline firing, shutdown, and failure backoff. Include monitor and
idle eligibility changes and analytics cursor replay/prune where applicable.
Use controlled clocks and synchronization rather than repeated timing sleeps.
Verify an idle consumer performs no recurring database reads, while a real
deadline still fires. Run focused tests and `just test-fast` for a bug fix under
`docs/testing.md`; record any live evidence separately. No live server change is
needed for this design.

## Phase 2 constraint and owner decision

Both scheduler registrations currently discard the change detail and wake on
any outbox commit (`runtime-server/src/main.rs:71-90`). The optional existing
hint contains **physical SQLite rowids**, not the outbox `kind`, `target`, or
logical `id` (`runtime-store/src/lib.rs:255-262, 685-688, 1554-1561`). Kind and
target are ordinary columns (`runtime-store/src/receipt_schema.rs:16-20`), so a
rowid alone does not classify a write. In particular, a deleted row may no
longer be read by rowid. Overflow and uncertain commit outcomes yield `Full`;
callbacks must not perform database I/O (`runtime-store/src/lib.rs:1540-1545`).
Thus a callback-only kind filter on existing hints cannot safely implement this
change. The implementation carries typed hints through the writer lifecycle.

Options for the owner:

1. **Writer-side kind tag.** Attach a bounded kind classification to each
   outbox change, then dispatch only relevant hints. This is the smallest new
   state, but must capture insert, update, delete, mixed writes, overflow, and
   uncertain commits. Any missing or uncertain classification must wake both.
   Both remote families use `remote_delivery`, so the minimal safe kind tag
   wakes both for that kind; distinguishing them requires additional data.
2. **Separate table.** Move journal and publication rows to their own table and
   leave remote deliveries in the outbox. This requires a migration, updated
   readers and writers, recovery and upgrade tests, and care for existing data.
   It makes invalidation naturally table-scoped but has the largest change and
   migration risk.
3. **Coalesce wakes.** Limit rereads to at most one per configurable or fixed
   interval X, with a maximum delay of one second. Startup catch-up, an already
   due or newly earlier deadline, shutdown, and failure-backoff expiry bypass
   coalescing. This adds up to X latency to other new remote work and still
   spends CPU under a sustained stream; each exception requires explicit tests.

The live profile supplied for this decision attributes approximately 340 of
13,741 samples (2.5% of server CPU) to `pump_due`, federation `snapshot`, and
`next_due_at` together. Samples do not prove every invocation is avoidable.

## Fixture measurement (2026-11-05)

On baseline `515fcf14`, a disposable store received 20 separate commits,
alternating 10 `journal_entry` and 10 `publication` inserts. The fixture
registered the same two observers as the server, consumed each retained permit
before the next write, and invoked the named read operations once per permit.
It observed **20 orchestration wakes and 20 federation wakes**; no remote
outbox rows were present. This deliberately serial schedule prevents `Notify`
coalescing and measures one wake per committed write, not burst throughput.

Callgrind 3.25.1 on the unoptimized Rust test binary measured the inclusive
`snapshot_on` database-read subtree for 20 calls of each operation:

| Operation                  |    Total Ir | Ir per wake (mean) |
| -------------------------- | ----------: | -----------------: |
| `pump_due` read            | 103,311,475 |          5,165,574 |
| `next_due_at` read         |  16,504,766 |            825,238 |
| Federation `snapshot` read |   7,435,222 |            371,761 |

These are instruction references, not elapsed time or production CPU. They
cover only each blocking database-read subtree, excluding async scheduling and
the remainder of the scheduler loops. The first call includes cold statement
and database setup effects; there are only 20 calls and the outbox grows from
1 to 20 rows. The numbers should not be extrapolated to a release build or a
populated installation. The full fixture process, including store creation and
migrations, recorded 4,080,933,856 Ir. Command: build and run the fixture test
with `cargo test -j 4 -p studio-runtime-server --bin studio-runtime-server
temporary_outbox_wake_measurement -- --nocapture`, then run that test binary
under `valgrind --tool=callgrind` and inspect inclusive `snapshot_on` entries
with `callgrind_annotate --inclusive=yes --threshold=100`. The fixture is
included below. This is the historical baseline measurement; the typed writer
comparison below records the later implementation.

## Regression witness

This paused-time witness illustrates the typed quiet path; the enabled test is in
`targeted_scheduler_wake_tests`. In addition, exercise remote
request and federation delivery writes, a separate raw SQL write that wakes both, mixed updates and deletes, overflow,
ambiguous commits, startup catch-up, and a wake racing the wait.

```rust
#[tokio::test(start_paused = true)]
async fn journal_and_publication_outbox_writes_do_not_wake_remote_schedulers() {
    let directory = tempfile::tempdir().expect("state directory");
    let store = RuntimeStore::open(Some(directory.path())).await.expect("runtime store");
    let orchestration = Arc::new(Notify::new());
    let federation = Arc::new(Notify::new());
    observe_table_wakes(&store, &ORCHESTRATION_WAKE_TABLES, Arc::clone(&orchestration));
    observe_table_wakes(&store, &FEDERATION_WAKE_TABLES, Arc::clone(&federation));
    use studio_runtime_schema::persistence::{OutboxKind, OutboxStatus, OwnerKind};
    use studio_runtime_store::outbox::{Outbox, OutboxRow};
    for kind in [OutboxKind::JournalEntry, OutboxKind::Publication] {
        store.transaction(move |tx| {
            Outbox::put(tx, &OutboxRow { id: format!("{kind:?}"), kind, owner_kind: OwnerKind::Account, owner_id: "account".into(), request_id: None, signature: "signature".into(), status: OutboxStatus::Pending, account: None, stream_id: None, generation: None, ordinal: None, next_at: None, payload: studio_test_deps::EmptyObject {}, created: 1 })?;
            Ok(())
        }).await.expect("commit outbox input");
        assert!(tokio::time::timeout(Duration::ZERO, orchestration.notified()).await.is_err());
        assert!(tokio::time::timeout(Duration::ZERO, federation.notified()).await.is_err());
    }
}
```

The measurement fixture used below can be reproduced by placing the following
in the same test module; it creates a temporary store and does not start the
server. It serializes writes and consumes each permit before the next commit;
a burst may coalesce and produce fewer scheduler iterations.

```rust
    #[tokio::test]
    async fn temporary_outbox_wake_measurement() {
        use studio_runtime_services::multi_server_orchestration::{OrchestrationDriver, HttpsPairedTransport, RuntimeOrchestrationEffects};
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let orchestration = Arc::new(Notify::new());
        let federation = Arc::new(Notify::new());
        observe_table_wakes(&store, &ORCHESTRATION_WAKE_TABLES, orchestration.clone());
        observe_table_wakes(&store, &FEDERATION_WAKE_TABLES, federation.clone());
        let driver = OrchestrationDriver::new(store.clone(), Arc::new(HttpsPairedTransport::new(store.clone())), Arc::new(RuntimeOrchestrationEffects::new(store.clone())));
        let mut counts = (0, 0);
        for n in 0..20 {
            store.transaction(move |tx| {
                let kind = if n % 2 == 0 { "journal_entry" } else { "publication" };
                tx.execute("INSERT INTO runtime_outbox(id,kind,owner_kind,owner_id,signature,status,payload,created) VALUES(?1,?2,'account','account','signature','pending','{}',1)", rusqlite::params![n.to_string(), kind])?;
                Ok(())
            }).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), orchestration.notified()).await.unwrap();
            counts.0 += 1;
            driver.pump_due(1).await.unwrap();
            driver.next_due_at().await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), federation.notified()).await.unwrap();
            counts.1 += 1;
            studio_runtime_services::federation::snapshot(&store).await.unwrap();
        }
        println!("wake counts: orchestration={}, federation={}", counts.0, counts.1);
    }


```

## Typed writer comparison (2026-11-05)

The ignored `outbox_wake_measurement` test uses `Outbox::put` for 20 alternating
journal and publication commits and consumes each permit before the next write.
On baseline it observed 20 orchestration and 20 federation wakes; with typed
hints it observed zero of each. Callgrind measured 4,078,930,390 Ir before and
3,945,017,511 Ir after for the whole unoptimized test process, including store
creation and migrations. The baseline scheduler read subtrees used 103,282,387
Ir for `pump_due`, 16,510,807 for `next_due_at`, and 7,434,395 for federation
`snapshot`; the after fixture invokes none of them. These counts are fixture
instruction references, not production CPU or latency measurements.
