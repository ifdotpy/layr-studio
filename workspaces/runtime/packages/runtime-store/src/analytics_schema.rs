//! Analytics descriptors. Legacy layouts, nullable selectors and evidence stay unchanged.
use crate::analytics_payloads::*;
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use studio_db_api::{Database, DbTable, Table};

/// Capability for analytics.sqlite3; only its transaction adapter constructs it.
pub struct Analytics {
    _private: (),
}
impl Database for Analytics {}
pub fn analytics_read(db: &rusqlite::Connection) -> studio_db_api::Read<'_, Analytics> {
    studio_db_api::Read::from_adapter(db, &Analytics { _private: () })
}
pub fn analytics_tx<'c>(tx: &'c rusqlite::Transaction<'c>) -> studio_db_api::Tx<'c, Analytics> {
    studio_db_api::Tx::from_adapter(tx, &Analytics { _private: () })
}

#[derive(DbTable)]
#[db(table = "analytics_agents", database = Analytics, legacy_ddl = "CREATE TABLE analytics_agents (id TEXT PRIMARY KEY, record TEXT NOT NULL)", state_check = check_agents)]
#[db(
    legacy_index = "CREATE INDEX analytics_agents_root ON analytics_agents(json_extract(record,'$.rootId'))"
)]
pub struct AnalyticsAgentsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<AnalyticsAgentRecord>)]
    pub record: ExactJson<AnalyticsAgentRecord>,
}

#[derive(DbTable)]
#[db(table = "analytics_turns", database = Analytics, legacy_ddl = "CREATE TABLE analytics_turns (\n            id TEXT PRIMARY KEY, agent TEXT NOT NULL, root TEXT, at REAL NOT NULL, record TEXT NOT NULL)", state_check = check_turns)]
#[db(legacy_index = "CREATE INDEX analytics_turns_scope ON analytics_turns(agent,at)")]
pub struct AnalyticsTurnsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub root: Option<String>,
    pub at: f64,
    #[db(codec = ExactJsonCodec<AnalyticsTurnRecord>)]
    pub record: ExactJson<AnalyticsTurnRecord>,
}

#[derive(DbTable)]
#[db(table = "analytics_items", database = Analytics, legacy_ddl = "CREATE TABLE analytics_items (\n            id TEXT PRIMARY KEY, agent TEXT NOT NULL, root TEXT, thread TEXT, turn TEXT,\n            at REAL NOT NULL, type TEXT, name TEXT, is_tool INTEGER NOT NULL, record TEXT NOT NULL)", state_check = check_items)]
#[db(legacy_index = "CREATE INDEX analytics_items_scope ON analytics_items(agent,at)")]
#[db(legacy_index = "CREATE INDEX analytics_items_team ON analytics_items(root,at)")]
#[db(legacy_index = "CREATE INDEX analytics_items_tool ON analytics_items(name,at)")]
pub struct AnalyticsItemsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub root: Option<String>,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub at: f64,
    #[db(column = "type")]
    pub item_type: Option<String>,
    pub name: Option<String>,
    pub is_tool: bool,
    pub record: AnalyticsItemEncoding,
}

#[derive(DbTable)]
#[db(table = "analytics_history", database = Analytics, legacy_ddl = "CREATE TABLE analytics_history (id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL)", state_check = check_history)]
pub struct AnalyticsHistoryRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec = ExactJsonCodec<AnalyticsHistoryRecord>)]
    pub record: ExactJson<AnalyticsHistoryRecord>,
}

#[derive(DbTable)]
#[db(table = "analytics_limits", database = Analytics, legacy_ddl = "CREATE TABLE analytics_limits (\n            id INTEGER PRIMARY KEY AUTOINCREMENT, account TEXT NOT NULL, at REAL NOT NULL, record TEXT NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsLimitsRow>)]
#[db(legacy_index = "CREATE INDEX analytics_limits_account_at ON analytics_limits(account, at)")]
pub struct AnalyticsLimitsRow {
    #[db(key)]
    pub id: i64,
    pub account: String,
    pub at: f64,
    #[db(codec = ExactJsonCodec<AnalyticsLimitRecord>)]
    pub record: ExactJson<AnalyticsLimitRecord>,
}

#[derive(DbTable)]
#[db(table = "analytics_notifications", database = Analytics, legacy_ddl = "CREATE TABLE analytics_notifications (\n            id TEXT PRIMARY KEY, agent TEXT, root TEXT, method TEXT, hour INTEGER,\n            count INTEGER NOT NULL, bytes INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsNotificationsRow>)]
pub struct AnalyticsNotificationsRow {
    #[db(key)]
    pub id: String,
    pub agent: Option<String>,
    pub root: Option<String>,
    pub method: Option<String>,
    pub hour: Option<i64>,
    pub count: i64,
    pub bytes: i64,
}

#[derive(DbTable)]
#[db(table = "analytics_usage", database = Analytics, legacy_ddl = "CREATE TABLE analytics_usage (\n            seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,\n            agent TEXT NOT NULL, root TEXT, thread TEXT, turn TEXT, at REAL NOT NULL, record TEXT NOT NULL)", state_check = check_usage)]
#[db(legacy_index = "CREATE INDEX analytics_usage_scope ON analytics_usage(agent,at)")]
#[db(legacy_index = "CREATE INDEX analytics_usage_team ON analytics_usage(root,at)")]
#[db(legacy_index = "CREATE INDEX analytics_usage_root_seq ON analytics_usage(root,seq)")]
#[db(
    legacy_index = "CREATE INDEX analytics_usage_response ON analytics_usage(agent,thread,json_extract(record,'$.responseId'))"
)]
#[db(legacy_index = "CREATE INDEX analytics_usage_thread ON analytics_usage(agent,thread,seq)")]
#[db(legacy_index = "CREATE INDEX analytics_usage_migration ON analytics_usage(agent,seq)")]
pub struct AnalyticsUsageRow {
    #[db(key)]
    pub seq: i64,
    pub id: String,
    pub agent: String,
    pub root: Option<String>,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub at: f64,
    #[db(codec = ExactJsonCodec<AnalyticsUsageRecord>)]
    pub record: ExactJson<AnalyticsUsageRecord>,
}

#[derive(DbTable)]
#[db(table = "analytics_cost_rows", database = Analytics, legacy_ddl = "CREATE TABLE analytics_cost_rows (\n    id TEXT PRIMARY KEY, root TEXT NOT NULL, agent TEXT NOT NULL, thread TEXT, turn TEXT,\n    seq INTEGER NOT NULL, at REAL NOT NULL, model TEXT, account_key TEXT, response TEXT,\n    input REAL, cached REAL, cache_write REAL, output REAL, uncached INTEGER NOT NULL,\n    counted INTEGER NOT NULL, counted_model TEXT, eligible INTEGER, final_evidence INTEGER NOT NULL, cache_write_1h REAL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsCostRowsRow>)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_rows_scope ON analytics_cost_rows(root, agent, thread, turn)"
)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_rows_response ON analytics_cost_rows(root, thread, response)\n    WHERE response IS NOT NULL"
)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_rows_agent_response\n    ON analytics_cost_rows(root,agent,response) WHERE response IS NOT NULL"
)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_rows_eligible_unmodeled\n    ON analytics_cost_rows(root,agent) WHERE model IS NULL AND eligible IS NOT 0"
)]
pub struct AnalyticsCostRowsRow {
    #[db(key)]
    pub id: String,
    pub root: String,
    pub agent: String,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub seq: i64,
    pub at: f64,
    pub model: Option<String>,
    pub account_key: Option<String>,
    pub response: Option<String>,
    pub input: Option<f64>,
    pub cached: Option<f64>,
    pub cache_write: Option<f64>,
    pub output: Option<f64>,
    pub uncached: bool,
    pub counted: bool,
    pub counted_model: Option<String>,
    pub eligible: Option<bool>,
    pub final_evidence: bool,
    pub cache_write_1h: Option<f64>,
}

#[derive(DbTable)]
#[db(table = "analytics_cost_groups", database = Analytics, legacy_ddl = "CREATE TABLE analytics_cost_groups (\n    root TEXT NOT NULL, model TEXT, account TEXT NOT NULL, input REAL, cached REAL,\n    cache_write REAL, output REAL, uncached INTEGER NOT NULL, count INTEGER NOT NULL, cache_write_1h REAL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsCostGroupsRow>)]
#[db(receipt)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_groups_cover\n    ON analytics_cost_groups(root,model,account,input,cached,cache_write,output,uncached,cache_write_1h,count)"
)]
pub struct AnalyticsCostGroupsRow {
    // SQLite rowid is the existing identity; no singleton/key constraint is added.
    #[db(key, generated)]
    pub rowid: i64,
    pub root: String,
    pub model: Option<String>,
    pub account: String,
    pub input: Option<f64>,
    pub cached: Option<f64>,
    pub cache_write: Option<f64>,
    pub output: Option<f64>,
    pub uncached: bool,
    pub count: i64,
    pub cache_write_1h: Option<f64>,
}

#[derive(DbTable)]
#[db(table = "analytics_usage_revisions", database = Analytics, legacy_ddl = "CREATE TABLE analytics_usage_revisions (id TEXT PRIMARY KEY, revision INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsUsageRevisionsRow>)]
#[db(
    legacy_index = "CREATE INDEX analytics_usage_revisions_revision ON analytics_usage_revisions(revision)"
)]
pub struct AnalyticsUsageRevisionsRow {
    #[db(key)]
    pub id: String,
    pub revision: i64,
}

#[derive(DbTable)]
#[db(table = "analytics_cost_group_revisions", database = Analytics, legacy_ddl = "CREATE TABLE analytics_cost_group_revisions (id INTEGER PRIMARY KEY, revision INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsCostGroupRevisionsRow>)]
#[db(
    legacy_index = "CREATE INDEX analytics_cost_group_revisions_revision ON analytics_cost_group_revisions(revision)"
)]
pub struct AnalyticsCostGroupRevisionsRow {
    #[db(key)]
    pub id: i64,
    pub revision: i64,
}

#[derive(DbTable)]
#[db(table = "analytics_usage_revision_clock", database = Analytics, legacy_ddl = "CREATE TABLE analytics_usage_revision_clock (revision INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsUsageRevisionClockRow>)]
#[db(receipt)]
pub struct AnalyticsUsageRevisionClockRow {
    // SQLite rowid is the existing identity; no singleton/key constraint is added.
    #[db(key, generated)]
    pub rowid: i64,
    pub revision: i64,
}

#[derive(DbTable)]
#[db(table = "analytics_usage_roots", database = Analytics, legacy_ddl = "CREATE TABLE analytics_usage_roots (root TEXT PRIMARY KEY, generation INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsUsageRootsRow>)]
pub struct AnalyticsUsageRootsRow {
    #[db(key)]
    pub root: String,
    pub generation: i64,
}

#[derive(DbTable)]
#[db(table = "analytics_meta", database = Analytics, legacy_ddl = "CREATE TABLE analytics_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsMetaRow>)]
pub struct AnalyticsMetaRow {
    #[db(key)]
    pub key: String,
    pub value: AnalyticsMetadataValue,
}

#[derive(DbTable)]
#[db(table = "analytics_stored_text_dictionaries", database = Analytics, legacy_ddl = "CREATE TABLE analytics_stored_text_dictionaries (\n        id INTEGER PRIMARY KEY, name TEXT NOT NULL, created REAL NOT NULL,\n        sha256 TEXT NOT NULL, dictionary BLOB NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsStoredTextDictionariesRow>)]
#[db(
    legacy_index = "CREATE INDEX analytics_stored_text_dictionaries_name\n        ON analytics_stored_text_dictionaries(name, created, id)"
)]
pub struct AnalyticsStoredTextDictionariesRow {
    #[db(key)]
    pub id: u32,
    pub name: String,
    pub created: f64,
    pub sha256: String,
    pub dictionary: Vec<u8>,
}

#[derive(DbTable)]
#[db(table = "analytics_publication_overflow_receipts", database = Analytics, legacy_ddl = "CREATE TABLE analytics_publication_overflow_receipts (receipt_id TEXT PRIMARY KEY,analytics_overflow INTEGER NOT NULL,publication_overflow INTEGER NOT NULL)", state_check = crate::state_check::count_legacy_rows::<AnalyticsPublicationOverflowReceiptsRow>)]
pub struct AnalyticsPublicationOverflowReceiptsRow {
    #[db(key)]
    pub receipt_id: String,
    pub analytics_overflow: i64,
    pub publication_overflow: i64,
}

pub(crate) fn registry() -> [studio_db_api::Registration; 16] {
    studio_db_api::db_schema![Analytics; AnalyticsAgentsRow, AnalyticsTurnsRow, AnalyticsItemsRow, AnalyticsHistoryRow, AnalyticsLimitsRow, AnalyticsNotificationsRow, AnalyticsUsageRow, AnalyticsCostRowsRow, AnalyticsCostGroupsRow, AnalyticsUsageRevisionsRow, AnalyticsCostGroupRevisionsRow, AnalyticsUsageRevisionClockRow, AnalyticsUsageRootsRow, AnalyticsMetaRow, AnalyticsStoredTextDictionariesRow, AnalyticsPublicationOverflowReceiptsRow]
}

#[cfg(test)]
pub(crate) fn install(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    for entry in registry() {
        entry.descriptor.install_if_absent(db)?;
    }
    db.execute_batch(include_str!("catalog-analytics.sql"))?;
    install_dictionary_triggers(db)
}

pub(crate) fn install_dictionary(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    AnalyticsStoredTextDictionariesRow::descriptor().install_if_absent(db)?;
    install_dictionary_triggers(db)
}

fn install_dictionary_triggers(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(DICTIONARY_TRIGGERS)
}

const DICTIONARY_TRIGGERS: &str = "CREATE TRIGGER IF NOT EXISTS analytics_stored_text_dictionaries_no_update\n        BEFORE UPDATE ON analytics_stored_text_dictionaries BEGIN\n        SELECT RAISE(ABORT, 'stored text dictionaries are immutable'); END;\nCREATE TRIGGER IF NOT EXISTS analytics_stored_text_dictionaries_no_delete\n        BEFORE DELETE ON analytics_stored_text_dictionaries BEGIN\n        SELECT RAISE(ABORT, 'stored text dictionaries are immutable'); END;\nCREATE TRIGGER IF NOT EXISTS analytics_stored_text_dictionaries_no_replace\n        BEFORE INSERT ON analytics_stored_text_dictionaries\n        WHEN EXISTS(SELECT 1 FROM analytics_stored_text_dictionaries WHERE id=NEW.id) BEGIN\n        SELECT RAISE(ABORT, 'stored text dictionaries are immutable'); END;";

fn check_agents(db: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::analytics_capture::check_analytics_table(
        db,
        crate::analytics_capture::CaptureTable::Agents,
    ))
}
fn check_turns(db: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::analytics_capture::check_analytics_table(
        db,
        crate::analytics_capture::CaptureTable::Turns,
    ))
}
fn check_items(db: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::analytics_capture::check_analytics_table(
        db,
        crate::analytics_capture::CaptureTable::Items,
    ))
}
fn check_usage(db: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::analytics_capture::check_analytics_table(
        db,
        crate::analytics_capture::CaptureTable::Usage,
    ))
}
fn check_history(db: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(
        crate::analytics_capture::check_history_rows(db).map(|(history, _)| history),
    )
}

/// Revision tracking shares one clock; never add a singleton constraint to old state.
pub(crate) fn install_revisions(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    for descriptor in [
        AnalyticsUsageRevisionClockRow::descriptor(),
        AnalyticsUsageRevisionsRow::descriptor(),
        AnalyticsCostGroupRevisionsRow::descriptor(),
    ] {
        descriptor.install_if_absent(db)?;
    }
    db.execute_batch(include_str!("catalog-analytics.sql"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn analytics_registry_preserves_every_legacy_object() {
        let legacy = rusqlite::Connection::open_in_memory().unwrap();
        legacy
            .execute_batch(include_str!("analytics-schema-before-w4.sql"))
            .unwrap();
        let current = rusqlite::Connection::open_in_memory().unwrap();
        install(&current).unwrap();
        install(&current).unwrap();
        let objects = |db: &rusqlite::Connection| {
            studio_db_api::catalog::export(db)
                .unwrap()
                .into_iter()
                .map(|entry| (entry.name, (entry.r#type, entry.table, entry.sql)))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(
            objects(&legacy),
            objects(&current),
            "only create_order may differ"
        );
        let names = registry()
            .map(|entry| entry.descriptor.name)
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 16);
        let ordered_names = |db: &rusqlite::Connection, table: &str, kind: &str| {
            db.prepare(
                "SELECT name FROM sqlite_master WHERE tbl_name=?1 AND type=?2 ORDER BY rowid",
            )
            .unwrap()
            .query_map([table, kind], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        };
        for table in names {
            let expected: BTreeMap<_, _> = objects(&legacy)
                .into_iter()
                .filter(|(_, (_, owner, _))| owner == table)
                .collect();
            let actual: BTreeMap<_, _> = objects(&current)
                .into_iter()
                .filter(|(_, (_, owner, _))| owner == table)
                .collect();
            assert_eq!(actual, expected, "{table} parity");
            for kind in ["index", "trigger"] {
                assert_eq!(
                    ordered_names(&current, table, kind),
                    ordered_names(&legacy, table, kind),
                    "{table} ordered {kind} names"
                );
            }
        }
    }

    #[test]
    fn analytics_registry_preserves_diagnostics_and_reports_each_table_once() {
        let root = tempfile::tempdir().unwrap();
        let db = rusqlite::Connection::open(root.path().join("analytics.sqlite3")).unwrap();
        install(&db).unwrap();
        db.execute_batch(r#"
            INSERT INTO analytics_agents VALUES('wrong','{"id":"other"}');
            INSERT INTO analytics_history VALUES('importer','','{"status":"error","error":"bad","updated":1.0}');
            INSERT INTO analytics_items(id,agent,at,is_tool,record) VALUES('broken','agent',1,0,'not json');
            INSERT INTO analytics_limits(account,at,record) VALUES('account',1,'not json');
            INSERT INTO analytics_usage_revision_clock VALUES(0),(7);
            INSERT INTO analytics_usage_roots VALUES(NULL,'legacy generation');
        "#).unwrap();
        let reports = crate::analytics_capture::check_state_readonly(root.path()).unwrap();
        let names = reports
            .iter()
            .map(|report| report.table)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reports.len(),
            17,
            "16 physical tables plus the history importer"
        );
        assert_eq!(names.len(), reports.len(), "never report a table twice");
        for registration in registry() {
            let actual = reports
                .iter()
                .find(|report| report.table == registration.descriptor.name)
                .unwrap();
            let expected = (registration.check)(&db).unwrap();
            assert_eq!(actual.row_count, expected.row_count, "{}", actual.table);
            assert_eq!(
                actual.rejected_count, expected.rejected_count,
                "{}",
                actual.table
            );
            assert_eq!(actual.failures, expected.failures, "{}", actual.table);
        }
        for (name, rows, rejected) in [
            ("analytics_agents", 1, 1),
            ("analytics_items", 1, 1),
            ("analytics_limits", 1, 0),
            ("analytics_usage_revision_clock", 2, 0),
            ("analytics_usage_roots", 1, 0),
            ("analytics_history", 0, 0),
            ("analytics_history_importer", 1, 0),
        ] {
            let report = reports.iter().find(|report| report.table == name).unwrap();
            assert_eq!(
                (report.row_count, report.rejected_count),
                (rows, rejected),
                "{name}"
            );
        }
    }

    #[test]
    fn analytics_records_keep_exact_bytes_duplicate_keys_and_stored_forms() {
        let raw = r#" { "id":"old", "id":"agent", "epoch":1.0, "future": { "x": 2 } } "#;
        let record = ExactJson::<AnalyticsAgentRecord>::from_stored(raw.into()).unwrap();
        assert_eq!(record.value().fields.id, "agent");
        assert_eq!(record.raw(), raw);
        assert_eq!(
            record.value().extensions["future"].as_str(),
            r#"{ "x": 2 }"#
        );
        let importer = ExactJson::<AnalyticsHistoryRecord>::from_stored(
            r#"{"status":"error","error":"old","error":"last","updated":1.0}"#.into(),
        )
        .unwrap();
        assert!(matches!(importer.value(), AnalyticsHistoryRecord::Importer(
            AnalyticsHistoryImporterDiagnostic::Error { error, .. }
        ) if error == "last"));
        let item = r#" { "id":"item", "agentId":"agent", "at":1, "type":"tool", "name":"tool", "isTool":true, "input":{"bytes":1.0,"chars":1,"lines":1} } "#;
        let compressed = crate::stored_text::encode(item);
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        install(&db).unwrap();
        let tx = db.transaction().unwrap();
        let mut typed = analytics_tx(&tx);
        typed
            .upsert(&AnalyticsItemsRow {
                id: "item".into(),
                agent: "agent".into(),
                root: None,
                thread: None,
                turn: None,
                at: 1.,
                item_type: Some("tool".into()),
                name: Some("tool".into()),
                is_tool: true,
                record: AnalyticsItemEncoding::Compressed(compressed.clone()),
            })
            .unwrap();
        let row = typed
            .get::<AnalyticsItemsRow>(&("item".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(row.record.record().unwrap().raw(), item);
        assert!(
            matches!(row.record, AnalyticsItemEncoding::Compressed(ref bytes) if bytes == &compressed)
        );
        // A narrow projection must not decode an unread malformed record.
        tx.execute(
            "UPDATE analytics_items SET record='not json' WHERE id='item'",
            [],
        )
        .unwrap();
        let read = analytics_read(&tx);
        let id = studio_db_api::db_query!(
            read,
            AnalyticsItemsRow,
            scalar(AnalyticsItemsRow::ID),
            filter(AnalyticsItemsRow::ID.eq_ref("item"))
        )
        .unwrap();
        assert_eq!(id.as_deref(), Some("item"));
        tx.rollback().unwrap();
    }
}
