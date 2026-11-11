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

#[cfg(test)]
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
        use crate::metadata_schema::TaskWindowState as Target;
        let key = Some(self.id.clone());
        let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
        let mut tx = crate::peer_update_settings::canvas_tx(transaction);
        let source_rowid = match tx.read().get::<Target>(&(identity.clone(),))? {
            Some(previous) => previous.source_rowid,
            None => next_metadata_rowid(
                transaction,
                crate::consolidation_kinds::CanvasMetadataKind::TaskWindowState,
            )?,
        };
        tx.partition::<Target>().put(&Target {
            identity,
            source_rowid,
            text_key: key,
            task_agent: self.agent.clone(),
            task_created: self.created,
            task_running: crate::metadata_values::StoredBoolean::from_stored(self.running),
        })
    }
}

/// Original-row ordering for new metadata rows, distinct from the physical rowid.
pub(crate) fn next_metadata_rowid(
    connection: &rusqlite::Connection,
    kind: crate::consolidation_kinds::CanvasMetadataKind,
) -> rusqlite::Result<i64> {
    crate::peer_update_settings::canvas_read(connection)
        .named(
            &queries::MetaRowidHigh::QUERY,
            &queries::MetaRowidHigh { kind },
        )?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(crate::StoreError::RecordContract(
                "metadata source rowid exhausted",
            )))
        })
}

fn sync_entity_state_check(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(crate::state_check::check_sync_entities(connection))
}

impl AgentRecordGeneration {
    pub fn get(connection: &rusqlite::Connection) -> rusqlite::Result<Option<Self>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::metadata_schema::AgentRecordGeneration>(&(
                studio_db_api::migration::encode_key(&(1_i64,)),
            ))?
            .map(|row| Self {
                id: row.integer_key,
                value: row.agent_record_generation,
            }))
    }
}

impl SyncEntityMeta {
    pub fn matches(
        connection: &rusqlite::Connection,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<bool> {
        Ok(Self::value(connection, key)?.as_deref() == Some(value))
    }
    fn target(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<crate::metadata_schema::SyncEntityMeta> {
        use crate::metadata_schema::{StoredSyncMetaValue, SyncEntityMeta as Target};
        let key = Some(key.to_owned());
        let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
        let source_rowid = match crate::peer_update_settings::canvas_read(transaction)
            .get::<Target>(&(identity.clone(),))?
        {
            Some(previous) => previous.source_rowid,
            None => next_metadata_rowid(
                transaction,
                crate::consolidation_kinds::CanvasMetadataKind::SyncEntityMeta,
            )?,
        };
        Ok(Target {
            identity,
            source_rowid,
            text_key: key.clone(),
            meta_value: StoredSyncMetaValue::from_stored(key, value.to_owned()),
        })
    }
    pub fn insert_if_absent(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<bool> {
        if Self::value(transaction, key)?.is_some() {
            return Ok(false);
        }
        crate::peer_update_settings::canvas_tx(transaction).insert_new(&Self::target(
            transaction,
            key,
            value,
        )?)
    }
    pub fn value(connection: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<String>> {
        let identity = crate::consolidation_codec::nullable_text_identity(&Some(key.to_owned()), 0);
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::metadata_schema::SyncEntityMeta>(&(identity,))?
            .map(|row| row.meta_value.as_stored().to_owned()))
    }
    pub fn put(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction)
            .partition::<crate::metadata_schema::SyncEntityMeta>()
            .put(&Self::target(transaction, key, value)?)?;
        crate::record_entity_metadata_updates(transaction.changes());
        Ok(())
    }
    pub fn insert(
        transaction: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction).insert(&Self::target(
            transaction,
            key,
            value,
        )?)?;
        crate::record_entity_metadata_updates(transaction.changes());
        Ok(())
    }
}

/// Exceptional clock reads. This module enumerates every declaration for schema checks.
#[studio_db_api::db_queries]
pub(crate) mod queries {
    use super::*;
    use studio_db_api::DbQuery;

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,
        decode=crate::metadata_schema::Metadata::SOURCE_ROWID,tables(rows=crate::metadata_schema::Metadata),
        columns(rowid=crate::metadata_schema::Metadata::SOURCE_ROWID,kind=crate::metadata_schema::Metadata::KIND),
        sql="SELECT COALESCE(MAX({rowid}),0) AS {rowid} FROM {rows} WHERE {kind}=?1")]
    pub struct MetaRowidHigh {
        #[db(column=crate::metadata_schema::Metadata::KIND)]
        pub kind: crate::consolidation_kinds::CanvasMetadataKind,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=crate::change_schema::Changes::SOURCE_ROWID,tables(rows=crate::change_schema::Changes),
        columns(rowid=crate::change_schema::Changes::SOURCE_ROWID,kind=crate::change_schema::Changes::KIND),
        sql="SELECT COALESCE(MAX({rowid}),0) AS {rowid} FROM {rows} WHERE {kind}=?1")]
    pub struct ChangeRowidHigh {
        #[db(column=crate::change_schema::Changes::KIND)]
        pub kind: crate::consolidation_kinds::CanvasChangesKind,
    }
    type ItemRowid = studio_db_api::Rowid<crate::item_monitor_terminal::ItemsRow>;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=optional,result=String,decode=crate::item_monitor_terminal::ItemsRow::ID,
        tables(items=crate::item_monitor_terminal::ItemsRow),columns(id=crate::item_monitor_terminal::ItemsRow::ID,rowid=ItemRowid::COLUMN),
        sql="SELECT {id} FROM {items} WHERE {rowid}=?1")]
    pub struct ItemIdByRowid {
        #[db(column=ItemRowid::COLUMN)]
        pub rowid: i64,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=crate::change_schema::Changes,items=crate::item_monitor_terminal::ItemsRow),
        columns(kind=crate::change_schema::Changes::KIND,identity=crate::change_schema::Changes::IDENTITY,
            source_rowid=crate::change_schema::Changes::SOURCE_ROWID,tbl=crate::change_schema::Changes::SOURCE_LABEL,
            id=crate::change_schema::Changes::DURABLE_ID,seq=crate::change_schema::Changes::SEQUENCE,
            deleted=crate::change_schema::Changes::DELETED,at=crate::change_schema::Changes::DELETED_AT,
            item_id=crate::item_monitor_terminal::ItemsRow::ID,rowid=ItemRowid::COLUMN),
        sql="INSERT INTO {rows}({kind},{identity},{source_rowid},{tbl},{id},{seq},{deleted},{at})
            SELECT ?3,?4,?5,?6,{item_id},?2+1,0,0 FROM {items} WHERE {rowid}=?1
            ON CONFLICT({kind},{identity}) DO NOTHING")]
    pub struct NumberItem<'a> {
        #[db(column=ItemRowid::COLUMN)]
        pub rowid: i64,
        #[db(column=DurableRows::SEQ)]
        pub high: i64,
        #[db(column=crate::change_schema::Changes::KIND)]
        pub kind: crate::consolidation_kinds::CanvasChangesKind,
        #[db(column=crate::change_schema::Changes::IDENTITY)]
        pub identity: &'a Vec<u8>,
        #[db(column=crate::change_schema::Changes::SOURCE_ROWID)]
        pub source_rowid: i64,
        #[db(column=DurableRows::TBL)]
        pub table: &'a str,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=crate::publication_schema::Publications::PUBLICATION_SEQ,
        tables(publications=crate::publication_schema::Publications,
            metadata=crate::metadata_schema::Metadata, changes=crate::change_schema::Changes),
        indexes(changes=crate::change_schema::Changes::INDEX_CHANGES_DURABLE_SEQUENCE),
        predicates(durable_rows=changes),
        columns(seq=crate::publication_schema::Publications::PUBLICATION_SEQ,
            generation=crate::metadata_schema::Metadata::SYNC_GENERATION,
            kind=crate::metadata_schema::Metadata::KIND,
            generation_id=crate::metadata_schema::Metadata::INTEGER_KEY,
            durable=crate::change_schema::Changes::SEQUENCE),
        sql="SELECT MAX((SELECT COALESCE(MAX({seq}),0) FROM {publications}),
            (SELECT COALESCE(MAX({generation}),0) FROM {metadata} WHERE {kind}=?1 AND {generation_id}=1),
            (SELECT COALESCE(MAX({durable}),0) FROM {changes} WHERE {durable_rows})) AS {seq}")]
    pub struct High {
        #[db(column=crate::metadata_schema::Metadata::KIND)]
        pub generation_kind: crate::consolidation_kinds::CanvasMetadataKind,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=DurableFloor::FLOOR,
        tables(rows=crate::metadata_schema::Metadata),
        columns(floor=DurableFloor::FLOOR,value=crate::metadata_schema::Metadata::DURABLE_FLOOR,
            kind=crate::metadata_schema::Metadata::KIND,id=crate::metadata_schema::Metadata::INTEGER_KEY),
        sql="SELECT {value} AS {floor} FROM {rows} WHERE {kind}=?1 AND {id}=1")]
    pub struct Floor {
        #[db(column=crate::metadata_schema::Metadata::KIND)]
        pub kind: crate::consolidation_kinds::CanvasMetadataKind,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,
        result=crate::durable_seq::DurableTombstone,projection=crate::durable_seq::DurableTombstone,
        tables(rows=crate::change_schema::Changes),indexes(rows=crate::change_schema::Changes::INDEX_CHANGES_DURABLE_SEQUENCE),predicates(durable=rows),
        columns(id=DurableRows::ID,seq=DurableRows::SEQ,value=crate::change_schema::Changes::SEQUENCE,
            actual_id=crate::change_schema::Changes::DURABLE_ID,tbl=crate::change_schema::Changes::SOURCE_LABEL,
            deleted=crate::change_schema::Changes::DELETED),
        sql="SELECT {actual_id} AS {id},{value} AS {seq} FROM {rows} WHERE {durable} AND {tbl}=?1 AND {deleted}=1 AND {value}>?2 AND {value}<=?3 ORDER BY {value}")]
    pub struct Tombstones<'a> {
        #[db(column=DurableRows::TBL)]
        pub table: &'a str,
        #[db(column=DurableRows::SEQ)]
        pub after: i64,
        #[db(column=DurableRows::SEQ)]
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

/// SQL high-plus-one for the shared durable/publication cursor, with INTEGER overflow refusal.
pub(crate) fn next_sequence_sql() -> String {
    let high = format!(
        "({})",
        queries::High::QUERY.sql().replace(
            "?1",
            crate::consolidation_kinds::CanvasMetadataKind::DB_CODE_SYNCGENERATION
        )
    );
    format!(
        "CASE WHEN typeof({high}+1)='integer' THEN {high}+1 ELSE RAISE(ABORT,'durable sequence exhausted') END"
    )
}

/// Seed only a newly created Canvas database; migration copies never repair clocks.
pub(crate) fn seed_fresh_clock_rows(
    transaction: &rusqlite::Transaction<'_>,
) -> crate::StoreResult<()> {
    use crate::metadata_schema::{
        AgentRecordGeneration, DurableFloor, SchedulerAgentChangeState, SyncGeneration,
    };
    let mut tx = crate::peer_update_settings::canvas_tx(transaction);
    let identity = studio_db_api::migration::encode_key(&(1_i64,));
    tx.insert_new(&SyncGeneration {
        identity: identity.clone(),
        source_rowid: 1,
        integer_key: 1,
        sync_generation: 0,
    })?;
    tx.insert_new(&DurableFloor {
        identity: identity.clone(),
        source_rowid: 1,
        integer_key: 1,
        durable_floor: 0,
    })?;
    tx.insert_new(&AgentRecordGeneration {
        identity: identity.clone(),
        source_rowid: 1,
        integer_key: 1,
        agent_record_generation: 0,
    })?;
    tx.insert_new(&SchedulerAgentChangeState {
        identity,
        source_rowid: 1,
        integer_key: 1,
        scheduler_floor: 0,
        scheduler_generation: 0,
        scheduler_entries: 0,
        scheduler_total: 0,
    })?;
    Ok(())
}

/// The change journal and transcript revision statements that the
/// `runtime_agents` triggers run, for a volatile row of agent `NEW.id`.
pub(crate) const JOURNAL_BODY: &str = "UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; \
     UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; \
     INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; \
     UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1;";
/// Canonical producer body for the authority transition; NEW is the source row.
/// The existing source owners whose writes advance a conversation clock.
#[derive(Clone, Copy)]
pub(crate) enum ClockSource {
    Agent,
    Item,
    Volatile,
}
impl ClockSource {
    fn agent_column(self) -> &'static str {
        match self {
            Self::Agent => {
                studio_db_api::query_column::<Canvas, _>(crate::agent_tables::AgentsStorageRow::ID)
            }
            Self::Item => studio_db_api::query_column::<Canvas, _>(
                crate::item_monitor_terminal::ItemsRow::AGENT,
            ),
            Self::Volatile => studio_db_api::query_column::<Canvas, _>(
                crate::agent_tables::AgentVolatileStorageRow::ID,
            ),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum AgentJournalEvent {
    Insert,
    Update,
    Delete,
}

pub(crate) fn target_journal_body(source: ClockSource, event: AgentJournalEvent) -> String {
    use crate::change_schema::Changes;
    use crate::consolidation_kinds::{
        CanvasChangesKind as ChangeKind, CanvasMetadataKind as MetaKind,
    };
    use crate::metadata_schema::Metadata;
    use studio_db_api::{
        migration::{identifier, key_sql},
        query_column as column,
    };
    let metadata = identifier(Metadata::NAME);
    let changes = identifier(Changes::NAME);
    let kind = column::<Canvas, _>(Metadata::KIND);
    let integer = column::<Canvas, _>(Metadata::INTEGER_KEY);
    let generation = column::<Canvas, _>(Metadata::AGENT_RECORD_GENERATION);
    let entries = column::<Canvas, _>(Metadata::SCHEDULER_ENTRIES);
    let total = column::<Canvas, _>(Metadata::SCHEDULER_TOTAL);
    let state_generation = column::<Canvas, _>(Metadata::SCHEDULER_GENERATION);
    let change_kind = column::<Canvas, _>(Changes::KIND);
    let identity = column::<Canvas, _>(Changes::IDENTITY);
    let rowid = column::<Canvas, _>(Changes::SOURCE_ROWID);
    let original = column::<Canvas, _>(Changes::ORIGINAL_KEY);
    let change_generation = column::<Canvas, _>(Changes::GENERATION);
    let source_id = source.agent_column();
    let gen_kind = MetaKind::DB_CODE_AGENTRECORDGENERATION;
    let state_kind = MetaKind::DB_CODE_SCHEDULERAGENTCHANGESTATE;
    let journal_kind = ChangeKind::DB_CODE_SCHEDULERAGENTCHANGES;
    // The logical identity is the shared nullable-TEXT framing, including its
    // NULL tie component. No trigger implements a second key encoding.
    let next_rowid = format!(
        "COALESCE((SELECT MAX({rowid}) FROM {changes} WHERE {change_kind}={journal_kind}),0)+1"
    );
    let encoded = key_sql(&["id", "tie"], "key");
    let high =
        format!("(SELECT {generation} FROM {metadata} WHERE {kind}={gen_kind} AND {integer}=1)");
    // These are the closed SQLite trigger aliases, never caller-supplied SQL.
    let touch = |old: bool, renamed: bool| {
        let row = if old { "OLD" } else { "NEW" };
        let guard = if renamed {
            format!("OLD.{source_id} IS NOT NEW.{source_id}")
        } else {
            "1".to_owned()
        };
        let key = format!(
            "(SELECT {encoded} FROM (SELECT {row}.{source_id} AS id,CASE WHEN {row}.{source_id} IS NULL THEN {next_rowid} END AS tie) AS key)"
        );
        let prior = format!(
            "SELECT {change_generation} FROM {changes} WHERE {change_kind}={journal_kind} AND {identity}={key}"
        );
        format!("UPDATE {metadata} SET {entries}={entries}+CASE WHEN EXISTS({prior}) THEN 0 ELSE 1 END,
            {total}={total}-COALESCE(({prior}),0)+{high} WHERE {kind}={state_kind} AND {integer}=1 AND {guard};
        INSERT INTO {changes}({change_kind},{identity},{rowid},{original},{change_generation})
            SELECT {journal_kind},{key},{next_rowid},{row}.{source_id},{high}
            WHERE {high} IS NOT NULL AND {guard}
            ON CONFLICT({change_kind},{identity}) DO UPDATE SET {change_generation}=excluded.{change_generation};")
    };
    let changes = match event {
        AgentJournalEvent::Insert => touch(false, false),
        AgentJournalEvent::Update => format!("{}{}", touch(true, true), touch(false, false)),
        AgentJournalEvent::Delete => touch(true, false),
    };
    format!(
        "UPDATE {metadata} SET {generation}={generation}+1 WHERE {kind}={gen_kind} AND {integer}=1;
        {changes}
        UPDATE {metadata} SET {state_generation}={high} WHERE {kind}={state_kind} AND {integer}=1;"
    )
}

/// Refresh the existing owned trigger names within the authority transaction.
pub(crate) fn target_agent_clock_sql() -> String {
    use studio_db_api::{migration::identifier, query_column};
    let table = identifier(crate::agent_tables::AgentsStorageRow::NAME);
    let id = query_column::<Canvas, _>(crate::agent_tables::AgentsStorageRow::ID);
    let record = query_column::<Canvas, _>(crate::agent_tables::AgentsStorageRow::RECORD);
    let rowid = query_column::<Canvas, _>(crate::agent_tables::AgentsStorageRow::ROWID);
    let mut sql = String::new();
    for (requirement, event, when, operation) in [
        (
            AGENT_CLOCK_TRIGGERS[0],
            "AFTER INSERT",
            String::new(),
            AgentJournalEvent::Insert,
        ),
        (
            AGENT_CLOCK_TRIGGERS[1],
            "AFTER UPDATE",
            format!(
                " WHEN OLD.{record} IS NOT NEW.{record} OR OLD.{id} IS NOT NEW.{id} OR OLD.{rowid} IS NOT NEW.{rowid}"
            ),
            AgentJournalEvent::Update,
        ),
        (
            AGENT_CLOCK_TRIGGERS[2],
            "AFTER DELETE",
            String::new(),
            AgentJournalEvent::Delete,
        ),
    ] {
        let name = identifier(requirement.name);
        let body = target_journal_body(ClockSource::Agent, operation);
        sql.push_str(&format!("DROP TRIGGER IF EXISTS {name}; CREATE TRIGGER {name} {event} ON {table}{when} BEGIN {body} END;"));
    }
    sql
}

pub(crate) const TRANSCRIPT_BODY: &str = "INSERT INTO runtime_transcript_revisions(agent,revision) VALUES (NEW.id,1) \
     ON CONFLICT(agent) DO UPDATE SET revision=revision+1;";

/// Refresh transcript producers on the two current source owners.
pub(crate) fn target_transcript_body(source: ClockSource, event: AgentJournalEvent) -> String {
    use crate::change_schema::Changes;
    use crate::consolidation_kinds::CanvasChangesKind as Kind;
    use studio_db_api::{migration::identifier, query_column};
    let target = identifier(Changes::NAME);
    let kind = query_column::<Canvas, _>(Changes::KIND);
    let identity = query_column::<Canvas, _>(Changes::IDENTITY);
    let rowid = query_column::<Canvas, _>(Changes::SOURCE_ROWID);
    let original = query_column::<Canvas, _>(Changes::ORIGINAL_KEY);
    let revision = query_column::<Canvas, _>(Changes::REVISION);
    let code = Kind::DB_CODE_TRANSCRIPTREVISIONS;
    let next = format!("COALESCE((SELECT MAX({rowid}) FROM {target} WHERE {kind}={code}),0)+1");
    let encoded = studio_db_api::migration::key_sql(&["id", "tie"], "key");
    let column = source.agent_column();
    let touch = |row: &str, renamed: bool| {
        let guard = if renamed {
            format!("OLD.{column}!=NEW.{column}")
        } else {
            "1".to_owned()
        };
        let key = format!(
            "(SELECT {encoded} FROM (SELECT {row}.{column} AS id,CASE WHEN {row}.{column} IS NULL THEN {next} END AS tie) AS key)"
        );
        format!(
            "INSERT INTO {target}({kind},{identity},{rowid},{original},{revision})
                    SELECT {code},{key},{next},{row}.{column},1 WHERE {guard}
                    ON CONFLICT({kind},{identity}) DO UPDATE SET {revision}={revision}+1;"
        )
    };
    match event {
        AgentJournalEvent::Insert => touch("NEW", false),
        AgentJournalEvent::Update => format!("{}{}", touch("NEW", false), touch("OLD", true)),
        AgentJournalEvent::Delete => touch("OLD", false),
    }
}

pub(crate) fn target_transcript_clock_sql() -> String {
    use studio_db_api::migration::identifier;
    let mut sql = String::new();
    for (table, source, names) in [
        (
            crate::agent_tables::AgentsStorageRow::NAME,
            ClockSource::Agent,
            AGENT_TRANSCRIPT_TRIGGERS,
        ),
        (
            crate::item_monitor_terminal::ItemsRow::NAME,
            ClockSource::Item,
            ITEM_TRANSCRIPT_TRIGGERS,
        ),
    ] {
        for (requirement, event, operation) in [
            (names[0], "AFTER INSERT", AgentJournalEvent::Insert),
            (names[1], "AFTER UPDATE", AgentJournalEvent::Update),
            (names[2], "AFTER DELETE", AgentJournalEvent::Delete),
        ] {
            let name = identifier(requirement.name);
            let table = identifier(table);
            let body = target_transcript_body(source, operation);
            sql.push_str(&format!("DROP TRIGGER IF EXISTS {name}; CREATE TRIGGER {name} {event} ON {table} BEGIN {body} END;"));
        }
    }
    sql
}
const ITEM_TRANSCRIPT_TRIGGERS: &[TriggerRequirement] = &[
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_items_insert",
        table: crate::item_monitor_terminal::ItemsRow::NAME,
    },
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_items_update",
        table: crate::item_monitor_terminal::ItemsRow::NAME,
    },
    TriggerRequirement {
        name: "sync_transcript_revision_runtime_items_delete",
        table: crate::item_monitor_terminal::ItemsRow::NAME,
    },
];

pub(crate) const AGENT_GENERATION_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
    columns: &[
        crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
        crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
        crate::metadata_schema::Metadata::__DB_COLUMN_AGENT_RECORD_GENERATION,
    ],
};
pub(crate) const SCHEDULER_STATE_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
    columns: &[
        crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
        crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
        crate::metadata_schema::Metadata::__DB_COLUMN_SCHEDULER_FLOOR,
        crate::metadata_schema::Metadata::__DB_COLUMN_SCHEDULER_GENERATION,
        crate::metadata_schema::Metadata::__DB_COLUMN_SCHEDULER_ENTRIES,
        crate::metadata_schema::Metadata::__DB_COLUMN_SCHEDULER_TOTAL,
    ],
};
pub(crate) const SCHEDULER_CHANGES_TABLE_REQUIREMENT: TableRequirement = TableRequirement {
    name: <crate::change_schema::Changes as studio_db_api::Table>::NAME,
    columns: &[
        crate::change_schema::Changes::__DB_COLUMN_KIND,
        crate::change_schema::Changes::__DB_COLUMN_IDENTITY,
        crate::change_schema::Changes::__DB_COLUMN_ORIGINAL_KEY,
        crate::change_schema::Changes::__DB_COLUMN_GENERATION,
    ],
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
        table_exists: <crate::change_schema::Changes as studio_db_api::Table>::NAME,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consolidation_codec::nullable_text_identity;
    use crate::metadata_schema::Metadata;
    use crate::peer_update_settings::{canvas_read, canvas_tx};

    #[test]
    fn journal_producer_keeps_generation_counts_and_rowid_on_canonical_targets() {
        use crate::agent_tables::{AgentVolatileStorageRow, AgentsStorageRow};
        use crate::change_schema::{Changes, SchedulerAgentChanges as Change};
        use crate::metadata_schema::{
            AgentRecordGeneration as Generation, SchedulerAgentChangeState as State,
        };
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            Metadata::descriptor(),
            Changes::descriptor(),
            crate::publication_schema::Publications::descriptor(),
            AgentVolatileStorageRow::descriptor(),
            AgentsStorageRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        let transaction = db.transaction().unwrap();
        seed_fresh_clock_rows(&transaction).unwrap();
        let mut typed = canvas_tx(&transaction);
        let singleton = studio_db_api::migration::encode_key(&(1_i64,));
        typed
            .partition::<Generation>()
            .put(&Generation {
                identity: singleton.clone(),
                source_rowid: 1,
                integer_key: 1,
                agent_record_generation: 40,
            })
            .unwrap();
        typed
            .partition::<State>()
            .put(&State {
                identity: singleton.clone(),
                source_rowid: 1,
                integer_key: 1,
                scheduler_floor: 3,
                scheduler_generation: 40,
                scheduler_entries: 0,
                scheduler_total: 0,
            })
            .unwrap();
        crate::bootstrap::install_current_producers(&transaction).unwrap();
        for epoch in [1, 2] {
            let raw = format!(
                r#"{{"id":"worker","provider":"codex","accountKey":"default","rootId":"root","epoch":{epoch},"status":"idle","deletedAt":1.5}}"#
            );
            let record =
                crate::json_record_codec::RawJsonRecord::from_stored(&transaction, "worker", raw)
                    .unwrap();
            typed
                .upsert(&AgentsStorageRow {
                    id: "worker".into(),
                    root_id: None,
                    deleted_at: None,
                    record,
                })
                .unwrap();
        }
        let state = typed
            .read()
            .get::<State>(&(singleton.clone(),))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                state.scheduler_generation,
                state.scheduler_entries,
                state.scheduler_total,
                state.scheduler_floor
            ),
            (42, 1, 42, 3)
        );
        assert_eq!(
            typed
                .read()
                .get::<Generation>(&(singleton,))
                .unwrap()
                .unwrap()
                .agent_record_generation,
            42
        );
        let change = typed
            .read()
            .get::<Change>(&(nullable_text_identity(&Some("worker".into()), 0),))
            .unwrap()
            .unwrap();
        assert_eq!((change.generation, change.source_rowid), (42, 1));
        // Refreshing producer DDL preserves counters; an unchanged record is silent.
        crate::bootstrap::install_current_producers(&transaction).unwrap();
        let unchanged = typed
            .get::<AgentsStorageRow>(&("worker".to_owned(),))
            .unwrap()
            .unwrap();
        typed.update_existing(&unchanged).unwrap();
        assert_eq!(
            AgentRecordGeneration::get(&transaction)
                .unwrap()
                .unwrap()
                .value,
            42
        );
        #[derive(studio_db_api::DbQuery)]
        #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
            tables(rows=AgentsStorageRow),columns(id=AgentsStorageRow::ID),sql="UPDATE {rows} SET {id}=?1 WHERE {id}=?2")]
        struct Rename<'a> {
            #[db(column=AgentsStorageRow::ID)]
            next: &'a str,
            #[db(column=AgentsStorageRow::ID)]
            prior: &'a str,
        }
        typed
            .named(
                &Rename::QUERY,
                &Rename {
                    next: "renamed",
                    prior: "worker",
                },
            )
            .unwrap();
        let state = typed
            .read()
            .get::<State>(&(studio_db_api::migration::encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                state.scheduler_generation,
                state.scheduler_entries,
                state.scheduler_total
            ),
            (43, 2, 86)
        );
        typed
            .delete::<AgentsStorageRow>(&("renamed".to_owned(),))
            .unwrap();
        let state = typed
            .read()
            .get::<State>(&(studio_db_api::migration::encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                state.scheduler_generation,
                state.scheduler_entries,
                state.scheduler_total
            ),
            (44, 2, 87)
        );
        transaction.rollback().unwrap();
        assert!(
            canvas_read(&db)
                .get::<Generation>(&(studio_db_api::migration::encode_key(&(1_i64,)),))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn transcript_producers_advance_canonical_revision_and_rollback() {
        use crate::agent_tables::{AgentVolatileStorageRow, AgentsStorageRow};
        use crate::item_monitor_terminal::ItemsRow;
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            crate::change_schema::Changes::descriptor(),
            crate::metadata_schema::Metadata::descriptor(),
            crate::publication_schema::Publications::descriptor(),
            AgentVolatileStorageRow::descriptor(),
            AgentsStorageRow::descriptor(),
            ItemsRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        let transaction = db.transaction().unwrap();
        seed_fresh_clock_rows(&transaction).unwrap();
        crate::bootstrap::install_current_producers(&transaction).unwrap();
        let mut typed = canvas_tx(&transaction);
        for epoch in [1, 2] {
            let raw = format!(
                r#"{{"id":"worker","provider":"codex","accountKey":"default","rootId":"root","epoch":{epoch},"status":"idle","deletedAt":1.5}}"#
            );
            let record =
                crate::json_record_codec::RawJsonRecord::from_stored(&transaction, "worker", raw)
                    .unwrap();
            typed
                .upsert(&AgentsStorageRow {
                    id: "worker".into(),
                    root_id: None,
                    deleted_at: None,
                    record,
                })
                .unwrap();
        }
        assert_eq!(
            crate::repositories::transcript::TranscriptRepository::revision(&transaction, "worker")
                .unwrap(),
            2
        );
        typed
            .delete::<AgentsStorageRow>(&("worker".to_owned(),))
            .unwrap();
        assert_eq!(
            crate::repositories::transcript::TranscriptRepository::revision(&transaction, "worker")
                .unwrap(),
            3
        );
        transaction.rollback().unwrap();
        assert_eq!(
            crate::repositories::transcript::TranscriptRepository::revision(&db, "worker").unwrap(),
            0
        );
    }

    #[test]
    fn metadata_markers_keep_insert_and_upsert_rowid_semantics_and_rollback() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        {
            let transaction = db.transaction().unwrap();
            assert!(SyncEntityMeta::insert_if_absent(&transaction, "marker", "one").unwrap());
            let identity = nullable_text_identity(&Some("marker".into()), 0);
            let before = canvas_read(&transaction)
                .get::<crate::metadata_schema::SyncEntityMeta>(&(identity.clone(),))
                .unwrap()
                .unwrap()
                .source_rowid;
            assert!(!SyncEntityMeta::insert_if_absent(&transaction, "marker", "ignored").unwrap());
            SyncEntityMeta::put(&transaction, "marker", "two").unwrap();
            assert!(SyncEntityMeta::matches(&transaction, "marker", "two").unwrap());
            assert!(!SyncEntityMeta::matches(&transaction, "marker", "one").unwrap());
            assert!(SyncEntityMeta::insert(&transaction, "marker", "duplicate").is_err());
            assert_eq!(
                SyncEntityMeta::value(&transaction, "marker")
                    .unwrap()
                    .as_deref(),
                Some("two")
            );
            assert_eq!(
                canvas_read(&transaction)
                    .get::<crate::metadata_schema::SyncEntityMeta>(&(identity,))
                    .unwrap()
                    .unwrap()
                    .source_rowid,
                before
            );
            assert!(
                !canvas_read(&transaction)
                    .table_exists::<SyncEntityMeta>()
                    .unwrap()
            );
        }
        assert!(SyncEntityMeta::value(&db, "marker").unwrap().is_none());
    }

    #[test]
    fn task_window_writer_preserves_integer_evidence_and_original_rowid() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut row = TaskWindowState {
            id: "task".into(),
            agent: "worker".into(),
            created: Some(1.5),
            running: 2,
        };
        row.upsert(&transaction).unwrap();
        let identity = nullable_text_identity(&Some("task".into()), 0);
        let before = canvas_read(&transaction)
            .get::<crate::metadata_schema::TaskWindowState>(&(identity.clone(),))
            .unwrap()
            .unwrap();
        assert_eq!(before.task_running.as_stored(), 2);
        row.created = Some(2.5);
        row.upsert(&transaction).unwrap();
        let after = canvas_read(&transaction)
            .get::<crate::metadata_schema::TaskWindowState>(&(identity,))
            .unwrap()
            .unwrap();
        assert_eq!(after.source_rowid, before.source_rowid);
        assert_eq!(after.task_created, Some(2.5));
        assert_eq!(after.task_agent, "worker");
        assert!(
            !canvas_read(&transaction)
                .table_exists::<TaskWindowState>()
                .unwrap()
        );
    }

    #[test]
    fn shared_high_seeks_the_owned_durable_sequence_index() {
        use crate::change_schema::Changes;
        let db = rusqlite::Connection::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let params = queries::High {
            generation_kind: crate::consolidation_kinds::CanvasMetadataKind::SyncGeneration,
        };
        let plan =
            crate::query_plan_test_support::explain(&db, queries::High::QUERY.sql(), &params)
                .unwrap()
                .join("\n");
        let index =
            studio_db_api::query_index::<Canvas, Changes>(&Changes::INDEX_CHANGES_DURABLE_SEQUENCE);
        assert!(
            plan.contains(index),
            "shared high scans unrelated changes instead of {index}: {plan}"
        );
        assert_eq!(
            canvas_read(&db)
                .named(&queries::High::QUERY, &params)
                .unwrap(),
            0
        );
    }

    #[test]
    fn canonical_floor_and_tombstone_queries_preserve_missing_and_cutoff_rules() {
        use crate::change_schema::{Changes, DurableRows as Target, DurableSource};
        use crate::metadata_values::StoredBoolean;
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        assert!(matches!(
            canvas_read(&db).named(
                &queries::Floor::QUERY,
                &queries::Floor {
                    kind: crate::consolidation_kinds::CanvasMetadataKind::DurableFloor
                }
            ),
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        tx.partition::<crate::metadata_schema::DurableFloor>()
            .put(&crate::metadata_schema::DurableFloor {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                durable_floor: 9,
            })
            .unwrap();
        assert_eq!(
            tx.read()
                .named(
                    &queries::Floor::QUERY,
                    &queries::Floor {
                        kind: crate::consolidation_kinds::CanvasMetadataKind::DurableFloor
                    }
                )
                .unwrap(),
            9
        );
        for (rowid, (label, id, seq, deleted)) in [
            ("runtime_work", "lower", 2, 1),
            ("runtime_work", "live", 3, 0),
            ("runtime_work", "middle", 4, 1),
            ("other", "other", 5, 1),
            ("runtime_work", "upper", 6, 1),
            ("runtime_work", "future", 7, 1),
            ("runtime_work", "legacy-two", 5, 2),
        ]
        .into_iter()
        .enumerate()
        {
            tx.partition::<Target>()
                .put(&Target {
                    identity: studio_db_api::migration::encode_key(&(
                        label.to_owned(),
                        id.to_owned(),
                    )),
                    source_rowid: rowid as i64 + 1,
                    source_label: DurableSource::from_stored(label.into()),
                    durable_id: id.into(),
                    sequence: seq,
                    deleted: StoredBoolean::from_stored(deleted),
                    deleted_at: 0,
                    scope: None,
                })
                .unwrap();
        }
        let actual = tx
            .read()
            .collect(
                &queries::Tombstones::QUERY,
                &queries::Tombstones {
                    table: "runtime_work",
                    after: 2,
                    high: 6,
                },
            )
            .unwrap();
        assert_eq!(
            actual,
            vec![
                crate::durable_seq::DurableTombstone {
                    id: "middle".into(),
                    seq: 4
                },
                crate::durable_seq::DurableTombstone {
                    id: "upper".into(),
                    seq: 6
                }
            ]
        );
    }

    #[test]
    fn record_generation_reader_uses_only_its_metadata_partition() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        assert!(AgentRecordGeneration::get(&db).unwrap().is_none());
        let transaction = db.transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<crate::metadata_schema::AgentRecordGeneration>()
            .put(&crate::metadata_schema::AgentRecordGeneration {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                agent_record_generation: 50,
            })
            .unwrap();
        assert_eq!(
            AgentRecordGeneration::get(&transaction)
                .unwrap()
                .unwrap()
                .value,
            50
        );
        assert!(
            !canvas_read(&transaction)
                .table_exists::<AgentRecordGeneration>()
                .unwrap()
        );
    }
}
