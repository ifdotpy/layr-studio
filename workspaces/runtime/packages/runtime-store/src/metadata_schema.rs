//! Typed F1 metadata partitions. Installation and authority switch belong to the family migration.
use crate::consolidation_codec::{Required, RequiredCodec, RequiredJson, nullable_text_identity};
use crate::consolidation_kinds::CanvasMetadataKind as Kind;
use crate::json_record_codec::ExactJson;
use crate::metadata_values::{
    OrchestrationSettingsCodec, OrchestrationSettingsEvidence, PublicationTeamCounts,
    PublicationTeamCountsCodec, SearchPhase, SearchPhaseCodec, SidebarOrderPayload, StoredBoolean,
    StoredBooleanCodec,
};
use crate::peer_update_settings::Canvas;
use studio_db_api::migration::{DirtyKey, encode_key};
use studio_db_api::{DbPartition, DbTable};
use studio_runtime_schema::persistence::MigrationStatus;

/// The legacy nullable key has its own evidence branch. Named values reuse the
/// single key classifier and retain the original value spelling.
pub(crate) enum StoredSyncMetaValue {
    Keyed(crate::metadata_values::SyncMetaValue),
    NullKey(String),
}
impl StoredSyncMetaValue {
    pub fn from_stored(key: Option<String>, raw: String) -> Self {
        match key {
            Some(key) => Self::Keyed(crate::metadata_values::SyncMetaValue::from_stored(key, raw)),
            None => Self::NullKey(raw),
        }
    }
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Keyed(value) => Some(value.key()),
            Self::NullKey(_) => None,
        }
    }
    pub fn as_stored(&self) -> &str {
        match self {
            Self::Keyed(value) => value.as_stored(),
            Self::NullKey(raw) => raw,
        }
    }
}
pub(crate) struct StoredSyncMetaCodec;
impl studio_db_api::Codec<StoredSyncMetaValue> for StoredSyncMetaCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(
        _: &studio_db_api::CodecContext,
        value: &'a StoredSyncMetaValue,
    ) -> Result<&'a str, studio_db_api::CodecError> {
        Ok(value.as_stored())
    }
    fn decode(
        _: &studio_db_api::CodecContext,
        _: String,
    ) -> Result<StoredSyncMetaValue, studio_db_api::CodecError> {
        Err(studio_db_api::CodecError::InvalidQuery)
    }
    fn decode_row(
        _: &studio_db_api::CodecContext,
        row: &studio_db_api::CodecRow<'_>,
        index: usize,
    ) -> Result<StoredSyncMetaValue, studio_db_api::CodecError> {
        Ok(StoredSyncMetaValue::from_stored(
            row.get(Metadata::__DB_COLUMN_TEXT_KEY)?,
            row.get(index)?,
        ))
    }
}

const TEXT_KEY_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_CLIENTCOMMANDWATERMARKS,
    ",",
    Kind::DB_CODE_FEDERATIONSETTINGS,
    ",",
    Kind::DB_CODE_ORCHESTRATIONSETTINGS,
    ",",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ",",
    Kind::DB_CODE_SIDEBARORDER,
    ",",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ",",
    Kind::DB_CODE_SYNCENTITYMETA,
    ",",
    Kind::DB_CODE_SYNCIDENTITY,
    ",",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ",",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR text_key IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_CLIENTCOMMANDWATERMARKS,
    ",",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR text_key IS NOT NULL)"
);
const APPLIED_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_CLIENTCOMMANDWATERMARKS,
    ") OR applied IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_CLIENTCOMMANDWATERMARKS,
    ") OR applied IS NOT NULL)",
    " AND (applied IS NULL OR applied>=0)"
);
const SOURCE_ROWID_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ",",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ",",
    Kind::DB_CODE_ACCESSSETTINGS,
    ",",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ",",
    Kind::DB_CODE_FEDERATIONSETTINGS,
    ",",
    Kind::DB_CODE_ORCHESTRATIONSETTINGS,
    ",",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ",",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ",",
    Kind::DB_CODE_SEARCHROLLOUT,
    ",",
    Kind::DB_CODE_SIDEBARORDER,
    ",",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ",",
    Kind::DB_CODE_SYNCENTITYMETA,
    ",",
    Kind::DB_CODE_SYNCGENERATION,
    ",",
    Kind::DB_CODE_SYNCIDENTITY,
    ",",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ",",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ",",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR source_rowid IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ",",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ",",
    Kind::DB_CODE_ACCESSSETTINGS,
    ",",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ",",
    Kind::DB_CODE_FEDERATIONSETTINGS,
    ",",
    Kind::DB_CODE_ORCHESTRATIONSETTINGS,
    ",",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ",",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ",",
    Kind::DB_CODE_SEARCHROLLOUT,
    ",",
    Kind::DB_CODE_SIDEBARORDER,
    ",",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ",",
    Kind::DB_CODE_SYNCENTITYMETA,
    ",",
    Kind::DB_CODE_SYNCGENERATION,
    ",",
    Kind::DB_CODE_SYNCIDENTITY,
    ",",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ",",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ",",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR source_rowid IS NOT NULL)"
);
const INTEGER_KEY_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ",",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ",",
    Kind::DB_CODE_ACCESSSETTINGS,
    ",",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ",",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ",",
    Kind::DB_CODE_SEARCHROLLOUT,
    ",",
    Kind::DB_CODE_SYNCGENERATION,
    ",",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR integer_key IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ",",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ",",
    Kind::DB_CODE_ACCESSSETTINGS,
    ",",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ",",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ",",
    Kind::DB_CODE_SEARCHROLLOUT,
    ",",
    Kind::DB_CODE_SYNCGENERATION,
    ",",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR integer_key IS NOT NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ",",
    Kind::DB_CODE_ACCESSSETTINGS,
    ",",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ",",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ",",
    Kind::DB_CODE_SEARCHROLLOUT,
    ",",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR integer_key=1)"
);
const DURABLE_FLOOR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ") OR durable_floor IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_DURABLEFLOOR,
    ") OR durable_floor IS NOT NULL)"
);
const ALIAS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ") OR alias IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ACCESSLOCALALIAS,
    ") OR alias IS NOT NULL)"
);
const AUTO_PAIR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_ACCESSSETTINGS,
    ") OR auto_pair IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ACCESSSETTINGS,
    ") OR auto_pair IS NOT NULL)"
);
const AGENT_RECORD_GENERATION_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ") OR agent_record_generation IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_AGENTRECORDGENERATION,
    ") OR agent_record_generation IS NOT NULL)"
);
const FEDERATION_RECORD_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_FEDERATIONSETTINGS,
    ") OR federation_record IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_FEDERATIONSETTINGS,
    ") OR federation_record IS NOT NULL)"
);
const ORCHESTRATION_RECORD_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_ORCHESTRATIONSETTINGS,
    ") OR orchestration_record IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ORCHESTRATIONSETTINGS,
    ") OR orchestration_record IS NOT NULL)"
);
const MIGRATION_CURSOR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_cursor IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_cursor IS NOT NULL)"
);
const MIGRATION_COMPLETE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_complete IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_complete IS NOT NULL)"
);
const MIGRATION_STATUS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_status IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_status IS NOT NULL)"
);
const MIGRATION_UPDATED_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_updated IS NULL)"
);
const MIGRATION_ERROR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_error IS NULL)"
);
const MIGRATION_CUTOVER_MS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_cutover_ms IS NULL)"
);
const MIGRATION_CURSOR_KEY_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_cursor_key IS NULL)"
);
const MIGRATION_SOURCE_ROWS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_source_rows IS NULL)"
);
const MIGRATION_TARGET_ROWS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PAYLOADMIGRATIONS,
    ") OR migration_target_rows IS NULL)"
);
const SCHEDULER_FLOOR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_floor IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_floor IS NOT NULL)"
);
const SCHEDULER_GENERATION_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_generation IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_generation IS NOT NULL)"
);
const SCHEDULER_ENTRIES_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_entries IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_entries IS NOT NULL)"
);
const SCHEDULER_TOTAL_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_total IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SCHEDULERAGENTCHANGESTATE,
    ") OR scheduler_total IS NOT NULL)"
);
const SEARCH_PHASE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_phase IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_phase IS NOT NULL)"
);
const SEARCH_CURSOR_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_cursor IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_cursor IS NOT NULL)"
);
const SEARCH_UPDATED_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_updated IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SEARCHROLLOUT,
    ") OR search_updated IS NOT NULL)"
);
const SIDEBAR_RECORD_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SIDEBARORDER,
    ") OR sidebar_record IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SIDEBARORDER,
    ") OR sidebar_record IS NOT NULL)"
);
const BATCH_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch IS NOT NULL)"
);
const FIRST_ROWID_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR first_rowid IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR first_rowid IS NOT NULL)"
);
const LAST_ROWID_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR last_rowid IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR last_rowid IS NOT NULL)"
);
const BATCH_ROWS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch_rows IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch_rows IS NOT NULL)"
);
const PLAIN_BYTES_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR plain_bytes IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR plain_bytes IS NOT NULL)"
);
const STORED_BYTES_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR stored_bytes IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR stored_bytes IS NOT NULL)"
);
const SHA256_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR sha256 IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR sha256 IS NOT NULL)"
);
const BATCH_CREATED_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch_created IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_STOREDTEXTBATCHES,
    ") OR batch_created IS NOT NULL)"
);
const META_VALUE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SYNCENTITYMETA,
    ") OR meta_value IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SYNCENTITYMETA,
    ") OR meta_value IS NOT NULL)"
);
const SYNC_GENERATION_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_SYNCGENERATION,
    ") OR sync_generation IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_SYNCGENERATION,
    ") OR sync_generation IS NOT NULL)"
);
const TASK_AGENT_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ") OR task_agent IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ") OR task_agent IS NOT NULL)"
);
const TASK_CREATED_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ") OR task_created IS NULL)"
);
const TASK_RUNNING_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ") OR task_running IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_TASKWINDOWSTATE,
    ") OR task_running IS NOT NULL)"
);
const PUBLICATION_LIVE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR publication_live IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR publication_live IS NOT NULL)"
);
const PUBLICATION_IN_FLIGHT_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR publication_in_flight IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR publication_in_flight IS NOT NULL)"
);
const PUBLICATION_TEAM_COUNTS_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_AGENTPUBLICATIONSTATE,
    ") OR publication_team_counts IS NULL)"
);
const AUTO_UPDATE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR auto_update IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_PEERUPDATESETTINGS,
    ") OR auto_update IS NOT NULL)"
);
const DIRTY_MIGRATION_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_ONLINEMIGRATIONDIRTY,
    ") OR dirty_migration IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ONLINEMIGRATIONDIRTY,
    ") OR dirty_migration IS NOT NULL)"
);
const DIRTY_KEY_SHAPE: &str = studio_db_api::static_sql!(
    "(kind IN (",
    Kind::DB_CODE_ONLINEMIGRATIONDIRTY,
    ") OR dirty_key IS NULL)",
    " AND (kind NOT IN (",
    Kind::DB_CODE_ONLINEMIGRATIONDIRTY,
    ") OR dirty_key IS NOT NULL)",
    " AND (dirty_key IS NULL OR typeof(dirty_key) IN ('integer','blob'))"
);

const TASK_HISTORY: &str = studio_db_api::static_sql!(
    "kind=",
    Kind::DB_CODE_TASKWINDOWSTATE,
    " AND NOT task_running"
);
#[derive(DbTable)]
#[db(table="metadata",database=Canvas,partitioned_by=Kind,
index(name="metadata_client_command_watermarks_key",columns(text_key),unique,predicate=kind.eq(Kind::ClientCommandWatermarks)),
index(name="metadata_durable_floor_key",columns(integer_key),unique,predicate=kind.eq(Kind::DurableFloor)),
index(name="metadata_access_local_alias_key",columns(integer_key),unique,predicate=kind.eq(Kind::AccessLocalAlias)),
index(name="metadata_access_settings_key",columns(integer_key),unique,predicate=kind.eq(Kind::AccessSettings)),
index(name="metadata_agent_record_generation_key",columns(integer_key),unique,predicate=kind.eq(Kind::AgentRecordGeneration)),
index(name="metadata_federation_settings_key",columns(text_key),unique,predicate=kind.eq(Kind::FederationSettings)),
index(name="metadata_orchestration_settings_key",columns(text_key),unique,predicate=kind.eq(Kind::OrchestrationSettings)),
index(name="metadata_payload_migrations_key",columns(text_key),unique,predicate=kind.eq(Kind::PayloadMigrations)),
index(name="metadata_scheduler_agent_change_state_key",columns(integer_key),unique,predicate=kind.eq(Kind::SchedulerAgentChangeState)),
index(name="metadata_search_rollout_key",columns(integer_key),unique,predicate=kind.eq(Kind::SearchRollout)),
index(name="metadata_sidebar_order_key",columns(text_key),unique,predicate=kind.eq(Kind::SidebarOrder)),
index(name="metadata_stored_text_batches_key",columns(text_key,batch),unique,predicate=kind.eq(Kind::StoredTextBatches)),
index(name="metadata_sync_entity_meta_key",columns(text_key),unique,predicate=kind.eq(Kind::SyncEntityMeta)),
index(name="metadata_sync_generation_key",columns(integer_key),unique,predicate=kind.eq(Kind::SyncGeneration)),
index(name="metadata_sync_identity_key",columns(text_key),unique,predicate=kind.eq(Kind::SyncIdentity)),
index(name="metadata_task_window_state_key",columns(text_key),unique,predicate=kind.eq(Kind::TaskWindowState)),
index(name="metadata_agent_publication_state_key",columns(text_key),unique,predicate=kind.eq(Kind::AgentPublicationState)),
index(name="metadata_peer_update_settings_key",columns(integer_key),unique,predicate=kind.eq(Kind::PeerUpdateSettings)),
index(name="metadata_online_migration_dirty_key",columns(dirty_migration,dirty_key),unique,predicate=kind.eq(Kind::OnlineMigrationDirty)),
index(name="metadata_source_rowid",columns(kind,source_rowid),unique),
index(name="metadata_task_window_history",columns(task_created,source_rowid),predicate=TASK_HISTORY) )]
pub(crate) struct Metadata {
    #[db(key)]
    kind: Kind,
    #[db(key)]
    identity: Vec<u8>,
    #[db(check=TEXT_KEY_SHAPE)]
    text_key: Option<String>,
    #[db(check=APPLIED_SHAPE)]
    applied: Option<i64>,
    #[db(check=SOURCE_ROWID_SHAPE)]
    source_rowid: Option<i64>,
    #[db(check=INTEGER_KEY_SHAPE)]
    integer_key: Option<i64>,
    #[db(check=DURABLE_FLOOR_SHAPE)]
    durable_floor: Option<i64>,
    #[db(check=ALIAS_SHAPE)]
    alias: Option<String>,
    #[db(check=AUTO_PAIR_SHAPE)]
    auto_pair: Option<i64>,
    #[db(check=AGENT_RECORD_GENERATION_SHAPE)]
    agent_record_generation: Option<i64>,
    #[db(check=FEDERATION_RECORD_SHAPE)]
    federation_record: Option<String>,
    #[db(check=ORCHESTRATION_RECORD_SHAPE)]
    orchestration_record: Option<String>,
    #[db(check=MIGRATION_CURSOR_SHAPE)]
    migration_cursor: Option<i64>,
    #[db(check=MIGRATION_COMPLETE_SHAPE)]
    migration_complete: Option<i64>,
    #[db(check=MIGRATION_STATUS_SHAPE)]
    migration_status: Option<MigrationStatus>,
    #[db(check=MIGRATION_UPDATED_SHAPE)]
    migration_updated: Option<f64>,
    #[db(check=MIGRATION_ERROR_SHAPE)]
    migration_error: Option<String>,
    #[db(check=MIGRATION_CUTOVER_MS_SHAPE)]
    migration_cutover_ms: Option<i64>,
    #[db(check=MIGRATION_CURSOR_KEY_SHAPE)]
    migration_cursor_key: Option<Vec<u8>>,
    #[db(check=MIGRATION_SOURCE_ROWS_SHAPE)]
    migration_source_rows: Option<i64>,
    #[db(check=MIGRATION_TARGET_ROWS_SHAPE)]
    migration_target_rows: Option<i64>,
    #[db(check=SCHEDULER_FLOOR_SHAPE)]
    scheduler_floor: Option<i64>,
    #[db(check=SCHEDULER_GENERATION_SHAPE)]
    scheduler_generation: Option<i64>,
    #[db(check=SCHEDULER_ENTRIES_SHAPE)]
    scheduler_entries: Option<i64>,
    #[db(check=SCHEDULER_TOTAL_SHAPE)]
    scheduler_total: Option<i64>,
    #[db(check=SEARCH_PHASE_SHAPE)]
    search_phase: Option<String>,
    #[db(check=SEARCH_CURSOR_SHAPE)]
    search_cursor: Option<i64>,
    #[db(check=SEARCH_UPDATED_SHAPE)]
    search_updated: Option<f64>,
    #[db(check=SIDEBAR_RECORD_SHAPE)]
    sidebar_record: Option<String>,
    #[db(check=BATCH_SHAPE)]
    batch: Option<i64>,
    #[db(check=FIRST_ROWID_SHAPE)]
    first_rowid: Option<i64>,
    #[db(check=LAST_ROWID_SHAPE)]
    last_rowid: Option<i64>,
    #[db(check=BATCH_ROWS_SHAPE)]
    batch_rows: Option<i64>,
    #[db(check=PLAIN_BYTES_SHAPE)]
    plain_bytes: Option<i64>,
    #[db(check=STORED_BYTES_SHAPE)]
    stored_bytes: Option<i64>,
    #[db(check=SHA256_SHAPE)]
    sha256: Option<String>,
    #[db(check=BATCH_CREATED_SHAPE)]
    batch_created: Option<f64>,
    #[db(check=META_VALUE_SHAPE)]
    meta_value: Option<String>,
    #[db(check=SYNC_GENERATION_SHAPE)]
    sync_generation: Option<i64>,
    #[db(check=TASK_AGENT_SHAPE)]
    task_agent: Option<String>,
    #[db(check=TASK_CREATED_SHAPE)]
    task_created: Option<f64>,
    #[db(check=TASK_RUNNING_SHAPE)]
    task_running: Option<i64>,
    #[db(check=PUBLICATION_LIVE_SHAPE)]
    publication_live: Option<i64>,
    #[db(check=PUBLICATION_IN_FLIGHT_SHAPE)]
    publication_in_flight: Option<i64>,
    #[db(check=PUBLICATION_TEAM_COUNTS_SHAPE)]
    publication_team_counts: Option<String>,
    #[db(check=AUTO_UPDATE_SHAPE)]
    auto_update: Option<i64>,
    #[db(check=DIRTY_MIGRATION_SHAPE)]
    dirty_migration: Option<String>,
    #[db(check=DIRTY_KEY_SHAPE)]
    dirty_key: Option<DirtyKey>,
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::ClientCommandWatermarks,check=check_client_command_watermarks)]
pub(crate) struct ClientCommandWatermarks {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<String>)]
    pub text_key: String,
    #[db(codec=Required<i64>)]
    pub applied: i64,
}
fn check_client_command_watermarks(row: &ClientCommandWatermarks) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.text_key.clone(),)) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::DurableFloor,check=check_durable_floor)]
pub(crate) struct DurableFloor {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=Required<i64>)]
    pub durable_floor: i64,
}
fn check_durable_floor(row: &DurableFloor) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::AccessLocalAlias,check=check_access_local_alias)]
pub(crate) struct AccessLocalAlias {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=Required<String>)]
    pub alias: String,
}
fn check_access_local_alias(row: &AccessLocalAlias) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::AccessSettings,check=check_access_settings)]
pub(crate) struct AccessSettings {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub auto_pair: StoredBoolean,
}
fn check_access_settings(row: &AccessSettings) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::AgentRecordGeneration,check=check_agent_record_generation)]
pub(crate) struct AgentRecordGeneration {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=Required<i64>)]
    pub agent_record_generation: i64,
}
fn check_agent_record_generation(row: &AgentRecordGeneration) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::FederationSettings,check=check_federation_settings)]
pub(crate) struct FederationSettings {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=RequiredJson<crate::settings_schema::FederationSettingsPayload>)]
    pub federation_record: ExactJson<crate::settings_schema::FederationSettingsPayload>,
}
fn check_federation_settings(row: &FederationSettings) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::OrchestrationSettings,check=check_orchestration_settings)]
pub(crate) struct OrchestrationSettings {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=RequiredCodec<OrchestrationSettingsCodec,OrchestrationSettingsEvidence>)]
    pub orchestration_record: OrchestrationSettingsEvidence,
}
fn check_orchestration_settings(row: &OrchestrationSettings) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::PayloadMigrations,check=check_payload_migrations)]
pub(crate) struct PayloadMigrations {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=Required<i64>)]
    pub migration_cursor: i64,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub migration_complete: StoredBoolean,
    #[db(codec=Required<MigrationStatus>)]
    pub migration_status: MigrationStatus,
    pub migration_updated: Option<f64>,
    pub migration_error: Option<String>,
    pub migration_cutover_ms: Option<i64>,
    pub migration_cursor_key: Option<Vec<u8>>,
    pub migration_source_rows: Option<i64>,
    pub migration_target_rows: Option<i64>,
}
fn check_payload_migrations(row: &PayloadMigrations) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SchedulerAgentChangeState,check=check_scheduler_agent_change_state)]
pub(crate) struct SchedulerAgentChangeState {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=Required<i64>)]
    pub scheduler_floor: i64,
    #[db(codec=Required<i64>)]
    pub scheduler_generation: i64,
    #[db(codec=Required<i64>)]
    pub scheduler_entries: i64,
    #[db(codec=Required<i64>)]
    pub scheduler_total: i64,
}
fn check_scheduler_agent_change_state(row: &SchedulerAgentChangeState) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SearchRollout,check=check_search_rollout)]
pub(crate) struct SearchRollout {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=RequiredCodec<SearchPhaseCodec,SearchPhase>)]
    pub search_phase: SearchPhase,
    #[db(codec=Required<i64>)]
    pub search_cursor: i64,
    #[db(codec=Required<f64>)]
    pub search_updated: f64,
}
fn check_search_rollout(row: &SearchRollout) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SidebarOrder,check=check_sidebar_order)]
pub(crate) struct SidebarOrder {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=RequiredJson<SidebarOrderPayload>)]
    pub sidebar_record: ExactJson<SidebarOrderPayload>,
}
fn check_sidebar_order(row: &SidebarOrder) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::StoredTextBatches,check=check_stored_text_batches)]
pub(crate) struct StoredTextBatches {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<String>)]
    pub text_key: String,
    #[db(codec=Required<i64>)]
    pub batch: i64,
    #[db(codec=Required<i64>)]
    pub first_rowid: i64,
    #[db(codec=Required<i64>)]
    pub last_rowid: i64,
    #[db(codec=Required<i64>)]
    pub batch_rows: i64,
    #[db(codec=Required<i64>)]
    pub plain_bytes: i64,
    #[db(codec=Required<i64>)]
    pub stored_bytes: i64,
    #[db(codec=Required<String>)]
    pub sha256: String,
    #[db(codec=Required<f64>)]
    pub batch_created: f64,
}
fn check_stored_text_batches(row: &StoredTextBatches) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.text_key.clone(), row.batch)) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SyncEntityMeta,check=check_sync_entity_meta)]
pub(crate) struct SyncEntityMeta {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=RequiredCodec<StoredSyncMetaCodec,StoredSyncMetaValue>)]
    pub meta_value: StoredSyncMetaValue,
}
fn check_sync_entity_meta(row: &SyncEntityMeta) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid)
        || row.meta_value.key() != row.text_key.as_deref()
    {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SyncGeneration,check=check_sync_generation)]
pub(crate) struct SyncGeneration {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=Required<i64>)]
    pub sync_generation: i64,
}
fn check_sync_generation(row: &SyncGeneration) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::SyncIdentity,check=check_sync_identity)]
pub(crate) struct SyncIdentity {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
}
fn check_sync_identity(row: &SyncIdentity) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::TaskWindowState,check=check_task_window_state)]
pub(crate) struct TaskWindowState {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=Required<String>)]
    pub task_agent: String,
    pub task_created: Option<f64>,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub task_running: StoredBoolean,
}
fn check_task_window_state(row: &TaskWindowState) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::AgentPublicationState,check=check_agent_publication_state)]
pub(crate) struct AgentPublicationState {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    pub text_key: Option<String>,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub publication_live: StoredBoolean,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub publication_in_flight: StoredBoolean,
    #[db(codec=PublicationTeamCountsCodec)]
    pub publication_team_counts: Option<PublicationTeamCounts>,
}
fn check_agent_publication_state(row: &AgentPublicationState) -> crate::StoreResult<()> {
    if row.identity != nullable_text_identity(&row.text_key, row.source_rowid) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::PeerUpdateSettings,check=check_peer_update_settings)]
pub(crate) struct PeerUpdateSettings {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    pub source_rowid: i64,
    #[db(codec=Required<i64>)]
    pub integer_key: i64,
    #[db(codec=RequiredCodec<StoredBooleanCodec,StoredBoolean>)]
    pub auto_update: StoredBoolean,
}
fn check_peer_update_settings(row: &PeerUpdateSettings) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.integer_key,)) || row.source_rowid != row.integer_key {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Metadata,kind=Kind::OnlineMigrationDirty,check=check_online_migration_dirty)]
pub(crate) struct OnlineMigrationDirty {
    #[db(key)]
    pub identity: Vec<u8>,
    #[db(codec=Required<String>)]
    pub dirty_migration: String,
    #[db(codec=Required<DirtyKey>)]
    pub dirty_key: DirtyKey,
}
fn check_online_migration_dirty(row: &OnlineMigrationDirty) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.dirty_migration.clone(), row.dirty_key.clone())) {
        return Err(crate::StoreError::RecordContract(
            "metadata identity disagrees with original key",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::canvas_tx;
    use studio_db_api::{CodecContext, MutableTable, PartitionTable, Table, Tx};

    fn admit<R: PartitionTable<Database = Canvas, Key = (Vec<u8>,)> + MutableTable>(
        tx: &mut Tx<'_, Canvas>,
        row: &R,
        key: Vec<u8>,
    ) {
        tx.partition::<R>().put(row).unwrap();
        assert!(tx.get::<R>(&(key,)).unwrap().is_some());
    }

    #[test]
    fn f1_metadata_all_nineteen_partitions_share_one_table() {
        assert_eq!(
            Metadata::INDEXES.len(),
            21,
            "the physical owner installs every metadata index"
        );
        let mut db = CodecContext::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        let integer = encode_key(&(1_i64,));
        let name = Some("key".to_owned());
        let text = nullable_text_identity(&name, -7);
        macro_rules! put { ($ty:ident, $key:expr, $($field:ident:$value:expr),+ $(,)?) => {{ let identity=$key; admit(&mut tx, &$ty {identity:identity.clone(), $($field:$value),+}, identity); }}; }
        put!(ClientCommandWatermarks, encode_key(&("client".to_owned(),)), text_key:"client".into(), applied:9);
        put!(DurableFloor, integer.clone(), integer_key:1,source_rowid:1,durable_floor:9);
        put!(AccessLocalAlias,integer.clone(),integer_key:1,source_rowid:1,alias:"alias".into());
        put!(AccessSettings,integer.clone(),integer_key:1,source_rowid:1,auto_pair:StoredBoolean::LegacyInteger(-7));
        put!(AgentRecordGeneration,integer.clone(),integer_key:1,source_rowid:1,agent_record_generation:19);
        put!(FederationSettings,text.clone(),text_key:name.clone(),source_rowid:-7,federation_record:ExactJson::from_stored("{ \"enabled\":true, \"extra\":null }".into()).unwrap());
        put!(OrchestrationSettings,text.clone(),text_key:name.clone(),source_rowid:-7,orchestration_record:OrchestrationSettingsEvidence::from_stored("{}".into()).unwrap());
        put!(PayloadMigrations,text.clone(),text_key:name.clone(),source_rowid:-7,migration_cursor:3,migration_complete:StoredBoolean::Valid(false),migration_status:MigrationStatus::Running,migration_updated:Some(0.125),migration_error:None,migration_cutover_ms:Some(7),migration_cursor_key:Some(vec![0,1]),migration_source_rows:Some(2),migration_target_rows:Some(1));
        put!(SchedulerAgentChangeState,integer.clone(),integer_key:1,source_rowid:1,scheduler_floor:2,scheduler_generation:3,scheduler_entries:4,scheduler_total:5);
        put!(SearchRollout,integer.clone(),integer_key:1,source_rowid:1,search_phase:SearchPhase::Other("historical".into()),search_cursor:7,search_updated:0.625);
        put!(SidebarOrder,text.clone(),text_key:name.clone(),source_rowid:-7,sidebar_record:ExactJson::from_stored("42".into()).unwrap());
        put!(StoredTextBatches,encode_key(&("batch".to_owned(), 2_i64)),text_key:"batch".into(),source_rowid:-7,batch:2,first_rowid:-9,last_rowid:9,batch_rows:3,plain_bytes:17,stored_bytes:11,sha256:"unchanged historical text".into(),batch_created:0.875);
        put!(SyncEntityMeta,text.clone(),text_key:name.clone(),source_rowid:-7,meta_value:StoredSyncMetaValue::from_stored(name.clone(),"exact historical text".into()));
        put!(SyncGeneration,integer.clone(),integer_key:1,source_rowid:1,sync_generation:99);
        put!(SyncIdentity,text.clone(),text_key:name.clone(),source_rowid:-7);
        put!(TaskWindowState,text.clone(),text_key:name.clone(),source_rowid:-7,task_agent:"agent".into(),task_created:Some(0.375),task_running:StoredBoolean::LegacyInteger(2));
        put!(AgentPublicationState,text.clone(),text_key:name,source_rowid:-7,publication_live:StoredBoolean::Valid(true),publication_in_flight:StoredBoolean::LegacyInteger(-2),publication_team_counts:Some(PublicationTeamCounts::from_stored("not JSON".into())));
        put!(PeerUpdateSettings,integer.clone(),integer_key:1,source_rowid:1,auto_update:StoredBoolean::Valid(true));
        let dirty = DirtyKey::Composite(vec![0, 255]);
        put!(OnlineMigrationDirty,encode_key(&("migration".to_owned(),dirty.clone())),dirty_migration:"migration".into(),dirty_key:dirty);
        // Identical integer/text identities in other kinds retain their own typed fields.
        assert_eq!(
            tx.get::<AccessSettings>(&(integer.clone(),))
                .unwrap()
                .unwrap()
                .auto_pair
                .as_stored(),
            -7
        );
        assert_eq!(
            tx.get::<PayloadMigrations>(&(text.clone(),))
                .unwrap()
                .unwrap()
                .migration_updated,
            Some(0.125)
        );
        assert_eq!(
            tx.get::<StoredTextBatches>(&(encode_key(&("batch".to_owned(), 2_i64)),))
                .unwrap()
                .unwrap()
                .batch_created,
            0.875
        );
        assert_eq!(
            tx.get::<AgentPublicationState>(&(text.clone(),))
                .unwrap()
                .unwrap()
                .publication_team_counts
                .unwrap()
                .as_raw(),
            "not JSON"
        );
        tx.delete::<SyncIdentity>(&(text.clone(),)).unwrap();
        assert!(tx.get::<TaskWindowState>(&(text,)).unwrap().is_some());
        assert!(tx.get::<SyncGeneration>(&(integer,)).unwrap().is_some());
    }

    #[test]
    fn f1_metadata_preserves_dirty_key_classes_and_enforces_original_constraints() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        for dirty_key in [
            DirtyKey::Integer(-7),
            DirtyKey::Integer(0),
            DirtyKey::Composite(Vec::new()),
            DirtyKey::Composite(b"0".to_vec()),
        ] {
            let identity = encode_key(&("other".to_owned(), dirty_key.clone()));
            tx.partition::<OnlineMigrationDirty>()
                .put(&OnlineMigrationDirty {
                    identity: identity.clone(),
                    dirty_migration: "other".into(),
                    dirty_key: dirty_key.clone(),
                })
                .unwrap();
            assert_eq!(
                tx.get::<OnlineMigrationDirty>(&(identity,))
                    .unwrap()
                    .unwrap()
                    .dirty_key,
                dirty_key
            );
        }
        assert!(
            tx.partition::<ClientCommandWatermarks>()
                .put(&ClientCommandWatermarks {
                    identity: encode_key(&("client".to_owned(),)),
                    text_key: "client".into(),
                    applied: -1
                })
                .is_err()
        );
        assert!(
            tx.partition::<AccessSettings>()
                .put(&AccessSettings {
                    identity: encode_key(&(2_i64,)),
                    integer_key: 2,
                    source_rowid: 2,
                    auto_pair: StoredBoolean::Valid(true)
                })
                .is_err()
        );
    }
    #[derive(studio_db_api::DbProjection)]
    #[db(of=SyncEntityMeta)]
    struct ReorderedSyncMeta {
        meta_value: StoredSyncMetaValue,
        text_key: Option<String>,
    }
    #[test]
    fn f1_keyed_metadata_context_retains_null_keys_and_original_value_spelling() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        for (source_rowid, key, raw) in [
            (-7, Some("entity_tombstone_count"), "001"),
            (3, None, "legacy null-key value"),
        ] {
            let key = key.map(str::to_owned);
            let identity = nullable_text_identity(&key, source_rowid);
            tx.partition::<SyncEntityMeta>()
                .put(&SyncEntityMeta {
                    identity: identity.clone(),
                    source_rowid,
                    text_key: key.clone(),
                    meta_value: StoredSyncMetaValue::from_stored(key.clone(), raw.into()),
                })
                .unwrap();
            let stored = tx.get::<SyncEntityMeta>(&(identity,)).unwrap().unwrap();
            assert_eq!(stored.meta_value.key(), key.as_deref());
            assert_eq!(stored.meta_value.as_stored(), raw);
        }
        for projected in
            studio_db_api::db_query!(tx, SyncEntityMeta, project(ReorderedSyncMeta)).unwrap()
        {
            assert_eq!(projected.meta_value.key(), projected.text_key.as_deref());
        }
        assert!(
            studio_db_api::db_query!(tx, SyncEntityMeta, scalar(SyncEntityMeta::META_VALUE))
                .is_err()
        );
        let key = Some("different".to_owned());
        assert!(
            tx.partition::<SyncEntityMeta>()
                .put(&SyncEntityMeta {
                    identity: nullable_text_identity(&key, 5),
                    source_rowid: 5,
                    text_key: key,
                    meta_value: StoredSyncMetaValue::from_stored(
                        Some("original".into()),
                        "001".into()
                    ),
                })
                .is_err()
        );
    }
}
