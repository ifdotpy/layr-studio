//! Work history entries (results, decisions, releases) as child rows of `runtime_work`.
//!
//! The work record keeps its three arrays. The child rows follow them: every write of a
//! work record reconciles the rows in the writer's transaction, and a migration copies the
//! arrays of old records and verifies the copy. A row id is `{work}:{index}`, the key the
//! published entry already used. A delete of the work row deletes its entries, and the
//! durable triggers of each child table write the tombstones.

use rusqlite::{Connection, Transaction};
use std::collections::BTreeMap;
use studio_db_api::{DbTable, MutableTable, Table, Tx, db_delete, db_query};
use studio_json::Value;

use crate::entity_publication::WorkHistoryKind;
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use crate::{StoreError, StoreResult};

trait HistoryRow: MutableTable<Database = Canvas, Key = (String,)> {
    fn from_raw(id: String, work: String, idx: i64, raw: String) -> StoreResult<Self>;
    fn raw(&self) -> &str;
    fn index(&self) -> i64;
    fn for_work(connection: &Connection, work: &str) -> rusqlite::Result<Vec<Self>>;
    fn trim(tx: &mut Tx<'_, Canvas>, work: &str, length: i64) -> rusqlite::Result<usize>;
}

// The old child-row writer admits every JSON shape. Object fields are open evidence,
// not a closed producer vocabulary; legacy scalar/array entries must remain readable.
pub enum HistoryEvidence {
    Object(BTreeMap<String, studio_json::OwnedRawJson>),
    Array(Vec<studio_json::OwnedRawJson>),
    Text(String),
    Number(studio_json::Number),
    Boolean(bool),
    Null,
}

impl<'de> serde::Deserialize<'de> for HistoryEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <studio_json::OwnedRawJson as serde::Deserialize>::deserialize(deserializer)?;
        let raw = raw.as_str();
        // Dispatch from the original token: untagged buffering cannot retain raw fields.
        match raw.trim_start().as_bytes()[0] {
            b'{' => studio_json::from_str(raw).map(Self::Object),
            b'[' => studio_json::from_str(raw).map(Self::Array),
            b'"' => studio_json::from_str(raw).map(Self::Text),
            b't' | b'f' => studio_json::from_str(raw).map(Self::Boolean),
            b'n' => Ok(Self::Null),
            _ => studio_json::from_str(raw).map(Self::Number),
        }
        .map_err(serde::de::Error::custom)
    }
}

macro_rules! history_table {
    ($row:ident, $payload:ident, $table:literal, $ddl:literal, $index:literal) => {
        pub struct $payload {
            pub evidence: HistoryEvidence,
        }
        impl<'de> serde::Deserialize<'de> for $payload {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self {
                    evidence: serde::Deserialize::deserialize(deserializer)?,
                })
            }
        }

        #[derive(DbTable)]
        #[db(table = $table, database = Canvas, legacy_ddl = $ddl, legacy_index = $index)]
        #[db(state_check = crate::state_check::count_legacy_rows::<$row>)]
        pub struct $row {
            #[db(key)]
            id: String,
            work: String,
            idx: i64,
            #[db(codec = ExactJsonCodec<$payload>)]
            record: ExactJson<$payload>,
        }

        impl HistoryRow for $row {
            fn from_raw(id: String, work: String, idx: i64, raw: String) -> StoreResult<Self> {
                Ok(Self {
                    id,
                    work,
                    idx,
                    record: ExactJson::from_stored(raw)?,
                })
            }
            fn raw(&self) -> &str {
                self.record.raw()
            }
            fn index(&self) -> i64 {
                self.idx
            }
            fn for_work(connection: &Connection, work: &str) -> rusqlite::Result<Vec<Self>> {
                let work = work.to_owned();
                db_query!(
                    canvas_read(connection),
                    $row,
                    filter($row::WORK.eq(&work)),
                    order_by($row::IDX.asc)
                )
            }
            fn trim(tx: &mut Tx<'_, Canvas>, work: &str, length: i64) -> rusqlite::Result<usize> {
                let work = work.to_owned();
                db_delete!(
                    *tx,
                    $row,
                    filter($row::WORK.eq(&work), $row::IDX.ge(&length))
                )
            }
        }
    };
}

history_table!(
    WorkResultsStorageRow,
    WorkResultRecord,
    "runtime_work_results",
    "CREATE TABLE runtime_work_results (id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL)",
    "CREATE INDEX runtime_work_results_work ON runtime_work_results(work,idx)"
);
history_table!(
    WorkDecisionsStorageRow,
    WorkDecisionRecord,
    "runtime_work_decisions",
    "CREATE TABLE runtime_work_decisions (id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL)",
    "CREATE INDEX runtime_work_decisions_work ON runtime_work_decisions(work,idx)"
);
history_table!(
    WorkReleasesStorageRow,
    WorkReleaseRecord,
    "runtime_work_releases",
    "CREATE TABLE runtime_work_releases (id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL)",
    "CREATE INDEX runtime_work_releases_work ON runtime_work_releases(work,idx)"
);

// Raw SQL: this cross-table delete cascade is not expressible as a single-table query.
const CASCADE: &str =
    "CREATE TRIGGER IF NOT EXISTS runtime_work_history_cascade AFTER DELETE ON runtime_work BEGIN
           DELETE FROM runtime_work_results WHERE work=OLD.id;
           DELETE FROM runtime_work_decisions WHERE work=OLD.id;
           DELETE FROM runtime_work_releases WHERE work=OLD.id;
         END;";

/// The child table that holds one history kind.
pub fn table_of(kind: WorkHistoryKind) -> &'static str {
    match kind {
        WorkHistoryKind::Result => WorkResultsStorageRow::NAME,
        WorkHistoryKind::Decision => WorkDecisionsStorageRow::NAME,
        WorkHistoryKind::Release => WorkReleasesStorageRow::NAME,
    }
}

/// Row id of one entry: `{work}:{index}`.
pub fn entry_id(work: &str, index: usize) -> String {
    format!("{work}:{index}")
}

/// Create the child tables, the cascade and their durable entries. Safe to repeat.
pub(crate) fn prepare_schema(tx: &Transaction<'_>) -> StoreResult<()> {
    for descriptor in [
        WorkResultsStorageRow::descriptor(),
        WorkDecisionsStorageRow::descriptor(),
        WorkReleasesStorageRow::descriptor(),
    ] {
        descriptor.install_if_absent(tx)?;
    }
    tx.execute_batch(CASCADE)?;
    crate::durable_seq::install(tx)
}

/// Replace the child rows of one work record with its arrays. Unchanged entries keep
/// their row, so they get no new change number.
pub fn reconcile(transaction: &Transaction<'_>, id: &str, record: &Value) -> StoreResult<()> {
    for kind in WorkHistoryKind::ALL {
        let entries: &[Value] = record
            .get(kind.field())
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice);
        match kind {
            WorkHistoryKind::Result => {
                reconcile_rows::<WorkResultsStorageRow>(transaction, id, entries)?
            }
            WorkHistoryKind::Decision => {
                reconcile_rows::<WorkDecisionsStorageRow>(transaction, id, entries)?
            }
            WorkHistoryKind::Release => {
                reconcile_rows::<WorkReleasesStorageRow>(transaction, id, entries)?
            }
        }
    }
    Ok(())
}

fn reconcile_rows<R: HistoryRow>(
    transaction: &Transaction<'_>,
    id: &str,
    entries: &[Value],
) -> StoreResult<()> {
    let mut tx = canvas_tx(transaction);
    for (index, entry) in entries.iter().enumerate() {
        let key = entry_id(id, index);
        let stored = tx.get::<R>(&(key.clone(),))?;
        let unchanged = stored
            .as_ref()
            .map(|row| studio_json::from_str::<Value>(row.raw()))
            .transpose()?
            .is_some_and(|stored| stored == *entry);
        if !unchanged {
            tx.upsert(&R::from_raw(
                key,
                id.to_owned(),
                index as i64,
                entry.to_string(),
            )?)?;
        }
    }
    R::trim(&mut tx, id, entries.len() as i64)?;
    Ok(())
}

/// The stored entries of one kind, in index order.
pub fn entries(
    connection: &Connection,
    id: &str,
    kind: WorkHistoryKind,
) -> StoreResult<Vec<(usize, Value)>> {
    match kind {
        WorkHistoryKind::Result => read_entries::<WorkResultsStorageRow>(connection, id),
        WorkHistoryKind::Decision => read_entries::<WorkDecisionsStorageRow>(connection, id),
        WorkHistoryKind::Release => read_entries::<WorkReleasesStorageRow>(connection, id),
    }
}

fn read_entries<R: HistoryRow>(
    connection: &Connection,
    id: &str,
) -> StoreResult<Vec<(usize, Value)>> {
    let rows = R::for_work(connection, id)?;
    rows.into_iter()
        .map(|row| {
            let index = usize::try_from(row.index())
                .map_err(|_| StoreError::RecordContract("work history index is negative"))?;
            Ok((index, studio_json::from_str(row.raw())?))
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
    let key = entry_id(id, index);
    match kind {
        WorkHistoryKind::Result => read_entry::<WorkResultsStorageRow>(connection, key),
        WorkHistoryKind::Decision => read_entry::<WorkDecisionsStorageRow>(connection, key),
        WorkHistoryKind::Release => read_entry::<WorkReleasesStorageRow>(connection, key),
    }
}

fn read_entry<R: HistoryRow>(connection: &Connection, key: String) -> StoreResult<Option<Value>> {
    canvas_read(connection)
        .get::<R>(&(key,))?
        .map(|row| studio_json::from_str(row.raw()).map_err(StoreError::from))
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
    // Raw SQL: parent history is a migration input, including invalid records repaired later.
    let rows = tx
        .prepare("SELECT id,record FROM runtime_work")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, raw) in rows {
        // A record that does not parse has no readable history; the typed decode reports it.
        let Ok(record) = studio_json::from_str::<Value>(&raw) else {
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
    use studio_json::json;

    #[test]
    fn work_history_descriptors_match_legacy_schema_and_admit_existing_json() {
        let db = Connection::open_in_memory().unwrap();
        // Pre-descriptor DDL: intentionally independent of schema_sql and the registry.
        for table in [
            "runtime_work_results",
            "runtime_work_decisions",
            "runtime_work_releases",
        ] {
            db.execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL);
                 CREATE INDEX IF NOT EXISTS {table}_work ON {table}(work,idx);"
            )).unwrap();
        }
        for descriptor in [
            WorkResultsStorageRow::descriptor(),
            WorkDecisionsStorageRow::descriptor(),
            WorkReleasesStorageRow::descriptor(),
        ] {
            let actual: String = db
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name=?1",
                    [descriptor.name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(actual, descriptor.ddl);
            for index in descriptor.indexes {
                let actual: String = db
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name=?1",
                        [index.name],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(actual, index.sql);
            }
            for (index, raw) in [
                r#" { "unknown": [null, 1.00], "text": "\u0061", "duplicate": 1, "duplicate": 2 } "#,
                "null",
                "[1,true]",
                "42",
                "false",
                r#""legacy""#,
            ]
            .into_iter()
            .enumerate()
            {
                db.execute(
                    &format!("INSERT INTO {} VALUES(?1,'old',?2,?3)", descriptor.name),
                    rusqlite::params![entry_id("old", index), index as i64, raw],
                )
                .unwrap();
            }
            let registration = crate::canvas_schema::registry()
                .into_iter()
                .find(|entry| entry.descriptor.name == descriptor.name)
                .unwrap();
            assert_eq!((registration.check)(&db).unwrap().counts(), (6, 0));
            db.execute(
                &format!(
                    "INSERT INTO {} VALUES(NULL,'old',6,'{{}}')",
                    descriptor.name
                ),
                [],
            )
            .unwrap();
            db.execute(
                &format!(
                    "INSERT INTO {} VALUES('old:7','old',7,'invalid JSON')",
                    descriptor.name
                ),
                [],
            )
            .unwrap();
            // These tables had no validator before descriptors; reports must stay lenient.
            assert_eq!((registration.check)(&db).unwrap().counts(), (8, 0));
        }
        for kind in WorkHistoryKind::ALL {
            assert_eq!(entry(&db, "old", kind, 0).unwrap().unwrap()["duplicate"], 2);
            assert_eq!(entry(&db, "old", kind, 1).unwrap(), Some(Value::Null));
            assert_eq!(entry(&db, "old", kind, 2).unwrap(), Some(json!([1, true])));
            assert_eq!(entry(&db, "old", kind, 5).unwrap(), Some(json!("legacy")));
            assert!(
                entry(&db, "old", kind, 7).is_err(),
                "typed reads still validate JSON"
            );
        }
    }

    #[test]
    fn unchanged_history_preserves_bytes_sequences_and_outer_rollback() {
        let db = crate::fresh_state::test_canvas();
        let tx = db.unchecked_transaction().unwrap();
        let original = work(
            json!([{"id":"r0","text":"a","unknown":[null,1.0]}]),
            json!([{"decision":"accept"}]),
        );
        let mut original = original;
        original["releases"] = json!([null, {"reason":"released"}]);
        WorkItemsRepository::put_work(&tx, "w", &original).unwrap();
        let raw = " \n {\"unknown\":[null,1.0],\"text\":\"\\u0061\",\"id\":\"r0\"} \t";
        tx.execute(
            "UPDATE runtime_work_results SET record=?1 WHERE id='w:0'",
            [raw],
        )
        .unwrap();
        tx.commit().unwrap();
        let before: i64 = db.query_row("SELECT max(seq) FROM durable_rows WHERE tbl IN ('runtime_work_results','runtime_work_decisions','runtime_work_releases')", [], |row| row.get(0)).unwrap();
        let tx = db.unchecked_transaction().unwrap();
        reconcile(&tx, "w", &original).unwrap();
        assert_eq!(stored_result(&tx), raw);
        let after: i64 = tx.query_row("SELECT max(seq) FROM durable_rows WHERE tbl IN ('runtime_work_results','runtime_work_decisions','runtime_work_releases')", [], |row| row.get(0)).unwrap();
        assert_eq!(
            before, after,
            "unchanged entries allocate no durable sequence"
        );
        WorkItemsRepository::update_work(&tx, "w", &work(json!([]), json!([]))).unwrap();
        for kind in WorkHistoryKind::ALL {
            assert!(entries(&tx, "w", kind).unwrap().is_empty());
            assert!(!tombstones(&tx, table_of(kind)).is_empty());
        }
        tx.rollback().unwrap();
        verify(&db, "w", &original).unwrap();
        for kind in WorkHistoryKind::ALL {
            assert!(tombstones(&db, table_of(kind)).is_empty());
        }
        assert_eq!(stored_result(&db), raw);
    }

    fn stored_result(db: &Connection) -> String {
        db.query_row(
            "SELECT record FROM runtime_work_results WHERE id='w:0'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

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
        let db = db.unchecked_transaction().unwrap();
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
        let db = db.unchecked_transaction().unwrap();
        let mut record = work(json!([{"id":"r0"}]), json!([{"decision":"accept"}]));
        record["releases"] = json!([{"reason":"released"}]);
        WorkItemsRepository::put_work(&db, "w", &record).unwrap();
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
        assert!(
            entries(&db, "w", WorkHistoryKind::Release)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            tombstones(&db, "runtime_work_releases"),
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
        let db = db.unchecked_transaction().unwrap();
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
