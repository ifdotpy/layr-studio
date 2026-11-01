//! Work history entries (results, decisions, releases) as child rows of `runtime_work`.
//!
//! The work record keeps its three arrays. The child rows follow them: every write of a
//! work record reconciles the rows in the writer's transaction, and a migration copies the
//! arrays of old records and verifies the copy. A row id is `{work}:{index}`, the key the
//! published entry already used. A delete of the work row deletes its entries, and the
//! durable triggers of each child table write the tombstones.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::entity_publication::WorkHistoryKind;
use crate::{StoreError, StoreResult};

/// The child table that holds one history kind.
pub fn table_of(kind: WorkHistoryKind) -> &'static str {
    match kind {
        WorkHistoryKind::Result => "runtime_work_results",
        WorkHistoryKind::Decision => "runtime_work_decisions",
        WorkHistoryKind::Release => "runtime_work_releases",
    }
}

/// Row id of one entry: `{work}:{index}`.
pub fn entry_id(work: &str, index: usize) -> String {
    format!("{work}:{index}")
}

/// DDL of the child tables and the delete cascade. Owner of the row shape.
fn schema_sql() -> String {
    let mut sql = String::new();
    for kind in WorkHistoryKind::ALL {
        let table = table_of(kind);
        sql.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS {table}_work ON {table}(work,idx);"
        ));
    }
    sql.push_str(
        "CREATE TRIGGER IF NOT EXISTS runtime_work_history_cascade AFTER DELETE ON runtime_work BEGIN
           DELETE FROM runtime_work_results WHERE work=OLD.id;
           DELETE FROM runtime_work_decisions WHERE work=OLD.id;
           DELETE FROM runtime_work_releases WHERE work=OLD.id;
         END;",
    );
    sql
}

/// Create the child tables, the cascade and their durable entries. Safe to repeat.
pub(crate) fn prepare_schema(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch(&schema_sql())?;
    crate::durable_seq::install(tx)
}

/// Replace the child rows of one work record with its arrays. Unchanged entries keep
/// their row, so they get no new change number.
pub fn reconcile(connection: &Connection, id: &str, record: &Value) -> StoreResult<()> {
    for kind in WorkHistoryKind::ALL {
        let table = table_of(kind);
        let entries: &[Value] = record
            .get(kind.field())
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice);
        for (index, entry) in entries.iter().enumerate() {
            let key = entry_id(id, index);
            let stored: Option<String> = connection
                .query_row(
                    &format!("SELECT record FROM {table} WHERE id=?1"),
                    [&key],
                    |row| row.get(0),
                )
                .optional()?;
            let unchanged = match stored {
                Some(raw) => serde_json::from_str::<Value>(&raw)? == *entry,
                None => false,
            };
            if !unchanged {
                connection.execute(
                    &format!(
                        "INSERT INTO {table}(id,work,idx,record) VALUES(?1,?2,?3,?4)
                         ON CONFLICT(id) DO UPDATE SET work=excluded.work,idx=excluded.idx,record=excluded.record"
                    ),
                    params![key, id, index as i64, entry.to_string()],
                )?;
            }
        }
        connection.execute(
            &format!("DELETE FROM {table} WHERE work=?1 AND idx>=?2"),
            params![id, entries.len() as i64],
        )?;
    }
    Ok(())
}

/// The stored entries of one kind, in index order.
pub fn entries(
    connection: &Connection,
    id: &str,
    kind: WorkHistoryKind,
) -> StoreResult<Vec<(usize, Value)>> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT idx,record FROM {} WHERE work=?1 ORDER BY idx",
        table_of(kind)
    ))?;
    let rows = statement
        .query_map([id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(index, raw)| {
            let index = usize::try_from(index)
                .map_err(|_| StoreError::RecordContract("work history index is negative"))?;
            Ok((index, serde_json::from_str::<Value>(&raw)?))
        })
        .collect()
}

/// One stored entry, or None when the index is absent.
pub fn entry(
    connection: &Connection,
    id: &str,
    kind: WorkHistoryKind,
    index: usize,
) -> StoreResult<Option<Value>> {
    let raw: Option<String> = connection
        .query_row(
            &format!("SELECT record FROM {} WHERE id=?1", table_of(kind)),
            [entry_id(id, index)],
            |row| row.get(0),
        )
        .optional()?;
    raw.map(|raw| serde_json::from_str::<Value>(&raw).map_err(StoreError::from))
        .transpose()
}

/// Check that the rows of one work record equal its arrays, in order.
pub(crate) fn verify(connection: &Connection, id: &str, record: &Value) -> StoreResult<()> {
    for kind in WorkHistoryKind::ALL {
        let expected: &[Value] = record
            .get(kind.field())
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice);
        let stored = entries(connection, id, kind)?;
        let same = stored.len() == expected.len()
            && stored.iter().zip(expected).enumerate().all(
                |(position, ((index, value), expected))| *index == position && value == expected,
            );
        if !same {
            return Err(StoreError::RecordContract(
                "work history rows differ from the work record",
            ));
        }
    }
    Ok(())
}

/// Migration step: copy the arrays of every work record into the child rows, then verify
/// each record. Nothing is deleted from `runtime_work`.
pub(crate) fn migrate(tx: &Transaction<'_>) -> StoreResult<()> {
    prepare_schema(tx)?;
    if !crate::schema::table_exists(tx, "runtime_work")? {
        return Ok(());
    }
    let rows = tx
        .prepare("SELECT id,record FROM runtime_work")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, raw) in rows {
        // A record that does not parse has no readable history; the typed decode reports it.
        let Ok(record) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if record.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            continue;
        }
        reconcile(tx, &id, &record)?;
        verify(tx, &id, &record)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable_seq::changes_since;
    use crate::repositories::work_items::WorkItemsRepository;
    use serde_json::json;

    fn work(results: Value, decisions: Value) -> Value {
        let mut value = json!({"id":"w","rootId":"lead","title":"t","status":"review","created":1,"updated":1,
               "version":1,"dependencies":[],"results":[],"decisions":[],"releases":[]});
        value["results"] = results;
        value["decisions"] = decisions;
        value
    }

    fn tombstones(db: &Connection, table: &str) -> Vec<String> {
        db.prepare("SELECT id FROM durable_rows WHERE tbl=?1 AND deleted=1 ORDER BY id")
            .unwrap()
            .query_map([table], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn writes_reconcile_rows_and_shorter_history_leaves_a_tombstone() {
        let db = crate::fresh_state::test_canvas();
        WorkItemsRepository::put_work(&db, "w", &work(json!([{"id":"r0"},{"id":"r1"}]), json!([])))
            .unwrap();
        assert_eq!(entries(&db, "w", WorkHistoryKind::Result).unwrap().len(), 2);
        WorkItemsRepository::update_work(
            &db,
            "w",
            &work(json!([{"id":"r0","text":"changed"}]), json!([])),
        )
        .unwrap();
        let rows = entries(&db, "w", WorkHistoryKind::Result).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1["text"], "changed");
        assert_eq!(
            tombstones(&db, "runtime_work_results"),
            vec!["w:1".to_owned()]
        );
    }

    #[test]
    fn deleting_the_work_row_deletes_its_entries() {
        let db = crate::fresh_state::test_canvas();
        WorkItemsRepository::put_work(
            &db,
            "w",
            &work(json!([{"id":"r0"}]), json!([{"decision":"accept"}])),
        )
        .unwrap();
        db.execute("DELETE FROM runtime_work WHERE id='w'", [])
            .unwrap();
        assert!(
            entries(&db, "w", WorkHistoryKind::Result)
                .unwrap()
                .is_empty()
        );
        assert!(
            entries(&db, "w", WorkHistoryKind::Decision)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            tombstones(&db, "runtime_work_decisions"),
            vec!["w:0".to_owned()]
        );
    }

    #[test]
    fn migration_copy_equals_the_arrays_and_is_repeatable() {
        let mut db = crate::fresh_state::test_canvas();
        db.execute(
            "INSERT INTO runtime_work(id,record) VALUES('old',?1)",
            [work(
                json!([{"id":"a"},{"id":"b"}]),
                json!([{"decision":"accept"}]),
            )
            .to_string()
            .replace("\"id\":\"w\"", "\"id\":\"old\"")],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        migrate(&tx).unwrap();
        migrate(&tx).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            entries(&db, "old", WorkHistoryKind::Result).unwrap().len(),
            2
        );
        assert_eq!(
            entries(&db, "old", WorkHistoryKind::Decision)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn child_rows_have_durable_entries_for_the_stream() {
        let db = crate::fresh_state::test_canvas();
        WorkItemsRepository::put_work(&db, "w", &work(json!([{"id":"r0"}]), json!([]))).unwrap();
        let crate::durable_seq::DurableChanges::Snapshot { rows, .. } =
            changes_since(&db, "runtime_work_results", None).unwrap()
        else {
            panic!("a cursor-less read is a snapshot");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "w:0");
    }
}
