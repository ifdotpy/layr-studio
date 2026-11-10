//! Read persisted monitor records.

use rusqlite::{Connection, OptionalExtension};

use crate::StoreResult;
use crate::item_monitor_terminal::MonitorsRow;
use crate::records::{MonitorRecord, Table};
use crate::schema::{SchemaRequirements, TableRequirement};
use studio_db_api::Table as _;

pub use studio_runtime_schema::monitor_record::{
    MonitorOperationEvidence, MonitorOperationRecord as MonitorOperation,
};

/// Compatibility name for callers that used the pre-typed monitor wrapper.
pub type StoredMonitor = MonitorRecord;

const MONITOR_TABLE: TableRequirement = TableRequirement {
    name: "runtime_monitors",
    columns: &["id", "record"],
};

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[MONITOR_TABLE],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

/// Partial index of the monitors whose durable result stream holds an
/// undelivered event. The monitor tick reads the stream head once per pass and
/// once per delivered event; without this index that read parsed every
/// historical monitor (93,650 rows, 209 MB, about 0.55 s on 22 October 2026).
const QUEUED_STREAM_INDEX: &str = "runtime_monitor_queued_stream";

/// SQL predicate of `QUEUED_STREAM_INDEX` and its queue-head query.
const QUEUED_STREAM_PREDICATE: &str = "stream_pending=1";

/// Partial index, by creation time, of the monitors that may be a
/// Studio-caused ending. The Studio error list reads them in nearly every
/// Canvas writer transaction; through the status index that read parsed 6,771
/// records (17 MB) on 25 October 2026, mostly commands that ended with their
/// own exit code, to keep about 680.
const STUDIO_ERROR_INDEX: &str = "runtime_monitor_studio_error_v2";
/// Retired derived index; its predicate excluded malformed and unclassified failed records.
const LEGACY_STUDIO_ERROR_INDEX: &str = "runtime_monitor_studio_error";

const CREATED_EXPRESSION: &str = "monitor_created";
const STUDIO_ERROR_PREDICATE: &str = "monitor_error_candidate=1";

// Raw SQL: the indexed UNION preserves malformed repair candidates with a narrow projection.
fn studio_error_candidates_sql() -> String {
    let predicate = STUDIO_ERROR_PREDICATE;
    format!(
        "SELECT id,record FROM runtime_monitors WHERE {predicate} \
         AND {CREATED_EXPRESSION}>=?1 UNION ALL \
         SELECT id,record FROM runtime_monitors WHERE {predicate} AND {CREATED_EXPRESSION}=0"
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MonitorsRepository;

/// Durable diagnostic identity also holds the original monitor and its receipts.
/// Dismissing a diagnostic does not authorize replay or release the hold.
pub const MONITOR_HOLD_PREFIX: &str = "monitor-hold:";

/// SQL for autonomous scans only. Direct reads retain access to held evidence.
pub fn unheld_sql(id_column: &str) -> String {
    format!(
        "NOT EXISTS(SELECT 1 FROM runtime_error_incidents h WHERE h.id='{MONITOR_HOLD_PREFIX}'||{id_column})"
    )
}

impl MonitorsRepository {
    /// Holds survive dismissal and restart; only explicit repair may release them.
    pub fn is_held(connection: &Connection, id: &str) -> StoreResult<bool> {
        Ok(connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_error_incidents WHERE id=?1)",
            [format!("{MONITOR_HOLD_PREFIX}{id}")],
            |row| row.get(0),
        )?)
    }

    pub fn get(connection: &Connection, id: &str) -> StoreResult<Option<MonitorRecord>> {
        Table::<MonitorRecord>::new(connection).get(&id.to_owned())
    }

    /// Create the queued-stream and Studio-error partial indexes on a state
    /// that has monitors.
    /// `IF NOT EXISTS` makes a reopen a no-op; the guarded predicate lets the
    /// build pass over malformed rows of an older state.
    pub(crate) fn prepare_indexes(connection: &Connection) -> StoreResult<()> {
        if crate::schema::table_exists(connection, MONITOR_TABLE.name)? {
            for index in MonitorsRow::INDEXES
                .iter()
                .filter(|index| matches!(index.name, QUEUED_STREAM_INDEX | STUDIO_ERROR_INDEX))
            {
                connection.execute_batch(&index.sql.replacen(
                    "CREATE INDEX ",
                    "CREATE INDEX IF NOT EXISTS ",
                    1,
                ))?;
            }
            connection
                .execute_batch(&format!("DROP INDEX IF EXISTS {LEGACY_STUDIO_ERROR_INDEX}"))?;
        }
        Ok(())
    }

    /// `(id, record)` of every monitor created at or after `since` (Unix
    /// seconds) that may be a Studio-caused ending, read through
    /// `STUDIO_ERROR_INDEX`. The caller classifies each row exactly.
    pub fn studio_error_candidates(
        connection: &Connection,
        since: f64,
    ) -> StoreResult<Vec<(String, String)>> {
        Ok(connection
            .prepare_cached(&studio_error_candidates_sql())?
            .query_map([since], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Raw changed monitor for classification by the Studio-error owner.
    pub fn studio_error_candidate(
        connection: &Connection,
        id: &str,
    ) -> StoreResult<Option<String>> {
        // Raw SQL: the exact stored scalar is shorter here and admits malformed repair evidence.
        Ok(connection
            .prepare_cached("SELECT record FROM runtime_monitors WHERE id=?1")?
            .query_row([id], |row| row.get(0))
            .optional()?)
    }

    /// `(id, record)` of the first monitor, by id, whose durable result stream
    /// (`streamQueue`) holds an undelivered event, whatever its status. The
    /// queued-stream index bounds this to monitors with pending events.
    // Raw SQL: this tuple projection is shorter than a separate generated projection owner.
    pub fn first_queued_stream(connection: &Connection) -> StoreResult<Option<(String, String)>> {
        Ok(connection
            .prepare_cached(&format!(
                "SELECT id,record FROM runtime_monitors WHERE {QUEUED_STREAM_PREDICATE} AND {} \
                 ORDER BY id LIMIT 1",
                unheld_sql("runtime_monitors.id")
            ))?
            .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?)
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "monitors",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::{MonitorsRepository, QUEUED_STREAM_INDEX, STUDIO_ERROR_INDEX};
    use rusqlite::Connection;

    #[tokio::test]
    async fn store_open_rejects_a_missing_monitor_table() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(directory.path()))
            .await
            .unwrap();
        drop(store);
        let db = Connection::open(directory.path().join("canvas.sqlite3")).unwrap();
        db.execute_batch("DROP TABLE runtime_monitors").unwrap();
        drop(db);
        assert!(matches!(
            crate::RuntimeStore::open(Some(directory.path())).await,
            Err(crate::StoreError::SchemaObjectMismatch { object, .. }) if object == "runtime_monitors"
        ));
    }

    #[tokio::test]
    async fn typed_monitor_writes_update_plain_queued_stream_index() {
        let dir = tempfile::tempdir().expect("state");
        let store = crate::RuntimeStore::open(Some(dir.path()))
            .await
            .expect("store");
        store.transaction(|tx| {
            let raw = r#"{"id":"m","agent":"a","epoch":1,"requestId":"r","status":"completed","activityAt":1,"activityGeneration":0,"bytes":0,"command":"","created":1,"cwd":"/","interactive":false,"livenessCommand":"","log":"/log","stallTimeoutSeconds":0,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":1000,"wakeOn":"exit","streamQueue":[{"kind":"exit","cursor":"result:m","code":0,"error":null,"finished":1,"byteLength":0,"operation":{"agent":"a","epoch":1,"accountKey":"default","connectionId":"c"}}]}"#;
            let mut monitor: super::MonitorRecord = studio_json::from_str(raw)?;
            crate::Table::<super::MonitorRecord>::new(tx).put(&monitor)?;
            let sql: String = tx.query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [QUEUED_STREAM_INDEX], |row| row.get(0))?;
            assert!(!sql.contains("json_"), "queue index must use a plain column");
            assert!(MonitorsRepository::first_queued_stream(tx)?.is_some());
            monitor.stream_queue = None;
            assert!(crate::Table::<super::MonitorRecord>::new(tx).update(&monitor)?);
            assert!(MonitorsRepository::first_queued_stream(tx)?.is_none());
            monitor.status = studio_runtime_schema::studio_errors::MonitorStatus::Running;
            assert!(crate::Table::<super::MonitorRecord>::new(tx).update(&monitor)?);
            let sql: String = tx.query_row("SELECT sql FROM sqlite_schema WHERE name='runtime_monitor_startup_active'", [], |row| row.get(0))?;
            assert!(!sql.contains("json_"), "active monitor index must use a plain status column");
            assert_eq!(crate::StartupRecoveryRepository::active_monitors(tx)?.len(), 1);
            let sql: String = tx.query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [STUDIO_ERROR_INDEX], |row| row.get(0))?;
            assert!(!sql.contains("json_"), "studio-error index must use plain columns");
            assert!(MonitorsRepository::studio_error_candidates(tx, 0.0)?.is_empty());
            monitor.status = studio_runtime_schema::studio_errors::MonitorStatus::Lost;
            assert!(crate::Table::<super::MonitorRecord>::new(tx).update(&monitor)?);
            assert_eq!(MonitorsRepository::studio_error_candidates(tx, 0.0)?.len(), 1);
            Ok::<_, crate::StoreError>(())
        }).await.expect("typed writes");
    }

    #[test]
    fn single_studio_error_candidate_does_not_parse_json_in_sql() {
        thread_local! { static JSON_SQL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        let connection = crate::fresh_state::test_canvas();
        connection.execute_batch(" INSERT INTO runtime_monitors(id,record) VALUES('m','{\"status\":\"lost\",\"created\":100}');").expect("monitor");
        connection.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Stmt(_, sql) = event
                    && sql.contains("json_")
                {
                    JSON_SQL.set(true);
                }
            }),
        );
        assert!(
            MonitorsRepository::studio_error_candidate(&connection, "m")
                .expect("candidate")
                .is_some()
        );
        assert!(
            !JSON_SQL.get(),
            "single-monitor lookup should not parse JSON in SQLite"
        );
    }

    #[test]
    fn monitor_rows_check_identity_and_discard_unknown_fields() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY, record TEXT NOT NULL)",
        )
        .unwrap();
        let raw = r#"{"id":"m","agent":"a","epoch":1,"requestId":"r","turnId":null,"status":"running","activityAt":1,"activityGeneration":0,"bytes":0,"command":"","created":1,"cwd":"/","interactive":false,"livenessCommand":"","log":"/log","stallTimeoutSeconds":0,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":1000,"wakeOn":"exit","future":true}"#;
        db.execute(
            "INSERT INTO runtime_monitors(id,record) VALUES ('m',?1)",
            [raw],
        )
        .unwrap();
        let saved = MonitorsRepository::get(&db, "m").unwrap().unwrap();
        assert!(
            !studio_json::to_string(&saved)
                .unwrap()
                .contains("\"future\"")
        );
        for invalid in [
            raw.replace("\"id\":\"m\"", "\"id\":\"wrong\""),
            raw.replace("\"status\":\"running\"", "\"status\":\"unknown\""),
            raw.replace("\"id\":\"m\"", "\"id\":\"m\",\"id\":\"wrong\""),
        ] {
            db.execute("UPDATE runtime_monitors SET record=?1", [&invalid])
                .unwrap();
            assert!(MonitorsRepository::get(&db, "m").is_err());
        }
    }

    #[test]
    fn historical_thread_is_readable_but_does_not_change_operation_identity() {
        let base = r#"{"agent":"a","epoch":1,"accountKey":"default","connectionId":"c"}"#;
        let historical = r#"{"agent":"a","epoch":1,"accountKey":"default","connectionId":"c","threadId":"thread"}"#;
        let original: super::MonitorOperation = studio_json::from_str(base).unwrap();
        let saved: super::MonitorOperation = studio_json::from_str(historical).unwrap();
        assert!(original.same_identity(&saved));
        assert_eq!(
            crate::projection_value(&saved),
            studio_json::from_str::<studio_json::Value>(historical).unwrap()
        );
        assert_eq!(
            studio_json::from_str::<super::MonitorOperation>(r#"{"agent":"a","epoch":1,"accountKey":"default","connectionId":"c","threadId":null}"#).unwrap(),
            original
        );
        assert!(studio_json::from_str::<super::MonitorOperation>(r#"{"agent":"a","epoch":1,"accountKey":"default","connectionId":"c","unexpected":true}"#).is_ok());
    }

    #[test]
    fn operation_evidence_rejects_unknown_fields_directly_and_when_flattened() {
        use serde::Deserialize;
        use studio_json::json;

        let evidence = json!({"threadId":"t", "rootRequestId":"r", "providerGeneration":3,
            "providerRequest":{"id":"i", "signature":"s", "created":1}});
        assert!(super::MonitorOperationEvidence::deserialize(evidence.clone()).is_ok());
        let mut invalid = evidence.clone();
        invalid["unexpected"] = json!(true);
        assert!(super::MonitorOperationEvidence::deserialize(invalid.clone()).is_ok());
        let mut operation =
            json!({"agent":"a", "epoch":1, "accountKey":"default", "connectionId":"c"});
        operation
            .as_object_mut()
            .unwrap()
            .extend(evidence.as_object().unwrap().clone());
        assert!(super::MonitorOperation::deserialize(operation.clone()).is_ok());
        operation["unexpected"] = json!(true);
        assert!(super::MonitorOperation::deserialize(operation).is_ok());
    }

    /// The index build runs at every store open, also on older states that
    /// may hold malformed monitor rows. It must succeed there, be a no-op when
    /// repeated, and serve the stream read without refusing the valid rows.
    #[test]
    fn queued_stream_index_builds_over_malformed_rows_and_serves_the_stream_read() {
        let connection = Connection::open_in_memory().unwrap();
        use studio_db_api::Table;
        connection
            .execute_batch(crate::item_monitor_terminal::MonitorsRow::DDL)
            .unwrap();
        crate::receipt_schema::create(&connection).unwrap();
        connection
            .execute_batch(
                "
                 INSERT INTO runtime_monitors(id,record) VALUES
                   ('a-malformed','{'),
                   ('b-done','{\"status\":\"completed\",\"streamQueue\":[]}'),
                   ('c-other','{\"status\":\"completed\",\"streamQueue\":{}}'),
                   ('d-queued','{\"status\":\"completed\",\"streamQueue\":[{\"cursor\":\"result:d\"}]}');",
            )
            .expect("monitor rows");
        MonitorsRepository::prepare_indexes(&connection).expect("index builds");
        MonitorsRepository::prepare_indexes(&connection).expect("repeated build is a no-op");
        let indexes: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                [QUEUED_STREAM_INDEX],
                |row| row.get(0),
            )
            .expect("catalog");
        assert_eq!(indexes, 1);
        let indexed: i64 = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM runtime_monitors INDEXED BY {QUEUED_STREAM_INDEX} WHERE {}", super::QUEUED_STREAM_PREDICATE),
                [],
                |row| row.get(0),
            )
            .expect("indexed rows");
        assert_eq!(
            indexed, 1,
            "only the monitor with a pending event is indexed"
        );
        let head = MonitorsRepository::first_queued_stream(&connection).expect("stream read");
        assert_eq!(head.map(|(id, _)| id).as_deref(), Some("d-queued"));
        connection
            .execute(
                "UPDATE runtime_monitors SET record='{\"streamQueue\":[]}' WHERE id='d-queued'",
                [],
            )
            .expect("acknowledge");
        assert!(
            MonitorsRepository::first_queued_stream(&connection)
                .expect("stream read")
                .is_none()
        );
    }

    /// The Studio error list runs this read in nearly every Canvas writer
    /// transaction. On 25 October 2026 it parsed 6,771 records (17 MB) per
    /// call, mostly commands that ended with their own exit code, to keep
    /// about 680. It must select only rows the classifier can accept, through
    /// a narrow index search, and a malformed row must not refuse it.
    #[test]
    fn studio_error_candidates_skip_own_exits_and_use_their_index() {
        let connection = Connection::open_in_memory().unwrap();
        use studio_db_api::Table;
        connection
            .execute_batch(crate::item_monitor_terminal::MonitorsRow::DDL)
            .unwrap();
        connection
            .execute_batch(
                r#"
                 INSERT INTO runtime_monitors(id,record) VALUES
                   ('a-malformed','{'),
                   ('b-lost','{"status":"lost","created":100,"exitCode":null}'),
                   ('c-lost-old','{"status":"lost","created":10}'),
                   ('d-not-recorded','{"status":"failed","created":100,"exitCode":null,"error":"Server restarted"}'),
                   ('e-no-exit-key','{"status":"failed","created":100,"error":"No live provider session","ruleId":"r"}'),
                   ('f-own-exit','{"status":"failed","created":100,"exitCode":1,"error":"","ruleId":"r"}'),
                   ('g-own-exit-error','{"status":"failed","created":100,"exitCode":0,"error":"boom"}'),
                   ('h-empty-error','{"status":"failed","created":100,"exitCode":null,"error":""}'),
                   ('i-no-error','{"status":"failed","created":100,"exitCode":null}'),
                   ('j-number-error','{"status":"failed","created":100,"exitCode":null,"error":7}'),
                   ('k-completed','{"status":"completed","created":100,"exitCode":null,"error":"x"}'),
                   ('l-running','{"status":"running","created":100}');"#,
            )
            .expect("monitor rows");
        MonitorsRepository::prepare_indexes(&connection).expect("index builds");
        MonitorsRepository::prepare_indexes(&connection).expect("repeated build is a no-op");
        let mut ids = MonitorsRepository::studio_error_candidates(&connection, 50.0)
            .expect("candidate read")
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(
            ids,
            [
                "a-malformed",
                "b-lost",
                "d-not-recorded",
                "e-no-exit-key",
                "h-empty-error",
                "i-no-error",
                "j-number-error"
            ]
        );
        let plan = connection
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                super::studio_error_candidates_sql()
            ))
            .expect("plan")
            .query_map([50.0], |row| row.get::<_, String>(3))
            .expect("plan rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows");
        assert_eq!(
            plan.iter()
                .filter(|step| step.contains(STUDIO_ERROR_INDEX))
                .count(),
            2,
            "both dated and undated ranges use the derived index: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|step| step.contains("SCAN runtime_monitors")),
            "{plan:?}"
        );
    }
}
