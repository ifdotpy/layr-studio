//! Agent/session descriptors preserve the legacy physical format.
//! Generated agent/attempt columns stay SQLite-owned; projections decode only selected fields.
use crate::json_record_codec::{ExactJson, ExactJsonCodec, JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use studio_db_api::DbTable;

// Live recovery candidates, matching AgentError::known_kind for text and
// single-key structured tags. Empty and ambiguous tag objects do not classify.
const LIMIT_RECOVERY: &str = "(deleted_at IS NULL OR deleted_at=0) AND (
    record->>'$.error.codexErrorInfo' IN ('usageLimitExceeded','rateLimitExceeded')
    OR (json_type(record,'$.error.codexErrorInfo.usageLimitExceeded') IS NOT NULL
        AND json_remove(record->'$.error.codexErrorInfo','$.usageLimitExceeded')='{}')
    OR (json_type(record,'$.error.codexErrorInfo.rateLimitExceeded') IS NOT NULL
        AND json_remove(record->'$.error.codexErrorInfo','$.rateLimitExceeded')='{}'))";

#[derive(DbTable)]
#[db(index(name="runtime_agent_limit_recovery",columns(account_key,id),predicate=LIMIT_RECOVERY))]
#[db(table = "runtime_agents", database = Canvas, legacy_ddl = "CREATE TABLE \"runtime_agents\"(id TEXT PRIMARY KEY,\"root_id\" TEXT GENERATED ALWAYS AS (record->>'$.rootId') STORED ,\"parent_id\" TEXT GENERATED ALWAYS AS (record->>'$.parentId') STORED ,\"account_key\" TEXT GENERATED ALWAYS AS (COALESCE(record->>'$.accountKey','default')) STORED NOT NULL,\"provider\" TEXT GENERATED ALWAYS AS (record->>'$.provider') STORED NOT NULL,\"role\" TEXT GENERATED ALWAYS AS (record->>'$.role') STORED ,\"deleted_at\"  GENERATED ALWAYS AS (record->>'$.deletedAt') STORED ,\"cwd\" TEXT GENERATED ALWAYS AS (record->>'$.cwd') STORED ,\"workspace_operation\"  GENERATED ALWAYS AS (record->>'$.workspaceOperation') STORED ,\"remote_worker\"  GENERATED ALWAYS AS (record->>'$.remoteWorker') STORED ,\"remote_anchor\"  GENERATED ALWAYS AS (record->>'$.remoteAnchor') STORED ,\"native_failure_hold\"  GENERATED ALWAYS AS (record->>'$.nativeFailureHold') STORED ,\"steer_rejected_turn_id\" TEXT GENERATED ALWAYS AS (record->>'$.steerRejectedTurnId') STORED ,\"native_tool_refresh_id\" TEXT GENERATED ALWAYS AS (record->>'$.nativeToolRefreshId') STORED ,\"context_repair_phase\" TEXT GENERATED ALWAYS AS (record->>'$.contextRepair.phase') STORED ,\"source_cleanup_phase\" TEXT GENERATED ALWAYS AS (record->>'$.contextRepair.sourceCleanup.phase') STORED ,\"source_cleanup_account_key\" TEXT GENERATED ALWAYS AS (record->>'$.contextRepair.sourceCleanup.accountKey') STORED ,\"source_cleanup_thread_id\" TEXT GENERATED ALWAYS AS (record->>'$.contextRepair.sourceCleanup.threadId') STORED ,\"has_disconnect_recovery\" INTEGER GENERATED ALWAYS AS (COALESCE(json_type(record,'$.disconnectRecovery')='object',0)) STORED NOT NULL,\"disconnect_recovery_stage\" TEXT GENERATED ALWAYS AS (record->>'$.disconnectRecovery.stage') STORED ,\"has_restart_recovery\" INTEGER GENERATED ALWAYS AS (COALESCE(json_type(record,'$.restartRecovery')='object',0)) STORED NOT NULL,\"restart_recovery_stage\" TEXT GENERATED ALWAYS AS (record->>'$.restartRecovery.stage') STORED ,\"has_workspace_cleanup_intent\" INTEGER GENERATED ALWAYS AS (COALESCE(json_type(record,'$.workspaceCleanupIntent')='object',0)) STORED NOT NULL,\"agent_archived\" INTEGER GENERATED ALWAYS AS (NOT (json_type(record,'$.agentArchive') IS NULL OR json_type(record,'$.agentArchive') IN ('null','false') OR (json_type(record,'$.agentArchive') IN ('integer','real') AND record->>'$.agentArchive'=0) OR (json_type(record,'$.agentArchive')='text' AND record->>'$.agentArchive'='') OR (json_type(record,'$.agentArchive')='array' AND json_array_length(record,'$.agentArchive')=0) OR (json_type(record,'$.agentArchive')='object' AND record->>'$.agentArchive'='{}'))) STORED NOT NULL,\"review_archive_failed\" INTEGER GENERATED ALWAYS AS (NOT (json_type(record,'$.reviewArchiveError') IS NULL OR json_type(record,'$.reviewArchiveError') IN ('null','false') OR (json_type(record,'$.reviewArchiveError') IN ('integer','real') AND record->>'$.reviewArchiveError'=0) OR (json_type(record,'$.reviewArchiveError')='text' AND record->>'$.reviewArchiveError'='') OR (json_type(record,'$.reviewArchiveError')='array' AND json_array_length(record,'$.reviewArchiveError')=0) OR (json_type(record,'$.reviewArchiveError')='object' AND record->>'$.reviewArchiveError'='{}'))) STORED NOT NULL,\"review_archive_marked\" INTEGER GENERATED ALWAYS AS (record->>'$.reviewArchiveScheduled' IS NOT NULL) STORED NOT NULL,\"review_archive_scheduled\" INTEGER GENERATED ALWAYS AS (COALESCE(json_type(record,'$.reviewArchiveScheduled')='text' AND record->>'$.reviewArchiveScheduled'<>'',0)) STORED NOT NULL,\"review_archive_next_at\"  GENERATED ALWAYS AS (record->>'$.reviewArchiveNextAt') STORED ,\"idle_episode_type\" TEXT GENERATED ALWAYS AS (json_type(record,'$.idleCompactionEpisode')) STORED ,\"idle_episode_fire_at\"  GENERATED ALWAYS AS (record->>'$.idleCompactionEpisode.fireAt') STORED ,\"idle_episode_id\"  GENERATED ALWAYS AS (record->>'$.idleCompactionEpisode.id') STORED ,\"legacy_unsettled\" INTEGER GENERATED ALWAYS AS (COALESCE(record->>'$.workspaceMode'='image' OR record->>'$.imageWorkspace'=1,0) AND COALESCE(record->>'$.legacyWorkspaceSettled',0)=0) STORED NOT NULL,record TEXT NOT NULL,hold_status TEXT NOT NULL DEFAULT 'clear' CHECK(\"hold_status\" IN ('clear','invalid','unrecognized')),hold_incident TEXT,title_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.titleJob.status' END) VIRTUAL CHECK(\"title_status\" IN ('scheduled','written','invalid','discarded','pending','submitted','unknown','not_sent','applied','failed')),vm_launch_status TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.vmLaunch.status' END) VIRTUAL CHECK(\"vm_launch_status\" IN ('completed','terminal','pending','retryable','submitted','unknown','not_sent','complete','applied','failed','cancelled')),idle_episode_outcome TEXT GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.idleCompactionEpisode.outcome' END) VIRTUAL CHECK(\"idle_episode_outcome\" IN ('compacted','disabled','compaction_off','idle_compaction_off','prefix_changed','started','dispatched','failed','ttl_changed','lapsed','small','newer_request','near_limit','active','queued_input','stopped','no_host','would_compact')),provider_stream_generation INTEGER GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.providerStream.generation' END) VIRTUAL,provider_stream_ack INTEGER GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.providerStream.ack' END) VIRTUAL,\"account_transfer_id\" GENERATED ALWAYS AS (record->>'$.accountTransferId') VIRTUAL,\"native_release_reset_pending\" GENERATED ALWAYS AS (record->>'$.nativeRelease.resetPending') VIRTUAL,\"is_lead\" GENERATED ALWAYS AS (record->>'$.isLead') VIRTUAL, \"auth_resume_proof_pending\"  GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN CASE WHEN json_type(record,'$.authResumeProofPending')='object' AND json(json_extract(record,'$.authResumeProofPending')) <> '{}' THEN 1 ELSE 0 END ELSE 0 END) VIRTUAL)", check = check_runtime_agents)]
#[db(legacy_index = "CREATE INDEX runtime_agent_account ON runtime_agents(account_key,deleted_at)")]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_account_wake_candidates_v7 ON runtime_agents(account_key,hold_status,id) WHERE deleted_at IS NULL AND COALESCE(remote_worker,0)=0 AND COALESCE(remote_anchor,0)=0"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_auth_resume_proof ON runtime_agents(id) WHERE auth_resume_proof_pending=1"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_cleanup_intent_v2 ON runtime_agents(id) WHERE has_workspace_cleanup_intent=1"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_deleted_unsettled_v2 ON runtime_agents(id) WHERE COALESCE(deleted_at,0)!=0"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_holds ON runtime_agents(id) WHERE hold_status!='clear'"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_idle_unload ON runtime_agents(account_key,deleted_at,id,account_transfer_id,native_release_reset_pending,is_lead)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_live_root_v2 ON runtime_agents(root_id,id) WHERE deleted_at IS NULL"
)]
#[db(legacy_index = "CREATE INDEX runtime_agent_parent_v2 ON runtime_agents(parent_id)")]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_reviewer_archive_pending_v2 ON runtime_agents(parent_id,id) WHERE role='reviewer' AND NOT agent_archived AND NOT review_archive_failed AND NOT review_archive_scheduled"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_reviewer_archive_retry_due_v2 ON runtime_agents(COALESCE(review_archive_next_at,0),id) WHERE role='reviewer' AND NOT agent_archived AND review_archive_marked"
)]
#[db(legacy_index = "CREATE INDEX runtime_agent_root_v2 ON runtime_agents(root_id)")]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_sweep_busy_v2 ON runtime_agents(account_key) WHERE provider='codex' AND (context_repair_phase IN ('preparing','submitted','unknown','ready') OR COALESCE(native_tool_refresh_id,'')!='')"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_sweep_cleanup_v2 ON runtime_agents(source_cleanup_account_key,source_cleanup_thread_id) WHERE source_cleanup_phase IN ('planned','submitted')"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_unsettled_legacy ON runtime_agents(id) WHERE legacy_unsettled=1"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_agent_workspace_operation ON runtime_agents(cwd) WHERE typeof(workspace_operation)='text' AND workspace_operation<>''"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_idle_compaction_deadline ON runtime_agents(idle_episode_type,idle_episode_fire_at,id,idle_episode_id) WHERE idle_episode_type='object'"
)]
#[db(state_check = crate::state_check::check_legacy_json::<crate::AgentRecord>)]
pub struct AgentsStorageRow {
    /// Stored account selector, generated from the record.
    #[db(generated)]
    pub account_key: String,
    #[db(key)]
    pub id: String,
    /// Existing generated selector; the storage owner never writes this field.
    #[db(generated, codec=crate::agent_columns::typed::AgentIdCodec)]
    pub root_id: Option<crate::AgentId>,
    /// Existing generated deletion time, in seconds; read-only.
    #[db(generated)]
    pub deleted_at: Option<f64>,
    #[db(codec = JsonRecordCodec<crate::AgentRecord>)]
    pub record: RawJsonRecord<crate::AgentRecord>,
}

impl AgentsStorageRow {
    /// Existing SQLite insertion order, available to narrow read projections.
    pub const ROWID: studio_db_api::Rowid<Self> = studio_db_api::Rowid::<Self>::COLUMN;
}

fn check_runtime_agents(row: &AgentsStorageRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_agent_volatile", database = Canvas, legacy_ddl = "CREATE TABLE runtime_agent_volatile(id TEXT PRIMARY KEY,\"events\" TEXT,\"lastEvent\" TEXT,\"activity\" TEXT,\"activeTools\" TEXT,\"tail\" TEXT,\"lastAnswer\" TEXT,\"contextUsage\" TEXT,\"tokensUsed\" TEXT,\"tokenUsageAccounting\" TEXT)")]
#[db(state_check = crate::state_check::count_legacy_rows::<AgentVolatileStorageRow>)]
pub struct AgentVolatileStorageRow {
    #[db(key)]
    pub id: String,
    #[db(column = "events", codec = ExactJsonCodec<EventCountPayload>)]
    pub events: Option<ExactJson<EventCountPayload>>,
    #[db(column = "lastEvent", codec = ExactJsonCodec<LastEventPayload>)]
    pub last_event: Option<ExactJson<LastEventPayload>>,
    #[db(column = "activity", codec = ExactJsonCodec<ActivityPayload>)]
    pub activity: Option<ExactJson<ActivityPayload>>,
    #[db(column = "activeTools", codec = ExactJsonCodec<ActiveToolsPayload>)]
    pub active_tools: Option<ExactJson<ActiveToolsPayload>>,
    #[db(column = "tail", codec = ExactJsonCodec<TailPayload>)]
    pub tail: Option<ExactJson<TailPayload>>,
    #[db(column = "lastAnswer", codec = ExactJsonCodec<LastAnswerPayload>)]
    pub last_answer: Option<ExactJson<LastAnswerPayload>>,
    #[db(column = "contextUsage", codec = ExactJsonCodec<ContextUsagePayload>)]
    pub context_usage: Option<ExactJson<ContextUsagePayload>>,
    #[db(column = "tokensUsed", codec = ExactJsonCodec<TokensUsedPayload>)]
    pub tokens_used: Option<ExactJson<TokensUsedPayload>>,
    #[db(column = "tokenUsageAccounting", codec = ExactJsonCodec<TokenAccountingPayload>)]
    pub token_usage_accounting: Option<ExactJson<TokenAccountingPayload>>,
}

#[derive(DbTable)]
#[db(table = "runtime_agent_moves", database = Canvas, legacy_ddl = "CREATE TABLE runtime_agent_moves(id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_runtime_agent_moves)]
#[db(state_check = crate::state_check::check_legacy_json::<crate::MoveRecord>)]
pub struct AgentMovesStorageRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<crate::MoveRecord>)]
    pub record: RawJsonRecord<crate::MoveRecord>,
}

fn check_runtime_agent_moves(row: &AgentMovesStorageRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "sessions", database = Canvas, legacy_ddl = "CREATE TABLE sessions(\n            id TEXT PRIMARY KEY, chat TEXT NOT NULL, thread_id TEXT NOT NULL,\n            account_key TEXT NOT NULL, state TEXT NOT NULL) STRICT")]
#[db(
    legacy_index = "CREATE INDEX sessions_thread ON sessions(thread_id,account_key) WHERE state<>'ended'"
)]
#[db(legacy_index = "CREATE INDEX sessions_chat ON sessions(chat)")]
#[db(state_check = crate::state_check::count_legacy_rows::<SessionsStorageRow>)]
pub struct SessionsStorageRow {
    #[db(key)]
    pub id: String,
    pub chat: String,
    pub thread_id: String,
    pub account_key: String,
    #[db(codec = DomainTextCodec)]
    pub state: crate::domain::SessionState,
    /// Hidden SQLite rowid, read only; physical DDL and logical key stay unchanged.
    #[db(generated, column = "rowid")]
    pub source_rowid: i64,
}

#[derive(DbTable)]
#[db(table = "turns", database = Canvas, check = check_turn, legacy_ddl = "CREATE TABLE turns(\n            id TEXT PRIMARY KEY, chat TEXT NOT NULL, state TEXT NOT NULL, session TEXT,\n            provider_turn TEXT, attempt TEXT, interrupt_ticket TEXT,\n            attempt_submitted INTEGER GENERATED ALWAYS AS (attempt->>'$.submitted') STORED,\n            attempt_connection_generation INTEGER GENERATED ALWAYS AS (attempt->>'$.connectionGeneration') STORED,\n            attempt_action TEXT GENERATED ALWAYS AS (attempt->>'$.action') STORED,\n            attempt_active_at_reservation INTEGER GENERATED ALWAYS AS (attempt->>'$.activeAtReservation') STORED) STRICT")]
#[db(
    legacy_index = "CREATE UNIQUE INDEX turns_one_attempt ON turns(chat) WHERE attempt IS NOT NULL"
)]
#[db(
    legacy_index = "CREATE INDEX turns_callback_context\n            ON turns(attempt_connection_generation,chat) WHERE attempt IS NOT NULL"
)]
#[db(
    legacy_index = "CREATE UNIQUE INDEX turns_one_unfinished\n            ON turns(chat) WHERE state IN ('starting','running')"
)]
#[db(legacy_index = "CREATE INDEX turns_provider_turn ON turns(chat,provider_turn)")]
#[db(state_check = crate::state_check::count_legacy_rows::<TurnsStorageRow>)]
pub struct TurnsStorageRow {
    #[db(key)]
    pub id: String,
    pub chat: String,
    /// Coupled state/session interpretation belongs to TurnState::from_columns.
    pub state: String,
    pub session: Option<String>,
    pub provider_turn: Option<String>,
    #[db(codec = ExactJsonCodec<TurnAttemptPayload>)]
    pub attempt: Option<ExactJson<TurnAttemptPayload>>,
    pub interrupt_ticket: Option<String>,
    /// Frozen generated source selectors retain SQLite's original affinity.
    #[db(generated)]
    pub attempt_submitted: Option<i64>,
    #[db(generated)]
    pub attempt_connection_generation: Option<i64>,
    #[db(generated)]
    pub attempt_action: Option<String>,
    #[db(generated)]
    pub attempt_active_at_reservation: Option<i64>,
    #[db(generated, column = "rowid")]
    pub source_rowid: i64,
}

#[derive(DbTable)]
#[db(table = "chats", database = Canvas, legacy_ddl = "CREATE TABLE chats(\n            id TEXT PRIMARY KEY, state TEXT NOT NULL, status TEXT NOT NULL,\n            auto_wake INTEGER, usage_resume_enabled INTEGER, execution_mode TEXT, turn_epoch INTEGER) STRICT")]
#[db(
    legacy_index = "CREATE INDEX chats_active_status ON chats(status) WHERE status IN ('running','starting','approval')"
)]
#[db(
    legacy_index = "CREATE INDEX chats_capacity_status ON chats(status) WHERE status IN ('starting','running','waiting','paused','approval')"
)]
#[db(legacy_index = "CREATE INDEX chats_vm ON chats(id) WHERE execution_mode='vm'")]
#[db(state_check = crate::state_check::count_legacy_rows::<ChatsStorageRow>)]
pub struct ChatsStorageRow {
    #[db(key)]
    pub id: String,
    #[db(codec = DomainTextCodec)]
    pub state: crate::domain::ChatState,
    #[db(codec = DomainTextCodec)]
    pub status: crate::AgentStatus,
    pub auto_wake: Option<bool>,
    pub usage_resume_enabled: Option<bool>,
    #[db(codec = DomainTextCodec)]
    pub execution_mode: Option<crate::AgentExecutionMode>,
    pub turn_epoch: Option<i64>,
}

#[derive(DbTable)]
#[db(table = "runtime_lead_requests", database = Canvas, legacy_ddl = "CREATE TABLE runtime_lead_requests (id TEXT PRIMARY KEY, agent TEXT NOT NULL, signature TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<LeadRequestsStorageRow>)]
pub struct LeadRequestsStorageRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub signature: String,
}

#[derive(DbTable)]
#[db(table = "command_send_admissions", database = Canvas, legacy_ddl = "CREATE TABLE command_send_admissions (id TEXT PRIMARY KEY, signature TEXT NOT NULL, created INTEGER NOT NULL) WITHOUT ROWID")]
#[db(
    legacy_index = "CREATE INDEX command_send_admissions_created ON command_send_admissions(created,id)"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<SendAdmissionsStorageRow>)]
pub struct SendAdmissionsStorageRow {
    #[db(key)]
    pub id: String,
    pub signature: String,
    pub created: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct EventCountPayload(pub f64);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct LastEventPayload(pub String);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ActivityPayload(pub crate::AgentActivity);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ActiveToolsPayload(pub Vec<crate::AgentActiveTool>);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TailPayload(pub String);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct LastAnswerPayload(pub String);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ContextUsagePayload(pub crate::AgentContextUsage);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TokensUsedPayload(pub f64);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TokenAccountingPayload(pub String);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TurnAttemptPayload(pub studio_runtime_schema::raw_json::RawJson);

/// Legacy physical TEXT is retained; the domain enums remain the vocabulary owners.
pub struct DomainTextCodec;
macro_rules! domain_state_codec {
    ($ty:ty) => {
        impl studio_db_api::Codec<$ty> for DomainTextCodec {
            type Stored = String;
            type Encoded<'a> = &'a str;
            fn encode<'a>(_: &rusqlite::Connection, value: &'a $ty) -> rusqlite::Result<&'a str> {
                Ok(value.column())
            }
            fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<$ty> {
                <$ty>::from_column(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            }
        }
    };
}
domain_state_codec!(crate::domain::SessionState);
domain_state_codec!(crate::domain::ChatState);
impl studio_db_api::Codec<crate::AgentStatus> for DomainTextCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a crate::AgentStatus,
    ) -> rusqlite::Result<&'a str> {
        Ok(value.as_str())
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<crate::AgentStatus> {
        crate::AgentStatus::parse(&value).map_err(|_| rusqlite::Error::InvalidQuery)
    }
}
impl studio_db_api::Codec<Option<crate::AgentExecutionMode>> for DomainTextCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a Option<crate::AgentExecutionMode>,
    ) -> rusqlite::Result<Self::Encoded<'a>> {
        Ok(value.map(|mode| match mode {
            crate::AgentExecutionMode::Native => "native",
            crate::AgentExecutionMode::Host => "host",
            crate::AgentExecutionMode::Vm => "vm",
        }))
    }
    fn decode(
        _: &rusqlite::Connection,
        value: Option<String>,
    ) -> rusqlite::Result<Option<crate::AgentExecutionMode>> {
        value
            .map(|value| {
                serde::Deserialize::deserialize(serde::de::value::StrDeserializer::<
                    serde::de::value::Error,
                >::new(&value))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .transpose()
    }
}
fn check_turn(row: &TurnsStorageRow) -> crate::StoreResult<()> {
    crate::domain::TurnState::from_columns(&row.state, row.session.clone())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_agent_visibility_columns_are_generated_and_never_written() {
        use studio_db_api::Table;
        let db = rusqlite::Connection::open_in_memory().unwrap();
        super::AgentVolatileStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        super::AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let record = crate::json_record_codec::RawJsonRecord::from_stored(
            &db,
            "worker",
            r#"{"id":"worker","provider":"codex","accountKey":"default","rootId":"root","epoch":1,"status":"idle","deletedAt":1.5}"#.into(),
        )
        .unwrap();
        let transaction = db.unchecked_transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&transaction)
            .insert(&super::AgentsStorageRow {
                account_key: String::new(),
                id: "worker".into(),
                record,
                root_id: None,
                deleted_at: Some(99.0),
            })
            .unwrap();
        let row = crate::peer_update_settings::canvas_read(&transaction)
            .get::<super::AgentsStorageRow>(&("worker".into(),))
            .unwrap()
            .unwrap();
        assert_eq!(row.root_id.as_ref().map(AsRef::as_ref), Some("root"));
        assert_eq!(row.deleted_at, Some(1.5));
        #[derive(studio_db_api::DbProjection)]
        #[db(of=super::AgentsStorageRow)]
        struct IdentityOrder {
            #[db(field=rowid)]
            source_rowid: i64,
        }
        let identities = studio_db_api::db_query!(
            crate::peer_update_settings::canvas_read(&transaction),
            super::AgentsStorageRow,
            project(IdentityOrder),
            filter(super::AgentsStorageRow::ID.eq_ref("worker"))
        )
        .unwrap();
        assert_eq!(identities.len(), 1);
        assert!(identities[0].source_rowid > 0);

        assert!(!super::AgentsStorageRow::INSERT.contains("root_id"));
        assert!(!super::AgentsStorageRow::INSERT.contains("deleted_at"));
        transaction.commit().unwrap();
        // The same descriptor also admits an existing table without replacement.
        super::AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let actual = studio_db_api::catalog::export(&db).unwrap();
        assert_eq!(
            actual
                .iter()
                .find(|entry| entry.name == "runtime_agents")
                .unwrap()
                .sql,
            super::AgentsStorageRow::DDL
        );
    }

    #[test]
    fn agent_family_descriptors_preserve_catalog_and_legacy_reports() {
        #[derive(serde::Deserialize)]
        struct Entry {
            name: String,
            sql: String,
        }
        #[derive(serde::Deserialize)]
        struct Database {
            sqlite_master: Vec<Entry>,
        }
        let snapshot: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).unwrap();
        let catalog: Database = studio_json::from_str(snapshot["canvas.sqlite3"].as_str()).unwrap();
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for entry in crate::canvas_schema::registry() {
            entry.descriptor.install_if_absent(&db).unwrap();
        }
        for name in [
            "runtime_agents",
            "runtime_agent_volatile",
            "runtime_agent_moves",
            "sessions",
            "turns",
            "chats",
            "runtime_lead_requests",
            "command_send_admissions",
        ] {
            let registration = crate::canvas_schema::registry()
                .into_iter()
                .find(|entry| entry.descriptor.name == name)
                .expect(name);
            let descriptor = registration.descriptor;
            for (name, sql) in std::iter::once((descriptor.name, descriptor.ddl)).chain(
                descriptor
                    .indexes
                    .iter()
                    .map(|index| (index.name, index.sql)),
            ) {
                assert_eq!(
                    catalog
                        .sqlite_master
                        .iter()
                        .find(|entry| entry.name == name)
                        .unwrap()
                        .sql,
                    sql,
                    "{name}"
                );
                let actual: String = db
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name=?1",
                        [name],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(actual, sql, "installed {name}");
            }
            assert_eq!(
                (registration.check)(&db).unwrap().counts(),
                (0, 0),
                "{name}"
            );
        }
        // Count-only reports admit malformed history and NULL keys without full-row decoding.
        db.execute(
            "INSERT INTO runtime_agent_volatile(id,tail) VALUES(NULL,'broken json')",
            [],
        )
        .unwrap();
        assert_eq!(
            crate::state_check::count_legacy_rows::<super::AgentVolatileStorageRow>(&db)
                .unwrap()
                .counts(),
            (1, 0)
        );
        db.execute(
            "INSERT INTO runtime_agent_volatile(id,tail) VALUES('bad','broken json')",
            [],
        )
        .unwrap();
        // A narrow projection must not decode the malformed tail column.
        let read = crate::peer_update_settings::canvas_read(&db);
        assert_eq!(
            studio_db_api::db_query!(
                read,
                super::AgentVolatileStorageRow,
                scalar(super::AgentVolatileStorageRow::ID),
                filter(super::AgentVolatileStorageRow::ID.eq_ref("bad"))
            )
            .unwrap(),
            Some("bad".to_owned())
        );
    }
    #[test]
    fn current_domain_descriptors_have_all_fresh_indexes() {
        let cold_db = rusqlite::Connection::open_in_memory().unwrap();
        for entry in crate::canvas_schema::registry()
            .into_iter()
            .filter(|entry| ["chats", "sessions", "turns"].contains(&entry.descriptor.name))
        {
            entry.descriptor.install_if_absent(&cold_db).unwrap();
        }
        let indexes = |db: &rusqlite::Connection| -> std::collections::BTreeSet<String> {
            db.prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name IN ('chats','sessions','turns') AND sql IS NOT NULL").unwrap()
                .query_map([], |row| row.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap()
        };
        let cold = indexes(&cold_db);
        let db = crate::fresh_state::test_canvas();
        let real = indexes(&db);
        assert_eq!(
            real, cold,
            "fresh domain indexes are owned directly by descriptors"
        );
    }

    #[test]
    fn agent_family_keeps_baseline_index_creation_order() {
        // SQLite uses index order to break equal-cost plans.
        let entry = crate::canvas_schema::registry()
            .into_iter()
            .find(|entry| entry.descriptor.name == "turns")
            .expect("turns descriptor");
        assert_eq!(
            entry
                .descriptor
                .indexes
                .iter()
                .map(|index| index.name)
                .collect::<Vec<_>>(),
            [
                "turns_one_attempt",
                "turns_callback_context",
                "turns_one_unfinished",
                "turns_provider_turn"
            ]
        );
    }

    #[test]
    fn domain_upgrade_keeps_session_index_creation_order() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&crate::domain::tables_sql()).unwrap();
        let indexes: Vec<String> = db.prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='sessions' AND sql IS NOT NULL ORDER BY rowid").unwrap().query_map([], |row| row.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(indexes, ["sessions_thread", "sessions_chat"]);
    }

    #[test]
    fn feature_preparation_recreates_exact_agent_indexes_in_order() {
        use studio_db_api::Table;
        let expected = [
            "runtime_agent_sweep_busy_v2",
            "runtime_agent_sweep_cleanup_v2",
            "runtime_agent_unsettled_legacy",
            "runtime_idle_compaction_deadline",
        ];
        let mut db = crate::fresh_state::test_canvas();
        for name in expected {
            db.execute_batch(&format!("DROP INDEX {name}")).unwrap();
        }
        let tx = db.transaction().unwrap();
        crate::feature_schema::prepare(&tx).unwrap();
        crate::feature_schema::prepare(&tx).unwrap();
        let actual: Vec<(String, String)> = tx.prepare("SELECT name,sql FROM sqlite_master WHERE type='index' AND name IN ('runtime_agent_sweep_busy_v2','runtime_agent_sweep_cleanup_v2','runtime_agent_unsettled_legacy','runtime_idle_compaction_deadline') ORDER BY rowid").unwrap().query_map([], |row| Ok((row.get(0)?,row.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(
            actual
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        for (name, sql) in actual {
            assert_eq!(
                sql,
                super::AgentsStorageRow::INDEXES
                    .iter()
                    .find(|index| index.name == name)
                    .unwrap()
                    .sql
            );
        }
    }

    #[test]
    fn legacy_state_codecs_follow_domain_vocabularies() {
        use studio_db_api::Codec;
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for mode in [
            None,
            Some(crate::AgentExecutionMode::Native),
            Some(crate::AgentExecutionMode::Host),
            Some(crate::AgentExecutionMode::Vm),
        ] {
            let encoded =
                <super::DomainTextCodec as Codec<Option<crate::AgentExecutionMode>>>::encode(
                    &db, &mode,
                )
                .unwrap();
            let decoded =
                <super::DomainTextCodec as Codec<Option<crate::AgentExecutionMode>>>::decode(
                    &db,
                    encoded.map(String::from),
                )
                .unwrap();
            assert_eq!(decoded, mode);
            if let Some(encoded) = encoded {
                assert_eq!(
                    studio_json::from_str::<String>(
                        &studio_json::to_string(&mode.unwrap()).unwrap()
                    )
                    .unwrap(),
                    encoded
                );
            }
        }
        assert!(
            <super::DomainTextCodec as Codec<Option<crate::AgentExecutionMode>>>::decode(
                &db,
                Some("future".into())
            )
            .is_err()
        );
        assert!(
            <super::DomainTextCodec as Codec<crate::domain::ChatState>>::decode(
                &db,
                "future".into()
            )
            .is_err()
        );
        assert!(
            <super::DomainTextCodec as Codec<crate::domain::SessionState>>::decode(
                &db,
                "future".into()
            )
            .is_err()
        );
        assert!(
            <super::DomainTextCodec as Codec<crate::AgentStatus>>::decode(&db, "future".into())
                .is_err()
        );
    }

    #[test]
    fn agent_family_json_keeps_exact_evidence() {
        use crate::json_record_codec::ExactJson;
        let raw = " 1.0 ";
        let count = ExactJson::<super::EventCountPayload>::from_stored(raw.into()).unwrap();
        assert_eq!(count.value().0, 1.0);
        assert_eq!(count.raw(), raw);
        let raw = r#" { "submitted": false, "submitted": true, "unknown": 1.0 } "#;
        let attempt = ExactJson::<super::TurnAttemptPayload>::from_stored(raw.into()).unwrap();
        assert_eq!(attempt.raw(), raw);
        let raw = r#" { "id":"old", "id":"move", "side":"source", "phase":"waiting", "agent":"a", "epoch":1.0, "server":"s", "result":{}, "fingerprint":"f", "created":1.0, "unknown":1, "unknown":2 } "#;
        let db = crate::fresh_state::test_canvas();
        let record = crate::json_record_codec::RawJsonRecord::<crate::MoveRecord>::from_stored(
            &db,
            "move",
            raw.into(),
        )
        .unwrap();
        assert_eq!(record.raw(), raw);
        record.check_key("move").unwrap();
        assert!(record.check_key("other").is_err());
    }
}
