//! Item, monitor and terminal descriptors; existing encodings and narrow readers stay intact.
use crate::json_record_codec::{JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use crate::{MonitorRecord, TerminalRecord, TranscriptItemRecord};
use studio_db_api::{DbTable, Table};
use studio_runtime_schema::studio_errors::MonitorStatus;

/// Original fulltext storage, including the original compressed frame bytes.
#[derive(Debug, PartialEq)]
pub enum FulltextStorage {
    Text(String),
    Compressed(Vec<u8>),
}
impl rusqlite::types::FromSql for FulltextStorage {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        use rusqlite::types::{FromSqlError, ValueRef};
        match value {
            ValueRef::Text(text) => std::str::from_utf8(text)
                .map(|text| Self::Text(text.into()))
                .map_err(|error| FromSqlError::Other(Box::new(error))),
            ValueRef::Blob(bytes) => Ok(Self::Compressed(bytes.into())),
            ValueRef::Null | ValueRef::Integer(_) | ValueRef::Real(_) => {
                Err(FromSqlError::InvalidType)
            }
        }
    }
}
impl rusqlite::types::ToSql for FulltextStorage {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::{ToSqlOutput, ValueRef};
        Ok(ToSqlOutput::Borrowed(match self {
            Self::Text(text) => ValueRef::Text(text.as_bytes()),
            Self::Compressed(bytes) => ValueRef::Blob(bytes),
        }))
    }
}
impl studio_db_api::DbValue for FulltextStorage {
    const SQL_TYPE: &'static str = "TEXT";
}

/// The physical generated column retains its historical TEXT encoding.
pub struct MonitorStatusCodec;
impl studio_db_api::Codec<MonitorStatus> for MonitorStatusCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(_: &rusqlite::Connection, value: &'a MonitorStatus) -> rusqlite::Result<&'a str> {
        Ok(value.as_str())
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<MonitorStatus> {
        MonitorStatus::ALL
            .iter()
            .copied()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid stored monitor status",
                    )),
                )
            })
    }
}

#[derive(DbTable)]
#[db(table = "runtime_items", database = Canvas, legacy_ddl = "CREATE TABLE runtime_items (\n                  id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL, created REAL NOT NULL)", check = check_runtime_items)]
#[db(legacy_index = "CREATE INDEX runtime_item_agent ON runtime_items(agent, created)")]
#[db(state_check = crate::state_check::check_legacy_json::<TranscriptItemRecord>)]
pub struct ItemsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec = JsonRecordCodec<TranscriptItemRecord>)]
    pub record: RawJsonRecord<TranscriptItemRecord>,
    pub created: f64,
}
fn check_runtime_items(row: &ItemsRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_item_blobs", database = Canvas, legacy_ddl = "CREATE TABLE runtime_item_blobs (id TEXT PRIMARY KEY, hash TEXT NOT NULL, size INTEGER NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<ItemBlobsRow>)]
pub struct ItemBlobsRow {
    #[db(key)]
    pub id: String,
    pub hash: String,
    pub size: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_item_fulltext", database = Canvas, legacy_ddl = "CREATE TABLE runtime_item_fulltext (id TEXT PRIMARY KEY, body TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<ItemFulltextRow>)]
pub struct ItemFulltextRow {
    #[db(key)]
    pub id: String,
    pub body: FulltextStorage,
}

#[derive(DbTable)]
#[db(table = "runtime_monitors", database = Canvas, legacy_ddl = "CREATE TABLE \"runtime_monitors\" (\n        id TEXT PRIMARY KEY, record TEXT NOT NULL,\n        stream_pending INTEGER GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN COALESCE(json_array_length(record,'$.streamQueue')>0,0) ELSE 0 END) STORED NOT NULL CHECK(stream_pending IN (0,1)),\n        monitor_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE WHEN json_type(record,'$.status')='text' AND json_extract(record,'$.status') IN ('approval','starting','running','completed','failed','cancelled','lost') THEN json_extract(record,'$.status') ELSE 'lost' END ELSE 'lost' END) STORED NOT NULL CHECK(monitor_status IN ('approval','starting','running','completed','failed','cancelled','lost')),\n        monitor_error_candidate INTEGER GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN json_type(record,'$.status') IS NOT 'text' OR json_extract(record,'$.status') NOT IN ('approval','starting','running','completed','failed','cancelled','lost') OR json_extract(record,'$.status')='lost' OR (json_extract(record,'$.status')='failed' AND json_type(record,'$.exitCode') IS NOT 'integer') ELSE 1 END) STORED NOT NULL CHECK(monitor_error_candidate IN (0,1)),\n        monitor_created REAL GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE WHEN json_type(record,'$.created') IN ('integer','real') AND json_extract(record,'$.created')>=0 AND json_extract(record,'$.created')<=1.7976931348623157e308 THEN json_extract(record,'$.created') ELSE 0 END ELSE 0 END) STORED NOT NULL CHECK(monitor_created >= 0 AND monitor_created <= 1.7976931348623157e308)\n    ,\"source_account_key\" GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN COALESCE(record->>'$.operation.accountKey',record->>'$.accountKey') END) VIRTUAL,\"agent\" TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.agent' END) VIRTUAL)", check = check_runtime_monitors)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_agent_status ON runtime_monitors(\n                    json_extract(record,'$.agent'),json_extract(record,'$.status'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_status ON runtime_monitors(json_extract(record,'$.status'),json_extract(record,'$.created'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_queued_stream ON runtime_monitors(id) WHERE stream_pending=1"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_studio_error_v2 ON runtime_monitors(monitor_created) WHERE monitor_error_candidate=1"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_startup_active ON runtime_monitors(id) WHERE monitor_status IN ('running','starting','approval')"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_idle_account ON runtime_monitors(source_account_key) WHERE monitor_status IN ('running','starting','approval')"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_idle_agent ON runtime_monitors(agent) WHERE monitor_status IN ('running','starting','approval')"
)]
#[db(state_check = crate::state_check::check_legacy_json::<MonitorRecord>)]
#[db(
    legacy_index = "CREATE INDEX runtime_monitor_rule_success ON runtime_monitors(json_extract(record,'$.ruleId'),json_extract(record,'$.created')) WHERE json_extract(record,'$.status')='completed'"
)]
#[db(index(
    name = "runtime_monitor_window_status",
    columns(monitor_status, monitor_created)
))]
pub struct MonitorsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<MonitorRecord>)]
    pub record: RawJsonRecord<MonitorRecord>,
    #[db(generated)]
    pub stream_pending: bool,
    #[db(generated)]
    #[db(codec = MonitorStatusCodec)]
    pub monitor_status: MonitorStatus,
    #[db(generated)]
    pub monitor_error_candidate: bool,
    #[db(generated)]
    pub monitor_created: f64,
    #[db(generated)]
    pub source_account_key: Option<String>,
    #[db(generated)]
    pub agent: Option<String>,
}
impl MonitorsRow {
    pub const ROWID: studio_db_api::Rowid<Self> = studio_db_api::Rowid::<Self>::COLUMN;
}

fn check_runtime_monitors(row: &MonitorsRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "user_terminals", database = Canvas, legacy_ddl = "CREATE TABLE user_terminals (id TEXT PRIMARY KEY, record TEXT NOT NULL, output TEXT NOT NULL, offset INTEGER NOT NULL, output_generation INTEGER NOT NULL DEFAULT 0)", check = check_user_terminals)]
#[db(state_check = crate::state_check::check_legacy_json::<TerminalRecord>)]
pub struct TerminalsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<TerminalRecord>)]
    pub record: RawJsonRecord<TerminalRecord>,
    pub output: String,
    pub offset: i64,
    pub output_generation: i64,
}
fn check_user_terminals(row: &TerminalsRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "user_terminal_output", database = Canvas, legacy_ddl = "CREATE TABLE user_terminal_output (terminal TEXT NOT NULL, start INTEGER NOT NULL, end INTEGER NOT NULL, text TEXT NOT NULL, PRIMARY KEY (terminal,start))")]
#[db(legacy_index = "CREATE INDEX user_terminal_output_end ON user_terminal_output (terminal,end)")]
#[db(state_check = crate::state_check::count_legacy_rows::<TerminalOutputRow>)]
pub struct TerminalOutputRow {
    #[db(key)]
    pub terminal: String,
    #[db(key)]
    pub start: i64,
    pub end: i64,
    pub text: String,
}

#[derive(DbTable)]
#[db(table = "user_terminal_decoder_state", database = Canvas, legacy_ddl = "CREATE TABLE user_terminal_decoder_state (terminal TEXT PRIMARY KEY, pending BLOB NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<TerminalDecoderRow>)]
pub struct TerminalDecoderRow {
    #[db(key)]
    pub terminal: String,
    pub pending: Vec<u8>,
}

#[derive(DbTable)]
#[db(table = "user_terminal_event_cursor", database = Canvas, legacy_ddl = "CREATE TABLE user_terminal_event_cursor (terminal TEXT PRIMARY KEY, sequence INTEGER NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<TerminalCursorRow>)]
pub struct TerminalCursorRow {
    #[db(key)]
    pub terminal: String,
    pub sequence: i64,
}

/// Lazy feature installation preserves the existing bootstrap boundary.
pub(crate) fn prepare(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::canvas_schema::install(
        connection,
        &[
            ItemBlobsRow::descriptor(),
            ItemFulltextRow::descriptor(),
            TerminalsRow::descriptor(),
            TerminalOutputRow::descriptor(),
            TerminalDecoderRow::descriptor(),
            TerminalCursorRow::descriptor(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::{canvas_read, canvas_tx};
    use studio_db_api::db_query;

    #[derive(serde::Deserialize)]
    struct Catalog {
        #[serde(rename = "canvas.sqlite3")]
        canvas: Database,
    }
    #[derive(serde::Deserialize)]
    struct Database {
        sqlite_master: Vec<Object>,
    }
    #[derive(serde::Deserialize)]
    struct Object {
        name: String,
        sql: String,
        create_order: u64,
        table: String,
        #[serde(rename = "type")]
        kind: String,
    }

    #[test]
    fn family_descriptors_match_catalog_and_install() {
        let catalog: Catalog =
            studio_json::from_str(include_str!("runtime-state-schema.json")).unwrap();
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            ItemsRow::descriptor(),
            ItemBlobsRow::descriptor(),
            ItemFulltextRow::descriptor(),
            MonitorsRow::descriptor(),
            TerminalsRow::descriptor(),
            TerminalOutputRow::descriptor(),
            TerminalDecoderRow::descriptor(),
            TerminalCursorRow::descriptor(),
        ] {
            assert_eq!(
                descriptor.ddl,
                catalog
                    .canvas
                    .sqlite_master
                    .iter()
                    .find(|entry| entry.name == descriptor.name)
                    .unwrap()
                    .sql,
                "{}",
                descriptor.name
            );
            for registration in crate::repositories::schema_registrations() {
                let requirements = registration.requirements;
                for table in requirements
                    .tables
                    .iter()
                    .chain(requirements.bootstrap_tables)
                    .chain(
                        requirements
                            .conditional_tables
                            .iter()
                            .flat_map(|conditional| conditional.tables),
                    )
                {
                    if table.name == descriptor.name {
                        for column in table.columns {
                            assert!(
                                descriptor.columns.iter().any(|owner| owner.name == *column),
                                "{} validator references {}.{} outside its descriptor",
                                registration.repository,
                                descriptor.name,
                                column
                            );
                        }
                    }
                }
            }
            descriptor.install_if_absent(&db).unwrap();
            for index in descriptor.indexes {
                assert_eq!(
                    index.sql,
                    catalog
                        .canvas
                        .sqlite_master
                        .iter()
                        .find(|entry| entry.name == index.name)
                        .unwrap()
                        .sql,
                    "{}",
                    index.name
                );
            }
            let sql: String = db
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name=?1",
                    [descriptor.name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(sql, descriptor.ddl);
        }
        // Macro-expanded work history names are descriptors too.
        for name in [
            crate::work_history::WorkResultsStorageRow::NAME,
            crate::work_history::WorkDecisionsStorageRow::NAME,
            crate::work_history::WorkReleasesStorageRow::NAME,
        ] {
            assert_eq!(
                crate::canvas_schema::registry()
                    .iter()
                    .filter(|registration| registration.descriptor.name == name)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn family_indexes_and_triggers_keep_original_relative_order() {
        let db = crate::fresh_state::test_canvas();
        let catalog = Catalog {
            canvas: Database {
                sqlite_master: studio_db_api::catalog::export(&db)
                    .expect("catalog exports")
                    .into_iter()
                    .map(|entry| Object {
                        name: entry.name,
                        sql: entry.sql,
                        table: entry.table,
                        kind: entry.r#type,
                        create_order: entry
                            .create_order
                            .try_into()
                            .expect("positive catalog rowid"),
                    })
                    .collect(),
            },
        };

        assert!(
            !crate::peer_update_settings::canvas_read(&db)
                .table_exists::<crate::context_settings::TranscriptRevisionsStorageRow>()
                .unwrap()
        );
        assert!(
            !canvas_read(&db)
                .table_exists::<crate::clock_schema::DurableRows>()
                .unwrap()
        );
        for trigger in catalog.canvas.sqlite_master.iter().filter(|object| {
            object.table == "runtime_items"
                && (object.name.starts_with("sync_transcript_revision_")
                    || object.name.starts_with("durable_"))
        }) {
            assert!(
                trigger.sql.contains("\"changes\""),
                "{} targets the canonical change owner",
                trigger.name
            );
            assert!(!trigger.sql.contains("runtime_transcript_revisions"));
            assert!(!trigger.sql.contains("durable_rows"));
        }
        for (table, kind, expected) in [
            (
                "runtime_items",
                "index",
                &["runtime_item_agent", "runtime_item_turn_scope"][..],
            ),
            (
                "runtime_items",
                "trigger",
                &[
                    "summary_receipt_item_delete",
                    "immutable_scope_runtime_items",
                    "immutable_scope_runtime_items_insert",
                    "sync_transcript_revision_runtime_items_insert",
                    "sync_transcript_revision_runtime_items_update",
                    "sync_transcript_revision_runtime_items_delete",
                    "durable_runtime_items_insert",
                    "durable_runtime_items_update",
                    "durable_runtime_items_delete",
                ][..],
            ),
            (
                "runtime_monitors",
                "index",
                &[
                    "runtime_monitor_agent_status",
                    "runtime_monitor_status",
                    "runtime_monitor_queued_stream",
                    "runtime_monitor_studio_error_v2",
                    "runtime_monitor_startup_active",
                    "runtime_monitor_idle_account",
                    "runtime_monitor_idle_agent",
                    "runtime_monitor_rule_success",
                    "runtime_monitor_window_status",
                ][..],
            ),
            (
                "runtime_monitors",
                "trigger",
                &[
                    "immutable_scope_runtime_monitors",
                    "immutable_scope_runtime_monitors_insert",
                    "immutable_scope_runtime_monitors_json_INSERT",
                    "immutable_scope_runtime_monitors_json_UPDATE",
                    "durable_runtime_monitors_insert",
                    "durable_runtime_monitors_update",
                    "durable_runtime_monitors_delete",
                ][..],
            ),
        ] {
            let mut objects = catalog
                .canvas
                .sqlite_master
                .iter()
                .filter(|object| object.table == table && object.kind == kind)
                .collect::<Vec<_>>();
            objects.sort_by_key(|object| object.create_order);
            assert_eq!(
                objects
                    .iter()
                    .map(|object| object.name.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "{table} {kind} order"
            );
        }
    }

    #[test]
    fn count_only_reports_keep_unchecked_history_and_json_diagnostics() {
        for (descriptor, fixture) in [
            (
                ItemBlobsRow::descriptor(),
                "INSERT INTO runtime_item_blobs VALUES(NULL,'old',-1)",
            ),
            (
                ItemFulltextRow::descriptor(),
                "INSERT INTO runtime_item_fulltext VALUES(NULL,x'ff')",
            ),
            (
                TerminalOutputRow::descriptor(),
                "INSERT INTO user_terminal_output VALUES('t',-1,-2,'old')",
            ),
            (
                TerminalDecoderRow::descriptor(),
                "INSERT INTO user_terminal_decoder_state VALUES(NULL,x'ff')",
            ),
            (
                TerminalCursorRow::descriptor(),
                "INSERT INTO user_terminal_event_cursor VALUES(NULL,-1)",
            ),
        ] {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch(descriptor.ddl).unwrap();
            db.execute_batch(fixture).unwrap();
            let registration = crate::canvas_schema::registry()
                .into_iter()
                .find(|entry| entry.descriptor.name == descriptor.name)
                .unwrap();
            assert_eq!(
                (registration.check)(&db).unwrap().counts(),
                (1, 0),
                "{}",
                descriptor.name
            );
        }
        let db = rusqlite::Connection::open_in_memory().unwrap();
        // No expression indexes: malformed history is admitted by the physical table.
        db.execute_batch(ItemsRow::DDL).unwrap();
        db.execute_batch("INSERT INTO runtime_items VALUES('i','a','not json',1)")
            .unwrap();
        let registration = crate::canvas_schema::registry()
            .into_iter()
            .find(|entry| entry.descriptor.name == ItemsRow::NAME)
            .unwrap();
        let report = (registration.check)(&db).unwrap();
        assert_eq!(report.counts(), (1, 1));
        assert_eq!(report.failures[0].keyword, "json");
        assert_eq!(
            db_query!(
                canvas_read(&db),
                ItemsRow,
                scalar(ItemsRow::AGENT),
                filter(ItemsRow::ID.eq_ref("i"))
            )
            .unwrap()
            .as_deref(),
            Some("a")
        );
    }

    #[test]
    fn fulltext_and_terminal_callers_preserve_storage_and_counters() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        prepare(&db).unwrap();
        for body in [
            FulltextStorage::Text("a🙂".into()),
            FulltextStorage::Compressed(crate::stored_text::encode("a🙂")),
        ] {
            let tx = db.transaction().unwrap();
            canvas_tx(&tx)
                .upsert(&ItemFulltextRow {
                    id: "i".into(),
                    body,
                })
                .unwrap();
            let stored = canvas_read(&tx)
                .get::<ItemFulltextRow>(&("i".into(),))
                .unwrap()
                .unwrap();
            let decoded: crate::stored_text::StoredText = tx
                .query_row(
                    "SELECT body FROM runtime_item_fulltext WHERE id='i'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(decoded.0, "a🙂");
            let original = stored.body;
            let bytes_before: (String, String) = tx
                .query_row(
                    "SELECT typeof(body),hex(body) FROM runtime_item_fulltext WHERE id='i'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            canvas_tx(&tx)
                .upsert(&ItemFulltextRow {
                    id: "i".into(),
                    body: original,
                })
                .unwrap();
            let bytes_after: (String, String) = tx
                .query_row(
                    "SELECT typeof(body),hex(body) FROM runtime_item_fulltext WHERE id='i'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(bytes_before, bytes_after);
            tx.commit().unwrap();
        }
        let tx = db.transaction().unwrap();
        crate::TerminalsRepository::save_event_cursor(&tx, "t", 7).unwrap();
        crate::TerminalsRepository::save_decoder_pending(&tx, "t", &[0xf0, 0x9f]).unwrap();
        assert_eq!(
            crate::TerminalsRepository::event_cursor(&tx, "t").unwrap(),
            Some(7)
        );
        assert_eq!(
            crate::TerminalsRepository::decoder_pending(&tx, "t").unwrap(),
            Some(vec![0xf0, 0x9f])
        );
        assert_eq!(
            crate::TerminalsRepository::event_cursor(&tx, "missing").unwrap(),
            None
        );
        tx.rollback().unwrap();
        assert_eq!(
            crate::TerminalsRepository::event_cursor(&db, "t").unwrap(),
            None
        );
    }
}
