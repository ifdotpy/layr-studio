//! The task and monitor windows keep only their members in `durable_rows`
//! (`durable_seq::TASK_WINDOW`, `durable_seq::MONITOR_WINDOW`); the documents are built
//! at read time (`entity_scope` in runtime-services). Move the members of the old
//! copies in `sync_entities` into the windows, keep what the task window compares, and
//! delete the copies. The counter keeps every number already used.
use rusqlite::Transaction;

use crate::StoreResult;
use crate::durable_seq::{MONITOR_WINDOW, TASK_WINDOW};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ('task','monitor'))",
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
    }
    crate::durable_seq::install(tx)?;
    if !copies {
        return Ok(());
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_window_state (id TEXT PRIMARY KEY,
             agent TEXT NOT NULL, created REAL, running INTEGER NOT NULL);
         INSERT OR REPLACE INTO task_window_state(id,agent,created,running)
           SELECT id,json_extract(payload,'$.value.agent'),json_extract(payload,'$.value.created'),
                  json_extract(payload,'$.value.status')='running'
           FROM sync_entities
           WHERE collection='task' AND deleted=0 AND json_extract(payload,'$.value.agent') IS NOT NULL;",
    )?;
    for (collection, window) in [("task", TASK_WINDOW), ("monitor", MONITOR_WINDOW)] {
        let members = tx
            .prepare("SELECT id FROM sync_entities WHERE collection=?1 AND deleted=0")?
            .query_map([collection], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in members {
            crate::durable_seq::join_window(tx, window, &id)?;
        }
    }
    tx.execute_batch("DELETE FROM sync_entities WHERE collection IN ('task','monitor')")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_move_into_the_windows_above_the_old_numbers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute_batch(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('task','t',999999,'h','{\"value\":{\"agent\":\"a\",\"created\":5,\"status\":\"running\"}}',0),
               ('task','gone',999998,'h','{}',1),
               ('monitor','m',999997,'h','{}',0),
               ('workspace','current',5,'h','{}',0);",
        )
        .expect("old copies");
        apply(&tx).expect("migration applies");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection IN ('task','monitor')"),
            0
        );
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='task_window' AND id='t' AND deleted=0")
                > 999_999
        );
        assert!(
            count(
                "SELECT seq FROM durable_rows WHERE tbl='monitor_window' AND id='m' AND deleted=0"
            ) > 999_999
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM durable_rows WHERE tbl='task_window' AND id='gone'"),
            0
        );
        assert_eq!(
            count("SELECT running FROM task_window_state WHERE id='t'"),
            1
        );
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
