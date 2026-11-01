//! Number old `runtime_items` online and check the transcript copies without holding
//! the writer. Copy retirement remains disabled until projected items have a safe cursor.

use std::sync::Arc;

use studio_runtime_store::RuntimeStore;
use studio_runtime_store::durable_seq::{
    transcript_copies_retired, transcript_copy_missing_from_owner,
};
use studio_runtime_store::item_seq_backfill::ItemSeqBackfill;
use studio_runtime_store::online_migration::{self, Step};

/// Run the backfill and check the copies, unless shutdown is requested.
/// Stopping is safe: the next start resumes from the saved mark.
pub async fn run(store: Arc<RuntimeStore>, shutdown: tokio::sync::watch::Receiver<bool>) {
    let stop = shutdown.clone();
    match online_migration::run(&store, Arc::new(ItemSeqBackfill), move || *stop.borrow()).await {
        Ok(Step::Complete) => {}
        Ok(Step::Progress) => return,
        Err(error) => {
            eprintln!("transcript sequence backfill failed: {error}");
            return;
        }
    }
    if *shutdown.borrow() {
        return;
    }
    let outcome = store
        .read_async("runtime-services/transcript-copy-check", |transaction| {
            if transcript_copies_retired(transaction)? {
                return Ok(Vec::new());
            }
            transcript_copy_missing_from_owner(transaction)
        })
        .await;
    match outcome {
        Ok(missing) if missing.is_empty() => {}
        Ok(missing) => eprintln!(
            "transcript copies kept: {} items are missing from runtime_items",
            missing.len()
        ),
        Err(error) => eprintln!("transcript copy check failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    #[tokio::test]
    async fn copy_check_does_not_admit_a_writer_or_retire_copies() {
        let root = tempfile::tempdir().expect("state");
        let store = Arc::new(
            studio_runtime_store::RuntimeStore::open(Some(root.path()))
                .await
                .expect("store"),
        );
        store
            .transaction(|tx| {
                tx.execute(
                    "DELETE FROM sync_entity_meta WHERE key='transcript_copies'",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("keep copies");
        let before = store.writer_admissions();
        let (_sender, receiver) = tokio::sync::watch::channel(false);
        super::run(Arc::clone(&store), receiver).await;
        assert_eq!(
            store.writer_admissions() - before,
            2,
            "only migration begin and step use the writer"
        );
        assert!(
            !store
                .read_async("test", |db| {
                    studio_runtime_store::durable_seq::transcript_copies_retired(db)
                })
                .await
                .expect("flag")
        );
    }
}
