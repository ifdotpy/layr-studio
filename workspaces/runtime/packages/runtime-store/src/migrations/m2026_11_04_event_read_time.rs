//! The event window and the message receipts are built at read time from
//! `runtime_events` and the receipt marks (`entity_scope` in runtime-services). Drop the
//! triggers that copied the window and the receipt tombstones into `sync_entities`,
//! number the events in `durable_rows`, mark the receipts of the copies, and delete the
//! copies. The counter keeps every number already used.
use rusqlite::Transaction;

use crate::StoreResult;

const RETIRED_TRIGGERS: &[&str] = &[
    "sync_entity_event_INSERT",
    "sync_entity_event_UPDATE",
    "sync_entity_event_UPDATE_WINDOW",
    "sync_entity_event_DELETE",
    "summary_messageReceipt_delete",
    "summary_task_delete",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for trigger in RETIRED_TRIGGERS {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
    }
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ('event','messageReceipt'))",
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
    }
    crate::durable_seq::install(tx)?;
    if copies {
        let receipts = tx
            .prepare(
                "SELECT id FROM sync_entities WHERE collection='messageReceipt' AND deleted=0",
            )?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in receipts {
            crate::durable_seq::mark(tx, crate::durable_seq::RECEIPT_VIEW_MARK, &id)?;
        }
        tx.execute_batch(
            "DELETE FROM sync_entities WHERE collection IN ('event','messageReceipt');
             DELETE FROM sync_entity_meta WHERE key='event_window_seq';",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_and_triggers_go_and_events_get_numbers_above_them() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute_batch(
            "DROP TRIGGER IF EXISTS durable_runtime_events_insert;
             INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch) VALUES('e','a','user','hi','pending',1,0);
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('event','e',999999,'h','{}',0);
             CREATE TRIGGER sync_entity_event_INSERT AFTER INSERT ON runtime_events BEGIN SELECT 1; END;",
        )
        .expect("old window");
        apply(&tx).expect("migration applies");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='event'"),
            0
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM sqlite_master WHERE name='sync_entity_event_INSERT'"),
            0
        );
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='runtime_events' AND id='e'") > 999_999
        );
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
