//! Supervisor's independent physical contracts; no transaction or durability policy lives here.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::supervisor_journal::JournalEventKind;
use studio_db_api::{Codec, Database, DbTable, db_schema};
use studio_runtime_providers::supervisor_protocol::NativeId;

pub struct Supervisor {
    _private: (),
}
impl Database for Supervisor {}

// These payloads are open protocol evidence, retained without parsing or rewriting their tokens.
#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct SupervisorInitResult(pub studio_json::OwnedRawJson);
impl<'de> serde::Deserialize<'de> for SupervisorInitResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = studio_json::OwnedRawJson::deserialize(deserializer)?;
        validate_legacy_json(&raw).map_err(serde::de::Error::custom)?;
        Ok(Self(raw))
    }
}

/// Keep the old readers' finite-number and nesting limits without materializing JSON.
/// Event payloads deliberately do not use this admission boundary.
pub(crate) fn validate_legacy_json(raw: &studio_json::OwnedRawJson) -> studio_json::Result<()> {
    struct LegacyJson;
    impl<'de> serde::Deserialize<'de> for LegacyJson {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = LegacyJson;
                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("legacy supervisor JSON")
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<LegacyJson, E> {
                    Ok(LegacyJson)
                }
                fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<LegacyJson, E> {
                    Ok(LegacyJson)
                }
                fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<LegacyJson, E> {
                    Ok(LegacyJson)
                }
                fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<LegacyJson, E> {
                    Ok(LegacyJson)
                }
                fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<LegacyJson, E> {
                    Ok(LegacyJson)
                }
                fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<LegacyJson, E> {
                    if value.is_finite() {
                        Ok(LegacyJson)
                    } else {
                        Err(E::custom("number out of range"))
                    }
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut sequence: A,
                ) -> Result<LegacyJson, A::Error> {
                    while sequence.next_element::<LegacyJson>()?.is_some() {}
                    Ok(LegacyJson)
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<LegacyJson, A::Error> {
                    match map.next_key::<String>()? {
                        Some(key) if key == studio_json::RAW_VALUE_TOKEN => {
                            let embedded = map.next_value::<String>()?;
                            studio_json::from_str::<LegacyJson>(&embedded)
                                .map_err(serde::de::Error::custom)?;
                        }
                        Some(_) => {
                            map.next_value::<LegacyJson>()?;
                            while map.next_entry::<String, LegacyJson>()?.is_some() {}
                        }
                        None => {}
                    }
                    Ok(LegacyJson)
                }
            }
            deserializer.deserialize_any(Visitor)
        }
    }
    studio_json::from_str::<LegacyJson>(raw.as_str()).map(|_| ())
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct SupervisorOperationResponse(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct SupervisorEventPayload(pub studio_json::OwnedRawJson);

// Keys are open: values may be journal identities, recovery JSON, or unknown legacy text.
// Keep the stored text even when it is not JSON; interpretation belongs to the key's reader.
pub struct SupervisorStateValue(pub String);
pub struct StateValueCodec;
impl Codec<SupervisorStateValue> for StateValueCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a SupervisorStateValue,
    ) -> rusqlite::Result<&'a str> {
        Ok(&value.0)
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<SupervisorStateValue> {
        Ok(SupervisorStateValue(value))
    }
}

// SQLite's legacy INTEGER affinity also admits text IDs; retain the storage class.
pub struct NativeIdStorage(rusqlite::types::Value);
impl rusqlite::types::FromSql for NativeIdStorage {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        <rusqlite::types::Value as rusqlite::types::FromSql>::column_result(value).map(Self)
    }
}
impl rusqlite::ToSql for NativeIdStorage {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl studio_db_api::DbValue for NativeIdStorage {
    const SQL_TYPE: &'static str = "INTEGER";
    const NULLABLE: bool = true;
}
pub struct NativeIdCodec;
impl Codec<Option<NativeId>> for NativeIdCodec {
    type Stored = NativeIdStorage;
    type Encoded<'a> = rusqlite::types::Value;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a Option<NativeId>,
    ) -> rusqlite::Result<Self::Encoded<'a>> {
        Ok(value
            .as_ref()
            .map(crate::supervisor_journal::native_sql)
            .unwrap_or(rusqlite::types::Value::Null))
    }
    fn decode(_: &rusqlite::Connection, value: Self::Stored) -> rusqlite::Result<Option<NativeId>> {
        crate::supervisor_journal::stored_native_id(rusqlite::types::ValueRef::from(&value.0))
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
    }
}

pub struct EventKindCodec;
impl Codec<JournalEventKind> for EventKindCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a JournalEventKind,
    ) -> rusqlite::Result<&'a str> {
        Ok(value.as_str())
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<JournalEventKind> {
        JournalEventKind::parse(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }
}

#[derive(DbTable)]
#[db(table = "handles", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<HandlesRow>,
    legacy_ddl = "CREATE TABLE handles(\n                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, pid INTEGER NOT NULL,\n                    sequence INTEGER NOT NULL DEFAULT 0, acknowledged INTEGER NOT NULL DEFAULT 0,\n                    rpc_sequence INTEGER NOT NULL DEFAULT 0, init_result TEXT, created REAL NOT NULL,\n                    closed_at REAL, closed_reason TEXT, generation INTEGER NOT NULL DEFAULT 0)")]
pub struct HandlesRow {
    #[db(key)]
    pub id: String,
    pub signature: String,
    pub pid: u32,
    pub sequence: i64,
    pub acknowledged: i64,
    pub rpc_sequence: i64,
    #[db(codec = ExactJsonCodec<SupervisorInitResult>)]
    pub init_result: Option<ExactJson<SupervisorInitResult>>,
    pub created: f64,
    pub closed_at: Option<f64>,
    pub closed_reason: Option<String>,
    pub generation: i64,
}

#[derive(DbTable)]
#[db(table = "operations", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<OperationsRow>,
    legacy_ddl = "CREATE TABLE operations(\n                    handle TEXT NOT NULL, operation_id TEXT NOT NULL, digest TEXT NOT NULL,\n                    native_id INTEGER, accepted REAL NOT NULL, generation INTEGER,\n                    response TEXT, response_sequence INTEGER,\n                    PRIMARY KEY(handle,operation_id))",
    legacy_index = "CREATE INDEX operations_monitor_native ON operations(handle,generation,native_id) WHERE substr(operation_id,1,8)='monitor:'")]
pub struct OperationsRow {
    #[db(key)]
    pub handle: String,
    #[db(key)]
    pub operation_id: String,
    pub digest: String,
    #[db(codec = NativeIdCodec)]
    pub native_id: Option<NativeId>,
    pub accepted: f64,
    pub generation: Option<i64>,
    #[db(codec = ExactJsonCodec<SupervisorOperationResponse>)]
    pub response: Option<ExactJson<SupervisorOperationResponse>>,
    pub response_sequence: Option<i64>,
}

#[derive(DbTable)]
#[db(table = "events", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<EventsRow>,
    legacy_ddl = "CREATE TABLE events(\n                    handle TEXT NOT NULL, sequence INTEGER NOT NULL, kind TEXT NOT NULL,\n                    payload TEXT NOT NULL, size INTEGER NOT NULL, generation INTEGER NOT NULL DEFAULT 1,\n                    PRIMARY KEY(handle,sequence))")]
pub struct EventsRow {
    #[db(key)]
    pub handle: String,
    #[db(key)]
    pub sequence: i64,
    #[db(codec = EventKindCodec)]
    pub kind: JournalEventKind,
    #[db(codec = ExactJsonCodec<SupervisorEventPayload>)]
    pub payload: ExactJson<SupervisorEventPayload>,
    pub size: i64,
    pub generation: i64,
}

#[derive(DbTable)]
#[db(table = "child_identities", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<ChildIdentitiesRow>,
    legacy_ddl = "CREATE TABLE child_identities(\n                    handle TEXT PRIMARY KEY, pid INTEGER NOT NULL, pgid INTEGER NOT NULL,\n                    start_time TEXT NOT NULL, job_name TEXT NOT NULL DEFAULT '',\n                    job_kill_on_close INTEGER NOT NULL DEFAULT 0)")]
pub struct ChildIdentitiesRow {
    #[db(key)]
    pub handle: String,
    pub pid: u32,
    pub pgid: u32,
    pub start_time: String,
    pub job_name: String,
    pub job_kill_on_close: bool,
}

#[derive(DbTable)]
#[db(table = "recovery_events", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<RecoveryEventsRow>,
    legacy_ddl = "CREATE TABLE recovery_events(\n                    id INTEGER PRIMARY KEY AUTOINCREMENT, at REAL NOT NULL,\n                    handle TEXT NOT NULL, pid INTEGER NOT NULL, start_time TEXT NOT NULL,\n                    outcome TEXT NOT NULL, detail TEXT)")]
pub struct RecoveryEventsRow {
    #[db(key)]
    pub id: i64,
    pub at: f64,
    pub handle: String,
    pub pid: u32,
    pub start_time: String,
    pub outcome: String,
    pub detail: Option<String>,
}

#[derive(DbTable)]
#[db(table = "supervisor_state", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<SupervisorStateRow>,
    legacy_ddl = "CREATE TABLE supervisor_state(\n                    key TEXT PRIMARY KEY, value TEXT NOT NULL)")]
pub struct SupervisorStateRow {
    #[db(key)]
    pub key: String,
    #[db(codec = StateValueCodec)]
    pub value: SupervisorStateValue,
}

#[derive(DbTable)]
#[db(table = "degraded_handles", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<DegradedHandlesRow>,
    legacy_ddl = "CREATE TABLE degraded_handles(\n                    handle TEXT PRIMARY KEY, prior_pid INTEGER NOT NULL,\n                    start_time TEXT NOT NULL, recovered_at REAL NOT NULL)")]
pub struct DegradedHandlesRow {
    #[db(key)]
    pub handle: String,
    pub prior_pid: u32,
    pub start_time: String,
    pub recovered_at: f64,
}

#[derive(DbTable)]
#[db(table = "operation_writes", database = Supervisor,
    state_check = crate::state_check::count_legacy_rows::<OperationWritesRow>,
    legacy_ddl = "CREATE TABLE operation_writes(handle TEXT NOT NULL,operation_id TEXT NOT NULL,PRIMARY KEY(handle,operation_id))")]
pub struct OperationWritesRow {
    #[db(key)]
    pub handle: String,
    #[db(key)]
    pub operation_id: String,
}

pub(crate) fn registry() -> [studio_db_api::Registration; 8] {
    db_schema![Supervisor; HandlesRow, OperationsRow, EventsRow, ChildIdentitiesRow, RecoveryEventsRow, SupervisorStateRow, DegradedHandlesRow, OperationWritesRow]
}

/// Export the real journal structure, including its migration-installed monitor index.
pub(crate) fn install(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    for registration in registry() {
        registration.descriptor.install_if_absent(connection)?;
    }
    Ok(())
}

pub(crate) fn check_state_readonly(
    root: &std::path::Path,
) -> crate::StoreResult<Vec<crate::StateCheckTable>> {
    let path = root.join("supervisor.sqlite3");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let connection =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.pragma_update(None, "query_only", true)?;
    registry()
        .into_iter()
        .map(|registration| {
            let report = (registration.check)(&connection)?;
            Ok(crate::StateCheckTable {
                table: registration.descriptor.name,
                row_count: report.row_count,
                rejected_count: report.rejected_count,
                failures: report.failures,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    #[derive(serde::Deserialize)]
    struct CatalogDatabase {
        sqlite_master: Vec<CatalogEntry>,
    }

    #[derive(serde::Deserialize)]
    struct CatalogEntry {
        name: String,
        sql: String,
        table: String,
        #[serde(rename = "type")]
        kind: String,
        create_order: i64,
    }

    #[test]
    fn supervisor_descriptors_preserve_every_catalog_object() {
        let catalog: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).unwrap();
        let expected: CatalogDatabase =
            studio_json::from_str(catalog["supervisor.sqlite3"].as_str()).unwrap();
        let db = rusqlite::Connection::open_in_memory().unwrap();
        install(&db).unwrap();
        let actual = studio_db_api::catalog::export(&db).unwrap();
        assert_eq!(actual.len(), expected.sqlite_master.len());
        for (entry, expected) in actual.iter().zip(&expected.sqlite_master) {
            assert_eq!(
                (
                    &entry.name,
                    &entry.sql,
                    &entry.table,
                    &entry.r#type,
                    entry.create_order
                ),
                (
                    &expected.name,
                    &expected.sql,
                    &expected.table,
                    &expected.kind,
                    expected.create_order
                )
            );
        }
        assert_eq!(registry().len(), 8);
        for registration in registry() {
            let descriptor = registration.descriptor;
            let entry = actual
                .iter()
                .find(|entry| entry.name == descriptor.name)
                .unwrap();
            assert_eq!(entry.sql, descriptor.ddl, "{} parity", descriptor.name);
            assert_eq!((registration.check)(&db).unwrap().counts(), (0, 0));
        }
        OperationsRow::descriptor().install_if_absent(&db).unwrap();
        let index: String = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='operations_monitor_native'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            index,
            "CREATE INDEX operations_monitor_native ON operations(handle,generation,native_id) WHERE substr(operation_id,1,8)='monitor:'"
        );
    }

    #[tokio::test]
    async fn supervisor_fresh_indexes_match_the_current_catalog() {
        let fresh = tempfile::tempdir().unwrap();
        drop(crate::RuntimeStore::open(Some(fresh.path())).await.unwrap());
        let catalog: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).unwrap();
        let expected: CatalogDatabase =
            studio_json::from_str(catalog["supervisor.sqlite3"].as_str()).unwrap();
        let expected: Vec<_> = expected
            .sqlite_master
            .into_iter()
            .filter(|entry| entry.kind == "index")
            .map(|entry| (entry.name, entry.table, entry.sql))
            .collect();
        assert_eq!(expected.len(), 1);
        {
            let root = fresh.path();
            let db = rusqlite::Connection::open(root.join("supervisor.sqlite3")).unwrap();
            let actual: Vec<_> = studio_db_api::catalog::export(&db)
                .unwrap()
                .into_iter()
                .filter(|entry| entry.r#type == "index")
                .map(|entry| (entry.name, entry.table, entry.sql))
                .collect();
            assert_eq!(actual, expected);
            assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('handles','child_identities','degraded_handles','operations','operation_writes','events','recovery_events','supervisor_state')", [], |row| row.get::<_, i64>(0)).unwrap(), 8);
        }
    }

    #[test]
    fn supervisor_registry_counts_legacy_rows_without_decoding_or_writing() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("supervisor.sqlite3");
        let db = rusqlite::Connection::open(&path).unwrap();
        install(&db).unwrap();
        db.execute_batch("INSERT INTO events VALUES('h',-1,'future','not json',-5,-3); INSERT INTO handles(id,signature,pid,created) VALUES(NULL,'s',-1,1.5); INSERT INTO supervisor_state VALUES(NULL,'not json')").unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        let reports = check_state_readonly(root.path()).unwrap();
        assert_eq!(reports.len(), 8);
        let names: std::collections::BTreeSet<_> =
            reports.iter().map(|report| report.table).collect();
        assert_eq!(names.len(), 8);
        for report in reports {
            assert_eq!(report.rejected_count, 0, "{}", report.table);
            assert!(report.failures.is_empty());
            assert_eq!(
                report.row_count,
                u64::from(matches!(
                    report.table,
                    "events" | "handles" | "supervisor_state"
                ))
            );
        }
        assert_eq!(before, std::fs::read(path).unwrap());
        assert!(
            check_state_readonly(&root.path().join("absent"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn supervisor_legacy_json_rejects_unrepresentable_numbers() {
        for raw in ["1e999", r#"{"future":[1e999]}"#] {
            assert!(
                studio_json::from_str::<SupervisorInitResult>(raw).is_err(),
                "init result: {raw}"
            );
            assert!(
                studio_json::from_str::<crate::supervisor_journal::SupervisorRecovery>(raw)
                    .is_err(),
                "recovery: {raw}"
            );
        }
        let raw = r#"{"future":[1.0,9007199254740993],"future":null}"#;
        assert_eq!(
            studio_json::from_str::<SupervisorInitResult>(raw)
                .unwrap()
                .0
                .as_str(),
            raw
        );
    }

    #[test]
    fn supervisor_legacy_json_preserves_raw_value_marker_admission() {
        for (raw, accepted) in [
            (r#"{"$serde_json::private::RawValue":0}"#, false),
            (r#"{"$serde_json::private::RawValue":"not json"}"#, false),
            (
                r#"{"$serde_json::private::RawValue":"{\"ok\":[1.0]}"}"#,
                true,
            ),
            (r#"{"$serde_json::private::RawValue":"1e999"}"#, false),
            (r#"{"first":0,"$serde_json::private::RawValue":0}"#, true),
        ] {
            assert_eq!(
                studio_json::from_str::<SupervisorInitResult>(raw).is_ok(),
                accepted,
                "init result: {raw}"
            );
            assert_eq!(
                studio_json::from_str::<crate::supervisor_journal::SupervisorRecovery>(raw).is_ok(),
                accepted,
                "recovery: {raw}"
            );
        }
    }

    #[test]
    fn supervisor_codecs_retain_exact_json_and_native_storage_classes() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let raw = r#" { "future": [1.0,9007199254740993], "x":1,"x":2 } "#;
        let record = ExactJson::<SupervisorOperationResponse>::from_stored(raw.into()).unwrap();
        assert_eq!(record.raw(), raw);
        assert_eq!(
            ExactJsonCodec::<SupervisorOperationResponse>::encode(&db, &record).unwrap(),
            raw
        );
        for id in [
            None,
            Some(NativeId::Integer(42)),
            Some(NativeId::Text("opaque-id".into())),
        ] {
            let stored = NativeIdCodec::encode(&db, &id).unwrap();
            assert_eq!(
                NativeIdCodec::decode(&db, NativeIdStorage(stored)).unwrap(),
                id
            );
        }
        for stored in [
            rusqlite::types::Value::Real(1.5),
            rusqlite::types::Value::Blob(vec![1]),
        ] {
            assert!(NativeIdCodec::decode(&db, NativeIdStorage(stored)).is_err());
        }
        let recovery: crate::supervisor_journal::SupervisorRecovery = studio_json::from_str(
            r#"{"blocked":true,"blocked":null,"degraded":1.0,"extra":1,"extra":2.00}"#,
        )
        .unwrap();
        assert!(!recovery.blocked());
        assert_eq!(
            studio_json::to_string(&recovery).unwrap(),
            r#"{"degraded":1.0,"blocked":null,"extra":2.00}"#
        );
    }
}
