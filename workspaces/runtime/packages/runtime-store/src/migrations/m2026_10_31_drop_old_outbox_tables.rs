//! Drop the four outbox tables that `m2026_10_30_one_outbox` replaced with `runtime_outbox`.
//!
//! A table with no rows is dropped. A table with rows is kept and its row count is
//! logged, so no row is deleted here. Nothing reads the kept table for delivery.
use rusqlite::Transaction;

use crate::StoreResult;

const RETIRED_TABLES: [&str; 4] = [
    "runtime_server_outbox",
    "runtime_federation_outbox",
    "runtime_access_outbound",
    "runtime_server_fetch",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for table in RETIRED_TABLES {
        if !crate::schema::table_exists(tx, table)? {
            continue;
        }
        let rows: i64 = tx.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })?;
        if rows == 0 {
            tx.execute_batch(&format!("DROP TABLE {table}"))?;
        } else {
            eprintln!("old outbox table {table} kept: {rows} rows");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn old_tables(db: &Connection) {
        db.execute_batch(
            "CREATE TABLE runtime_server_outbox(id TEXT PRIMARY KEY);
             CREATE TABLE runtime_federation_outbox(id TEXT PRIMARY KEY, peer_id TEXT, record TEXT);
             CREATE TABLE runtime_access_outbound(server TEXT, request_id TEXT, hash TEXT, record TEXT);
             CREATE TABLE runtime_server_fetch(id TEXT PRIMARY KEY, server TEXT, signature TEXT, state TEXT, result TEXT, admission TEXT);
             INSERT INTO runtime_server_fetch VALUES('kept','peer','sig','fetching',NULL,'{}');",
        )
        .expect("old tables");
    }

    #[test]
    fn empty_old_tables_drop_and_a_table_with_rows_stays() {
        let mut db = Connection::open_in_memory().expect("database");
        old_tables(&db);
        let tx = db.transaction().unwrap();
        apply(&tx).expect("drop applies");
        tx.commit().unwrap();
        for table in [
            "runtime_server_outbox",
            "runtime_federation_outbox",
            "runtime_access_outbound",
        ] {
            assert!(!crate::schema::table_exists(&db, table).unwrap(), "{table}");
        }
        let kept: i64 = db
            .query_row("SELECT count(*) FROM runtime_server_fetch", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(kept, 1, "rows of a non-empty table are never deleted");
    }

    #[test]
    fn fresh_schema_without_the_old_tables_applies() {
        // The snapshot no longer lists the old tables, so this is a fresh store.
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::migrations::m2026_10_31_current_checks::apply(&tx).expect("current checks");
        apply(&tx).expect("drop applies to a fresh store");
        tx.commit().unwrap();
        for table in RETIRED_TABLES {
            assert!(!crate::schema::table_exists(&db, table).unwrap(), "{table}");
        }
    }
}
