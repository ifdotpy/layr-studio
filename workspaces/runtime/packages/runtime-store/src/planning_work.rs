//! Planning and work descriptors retain the existing Canvas physical format.
use crate::json_record_codec::{ExactJson, ExactJsonCodec, JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use crate::{
    AnnotationRecord, CheckpointRecord, PlanRecord, RuleRecord, StoreResult, TaskRecord,
    WorkItemRecord, WorkspaceOperationRecord,
};
use studio_db_api::DbTable;

/// Replace abbreviated test table declarations with their descriptor-owned format.
#[doc(hidden)]
pub fn fixture_sql(batch: &str) -> String {
    use studio_db_api::Table;
    let mut batch = batch.to_owned();
    for (name, ddl) in [
        (WorkRow::NAME, WorkRow::DDL),
        (ProjectRow::NAME, ProjectRow::DDL),
    ] {
        if let Some(start) = batch.find(&format!("CREATE TABLE {name}")) {
            let end = batch[start..]
                .find(';')
                .map_or(batch.len(), |offset| start + offset);
            batch.replace_range(start..end, ddl);
        }
    }
    batch
}

#[derive(DbTable)]
#[db(table = "runtime_annotations",
    state_check = crate::state_check::check_legacy_json::<AnnotationRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_annotations (id TEXT PRIMARY KEY, record TEXT NOT NULL, rev INTEGER NOT NULL DEFAULT 1)",
    check = check_runtime_annotations)]
pub struct AnnotationRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<AnnotationRecord>)]
    pub record: RawJsonRecord<AnnotationRecord>,
    #[db(generated)]
    pub rev: i64,
}
fn check_runtime_annotations(row: &AnnotationRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_checkpoints",
    state_check = crate::state_check::check_legacy_json::<CheckpointRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_checkpoints (id TEXT PRIMARY KEY, record TEXT NOT NULL)",
    check = check_runtime_checkpoints)]
pub struct CheckpointRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<CheckpointRecord>)]
    pub record: RawJsonRecord<CheckpointRecord>,
}
fn check_runtime_checkpoints(row: &CheckpointRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_plans",
    state_check = crate::state_check::check_legacy_json::<PlanRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_plans (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct PlanRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<PlanRecord>)]
    pub record: ExactJson<PlanRecord>,
}

#[derive(DbTable)]
#[db(table = "runtime_projects",
    database = Canvas,
    state_check = crate::state_check::check_legacy_json::<crate::ProjectLocationRecord>,
    legacy_ddl = "CREATE TABLE runtime_projects (id TEXT PRIMARY KEY, record TEXT NOT NULL, removal_reason TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.removal.reason' END) VIRTUAL CHECK(\"removal_reason\" IN ('delete')))",
    check = check_project)]
pub struct ProjectRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<studio_runtime_schema::project_record::ProjectRecord>)]
    pub record: ExactJson<studio_runtime_schema::project_record::ProjectRecord>,
    #[db(generated)]
    pub removal_reason: Option<String>,
}

#[derive(DbTable)]
#[db(table = "runtime_rules",
    state_check = crate::state_check::check_legacy_json::<RuleRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_rules (id TEXT PRIMARY KEY, record TEXT NOT NULL)",
    check = check_runtime_rules,
    legacy_index = "CREATE INDEX runtime_rule_agent_inflight ON runtime_rules(json_extract(record,'$.agent'),json_extract(record,'$.inFlight'))",
    legacy_index = "CREATE INDEX runtime_rule_ready_tick ON runtime_rules(json_extract(record,'$.status')) WHERE NOT COALESCE(json_extract(record,'$.inFlight'),0)")]
pub struct RuleRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<RuleRecord>)]
    pub record: RawJsonRecord<RuleRecord>,
}
fn check_runtime_rules(row: &RuleRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_tasks",
    state_check = crate::state_check::check_legacy_json::<TaskRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_tasks (id TEXT PRIMARY KEY, record TEXT NOT NULL)",
    check = check_runtime_tasks,
    legacy_index = "CREATE INDEX runtime_task_agent_created_id ON runtime_tasks(\n                    json_extract(record,'$.agent'), json_extract(record,'$.created') DESC, id DESC)",
    legacy_index = "CREATE INDEX runtime_task_agent_updated_id ON runtime_tasks(\n                    json_extract(record,'$.agent'),\n                    CASE WHEN COALESCE(json_extract(record,'$.finished'),0) > COALESCE(json_extract(record,'$.created'),0)\n                         THEN json_extract(record,'$.finished') ELSE json_extract(record,'$.created') END,\n                    id)",
    legacy_index = "CREATE INDEX runtime_task_history ON runtime_tasks(json_extract(record,'$.created') DESC, json_extract(record,'$.agent')) WHERE json_extract(record,'$.status')!='running'",
    legacy_index = "CREATE INDEX runtime_task_status ON runtime_tasks(json_extract(record,'$.status'), json_extract(record,'$.created'))")]
#[db(
    legacy_index = "CREATE INDEX runtime_task_item_turn ON runtime_tasks(json_extract(record,'$.itemId'),json_extract(record,'$.turnId'))"
)]
pub struct TaskRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<TaskRecord>)]
    pub record: RawJsonRecord<TaskRecord>,
}
fn check_runtime_tasks(row: &TaskRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_work",
    state_check = crate::state_check::check_legacy_json::<WorkItemRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_work (id TEXT PRIMARY KEY, record TEXT NOT NULL, status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.status' END) VIRTUAL CHECK(\"status\" IN ('ready','blocked','running','review','accepted','cancelled')), archive_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.archiveIntent.status' END) VIRTUAL CHECK(\"archive_status\" IN ('completed','terminal','pending','retryable','submitted','unknown','not_sent','complete','applied','failed','cancelled')), archive_next_at REAL GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN COALESCE(record->>'$.archiveIntent.nextAttemptAt',0) END) VIRTUAL, vm_submit_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.vmSubmit.status' END) VIRTUAL CHECK(\"vm_submit_status\" IN ('completed','terminal','pending','retryable','submitted','unknown','not_sent','complete','applied','failed','cancelled')), vm_accept_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.vmAccept.status' END) VIRTUAL CHECK(\"vm_accept_status\" IN ('completed','terminal','pending','retryable','submitted','unknown','not_sent','complete','applied','failed','cancelled')),\"owner\" TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.owner' END) VIRTUAL)",
    check = check_runtime_work,
    legacy_index = "CREATE INDEX runtime_work_archive_due ON runtime_work(archive_next_at,id) WHERE archive_status IN ('pending','retryable','unknown')",
    legacy_index = "CREATE INDEX runtime_work_owner_root_status_id ON runtime_work(\n                json_extract(record,'$.owner'),json_extract(record,'$.rootId'),\n                json_extract(record,'$.status'),id)",
    legacy_index = "CREATE INDEX runtime_work_owner_status\n                ON runtime_work(json_extract(record,'$.owner'),json_extract(record,'$.status'))",
    legacy_index = "CREATE INDEX runtime_work_root_status\n                ON runtime_work(json_extract(record,'$.rootId'),json_extract(record,'$.status'))",
    legacy_index = "CREATE INDEX runtime_work_idle_owner ON runtime_work(owner,json_extract(record,'$.status'))")]
pub struct WorkRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<WorkItemRecord>)]
    pub record: RawJsonRecord<WorkItemRecord>,
    #[db(generated)]
    pub status: Option<String>,
    #[db(generated)]
    pub archive_status: Option<String>,
    #[db(generated)]
    pub archive_next_at: Option<f64>,
    #[db(generated)]
    pub vm_submit_status: Option<String>,
    #[db(generated)]
    pub vm_accept_status: Option<String>,
    #[db(generated)]
    pub owner: Option<String>,
}
fn check_runtime_work(row: &WorkRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_workspace_operations",
    state_check = crate::state_check::check_legacy_json::<WorkspaceOperationRecord>,
    database = Canvas,
    legacy_ddl = "CREATE TABLE runtime_workspace_operations (id TEXT PRIMARY KEY, record TEXT NOT NULL)",
    check = check_runtime_workspace_operations,
    legacy_index = "CREATE INDEX runtime_workspace_operation_agent_phase ON runtime_workspace_operations(json_extract(record,'$.agent'),json_extract(record,'$.phase'))",
    legacy_index = "CREATE INDEX runtime_workspace_operation_phase ON runtime_workspace_operations(\n                json_extract(record,'$.phase'), json_extract(record,'$.agent'))")]
pub struct WorkspaceOperationRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<WorkspaceOperationRecord>)]
    pub record: RawJsonRecord<WorkspaceOperationRecord>,
}
fn check_runtime_workspace_operations(row: &WorkspaceOperationRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

fn check_project(row: &ProjectRow) -> StoreResult<()> {
    use studio_runtime_schema::project_record::ProjectRecord;
    let id = match row.record.value() {
        ProjectRecord::Live(record) => &record.id,
        ProjectRecord::Removal(record) => &record.id,
    };
    if id == &row.id {
        Ok(())
    } else {
        Err(crate::StoreError::InvalidStoredObject {
            table: "runtime_projects",
            id: row.id.clone(),
        })
    }
}

impl WorkRow {
    pub(crate) fn from_record(
        connection: &rusqlite::Connection,
        record: &WorkItemRecord,
    ) -> StoreResult<Self> {
        Ok(Self {
            id: record.id().as_ref().to_owned(),
            record: RawJsonRecord::from_record(connection, record)?,
            status: None,
            archive_status: None,
            archive_next_at: None,
            vm_submit_status: None,
            vm_accept_status: None,
            owner: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::{Table, db_schema};

    #[test]
    fn planning_work_descriptors_install_and_validate_empty_tables() {
        for registration in db_schema![Canvas; AnnotationRow, CheckpointRow, PlanRow, ProjectRow, RuleRow, TaskRow, WorkRow, WorkspaceOperationRow]
        {
            let descriptor = registration.descriptor;
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch(descriptor.ddl).unwrap();
            for index in descriptor.indexes {
                db.execute_batch(index.sql).unwrap();
            }
            assert_eq!((registration.check)(&db).unwrap().counts(), (0, 0));
        }
    }

    #[test]
    fn planning_work_registry_retains_legacy_duplicate_acceptance_and_key_diagnostics() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(CheckpointRow::DDL).unwrap();
        db.execute(
            "INSERT INTO runtime_checkpoints VALUES('checkpoint',?1)",
            [r#"{"id":"old","id":"checkpoint","agent":null}"#],
        )
        .unwrap();
        let [registration] = db_schema![Canvas; CheckpointRow];
        assert_eq!((registration.check)(&db).unwrap().counts(), (1, 0));
        db.execute(
            "UPDATE runtime_checkpoints SET record=?1",
            [r#"{"id":"other"}"#],
        )
        .unwrap();
        let report = (registration.check)(&db).unwrap();
        assert_eq!(report.counts(), (1, 1));
        assert_eq!(report.failures[0].pointer, "/id");
        assert_eq!(report.failures[0].keyword, "rowKey");
    }

    #[test]
    fn planning_work_projects_retain_legacy_malformed_row_diagnostics() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(ProjectRow::DDL).unwrap();
        db.execute(
            "INSERT INTO runtime_projects(id,record) VALUES('project',?1)",
            [r#"{"recordKind":"live","id":"project","path":"/work","peerTeams":false}"#],
        )
        .unwrap();
        let expected =
            crate::state_check::check_legacy_json::<crate::ProjectLocationRecord>(&db).unwrap();
        assert_eq!(expected.counts(), (1, 1));
        assert!(!expected.failures.is_empty());
        let [registration] = db_schema![Canvas; ProjectRow];
        let actual = (registration.check)(&db).unwrap();
        assert_eq!(actual.counts(), expected.counts());
        assert_eq!(actual.failures, expected.failures);
    }

    #[test]
    fn planning_work_reads_preserve_unvalidated_generated_selector_evidence() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(WorkRow::DDL).unwrap();
        let raw = r#"{"id":"work","status":"ready","archiveIntent":{"status":"pending","nextAttemptAt":"unknown"}}"#;
        db.execute(
            "INSERT INTO runtime_work(id,record) VALUES('work',?1)",
            [raw],
        )
        .unwrap();
        assert!(
            crate::Table::<WorkItemRecord>::new(&db)
                .get(&crate::TaskId::from("work"))
                .unwrap()
                .is_some()
        );
        assert!(
            crate::WorkItemsRepository::work_record(&db, "work")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn planning_work_annotation_writer_preserves_caller_encoding() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(AnnotationRow::DDL).unwrap();
        let data = studio_runtime_schema::annotation_command::AnnotationData {
            id: "note".into(),
            agent: "agent".into(),
            root_id: "root".into(),
            path: "src".into(),
            line: 1,
            text: "review".into(),
            created: 1.0,
            turn_id: None,
        };
        let encoded = studio_json::to_string(&data).unwrap();
        let tx = db.transaction().unwrap();
        let record = RawJsonRecord::from_stored(&tx, "note", encoded.clone()).unwrap();
        crate::EventWriteRepository::create_annotation(&tx, record).unwrap();
        let stored: String = tx
            .query_row("SELECT record FROM runtime_annotations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, encoded);
    }

    #[test]
    fn planning_work_checkpoint_writer_preserves_caller_encoding() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(CheckpointRow::DDL).unwrap();
        let encoded = r#"{"agent":"agent","created":1.0,"id":"checkpoint","tree":"tree"}"#;
        let tx = db.transaction().unwrap();
        let record = RawJsonRecord::from_stored(&tx, "checkpoint", encoded.to_owned()).unwrap();
        crate::CheckpointsRepository::insert(&tx, record).unwrap();
        let stored: String = tx
            .query_row("SELECT record FROM runtime_checkpoints", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, encoded);
    }

    #[test]
    fn planning_work_existing_records_keep_external_keys_exact_text_and_generated_columns() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for ddl in [PlanRow::DDL, WorkRow::DDL, AnnotationRow::DDL] {
            db.execute_batch(ddl).unwrap();
        }
        let plan = " \n{\"id\":\"payload\",\"rootId\":\"root\",\"text\":\"\\u0061\",\"version\":1.0,\"updated\":null,\"steps\":[{\"opaque\":[9007199254740993,2.00]}]} \t";
        db.execute("INSERT INTO runtime_plans VALUES('external',?1)", [plan])
            .unwrap();
        let work = " \n{\"id\":\"work\",\"status\":\"ready\",\"owner\":\"worker\",\"archiveIntent\":{\"status\":\"pending\",\"nextAttemptAt\":2.5}} \t";
        db.execute(
            "INSERT INTO runtime_work(id,record) VALUES('work',?1)",
            [work],
        )
        .unwrap();
        db.execute(
            "INSERT INTO runtime_annotations(id,record,rev) VALUES('note','{\"id\":\"note\"}',7)",
            [],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        let mut typed = crate::peer_update_settings::canvas_tx(&tx);
        let row = typed
            .get::<PlanRow>(&("external".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(row.record.raw(), plan);
        assert_eq!(row.record.value().id(), "payload");
        typed.upsert(&row).unwrap();
        let row = typed.get::<WorkRow>(&("work".into(),)).unwrap().unwrap();
        assert_eq!(row.record.raw(), work);
        assert_eq!(row.archive_next_at, Some(2.5));
        assert_eq!(row.owner.as_deref(), Some("worker"));
        typed.upsert(&row).unwrap();
        let note = typed
            .get::<AnnotationRow>(&("note".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(note.rev, 7);
        typed.upsert(&note).unwrap();
        assert_eq!(
            tx.query_row("SELECT rev FROM runtime_annotations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            7
        );
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_work", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            work
        );
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_plans", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            plan
        );
        tx.rollback().unwrap();
    }
}
