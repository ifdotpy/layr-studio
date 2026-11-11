//! Background Canvas maintenance: entity tombstone pruning and expired request
//! receipt cleanup in short writer transactions.
//!
//! Studio keeps all user history. This pass never shortens or deletes task
//! output or transcripts. Releases from 2026-10-17
//! (`478b2e176`) up to its removal also trimmed the output of finished tasks
//! older than seven days here; the `payload-v1:tasks` row that it left in
//! `runtime_payload_migrations` is a historical marker only.

use std::sync::Arc;
use std::time::Duration;

use studio_runtime_schema::persistence::REQUEST_RESULT_TTL_MS;
use studio_runtime_store::request_results::{REQUEST_RESULT_GC_BATCH_ROWS, RequestResults};
use studio_runtime_store::{RuntimeStore, StoreResult, SyncDocumentsRepository};

/// Pause between prune batches so interactive writers keep the actor.
pub const RETENTION_BATCH_PAUSE: Duration = Duration::from_millis(25);
/// Delay between complete passes. A pass that finds nothing to do performs no
/// writes.
pub const RETENTION_PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Run maintenance passes until shutdown. Stopping between awaits is safe: a
/// submitted batch commits or rolls back on its own.
pub async fn run(store: Arc<RuntimeStore>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    while !*shutdown.borrow() {
        tokio::select! {
            result = run_pass(&store) => {
                if let Err(error) = result {
                    eprintln!("storage retention pass failed: {error}");
                }
            }
            _ = shutdown.changed() => return,
        }
        tokio::select! {
            () = tokio::time::sleep(RETENTION_PASS_INTERVAL) => {}
            _ = shutdown.changed() => return,
        }
    }
}

/// Remove expired receipts and prune excess entity tombstones in batches.
pub async fn run_pass(store: &Arc<RuntimeStore>) -> StoreResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let cutoff = now.saturating_sub(REQUEST_RESULT_TTL_MS);
    let receipts_due = store
        .read_async("runtime-services/storage_retention", move |connection| {
            Ok::<_, studio_runtime_store::StoreError>(connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM request_results INDEXED BY request_results_created WHERE created<=?1)",
                [cutoff],
                |row| row.get::<_, bool>(0),
            )?)
        })
        .await?;
    if receipts_due {
        loop {
            let removed = store
                .transaction(move |transaction| Ok(RequestResults::gc(transaction, now)?))
                .await?;
            if removed < REQUEST_RESULT_GC_BATCH_ROWS {
                break;
            }
            tokio::time::sleep(RETENTION_BATCH_PAUSE).await;
        }
    }
    if !store
        .read_async("runtime-services/storage_retention", |connection| {
            SyncDocumentsRepository::entity_tombstone_prune_due(connection)
        })
        .await?
    {
        return Ok(());
    }
    loop {
        let removed = store
            .transaction(|transaction| {
                SyncDocumentsRepository::prune_entity_tombstones(transaction)
            })
            .await?;
        if removed == 0 {
            break;
        }
        tokio::time::sleep(RETENTION_BATCH_PAUSE).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use studio_runtime_store::{ENTITY_TOMBSTONE_LIMIT, ENTITY_TOMBSTONE_PRUNE_BATCH};

    async fn store() -> (tempfile::TempDir, Arc<RuntimeStore>) {
        let directory = tempfile::tempdir().expect("temporary state directory");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("open runtime store");
        (directory, Arc::new(store))
    }

    #[tokio::test]
    async fn maintenance_deletes_expired_request_results_in_batches() {
        use studio_runtime_schema::persistence::REQUEST_RESULT_TTL_MS;
        use studio_runtime_store::request_results::REQUEST_RESULT_GC_BATCH_ROWS;

        let (_directory, store) = store().await;
        let now = chrono::Utc::now().timestamp_millis();
        store.transaction(move |tx| {
            for index in 0..=REQUEST_RESULT_GC_BATCH_ROWS {
                tx.execute("INSERT INTO request_results(id,signature,result,created) VALUES(?1,'sig','{}',?2)", params![format!("expired-{index}"), now - REQUEST_RESULT_TTL_MS - 1])?;
            }
            tx.execute("INSERT INTO request_results(id,signature,result,created) VALUES('fresh','sig','{}',?1)", [now])?;
            Ok(())
        }).await.expect("seed receipts");
        run_pass(&store).await.expect("retention pass");
        let (expired, fresh): (i64, i64) = store.read_async("runtime-services/storage_retention", |connection| {
            Ok::<_, studio_runtime_store::StoreError>(connection.query_row("SELECT count(*) FILTER (WHERE id LIKE 'expired-%'), count(*) FILTER (WHERE id='fresh') FROM request_results", [], |row| Ok((row.get(0)?, row.get(1)?)))?)
        }).await.expect("read receipts");
        assert_eq!(expired, 0);
        assert_eq!(fresh, 1);
    }

    #[tokio::test]
    async fn tombstone_pruning_keeps_the_limit_and_raises_the_floor() {
        const TRANSCRIPT_TOMBSTONES: i64 = 50;
        let (_directory, store) = store().await;
        let excess = ENTITY_TOMBSTONE_PRUNE_BATCH + 100;
        let total = ENTITY_TOMBSTONE_LIMIT + excess;
        store
            .transaction(move |transaction| {
                let first_rowid =
                    studio_runtime_store::test_fixtures::canvas::entities(transaction)?
                        .into_iter()
                        .map(|row| row.source_rowid)
                        .max()
                        .unwrap_or(0)
                        + 1;
                for index in 0..total {
                    studio_runtime_store::test_fixtures::canvas::seed_entity(
                        transaction,
                        &studio_runtime_store::clock_schema::SyncEntities {
                            collection: "agent".into(),
                            id: format!("gone-{index:05}"),
                            seq: 1_000 + index,
                            hash: "h".into(),
                            payload: None,
                            deleted: 1,
                        },
                        first_rowid + index,
                    )?;
                }
                // Older transcript tombstones remain outside entity retention.
                for index in 0..TRANSCRIPT_TOMBSTONES {
                    studio_runtime_store::test_fixtures::canvas::seed_entity(
                        transaction,
                        &studio_runtime_store::clock_schema::SyncEntities {
                            collection: "transcript:agent".into(),
                            id: format!("item:item-{index:03}"),
                            seq: 1 + index,
                            hash: studio_runtime_store::sha256_hex(b""),
                            payload: None,
                            deleted: 1,
                        },
                        first_rowid + total + index,
                    )?;
                }
                Ok(())
            })
            .await
            .expect("seed tombstones");

        // Store startup and seeding use real time; only the batch pause is virtual.
        tokio::time::pause();
        let started = tokio::time::Instant::now();
        run_pass(&store).await.expect("retention pass");
        assert!(started.elapsed() >= RETENTION_BATCH_PAUSE * 2);
        tokio::time::resume();

        let (count, floor, remaining) = store
            .read_async("runtime-services/storage_retention", |connection| {
                let count = studio_runtime_store::clock_schema::SyncEntityMeta::value(
                    connection,
                    "entity_tombstone_count",
                )?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                let entities = studio_runtime_store::test_fixtures::canvas::entities(connection)?;
                let remaining = entities
                    .iter()
                    .filter(|row| row.deleted == 1 && !row.collection.starts_with("transcript:"))
                    .count() as i64;
                let transcript = entities
                    .iter()
                    .filter(|row| row.deleted == 1 && row.collection.starts_with("transcript:"))
                    .count() as i64;
                assert_eq!(transcript, TRANSCRIPT_TOMBSTONES);
                Ok::<_, studio_runtime_store::StoreError>((
                    count,
                    SyncDocumentsRepository::entity_tombstone_floor(connection)?,
                    remaining,
                ))
            })
            .await
            .expect("read tombstones");
        assert_eq!(count, ENTITY_TOMBSTONE_LIMIT.to_string());
        assert_eq!(remaining, ENTITY_TOMBSTONE_LIMIT);
        assert_eq!(floor, 1_000 + excess - 1);

        // A client whose cursor predates a pruned deletion resets instead of
        // silently missing it; a client at the floor already saw every one.
        let (behind, current) = store
            .transaction(move |transaction| {
                let pull = |after| {
                    SyncDocumentsRepository::pull_entities(transaction, after, 10, false, 0, true)
                };
                Ok((pull(1_000)?, pull(floor)?))
            })
            .await
            .expect("pull entities");
        assert!(behind.reset, "a cursor below the floor resets");
        assert_eq!(behind.tombstone_floor, floor);
        assert!(!current.reset, "a cursor at the floor continues");
        assert!(current.rows.iter().all(|row| row.sequence > floor));

        let generation = store.committed_change_generation();
        run_pass(&store).await.expect("pass at the retained limit");
        assert_eq!(store.committed_change_generation(), generation);
    }
}
