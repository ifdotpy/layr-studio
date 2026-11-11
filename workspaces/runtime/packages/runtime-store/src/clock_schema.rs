//! Legacy clock and publication descriptors. Physical format and owner units are unchanged.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::peer_update_settings::Canvas;
use crate::schema::{ConditionalTriggerRequirement, TableRequirement, TriggerRequirement};
use studio_db_api::{DbTable, Table};

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<DurableFloor>)]
#[db(table = "durable_floor", database = Canvas, legacy_ddl = "CREATE TABLE durable_floor (\n        id INTEGER PRIMARY KEY, floor INTEGER NOT NULL)")]
pub struct DurableFloor {
    #[db(key)]
    pub id: i64,
    pub floor: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<DurableRows>)]
#[db(table = "durable_rows", database = Canvas, legacy_ddl = "CREATE TABLE durable_rows (\n        tbl TEXT NOT NULL, id TEXT NOT NULL, seq INTEGER NOT NULL,\n        deleted INTEGER NOT NULL DEFAULT 0, at INTEGER NOT NULL DEFAULT 0, scope TEXT,\n        PRIMARY KEY(tbl, id))")]
#[db(legacy_index = "CREATE INDEX durable_rows_seq ON durable_rows(seq)")]
#[db(legacy_index = "CREATE INDEX durable_rows_window_members ON durable_rows(tbl,deleted,id)")]
#[db(legacy_index = "CREATE INDEX durable_rows_tombstone_at ON durable_rows(at) WHERE deleted=1")]
#[db(
    legacy_index = "CREATE INDEX durable_rows_task_window_live\n                ON durable_rows(id) WHERE tbl='task_window' AND deleted=0"
)]
pub struct DurableRows {
    #[db(key)]
    pub tbl: String,
    #[db(key)]
    pub id: String,
    pub seq: i64,
    pub deleted: i64,
    pub at: i64,
    #[db(codec = ExactJsonCodec<crate::entity_scopes::DeletedScope>)]
    pub scope: Option<ExactJson<crate::entity_scopes::DeletedScope>>,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<AgentRecordGeneration>)]
#[db(table = "runtime_agent_record_generation", database = Canvas, legacy_ddl = "CREATE TABLE runtime_agent_record_generation (\n                    id INTEGER PRIMARY KEY CHECK(id=1), value INTEGER NOT NULL)")]
pub struct AgentRecordGeneration {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub value: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SchedulerAgentChangeState>)]
#[db(table = "runtime_scheduler_agent_change_state", database = Canvas, legacy_ddl = "CREATE TABLE runtime_scheduler_agent_change_state (id INTEGER PRIMARY KEY CHECK(id=1),floor INTEGER NOT NULL,generation INTEGER NOT NULL,entries INTEGER NOT NULL,total INTEGER NOT NULL)")]
pub struct SchedulerAgentChangeState {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub floor: i64,
    pub generation: i64,
    pub entries: i64,
    pub total: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SchedulerAgentChanges>)]
#[db(table = "runtime_scheduler_agent_changes", database = Canvas, legacy_ddl = "CREATE TABLE runtime_scheduler_agent_changes (id TEXT PRIMARY KEY,generation INTEGER NOT NULL CHECK(typeof(generation)='integer' AND generation>=0))")]
#[db(
    legacy_index = "CREATE INDEX runtime_scheduler_agent_change_generation ON runtime_scheduler_agent_changes(generation)"
)]
pub struct SchedulerAgentChanges {
    #[db(key)]
    pub id: String,
    #[db(check = "typeof(generation)='integer' AND generation>=0")]
    pub generation: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SyncDocuments>)]
#[db(table = "sync_documents", database = Canvas, legacy_ddl = "CREATE TABLE sync_documents (\n        seq INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT NOT NULL,\n        id TEXT NOT NULL, payload TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0,\n        UNIQUE(scope,id))")]
#[db(legacy_index = "CREATE INDEX sync_scope_seq ON sync_documents(scope,seq)")]
pub struct SyncDocuments {
    #[db(key)]
    pub seq: i64,
    pub scope: String,
    pub id: String,
    // Raw historical evidence: existing readers admit shapes beyond a typed envelope.
    pub payload: String,
    pub deleted: i64,
}

#[derive(DbTable)]
#[db(table = "sync_entities", database = Canvas, state_check = sync_entity_state_check, legacy_ddl = "CREATE TABLE sync_entities (\n        collection TEXT NOT NULL, id TEXT NOT NULL, seq INTEGER NOT NULL,\n        hash TEXT NOT NULL, payload TEXT, deleted INTEGER NOT NULL DEFAULT 0,\n        PRIMARY KEY(collection,id))")]
#[db(
    legacy_index = "CREATE INDEX sync_entities_collection_deleted\n        ON sync_entities(collection,deleted)"
)]
#[db(legacy_index = "CREATE INDEX sync_entities_collection_seq ON sync_entities(collection,seq)")]
#[db(
    legacy_index = "CREATE INDEX sync_entities_non_transcript_seq ON sync_entities(seq)\n        WHERE collection NOT LIKE 'transcript:%'"
)]
#[db(legacy_index = "CREATE INDEX sync_entities_seq ON sync_entities(seq)")]
#[db(
    legacy_index = "CREATE INDEX sync_entities_tombstone_order\n        ON sync_entities(seq,collection,id)\n        WHERE deleted=1 AND collection NOT LIKE 'transcript:%'"
)]
#[db(legacy_index = "CREATE INDEX sync_entities_id ON sync_entities(id)")]
pub struct SyncEntities {
    #[db(key)]
    pub collection: String,
    #[db(key)]
    pub id: String,
    pub seq: i64,
    pub hash: String,
    // Raw historical evidence: existing readers admit shapes beyond a typed envelope.
    pub payload: Option<String>,
    pub deleted: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SyncEntityMeta>)]
#[db(table = "sync_entity_meta", database = Canvas, legacy_ddl = "CREATE TABLE sync_entity_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)")]
pub struct SyncEntityMeta {
    #[db(key)]
    pub key: String,
    pub value: String,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SyncGeneration>)]
#[db(table = "sync_generation", database = Canvas, legacy_ddl = "CREATE TABLE sync_generation (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)")]
pub struct SyncGeneration {
    #[db(key)]
    pub id: i64,
    pub value: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SyncIdentity>)]
#[db(table = "sync_identity", database = Canvas, legacy_ddl = "CREATE TABLE sync_identity (id TEXT PRIMARY KEY)")]
pub struct SyncIdentity {
    #[db(key)]
    pub id: String,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<TaskWindowState>)]
#[db(table = "task_window_state", database = Canvas, legacy_ddl = "CREATE TABLE task_window_state (id TEXT PRIMARY KEY,\n                agent TEXT NOT NULL, created REAL, running INTEGER NOT NULL)")]
#[db(
    legacy_index = "CREATE INDEX task_window_state_history\n                ON task_window_state(created DESC) WHERE NOT running"
)]
pub struct TaskWindowState {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub created: Option<f64>,
    pub running: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<AgentPublicationState>)]
#[db(table = "agent_publication_state", database = Canvas, legacy_ddl = "CREATE TABLE agent_publication_state (id TEXT PRIMARY KEY,\n                live INTEGER NOT NULL, in_flight INTEGER NOT NULL,\n                team_counts TEXT)")]
pub struct AgentPublicationState {
    #[db(key)]
    pub id: String,
    pub live: i64,
    pub in_flight: i64,
    #[db(codec = ExactJsonCodec<studio_runtime_schema::agent_progress::TeamCounts>)]
    pub team_counts: Option<ExactJson<studio_runtime_schema::agent_progress::TeamCounts>>,
}

pub(crate) fn prepare_sync_tables(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::canvas_schema::install(
        connection,
        &[
            DurableFloor::descriptor(),
            DurableRows::descriptor(),
            SyncDocuments::descriptor(),
            SyncEntities::descriptor(),
            SyncEntityMeta::descriptor(),
            SyncGeneration::descriptor(),
            SyncIdentity::descriptor(),
        ],
    )?;
    prepare_durable_scope(connection)
}

pub(crate) fn prepare_durable_scope(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('durable_rows') WHERE name='scope')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        connection.execute_batch("ALTER TABLE durable_rows ADD COLUMN scope TEXT")?;
    }
    Ok(())
}

pub(crate) fn prepare_features(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::canvas_schema::install(
        connection,
        &[
            TaskWindowState::descriptor(),
            AgentPublicationState::descriptor(),
        ],
    )
}

impl TaskWindowState {
    pub fn upsert(&self, transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction).upsert(self)
    }
}

fn sync_entity_state_check(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::state_check::check_sync_entities(connection))
}

impl AgentRecordGeneration {
    pub fn get(connection: &rusqlite::Connection) -> rusqlite::Result<Option<Self>> {
        crate::peer_update_settings::canvas_read(connection).get::<Self>(&(1,))
    }
}

impl SyncEntityMeta {
    pub fn matches(
        connection: &rusqlite::Connection,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<bool> {
        let read = crate::peer_update_settings::canvas_read(connection);
        let (key, value) = (key.to_owned(), value.to_owned());
        Ok(!studio_db_api::db_query!(
            read,
            SyncEntityMeta,
            filter(
                SyncEntityMeta::KEY.eq(&key),
                SyncEntityMeta::VALUE.eq(&value)
            ),
            limit(1)
        )?
        .is_empty())
    }
    pub fn insert_if_absent(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<bool> {
        crate::peer_update_settings::canvas_tx(transaction).insert_new(&Self {
            key: key.to_owned(),
            value: value.to_owned(),
        })
    }
    pub fn value(connection: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<String>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<Self>(&(key.to_owned(),))?
            .map(|row| row.value))
    }
    pub fn put(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction).upsert(&Self {
            key: key.to_owned(),
            value: value.to_owned(),
        })
    }
    pub fn insert(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction).insert(&Self {
            key: key.to_owned(),
            value: value.to_owned(),
        })
    }
}

/// Exceptional clock reads. This module enumerates every declaration for schema checks.
#[studio_db_api::db_queries]
pub(crate) mod queries {
    use super::*;
    use studio_db_api::DbQuery;

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(progress=crate::settings_schema::PayloadMigrations),
        columns(name=crate::settings_schema::PayloadMigrations::NAME,
            count=crate::settings_schema::PayloadMigrations::COPY_TARGET_ROWS),
        sql="UPDATE {progress} SET {count}={count}+?2 WHERE {name}=?1")]
    pub struct AdjustCopyTargetRows<'a> {
        #[db(column=crate::settings_schema::PayloadMigrations::NAME)]
        pub name: &'a str,
        #[db(column=crate::settings_schema::PayloadMigrations::COPY_TARGET_ROWS)]
        pub delta: Option<i64>,
    }

    type ItemRowid = studio_db_api::Rowid<crate::item_monitor_terminal::ItemsRow>;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=DurableRows,items=crate::item_monitor_terminal::ItemsRow),
        columns(tbl=DurableRows::TBL,id=DurableRows::ID,seq=DurableRows::SEQ,
            deleted=DurableRows::DELETED,at=DurableRows::AT,
            item_id=crate::item_monitor_terminal::ItemsRow::ID,rowid=ItemRowid::COLUMN),
        sql="INSERT INTO {rows}({tbl},{id},{seq},{deleted},{at})
            SELECT 'runtime_items',{item_id},?2+1,0,0 FROM {items} WHERE {rowid}=?1
            ON CONFLICT({tbl},{id}) DO NOTHING")]
    pub struct NumberItem {
        #[db(column=ItemRowid::COLUMN)]
        pub rowid: i64,
        #[db(column=DurableRows::SEQ)]
        pub high: i64,
    }

    #[derive(DbQuery)]
    #[db_query(database = Canvas, mode = read, cardinality = one, result = i64,
        decode = SyncGeneration::VALUE,
        tables(entities = SyncEntities, documents = SyncDocuments, generation = SyncGeneration, durable = DurableRows),
        columns(entity_seq = SyncEntities::SEQ, document_seq = SyncDocuments::SEQ,
            value = SyncGeneration::VALUE, generation_id = SyncGeneration::ID, durable_seq = DurableRows::SEQ),
        sql = "SELECT MAX((SELECT COALESCE(MAX({entity_seq}),0) FROM {entities}),
            (SELECT COALESCE(MAX({document_seq}),0) FROM {documents}),
            (SELECT COALESCE(MAX({value}),0) FROM {generation} WHERE {generation_id}=1),
            (SELECT COALESCE(MAX({durable_seq}),0) FROM {durable})) AS {value}")]
    pub struct High;

    #[derive(DbQuery)]
    #[db_query(database = Canvas, mode = read, cardinality = one, result = i64,
        decode = DurableFloor::FLOOR,
        tables(floors = DurableFloor), columns(floor = DurableFloor::FLOOR, id = DurableFloor::ID),
        sql = "SELECT {floor} AS {floor} FROM {floors} WHERE {id}=1")]
    pub struct Floor;

    #[derive(DbQuery)]
    #[db_query(database = Canvas, mode = read, cardinality = many,
        result = crate::durable_seq::DurableTombstone, projection = crate::durable_seq::DurableTombstone,
        tables(rows = DurableRows), columns(id = DurableRows::ID, seq = DurableRows::SEQ,
            tbl = DurableRows::TBL, deleted = DurableRows::DELETED),
        sql = "SELECT {id} AS {id},{seq} AS {seq} FROM {rows} WHERE {tbl}=?1 AND {deleted}=1 AND {seq}>?2 AND {seq}<=?3 ORDER BY {seq}")]
    pub struct Tombstones<'a> {
        #[db(column = DurableRows::TBL)]
        pub table: &'a str,
        #[db(column = DurableRows::SEQ)]
        pub after: i64,
        #[db(column = DurableRows::SEQ)]
        pub high: i64,
    }
}

/// Validates all descriptor-owned clock declarations against a disposable schema.
#[cfg(any(test, feature = "test-support"))]
pub fn validate_named_queries(
    connection: &rusqlite::Connection,
) -> Result<(), studio_db_api::QueryValidationError> {
    queries::validate_queries(connection)
}

/// The change journal and transcript revision statements that the
/// `runtime_agents` triggers run, for a volatile row of agent `NEW.id`.
pub(crate) const JOURNAL_BODY: &str = "UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; \
     UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; \
     INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; \
     UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1;";
pub(crate) const TRANSCRIPT_BODY: &str = "INSERT INTO runtime_transcript_revisions(agent,revision) VALUES (NEW.id,1) \
     ON CONFLICT(agent) DO UPDATE SET revision=revision+1;";

pub(crate) const AGENT_GENERATION_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: AgentRecordGeneration::NAME,
    columns: &["id", "value"],
};
pub(crate) const SCHEDULER_STATE_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: SchedulerAgentChangeState::NAME,
    columns: &["id", "entries", "total", "generation"],
};
pub(crate) const SCHEDULER_CHANGES_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: SchedulerAgentChanges::NAME,
    columns: &["id", "generation"],
};
pub(crate) const AGENT_CLOCK_TRIGGERS: &[TriggerRequirement] = &[
    TriggerRequirement {
        name: "runtime_agent_record_generation_insert",
        table: "runtime_agents",
    },
    TriggerRequirement {
        name: "runtime_agent_record_generation_update",
        table: "runtime_agents",
    },
    TriggerRequirement {
        name: "runtime_agent_record_generation_delete",
        table: "runtime_agents",
    },
];
const AGENT_TRANSCRIPT_TRIGGERS: &[TriggerRequirement] = &[
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_agents_insert",
        table: "runtime_agents",
    },
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_agents_update",
        table: "runtime_agents",
    },
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_agents_delete",
        table: "runtime_agents",
    },
];

pub(crate) const AGENT_TRANSCRIPT_TRIGGER_REQUIREMENTS: &[ConditionalTriggerRequirement] =
    &[ConditionalTriggerRequirement {
        table_exists: crate::context_settings::TranscriptRevisionsStorageRow::NAME,
        triggers: AGENT_TRANSCRIPT_TRIGGERS,
    }];

pub(crate) fn agent_change_journal_exists(
    connection: &rusqlite::Connection,
) -> crate::StoreResult<bool> {
    crate::schema::table_exists(connection, AgentRecordGeneration::NAME)
}
pub(crate) fn transcript_revisions_exist(
    connection: &rusqlite::Connection,
) -> crate::StoreResult<bool> {
    crate::schema::table_exists(
        connection,
        crate::context_settings::TranscriptRevisionsStorageRow::NAME,
    )
}
pub(crate) fn prepare_agent_change_journal(
    transaction: &rusqlite::Transaction<'_>,
) -> crate::StoreResult<()> {
    if agent_change_journal_exists(transaction)?
        && crate::schema::table_exists(transaction, SchedulerAgentChangeState::NAME)?
    {
        let mut typed = crate::peer_update_settings::canvas_tx(transaction);
        typed.insert_new(&AgentRecordGeneration { id: 1, value: 0 })?;
        typed.insert_new(&SchedulerAgentChangeState {
            id: 1,
            floor: 0,
            generation: 0,
            entries: 0,
            total: 0,
        })?;
    }
    Ok(())
}
