//! A resolve command refers to an outbox entry, so `request_results` admits the `outbox`
//! reference kind. Its `status` and `ref_kind` values are checked by generated triggers
//! instead of a table CHECK: a new enum value then needs a trigger replacement, not a table
//! rebuild. This migration is the one rebuild. The copy runs in the migration transaction,
//! and the row count must match before the new table is kept.
use rusqlite::Transaction;
use studio_runtime_schema::persistence::{
    RequestReferenceKind, RequestResultStatus, enum_check, enum_check_triggers,
};

use super::m2026_10_31_current_checks::rebuild;
use crate::{StoreError, StoreResult};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "request_results")? {
        return Ok(());
    }
    let table_sql: String = tx.query_row(
        "SELECT sql FROM sqlite_schema WHERE type='table' AND name='request_results'",
        [],
        |row| row.get(0),
    )?;
    if !table_sql.contains(&enum_check::<RequestResultStatus>("status"))
        && !table_sql.contains(&enum_check::<RequestReferenceKind>("ref_kind"))
    {
        return Ok(());
    }
    let before: i64 = tx.query_row("SELECT count(*) FROM request_results", [], |row| row.get(0))?;
    let ddl = "CREATE TABLE request_results_current_checks (
           id TEXT PRIMARY KEY,
           signature TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'applied',
           ref_kind TEXT,
           ref_id TEXT,
           ref_rev INTEGER,
           result TEXT,
           created INTEGER NOT NULL,
           CHECK ((result IS NOT NULL AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)
               OR (result IS NULL AND ref_kind IS NOT NULL AND ref_id IS NOT NULL AND ref_rev IS NOT NULL)
               OR (result IS NULL AND status = 'rejected' AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)))";
    rebuild(
        tx,
        "request_results",
        ddl,
        "INSERT INTO request_results_current_checks(id,signature,status,ref_kind,ref_id,ref_rev,result,created)
         SELECT id,signature,status,ref_kind,ref_id,ref_rev,result,created FROM request_results ORDER BY rowid",
        &[],
    )?;
    let after: i64 = tx.query_row("SELECT count(*) FROM request_results", [], |row| row.get(0))?;
    if after != before {
        return Err(StoreError::RecordContract(
            "request_results copy changed the row count",
        ));
    }
    tx.execute_batch(&format!(
        "{}\n{}",
        enum_check_triggers::<RequestResultStatus>("request_results", "status"),
        enum_check_triggers::<RequestReferenceKind>("request_results", "ref_kind"),
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use studio_runtime_schema::persistence::enum_check_trigger_statements;

    /// The fresh-store snapshot carries exactly the generated triggers, so a new store and a
    /// migrated store check the same values.
    #[test]
    fn the_snapshot_holds_the_generated_request_result_triggers() {
        let snapshot: serde_json::Value =
            serde_json::from_str(include_str!("../runtime-state-schema.json")).unwrap();
        let objects = snapshot["canvas.sqlite3"]["sqlite_master"]
            .as_array()
            .unwrap();
        let mut expected =
            enum_check_trigger_statements::<RequestResultStatus>("request_results", "status")
                .to_vec();
        expected.extend(enum_check_trigger_statements::<RequestReferenceKind>(
            "request_results",
            "ref_kind",
        ));
        for (name, sql) in expected {
            let stored = objects
                .iter()
                .find(|object| {
                    object["type"] == "trigger" && object["name"] == name.trim_matches('"')
                })
                .unwrap_or_else(|| panic!("trigger {name} is missing from the snapshot"));
            assert_eq!(stored["sql"], sql, "{name}");
        }
    }

    #[test]
    fn old_rows_are_kept_and_a_new_kind_is_checked_by_a_trigger() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!(
            "CREATE TABLE request_results (
               id TEXT PRIMARY KEY, signature TEXT NOT NULL,
               status TEXT NOT NULL DEFAULT 'applied' {status},
               ref_kind TEXT {kind}, ref_id TEXT, ref_rev INTEGER, result TEXT, created INTEGER NOT NULL,
               CHECK ((result IS NOT NULL AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)
                   OR (result IS NULL AND ref_kind IS NOT NULL AND ref_id IS NOT NULL AND ref_rev IS NOT NULL)
                   OR (result IS NULL AND status = 'rejected' AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)));
             INSERT INTO request_results VALUES('a','s','applied','agent','x',1,NULL,1);
             INSERT INTO request_results VALUES('b','s','applied',NULL,NULL,NULL,'{{}}',2);",
            status = enum_check::<RequestResultStatus>("status"),
            kind = enum_check::<RequestReferenceKind>("ref_kind"),
        ))
        .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let count: i64 = tx
            .query_row("SELECT count(*) FROM request_results", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "every row is kept");
        tx.execute(
            "INSERT INTO request_results VALUES('c','s','applied','outbox','e',1,NULL,3)",
            [],
        )
        .expect("outbox is a valid kind");
        let rejected = tx.execute(
            "INSERT INTO request_results VALUES('d','s','applied','bogus','e',1,NULL,4)",
            [],
        );
        assert!(
            rejected.is_err(),
            "a value outside the enum is rejected by the trigger"
        );
        let rejected_update = tx.execute(
            "UPDATE request_results SET ref_kind='bogus' WHERE id='a'",
            [],
        );
        assert!(
            rejected_update.is_err(),
            "an update outside the enum is rejected"
        );
    }
}
