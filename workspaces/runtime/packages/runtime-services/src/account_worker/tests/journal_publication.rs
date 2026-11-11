//! Count publication work at the real journal and timer callers, before hash deduplication.
use super::journal_cost::{entry, journal, running_store};
use super::*;
use crate::scheduler::publication_work::{self, Calls};
use std::collections::BTreeMap;
use studio_runtime_schema::persistence::{OutboxKind, OutboxStatus};

const AGENTS: [(&str, &str); 2] = [("agent", "thread"), ("second", "second-thread")];
const FIRST_CURSOR: u64 = 1;
const LONG_ANSWER_CHARS: usize = crate::sync_views::AGENT_SYNC_LAST_ANSWER_CHARS + 1;

#[tokio::test]
async fn journal_dispatch_reuses_the_correlated_agent_without_full_projections() {
    const NOTIFICATIONS: usize = 8;
    for method in [
        "item/agentMessage/delta",
        "item/commandExecution/outputDelta",
        "item/reasoning/textDelta",
        "item/reasoning/summaryTextDelta",
    ] {
        let (_directory, store) = running_store().await;
        let entries = (1..=NOTIFICATIONS as u64)
            .map(|cursor| {
                entry(
                    cursor,
                    json!({"method":method,"params":{"threadId":"thread","turnId":"turn",
                "itemId":"answer","delta":"x"}}),
                )
            })
            .collect::<Vec<_>>();
        let work = store
            .transaction_with(move |tx| store::test_journal_dispatch_work(tx, &entries))
            .await
            .expect("journal dispatch");
        eprintln!(
            "{method}: {} full decodes, {} full projections for {NOTIFICATIONS} notifications",
            work.0, work.1
        );
        // One correlation read and the test-only stored-row equivalence read per delta.
        assert_eq!(work, (2 * NOTIFICATIONS, 0), "{method}");
    }
}

async fn fixture() -> (tempfile::TempDir, Arc<RuntimeStore>) {
    let (directory, store) = running_store().await;
    store
        .transaction(|tx| {
            let mut second = AgentRepository::get(tx, &AgentId::from("agent"))?
                .expect("fixture agent")
                .value();
            second["id"] = json!("second");
            second["rootId"] = json!("second");
            second["threadId"] = json!("second-thread");
            second["startAttempt"]["id"] = json!("second-attempt");
            second["startAttempt"]["threadId"] = json!("second-thread");
            second
                .as_object_mut()
                .expect("fixture agent")
                .remove("activeExecution");
            AgentRepository::put(tx, &AgentRecord::from_value(second)?)
        })
        .await
        .expect("second agent");
    (directory, store)
}

async fn start_counting(store: &RuntimeStore) {
    store
        .transaction(|_| {
            publication_work::start();
            drop(studio_runtime_store::agent_volatile::publication_work::take());
            Ok(())
        })
        .await
        .expect("start writer accounting");
}

async fn calls(store: &RuntimeStore) -> BTreeMap<String, Calls> {
    store
        .transaction(|_| Ok(publication_work::take()))
        .await
        .expect("read writer accounting")
}

fn assert_once(calls: BTreeMap<String, Calls>) {
    let expected = AGENTS
        .into_iter()
        .map(|(id, _)| {
            (
                id.to_owned(),
                Calls {
                    projections: 0,
                    validations: 1,
                },
            )
        })
        .collect();
    assert_eq!(
        calls, expected,
        "one final-row validation per changed agent; views are built at read time"
    );
}

fn notifications() -> Vec<JournalEntry> {
    AGENTS
        .into_iter()
        .flat_map(|(_, thread)| {
            [
                json!({"method":"item/started","params":{"threadId":thread,"turnId":"turn",
            "item":{"id":"answer","type":"agentMessage","text":""}}}),
                json!({"method":"item/completed","params":{"threadId":thread,"turnId":"turn",
            "item":{"id":"answer","type":"agentMessage","text":"x".repeat(LONG_ANSWER_CHARS)}}}),
            ]
        })
        .enumerate()
        .map(|(index, frame)| entry(FIRST_CURSOR + index as u64, frame))
        .collect()
}

/// The canonical projection witness now exercises the outer journal owner; a batch
/// with repeated source writes must still project and validate each agent once.
#[tokio::test]
async fn long_answers_use_canonical_bounded_agent_sync_projection() {
    let (_directory, store) = fixture().await;
    drop(
        store
            .test_reader()
            .expect("fixture database remains readable"),
    );
    start_counting(&store).await;
    let streams = journal(&store, NotificationStreamBuffer::default(), notifications()).await;
    assert_once(calls(&store).await);
    store
        .transaction(|tx| {
            for (id, _) in AGENTS {
                let raw = crate::entity_scope::direct_document(tx, "agent", id)?
                    .and_then(|row| row.payload)
                    .expect("agent projection");
                let envelope: Value = studio_json::from_str(&raw)?;
                let payload = &envelope["value"];
                assert_eq!(
                    payload["lastAnswer"].as_str().map(str::len),
                    Some(crate::sync_views::AGENT_SYNC_LAST_ANSWER_CHARS)
                );
                assert!(payload.get("turnEpoch").is_none());
                assert!(payload["startAttempt"].get("id").is_none());
            }
            Ok(())
        })
        .await
        .expect("canonical bounded projection");
    start_counting(&store).await;
    journal(&store, streams, notifications()).await;
    assert!(
        calls(&store).await.is_empty(),
        "exact replay does no publication work"
    );
}

#[tokio::test]
async fn stream_flush_batch_publishes_each_agent_once() {
    let (_directory, store) = fixture().await;
    let entries = AGENTS
        .into_iter()
        .enumerate()
        .map(|(index, (_, thread))| {
            entry(
                FIRST_CURSOR + index as u64,
                json!({"method":"item/agentMessage/delta","params":{"threadId":thread,
            "turnId":"turn","itemId":"answer","delta":"pending"}}),
            )
        })
        .collect();
    journal(&store, NotificationStreamBuffer::default(), entries).await;
    start_counting(&store).await;
    store::flush_streams(Arc::clone(&store), "default", Vec::new())
        .await
        .expect("timer flush");
    assert_once(calls(&store).await);
    start_counting(&store).await;
    store::flush_streams(Arc::clone(&store), "default", Vec::new())
        .await
        .expect("empty timer flush");
    assert!(
        calls(&store).await.is_empty(),
        "a settled flush does not republish"
    );
}

#[tokio::test]
async fn journal_projection_failure_rolls_back_sources_and_receipts() {
    let (_directory, store) = fixture().await;
    store.transaction(|tx| {
        tx.execute_batch(&format!("CREATE TRIGGER reject_agent_projection BEFORE INSERT ON metadata WHEN NEW.kind={kind} BEGIN SELECT RAISE(ABORT,'projection failure'); END;", kind=studio_runtime_store::consolidation_kinds::CanvasMetadataKind::DB_CODE_AGENTPUBLICATIONSTATE))?;
        Ok(())
    }).await.expect("publication failpoint");
    let result = store::journal_batch(
        Arc::clone(&store),
        "default",
        "default",
        &NotificationStreamBuffer::default(),
        notifications(),
        &Value::Null,
        None,
        Vec::new(),
        None,
        false,
    )
    .await;
    assert!(
        result.is_err(),
        "outer publication failure must fail the batch"
    );
    store
        .transaction(|tx| {
            for (id, _) in AGENTS {
                let agent = AgentRepository::get(tx, &AgentId::from(id))?.expect("source agent");
                assert_eq!(agent.value()["events"], 0);
            }
            let obligation_count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM runtime_outbox WHERE account='default'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(
                obligation_count, 0,
                "projection failure rolls back every journal, publication and stream obligation"
            );
            assert!(
                studio_runtime_store::account_state::streams(tx, "default")?.is_none(),
                "projection failure rolls back current stream metadata"
            );
            assert_eq!(
                studio_runtime_store::account_state::provider_checkpoint(tx, "default", 7)?,
                0,
                "projection failure cannot advance journal ACK"
            );
            assert_eq!(
                tx.query_row("SELECT COUNT(*) FROM request_results", [], |row| row
                    .get::<_, i64>(0))?,
                0,
                "projection failure cannot settle a callable result"
            );
            tx.execute_batch("DROP TRIGGER reject_agent_projection")?;
            Ok(())
        })
        .await
        .expect("atomic rollback");
    start_counting(&store).await;
    journal(&store, NotificationStreamBuffer::default(), notifications()).await;
    assert_once(calls(&store).await);
}

#[tokio::test]
async fn changed_agents_decode_once_through_final_publication() {
    let (_directory, store) = fixture().await;
    store
        .transaction(|tx| {
            let mut unrelated = AgentRepository::get(tx, &AgentId::from("agent"))?
                .expect("source agent")
                .value();
            unrelated["id"] = json!("unrelated");
            unrelated["threadId"] = json!("unrelated-thread");
            unrelated["startAttempt"]["id"] = json!("unrelated-attempt");
            unrelated["startAttempt"]["threadId"] = json!("unrelated-thread");
            unrelated
                .as_object_mut()
                .expect("fixture agent")
                .remove("activeExecution");
            AgentRepository::put(tx, &AgentRecord::from_value(unrelated)?)
        })
        .await
        .expect("unrelated agent");
    store
        .transaction(|tx| {
            let changes = crate::sync_policy::EntityChanges {
                agents: AGENTS
                    .into_iter()
                    .map(|(id, _)| id.to_owned())
                    .chain(["unrelated".to_owned()])
                    .collect(),
                ..Default::default()
            };
            crate::sync_policy::publish_entity_changes(tx, &changes, None, &|_, _, _, _| true)
        })
        .await
        .expect("initial projections");
    start_counting(&store).await;
    let streams = journal(&store, NotificationStreamBuffer::default(), notifications()).await;
    assert_once(calls(&store).await);
    let measured = store
        .transaction(|_| Ok(studio_runtime_store::agent_volatile::publication_work::take()))
        .await
        .expect("decode accounting");
    let expected = AGENTS
        .into_iter()
        .map(|(id, _)| {
            (
                id.to_owned(),
                studio_runtime_store::agent_volatile::publication_work::Calls {
                    decodes: 1,
                    volatile_loads: 1,
                    text_round_trips: 0,
                },
            )
        })
        .collect();
    assert_eq!(
        measured, expected,
        "decode/merge once, no intermediate text, no unrelated agent"
    );
    start_counting(&store).await;
    journal(&store, streams, notifications()).await;
    assert!(calls(&store).await.is_empty());
    assert!(
        store
            .transaction(|_| {
                Ok(studio_runtime_store::agent_volatile::publication_work::take().is_empty())
            })
            .await
            .expect("replay accounting"),
        "replay does no decoding"
    );
    crate::entity_publication::install(&store)
        .await
        .expect("publisher");
    store.transaction(|tx| {
        for (id, root, lead) in [("cost-lead", "cost-lead", true), ("cost-worker", "cost-lead", false)] {
            let agent = AgentRecord::from_value(json!({"compactions":0,"id":id,"rootId":root,"isLead":lead,"epoch":1,"status":"running","accountKey":"default","provider":"codex"}))?;
            AgentRepository::put(tx, &agent)?;
        }
        Ok(())
    }).await.expect("team fixture");
    for change in [true, false] {
        start_counting(&store).await;
        store
            .transaction(move |tx| {
                let mut worker = AgentRepository::get(tx, &AgentId::from("cost-worker"))?
                    .expect("worker")
                    .value();
                if change {
                    worker["tail"] = json!("new tail");
                }
                AgentRepository::put(tx, &AgentRecord::from_value(worker)?)
            })
            .await
            .expect("volatile or unchanged write");
        assert!(
            !calls(&store).await.contains_key("cost-lead"),
            "volatile or unchanged worker writes must not project the lead"
        );
        let work = store
            .transaction(|_| Ok(studio_runtime_store::agent_volatile::publication_work::take()))
            .await
            .expect("decode accounting");
        assert!(
            !work.contains_key("cost-lead"),
            "volatile or unchanged worker writes must not decode the lead"
        );
    }
}

/// Recovery may commit while the account worker still holds an older cache.
/// The next journal must build on the committed buffer, preserving its receipts.
#[tokio::test]
async fn journal_uses_committed_streams_after_recovery_advances_publications() {
    let (_directory, store) = running_store().await;
    let stale = NotificationStreamBuffer::default();
    let committed = journal(
        &store,
        stale.clone(),
        vec![entry(
            1,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
            "turnId":"turn","itemId":"answer","delta":"before"}}),
        )],
    )
    .await;
    let updated = journal(
        &store,
        stale,
        vec![entry(
            2,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
            "turnId":"turn","itemId":"answer","delta":"after"}}),
        )],
    )
    .await;
    assert!(updated.changed_from(&committed));
    store::flush_streams(Arc::clone(&store), "default", Vec::new())
        .await
        .expect("flush preserved text");
    let (record, _) = store
        .transaction(|tx| {
            studio_runtime_store::NotificationDispatchRepository::transcript_item(
                tx,
                "agent",
                "agent:answer",
            )
        })
        .await
        .expect("transcript read")
        .expect("answer transcript");
    assert_eq!(record["text"], "beforeafter");
    // Exact journal replay leaves the committed buffer untouched even with stale cache input.
    let replay = journal(
        &store,
        committed,
        vec![entry(
            2,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
            "turnId":"turn","itemId":"answer","delta":"after"}}),
        )],
    )
    .await;
    assert_eq!(
        replay,
        store
            .transaction_with(|tx| store::load_streams(tx, "default"))
            .await
            .expect("durable streams")
    );
}

async fn recovery_preserves_interleaved_journal(drain: bool) {
    use crate::recovery::{CompletionMode, RecoveryAction, RecoveryEffects, RecoveryOutcome};
    use crate::recovery_history::ConnectionScope;
    use crate::recovery_policy::RecoverySnapshot;
    let (_directory, store) = fixture().await;
    store.transaction(|tx| {
        let mut a = AgentRepository::get(tx, &AgentId::from("agent"))?.expect("agent A").value();
        a["startAttempt"]["events"] = json!(["recovery-input-a"]);
        AgentRepository::put(tx, &AgentRecord::from_value(a)?)?;
        tx.execute("INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch,turn_id) VALUES('recovery-input-a','agent','user','input A','delivered',1,4,'turn')", [])?;
        Ok(())
    }).await.expect("authorized running input for A");
    journal(
        &store,
        NotificationStreamBuffer::default(),
        vec![entry(
            1,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
            "turnId":"turn","itemId":"recovered-one","delta":"partial A"}}),
        )],
    )
    .await;
    let completion = super::super::connection_recovery::lifecycle_completion(
        &store,
        "default",
        "interleaved-recovery".into(),
        1000.0,
    )
    .await
    .expect("recovery writer");
    let expected = store
        .transaction(|tx| {
            Ok(AgentRepository::get(tx, &AgentId::from("agent"))?
                .expect("agent A")
                .value())
        })
        .await
        .expect("snapshot A before history read");
    let snapshot = RecoverySnapshot::from_record(expected).expect("recovery snapshot");
    let streams = journal(
        &store,
        NotificationStreamBuffer::default(),
        vec![entry(
            2,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"second-thread",
            "turnId":"turn","itemId":"answer-b","delta":"pending B"}}),
        )],
    )
    .await;
    let before: Value = studio_json::to_value(&streams).expect("stream JSON");
    if drain {
        store
            .transaction(|tx| {
                tx.execute(
                    "DELETE FROM runtime_outbox WHERE account='default' AND kind=?1 AND status=?2",
                    (OutboxKind::Publication, OutboxStatus::Pending),
                )?;
                Ok(())
            })
            .await
            .expect("receipt deletion after successful delivery");
    }
    let mut recovery = crate::recovery_store::RecoveryStore {
        store: Arc::clone(&store),
        connection_current: Arc::new(|_| true),
        completion,
        at: 1000.0,
        observation_current: Arc::new(|_| None),
    };
    let scope = ConnectionScope {
        account: "default".into(),
        connection: "fixture-connection".into(),
        generation: 7,
    };
    let action = RecoveryAction::Terminal {
        turn: json!({"id":"turn","threadId":"thread","status":"completed","items":[
            {"id":"recovered-one","type":"agentMessage","text":"recovered one"},
            {"id":"recovered-two","type":"agentMessage","text":"recovered two"}]}),
        native_state: "idle".into(),
        mode: CompletionMode::ObservedTurn,
    };
    let result = recovery
        .apply(&snapshot, &scope, action.clone())
        .await
        .expect("A recovery commits after B journal");
    assert!(matches!(result.outcome, RecoveryOutcome::Applied(_)));
    let committed = store
        .transaction_with(|tx| store::load_streams(tx, "default"))
        .await
        .expect("committed streams");
    let after = studio_json::to_value(&committed).expect("stream JSON");
    assert!(
        after["publication_sequence"].as_u64().expect("sequence")
            > before["publication_sequence"].as_u64().expect("sequence")
    );
    assert_eq!(
        committed.pending_bytes(),
        "pending B".len(),
        "A recovery preserves B pending text"
    );
    store::flush_streams(Arc::clone(&store), "default", Vec::new())
        .await
        .expect("flush B");
    let item = store
        .transaction(|tx| {
            studio_runtime_store::NotificationDispatchRepository::transcript_item(
                tx,
                "second",
                "second:answer-b",
            )
        })
        .await
        .expect("B transcript")
        .expect("B answer")
        .0;
    assert_eq!(item["text"], "pending B");
    // The exact successful recovery retry must not publish either recovered item twice.
    let retried = recovery
        .apply(&snapshot, &scope, action)
        .await
        .expect("stale recovery retry");
    assert!(matches!(retried.outcome, RecoveryOutcome::Superseded));
}

#[tokio::test]
async fn recovery_uses_committed_streams_with_pending_publications() {
    recovery_preserves_interleaved_journal(false).await;
}

#[tokio::test]
async fn recovery_uses_committed_streams_after_publications_are_delivered() {
    recovery_preserves_interleaved_journal(true).await;
}

/// A fresh cache must require only one scalar lookup, independent of stream count.
#[tokio::test]
async fn stream_cache_revision_keeps_allocations_and_bounds_validation_work() {
    use std::sync::atomic::{AtomicU64, Ordering};
    const MAX_REVISION_LOOKUP_STEPS: u64 = 64;
    for count in [32, 256] {
        let (_directory, store) = running_store().await;
        let base = journal(
            &store,
            NotificationStreamBuffer::default(),
            vec![entry(
                1,
                json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
                "turnId":"turn","itemId":"answer","delta":"pending"}}),
            )],
        )
        .await;
        let mut value = studio_json::to_value(base).expect("stream JSON");
        let prototype = value["entries"][0].clone();
        value["entries"] = Value::Array(
            (0..count)
                .map(|index| {
                    let mut item = prototype.clone();
                    item["key"]["item"] = json!(format!("item-{index}"));
                    item
                })
                .collect(),
        );
        let mut cached: NotificationStreamBuffer =
            studio_json::from_value(value).expect("many streams");
        let ticks = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&ticks);
        let elapsed = store
            .transaction_with(move |tx| {
                store::persist_streams(tx, "default", &mut cached)?;
                tx.progress_handler(
                    1,
                    Some(move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                        false
                    }),
                )?;
                let started = std::time::Instant::now();
                let validated = store::current_streams(tx, "default", Some(cached.clone()))?;
                let elapsed = started.elapsed();
                tx.progress_handler(1, None::<fn() -> bool>)?;
                assert!(
                    !validated.changed_from(&cached),
                    "no JSON reconstruction or entry copies"
                );
                Ok::<_, WorkerError>(elapsed)
            })
            .await
            .expect("validate cached streams");
        let steps = ticks.load(Ordering::Relaxed);
        eprintln!(
            "stream revision validation: {count} streams, {steps} SQLite VM steps, {elapsed:?}"
        );
        assert!(
            steps <= MAX_REVISION_LOOKUP_STEPS,
            "revision lookup grew with streams: {steps}"
        );
    }
}

#[tokio::test]
async fn stream_cache_revision_handles_owner_snapshots_and_rolled_back_handoffs() {
    let (_directory, store) = running_store().await;
    journal(
        &store,
        NotificationStreamBuffer::default(),
        vec![entry(
            1,
            json!({"method":"item/agentMessage/delta","params":{"threadId":"thread",
            "turnId":"turn","itemId":"answer","delta":"pending"}}),
        )],
    )
    .await;
    let canonical = store
        .transaction_with(|tx| store::load_streams(tx, "default"))
        .await
        .expect("current owner snapshot");
    let staged = Arc::new(std::sync::Mutex::new(None));
    let capture = Arc::clone(&staged);
    let original = canonical.clone();
    let failed = store
        .transaction_with(move |tx| {
            let mut next = store::current_streams(tx, "default", Some(original))?;
            store::persist_streams(tx, "default", &mut next)?;
            *capture.lock().expect("staged handoff") = Some(next);
            Err::<(), _>(store::invalid("rollback-fixture"))
        })
        .await;
    assert!(failed.is_err());
    let rolled_back = staged
        .lock()
        .expect("staged handoff")
        .take()
        .expect("buffer");
    let restored = store
        .transaction_with(move |tx| store::current_streams(tx, "default", Some(rolled_back)))
        .await
        .expect("rollback invalidates staged handoff");
    assert_eq!(restored, canonical);
    // A commit can change pending text without advancing publication_sequence.
    let old = canonical.clone();
    let mut value = studio_json::to_value(&canonical).expect("stream JSON");
    value["entries"][0]["pending"] = json!("new pending text");
    let mut replacement: NotificationStreamBuffer =
        studio_json::from_value(value).expect("new buffer");
    let current = store
        .transaction_with(move |tx| {
            store::persist_streams(tx, "default", &mut replacement)?;
            let current = store::current_streams(tx, "default", Some(old))?;
            assert_eq!(
                current, replacement,
                "delayed handoff must reload even at the same publication sequence"
            );
            Ok::<_, WorkerError>(current)
        })
        .await
        .expect("pending-only commit invalidates cache");
    assert_eq!(current.pending_bytes(), "new pending text".len());
}

#[tokio::test]
async fn preparation_expiry_skips_full_agent_decode_when_no_expiry_is_due() {
    let (_directory, store) = running_store().await;
    for pending in [false, true] {
        store
            .transaction(move |tx| {
                if pending {
                    let mut agent = AgentRepository::get(tx, &AgentId::from("agent"))?.unwrap();
                    agent.status = studio_runtime_store::AgentStatus::Starting;
                    let attempt = agent.start_attempt.as_mut().unwrap();
                    attempt.created = Some(100.0);
                    attempt.submitted = Some(false);
                    attempt.prepare_error = Some("workspace-readiness/pending".into());
                    AgentRepository::put(tx, &agent)?;
                }
                Ok(())
            })
            .await
            .unwrap();
        let decodes = store
            .transaction_with(|tx| {
                let _scope = studio_runtime_store::agent_volatile::publication_work::scope();
                assert!(!store::expire_preparation_in(
                    tx, "agent", "default", 4, 100.0, None
                )?);
                Ok::<_, WorkerError>(
                    studio_runtime_store::agent_volatile::publication_work::take_full_decodes(),
                )
            })
            .await
            .unwrap();
        assert_eq!(
            decodes, 0,
            "expiry probe must not decode the full agent (pending={pending})"
        );
    }
}

#[tokio::test]
async fn pending_metadata_ownership_skips_full_agent_decode() {
    let (_directory, store) = running_store().await;
    let decodes = store
        .transaction_with(|tx| {
            let _scope = studio_runtime_store::agent_volatile::publication_work::scope();
            assert!(store::owns_pending_metadata(tx, "agent", "default", 4)?);
            assert!(!store::owns_pending_metadata(tx, "agent", "other", 4)?);
            assert!(!store::owns_pending_metadata(tx, "agent", "default", 5)?);
            assert!(!store::owns_pending_metadata(tx, "missing", "default", 4)?);
            Ok::<_, WorkerError>(
                studio_runtime_store::agent_volatile::publication_work::take_full_decodes(),
            )
        })
        .await
        .unwrap();
    assert_eq!(
        decodes, 0,
        "metadata ownership must not decode the full agent"
    );
}

#[test]
fn pending_metadata_view_validates_identity_and_consumed_fields() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
        .unwrap();
    let valid = json!({"id":"agent","accountKey":"escaped\"account","epoch":4,
        "status":"running","startAttempt":null,"unrelated":{"large":"x".repeat(8192)}});
    db.execute(
        "INSERT INTO runtime_agents VALUES('agent',?1)",
        [valid.to_string()],
    )
    .unwrap();
    assert!(store::owns_pending_metadata(&db, "agent", "escaped\"account", 4).unwrap());
    for (field, value) in [
        ("id", json!("another-agent")),
        ("epoch", json!(-1)),
        (
            "epoch",
            json!(studio_runtime_schema::shared_values::MAX_SAFE_INTEGER + 1),
        ),
        ("accountKey", json!(false)),
        ("status", json!([])),
        ("startAttempt", json!({"submitted":"true"})),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        db.execute("UPDATE runtime_agents SET record=?1", [invalid.to_string()])
            .unwrap();
        assert!(
            store::owns_pending_metadata(&db, "agent", "escaped\"account", 4).is_err(),
            "{field}"
        );
    }
}
