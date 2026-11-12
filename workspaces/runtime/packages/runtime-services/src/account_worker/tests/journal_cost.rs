//! Journal cost against receipt backlog: a journal receipt that is not ready
//! holds compaction, so later settled receipts accumulate behind it.
use super::*;
use studio_runtime_providers::{DecodedJournalPayload, decode_journal_value};
use studio_runtime_schema::persistence::{OutboxKind, OutboxStatus, OwnerKind};
use studio_runtime_store::outbox::{Outbox, OutboxRow};

const GENERATION: u64 = 7;
const SCOPE: &str = "default";

pub(super) fn entry(cursor: u64, frame: Value) -> JournalEntry {
    let DecodedJournalPayload::Frame(frame) =
        decode_journal_value(cursor, GENERATION, &frame).payload
    else {
        panic!("fixture frame decodes");
    };
    JournalEntry {
        boundary_at: None,
        received_at_ms: cursor,
        cursor: EventCursor(cursor),
        generation: Generation(GENERATION),
        payload: JournalPayload::Frame(frame),
    }
}

fn delta(cursor: u64) -> JournalEntry {
    entry(
        cursor,
        json!({"method":"item/agentMessage/delta","params":{
            "threadId":"thread","turnId":"turn","itemId":"answer","delta":"x"}}),
    )
}

pub(super) async fn running_store() -> (tempfile::TempDir, Arc<RuntimeStore>) {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = Arc::new(
        RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store"),
    );
    let agent = AgentRecord::from_value(json!({"compactions":0,
        "id":"agent","name":"Agent","rootId":"agent","epoch":4,"status":"running",
        "accountHistory":[],"model":"gpt-5","effort":"high","fastMode":false,
        "daybreakEnabled":false,"cyberAccessProgram":false,"cwd":"/disposable","deletedAt":null,"events":0,"activeTools":[],
        "provider":"codex","accountKey":"default","executionMode":"host",
        "threadId":"thread","turnId":"turn","autoWake":true,"inFlight":true,"turnEpoch":4,
        "startAttempt":{"id":"attempt","epoch":4,"submitted":true,
            "accountKey":"default","threadId":"thread","turnId":"turn",
            "connectionGeneration":GENERATION,"requestTicket":"ticket",
            "executionOutcome":"unknown","events":[]}
    }))
    .expect("agent");
    store
        .transaction(move |tx| AgentRepository::put(tx, &agent))
        .await
        .expect("seed agent");
    (directory, store)
}

pub(super) async fn journal(
    store: &Arc<RuntimeStore>,
    streams: NotificationStreamBuffer,
    entries: Vec<JournalEntry>,
) -> NotificationStreamBuffer {
    let store::JournalBatch {
        streams,
        committed,
        deferred,
        ..
    } = store::journal_batch(
        Arc::clone(store),
        "default",
        SCOPE,
        &streams,
        entries,
        &Value::Null,
        None,
        Vec::new(),
        None,
        false,
    )
    .await
    .expect("journal batch commits");
    assert!(deferred.is_empty());
    assert!(!committed.is_empty());
    streams
}

fn journal_rows(store: &RuntimeStore) -> i64 {
    store
        .test_reader()
        .expect("reader")
        .query_row(
            "SELECT COUNT(*) FROM runtime_outbox WHERE stream_id=?1 AND generation=?2 AND kind=?3",
            (SCOPE, GENERATION as i64, OutboxKind::JournalEntry),
            |row| row.get(0),
        )
        .expect("count")
}

/// The worker drains publications after each batch. Fixtures that measure
/// journal receipts drop them instead.
async fn clear_publications(store: &RuntimeStore) {
    store
        .transaction_with(|tx| -> Result<(), WorkerError> {
            tx.execute(
                "DELETE FROM runtime_outbox WHERE account=?1 AND kind=?2 AND status=?3",
                (SCOPE, OutboxKind::Publication, OutboxStatus::Pending),
            )?;
            Ok(())
        })
        .await
        .expect("clear publications");
}

fn seed_journal_gap(
    tx: &studio_runtime_store::Transaction<'_>,
    cursor: u64,
    ready: bool,
) -> Result<(), WorkerError> {
    let id = store::journal_operation(SCOPE, GENERATION, cursor);
    let payload = json!({"cursor":cursor,"digest":"d","ready":ready,"committed":true});
    let mut row = if let Some(row) = Outbox::get::<Value>(tx, &id).map_err(StoreError::from)? {
        row
    } else {
        OutboxRow {
            signature: studio_runtime_store::sha256_hex(id.as_bytes()),
            id,
            kind: OutboxKind::JournalEntry,
            owner_kind: OwnerKind::Account,
            owner_id: SCOPE.into(),
            request_id: None,
            status: OutboxStatus::Committed,
            account: Some(SCOPE.into()),
            stream_id: Some(SCOPE.into()),
            generation: Some(GENERATION as i64),
            ordinal: Some(cursor as i64),
            next_at: None,
            payload: Value::Null,
            created: (now() * 1000.0) as i64,
        }
    };
    row.payload = payload;
    Outbox::put(tx, &row).map_err(StoreError::from)?;
    Ok(())
}

/// SQLite virtual machine steps between progress callbacks.
const VM_STEPS_PER_TICK: std::ffi::c_int = 1;

/// SQLite work of one journal batch, in [`VM_STEPS_PER_TICK`] units. The
/// count is deterministic for a given database state, unlike wall time.
async fn batch_work(
    store: &Arc<RuntimeStore>,
    streams: NotificationStreamBuffer,
    entries: Vec<JournalEntry>,
) -> (NotificationStreamBuffer, u64) {
    let ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter = Arc::clone(&ticks);
    store
        .transaction_with(move |tx| -> Result<(), WorkerError> {
            tx.progress_handler(
                VM_STEPS_PER_TICK,
                Some(move || {
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    false
                }),
            )?;
            Ok(())
        })
        .await
        .expect("install progress counter");
    let streams = journal(store, streams, entries).await;
    store
        .transaction_with(|tx| -> Result<(), WorkerError> {
            tx.progress_handler(VM_STEPS_PER_TICK, None::<fn() -> bool>)?;
            Ok(())
        })
        .await
        .expect("remove progress counter");
    (streams, ticks.load(std::sync::atomic::Ordering::Relaxed))
}

#[tokio::test]
async fn journal_writer_work_is_independent_of_idle_account_agents() {
    let (_directory, store) = running_store().await;
    let streams = journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
    clear_publications(&store).await;
    let (streams, small) = batch_work(&store, streams, vec![delta(2)]).await;
    clear_publications(&store).await;
    store
        .transaction(|tx| {
            for index in 0..1000 {
                let id = AgentId::from(format!("idle-{index}"));
                let mut agent = AgentRecord::new(
                    id.clone(),
                    id,
                    0,
                    studio_runtime_store::AgentStatus::Waiting,
                    "default".into(),
                );
                agent.provider = studio_runtime_schema::queue_command::Field::Value(
                    studio_runtime_providers::ProviderKind::Codex,
                );
                AgentRepository::put(tx, &agent)?;
            }
            Ok(())
        })
        .await
        .unwrap();
    let (_, large) = batch_work(&store, streams, vec![delta(3)]).await;
    eprintln!("journal writer VM steps: {small} -> {large} with 1000 idle account agents");
    assert!(
        large <= small + small / 10,
        "writer visited unrelated account agents: {small} -> {large}"
    );
}

async fn writer_preparation_work(store: &RuntimeStore) -> (u64, u64, u64) {
    store
        .transaction(|_| Ok(store::writer_preparation_tests::take()))
        .await
        .unwrap()
}

#[tokio::test]
async fn journal_prepares_payload_before_writer() {
    let (_directory, store) = running_store().await;
    let streams = journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
    writer_preparation_work(&store).await;
    let entry = entry(
        2,
        json!({"method":"item/agentMessage/delta","params":{
        "threadId":"thread","turnId":"turn","itemId":"answer","delta":"x".repeat(4096)}}),
    );
    journal(&store, streams, vec![entry]).await;
    assert_eq!(
        writer_preparation_work(&store).await.0,
        0,
        "immutable provider frames must be encoded before writer admission"
    );
}

#[tokio::test]
async fn journal_restores_stale_streams_before_writer() {
    let (_directory, store) = running_store().await;
    journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
    writer_preparation_work(&store).await;
    // A background recovery can commit before its cache handoff arrives.
    journal(&store, NotificationStreamBuffer::default(), vec![delta(2)]).await;
    assert_eq!(
        writer_preparation_work(&store).await.1,
        0,
        "reconstruct the committed stream buffer before taking the writer"
    );
}

#[tokio::test]
async fn flush_restores_streams_before_writer() {
    let (_directory, store) = running_store().await;
    journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
    writer_preparation_work(&store).await;
    store::flush_streams(Arc::clone(&store), "default", Vec::new())
        .await
        .unwrap();
    assert_eq!(
        writer_preparation_work(&store).await.1,
        0,
        "flush must reconstruct the committed buffer before writer admission"
    );
}

#[tokio::test]
async fn journal_and_flush_prepare_pending_row_before_writer() {
    for flush in [false, true] {
        let (_directory, store) = running_store().await;
        let streams = journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
        writer_preparation_work(&store).await;
        if flush {
            store::flush_streams(Arc::clone(&store), "default", Vec::new())
                .await
                .unwrap();
        } else {
            journal(&store, streams, vec![delta(2)]).await;
        }
        assert_eq!(
            writer_preparation_work(&store).await.2,
            0,
            "preserve outbox metadata from the read snapshot without decoding the old payload on the writer; flush={flush}"
        );
    }
}

#[tokio::test]
async fn claim_reuses_checked_agent_before_writer() {
    let state = State::new().await;
    let id = state.agent.clone();
    let epoch = state.epoch;
    state
        .bridge
        .worker_run("claim-writer-witness", move |store| async move {
            store
                .transaction(|_| Ok(store::writer_preparation_tests::take_claim_decodes()))
                .await?;
            assert!(
                store::claim(Arc::clone(&store), "default", &id, epoch)
                    .await?
                    .is_some()
            );
            let decodes = store
                .transaction(|_| Ok(store::writer_preparation_tests::take_claim_decodes()))
                .await?;
            assert_eq!(
                decodes, 0,
                "the outer claim must reuse its checked agent after a keyed revision check"
            );
            Ok(())
        })
        .await
        .unwrap();
}

async fn wait_for_writer_admission(store: &RuntimeStore, before: u64) {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while store.writer_admissions() == before {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("prepared read reaches writer admission");
}

#[tokio::test]
async fn stale_claim_snapshot_retries_without_losing_eligible_input() {
    let state = State::new().await;
    let id = state.agent.clone();
    let epoch = state.epoch;
    state
        .bridge
        .worker_run("claim-retry-witness", move |store| async move {
            let (entered, ready) = tokio::sync::oneshot::channel();
            let (release, held) = std::sync::mpsc::channel();
            let writer_store = Arc::clone(&store);
            let writer_id = id.clone();
            let writer = tokio::spawn(async move {
                writer_store
                    .transaction(move |tx| {
                        entered.send(()).unwrap();
                        held.recv_timeout(std::time::Duration::from_secs(30))
                            .unwrap();
                        let mut agent =
                            AgentRepository::get(tx, &AgentId::from(writer_id.as_str()))?.unwrap();
                        agent.name = Some("updated while claim was prepared".into());
                        AgentRepository::put(tx, &agent)
                    })
                    .await
            });
            ready.await.unwrap();
            let before = store.writer_admissions();
            let claim_store = Arc::clone(&store);
            let claim_id = id.clone();
            let claim = tokio::spawn(async move {
                store::claim(claim_store, "default", &claim_id, epoch).await
            });
            wait_for_writer_admission(&store, before).await;
            release.send(()).unwrap();
            writer.await.unwrap().unwrap();
            assert!(
                claim.await.unwrap()?.is_some(),
                "a stale snapshot must not consume the wake as Nothing"
            );
            let agent =
                AgentRepository::get(&store.test_reader()?, &AgentId::from(id.as_str()))?.unwrap();
            assert_eq!(
                agent.name.as_deref(),
                Some("updated while claim was prepared")
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn repeated_stream_revision_changes_use_one_logged_fallback() {
    for flush in [false, true] {
        let (_directory, store) = running_store().await;
        let streams = journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
        writer_preparation_work(&store).await;
        let (first_entered, first_ready) = tokio::sync::oneshot::channel();
        let (first_release, first_held) = std::sync::mpsc::channel();
        let first_store = Arc::clone(&store);
        let mut first_streams = streams.clone();
        let first = tokio::spawn(async move {
            first_store
                .transaction_with(move |tx| {
                    first_entered.send(()).unwrap();
                    first_held
                        .recv_timeout(std::time::Duration::from_secs(30))
                        .unwrap();
                    store::persist_streams(tx, "default", &mut first_streams)
                })
                .await
        });
        first_ready.await.unwrap();
        let before = store.writer_admissions();
        let work_store = Arc::clone(&store);
        let work = tokio::spawn(async move {
            if flush {
                store::flush_streams(work_store, "default", Vec::new())
                    .await
                    .unwrap();
            } else {
                journal(
                    &work_store,
                    NotificationStreamBuffer::default(),
                    vec![delta(2)],
                )
                .await;
            }
        });
        wait_for_writer_admission(&store, before).await;
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let writer_store = Arc::clone(&store);
        let mut streams = streams.clone();
        let second = tokio::spawn(async move {
            writer_store
                .transaction_with(move |tx| {
                    entered.send(()).unwrap();
                    held.recv_timeout(std::time::Duration::from_secs(30))
                        .unwrap();
                    store::persist_streams(tx, "default", &mut streams)
                })
                .await
        });
        wait_for_writer_admission(&store, before + 1).await;
        first_release.send(()).unwrap();
        first.await.unwrap().unwrap();
        ready.await.unwrap();
        wait_for_writer_admission(&store, before + 2).await;
        release.send(()).unwrap();
        second.await.unwrap().unwrap();
        work.await.unwrap();
        assert_eq!(
            store.writer_admissions() - before,
            3,
            "two worker admissions and one competitor; flush={flush}"
        );
        assert_eq!(
            writer_preparation_work(&store).await.1,
            1,
            "the second conflict reconstructs streams under the writer exactly once"
        );
    }
}

#[tokio::test]
async fn stream_revision_change_retries_before_journal_or_flush_writes() {
    for flush in [false, true] {
        let (_directory, store) = running_store().await;
        let mut streams =
            journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let writer_store = Arc::clone(&store);
        let writer = tokio::spawn(async move {
            writer_store
                .transaction_with(move |tx| {
                    entered.send(()).unwrap();
                    held.recv_timeout(std::time::Duration::from_secs(30))
                        .unwrap();
                    // The pending text is unchanged, but recovery has committed a
                    // newer revision before its cache handoff reaches the worker.
                    store::persist_streams(tx, "default", &mut streams)
                })
                .await
        });
        ready.await.unwrap();
        let before = store.writer_admissions();
        let work_store = Arc::clone(&store);
        let work = tokio::spawn(async move {
            if flush {
                store::flush_streams(work_store, "default", Vec::new())
                    .await
                    .unwrap();
            } else {
                journal(
                    &work_store,
                    NotificationStreamBuffer::default(),
                    vec![delta(2)],
                )
                .await;
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while store.writer_admissions() == before {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("read snapshot reaches writer admission");
        release.send(()).unwrap();
        writer.await.unwrap().unwrap();
        work.await.unwrap();
        assert_eq!(
            store.writer_admissions() - before,
            2,
            "one stale keyed check, then one fresh delta; flush={flush}"
        );
        let restored = store
            .read_async("stream-retry-witness", |tx| {
                store::load_streams(tx, "default")
            })
            .await
            .unwrap();
        assert_eq!(restored.pending_bytes(), if flush { 0 } else { 2 });
    }
}

/// An unready receipt keeps the checkpoint below it while later
/// notifications settle. Journal work per batch must not grow with that
/// backlog, and the backlog must drain once the receipt is ready.
#[tokio::test]
async fn journal_batch_work_is_independent_of_held_receipt_backlog() {
    const BACKLOG: u64 = 4_000;
    const BATCH: u64 = super::JOURNAL_BATCH_MAX_EVENTS as u64;
    let (_directory, store) = running_store().await;
    store
        .transaction_with(|tx| -> Result<(), WorkerError> { seed_journal_gap(tx, 1, false) })
        .await
        .expect("held receipt");
    let streams = NotificationStreamBuffer::default();
    // The first batch opens the stream; compare two batches that extend it.
    let streams = journal(&store, streams, (2..2 + BATCH).map(delta).collect()).await;
    clear_publications(&store).await;
    let (streams, small) = batch_work(
        &store,
        streams,
        (2 + BATCH..2 + 2 * BATCH).map(delta).collect(),
    )
    .await;
    clear_publications(&store).await;
    let base = 2 + 2 * BATCH;
    store
        .transaction_with(move |tx| -> Result<(), WorkerError> {
            for cursor in base..base + BACKLOG {
                seed_journal_gap(tx, cursor, true)?;
            }
            Ok(())
        })
        .await
        .expect("held backlog");
    let next = base + BACKLOG;
    let (mut streams, large) =
        batch_work(&store, streams, (next..next + BATCH).map(delta).collect()).await;
    eprintln!("journal batch SQLite work: {small} ticks, {large} behind {BACKLOG} held receipts");
    assert!(
        large <= small + small / 20,
        "journal work grew with the held backlog: {small} -> {large} ticks"
    );
    assert_eq!(journal_rows(&store), (3 * BATCH + BACKLOG + 1) as i64);

    store
        .transaction_with(|tx| -> Result<(), WorkerError> { seed_journal_gap(tx, 1, true) })
        .await
        .expect("release the held receipt");
    let mut cursor = next + BATCH;
    while journal_rows(&store) > 0 {
        streams = journal(&store, streams, vec![delta(cursor)]).await;
        cursor += 1;
    }
    let checkpoint = store
        .transaction_with(move |tx| store::checkpoint(tx, SCOPE, GENERATION))
        .await
        .expect("checkpoint");
    assert_eq!(checkpoint, cursor - 1);
}

/// Current journal and publication keys preserve numeric order and committed proof on reopen.
#[tokio::test]
async fn journal_and_publication_keys_keep_order_and_proof_on_reopen() {
    let (directory, store) = running_store().await;
    store
        .transaction_with(|tx| -> Result<(), WorkerError> {
            for cursor in [2, 10] {
                seed_journal_gap(tx, cursor, false)?;
            }
            Ok(())
        })
        .await
        .expect("journal gaps");
    super::publication_drain::stage(
        &store,
        3,
        1,
        json!({"account":"default","sequence":3,"index":1,"class":"pending","analyticsOverflow":1}),
    )
    .await;
    let original = store.transaction_with(|tx| -> Result<_, WorkerError> {
        for cursor in [2,10] { assert!(store::journal_committed(tx,SCOPE,GENERATION,cursor)?); }
        let ids = tx.prepare("SELECT id FROM runtime_outbox WHERE stream_id=?1 AND generation=?2 AND kind=?3 ORDER BY id")?
            .query_map((SCOPE,GENERATION as i64,OutboxKind::JournalEntry),|row|row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        assert_eq!(ids,["journal:default:7:00000000000000000002","journal:default:7:00000000000000000010"]);
        let publication:String=tx.query_row("SELECT id FROM runtime_outbox WHERE account=?1 AND kind=?2",(SCOPE,OutboxKind::Publication),|row|row.get(0))?;
        assert_eq!(publication,"notification-publication:default:00000000000000000003:00000000000000000001");
        Ok(tx.prepare("SELECT id,signature,payload,created FROM runtime_outbox WHERE account=?1 ORDER BY id")?
            .query_map([SCOPE],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,i64>(3)?)))?.collect::<Result<Vec<_>,_>>()?)
    }).await.expect("ordered current keys and proof");
    drop(store);
    let reopened = RuntimeStore::open(Some(directory.path()))
        .await
        .expect("reopen");
    let restored=reopened.transaction_with(|tx| -> Result<_,WorkerError> {
        for cursor in [2,10] { assert!(store::journal_committed(tx,SCOPE,GENERATION,cursor)?); }
        Ok(tx.prepare("SELECT id,signature,payload,created FROM runtime_outbox WHERE account=?1 ORDER BY id")?
            .query_map([SCOPE],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,i64>(3)?)))?.collect::<Result<Vec<_>,_>>()?)
    }).await.expect("restored current proof");
    assert_eq!(
        restored, original,
        "reopen cannot rename or rewrite committed evidence"
    );
}

/// At the publication receipt bound each committed token-rate effect reclaims
/// the oldest stream capture. That lookup must not grow with the outbox.
#[tokio::test]
async fn journal_batch_work_at_the_publication_bound_reads_by_key() {
    use crate::notification_dispatch::{
        NotificationObservation, NotificationPostCommit, NotificationPublicationBatch,
        NotificationStreamMetrics, ProviderDeliveryIdentity, SharedJson,
    };
    const BATCH: u64 = super::JOURNAL_BATCH_MAX_EVENTS as u64;
    let capture = |cursor: u64| NotificationPostCommit::AnalyticsStream {
        observation: NotificationObservation {
            boundary_at: None,
            agent: None,
            method: "item/agentMessage/delta".into(),
            params: SharedJson::new(
                json!({"threadId":"thread","turnId":"turn","itemId":"answer","delta":"x".repeat(4096)}),
            ),
            delivery: ProviderDeliveryIdentity {
                provider_handle: "account:default".into(),
                generation: GENERATION,
                cursor,
            },
            observed_at: crate::notification_dispatch::ObservationTime::new(cursor as f64).unwrap(),
            provider_at: crate::notification_dispatch::ObservationTime::new(cursor as f64).unwrap(),
        },
        metrics: NotificationStreamMetrics::default(),
    };
    let (_directory, store) = running_store().await;
    let streams = journal(
        &store,
        NotificationStreamBuffer::default(),
        (1..=BATCH).map(delta).collect(),
    )
    .await;
    clear_publications(&store).await;
    let (streams, below) = batch_work(
        &store,
        streams,
        (BATCH + 1..=2 * BATCH).map(delta).collect(),
    )
    .await;
    clear_publications(&store).await;
    let captures = NotificationPublicationBatch {
        sequence: u64::MAX / 2,
        effects: (1..=store::MAX_PUBLICATION_RECEIPTS as u64)
            .map(capture)
            .collect(),
        analytics_overflow: 0,
    };
    store
        .transaction_with(move |tx| store::persist_publication_batch(tx, "default", &captures))
        .await
        .expect("fill the publication bound");
    let (_, at_bound) = batch_work(
        &store,
        streams,
        (2 * BATCH + 1..=3 * BATCH).map(delta).collect(),
    )
    .await;
    eprintln!("journal batch SQLite work: {below} ticks below the bound, {at_bound} at it");
    assert!(
        at_bound <= 3 * below,
        "journal work grew at the publication bound: {below} -> {at_bound} ticks"
    );
}

#[derive(Default)]
struct RecordingSink(std::sync::Mutex<Vec<u64>>);

impl crate::notification_runtime_effects::NotificationPostCommitSink for RecordingSink {
    fn publish<'a>(
        &'a self,
        effect: &'a crate::notification_dispatch::NotificationPostCommit,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<(), crate::notification_dispatch::NotificationDispatchError>,
                > + Send
                + 'a,
        >,
    > {
        use crate::notification_dispatch::NotificationPostCommit as Effect;
        if let Effect::TokenRateStream(observation) = effect {
            self.0
                .lock()
                .expect("sink")
                .push(observation.delivery.cursor);
        }
        Box::pin(async { Ok(()) })
    }
}

/// The outbox drains in sequence order (10 after 9, unlike decimal text) and
/// deletes a delivered page in one writer transaction.
#[tokio::test]
async fn publications_drain_in_sequence_order_with_one_delete_transaction() {
    const ENTRIES: u64 = 12;
    let (_directory, store) = running_store().await;
    journal(
        &store,
        NotificationStreamBuffer::default(),
        (1..=ENTRIES).map(delta).collect(),
    )
    .await;
    let pending = |store: &RuntimeStore| -> i64 {
        store
            .test_reader()
            .expect("reader")
            .query_row(
                "SELECT COUNT(*) FROM runtime_outbox WHERE account='default' AND kind=?1 AND status=?2",
                (OutboxKind::Publication, OutboxStatus::Pending),
                |row| row.get(0),
            )
            .expect("count")
    };
    assert_eq!(pending(&store), ENTRIES as i64);
    let generation = async |store: &Arc<RuntimeStore>| {
        store
            .transaction_with_commit_generation(|tx| -> Result<(), WorkerError> {
                studio_runtime_store::clock_schema::SyncEntityMeta::put(
                    tx,
                    "drainProbe",
                    &uuid::Uuid::new_v4().to_string(),
                )?;
                Ok(())
            })
            .await
            .expect("probe transaction")
            .1
    };
    let sink = Arc::new(RecordingSink::default());
    let before = generation(&store).await;
    let delivered = store::drain_publications(Arc::clone(&store), "default".into(), sink.clone())
        .await
        .expect("drain");
    assert_eq!(
        generation(&store).await - before,
        2,
        "one delete transaction"
    );
    assert_eq!(delivered, ENTRIES as usize);
    assert_eq!(pending(&store), 0);
    assert_eq!(
        *sink.0.lock().expect("sink"),
        (1..=ENTRIES).collect::<Vec<_>>()
    );
}

/// The durable stream metadata text of the `default` account, if any.
fn stream_metadata(store: &RuntimeStore) -> Option<String> {
    studio_runtime_store::account_state::streams(&store.test_reader().expect("reader"), SCOPE)
        .expect("current stream owner")
}

/// A batch whose events leave every stream untouched writes no stream
/// snapshot; a later batch that changes a stream persists exactly the buffer
/// a restart restores.
#[tokio::test]
async fn journal_batch_persists_streams_only_when_they_change() {
    let (_directory, store) = running_store().await;
    let streams = journal(&store, NotificationStreamBuffer::default(), vec![delta(1)]).await;
    store
        .transaction_with(|tx| -> Result<(), WorkerError> {
            tx.execute(
                "UPDATE runtime_account_state SET streams=json_set(streams,'$.testMarker',1) WHERE account='default'",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("mark the stored snapshot");
    let marked = stream_metadata(&store).expect("marked snapshot");
    let rate_limits = entry(
        2,
        json!({"method":"account/rateLimits/updated","params":{"rateLimits":{}}}),
    );
    let streams = journal(&store, streams, vec![rate_limits]).await;
    assert_eq!(stream_metadata(&store).as_deref(), Some(marked.as_str()));
    let streams = journal(&store, streams, vec![delta(3)]).await;
    assert_ne!(stream_metadata(&store).as_deref(), Some(marked.as_str()));
    let restored = store
        .transaction_with(|tx| store::load_streams(tx, "default"))
        .await
        .expect("restore streams");
    assert_eq!(restored, streams);
    assert_eq!(restored.pending_bytes(), 2);
}

/// The store and its owner unwind together even when an assertion fails.
#[tokio::test]
async fn journal_fixture_directory_is_removed_on_return_and_unwind() {
    for fail in [false, true] {
        let (directory, store) = running_store().await;
        let path = directory.path().to_path_buf();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let (_directory, _store) = (directory, store);
            assert!(!fail, "injected fixture failure");
        }));
        assert_eq!(result.is_err(), fail);
        assert!(
            !path.exists(),
            "fixture state survived cleanup: {}",
            path.display()
        );
    }
}
