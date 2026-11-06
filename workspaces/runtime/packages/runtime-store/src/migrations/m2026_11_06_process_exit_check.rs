//! A completed process (monitor) always has its exit code: the check triggers come from
//! `ProcessState::monitor_check_triggers` (#630 item 67). Repeatable.
use crate::StoreResult;
use rusqlite::Transaction;
use studio_runtime_schema::monitor_record::ProcessState;

pub(crate) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_monitors")? {
        return Ok(());
    }
    for (_, sql) in ProcessState::monitor_check_triggers() {
        tx.execute_batch(&sql)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn a_completed_process_without_an_exit_code_is_refused() {
        let mut db = Connection::open_in_memory().expect("db");
        db.execute_batch(
            "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY, record TEXT NOT NULL);",
        )
        .expect("table");
        for _ in 0..2 {
            let tx = db.transaction().expect("tx");
            super::apply(&tx).expect("apply");
            tx.commit().expect("commit");
        }
        db.execute(
            "INSERT INTO runtime_monitors VALUES('done','{\"status\":\"completed\",\"exitCode\":0}')",
            [],
        )
        .expect("completed with exit");
        db.execute(
            "INSERT INTO runtime_monitors VALUES('failed','{\"status\":\"failed\"}')",
            [],
        )
        .expect("failed without exit");
        let refused = db.execute(
            "INSERT INTO runtime_monitors VALUES('bad','{\"status\":\"completed\"}')",
            [],
        );
        assert!(refused.is_err());
        let refused = db.execute(
            "UPDATE runtime_monitors SET record='{\"status\":\"completed\",\"exitCode\":null}' WHERE id='failed'",
            [],
        );
        assert!(refused.is_err());
    }
}
