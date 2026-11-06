//! A checkpoint and an attachment never change after they are written (#630 item 69).
//! The check triggers refuse an update that changes the id or the record; writing the
//! same record again (a replayed write) and deleting the row stay allowed. Repeatable.
use crate::StoreResult;
use rusqlite::Transaction;

/// The tables whose rows are immutable, and the name of the row kind in the error.
const IMMUTABLE: [(&str, &str); 2] = [
    ("runtime_checkpoints", "checkpoint"),
    ("runtime_assets", "attachment"),
];

pub(crate) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for (table, kind) in IMMUTABLE {
        if !crate::schema::table_exists(tx, table)? {
            continue;
        }
        tx.execute_batch(&format!(
            "CREATE TRIGGER IF NOT EXISTS {table}_immutable BEFORE UPDATE ON {table} \
             WHEN OLD.id IS NOT NEW.id OR OLD.record IS NOT NEW.record \
             BEGIN SELECT RAISE(ABORT,'a {kind} never changes'); END"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn rows_stay_as_written_and_may_be_deleted() {
        let mut db = Connection::open_in_memory().expect("db");
        db.execute_batch(
            "CREATE TABLE runtime_checkpoints(id TEXT PRIMARY KEY, record TEXT NOT NULL);
             CREATE TABLE runtime_assets(id TEXT PRIMARY KEY, record TEXT NOT NULL);
             INSERT INTO runtime_checkpoints VALUES('c','{\"id\":\"c\"}');
             INSERT INTO runtime_assets VALUES('a','{\"id\":\"a\"}');",
        )
        .expect("tables");
        for _ in 0..2 {
            let tx = db.transaction().expect("tx");
            super::apply(&tx).expect("apply");
            tx.commit().expect("commit");
        }
        for table in ["runtime_checkpoints", "runtime_assets"] {
            assert!(
                db.execute(&format!("UPDATE {table} SET record='{{}}'"), [])
                    .is_err(),
                "{table} record"
            );
            assert!(
                db.execute(&format!("UPDATE {table} SET id='other'"), [])
                    .is_err(),
                "{table} id"
            );
            db.execute(&format!("UPDATE {table} SET record=record"), [])
                .expect("the same record again");
            db.execute(&format!("DELETE FROM {table}"), [])
                .expect("delete");
        }
    }
}
