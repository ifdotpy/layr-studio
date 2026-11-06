//! Work documents are built at read time from `runtime_work` and its history tables
//! (`entity_scope` in runtime-services). Delete their old copies in `sync_entities` and
//! the triggers that journaled work history for the copy. The counter keeps every number
//! already used, so no cursor repeats a number.
use rusqlite::Transaction;

use crate::StoreResult;

/// Journal triggers of the removed work publication.
const RETIRED_TRIGGERS: &[&str] = &[
    "work_history_results_insert",
    "work_history_results_update",
    "work_history_decisions_insert",
    "work_history_decisions_update",
    "work_history_releases_insert",
    "work_history_releases_update",
    "work_identity_insert",
    "work_identity_update",
    "work_owner_identity_update",
    "work_attempt_identity_insert",
    "work_attempt_identity_delete",
    "work_attempt_identity_update",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for trigger in RETIRED_TRIGGERS {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
    }
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let list = crate::WORK_COLLECTIONS
        .iter()
        .map(|collection| format!("'{collection}'"))
        .collect::<Vec<_>>()
        .join(",");
    let copies: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ({list}))"),
        [],
        |row| row.get(0),
    )?;
    // A state without copies keeps its counter as it is.
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute_batch(&format!(
            "DELETE FROM sync_entities WHERE collection IN ({list})"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_copies_go_and_the_counter_keeps_their_numbers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        crate::SyncDocumentsRepository::put_entity(
            &tx,
            "rule",
            "r",
            studio_json::json!({"id":"r"}),
            false,
        )
        .expect("other entity");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('workDetail','w',999999,'h','{}',0)",
            [],
        )
        .expect("old work copy");
        tx.execute_batch(
            "CREATE TRIGGER work_identity_insert AFTER INSERT ON runtime_work BEGIN SELECT 1; END;",
        )
        .expect("old trigger");
        apply(&tx).expect("migration applies");
        let work: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM sync_entities WHERE collection='workDetail'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(work, 0);
        let other: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM sync_entities WHERE collection='rule'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(other, 1);
        assert!(crate::SyncDocumentsRepository::entity_sequence(&tx).unwrap() >= 999_999);
        let trigger: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='trigger' AND name='work_identity_insert')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!trigger);
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
