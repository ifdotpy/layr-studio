//! The account observations move out of `sync_entities` into their own tables: the
//! rate-limit views (`runtime_rate_limits`), the account list summary
//! (`runtime_account_summaries`) and the cost summaries (`runtime_account_costs`,
//! `runtime_session_costs`) and the desktop state (`runtime_desktop_states`). A row's `record` is the old payload envelope; the client
//! documents are built from it at read time. The counter keeps every number already
//! used (#630 item 127).
use rusqlite::Transaction;

use crate::StoreResult;

const MOVES: [(&str, &str); 5] = [
    ("rateLimit", "runtime_rate_limits"),
    ("accounts", "runtime_account_summaries"),
    ("accountCost", "runtime_account_costs"),
    ("sessionCost", "runtime_session_costs"),
    ("desktop", "runtime_desktop_states"),
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for (_, table) in MOVES {
        tx.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, record TEXT NOT NULL);"
        ))?;
    }
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    for (collection, table) in MOVES {
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO {table}(id,record)
                   SELECT id,payload FROM sync_entities
                   WHERE collection=?1 AND deleted=0 AND payload IS NOT NULL"
            ),
            [collection],
        )?;
    }
    crate::durable_seq::install(tx)?;
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities
           WHERE collection IN ('rateLimit','accounts','accountCost','sessionCost','desktop'))",
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute(
            "DELETE FROM sync_entities
             WHERE collection IN ('rateLimit','accounts','accountCost','sessionCost','desktop')",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observations_move_and_copies_go() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('rateLimit','default',999999,'h','{\"collection\":\"rateLimit\",\"id\":\"default\",\"value\":{\"a\":1}}',0),
               ('accountCost','gone',7,'h',NULL,1)",
            [],
        )
        .expect("old copies");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count(
                "SELECT COUNT(*) FROM sync_entities WHERE collection IN ('rateLimit','accountCost')"
            ),
            0
        );
        assert_eq!(count("SELECT COUNT(*) FROM runtime_rate_limits"), 1);
        assert_eq!(count("SELECT COUNT(*) FROM runtime_account_costs"), 0);
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='runtime_rate_limits' AND id='default'")
                > 999_999
        );
    }
}
