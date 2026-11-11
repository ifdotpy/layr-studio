//! Queue and execution storage owners. Historical bytes and physical layouts stay unchanged.
use super::queue_payloads::*;
use crate::json_record_codec::{LazyJson, LazyJsonCodec};
use crate::peer_update_settings::Canvas;
use studio_db_api::{DbTable, Table};
use studio_runtime_schema::persistence::*;

#[derive(DbTable)]
#[db(table = "runtime_events", database = Canvas, legacy_ddl = "CREATE TABLE runtime_events (\n                  id TEXT PRIMARY KEY, agent TEXT NOT NULL, kind TEXT NOT NULL,\n                  text TEXT NOT NULL, status TEXT NOT NULL, created REAL NOT NULL,\n                  epoch INTEGER NOT NULL, turn_id TEXT, error TEXT, receipt_enum_guard INTEGER GENERATED ALWAYS AS (NULL) VIRTUAL CHECK(\"kind\" IN ('user','followup','agent_message','child_result','child_stopped','complaint','complaint_response','chat_review','work_review','work_decision','work_ready','monitor_exit','monitor_stall','rule','rule_missed','rule_stall','radio_turn','browser_declined','browser_joined','browser_outcome','browser_unanswered','event_wake','monitor_cancelled','monitor_output','plan_update','shell_policy','work_released','work_result_invalid','context_repair_wait','monitor_correction','rule_hold')) CHECK(\"status\" IN ('stored_only','pending','reserved','dispatching','delivered','uncertain','cancelled','failed')))")]
#[db(state_check = check_runtime_events)]
#[db(legacy_index = "CREATE INDEX runtime_event_queue ON runtime_events(status, agent, created)")]
#[db(
    legacy_index = "CREATE INDEX runtime_event_repair_candidates_v3 ON runtime_events(agent,created,id)\n                  WHERE status='delivered' AND kind IN ('monitor_exit','agent_message','work_review','work_decision')"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_event_user_turns ON runtime_events(agent,kind,status,turn_id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_events_agent_epoch_status_created\n                  ON runtime_events(agent,epoch,status,created,id)"
)]
#[db(legacy_index = "CREATE INDEX runtime_event_created_id ON runtime_events(created DESC,id)")]
#[db(
    legacy_index = "CREATE INDEX runtime_event_child_result_pending ON runtime_events(created) WHERE kind='child_result' AND status='pending'"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_event_reviewer_archive_delivery ON runtime_events(agent, CASE WHEN json_valid(text) THEN json_extract(text,'$.agent_id') END,id) WHERE kind='child_result' AND status='delivered'"
)]
#[db(legacy_index = "CREATE INDEX runtime_event_idle_agent_status ON runtime_events(agent,status)")]
pub struct EventsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec = LegacyTextCodec<EventKind>)]
    pub kind: EventKind,
    pub text: String,
    #[db(codec = LegacyTextCodec<EventStatus>)]
    pub status: EventStatus,
    pub created: f64,
    pub epoch: i64,
    pub turn_id: Option<String>,
    pub error: Option<String>,
    #[db(generated)]
    pub receipt_enum_guard: Option<i64>,
}

fn check_runtime_events(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::legacy_system_check(connection, EventsRow::NAME, "status")
}

#[derive(DbTable)]
#[db(table = "runtime_event_meta", database = Canvas, legacy_ddl = "CREATE TABLE runtime_event_meta (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::check_legacy_json::<crate::EventMetadataRecord>)]
#[db(
    legacy_index = "CREATE INDEX runtime_receipt_transcript_item ON runtime_event_meta(json_extract(record,'$.transcriptItemId'))"
)]
pub struct EventMetaRow {
    #[db(key)]
    pub id: String,
    #[db(codec = LazyJsonCodec<EventMetadataPayload>)]
    pub record: LazyJson<EventMetadataPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_event_revisions", database = Canvas, legacy_ddl = "CREATE TABLE runtime_event_revisions(id TEXT PRIMARY KEY, rev INTEGER NOT NULL) WITHOUT ROWID")]
#[db(state_check = crate::state_check::count_legacy_rows::<EventRevisionsRow>)]
pub struct EventRevisionsRow {
    #[db(key)]
    pub id: String,
    pub rev: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_native_notices", database = Canvas, legacy_ddl = "CREATE TABLE runtime_native_notices (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<NativeNoticesRow>)]
pub struct NativeNoticesRow {
    #[db(key)]
    pub id: String,
    #[db(codec = LazyJsonCodec<NativeNoticePayload>)]
    pub record: LazyJson<NativeNoticePayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_execution_runs", database = Canvas, legacy_ddl = "CREATE TABLE runtime_execution_runs (id TEXT PRIMARY KEY, agent TEXT NOT NULL, account TEXT NOT NULL, epoch INTEGER NOT NULL, thread TEXT, turn TEXT, created REAL NOT NULL, record TEXT NOT NULL, status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.status' END) VIRTUAL CHECK(\"status\" IN ('unsent','pending','accepted','observed','running','unknown','rejected','completed','failed','interrupted','cancelled')))")]
#[db(state_check = check_runtime_execution_runs)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_run_agent ON runtime_execution_runs(agent,created DESC)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_run_created ON runtime_execution_runs(created DESC)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_run_message ON runtime_execution_runs(agent,thread,turn)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_run_native ON runtime_execution_runs(agent,account,thread,turn)"
)]
pub struct ExecutionRunsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub account: String,
    pub epoch: i64,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub created: f64,
    #[db(codec = LazyJsonCodec<ExecutionRunPayload>)]
    pub record: LazyJson<ExecutionRunPayload>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ExecutionStatus>)]
    pub status: Option<ExecutionStatus>,
}

fn check_runtime_execution_runs(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::legacy_system_check(connection, ExecutionRunsRow::NAME, "record")
}

#[derive(DbTable)]
#[db(table = "runtime_execution_nodes", database = Canvas, legacy_ddl = "CREATE TABLE runtime_execution_nodes (id TEXT PRIMARY KEY, run TEXT NOT NULL, record TEXT NOT NULL, status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.status' END) VIRTUAL CHECK(\"status\" IN ('unknown','idle','queued','starting','running','waiting','waiting_on_monitor','approval','completed','interrupted','failed','blocked','paused','parked','abandoned','capacity-retry')))")]
#[db(state_check = crate::state_check::count_legacy_rows::<ExecutionNodesRow>)]
#[db(legacy_index = "CREATE INDEX runtime_execution_node_run ON runtime_execution_nodes(run)")]
pub struct ExecutionNodesRow {
    #[db(key)]
    pub id: String,
    pub run: String,
    #[db(codec = LazyJsonCodec<ExecutionNodePayload>)]
    pub record: LazyJson<ExecutionNodePayload>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ExecutionNodeStatus>)]
    pub status: Option<ExecutionNodeStatus>,
}

#[derive(DbTable)]
#[db(table = "runtime_execution_effects", database = Canvas, legacy_ddl = "CREATE TABLE runtime_execution_effects (id TEXT PRIMARY KEY, run TEXT NOT NULL, kind TEXT NOT NULL, reference TEXT NOT NULL, record TEXT NOT NULL, status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.status' END) VIRTUAL CHECK(\"status\" IN ('unknown','idle','queued','starting','running','waiting','waiting_on_monitor','approval','completed','interrupted','failed','blocked','paused','parked','abandoned','capacity-retry','pending','cancelled','answering','answered','uncertain','resolved','lost','expired','deleted')), receipt_enum_guard INTEGER GENERATED ALWAYS AS (NULL) VIRTUAL CHECK(\"kind\" IN ('spawn','tool_requests','requests','monitors','task_submit','unrecognized')))")]
#[db(state_check = check_runtime_execution_effects)]
#[db(legacy_index = "CREATE INDEX runtime_execution_effect_run ON runtime_execution_effects(run)")]
pub struct ExecutionEffectsRow {
    #[db(key)]
    pub id: String,
    pub run: String,
    #[db(codec = LegacyTextCodec<ExecutionEffectKind>)]
    pub kind: ExecutionEffectKind,
    pub reference: String,
    #[db(codec = LazyJsonCodec<ExecutionEffectPayload>)]
    pub record: LazyJson<ExecutionEffectPayload>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ExecutionEffectStatus>)]
    pub status: Option<ExecutionEffectStatus>,
    #[db(generated)]
    pub receipt_enum_guard: Option<i64>,
}

fn check_runtime_execution_effects(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::legacy_system_check(connection, ExecutionEffectsRow::NAME, "record")
}

#[derive(DbTable)]
#[db(table = "runtime_execution_attempts", database = Canvas, legacy_ddl = "CREATE TABLE runtime_execution_attempts (id TEXT PRIMARY KEY, run TEXT NOT NULL, record TEXT NOT NULL, submission TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.submission' END) VIRTUAL CHECK(\"submission\" IN ('unsent','pending','accepted','observed','rejected','unknown')), turn_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.turnStatus' END) VIRTUAL CHECK(\"turn_status\" IN ('unsent','pending','accepted','observed','running','unknown','rejected','completed','failed','interrupted','cancelled')))")]
#[db(state_check = crate::state_check::check_legacy_json::<crate::repositories::claude_records::ClaudeAttemptRecord>)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_attempt_run ON runtime_execution_attempts(run)"
)]
#[db(
    legacy_index = "CREATE UNIQUE INDEX runtime_execution_terminal_source ON runtime_execution_attempts(json_extract(record,'$.terminalRepair.source.accountKey'),json_extract(record,'$.terminalRepair.source.threadId'),json_extract(record,'$.terminalRepair.proof.turnId')) WHERE json_type(record,'$.terminalRepair')='object'"
)]
pub struct ExecutionAttemptsRow {
    #[db(key)]
    pub id: String,
    pub run: String,
    #[db(codec = LazyJsonCodec<ExecutionAttemptPayload>)]
    pub record: LazyJson<ExecutionAttemptPayload>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<AttemptSubmission>)]
    pub submission: Option<AttemptSubmission>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ExecutionStatus>)]
    pub turn_status: Option<ExecutionStatus>,
}

#[derive(DbTable)]
#[db(table = "runtime_execution_inputs", database = Canvas, legacy_ddl = "CREATE TABLE runtime_execution_inputs (run TEXT NOT NULL, event TEXT NOT NULL, PRIMARY KEY(run,event))")]
#[db(state_check = crate::state_check::count_legacy_rows::<ExecutionInputsRow>)]
#[db(
    legacy_index = "CREATE INDEX runtime_execution_input_event ON runtime_execution_inputs(event)"
)]
pub struct ExecutionInputsRow {
    #[db(key)]
    pub run: String,
    #[db(key)]
    pub event: String,
}

#[derive(DbTable)]
#[db(table = "runtime_process_actions", database = Canvas, legacy_ddl = "CREATE TABLE runtime_process_actions (id TEXT PRIMARY KEY, signature TEXT NOT NULL, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<ProcessActionsRow>)]
pub struct ProcessActionsRow {
    #[db(key)]
    pub id: String,
    pub signature: String,
    #[db(codec = LazyJsonCodec<crate::ProcessActionRecord>)]
    pub record: LazyJson<crate::ProcessActionRecord>,
}

/// Trigger-owned lookup index; receipts retain all admission authority.
#[derive(DbTable)]
#[db(table = "runtime_tool_request_alias_index", database = Canvas, without_rowid,
    index(name = "runtime_tool_request_alias_index_owner", columns(agent,request_id)))]
pub struct ToolRequestAliasIndex {
    #[db(key)]
    pub agent: String,
    #[db(key)]
    pub alias: String,
    #[db(key)]
    pub request_id: String,
}

#[derive(DbTable)]
#[db(table = "runtime_tool_requests", database = Canvas, legacy_ddl = "CREATE TABLE runtime_tool_requests (id TEXT PRIMARY KEY, record TEXT NOT NULL, agent TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE record->>'$.kind' WHEN 'managed_tool' THEN record->>'$.agent' WHEN 'provider_callback' THEN record->>'$.payload.identity.context.agent' END END) VIRTUAL, kind TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.kind' END) VIRTUAL CHECK(\"kind\" IN ('managed_tool','provider_callback','unrecognized')), status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE record->>'$.kind' WHEN 'managed_tool' THEN record->>'$.stage' WHEN 'provider_callback' THEN record->>'$.status' END END) VIRTUAL CHECK(\"status\" IN ('queued','running','cancelled','interrupted','failed','unknown','completed')), request_id TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE record->>'$.kind' WHEN 'managed_tool' THEN record->>'$.admitted.id' WHEN 'provider_callback' THEN record->>'$.id' END END) VIRTUAL, origin_key TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.originKey' END) VIRTUAL, updated INTEGER GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.updated' END) VIRTUAL)")]
#[db(state_check = check_runtime_tool_requests)]
#[db(
    legacy_index = "CREATE INDEX runtime_tool_request_actor\n                ON runtime_tool_requests(json_extract(record,'$.agent'), json_extract(record,'$.updated') DESC)"
)]
#[db(
    legacy_index = "CREATE UNIQUE INDEX runtime_tool_request_origin ON runtime_tool_requests(origin_key) WHERE origin_key IS NOT NULL"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_tool_request_current_actor ON runtime_tool_requests(agent,updated DESC,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_tool_request_current_status ON runtime_tool_requests(status,agent,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_tool_request_startup_unsettled_v2 ON runtime_tool_requests(id) WHERE kind='managed_tool' AND status IN ('queued','running')"
)]
pub struct ToolRequestsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = LazyJsonCodec<ToolRequestPayload>)]
    pub record: LazyJson<ToolRequestPayload>,
    #[db(generated)]
    pub agent: Option<String>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ToolRequestKind>)]
    pub kind: Option<ToolRequestKind>,
    #[db(generated)]
    #[db(codec = LegacyTextCodec<ToolRequestStatus>)]
    pub status: Option<ToolRequestStatus>,
    #[db(generated)]
    pub request_id: Option<String>,
    #[db(generated)]
    pub origin_key: Option<String>,
    #[db(generated)]
    pub updated: Option<i64>,
}

fn check_runtime_tool_requests(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::legacy_system_check(connection, ToolRequestsRow::NAME, "record")
}

pub(crate) fn prepare_features(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    for descriptor in [
        EventMetaRow::descriptor(),
        NativeNoticesRow::descriptor(),
        ExecutionRunsRow::descriptor(),
        ExecutionNodesRow::descriptor(),
        ExecutionEffectsRow::descriptor(),
        ExecutionAttemptsRow::descriptor(),
        ExecutionInputsRow::descriptor(),
        ProcessActionsRow::descriptor(),
        ToolRequestsRow::descriptor(),
    ] {
        connection.execute_batch(&descriptor.ddl.replacen(
            "CREATE TABLE ",
            "CREATE TABLE IF NOT EXISTS ",
            1,
        ))?;
        // The established migrations add generated columns before their newer indexes.
        for index in descriptor.indexes.iter().filter(|index| {
            !matches!(
                index.name,
                "runtime_tool_request_current_actor"
                    | "runtime_tool_request_current_status"
                    | "runtime_tool_request_origin"
                    | "runtime_tool_request_startup_unsettled_v2"
                    | "runtime_execution_terminal_source"
            )
        }) {
            install_index_sql(connection, index.sql)?;
        }
    }
    Ok(())
}

pub(crate) fn index_sql<R: Table>(name: &str) -> &'static str {
    R::INDEXES
        .iter()
        .find(|index| index.name == name)
        .expect("descriptor-owned index")
        .sql
}
pub(crate) fn install_index_sql(
    connection: &rusqlite::Connection,
    sql: &str,
) -> rusqlite::Result<()> {
    connection.execute_batch(&sql.replacen("INDEX ", "INDEX IF NOT EXISTS ", 1))
}
pub(crate) fn install_index<R: Table>(
    connection: &rusqlite::Connection,
    name: &str,
) -> rusqlite::Result<()> {
    install_index_sql(connection, index_sql::<R>(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::db_query;

    #[test]
    fn queue_descriptors_preserve_catalog_objects_and_fresh_install() {
        #[derive(serde::Deserialize)]
        struct Snapshot {
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
        }
        let snapshot: Snapshot = studio_json::from_str(crate::fresh_state::SNAPSHOT).unwrap();
        let connection = crate::fresh_state::test_canvas();
        prepare_features(&connection).unwrap();
        for registration in crate::canvas_schema::registry().into_iter().filter(|r| {
            matches!(
                r.descriptor.name,
                "runtime_events"
                    | "runtime_event_meta"
                    | "runtime_event_revisions"
                    | "runtime_native_notices"
                    | "runtime_execution_runs"
                    | "runtime_execution_nodes"
                    | "runtime_execution_effects"
                    | "runtime_execution_attempts"
                    | "runtime_execution_inputs"
                    | "runtime_process_actions"
                    | "runtime_tool_requests"
            )
        }) {
            let descriptor = registration.descriptor;
            for (name, sql) in std::iter::once((descriptor.name, descriptor.ddl)).chain(
                descriptor
                    .indexes
                    .iter()
                    .map(|index| (index.name, index.sql)),
            ) {
                let expected = snapshot
                    .canvas
                    .sqlite_master
                    .iter()
                    .find(|o| o.name == name)
                    .unwrap();
                assert_eq!(sql, expected.sql, "{name} parity");
                let actual: String = connection
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name=?1",
                        [name],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(actual, sql, "{name} installed parity");
            }
            assert_eq!(
                (registration.check)(&connection).unwrap().rejected_count,
                0,
                "{}",
                descriptor.name
            );
        }
    }

    #[test]
    fn queue_raw_projection_and_lazy_decode_preserve_historical_evidence() {
        let mut connection = crate::fresh_state::test_canvas();
        let raw = " \n{\"id\":\"old\",\"id\":\"run\",\"epoch\":1.0,\"future\": { \"x\": 1.00 }}\t";
        connection.execute("INSERT INTO runtime_execution_runs(id,agent,account,epoch,created,record) VALUES('run','agent','default',1,1,?1)", [raw]).unwrap();
        let saved = crate::peer_update_settings::canvas_read(&connection)
            .get::<ExecutionRunsRow>(&("run".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(saved.record.as_raw(), raw);
        let view = saved.record.decode().unwrap();
        assert_eq!(view.id.as_deref(), Some("run"));
        assert_eq!(view.extensions["future"].as_str(), "{ \"x\": 1.00 }");
        let tx = connection.transaction().unwrap();
        tx.execute("INSERT INTO runtime_native_notices VALUES('bad','{')", [])
            .unwrap();
        let notice = crate::peer_update_settings::canvas_read(&tx)
            .get::<NativeNoticesRow>(&("bad".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(notice.record.as_raw(), "{");
        assert!(notice.record.decode().is_err());
        let raw = db_query!(
            crate::peer_update_settings::canvas_read(&tx),
            NativeNoticesRow,
            scalar_raw(NativeNoticesRow::RECORD),
            filter(NativeNoticesRow::ID.eq_ref("bad"))
        )
        .unwrap();
        assert_eq!(raw.as_deref(), Some("{"));
        assert_eq!(
            crate::state_check::count_legacy_rows::<NativeNoticesRow>(&tx)
                .unwrap()
                .counts(),
            (1, 0)
        );
        tx.rollback().unwrap();
    }

    #[test]
    fn queue_legacy_reports_keep_acceptance_and_appear_once() {
        let directory = tempfile::tempdir().unwrap();
        let connection =
            rusqlite::Connection::open(directory.path().join("canvas.sqlite3")).unwrap();
        connection.execute_batch("CREATE TABLE runtime_events(id,status,error); INSERT INTO runtime_events VALUES('e','unknown',NULL); CREATE TABLE runtime_event_meta(id,record); INSERT INTO runtime_event_meta VALUES('m','{\"delivery\":\"unknown\"}'); CREATE TABLE runtime_execution_attempts(id,run,record); INSERT INTO runtime_execution_attempts VALUES(NULL,NULL,'{');").unwrap();
        let report = crate::check_state_readonly(directory.path()).unwrap();
        for (table, rows, rejected) in [
            (EventsRow::NAME, 1, 1),
            (EventMetaRow::NAME, 1, 1),
            (ExecutionAttemptsRow::NAME, 1, 1),
        ] {
            let matches: Vec<_> = report.tables.iter().filter(|r| r.table == table).collect();
            assert_eq!(matches.len(), 1, "{table} one CLI report");
            assert_eq!(
                (matches[0].row_count, matches[0].rejected_count),
                (rows, rejected)
            );
            if rejected != 0 {
                assert!(!matches[0].failures.is_empty());
            }
        }
    }
    #[test]
    fn queue_index_trigger_order_preserves_planner_ties() {
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
            table: String,
            create_order: i64,
            #[serde(rename = "type")]
            kind: String,
        }
        let catalog: Catalog =
            studio_json::from_str(&crate::storage_catalog::generate().unwrap()).unwrap();
        let order = |table: &str, kind: &str| {
            let mut entries: Vec<_> = catalog
                .canvas
                .sqlite_master
                .iter()
                .filter(|o| o.table == table && o.kind == kind)
                .collect();
            entries.sort_by_key(|o| o.create_order);
            entries
                .into_iter()
                .map(|o| o.name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            order("runtime_events", "index"),
            [
                "runtime_event_queue",
                "runtime_event_repair_candidates_v3",
                "runtime_event_user_turns",
                "runtime_events_agent_epoch_status_created",
                "runtime_event_created_id",
                "runtime_event_child_result_pending",
                "runtime_event_reviewer_archive_delivery",
                "runtime_event_idle_agent_status"
            ],
            "runtime_events historical index order"
        );
        assert_eq!(
            order("runtime_events", "trigger"),
            [
                "runtime_event_revision",
                "runtime_event_revision_cleanup",
                "summary_messageReceipt_delete",
                "sync_entity_event_INSERT",
                "sync_entity_event_UPDATE",
                "sync_entity_event_UPDATE_WINDOW",
                "sync_entity_event_DELETE",
                "immutable_scope_runtime_events",
                "immutable_scope_runtime_events_insert"
            ],
            "runtime_events historical trigger order"
        );
        assert_eq!(
            order("runtime_tool_requests", "index"),
            [
                "runtime_tool_request_actor",
                "runtime_tool_request_origin",
                "runtime_tool_request_current_actor",
                "runtime_tool_request_current_status",
                "runtime_tool_request_startup_unsettled_v2"
            ],
            "runtime_tool_requests historical index order"
        );
        assert_eq!(
            order("runtime_tool_requests", "trigger"),
            [
                "runtime_tool_request_alias_index_insert",
                "runtime_tool_request_alias_index_update",
                "runtime_tool_request_alias_index_delete"
            ]
        );
    }
    #[test]
    fn queue_historical_feature_install_defers_migrated_column_indexes() {
        let mut connection = crate::fresh_state::test_canvas();
        // Independent pre-receipt layouts: none of the later generated columns
        // exist when the actual early feature installer runs.
        connection.execute_batch("DROP TABLE runtime_tool_requests;
            CREATE TABLE runtime_tool_requests(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            DROP TABLE runtime_execution_runs;
            CREATE TABLE runtime_execution_runs(id TEXT PRIMARY KEY,agent TEXT NOT NULL,account TEXT NOT NULL,epoch INTEGER NOT NULL,thread TEXT,turn TEXT,created REAL NOT NULL,record TEXT NOT NULL);
            DROP TABLE runtime_execution_attempts;
            CREATE TABLE runtime_execution_attempts(id TEXT PRIMARY KEY,run TEXT NOT NULL,record TEXT NOT NULL);
            DROP TABLE runtime_execution_effects;
            CREATE TABLE runtime_execution_effects(id TEXT PRIMARY KEY,run TEXT NOT NULL,kind TEXT NOT NULL,reference TEXT NOT NULL,record TEXT NOT NULL);
            DROP TABLE runtime_execution_nodes;
            CREATE TABLE runtime_execution_nodes(id TEXT PRIMARY KEY,run TEXT NOT NULL,record TEXT NOT NULL);
            DROP TABLE runtime_execution_inputs;
            CREATE TABLE runtime_execution_inputs(run TEXT NOT NULL,event TEXT NOT NULL,PRIMARY KEY(run,event));
            INSERT INTO runtime_tool_requests VALUES('history',' { } ');").unwrap();
        let tx = connection.transaction().unwrap();
        crate::feature_schema::prepare(&tx).expect("early preparation admits historical columns");
        for name in [
            "runtime_tool_request_current_actor",
            "runtime_tool_request_current_status",
            "runtime_tool_request_origin",
            "runtime_tool_request_startup_unsettled_v2",
            "runtime_execution_terminal_source",
        ] {
            let present: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1)",
                    [name],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(
                !present,
                "{name} remains at its original later migration stage"
            );
        }
        let has_status: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo('runtime_tool_requests') WHERE name='status')", [], |row| row.get(0)).unwrap();
        assert!(
            !has_status,
            "early preparation must not silently add later columns"
        );
        crate::schema::prepare_receipt_owners(&tx)
            .expect("later preparation adds columns before indexes");
        for row in [
            ExecutionRunsRow::descriptor(),
            ExecutionNodesRow::descriptor(),
            ExecutionEffectsRow::descriptor(),
            ExecutionAttemptsRow::descriptor(),
            ExecutionInputsRow::descriptor(),
            ToolRequestsRow::descriptor(),
        ] {
            for index in row.indexes {
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1)",
                        [index.name],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(exists, "{} installed after its columns", index.name);
            }
        }
        let retained: String = tx
            .query_row(
                "SELECT record FROM runtime_tool_requests WHERE id='history'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained, " { } ");
        tx.commit().unwrap();
    }
}
