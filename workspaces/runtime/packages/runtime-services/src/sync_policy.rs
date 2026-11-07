//! Pure sync document, cursor and preference decisions.
use studio_runtime_schema::shared_values::MODEL_CHOICES_PREFERENCE_KEY;

use crate::event_write::truthy;
pub use crate::sync_views::{
    AgentViewContext, ComplaintViewContext, RoomViewContext, agent_entity_view, openapi_document,
    project_persisted_record,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use studio_json::OwnedRawJson as RawValue;
use studio_json::{Map, Value, json};
use studio_runtime_schema::SYNC_ENTITY_SCOPE;
pub use studio_runtime_schema::shared_values::{SYNC_BATCH_LIMIT, SYNC_ENTITY_PAGE_LIMIT};
use studio_runtime_store::{
    RuntimeStore, StoreError, StoreResult, SyncDocumentsRepository, Table, TranscriptItemId,
    TranscriptItemRecord, agent_volatile, sha256_hex,
};
use thiserror::Error;

pub const ENTITY_PAGE_LIMIT: i64 = SYNC_ENTITY_PAGE_LIMIT;
pub const OTHER_PAGE_LIMIT: i64 = SYNC_BATCH_LIMIT;
const PREFERENCE_FIELD_CAP: usize = 3000;
const MAX_CLOCK_SKEW_MS: i64 = 5 * 60 * 1000;
// Independent projection caps, in monitor rows: terminal history and final
// window membership have separate meanings despite sharing a value.
const MONITOR_TERMINAL_WINDOW: usize = 100;
const MONITOR_FINAL_WINDOW: usize = 100;
use studio_runtime_store::startup_recovery::ACTIVE_MONITOR_STATUSES as MONITOR_ACTIVE_STATUSES;
const MONITOR_TERMINAL_STATUSES: [&str; 4] = ["completed", "failed", "cancelled", "lost"];
const TASK_RETIRE_BATCH: usize = 100;
/// Newest finished tasks the renderer task window keeps beside running ones;
/// the workspace entity advertises it as `tasksHistoryLimit`. Unit: rows.
pub const TASK_WINDOW_HISTORY_ROWS: usize = 100;
/// Restricts a task query to tasks of live (not deleted) agents.
const LIVE_TASK_AGENT: &str = "json_extract(t.record,'$.agent') IN \
     (SELECT id FROM runtime_agents WHERE deleted_at IS NULL)";
/// Newest runtime events the renderer event window keeps. Unit: rows. The
/// runtime_events triggers in runtime-store's sync_schema.sql use the same
/// bound; a parity test below checks every `LIMIT` there.
pub const EVENT_WINDOW_ROWS: usize = 200;
const SYNC_SEEDED_MARKER: &str = "seeded";
const AGENT_ORGANIZATION_FIELDS_MARKER: &str = "agent_organization_fields";
const AGENT_ORGANIZATION_FIELDS_VERSION: i64 = 3;
use studio_runtime_store::{
    AGENT_PROJECTIONS_SEED_MARKER, CONVERSATION_DETAILS_SEED_MARKER, SUMMARY_ENTITIES_SEED_MARKER,
    WORK_DETAILS_SEED_MARKER,
};

/// Whether Python's durable seed and entity-view upgrade markers prove the
/// persisted sync projection is complete. Legacy behavior: codex_sync_entities.seed.
pub fn has_complete_entity_seed(connection: &Connection) -> StoreResult<bool> {
    let seeded = connection
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key=?1",
            [SYNC_SEEDED_MARKER],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let version = connection
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key=?1",
            [AGENT_ORGANIZATION_FIELDS_MARKER],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let mut complete = seeded.as_deref() == Some("1")
        && version
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|value| value >= AGENT_ORGANIZATION_FIELDS_VERSION);
    if !complete {
        return Ok(false);
    }
    complete &= connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [SUMMARY_ENTITIES_SEED_MARKER],
        |row| row.get::<_, bool>(0),
    )?;
    complete &= connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [CONVERSATION_DETAILS_SEED_MARKER],
        |row| row.get::<_, bool>(0),
    )?;
    complete &= connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [AGENT_PROJECTIONS_SEED_MARKER],
        |row| row.get::<_, bool>(0),
    )?;
    complete &= connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [WORK_DETAILS_SEED_MARKER],
        |row| row.get::<_, bool>(0),
    )?;
    let table_exists = |name: &str| {
        connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
    };
    if table_exists("runtime_tasks")? && table_exists("runtime_agents")? {
        let task_window = connection
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key='task_window_migrated'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        complete &= task_window.as_deref() == Some("1");
    }
    if table_exists("runtime_events")? {
        if table_exists("runtime_agents")? {
            complete &= !connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM runtime_agents a WHERE a.deleted_at IS NULL AND NOT EXISTS(SELECT 1 FROM durable_rows q WHERE q.tbl='queue_window' AND q.id=a.id AND q.deleted=0))", [], |row| row.get::<_, bool>(0),
            )?;
        }
        let event_window = connection
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key='event_window_seq'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let projected_sequence = connection.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM sync_entities WHERE collection='event'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        complete &= event_window
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|sequence| sequence == projected_sequence);
    }
    Ok(complete)
}

/// Filesystem vault views and Mac folder sync reports captured before opening a
/// projection write transaction.
#[derive(Clone, Debug, Default)]
pub struct ProjectVaultSnapshot {
    projects: BTreeMap<String, Value>,
    views: BTreeMap<String, Result<Option<Value>, (&'static str, String)>>,
    /// The sidebar's Mac folder sync report per project (`layrSync`).
    sync_views: BTreeMap<String, Option<Value>>,
}

impl ProjectVaultSnapshot {
    /// Read SQL inputs, release the snapshot, then read vault files on the blocking pool.
    pub async fn capture_for_sync(store: &RuntimeStore, state_dir: &Path) -> StoreResult<Self> {
        let projects = store
            .read_async("runtime-services/sync-policy/projects", |connection| {
                Self::stored_projects(connection)
            })
            .await?;
        let state_dir = state_dir.to_path_buf();
        studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/sync-policy/vault",
            move || Self::capture(&state_dir, &projects),
        )
        .await
        .map_err(|_| StoreError::ReadTaskFailed)
    }

    /// Whether the SQL inputs still match the filesystem snapshot.
    pub fn matches_sources(&self, connection: &Connection) -> StoreResult<bool> {
        Ok(Self::stored_projects(connection)?
            .into_iter()
            .collect::<BTreeMap<_, _>>()
            == self.projects)
    }

    /// Valid stored projects for [`Self::capture`], read in a short snapshot so
    /// the filesystem capture can run after the read ends.
    pub fn stored_projects(connection: &Connection) -> StoreResult<Vec<(String, Value)>> {
        let mut statement =
            connection.prepare_cached("SELECT id,record FROM runtime_projects ORDER BY rowid")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut projects = Vec::new();
        for row in rows {
            let (id, raw) = row?;
            if valid_stored_record("runtime_projects", &id, &raw) {
                projects.push((id, studio_json::from_str(&raw)?));
            }
        }
        Ok(projects)
    }

    /// Read cached vault files outside SQL callbacks. Projection writers recheck
    /// the captured project records before committing.
    pub fn capture(state_dir: &Path, projects: &[(String, Value)]) -> Self {
        studio_runtime_store::debug_assert_outside_sql();
        let vm_state_dir = crate::vault::linux_vm_state_dir(state_dir);
        let mut snapshot = Self::default();
        for (id, project) in projects {
            snapshot.projects.insert(id.clone(), project.clone());
            let Some(source) = project_vault_source(project) else {
                snapshot.views.insert(id.clone(), Ok(None));
                continue;
            };
            let project_id = sha256_hex(source.as_bytes());
            snapshot.sync_views.insert(
                id.clone(),
                crate::mac_sync::sidebar_view(&vm_state_dir, &project_id),
            );
            let view = crate::vault::view(&vm_state_dir, &project_id).map_err(|error| {
                let object = match error {
                    crate::vault::VaultError::InvalidStoredRecord(object) => object,
                    crate::vault::VaultError::InvalidProjectId
                    | crate::vault::VaultError::AlreadyConfigured
                    | crate::vault::VaultError::ProjectSourceMissing
                    | crate::vault::VaultError::InvalidPrefix
                    | crate::vault::VaultError::InvalidSite
                    | crate::vault::VaultError::DeskMissing
                    | crate::vault::VaultError::InvalidConfiguration
                    | crate::vault::VaultError::GuestFailed
                    | crate::vault::VaultError::DeskTimedOut
                    | crate::vault::VaultError::DeskFailed
                    | crate::vault::VaultError::MissingRecipient
                    | crate::vault::VaultError::InvalidProfile
                    | crate::vault::VaultError::InvalidProfileOrRegion
                    | crate::vault::VaultError::InvalidPullFolder
                    | crate::vault::VaultError::NoPullTarget => "vault project id",
                };
                (object, project_id)
            });
            snapshot.views.insert(id.clone(), view);
        }
        snapshot
    }

    /// Hash the validated vault views used by project projections for a process-local skip key.
    pub fn signature(&self) -> String {
        let serialized = studio_json::to_vec(&(&self.views, &self.sync_views))
            .expect("project vault views serialize as JSON values and error tuples");
        studio_runtime_store::sha256_hex(&serialized)
    }

    fn recheck(&self, transaction: &Transaction<'_>) -> StoreResult<()> {
        let current = projection_records(transaction, "runtime_projects")?;
        if current.len() != self.projects.len()
            || current.into_iter().any(|(id, raw)| {
                studio_json::from_str::<Value>(&raw).ok().as_ref() != self.projects.get(&id)
            })
        {
            return Err(StoreError::RecordContract(
                "project records changed after cached vault views were read",
            ));
        }
        Ok(())
    }
}

/// Rebuild renderer entity projections from persisted records in the caller's
/// transaction. Re-running this operation preserves unchanged entity
/// sequences because the repository compares canonical payloads before write.
pub fn maintain_entity_projections(
    transaction: &Transaction<'_>,
    _state_dir: &str,
    vault_snapshot: &ProjectVaultSnapshot,
    rooms: &crate::sync_entity_context::RoomDirectory,
    validate_projection: impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let project_server_id =
        studio_runtime_store::RuntimeStore::transaction_local_server_id(transaction)?;
    maintain_entity_projections_with_server_id(
        transaction,
        _state_dir,
        project_server_id.as_deref(),
        vault_snapshot,
        rooms,
        validate_projection,
    )
}

pub fn maintain_entity_projections_with_server_id(
    transaction: &Transaction<'_>,
    state_dir: &str,
    project_server_id: Option<&str>,
    vault_snapshot: &ProjectVaultSnapshot,
    rooms: &crate::sync_entity_context::RoomDirectory,
    validate_projection: impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let mut source_records =
        crate::diagnostics::measure_runtime_stage("projection.source_snapshot", || {
            [
                "runtime_agents",
                "runtime_rooms",
                "runtime_complaints",
                "runtime_projects",
                "runtime_rules",
                "runtime_federation_rooms",
            ]
            .into_iter()
            .map(|table| {
                let rows = if matches!(table, "runtime_rooms" | "runtime_projects") {
                    stored_record_rows(transaction, table)
                } else {
                    projection_records(transaction, table)
                };
                rows.map(|records| (table, records))
            })
            .collect::<StoreResult<HashMap<_, _>>>()
        })?;
    source_records.insert(
        "runtime_rules",
        stored_record_rows(transaction, "runtime_rules")?,
    );
    let mut work_records = Vec::new();
    for (id, raw) in stored_record_rows(transaction, "runtime_work")? {
        if let Some(record) = checked_owned_work(&id, &raw)? {
            work_records.push(record);
        }
    }
    crate::diagnostics::measure_runtime_stage("projection.vault_recheck", || {
        vault_snapshot.recheck(transaction)
    })?;
    let peer_agents = source_records
        .get("runtime_agents")
        .ok_or(StoreError::RecordContract(
            "entity projection source snapshot is incomplete",
        ))?;
    let peer_projects =
        source_records
            .get("runtime_projects")
            .ok_or(StoreError::RecordContract(
                "entity projection source snapshot is incomplete",
            ))?;
    crate::diagnostics::measure_runtime_stage("projection.peer_team_entities", || {
        maintain_peer_team_entities(
            transaction,
            peer_agents,
            peer_projects,
            &validate_projection,
        )
    })?;
    crate::diagnostics::measure_runtime_stage("projection.agent_entities", || {
        let parsed = peer_agents
            .iter()
            .map(|(id, raw)| studio_json::from_str::<Value>(raw).map(|value| (id.as_str(), value)))
            .collect::<Result<Vec<_>, _>>()?;
        let agents = parsed
            .iter()
            .map(|(id, value)| (*id, value))
            .collect::<Vec<_>>();
        publish_agents(
            transaction,
            &agents,
            &work_records,
            project_server_id,
            &validate_projection,
        )
    })?;
    for (id, _) in peer_agents {
        crate::queue_write::publish_queue(transaction, id)?;
    }
    let mut current_rooms = BTreeSet::new();
    crate::diagnostics::measure_runtime_stage("projection.direct_entities", || {
        for (table, collection) in [
            ("runtime_rooms", "room"),
            ("runtime_complaints", "complaint"),
        ] {
            let records = source_records.get(table).ok_or(StoreError::RecordContract(
                "entity projection source snapshot is incomplete",
            ))?;
            for (id, raw) in records {
                if collection == "room" {
                    let Ok(room) = studio_json::from_str::<
                        studio_runtime_schema::room_record::RoomRecord,
                    >(raw) else {
                        continue;
                    };
                    if room.id != *id {
                        continue;
                    }
                    let context = rooms.for_room_identity(
                        transaction,
                        id,
                        room.kind == studio_runtime_schema::room_record::RoomKind::Federated,
                        true,
                    )?;
                    if let Some(projected) = crate::room_projection::project_room(&room, &context)?
                    {
                        current_rooms.insert(id.clone());
                        SyncDocumentsRepository::put_entity(
                            transaction,
                            "room",
                            id,
                            projected,
                            false,
                        )?;
                    }
                    continue;
                }
                if collection == "complaint" {
                    let complaint: studio_runtime_schema::complaint_record::ComplaintRecord =
                        studio_json::from_str(raw)?;
                    let context = ComplaintViewContext {
                        agents: rooms.agents(),
                    };
                    if let Some(view) =
                        crate::sync_views::project_complaint_record(&complaint, &context)
                    {
                        SyncDocumentsRepository::put_entity(
                            transaction,
                            collection,
                            id,
                            view,
                            false,
                        )?;
                    }
                }
            }
        }
        Ok::<(), StoreError>(())
    })?;
    if table_exists(transaction, "runtime_rooms")? {
        retire_unkept(
            transaction,
            "room",
            &current_rooms,
            None,
            &validate_projection,
        )?;
    }

    crate::diagnostics::measure_runtime_stage("projection.task_window", || {
        maintain_task_window(transaction, &validate_projection)
    })?;
    crate::diagnostics::measure_runtime_stage("projection.monitor_window", || {
        maintain_monitor_window(transaction, &validate_projection)
    })?;
    transaction.execute(
        "INSERT OR REPLACE INTO sync_entity_meta(key,value) VALUES(?1,'1')",
        [AGENT_PROJECTIONS_SEED_MARKER],
    )?;
    seed_summary_entities(transaction)?;
    transaction.execute(
        "INSERT OR REPLACE INTO sync_entity_meta(key,value) VALUES(?1,'1')",
        [CONVERSATION_DETAILS_SEED_MARKER],
    )?;
    transaction.execute(
        "INSERT OR REPLACE INTO sync_entity_meta(key,value) VALUES(?1,'1')",
        [WORK_DETAILS_SEED_MARKER],
    )?;
    crate::diagnostics::measure_runtime_stage("projection.canvas_chat_entities", || {
        maintain_canvas_chat_entities(transaction, &validate_projection)
    })?;

    // Python refreshes workspace/current after entity mutations only when a
    // workspace projection already exists. Initial creation belongs to seed
    // and explicit workspace publishers, not unrelated lifecycle mutations.
    let prior = transaction
        .query_row(
            "SELECT payload FROM sync_entities WHERE collection='workspace' AND id='current' AND deleted=0",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?;
    if let Some(Ok(payload)) = prior
        .flatten()
        .map(|payload| studio_json::from_str::<Value>(&payload))
        && let Some(mut workspace) = payload.get("value").and_then(Value::as_object).cloned()
    {
        workspace.insert("stateDir".to_owned(), Value::String(state_dir.to_owned()));
        workspace.entry("connected").or_insert(Value::Bool(false));
        workspace
            .entry("projectOrganizationVersion")
            .or_insert(json!(1));
        workspace.entry("peerTeamsVersion").or_insert(json!(1));
        workspace
            .entry("tasksHistoryLimit")
            .or_insert(json!(TASK_WINDOW_HISTORY_ROWS));
        put_projection(
            transaction,
            "workspace",
            "current",
            Value::Object(workspace),
            false,
            &validate_projection,
        )?;
    }
    Ok(())
}

/// A final agent row checked by the store decoder, kept in source row order.
/// Private fields prevent publication callers from substituting unchecked text.
#[derive(Clone, Debug)]
pub struct CheckedAgentRow {
    rowid: i64,
    id: String,
    value: Value,
}

impl CheckedAgentRow {
    pub(crate) fn decode(
        connection: &Connection,
        rowid: i64,
        id: &str,
        raw: &str,
    ) -> StoreResult<Self> {
        Ok(Self {
            rowid,
            id: id.to_owned(),
            value: studio_runtime_store::decode_agent_record(connection, id, raw)?.into_value(),
        })
    }
}

/// Source rows one writer transaction changed, by table. Writers pass these
/// ids so projection work is bounded by the change, not by stored history.
#[derive(Clone, Debug, Default)]
pub struct EntityChanges {
    pub agents: BTreeSet<String>,
    /// Team leads whose members changed: marked only when their team counts change.
    pub team_roots: BTreeSet<String>,
    /// Changed work rows. Their documents are built at read time (`entity_scope`); a
    /// change here refreshes only the owner agent's views.
    pub work: BTreeSet<String>,
    /// Final merged agent values already checked by this transaction.
    pub checked_agents: Vec<CheckedAgentRow>,
}

/// Publish the renderer views that depend on `changes`, using the same view
/// code as the full rebuild. A full rebuild afterwards changes nothing, except
/// that a deleted team's broadcast room keeps its last view (Python retains
/// it). Agent liveness changes also refresh peer teams, rooms naming those
/// agents, and the task and monitor windows that filter by owner liveness.
pub fn publish_entity_changes(
    transaction: &Transaction<'_>,
    changes: &EntityChanges,
    project_server_id: Option<&str>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let work = records_by_id(transaction, "runtime_work", &changes.work)?;
    let mut agent_ids = changes.agents.clone();
    for (_, raw) in &work {
        let record = studio_json::from_str::<Value>(raw)?;
        if let Some(owner) = record.get("owner").and_then(Value::as_str) {
            agent_ids.insert(owner.to_owned());
        }
    }
    let mut unread = agent_ids.clone();
    for row in &changes.checked_agents {
        unread.remove(&row.id);
    }
    let mut additional_agents = rows_by_id(transaction, "runtime_agents", &unread)?
        .into_iter()
        .map(|(rowid, id, raw)| {
            #[cfg(test)]
            studio_runtime_store::agent_volatile::publication_work::record(
                &id,
                studio_runtime_store::agent_volatile::publication_work::Operation::Decode,
            );
            studio_json::from_str::<Value>(&raw).map(|value| CheckedAgentRow { rowid, id, value })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Ending an acknowledged interruption can make a pending transfer
    // cancellable. Refresh only its owning lead, in this publication batch.
    let mut transfer_leads = BTreeSet::new();
    for row in additional_agents.iter().chain(&changes.checked_agents) {
        if !changes.agents.contains(&row.id)
            || row.value.get("inFlight").and_then(Value::as_bool) != Some(false)
        {
            continue;
        }
        let Some(root) = row
            .value
            .get("rootId")
            .and_then(Value::as_str)
            .filter(|root| *root != row.id)
        else {
            continue;
        };
        let was_in_flight: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_publication_state WHERE id=?1 AND live=1 AND in_flight=1)",
            [&row.id], |row| row.get(0),
        )?;
        if !was_in_flight || !table_exists(transaction, "runtime_account_transfers")? {
            continue;
        }
        let is_member: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_agents lead JOIN runtime_account_transfers op ON op.id=COALESCE(json_extract(lead.record,'$.accountTransferId'),json_extract(lead.record,'$.accountTransfer.id')) WHERE lead.id=?1 AND json_extract(op.record,'$.status')='pending' AND EXISTS(SELECT 1 FROM json_each(op.record,'$.members') member WHERE member.key=?2 AND json_extract(member.value,'$.interruptOutcome')='acknowledged'))",
            rusqlite::params![root, row.id], |row| row.get(0),
        )?;
        if is_member && !agent_ids.contains(root) {
            transfer_leads.insert(root.to_owned());
        }
    }
    agent_ids.extend(transfer_leads.iter().cloned());
    additional_agents.extend(
        rows_by_id(transaction, "runtime_agents", &transfer_leads)?
            .into_iter()
            .map(|(rowid, id, raw)| {
                studio_json::from_str::<Value>(&raw).map(|value| CheckedAgentRow {
                    rowid,
                    id,
                    value,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    let mut ordered_agents = additional_agents
        .iter()
        .chain(&changes.checked_agents)
        .collect::<Vec<_>>();
    ordered_agents.sort_unstable_by_key(|row| row.rowid);
    let agents = ordered_agents
        .iter()
        .map(|row| (row.id.as_str(), &row.value))
        .collect::<Vec<_>>();
    let mut liveness_changed = BTreeSet::new();
    let mut room_agents = BTreeSet::new();
    let mut changed_roots = BTreeSet::new();
    for &(id, record) in &agents {
        let previous: Option<(Option<String>, Option<bool>)> = transaction
            .query_row(
                "SELECT root_id,is_lead FROM agent_publication_state WHERE id=?1 AND live=1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((old_root, old_lead)) = previous {
            let new_root = record.get("rootId").and_then(Value::as_str);
            let new_lead = record.get("isLead").and_then(Value::as_bool);
            if old_root.as_deref() != new_root || old_lead != new_lead {
                room_agents.insert(id.to_owned());
                changed_roots.extend(old_root);
                changed_roots.extend(new_root.map(str::to_owned));
            }
        }
        let projected_live: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_publication_state WHERE id=?1 AND live=1)",
            [id],
            |row| row.get(0),
        )?;
        if projected_live == truthy(record.get("deletedAt")) {
            liveness_changed.insert(id.to_owned());
            room_agents.insert(id.to_owned());
            if let Some(root) = record.get("rootId").and_then(Value::as_str) {
                changed_roots.insert(root.to_owned());
            }
        }
    }
    if !liveness_changed.is_empty() {
        maintain_peer_team_entities(
            transaction,
            &live_agent_records(transaction)?,
            &projection_records(transaction, "runtime_projects")?,
            validate,
        )?;
    }
    let mut owned_work = Vec::new();
    if table_exists(transaction, "runtime_work")? {
        // The runtime_work_owner_status index serves this owner lookup.
        let mut statement = transaction.prepare_cached(
            "SELECT id,record FROM runtime_work WHERE json_extract(record,'$.owner')=?1 ORDER BY rowid",
        )?;
        for owner in &agent_ids {
            for row in statement.query_map([owner], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (id, raw) = row?;
                if let Some(record) = checked_owned_work(&id, &raw)? {
                    owned_work.push(record);
                }
            }
        }
    }
    publish_agents(
        transaction,
        &agents,
        &owned_work,
        project_server_id,
        validate,
    )?;
    let published = agents
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    publish_team_roots(transaction, &changes.team_roots, &published)?;
    if !room_agents.is_empty() {
        publish_rooms_of(transaction, &room_agents, &changed_roots, validate)?;
    }
    if !liveness_changed.is_empty() {
        maintain_task_window(transaction, validate)?;
        maintain_monitor_window(transaction, validate)?;
    }
    // runtime_events triggers keep the event window current on every write.
    Ok(())
}

/// Publish bounded receipt, plan and annotation DTOs at their source commit.
/// Receipt materialization has the same owner as the explicit recovery read.
pub(crate) fn publish_summary_changes(
    transaction: &Transaction<'_>,
    pending: &studio_runtime_store::PendingEntities,
) -> StoreResult<()> {
    for (id, raw) in records_by_id(transaction, "runtime_tasks", &pending.tasks)? {
        let task: studio_runtime_store::TaskRecord = studio_json::from_str(&raw)?;
        if task.id().as_ref() != id {
            return Err(StoreError::InvalidStoredObject {
                table: "runtime_tasks",
                id,
            });
        }
        let prior = task_window_prior(transaction, &id)?;
        maintain_task_window_after_write(transaction, prior.as_ref(), &task, &|_, _, _, _| true)?;
    }
    let mut events = pending.receipt_events.clone();
    for id in &pending.receipt_items {
        let Some(item) = Table::<TranscriptItemRecord>::new(transaction)
            .get(&TranscriptItemId::from(id.as_str()))?
        else {
            continue;
        };
        let agent: String =
            transaction.query_row("SELECT agent FROM runtime_items WHERE id=?1", [id], |row| {
                row.get(0)
            })?;
        if let Some(event_id) = id.strip_prefix(&format!("{agent}:")) {
            events.insert(event_id.to_owned());
        }
        for input in item.inputs().value().into_iter().flatten() {
            if input.kind() == Some("user")
                && let Some(id) = input.id()
            {
                events.insert(id.to_owned());
            }
        }
        let mut statement = transaction.prepare_cached(
            "SELECT id FROM runtime_event_meta WHERE json_extract(record,'$.transcriptItemId')=?1",
        )?;
        for event in statement.query_map([id], |row| row.get::<_, String>(0))? {
            events.insert(event?);
        }
    }
    if events.is_empty() {
        return Ok(());
    }
    for table in ["runtime_agents", "runtime_event_meta", "runtime_items"] {
        if !table_exists(transaction, table)? {
            return Ok(());
        }
    }
    for id in events {
        studio_runtime_store::durable_seq::mark(
            transaction,
            studio_runtime_store::durable_seq::RECEIPT_VIEW_MARK,
            &id,
        )?;
    }
    Ok(())
}

/// The `messageReceipt` document of user event `id` at read time; None when the event
/// is not a user message of a live agent.
pub(crate) fn receipt_view(connection: &Connection, id: &str) -> StoreResult<Option<RawValue>> {
    for table in [
        "runtime_agents",
        "runtime_events",
        "runtime_event_meta",
        "runtime_items",
    ] {
        if !table_exists(connection, table)? {
            return Ok(None);
        }
    }
    let agent = connection
        .prepare_cached(
            "SELECT e.agent FROM runtime_events e JOIN runtime_agents a ON a.id=e.agent \
             WHERE e.id=?1 AND e.kind='user' AND a.deleted_at IS NULL",
        )?
        .query_row([id], |row| row.get::<_, String>(0))
        .optional()?;
    let Some(agent) = agent else {
        return Ok(None);
    };
    let response = crate::coordination_reads::delivery_receipts(
        connection,
        &agent,
        std::slice::from_ref(&id.to_owned()),
    )
    .map_err(|_| StoreError::RecordContract("message receipt projection failed"))?;
    #[derive(serde::Serialize)]
    struct ReceiptProjection {
        agent: String,
        #[serde(flatten)]
        receipt: crate::coordination_reads::MessageReceipt,
    }
    let Some(mut receipt) = response.items.into_iter().next() else {
        return Ok(None);
    };
    receipt.id = crate::sync_views::bound_sync_text(&receipt.id, "id");
    receipt.error = receipt
        .error
        .map(|error| crate::sync_views::bound_sync_text(&error, "error"));
    Ok(Some(studio_json::to_raw_value(&ReceiptProjection {
        agent: crate::sync_views::bound_sync_text(&agent, "agent"),
        receipt,
    })?))
}

/// Seed pre-existing rows once after the summary contract is installed. Normal
/// pulls and unrelated writes never reconstruct these collections.
fn seed_summary_entities(transaction: &Transaction<'_>) -> StoreResult<()> {
    if transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [SUMMARY_ENTITIES_SEED_MARKER],
        |r| r.get::<_, bool>(0),
    )? {
        return Ok(());
    }
    let mut pending = studio_runtime_store::PendingEntities::default();
    if table_exists(transaction, "runtime_events")? {
        let mut statement =
            transaction.prepare("SELECT id FROM runtime_events WHERE kind='user'")?;
        for id in statement.query_map([], |r| r.get::<_, String>(0))? {
            pending.receipt_events.insert(id?);
        }
    }
    publish_summary_changes(transaction, &pending)?;
    transaction.execute(
        "INSERT INTO sync_entity_meta(key,value) VALUES(?1,'1')",
        [SUMMARY_ENTITIES_SEED_MARKER],
    )?;
    Ok(())
}

/// The writer-local agent record generation, advanced by runtime_agents triggers.
pub(crate) fn agent_record_generation(transaction: &Transaction<'_>) -> StoreResult<i64> {
    Ok(transaction.query_row(
        "SELECT value FROM runtime_agent_record_generation WHERE id=1",
        [],
        |row| row.get(0),
    )?)
}

/// Existing agent rows the trigger journal records as changed after
/// `generation`, as (rowid, id, record).
pub(crate) fn agents_changed_since(
    transaction: &Transaction<'_>,
    generation: i64,
) -> StoreResult<Vec<(i64, String, String)>> {
    let mut statement = transaction.prepare_cached(
        "SELECT a.rowid,a.id,a.record FROM runtime_agents a JOIN runtime_scheduler_agent_changes c \
         ON c.id=a.id WHERE c.generation>?1 ORDER BY a.id",
    )?;
    let rows = statement
        .query_map([generation], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Valid stored rows for `ids`, in rowid order like the full rebuild.
fn records_by_id(
    transaction: &Transaction<'_>,
    table: &str,
    ids: &BTreeSet<String>,
) -> StoreResult<Vec<(String, String)>> {
    Ok(rows_by_id(transaction, table, ids)?
        .into_iter()
        .map(|(_, id, raw)| (id, raw))
        .collect())
}

/// [`records_by_id`] with each row's rowid.
fn rows_by_id(
    transaction: &Transaction<'_>,
    table: &str,
    ids: &BTreeSet<String>,
) -> StoreResult<Vec<(i64, String, String)>> {
    if ids.is_empty() || !table_exists(transaction, table)? {
        return Ok(Vec::new());
    }
    let mut statement =
        transaction.prepare_cached(&format!("SELECT rowid,record FROM {table} WHERE id=?1"))?;
    let mut rows = Vec::new();
    for id in ids {
        if let Some((rowid, raw)) = statement
            .query_row([id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .optional()?
        {
            // The chat and turn rows own some agent fields: check the merged record.
            let raw = if table == agent_volatile::AGENT_TABLE {
                agent_volatile::merged_text(transaction, id, raw)?
            } else {
                raw
            };
            if valid_stored_record(table, id, &raw) {
                rows.push((rowid, id.clone(), raw));
            }
        }
    }
    rows.sort_unstable_by_key(|(rowid, _, _)| *rowid);
    Ok(rows)
}

/// Publish agent views; `work` must hold every work item the agents own.
/// Mark the views of the team leads in `roots` whose team counts changed since their
/// last publication. A lead in `skip` is published by the caller already.
fn publish_team_roots(
    transaction: &Transaction<'_>,
    roots: &BTreeSet<String>,
    skip: &BTreeSet<String>,
) -> StoreResult<()> {
    for root in roots.difference(skip) {
        let current = crate::sync_entity_context::team_counts_text(transaction, root)?;
        let published: Option<Option<String>> = transaction
            .prepare_cached("SELECT team_counts FROM agent_publication_state WHERE id=?1")?
            .query_row([root], |row| row.get(0))
            .optional()?;
        if published.as_ref() != Some(&current) {
            publish_agent_projection(transaction, root, false)?;
        }
    }
    Ok(())
}

fn publish_agents(
    transaction: &Transaction<'_>,
    agents: &[(&str, &Value)],
    _work: &[studio_runtime_store::WorkItemRecord],
    _project_server_id: Option<&str>,
    _validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    for &(id, record) in agents {
        publish_agent_projection(transaction, id, truthy(record.get("deletedAt")))?;
    }
    Ok(())
}

/// The single publication owner of the agent views. The views are built at read time
/// (`entity_scope`); a publication marks them, so readers build them again, and
/// records what it saw for the next publication's comparison.
pub(crate) fn publish_agent_projection(
    transaction: &Transaction<'_>,
    id: &str,
    deleted: bool,
) -> StoreResult<()> {
    crate::account_session_runtime::limits::publish_agent_change(transaction, id, deleted)?;
    studio_runtime_store::durable_seq::mark(
        transaction,
        studio_runtime_store::durable_seq::AGENT_VIEW_MARK,
        id,
    )?;
    let team_counts = crate::sync_entity_context::team_counts_text(transaction, id)?;
    transaction
        .prepare_cached(&format!(
            "INSERT INTO agent_publication_state(id,root_id,is_lead,live,in_flight,team_counts)
           SELECT ?1,json_extract(record,'$.rootId'),json_extract(record,'$.isLead'),
                  json_extract(record,'$.deletedAt') IS NULL AND NOT ?2,
                  {},?3
           FROM runtime_agents WHERE id=?1
         ON CONFLICT(id) DO UPDATE SET root_id=excluded.root_id,is_lead=excluded.is_lead,
           live=excluded.live,in_flight=excluded.in_flight,team_counts=excluded.team_counts",
            studio_runtime_store::domain::chat_busy_sql("runtime_agents.id")
        ))?
        .execute(rusqlite::params![id, deleted, team_counts])?;
    if deleted {
        transaction
            .prepare_cached("UPDATE agent_publication_state SET live=0 WHERE id=?1")?
            .execute([id])?;
    }
    Ok(())
}

/// The documents of one agent at read time: `agentSummary`, `agentDetail`, the legacy
/// `agent` DTO and `retainedWorktree`, each a value or None for a delete. None when the
/// agent row is gone.
pub(crate) struct AgentViewDocuments {
    pub summary: Option<Value>,
    pub detail: Option<Value>,
    pub agent: Option<Value>,
    pub retained_worktree: Option<Value>,
}

pub(crate) fn agent_view_documents(
    connection: &Connection,
    id: &str,
    project_server_id: Option<&str>,
) -> StoreResult<Option<AgentViewDocuments>> {
    let row: Option<(String, Option<String>)> = connection
        .prepare_cached("SELECT record,hold_incident FROM runtime_agents WHERE id=?1")?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let Some((raw, incident)) = row else {
        return Ok(None);
    };
    let raw = agent_volatile::merged_text(connection, id, raw)?;
    let (record, view) = match studio_runtime_store::decode_agent_record(connection, id, &raw) {
        Ok(record) => {
            let record = record.into_value();
            let context = crate::sync_entity_context::agent_view_context(
                connection,
                &record,
                project_server_id,
            )?;
            let view = crate::sync_views::agent_entity_view(&record, &context);
            (record, view)
        }
        // A row that does not decode is held for repair; readers see a minimal safe view.
        Err(error) => {
            let parsed: Value = studio_json::from_str(&raw).unwrap_or_else(|_| json!({"id": id}));
            let diagnostic = incident
                .or_else(|| {
                    parsed
                        .get("error")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| error.to_string());
            let public = crate::sync_views::invalid_agent_view(id, &parsed, &diagnostic);
            let context = crate::sync_views::AgentViewContext {
                project_server_id: project_server_id.map(str::to_owned),
                ..Default::default()
            };
            let view = crate::sync_views::agent_entity_view(&public, &context);
            (parsed, view)
        }
    };
    let retained_worktree = crate::sync_views::retained_worktree_entity(&record);
    let view = view.filter(|_| !truthy(record.get("deletedAt")));
    let Some(view) = view else {
        return Ok(Some(AgentViewDocuments {
            summary: None,
            detail: None,
            agent: None,
            retained_worktree,
        }));
    };
    let summary = project_persisted_record("agentSummary", &view);
    let detail = project_persisted_record("agentDetail", &view);
    let mut agent = view;
    if let Some(fields) = agent.as_object_mut() {
        fields.remove("teamCounts");
        for field in crate::sync_views::AGENT_SUMMARY_ATTENTION_FIELDS {
            fields.remove(field);
        }
    }
    Ok(Some(AgentViewDocuments {
        summary,
        detail,
        agent: Some(agent),
        retained_worktree,
    }))
}

/// The client view of a plan row; None for a row without an id.
pub(crate) fn plan_view(id: &str, raw: &str) -> StoreResult<Option<RawValue>> {
    let record: studio_runtime_store::PlanRecord =
        studio_json::from_str(raw).map_err(|_| StoreError::InvalidStoredRecord {
            object: "plans",
            id: id.to_owned(),
        })?;
    if record.id().is_empty() {
        return Ok(None);
    }
    let view = record.sync_view(
        &|value, field| crate::sync_views::bound_sync_text(value, field),
        crate::sync_views::SYNC_ARRAY_ITEMS,
        &|value, depth| crate::sync_views::bound_opaque_json(value, "", depth),
    );
    Ok(Some(studio_json::to_raw_value(&view)?))
}

/// The client view of an annotation row; None for a row without an id.
pub(crate) fn annotation_view(id: &str, raw: &str) -> StoreResult<Option<RawValue>> {
    let record: studio_runtime_store::AnnotationRecord =
        studio_json::from_str(raw).map_err(|_| StoreError::InvalidStoredRecord {
            object: "annotations",
            id: id.to_owned(),
        })?;
    if record.id().is_empty() {
        return Ok(None);
    }
    let view = record.sync_view(&|value, field| crate::sync_views::bound_sync_text(value, field));
    Ok(Some(studio_json::to_raw_value(&view)?))
}

/// The client view of a complaint row, with the current names of its author and lead.
pub(crate) fn complaint_view(
    connection: &Connection,
    id: &str,
    raw: &str,
) -> StoreResult<Option<RawValue>> {
    let record: studio_runtime_schema::complaint_record::ComplaintRecord =
        studio_json::from_str(raw).map_err(|_| StoreError::InvalidStoredRecord {
            object: "complaints",
            id: id.to_owned(),
        })?;
    let mut agents = Vec::new();
    for agent in [record.author.as_deref(), record.lead_id.as_deref()]
        .into_iter()
        .flatten()
    {
        let raw: Option<String> = connection
            .prepare_cached("SELECT record FROM runtime_agents WHERE id=?1")?
            .query_row([agent], |row| row.get(0))
            .optional()?;
        if let Some(raw) = raw {
            agents.push(studio_json::from_str::<
                crate::submit_gates::AgentAuthorityRow,
            >(&raw)?);
        }
    }
    let context = crate::sync_views::ComplaintViewContext { agents: &agents };
    crate::sync_views::project_complaint_record(&record, &context)
        .map(|view| studio_json::to_raw_value(&view))
        .transpose()
        .map_err(StoreError::from)
}

/// The client document of a window member: a `task` or a `monitor`. None when the row
/// is gone or has no view.
pub(crate) fn window_view(
    connection: &Connection,
    collection: &str,
    id: &str,
) -> StoreResult<Option<(String, RawValue)>> {
    let table = if collection == "task" {
        "runtime_tasks"
    } else {
        "runtime_monitors"
    };
    let raw: Option<String> = connection
        .prepare_cached(&format!("SELECT record FROM {table} WHERE id=?1"))?
        .query_row([id], |row| row.get(0))
        .optional()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    if collection == "task" {
        let record: studio_runtime_store::TaskRecord = studio_json::from_str(&raw)?;
        let agent = record.agent().to_owned();
        return crate::sync_views::project_task_record(&record)
            .map(|view| Ok((agent, studio_json::to_raw_value(&view)?)))
            .transpose();
    }
    let record = decode_monitor(id, &raw)?;
    let view = project_monitor_record(&record)?;
    Ok(Some((record.agent, studio_json::to_raw_value(&view)?)))
}

/// The client view of a rule row: its id, agent and name.
pub(crate) fn rule_view(id: &str, raw: &str) -> StoreResult<RawValue> {
    let invalid = || StoreError::InvalidStoredRecord {
        object: "rules",
        id: id.to_owned(),
    };
    let record: studio_runtime_store::RuleRecord =
        studio_json::from_str(raw).map_err(|_| invalid())?;
    if record.id != id {
        return Err(invalid());
    }
    Ok(studio_json::to_raw_value(
        &json!({"agent": record.agent, "id": id, "name": record.name}),
    )?)
}

/// Seed root agent views and the task and monitor windows once, before the first SSE
/// or runtime write. Work documents need no seed: they are built at read time.
pub(crate) fn seed_work_details(transaction: &Transaction<'_>) -> StoreResult<()> {
    if transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
        [WORK_DETAILS_SEED_MARKER],
        |row| row.get::<_, bool>(0),
    )? {
        return Ok(());
    }
    let work = transaction
        .prepare_cached("SELECT id FROM runtime_work")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    let agents = transaction.prepare_cached("SELECT id FROM runtime_agents WHERE deleted_at IS NULL AND json_extract(record,'$.rootId')=id")?.query_map([], |row| row.get::<_,String>(0))?.collect::<Result<BTreeSet<_>,_>>()?;
    let server = RuntimeStore::transaction_local_server_id(transaction)?;
    publish_entity_changes(
        transaction,
        &EntityChanges {
            agents,
            work,
            ..Default::default()
        },
        server.as_deref(),
        &|_, _, _, _| true,
    )?;
    maintain_task_window(transaction, &|_, _, _, _| true)?;
    maintain_monitor_window(transaction, &|_, _, _, _| true)?;
    transaction.execute("INSERT INTO sync_entity_meta(key,value) VALUES(?1,'1') ON CONFLICT(key) DO UPDATE SET value='1'", [WORK_DETAILS_SEED_MARKER])?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod work_publication_probe {
    use std::{
        collections::BTreeMap,
        sync::{Mutex, OnceLock},
    };
    #[derive(Default, Debug, Clone, Copy)]
    pub(crate) struct Counts {
        pub owned_decodes: usize,
    }
    pub(crate) fn owned_decode(id: &str) {
        counts()
            .lock()
            .expect("probe mutex")
            .entry(id.into())
            .or_default()
            .owned_decodes += 1;
    }
    fn counts() -> &'static Mutex<BTreeMap<String, Counts>> {
        static COUNTS: OnceLock<Mutex<BTreeMap<String, Counts>>> = OnceLock::new();
        COUNTS.get_or_init(Mutex::default)
    }
    pub(crate) fn take(id: &str) -> Counts {
        counts()
            .lock()
            .expect("probe mutex")
            .remove(id)
            .unwrap_or_default()
    }
}

/// Reproject rooms whose view can name an agent whose liveness changed.
fn publish_rooms_of(
    transaction: &Transaction<'_>,
    agents: &BTreeSet<String>,
    roots: &BTreeSet<String>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let directory = crate::sync_entity_context::RoomDirectory::load_live_for_sync(transaction)?;
    for (id, raw) in stored_record_rows(transaction, "runtime_rooms")? {
        use studio_runtime_schema::room_record::{RoomKind, RoomRecord};
        let Ok(room) = studio_json::from_str::<RoomRecord>(&raw) else {
            continue;
        };
        if room.id != id {
            continue;
        }
        let root = room.root_id.as_ref().map(String::as_str);
        let affected = match room.kind {
            RoomKind::Broadcast => root.is_some_and(|root| root == "all" || roots.contains(root)),
            // Federated membership lives in another table; these rooms are few.
            RoomKind::Federated => true,
            RoomKind::Private => room
                .members
                .as_ref()
                .is_some_and(|members| members.iter().any(|member| agents.contains(member))),
        };
        if !affected {
            continue;
        }
        let context = directory.for_room_identity(
            transaction,
            &room.id,
            room.kind == RoomKind::Federated,
            true,
        )?;
        match crate::room_projection::project_room(&room, &context)? {
            Some(view) => {
                SyncDocumentsRepository::put_entity(transaction, "room", &id, view, false)?;
            }
            // Python retains a deleted team's broadcast view.
            None if room.kind == RoomKind::Broadcast && room.project_deleted_at.is_none() => {}
            None => retire_if_live(transaction, "room", &id, validate)?,
        }
    }
    Ok(())
}

/// Seed the initial workspace entity when a sync pull is the first projection
/// operation. Python's `codex_sync_entities.seed` creates this row on first
/// seed; ordinary projection maintenance only refreshes an existing row.
pub fn seed_workspace_projection(
    transaction: &Transaction<'_>,
    state_dir: &str,
    validate_projection: impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='workspace' AND id='current' AND deleted=0)",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        return Ok(());
    }
    put_projection(
        transaction,
        "workspace",
        "current",
        json!({
            "stateDir": state_dir,
            "connected": false,
            "projectOrganizationVersion": 1,
            "peerTeamsVersion": 1,
            "tasksHistoryLimit": TASK_WINDOW_HISTORY_ROWS,
        }),
        false,
        &validate_projection,
    )
}

/// Identity selected by Python's `codex_vm_agents.project_vault` projection.
fn project_vault_source(project: &Value) -> Option<&str> {
    let aliases = project.get("projectAliases").and_then(Value::as_array);
    let logical_id = aliases
        .and_then(|aliases| aliases.first())
        .and_then(|alias| alias.get("projectId"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    logical_id
        .or_else(|| {
            project
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
        })
        .or_else(|| {
            project
                .get("path")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
        })
}

fn put_projection(
    transaction: &Transaction<'_>,
    collection: &str,
    id: &str,
    value: Value,
    deleted: bool,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !validate(collection, id, &value, deleted) {
        return Err(StoreError::InvalidSyncObject {
            scope: collection.to_owned(),
            id: id.to_owned(),
        });
    }
    SyncDocumentsRepository::put_entity(transaction, collection, id, value, deleted)?;
    Ok(())
}

pub(crate) fn table_exists(connection: &Connection, name: &str) -> StoreResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )?)
}

/// Whether `runtime_monitors` has its owner expression index. Its indexed JSON
/// expressions reject malformed rows at write time, so a query bounded by that
/// index needs no per-row `json_valid` guard. Legacy and test schemas without
/// it may hold malformed rows and keep their guarded scans.
pub(crate) fn monitor_owner_index_exists(connection: &Connection) -> StoreResult<bool> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='runtime_monitor_agent_status')",
        )?
        .query_row([], |row| row.get(0))?)
}

pub(crate) fn projection_records(
    transaction: &Connection,
    table: &str,
) -> StoreResult<Vec<(String, String)>> {
    // Legacy behavior: codex_sync_entities._stored_record skips malformed rows
    // while refreshing sync projections instead of failing the whole pull.
    Ok(stored_record_rows(transaction, table)?
        .into_iter()
        .filter(|(id, raw)| valid_stored_record(table, id, raw))
        .collect())
}

/// Stored agent rows without `deletedAt`, in rowid order (the agent columns
/// refuse malformed JSON at write). The `runtime_agent_live_root_v2` partial
/// index serves the predicate, so tombstones are not read.
pub(crate) fn live_agent_records(connection: &Connection) -> StoreResult<Vec<(String, String)>> {
    Ok(
        agent_volatile::merged_rows(connection, live_agent_record_rows(connection)?)?
            .into_iter()
            .filter(|(id, raw)| valid_stored_record("runtime_agents", id, raw))
            .collect(),
    )
}

pub(crate) fn live_agent_record_rows(
    connection: &Connection,
) -> StoreResult<Vec<(String, String)>> {
    if !table_exists(connection, "runtime_agents")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare_cached(
        "SELECT id,record FROM runtime_agents INDEXED BY runtime_agent_live_root_v2 \
         WHERE deleted_at IS NULL ORDER BY rowid",
    )?;
    Ok(statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

/// Every stored row of `table`, refusing the first invalid one.
#[cfg(test)]
pub(crate) fn stored_records(
    transaction: &Transaction<'_>,
    table: &str,
) -> StoreResult<Vec<(String, String)>> {
    let rows = stored_record_rows(transaction, table)?;
    for (id, raw) in &rows {
        if !valid_stored_record(table, id, raw) {
            return Err(StoreError::InvalidSyncObject {
                scope: table.to_owned(),
                id: id.clone(),
            });
        }
    }
    Ok(rows)
}

pub(crate) fn stored_record_rows(
    connection: &Connection,
    table: &str,
) -> StoreResult<Vec<(String, String)>> {
    if !table_exists(connection, table)? {
        return Ok(Vec::new());
    }
    let mut statement =
        connection.prepare(&format!("SELECT id,record FROM {table} ORDER BY rowid"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if table == agent_volatile::AGENT_TABLE {
        return agent_volatile::merged_rows(connection, rows);
    }
    Ok(rows)
}

fn valid_stored_record(table: &str, id: &str, raw: &str) -> bool {
    studio_json::from_str::<Value>(raw).is_ok_and(|record| valid_stored_value(table, id, &record))
}

fn valid_stored_value(table: &str, id: &str, record: &Value) -> bool {
    record.is_object()
        && record.get("id").and_then(Value::as_str) == Some(id)
        && studio_runtime_schema::table_schema(table)
            .is_none_or(|schema| studio_runtime_schema::validate(schema, record).unwrap_or(false))
}

fn checked_owned_work(
    id: &str,
    raw: &str,
) -> StoreResult<Option<studio_runtime_store::WorkItemRecord>> {
    #[cfg(test)]
    work_publication_probe::owned_decode(id);
    let Ok(record) = studio_json::from_str::<studio_runtime_store::WorkItemRecord>(raw) else {
        return Ok(None);
    };
    Ok((record.id().as_ref() == id).then_some(record))
}

fn retire_if_live(
    transaction: &Transaction<'_>,
    collection: &str,
    id: &str,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let live: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection=?1 AND id=?2 AND deleted=0)",
        [collection, id],
        |row| row.get(0),
    )?;
    if live {
        put_projection(transaction, collection, id, json!({}), true, validate)?;
    }
    Ok(())
}

/// What the task window holds of a member: owner, created time and running state, so
/// a write can compare with it. The documents themselves are built at read time.
fn join_task_window(
    transaction: &Transaction<'_>,
    task: &studio_runtime_store::TaskRecord,
) -> StoreResult<()> {
    let id = task.id().as_ref();
    studio_runtime_store::durable_seq::join_window(
        transaction,
        studio_runtime_store::durable_seq::TASK_WINDOW,
        id,
    )?;
    // The state of a task that left is kept, so a return under another owner is seen.
    let previous: Option<String> = transaction
        .prepare_cached("SELECT agent FROM task_window_state WHERE id=?1")?
        .query_row([id], |row| row.get(0))
        .optional()?;
    if let Some(previous) = previous.filter(|previous| previous != task.agent()) {
        // The task moved: the scoped readers of both owners start from a snapshot.
        for owner in [previous.as_str(), task.agent()] {
            SyncDocumentsRepository::move_scope_visibility(transaction, "task", owner)?;
        }
    }
    transaction
        .prepare_cached(
            "INSERT INTO task_window_state(id,agent,created,running) VALUES(?1,?2,?3,?4)
             ON CONFLICT(id) DO UPDATE SET agent=excluded.agent,created=excluded.created,
               running=excluded.running",
        )?
        .execute(rusqlite::params![
            task.id().as_ref(),
            task.agent(),
            task.created_number().number().as_f64(),
            task.status() == studio_runtime_schema::task_record::TaskStatus::Running
        ])?;
    Ok(())
}

fn leave_task_window(transaction: &Transaction<'_>, id: &str) -> StoreResult<()> {
    studio_runtime_store::durable_seq::leave_window(
        transaction,
        studio_runtime_store::durable_seq::TASK_WINDOW,
        id,
    )?;
    Ok(())
}

/// The window state of task `id` as the fields a view had: `status` (running or not),
/// `agent` and `created`. None when the task is not in the window.
fn task_window_prior(transaction: &Transaction<'_>, id: &str) -> StoreResult<Option<Value>> {
    let row: Option<(String, Option<f64>, bool)> = transaction
        .prepare_cached(
            "SELECT s.agent,s.created,s.running FROM task_window_state s
             JOIN durable_rows w ON w.tbl='task_window' AND w.id=s.id AND w.deleted=0
             WHERE s.id=?1",
        )?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .optional()?;
    Ok(row.map(|(agent, created, running)| {
        json!({"agent":agent,"created":created,"status":if running {"running"} else {"finished"}})
    }))
}

pub(crate) fn maintain_task_window(
    transaction: &Transaction<'_>,
    _validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !table_exists(transaction, "runtime_tasks")? || !table_exists(transaction, "runtime_agents")?
    {
        return Ok(());
    }
    // The live-agent set is a subquery, so the runtime_task_status and
    // runtime_task_history indexes bound both parts and the agent records are
    // read once instead of once per task row.
    let mut rows = transaction.prepare(&format!(
        "SELECT t.id,t.record FROM runtime_tasks t \
         WHERE json_extract(t.record,'$.status')='running' AND {LIVE_TASK_AGENT} \
         UNION ALL SELECT id,record FROM (SELECT t.id,t.record FROM runtime_tasks t \
         WHERE json_extract(t.record,'$.status')!='running' AND {LIVE_TASK_AGENT} \
         ORDER BY json_extract(t.record,'$.created') DESC LIMIT {TASK_WINDOW_HISTORY_ROWS})"
    ))?;
    let eligible = rows
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(rows);
    let mut kept = BTreeSet::new();
    for (id, raw) in eligible {
        let record: studio_runtime_store::TaskRecord = studio_json::from_str(&raw)?;
        if record.id().as_ref() != id {
            return Err(StoreError::InvalidStoredObject {
                table: "runtime_tasks",
                id,
            });
        }
        if crate::sync_views::project_task_record(&record).is_some() {
            kept.insert(id.clone());
            join_task_window(transaction, &record)?;
        }
    }
    let candidates = studio_runtime_store::durable_seq::window_members(
        transaction,
        studio_runtime_store::durable_seq::TASK_WINDOW,
    )?;
    let stale = candidates
        .into_iter()
        .filter(|id| !kept.contains(id))
        .collect::<Vec<_>>();
    for id in stale.iter().take(TASK_RETIRE_BATCH) {
        leave_task_window(transaction, id)?;
    }
    if stale.is_empty() {
        let migrated = transaction
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key='task_window_migrated'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if migrated.as_deref() != Some("1") {
            transaction.execute(
                "INSERT INTO sync_entity_meta(key,value) VALUES('task_window_migrated','1') \
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [],
            )?;
        }
    }
    Ok(())
}

/// Apply one task write to the window without rescanning history. A running
/// task is always in the window, so it only projects itself. A finished task
/// joins the newest-first history, which then evicts at most one row (the
/// 101st projected one, possibly the task itself). Writes that shrink the
/// history (a finished task running again) or touch a deleted agent's task
/// rebuild the canonical window, which is also the test oracle.
pub(crate) fn maintain_task_window_after_write(
    transaction: &Transaction<'_>,
    previous: Option<&Value>,
    task: &studio_runtime_store::TaskRecord,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let migration_complete: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entity_meta \
         WHERE key='task_window_migrated' AND value='1')",
        [],
        |row| row.get(0),
    )?;
    let running = |value: &Value| value.get("status").and_then(Value::as_str) == Some("running");
    let task_running = task.status() == studio_runtime_schema::task_record::TaskStatus::Running;
    let task_id = task.id().as_ref();
    let agent_id = task.agent();
    let previous_finished = previous.is_some_and(|previous| !running(previous));
    if migration_complete
        && !(task_running && previous_finished)
        && table_exists(transaction, "runtime_agents")?
        && let Some(projected) = crate::sync_views::project_task_record(task)
        && transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_agents WHERE id=?1 \
             AND deleted_at IS NULL)",
            [agent_id],
            |row| row.get::<_, bool>(0),
        )?
    {
        let unmoved_finished = previous_finished
            && !task_running
            && previous.is_some_and(|previous| {
                previous.get("agent").and_then(Value::as_str) == Some(agent_id)
                    && previous.get("created").and_then(Value::as_f64)
                        == task.created_number().number().as_f64()
            });
        if unmoved_finished
            && !studio_runtime_store::durable_seq::in_window(
                transaction,
                studio_runtime_store::durable_seq::TASK_WINDOW,
                task_id,
            )?
        {
            return Ok(());
        }
        let _ = projected;
        join_task_window(transaction, task)?;
        if !task_running {
            let evicted: Option<String> = transaction
                .query_row(
                    "SELECT s.id FROM task_window_state s \
                     JOIN durable_rows w ON w.tbl='task_window' AND w.id=s.id AND w.deleted=0 \
                     WHERE NOT s.running ORDER BY s.created DESC LIMIT 1 OFFSET ?1",
                    [TASK_WINDOW_HISTORY_ROWS as i64],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = evicted {
                leave_task_window(transaction, &id)?;
            }
        }
        return Ok(());
    }
    maintain_task_window(transaction, validate)
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorProjection {
    display: studio_runtime_schema::process_controls::BackgroundTaskDisplay,
    pub agent: String,
    bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancel_requested: Option<bool>,
    pub command: String,
    created: f64,
    cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished: Option<f64>,
    pub id: String,
    interactive: bool,
    log: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_wake_budget: Option<MonitorWakeBudgetProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub status: studio_runtime_schema::studio_errors::MonitorStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdin_close_requested: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdin_closed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdin_error: Option<String>,
    tail: String,
    #[serde(rename = "timeout_ms")]
    timeout_ms: i64,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorWakeBudgetProjection {
    paused: bool,
    tokens: f64,
    total_suppressed_matches: u64,
}

pub fn project_monitor_record(
    record: &studio_runtime_store::MonitorRecord,
) -> StoreResult<MonitorProjection> {
    if record.id.is_empty() || crate::monitor_matches::validate(record).is_err() {
        return Err(StoreError::InvalidSyncObject {
            scope: "monitor".into(),
            id: record.id.clone(),
        });
    }
    let bound = |text: &str, field: &str| crate::sync_views::bound_sync_text(text, field);
    Ok(MonitorProjection {
        display: crate::process_controls::monitor_record_display(record),
        agent: bound(&record.agent, "agent"),
        bytes: record.bytes.get(),
        cancel_requested: record.cancel_requested,
        command: bound(record.command.as_str(), "command"),
        created: record.created.seconds(),
        cwd: bound(&record.cwd, "cwd"),
        error: record.error.as_ref().map(|v| bound(v.as_str(), "error")),
        exit_code: record.exit_code,
        finished: record.finished.as_ref().map(|v| v.seconds()),
        id: bound(&record.id, "id"),
        interactive: record.interactive,
        log: bound(&record.log, "log"),
        output_patterns: record
            .output_patterns
            .as_ref()
            .filter(|patterns| !patterns.is_empty())
            .cloned(),
        output_truncated: record.output_truncated,
        output_wake_budget: record.output_wake_budget.as_ref().map(|v| {
            MonitorWakeBudgetProjection {
                paused: v.paused,
                tokens: v.tokens,
                total_suppressed_matches: v.total_suppressed_matches,
            }
        }),
        rule_id: record.rule_id.as_ref().map(|v| bound(v, "ruleId")),
        status: record.status,
        stdin_close_requested: record
            .stdin_close_requested
            .as_ref()
            .map(|v| bound(v, "stdinCloseRequested")),
        stdin_closed: record.stdin_closed,
        stdin_error: record.stdin_error.as_ref().map(|v| bound(v, "stdinError")),
        tail: bound(record.tail.as_str(), "tail"),
        timeout_ms: record.timeout_ms.get(),
    })
}
/// Put a checked monitor in the client window; its document is built at read time.
fn publish_typed_monitor(
    tx: &Transaction<'_>,
    record: &studio_runtime_store::MonitorRecord,
) -> StoreResult<()> {
    project_monitor_record(record)?;
    studio_runtime_store::durable_seq::join_window(
        tx,
        studio_runtime_store::durable_seq::MONITOR_WINDOW,
        &record.id,
    )?;
    Ok(())
}

/// Checked content updates retain membership and publish only their own row.
/// The canonical selector stays private so runtime writers cannot rebuild it
/// without comparing checked membership fields. Owner liveness and cold sync
/// maintenance remain in this module.
pub(crate) fn publish_monitor_change(
    transaction: &Transaction<'_>,
    previous: Option<&crate::monitor_policy::MonitorSnapshot>,
    monitor: &crate::monitor_policy::MonitorSnapshot,
) -> StoreResult<()> {
    let validate = &|_: &str, _: &str, view: &Value, deleted| {
        deleted || studio_runtime_schema::validate("MonitorEntityDto", view).unwrap_or(false)
    };
    if previous.is_none_or(|previous| !previous.same_window_position(monitor)) {
        return maintain_monitor_window(transaction, validate);
    }
    let retained = studio_runtime_store::durable_seq::in_window(
        transaction,
        studio_runtime_store::durable_seq::MONITOR_WINDOW,
        monitor.id(),
    )?;
    if !monitor.live_window_owner() {
        if retained {
            studio_runtime_store::durable_seq::leave_window(
                transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                monitor.id(),
            )?;
        }
    } else if monitor.active() || retained {
        publish_typed_monitor(transaction, monitor.record())?;
    }
    Ok(())
}

fn maintain_monitor_window(
    transaction: &Transaction<'_>,
    _validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !table_exists(transaction, "runtime_monitors")? {
        return Ok(());
    }
    let mut kept = BTreeMap::<String, studio_runtime_store::MonitorRecord>::new();
    let has_status_index = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='runtime_monitor_status')",
        [], |row| row.get::<_, bool>(0),
    )?;
    if has_status_index {
        let placeholders = (1..=MONITOR_ACTIVE_STATUSES.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = transaction.prepare(&format!("SELECT id,record FROM runtime_monitors WHERE json_extract(record,'$.status') IN ({placeholders})"))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(MONITOR_ACTIVE_STATUSES), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (id, raw) in rows {
            let record = decode_monitor(&id, &raw)?;
            kept.insert(id, record);
        }
        let mut terminals = Vec::new();
        for status in MONITOR_TERMINAL_STATUSES {
            let mut statement = transaction.prepare("SELECT id,record FROM runtime_monitors WHERE json_extract(record,'$.status')=?1 ORDER BY json_extract(record,'$.created') DESC LIMIT ?2")?;
            let rows = statement
                .query_map(
                    rusqlite::params![status, MONITOR_TERMINAL_WINDOW as i64],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for (id, raw) in rows {
                terminals.push(decode_monitor(&id, &raw)?);
            }
        }
        terminals.sort_by(compare_created_desc);
        for record in terminals.into_iter().take(MONITOR_FINAL_WINDOW) {
            kept.insert(record.id.clone(), record);
        }
    } else {
        let mut statement =
            transaction.prepare("SELECT id,record FROM runtime_monitors ORDER BY rowid")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let records = rows
            .into_iter()
            .map(|(id, raw)| decode_monitor(&id, &raw))
            .collect::<StoreResult<Vec<_>>>()?;
        for status in MONITOR_ACTIVE_STATUSES {
            for record in records
                .iter()
                .filter(|record| record.status.as_str() == status)
            {
                kept.insert(record.id.clone(), record.clone());
            }
        }
        let mut terminals = Vec::new();
        for status in MONITOR_TERMINAL_STATUSES {
            let mut status_rows = records
                .iter()
                .filter(|record| record.status.as_str() == status)
                .cloned()
                .collect::<Vec<_>>();
            status_rows.sort_by(compare_created_desc);
            terminals.extend(status_rows.into_iter().take(MONITOR_TERMINAL_WINDOW));
        }
        terminals.sort_by(compare_created_desc);
        for record in terminals.into_iter().take(MONITOR_FINAL_WINDOW) {
            kept.insert(record.id.clone(), record);
        }
    }

    let owner_ids = kept
        .values()
        .map(|record| record.agent.clone())
        .collect::<BTreeSet<_>>();
    let mut agents = BTreeMap::new();
    let mut invalid_agent_ids = BTreeSet::new();
    for chunk in owner_ids.iter().collect::<Vec<_>>().chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let placeholders = (1..=chunk.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut statement = transaction.prepare(&format!(
            "SELECT id,record FROM runtime_agents WHERE id IN ({placeholders})"
        ))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(chunk.iter().copied()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (id, raw) in rows {
            match studio_json::from_str::<Value>(&raw) {
                Ok(record) if record.is_object() => {
                    agents.insert(id, record.get("deletedAt").is_none_or(Value::is_null));
                }
                _ => {
                    eprintln!(
                        "monitor projection recovery skipped owner row id={id}: invalid record"
                    );
                    invalid_agent_ids.insert(id);
                }
            }
        }
    }
    let mut kept_ids = BTreeSet::new();
    for (id, record) in kept {
        let owner = Some(record.agent.as_str());
        if owner.is_some_and(|agent| invalid_agent_ids.contains(agent)) {
            eprintln!("monitor projection recovery skipped row id={id}: invalid owner record");
            kept_ids.insert(id);
            continue;
        }
        let active_owner = owner
            .and_then(|agent| agents.get(agent))
            .copied()
            .unwrap_or(false);
        if !active_owner {
            continue;
        }
        kept_ids.insert(id);
        publish_typed_monitor(transaction, &record)?;
    }
    for id in studio_runtime_store::durable_seq::window_members(
        transaction,
        studio_runtime_store::durable_seq::MONITOR_WINDOW,
    )? {
        if !kept_ids.contains(&id) {
            studio_runtime_store::durable_seq::leave_window(
                transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                &id,
            )?;
        }
    }
    Ok(())
}

/// The client view of one event: its bounded id, owner, kind, status, created time
/// and error. None when the row is gone or has no id.
pub(crate) fn event_view(connection: &Connection, id: &str) -> StoreResult<Option<Value>> {
    let bound = |value: Option<String>, field| {
        value.map(|text| crate::sync_views::bound_sync_text(&text, field))
    };
    /// Owner, kind, status, created time and error of an event row.
    type EventRow = (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<String>,
    );
    let row: Option<EventRow> = connection
        .prepare_cached("SELECT agent,kind,status,created,error FROM runtime_events WHERE id=?1")?
        .query_row([id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .optional()?;
    Ok(row
        .filter(|_| !id.is_empty())
        .map(|(agent, kind, status, created, error)| {
            json!({"id":crate::sync_views::bound_sync_text(id, "id"),"agent":bound(agent, "agent"),
            "kind":bound(kind, "kind"),"status":bound(status, "status"),"created":created,
            "error":bound(error, "error")})
        }))
}

/// The ids of the events ranked `offset+1..=offset+limit` by the event window order
/// (newest first); the window is the first [`EVENT_WINDOW_ROWS`].
pub(crate) fn event_window_ids(
    connection: &Connection,
    offset: usize,
    limit: usize,
) -> StoreResult<Vec<String>> {
    if !table_exists(connection, "runtime_events")? {
        return Ok(Vec::new());
    }
    Ok(connection
        .prepare_cached(
            "SELECT id FROM runtime_events ORDER BY created DESC,id LIMIT ?1 OFFSET ?2",
        )?
        .query_map([limit as i64, offset as i64], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

pub(crate) fn decode_monitor(
    id: &str,
    raw: &str,
) -> StoreResult<studio_runtime_store::MonitorRecord> {
    let invalid = || StoreError::InvalidStoredRecord {
        object: "monitors",
        id: id.to_owned(),
    };
    let record: studio_runtime_store::MonitorRecord =
        studio_json::from_str(raw).map_err(|_| invalid())?;
    if record.id != id {
        return Err(invalid());
    }
    Ok(record)
}
fn compare_created_desc(
    left: &studio_runtime_store::MonitorRecord,
    right: &studio_runtime_store::MonitorRecord,
) -> std::cmp::Ordering {
    right.created.seconds().total_cmp(&left.created.seconds())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PeerTeamProjection {
    id: String,
    name: String,
    project_path: String,
    members: Vec<String>,
    revision: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    peer_leads: Vec<studio_runtime_schema::project_record::PeerLead>,
}

/// Republish every peer team entity from the project records and the project grants.
/// A writer of grants calls it in its transaction, so the peer leads show or hide at once.
pub(crate) fn publish_peer_teams(transaction: &Transaction<'_>) -> StoreResult<()> {
    maintain_peer_team_entities(
        transaction,
        &live_agent_records(transaction)?,
        &projection_records(transaction, "runtime_projects")?,
        &|_, _, _, _| true,
    )
}

fn maintain_peer_team_entities(
    transaction: &Transaction<'_>,
    agent_records: &[(String, String)],
    project_records: &[(String, String)],
    _validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !table_exists(transaction, "runtime_projects")?
        || !table_exists(transaction, "runtime_agents")?
    {
        return Ok(());
    }
    // An undecodable project keeps its last published state until it is repaired.
    if project_records.iter().any(|(id, raw)| {
        !valid_stored_record("runtime_projects", id, raw)
            || studio_json::from_str::<studio_runtime_schema::project_record::ProjectRecord>(raw)
                .is_err()
    }) {
        return Ok(());
    }
    // The documents are built at read time (`peer_team_document`); the window holds the
    // visible teams and gives a changed team a new number.
    let current = peer_team_views(transaction, agent_records, project_records)?;
    for (id, view) in &current {
        let hash = studio_runtime_store::sha256_hex(&studio_json::to_vec(view)?);
        studio_runtime_store::durable_seq::set_window_view(
            transaction,
            PEER_TEAM_WINDOW,
            id,
            Some(&hash),
        )?;
    }
    for id in studio_runtime_store::durable_seq::window_members(transaction, PEER_TEAM_WINDOW)? {
        if !current.contains_key(&id) {
            studio_runtime_store::durable_seq::set_window_view(
                transaction,
                PEER_TEAM_WINDOW,
                &id,
                None,
            )?;
        }
    }
    Ok(())
}

/// The `durable_rows` window of the visible peer teams.
pub(crate) const PEER_TEAM_WINDOW: &str = studio_runtime_store::durable_seq::PEER_TEAM_WINDOW;

/// The view of every visible peer team, by id: a team of a live project with at least
/// two live lead members in the project folder. Owner of the peer team view rule.
fn peer_team_views(
    transaction: &Connection,
    agent_records: &[(String, String)],
    project_records: &[(String, String)],
) -> StoreResult<BTreeMap<String, PeerTeamProjection>> {
    let agents = agent_records
        .iter()
        .filter_map(|(_, raw)| studio_json::from_str::<Value>(raw).ok())
        .filter_map(|agent| {
            let id = agent.get("id")?.as_str()?.to_owned();
            let live = agent.get("deletedAt").is_none_or(Value::is_null);
            let lead = agent.get("isLead").and_then(Value::as_bool) == Some(true)
                && agent.get("parentId").is_none_or(Value::is_null)
                && agent.get("rootId").and_then(Value::as_str) == Some(id.as_str());
            (live && lead).then_some((id, agent))
        })
        .collect::<BTreeMap<_, _>>();
    let mut current = BTreeMap::<String, PeerTeamProjection>::new();
    for (_, raw) in project_records {
        let Ok(studio_runtime_schema::project_record::ProjectRecord::Live(project)) =
            studio_json::from_str(raw)
        else {
            continue;
        };
        // A project hidden from the sidebar shows no teams.
        if project.sidebar_hidden.value() == Some(&true) {
            continue;
        }
        let Some(project_path) = (!project.path.trim().is_empty())
            .then(|| studio_runtime_store::path_identity::lexical_text(&project.path))
            .flatten()
        else {
            continue;
        };
        let peer_leads = crate::project_projection::peer_leads_of(transaction, &project.id)?;
        let revision = match project.peer_teams_revision {
            studio_runtime_schema::queue_command::Field::Missing => Some(0),
            studio_runtime_schema::queue_command::Field::Null => None,
            studio_runtime_schema::queue_command::Field::Value(value) => Some(value),
        };
        for team in project.peer_teams.value().into_iter().flatten() {
            let mut selected = Vec::new();
            for member in &team.members {
                if agents.get(member.as_str()).is_some_and(|agent| {
                    agent
                        .get("cwd")
                        .and_then(Value::as_str)
                        .and_then(studio_runtime_store::path_identity::lexical_text)
                        .as_deref()
                        == Some(project_path.as_path())
                }) && !selected.iter().any(|prior| prior == member)
                {
                    selected.push(member.clone());
                }
            }
            if selected.len() >= 2 && !team.id.is_empty() {
                let bound = |value: &str, field| crate::sync_views::bound_sync_text(value, field);
                current.insert(
                    team.id.clone(),
                    PeerTeamProjection {
                        id: bound(&team.id, "id"),
                        name: bound(&team.name, "name"),
                        project_path: bound(&project_path.to_string_lossy(), "projectPath"),
                        members: selected
                            .into_iter()
                            .take(crate::sync_views::SYNC_ARRAY_ITEMS)
                            .map(|member| bound(&member, ""))
                            .collect(),
                        revision,
                        peer_leads: peer_leads.clone(),
                    },
                );
            }
        }
    }
    Ok(current)
}

/// The `peerTeam` document of team `id`, built at read time from the projects, the
/// grants and the live lead agents. None when the team is not visible.
pub(crate) fn peer_team_document(
    connection: &Connection,
    id: &str,
) -> StoreResult<Option<RawValue>> {
    if !table_exists(connection, "runtime_projects")?
        || !table_exists(connection, "runtime_agents")?
    {
        return Ok(None);
    }
    let projects = projection_records(connection, "runtime_projects")?;
    let agents = live_agent_records(connection)?;
    peer_team_document_with(connection, &agents, &projects, id)
}

fn peer_team_document_with(
    connection: &Connection,
    agents: &[(String, String)],
    projects: &[(String, String)],
    id: &str,
) -> StoreResult<Option<RawValue>> {
    peer_team_views(connection, agents, projects)?
        .remove(id)
        .map(|view| RawValue::from_string(studio_json::to_string(&view)?))
        .transpose()
        .map_err(Into::into)
}

fn retire_unkept(
    transaction: &Transaction<'_>,
    collection: &str,
    kept: &BTreeSet<String>,
    marker: Option<&str>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let mut statement =
        transaction.prepare("SELECT id FROM sync_entities WHERE collection=?1 AND deleted=0")?;
    let stale = statement
        .query_map([collection], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for id in stale {
        if !kept.contains(&id) {
            put_projection(transaction, collection, &id, json!({}), true, validate)?;
        }
    }
    if let Some(marker) = marker {
        let sequence = transaction.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM sync_entities WHERE collection=?1",
            [collection],
            |row| row.get::<_, i64>(0),
        )?;
        let sequence = sequence.to_string();
        let current = transaction
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key=?1",
                [marker],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if current.as_deref() != Some(sequence.as_str()) {
            transaction.execute(
                "INSERT INTO sync_entity_meta(key,value) VALUES(?1,?2) \
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![marker, sequence],
            )?;
        }
    }
    Ok(())
}

fn maintain_canvas_chat_entities(
    transaction: &Transaction<'_>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !table_exists(transaction, "groups")? {
        return Ok(());
    }
    let mut statement = transaction.prepare("SELECT id FROM groups ORDER BY id")?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut kept = BTreeSet::new();
    for id in ids {
        let name = transaction.query_row("SELECT name FROM groups WHERE id=?1", [&id], |row| {
            row.get::<_, String>(0)
        })?;
        let members = transaction
            .prepare(
                "SELECT source FROM graph_edges WHERE target=?1 AND kind='chat' ORDER BY source",
            )?
            .query_map([&id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let last = transaction
            .query_row(
                "SELECT text,at FROM messages WHERE room=?1 ORDER BY at DESC LIMIT 1",
                [&id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
            )
            .optional()?;
        let count: i64 = transaction.query_row(
            "SELECT count(*) FROM messages WHERE room=?1",
            [&id],
            |row| row.get(0),
        )?;
        let tail = last
            .as_ref()
            .map(|message| message.0.chars().take(2_000).collect::<String>())
            .unwrap_or_default();
        let value = json!({
            "id":id,
            "name":name,
            "members":members,
            "kind":"chat",
            "messageCount":count,
            "tail":tail,
            "lastMessageAt":last.map(|message| message.1),
        });
        put_projection(transaction, "chat", &id, value, false, validate)?;
        kept.insert(id);
    }
    retire_unkept(transaction, "chat", &kept, None, validate)?;

    if !table_exists(transaction, "graph_edges")? {
        return Ok(());
    }
    let mut statement =
        transaction.prepare("SELECT id,source,target,kind FROM graph_edges ORDER BY id")?;
    let edges = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (id, source, target, kind) in edges {
        put_projection(
            transaction,
            "edge",
            &id,
            json!({"id":id,"source":source,"target":target,"kind":kind}),
            false,
            validate,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferenceScope {
    User,
    Server,
}

#[derive(Debug, Error)]
pub enum SyncPolicyError {
    #[error("invalid preference fields")]
    InvalidPreferences,
    #[error("invalid stored sync object {scope}/{id}")]
    InvalidStoredObject { scope: String, id: String },
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub fn effective_limit(scope: &str, limit: i64) -> i64 {
    limit.clamp(
        1,
        if scope == SYNC_ENTITY_SCOPE {
            ENTITY_PAGE_LIMIT
        } else {
            OTHER_PAGE_LIMIT
        },
    )
}

/// Whether a JSON integer is outside serde_json's exact signed/unsigned range.
pub fn contains_unrepresentable_integer(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && !matches!(
                    bytes[index],
                    b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t'
                )
            {
                index += 1;
            }
            let token = &raw[start..index];
            if !token.contains(['.', 'e', 'E'])
                && token.parse::<i64>().is_err()
                && token.parse::<u64>().is_err()
            {
                return true;
            }
            continue;
        }
        index += 1;
    }
    false
}

pub fn parse_preferences(
    value: &Value,
    scope: PreferenceScope,
) -> Result<Map<String, Value>, SyncPolicyError> {
    let object = value
        .as_object()
        .ok_or(SyncPolicyError::InvalidPreferences)?;
    let mut fields = Map::new();
    for (name, field) in object {
        validate_preference(name, field, scope)?;
        fields.insert(name.clone(), field.clone());
    }
    Ok(fields)
}

pub fn merge_preferences(
    current: &Map<String, Value>,
    incoming: &Map<String, Value>,
    now_ms: i64,
) -> Result<Map<String, Value>, SyncPolicyError> {
    let mut fields = current.clone();
    for (key, value) in incoming {
        let mut candidate = value.clone();
        let timestamp = candidate
            .get("timestamp")
            .and_then(Value::as_i64)
            .ok_or(SyncPolicyError::InvalidPreferences)?;
        if timestamp > now_ms.saturating_add(MAX_CLOCK_SKEW_MS) {
            candidate["timestamp"] = json!(now_ms);
        }
        let replace = fields
            .get(key)
            .is_none_or(|prior| preference_rank(&candidate) > preference_rank(prior));
        if replace {
            fields.insert(key.clone(), candidate);
        }
    }
    prune_preferences(&mut fields);
    Ok(fields)
}

fn validate_preference(
    name: &str,
    field: &Value,
    scope: PreferenceScope,
) -> Result<(), SyncPolicyError> {
    if name.chars().count() > 2048 {
        return Err(SyncPolicyError::InvalidPreferences);
    }
    let parts: Vec<String> =
        studio_json::from_str(name).map_err(|_| SyncPolicyError::InvalidPreferences)?;
    if !matches!(parts.len(), 1 | 2) || canonical_json(&json!(parts)) != name {
        return Err(SyncPolicyError::InvalidPreferences);
    }
    let key = &parts[0];
    let value = field.get("value").unwrap_or(&Value::Null);

    let model_choices = key == MODEL_CHOICES_PREFERENCE_KEY;
    let appearance = key == "codex-studio-preferences-v1";
    let visual = key.starts_with("codex-project-tree:")
        || key.starts_with("codex-project-compact:")
        || key.starts_with("codex-progress-hidden:")
        || key.starts_with("studio-prompt-bookmarks:")
        || matches!(
            key.as_str(),
            "codex-worker-disclosures"
                | "studio-logical-project-collapsed"
                | "studio-logical-project-compact"
                | "studio-server-project-compact-v1"
        );
    let shell = key == "server-order"
        || key.starts_with("server-alias:")
        || key.starts_with("server-label:");
    if !appearance && !visual && !shell && !model_choices {
        return Err(SyncPolicyError::InvalidPreferences);
    }
    let expected_user_scope = appearance
        || key.starts_with("server-")
        || key.starts_with("studio-logical-")
        || key == "studio-server-project-compact-v1";
    if expected_user_scope != matches!(scope, PreferenceScope::User) {
        return Err(SyncPolicyError::InvalidPreferences);
    }

    if model_choices {
        if parts.len() != 1
            || !studio_runtime_schema::validate("ModelChoices", value).unwrap_or(false)
        {
            return Err(SyncPolicyError::InvalidPreferences);
        }
    } else if appearance {
        if parts.len() != 2 || !valid_appearance(&parts[1], value) {
            return Err(SyncPolicyError::InvalidPreferences);
        }
    } else if visual {
        if !value.is_null() && !value.is_boolean() {
            return Err(SyncPolicyError::InvalidPreferences);
        }
    } else if key == "server-order" {
        if !value.as_array().is_some_and(|items| {
            items.len() <= 256
                && items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|s| s.chars().count() <= 256))
        }) {
            return Err(SyncPolicyError::InvalidPreferences);
        }
    } else if key.starts_with("server-alias:") {
        let Some(label) = value.as_str() else {
            return Err(SyncPolicyError::InvalidPreferences);
        };
        if label.is_empty()
            || label.len() > 3
            || !label.is_ascii()
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() && byte.is_ascii_uppercase())
        {
            return Err(SyncPolicyError::InvalidPreferences);
        }
    } else if !value.as_str().is_some_and(|label| {
        !crate::event_write::python_strip(label).is_empty() && label.chars().count() <= 128
    }) {
        return Err(SyncPolicyError::InvalidPreferences);
    }
    Ok(())
}

fn valid_appearance(name: &str, value: &Value) -> bool {
    match name {
        "theme" => matches!(value.as_str(), Some("auto" | "light" | "dark")),
        "typography" | "contentLayout" => matches!(value.as_str(), Some("original" | "custom")),
        "fontFamily" => matches!(
            value.as_str(),
            Some("geist" | "system" | "arial" | "inter" | "georgia" | "verdana")
        ),
        "sidebarFontSize" | "mainFontSize" => {
            value.as_i64().is_some_and(|n| (12..=24).contains(&n))
        }
        "contentWidth" => value.as_i64().is_some_and(|n| (60..=100).contains(&n)),
        "sidebarShortcut" => value.as_str().is_some_and(|s| s.chars().count() <= 32),
        "showMessageAvatars" | "checkProviderUpdates" => value.is_boolean(),
        "showTerminals" => studio_runtime_schema::validate(
            "AppearancePreferences",
            &json!({ "showTerminals": value }),
        )
        .unwrap_or(false),
        "hideOldChatsThreshold" => value.as_i64().is_some_and(|n| (1..=50).contains(&n)),
        _ => false,
    }
}

fn preference_rank(field: &Value) -> (i64, String, String) {
    (
        field
            .get("timestamp")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        field
            .get("writer")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        canonical_json(field.get("value").unwrap_or(&Value::Null)),
    )
}

fn prune_preferences(fields: &mut Map<String, Value>) {
    let mut removable = fields
        .iter()
        .filter(|(name, _)| {
            !preference_key(name).is_some_and(|key| {
                key == "codex-studio-preferences-v1"
                    || key.starts_with("server-")
                    || key == MODEL_CHOICES_PREFERENCE_KEY
            })
        })
        .map(|(name, field)| {
            (
                field
                    .get("timestamp")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                name.clone(),
            )
        })
        .collect::<Vec<_>>();
    removable.sort_by(|left, right| right.cmp(left));
    let retained = removable
        .into_iter()
        .take(PREFERENCE_FIELD_CAP)
        .map(|(_, name)| name)
        .collect::<std::collections::BTreeSet<_>>();
    fields.retain(|name, _| {
        preference_key(name).is_some_and(|key| {
            key == "codex-studio-preferences-v1"
                || key.starts_with("server-")
                || key == MODEL_CHOICES_PREFERENCE_KEY
        }) || retained.contains(name)
    });
}

fn preference_key(name: &str) -> Option<String> {
    studio_json::from_str::<Vec<String>>(name)
        .ok()
        .and_then(|parts| parts.into_iter().next())
}

fn canonical_json(value: &Value) -> String {
    studio_json::to_string(value).expect("studio_json::Value is serializable")
}

fn stored_preference_fields(
    payload: Option<String>,
    scope: &str,
    validate_field: &(impl Fn(&Value) -> bool + Send + Sync),
) -> Result<Map<String, Value>, SyncPolicyError> {
    let Some(payload) = payload else {
        return Ok(Map::new());
    };
    let invalid_object = || SyncPolicyError::InvalidStoredObject {
        scope: "uiPreferences".to_owned(),
        id: scope.to_owned(),
    };
    let envelope: Value = studio_json::from_str(&payload).map_err(|_| invalid_object())?;
    if envelope.as_object().is_none_or(|object| {
        object.len() != 3
            || envelope.get("collection").and_then(Value::as_str) != Some("uiPreferences")
            || envelope.get("id").and_then(Value::as_str) != Some(scope)
    }) {
        return Err(invalid_object());
    }
    let value = envelope
        .get("value")
        .and_then(Value::as_object)
        .ok_or_else(invalid_object)?;
    if value.len() != 1 {
        return Err(invalid_object());
    }
    let mut fields = value
        .get("fields")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(invalid_object)?;
    for (name, field) in &fields {
        if !validate_field(field)
            || validate_preference(
                name,
                field,
                if scope == "user" {
                    PreferenceScope::User
                } else {
                    PreferenceScope::Server
                },
            )
            .is_err()
        {
            return Err(invalid_object());
        }
    }
    fields.retain(|name, _| {
        !preference_key(name).is_some_and(|key| key.starts_with("studio-turns:"))
    });
    Ok(fields)
}

pub async fn push_preferences(
    store: &RuntimeStore,
    scope: PreferenceScope,
    incoming: Map<String, Value>,
    validate_field: impl Fn(&Value) -> bool + Send + Sync + 'static,
    on_provider_update_change: impl FnOnce() + Send + 'static,
) -> Result<Map<String, Value>, SyncPolicyError> {
    let scope_name = match scope {
        PreferenceScope::User => "user",
        PreferenceScope::Server => "server",
    };
    let ((merged, _provider_updates_changed), _) = store
        .transaction_with_post_commit(
            move |transaction| {
                let current_payload =
                    SyncDocumentsRepository::preference_payload(transaction, scope_name)?;
                let current =
                    stored_preference_fields(current_payload, scope_name, &validate_field)
                        .map_err(|error| match error {
                            SyncPolicyError::InvalidStoredObject { scope, id } => {
                                StoreError::InvalidSyncObject { scope, id }
                            }
                            other @ SyncPolicyError::InvalidPreferences
                            | other @ SyncPolicyError::Store(_) => {
                                StoreError::SchemaMismatch(other.to_string())
                            }
                        })?;
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |duration| {
                        i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
                    });
                let merged = merge_preferences(&current, &incoming, now_ms)
                    .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
                let provider_updates_changed = scope == PreferenceScope::User
                    && crate::provider_versions::provider_updates_enabled_from_fields(&current)
                        != crate::provider_versions::provider_updates_enabled_from_fields(&merged);
                let payload = canonical_json(&json!({
                    "collection": "uiPreferences",
                    "id": scope_name,
                    "value": {"fields": merged},
                }));
                SyncDocumentsRepository::put_preferences(transaction, scope_name, &payload)?;
                Ok((merged, provider_updates_changed))
            },
            move |(_, provider_updates_changed)| {
                if *provider_updates_changed {
                    on_provider_update_change();
                }
            },
        )
        .await
        .map_err(|error| match error {
            StoreError::InvalidSyncObject { scope, id } => {
                SyncPolicyError::InvalidStoredObject { scope, id }
            }
            other @ StoreError::StateDirectory(_)
            | other @ StoreError::AlreadyOwned
            | other @ StoreError::IntermediateReleaseRequired
            | other @ StoreError::MissingDatabase(_)
            | other @ StoreError::Migration { .. }
            | other @ StoreError::UnsafeStateDirectory
            | other @ StoreError::SchemaMismatch(_)
            | other @ StoreError::InvalidSyncObjectField { .. }
            | other @ StoreError::InvalidStoredRecord { .. }
            | other @ StoreError::SchemaObjectMismatch { .. }
            | other @ StoreError::Sqlite(_)
            | other @ StoreError::CommitOutcomeUnknown(_)
            | other @ StoreError::RequestResult(_)
            | other @ StoreError::Outbox(_)
            | other @ StoreError::Json(_)
            | other @ StoreError::InvalidStoredObject { .. }
            | other @ StoreError::TranscriptFullTextUnavailable { .. }
            | other @ StoreError::TranscriptInputsIncomplete { .. }
            | other @ StoreError::SchemaValidation(_)
            | other @ StoreError::RecordContract(_)
            | other @ StoreError::WriterPoisoned
            | other @ StoreError::WriterActorInitialization(_)
            | other @ StoreError::WriterActorControl(_)
            | other @ StoreError::CommitActorOutcomeUnknown(_)
            | other @ StoreError::ReadTaskFailed => SyncPolicyError::Store(other),
        })?;
    Ok(merged)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn checked_work_uses_typed_shape_and_row_identity() {
        let raw = r#"{"id":"w","status":"ready","owner":"agent"}"#;
        let work = super::checked_owned_work("w", raw).unwrap().unwrap();
        assert_eq!(work.id().as_ref(), "w");
        assert!(super::checked_owned_work("other", raw).unwrap().is_none());
        assert!(
            super::checked_owned_work("w", r#"{"id":"w","status":false}"#)
                .unwrap()
                .is_none()
        );
        assert!(super::checked_owned_work("w", "{").unwrap().is_none());
    }

    #[tokio::test]
    async fn project_maintenance_skips_unmigrated_float_revision() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_projects(id,record) VALUES('bad',?1)",
                    [r#"{"recordKind":"live","id":"bad","path":"/bad","accountRevision":1.0}"#],
                )?;
                tx.execute(
                    "INSERT INTO runtime_projects(id,record) VALUES('good',?1)",
                    [r#"{"recordKind":"live","id":"good","path":"/good"}"#],
                )?;
                Ok(())
            })
            .await
            .unwrap();
        let snapshot = ProjectVaultSnapshot::capture_for_sync(&store, root.path())
            .await
            .unwrap();
        store
            .transaction(move |tx| {
                let rooms = crate::sync_entity_context::RoomDirectory::load(tx)?;
                maintain_entity_projections_with_server_id(
                    tx,
                    "",
                    None,
                    &snapshot,
                    &rooms,
                    |_, _, _, _| true,
                )?;
                // Project documents are built at read time: the row that does not decode has
                // none, the good row has one.
                assert!(crate::entity_scope::direct_document(tx, "project", "good")?.is_some());
                assert!(crate::entity_scope::direct_document(tx, "project", "bad")?.is_none());
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn event_view_is_bounded_and_typed() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store.transaction_with(|tx| {
            let error = "é".repeat(2_100);
            tx.execute("INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch,error) VALUES('event','agent','user','text','pending',1,1,?1)", [&error])?;
            let actual = event_view(tx, "event")?.expect("event view");
            assert_eq!(actual, json!({"id":"event","agent":"agent","kind":"user","status":"pending","created":1.0,"error":"é".repeat(2_000)}));
            assert_eq!(event_window_ids(tx, 0, EVENT_WINDOW_ROWS)?, ["event"]);
            Ok::<_, StoreError>(())
        }).await.unwrap();
    }

    #[tokio::test]
    async fn peer_team_projection_preserves_revision_presence() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store.transaction_with(|tx| {
            let agents = ["a", "b"].map(|id| (id.to_owned(), json!({"id":id,"rootId":id,"isLead":true,"cwd":"/project"}).to_string()));
            for (revision, expected) in [(None, json!(0)), (Some(Value::Null), Value::Null), (Some(json!(3)), json!(3))] {
                let mut project = json!({"recordKind":"live","id":"p","path":"/project","peerTeams":[{"id":"team","name":"Team","members":["a","b"]}]});
                if let Some(revision) = revision { project["peerTeamsRevision"] = revision; }
                maintain_peer_team_entities(tx, &agents, &[("p".into(), project.to_string())], &|_, _, _, _| true)?;
                let payload = super::peer_team_document_with(tx, &agents, &[("p".into(), project.to_string())], "team")?.expect("visible team");
                let value: Value = studio_json::from_str(payload.as_str())?;
                assert_eq!(value["revision"], expected);
                assert_eq!(value["members"], json!(["a", "b"]));
            }
            Ok::<_, StoreError>(())
        }).await.unwrap();
    }

    #[test]
    fn plan_projection_bounds_nested_steps() {
        let raw = json!({"id":"p","rootId":"r","text":"plan","version":1,"updated":null,"steps":[{"nested":{"long":"x".repeat(12_001),"items":vec![1;201]}}]});
        let record: studio_runtime_store::PlanRecord =
            serde::Deserialize::deserialize(&raw).unwrap();
        let view = record.sync_view(
            &|value, field| crate::sync_views::bound_sync_text(value, field),
            crate::sync_views::SYNC_ARRAY_ITEMS,
            &|value, depth| crate::sync_views::bound_opaque_json(value, "", depth),
        );
        let result = view.value();
        assert_eq!(
            result["steps"][0]["nested"]["long"].as_str().unwrap().len(),
            12_000
        );
        assert_eq!(
            result["steps"][0]["nested"]["items"]
                .as_array()
                .unwrap()
                .len(),
            200
        );
    }

    #[test]
    fn typed_monitor_projection_keeps_existing_public_bounds() {
        let raw = format!(
            r#"{{"id":"monitor","agent":"owner","epoch":0,"status":"running","requestId":"fixture","created":0.0,"activityAt":0.0,"activityGeneration":0,"bytes":0,"command":{},"cwd":{},"interactive":false,"livenessCommand":"","log":{},"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":{},"error":{},"timeout_ms":3600000,"wakeOn":"exit","outputPatterns":["ready"]}}"#,
            studio_json::to_string(&"雪".repeat(2100)).unwrap(),
            studio_json::to_string(&std::env::temp_dir()).unwrap(),
            studio_json::to_string(&std::env::temp_dir().join("watch.log")).unwrap(),
            studio_json::to_string(&"é".repeat(2100)).unwrap(),
            studio_json::to_string(&"x".repeat(2100)).unwrap()
        );
        let typed: studio_runtime_store::MonitorRecord = studio_json::from_str(&raw).unwrap();
        let new = super::project_monitor_record(&typed).unwrap();
        assert_eq!(new.command, "雪".repeat(2_000));
        assert_eq!(new.tail, "é".repeat(2_000));
        assert_eq!(new.error.as_deref(), Some("x".repeat(2_000).as_str()));
        assert_eq!(new.output_patterns, Some(vec!["ready".to_owned()]));
    }

    #[test]
    fn changed_agent_publication_rejects_decimal_root_epoch() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL)")
            .unwrap();
        db.execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .unwrap();
        let raw = r#"{"id":"a","rootId":"r","epoch":1.0,"status":"idle"}"#;
        assert!(super::CheckedAgentRow::decode(&db, 1, "a", raw).is_err());
    }

    use super::*;
    use rusqlite::Connection;
    use rusqlite::trace::{TraceEvent, TraceEventCodes};
    use std::cell::RefCell;

    thread_local! {
        static TRACED_SQL: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn trace_sql(event: TraceEvent<'_>) {
        if let TraceEvent::Stmt(statement, _) = event {
            TRACED_SQL.with_borrow_mut(|statements| {
                statements.push(statement.sql().into_owned());
            });
        }
    }

    fn trace_expanded_sql(event: TraceEvent<'_>) {
        if let TraceEvent::Stmt(statement, _) = event {
            TRACED_SQL.with_borrow_mut(|statements| {
                statements.push(statement.expanded_sql().expect("bound statement"));
            });
        }
    }

    /// The window of `collection` as a reader builds it: id to (envelope, left).
    fn window_rows(
        connection: &Connection,
        collection: &str,
    ) -> BTreeMap<String, (Option<String>, bool)> {
        let window = if collection == "task" {
            studio_runtime_store::durable_seq::TASK_WINDOW
        } else {
            studio_runtime_store::durable_seq::MONITOR_WINDOW
        };
        let entries: Vec<(String, bool)> = connection
            .prepare("SELECT id,deleted FROM durable_rows WHERE tbl=?1 ORDER BY id")
            .unwrap()
            .query_map([window], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        entries
            .into_iter()
            .map(|(id, left)| {
                let payload = (!left)
                    .then(|| window_view(connection, collection, &id).ok().flatten())
                    .flatten()
                    .map(|(_, value)| {
                        json!({"collection":collection,"id":id,"value":studio_json::from_str::<Value>(value.as_str()).unwrap()})
                            .to_string()
                    });
                (id, (payload, left))
            })
            .collect()
    }

    fn monitor_entity(connection: &Connection, id: &str) -> StoreResult<Option<Value>> {
        Ok(window_rows(connection, "monitor")
            .remove(id)
            .and_then(|(payload, left)| (!left).then_some(payload).flatten())
            .map(|raw| studio_json::from_str::<Value>(&raw).unwrap()["value"].clone()))
    }

    fn monitor_entities(connection: &Connection) -> StoreResult<Vec<(String, String, bool)>> {
        Ok(window_rows(connection, "monitor")
            .into_iter()
            .map(|(id, (payload, left))| (id, payload.unwrap_or_else(|| "{}".into()), left))
            .collect())
    }

    #[tokio::test]
    async fn monitor_output_projects_exactly_one_row_and_keeps_window_membership() {
        use crate::{monitor_policy, monitor_runtime};
        use studio_runtime_store::{AgentRecord, AgentRepository, monitor_runtime as repository};

        let root = tempfile::tempdir().expect("isolated state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        let cwd = root.path().to_string_lossy().into_owned();
        let id =
            store
                .transaction_with(move |tx| {
                    let actor = AgentRecord::from_value(
                        json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
                            "id":"agent", "rootId":"agent", "epoch":1, "autoWake":true,
                            "status":"idle", "cwd":cwd, "accountKey":"default"
                        }),
                    )
                    .map_err(StoreError::from)?;
                    AgentRepository::put(tx, &actor)?;
                    let mut monitor = monitor_policy::create_record(
                        &actor.value(),
                        &serde::Deserialize::deserialize(&json!({"command":"printf output"})).unwrap(),
                        "output-watch",
                        &cwd,
                        1.0,
                    )?;
                    monitor.status = studio_runtime_schema::studio_errors::MonitorStatus::Running;
                    monitor.operation = Some(studio_json::from_str(r#"{"agent":"agent","epoch":1,"accountKey":"default","connectionId":"connection"}"#).unwrap());
                    let id = monitor.id.clone();
                    repository::put(tx, &monitor)?;
                    // Equal timestamps exercise the existing index tie order. Include
                    // an evicted terminal row so content updates cannot resurrect it.
                    for index in 0..=MONITOR_TERMINAL_WINDOW {
                        let mut terminal = monitor.clone();
                        terminal.id = format!("terminal-{index}");
                        terminal.status = studio_runtime_schema::studio_errors::MonitorStatus::Completed;
                        terminal.exit_code.get_or_insert(0);
                        repository::put(tx, &terminal)?;
                    }
                    maintain_monitor_window(tx, &|_, _, _, _| true)?;
                    let retired: String = tx.query_row(
                    "SELECT id FROM runtime_monitors WHERE id LIKE 'terminal-%' AND id NOT IN \
                     (SELECT id FROM durable_rows WHERE tbl='monitor_window' AND deleted=0)",
                    [],
                    |row| row.get(0),
                ).map_err(StoreError::from)?;
                    studio_runtime_store::durable_seq::leave_window(tx, "monitor_window", &retired).map(|_| true)?;
                    tx.execute(
                        "INSERT INTO runtime_monitors(id,record) VALUES('invalid-monitor','[]')",
                        [],
                    )
                    .map_err(StoreError::from)?;
                    studio_runtime_store::durable_seq::join_window(
                        tx,
                        studio_runtime_store::durable_seq::MONITOR_WINDOW,
                        "invalid-monitor",
                    )?;
                    Ok::<_, monitor_policy::MonitorError>(id)
                })
                .await
                .expect("seed canonical monitor window");

        let output_id = id.clone();
        store
            .transaction_with(move |tx| {
                let (before, actor) = monitor_runtime::snapshot(tx, &output_id)?;
                let decision = monitor_policy::output(
                    &before,
                    before.operation().expect("operation"),
                    b"fragment",
                    2.0,
                )?;
                TRACED_SQL.with_borrow_mut(Vec::clear);
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_expanded_sql));
                monitor_runtime::apply(tx, &before, &actor, &decision, 2.0)?;
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                let statements = TRACED_SQL.with_borrow_mut(std::mem::take);
                // Count attempted window joins, including a member already there.
                let publications = statements
                    .iter()
                    .filter(|sql| {
                        sql.starts_with("INSERT INTO durable_rows")
                            && sql.contains("monitor_window")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(publications.len(), 1, "{publications:?}");
                assert!(publications[0].contains(&output_id));
                assert!(
                    !statements.iter().any(|sql| sql
                        .starts_with("SELECT id,record FROM runtime_monitors")
                        && !sql.contains("json_extract(record,'$.agent')")),
                    "output must not run the window selector: {statements:?}"
                );
                let projected = monitor_entity(tx, &output_id)?.expect("updated projection");
                assert_eq!(projected["tail"], "fragment");
                assert_eq!(projected["bytes"], b"fragment".len());
                // A member that does not decode keeps its place; readers skip it.
                assert!(studio_runtime_store::durable_seq::in_window(
                    tx,
                    studio_runtime_store::durable_seq::MONITOR_WINDOW,
                    "invalid-monitor",
                )?);
                Ok::<_, monitor_policy::MonitorError>(())
            })
            .await
            .expect("one output fragment publishes only its own monitor");

        let rollback_id = id.clone();
        let rollback = store
            .transaction_with(move |tx| {
                let (before, actor) = monitor_runtime::snapshot(tx, &rollback_id)?;
                let decision = monitor_policy::output(
                    &before,
                    before.operation().expect("operation"),
                    b"rolled back",
                    3.0,
                )?;
                monitor_runtime::apply(tx, &before, &actor, &decision, 3.0)?;
                Err::<(), _>(monitor_policy::MonitorError::Refused("rollback witness"))
            })
            .await;
        assert!(matches!(
            rollback,
            Err(monitor_policy::MonitorError::Refused("rollback witness"))
        ));
        let read_id = id.clone();
        store
            .read_async("test/monitor-rollback", move |tx| {
                assert_eq!(
                    monitor_entity(tx, &read_id)?.expect("projection")["tail"],
                    "fragment"
                );
                let raw: String = tx.query_row(
                    "SELECT record FROM runtime_monitors WHERE id=?1",
                    [&read_id],
                    |row| row.get(0),
                )?;
                assert_eq!(studio_json::from_str::<Value>(&raw)?["tail"], "fragment");
                Ok::<_, StoreError>(())
            })
            .await
            .expect("source and projection roll back together");

        // An unretained terminal gets no publication; changing its ordering
        // field must run the canonical selector and admit it instead.
        store
            .transaction_with(move |tx| {
                let excluded: String = tx
                    .query_row(
                        "SELECT id FROM runtime_monitors WHERE id LIKE 'terminal-%' AND id NOT IN \
                 (SELECT id FROM durable_rows WHERE tbl='monitor_window' AND deleted=0)",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(StoreError::from)?;
                let (before, actor) = monitor_runtime::snapshot(tx, &excluded)?;
                let mut decision = before.decision();
                decision.record.tail = "late metadata".to_owned().try_into().expect("tail");
                monitor_runtime::apply(tx, &before, &actor, &decision, 3.0)?;
                assert!(monitor_entity(tx, &excluded)?.is_none());
                let (before, actor) = monitor_runtime::snapshot(tx, &excluded)?;
                let mut decision = before.decision();
                decision.record.created = 4.0.try_into().expect("created");
                monitor_runtime::apply(tx, &before, &actor, &decision, 4.0)?;
                assert!(monitor_entity(tx, &excluded)?.is_some());
                // The full selector remains an oracle for exact tie membership.
                let before = monitor_entities(tx)?;
                maintain_monitor_window(tx, &|_, _, _, _| true)?;
                assert_eq!(before, monitor_entities(tx)?);
                Ok::<_, monitor_policy::MonitorError>(())
            })
            .await
            .expect("terminal membership and ordering");

        store
            .transaction_with(move |tx| {
                let (before, actor) = monitor_runtime::snapshot(tx, &id)?;
                let decision =
                    monitor_policy::finish(&before, before.operation(), Some(0), None, 5.0)?;
                monitor_runtime::apply(tx, &before, &actor, &decision, 5.0)?;
                let after_transition = monitor_entities(tx)?;
                maintain_monitor_window(tx, &|_, _, _, _| true)?;
                assert_eq!(after_transition, monitor_entities(tx)?);
                assert!(monitor_entity(tx, "invalid-monitor")?.is_none());
                Ok::<_, monitor_policy::MonitorError>(())
            })
            .await
            .expect("status transition keeps canonical membership and retires invalid row");
    }

    #[test]
    fn monitor_window_keeps_active_rows_without_projecting_inactive_owners() {
        let mut connection = Connection::open_in_memory().expect("database");
        // Keep a record-only legacy source table so non-object corruption reaches the reader.
        // The current provider column rejects that corruption at write time.
        connection
            .execute_batch(
                "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY, record TEXT NOT NULL, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
                 CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
                 CREATE TABLE sync_entities(
                     collection TEXT NOT NULL,id TEXT NOT NULL,seq INTEGER NOT NULL,
                     hash TEXT NOT NULL,payload TEXT,deleted INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY(collection,id));
                 CREATE TABLE sync_documents(seq INTEGER PRIMARY KEY);
                 CREATE TABLE durable_rows(tbl TEXT,id TEXT,seq INTEGER,deleted INTEGER,at INTEGER,PRIMARY KEY(tbl,id)); CREATE TABLE sync_generation(id INTEGER PRIMARY KEY,value INTEGER NOT NULL); INSERT INTO sync_generation VALUES(1,0);
                 CREATE TABLE sync_entity_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                 INSERT INTO runtime_agents VALUES
                     ('active','{\"id\":\"active\",\"provider\":\"codex\",\"deletedAt\":null}'),
                     ('deleted','{\"id\":\"deleted\",\"provider\":\"codex\",\"deletedAt\":1}'),
                     ('broken','[\"not an agent object\"]');",
            )
            .expect("projection fixture");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        let insert_monitor = |connection: &Connection,
                              id: &str,
                              agent: &str,
                              status: &str,
                              created: i64| {
            connection
                    .execute(
                        "INSERT INTO runtime_monitors(id,record) VALUES(?1,?2)",
                        rusqlite::params![
                            id,
                            json!({"id":id,"agent":agent,"epoch":0,"status":status,"exitCode":(status == "completed").then_some(0),"created":created,"requestId":format!("request-{id}"),"activityAt":created,"activityGeneration":0,"bytes":0,"command":"","cwd":std::env::temp_dir(),"interactive":false,"livenessCommand":"","log":std::env::temp_dir().join("watch.log"),"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit"})
                                .to_string()
                        ],
                    )
                    .expect("monitor row");
        };
        for index in 0..105 {
            let id = format!("active-{index}");
            insert_monitor(&connection, &id, "active", "running", index);
        }
        for index in 0..104 {
            let id = format!("terminal-{index}");
            let owner = if index == 50 { "deleted" } else { "active" };
            insert_monitor(&connection, &id, owner, "completed", 10);
        }
        insert_monitor(&connection, "deleted-owner", "deleted", "starting", 200);
        insert_monitor(&connection, "invalid-owner", "broken", "approval", 201);
        insert_monitor(&connection, "missing-owner", "missing", "running", 202);
        // Legacy selector: codex_entity_contracts.monitor_records.
        // Equal timestamps intentionally exercise the SQL tie behavior at the
        // terminal window boundary, including owner filtering after selection.
        let expected_terminal_ids = connection
            .prepare(
                "SELECT id FROM runtime_monitors \
                 WHERE json_extract(record,'$.status')='completed' \
                 ORDER BY json_extract(record,'$.created') DESC LIMIT ?1",
            )
            .expect("canonical terminal selector")
            .query_map([MONITOR_TERMINAL_WINDOW as i64], |row| {
                row.get::<_, String>(0)
            })
            .expect("canonical terminal ids")
            .collect::<Result<BTreeSet<_>, _>>()
            .expect("canonical terminal selection");
        assert_eq!(expected_terminal_ids.len(), MONITOR_TERMINAL_WINDOW);
        assert!(expected_terminal_ids.contains("terminal-50"));
        let transaction = connection.transaction().expect("projection transaction");
        for id in ["stale", "invalid-monitor", "invalid-owner"] {
            studio_runtime_store::durable_seq::join_window(
                &transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                id,
            )
            .expect("prior window member");
        }
        maintain_monitor_window(&transaction, &|_, _, _, _| true)
            .expect("bounded monitor projection");

        let count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM durable_rows WHERE tbl='monitor_window' AND deleted=0",
                [],
                |row| row.get(0),
            )
            .expect("live projected monitors");
        assert_eq!(count, 205);
        for id in ["deleted-owner", "missing-owner"] {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM durable_rows WHERE tbl='monitor_window' AND id=?1 AND deleted=0)",
                    [id],
                    |row| row.get(0),
                )
                .expect("owner-filtered projection query");
            assert!(!exists, "inactive or missing owner must not project {id}");
        }
        for id in ["active-0", "active-104", "invalid-owner"] {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM durable_rows WHERE tbl='monitor_window' AND id=?1 AND deleted=0)",
                    [id],
                    |row| row.get(0),
                )
                .expect("retained monitor query");
            assert!(exists, "active or malformed source must retain {id}");
        }
        let actual_terminal_ids = transaction
            .prepare(
                "SELECT id FROM durable_rows WHERE tbl='monitor_window' AND id GLOB 'terminal-*' AND deleted=0",
            )
            .expect("projected terminal ids")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("projected terminal rows")
            .collect::<Result<BTreeSet<_>, _>>()
            .expect("projected terminal selection");
        let expected_active_terminal_ids = expected_terminal_ids
            .into_iter()
            .filter(|id| id != "terminal-50")
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_terminal_ids, expected_active_terminal_ids);
        let stale_deleted: bool = transaction
            .query_row(
                "SELECT COALESCE((SELECT deleted=1 FROM durable_rows WHERE tbl='monitor_window' AND id='stale'),1)",
                [],
                |row| row.get(0),
            )
            .expect("stale canonical tombstone");
        assert!(stale_deleted, "unselected canonical rows still tombstone");
        let invalid_deleted: bool = transaction.query_row(
            "SELECT COALESCE((SELECT deleted=1 FROM durable_rows WHERE tbl='monitor_window' AND id='invalid-monitor'),1)", [], |row| row.get(0),
        ).expect("unselected monitor tombstone");
        assert!(invalid_deleted);
        assert!(
            studio_runtime_store::durable_seq::in_window(
                &transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                "invalid-owner",
            )
            .expect("window reads"),
            "invalid owner keeps its window member"
        );
        transaction
            .execute(
                "INSERT INTO runtime_monitors(id,record) VALUES('invalid-monitor','{')",
                [],
            )
            .expect("invalid monitor");
        assert!(
            matches!(maintain_monitor_window(&transaction, &|_, _, _, _| true), Err(StoreError::InvalidStoredRecord { object: "monitors", id }) if id == "invalid-monitor")
        );
    }

    #[test]
    fn indexed_monitor_window_matches_python_tie_boundary_and_filters_owner_after_selection() {
        let mut connection = Connection::open_in_memory().expect("database");
        // Keep a record-only legacy source table so non-object corruption reaches the reader.
        // The current provider column rejects that corruption at write time.
        connection
            .execute_batch(
                "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY,record TEXT NOT NULL, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
                 CREATE INDEX runtime_monitor_status ON runtime_monitors(
                     json_extract(record,'$.status'),json_extract(record,'$.created'));
                 CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
                 CREATE TABLE sync_entities(
                     collection TEXT NOT NULL,id TEXT NOT NULL,seq INTEGER NOT NULL,
                     hash TEXT NOT NULL,payload TEXT,deleted INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY(collection,id));
                 CREATE TABLE sync_documents(seq INTEGER PRIMARY KEY);
                 CREATE TABLE durable_rows(tbl TEXT,id TEXT,seq INTEGER,deleted INTEGER,at INTEGER,PRIMARY KEY(tbl,id)); CREATE TABLE sync_generation(id INTEGER PRIMARY KEY,value INTEGER NOT NULL); INSERT INTO sync_generation VALUES(1,0);
                 CREATE TABLE sync_entity_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                 INSERT INTO runtime_agents VALUES
                     ('active','{\"id\":\"active\",\"provider\":\"codex\",\"deletedAt\":null}'),
                     ('deleted','{\"id\":\"deleted\",\"provider\":\"codex\",\"deletedAt\":1}'),
                     ('broken','[\"not an agent object\"]');",
            )
            .expect("indexed projection fixture");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        for index in 0..104 {
            let id = format!("terminal-{index}");
            let agent = if index == 50 { "deleted" } else { "active" };
            connection
                .execute(
                    "INSERT INTO runtime_monitors(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        id,
                        json!({"id":id,"agent":agent,"epoch":0,"status":"completed","exitCode":0,"created":10,"requestId":format!("request-{id}"),"activityAt":10,"activityGeneration":0,"bytes":0,"command":"","cwd":std::env::temp_dir(),"interactive":false,"livenessCommand":"","log":std::env::temp_dir().join("watch.log"),"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit"})
                            .to_string()
                    ],
                )
                .expect("indexed monitor row");
        }
        let expected_terminal_ids = connection
            .prepare(
                "SELECT id FROM runtime_monitors \
                 WHERE json_extract(record,'$.status')='completed' \
                 ORDER BY json_extract(record,'$.created') DESC LIMIT ?1",
            )
            .expect("canonical terminal selector")
            .query_map([MONITOR_TERMINAL_WINDOW as i64], |row| {
                row.get::<_, String>(0)
            })
            .expect("canonical terminal ids")
            .collect::<Result<BTreeSet<_>, _>>()
            .expect("canonical terminal selection");
        assert_eq!(expected_terminal_ids.len(), MONITOR_TERMINAL_WINDOW);
        assert!(expected_terminal_ids.contains("terminal-50"));

        let transaction = connection.transaction().expect("projection transaction");
        TRACED_SQL.with_borrow_mut(Vec::clear);
        transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_expanded_sql));
        maintain_monitor_window(&transaction, &|_, _, _, _| true)
            .expect("indexed monitor projection");
        transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
        let selectors = TRACED_SQL
            .with_borrow_mut(std::mem::take)
            .into_iter()
            .filter(|sql| sql.starts_with("SELECT id,record FROM runtime_monitors"))
            .collect::<Vec<_>>();
        assert_eq!(selectors.len(), 1 + MONITOR_TERMINAL_STATUSES.len());
        for sql in selectors {
            let plan = transaction
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("actual selector plan")
                .query_map([], |row| row.get::<_, String>(3))
                .expect("plan rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("plan details");
            assert!(
                plan.iter().any(|line| line
                    .contains("SEARCH runtime_monitors USING INDEX runtime_monitor_status")),
                "{sql}: {plan:?}"
            );
        }
        let actual_ids = transaction
            .prepare("SELECT id FROM durable_rows WHERE tbl='monitor_window' AND deleted=0")
            .expect("projected terminal ids")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("projected terminal rows")
            .collect::<Result<BTreeSet<_>, _>>()
            .expect("projected terminal set");
        let expected_active_ids = expected_terminal_ids
            .into_iter()
            .filter(|id| id != "terminal-50")
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_ids, expected_active_ids);
    }

    #[test]
    fn sync_projection_skips_malformed_persisted_rows() {
        let mut connection = Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch("CREATE TABLE runtime_agents (id TEXT, record TEXT);")
            .expect("create source table");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        connection
            .execute(
                "INSERT INTO runtime_agents (id, record) VALUES ('broken-agent', '{')",
                [],
            )
            .expect("insert malformed source row");
        let transaction = connection.transaction().expect("begin test transaction");

        assert!(
            projection_records(&transaction, "runtime_agents")
                .expect("sync projection ignores one malformed stored row")
                .is_empty()
        );
        assert!(matches!(
            stored_records(&transaction, "runtime_agents"),
            Err(StoreError::InvalidSyncObject { scope, id })
                if scope == "runtime_agents" && id == "broken-agent"
        ));
    }

    #[tokio::test]
    async fn entity_projection_skips_unused_task_snapshot_and_keeps_task_window_behavior() {
        let root = tempfile::tempdir().expect("isolated state directory");
        let store = RuntimeStore::open(Some(root.path()))
            .await
            .expect("open isolated runtime store");
        let agent = json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"agent","rootId":"agent","epoch":0,"status":"idle","isLead":true});
        let snapshot = ProjectVaultSnapshot::default();

        store
            .transaction(move |transaction| {
                let agent = studio_runtime_store::AgentRecord::from_value(agent.clone())?;
                studio_runtime_store::AgentRepository::put(transaction, &agent)?;
                transaction.execute(
                    "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        "task",
                        json!({"id":"task","agent":"agent","status":"running","created":1,"itemId":"item","turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"})
                            .to_string()
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("seed task-window fixture");

        let state_dir = root.path().to_string_lossy().into_owned();
        let snapshot_for_first_maintenance = snapshot.clone();
        let statements = store
            .transaction(move |transaction| {
                TRACED_SQL.with_borrow_mut(Vec::clear);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                let rooms = crate::sync_entity_context::RoomDirectory::load(transaction)?;
                maintain_entity_projections_with_server_id(
                    transaction,
                    &state_dir,
                    None,
                    &snapshot_for_first_maintenance,
                    &rooms,
                    |_, _, _, _| true,
                )?;
                let payload: Value = studio_json::from_str(
                    window_rows(transaction, "task")["task"]
                        .0
                        .as_deref()
                        .expect("task document"),
                )?;
                assert_eq!(payload["value"]["status"], "running");
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                Ok(TRACED_SQL.with_borrow_mut(std::mem::take))
            })
            .await
            .expect("maintain canonical task projection");

        assert!(
            statements
                .iter()
                .any(|sql| { sql.starts_with("SELECT t.id,t.record FROM runtime_tasks t WHERE") })
        );
        assert!(
            !statements
                .iter()
                .any(|sql| { sql == "SELECT id,record FROM runtime_tasks ORDER BY rowid" })
        );

        let state_dir = root.path().to_string_lossy().into_owned();
        let snapshot_for_second_maintenance = snapshot.clone();
        store
            .transaction(move |transaction| {
                transaction.execute(
                    "UPDATE runtime_tasks SET record=?1 WHERE id='task'",
                    [json!({"id":"task","agent":"agent","status":"completed","created":2,"itemId":"item","turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"}).to_string()],
                )?;
                let rooms = crate::sync_entity_context::RoomDirectory::load(transaction)?;
                maintain_entity_projections_with_server_id(
                    transaction,
                    &state_dir,
                    None,
                    &snapshot_for_second_maintenance,
                    &rooms,
                    |_, _, _, _| true,
                )?;
                let payload: Value = studio_json::from_str(
                    window_rows(transaction, "task")["task"].0.as_deref().expect("task document"),
                )?;
                assert_eq!(payload["value"]["status"], "completed");

                let mut deleted_agent = studio_runtime_store::AgentRepository::get(
                    transaction,
                    &studio_runtime_store::AgentId::from("agent"),
                )?
                .expect("fixture agent exists");
                deleted_agent.deleted_at = Some((3) as f64);
                studio_runtime_store::AgentRepository::put(transaction, &deleted_agent)?;
                let rooms = crate::sync_entity_context::RoomDirectory::load(transaction)?;
                maintain_entity_projections_with_server_id(
                    transaction,
                    &state_dir,
                    None,
                    &snapshot_for_second_maintenance,
                    &rooms,
                    |_, _, _, _| true,
                )?;
                let deleted: bool = transaction.query_row(
                    "SELECT COALESCE((SELECT deleted FROM durable_rows WHERE tbl='task_window' AND id='task'),1)",
                    [],
                    |row| row.get(0),
                )?;
                assert!(deleted);
                Ok(())
            })
            .await
            .expect("task-window eligibility transition remains canonical");
    }

    #[tokio::test]
    async fn task_window_fast_path_preserves_full_rebuild_results_for_stable_running_updates() {
        let root = tempfile::tempdir().expect("isolated state directory");
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .expect("open isolated runtime store");
        store
            .transaction(move |transaction| {
                for id in ["live", "deleting"] {
                    let agent = studio_runtime_store::AgentRecord::from_value(
                        json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
                            "id": id,
                            "rootId": id,
                            "epoch": 0,
                            "accountKey": "default",
                            "status": "idle",
                            "isLead": true
                        }),
                    )?;
                    studio_runtime_store::AgentRepository::put(transaction, &agent)?;
                }
                for index in 0..105 {
                    let id = format!("history-{index}");
                    transaction.execute(
                        "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                        rusqlite::params![
                            id,
                            json!({
                                "id": id,
                                "agent": "live",
                                "status": "completed",
                                "created": index, "itemId": id, "turnId":"turn", "type":"commandExecution", "kind":"command", "name":"commandExecution"
                            })
                            .to_string()
                        ],
                    )?;
                }
                for (id, agent) in [("running-stable", "live"), ("running-deleting", "deleting")] {
                    transaction.execute(
                        "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                        rusqlite::params![
                            id,
                            json!({
                                "id": id,
                                "agent": agent,
                                "status": "running",
                                "created": 200, "itemId": id, "turnId":"turn", "type":"commandExecution", "kind":"command", "name":"commandExecution"
                            })
                            .to_string()
                        ],
                    )?;
                }
                maintain_task_window(transaction, &|_, _, _, _| true)
            })
            .await
            .expect("seed complete task-window projection");

        let task_projection = |connection: &Connection| window_rows(connection, "task");
        let full_window_was_traced = || {
            TRACED_SQL.with_borrow(|statements| {
                statements
                    .iter()
                    .any(|sql| sql.starts_with("SELECT t.id,t.record FROM runtime_tasks t WHERE"))
            })
        };

        let direct_running_projection = store
            .transaction(move |transaction| {
                TRACED_SQL.with_borrow_mut(Vec::clear);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                let previous_raw: String = transaction.query_row(
                    "SELECT record FROM runtime_tasks WHERE id='running-stable'",
                    [],
                    |row| row.get(0),
                )?;
                let previous: Value = studio_json::from_str(&previous_raw)?;
                let mut updated = previous.clone();
                updated["tail"] = json!("streaming progress");
                transaction.execute(
                    "UPDATE runtime_tasks SET record=?1 WHERE id='running-stable'",
                    [updated.to_string()],
                )?;
                maintain_task_window_after_write(
                    transaction,
                    Some(&previous),
                    &<studio_runtime_store::TaskRecord as serde::Deserialize>::deserialize(
                        &updated,
                    )?,
                    &|_, _, _, _| true,
                )?;
                assert!(
                    !full_window_was_traced(),
                    "stable running write must not rescan history"
                );
                let direct = task_projection(transaction);
                maintain_task_window(transaction, &|_, _, _, _| true)?;
                assert_eq!(task_projection(transaction), direct);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                Ok(direct)
            })
            .await
            .expect("stable running update stays equivalent to a full rebuild");
        assert_eq!(direct_running_projection.len(), 102);

        store
            .transaction(move |transaction| {
                TRACED_SQL.with_borrow_mut(Vec::clear);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                let raw: String = transaction.query_row(
                    "SELECT record FROM runtime_tasks WHERE id='running-stable'",
                    [],
                    |row| row.get(0),
                )?;
                let task: studio_runtime_store::TaskRecord = studio_json::from_str(&raw)?;
                let task = studio_runtime_store::projection_value(&task);
                leave_task_window(transaction, "running-stable")?;
                maintain_task_window_after_write(
                    transaction,
                    Some(&task),
                    &<studio_runtime_store::TaskRecord as serde::Deserialize>::deserialize(&task)?,
                    &|_, _, _, _| true,
                )?;
                assert!(
                    !full_window_was_traced(),
                    "a running write projects itself without a rebuild"
                );
                let repaired = task_projection(transaction);
                maintain_task_window(transaction, &|_, _, _, _| true)?;
                assert_eq!(task_projection(transaction), repaired);
                assert!(
                    repaired
                        .get("running-stable")
                        .is_some_and(|(_, deleted)| !deleted)
                );
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                Ok(())
            })
            .await
            .expect("missing stable-running projection is repaired by full rebuild");

        let compare_after_full_write = |id: &str, status: &str, created: i64| {
            let store = &store;
            let id = id.to_owned();
            let status = status.to_owned();
            async move {
                let task_projection = |connection: &Connection| window_rows(connection, "task");
                let full_window_was_traced = || {
                    TRACED_SQL.with_borrow(|statements| {
                        statements.iter().any(|sql| {
                            sql.starts_with("SELECT t.id,t.record FROM runtime_tasks t WHERE")
                        })
                    })
                };
                store
                    .transaction(move |transaction| {
                        TRACED_SQL.with_borrow_mut(Vec::clear);
                        transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                        let previous_raw: Option<String> = transaction
                            .query_row(
                                "SELECT record FROM runtime_tasks WHERE id=?1",
                                [&id],
                                |row| row.get(0),
                            )
                            .optional()?;
                        let previous = previous_raw
                            .as_deref()
                            .map(studio_json::from_str::<Value>)
                            .transpose()?;
                        let mut task = previous
                            .clone()
                            .unwrap_or_else(|| json!({"id":id,"agent":"live","itemId":id,"turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"}));
                        task["status"] = json!(status);
                        task["created"] = json!(created);
                        transaction.execute(
                            "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2) \
                         ON CONFLICT(id) DO UPDATE SET record=excluded.record",
                            rusqlite::params![id, task.to_string()],
                        )?;
                        maintain_task_window_after_write(
                            transaction,
                            previous.as_ref(),
                            &<studio_runtime_store::TaskRecord as serde::Deserialize>::deserialize(&task)?,
                            &|_, _, _, _| true,
                        )?;
                        assert!(
                            !full_window_was_traced(),
                            "a live-agent write must not rebuild the window"
                        );
                        let updated = task_projection(transaction);
                        maintain_task_window(transaction, &|_, _, _, _| true)?;
                        assert_eq!(task_projection(transaction), updated);
                        transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                        Ok(())
                    })
                    .await
                    .expect("changed task must match a full window rebuild");
            }
        };

        compare_after_full_write("running-stable", "completed", 300).await;
        compare_after_full_write("new-latest", "completed", 301).await;

        store
            .transaction(move |transaction| {
                TRACED_SQL.with_borrow_mut(Vec::clear);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                let previous_raw: String = transaction.query_row(
                    "SELECT record FROM runtime_tasks WHERE id='running-deleting'",
                    [],
                    |row| row.get(0),
                )?;
                let previous: Value = studio_json::from_str(&previous_raw)?;
                let mut deleting_agent = studio_runtime_store::AgentRepository::get(
                    transaction,
                    &studio_runtime_store::AgentId::from("deleting"),
                )?
                .expect("agent exists");
                deleting_agent.deleted_at = Some((1) as f64);
                studio_runtime_store::AgentRepository::put(transaction, &deleting_agent)?;
                maintain_task_window_after_write(
                    transaction,
                    Some(&previous),
                    &<studio_runtime_store::TaskRecord as serde::Deserialize>::deserialize(
                        &previous,
                    )?,
                    &|_, _, _, _| true,
                )?;
                assert!(
                    full_window_was_traced(),
                    "agent eligibility change must rebuild"
                );
                let projection = task_projection(transaction);
                assert!(
                    projection
                        .get("running-deleting")
                        .is_some_and(|(_, deleted)| *deleted)
                );
                maintain_task_window(transaction, &|_, _, _, _| true)?;
                assert_eq!(task_projection(transaction), projection);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                Ok(())
            })
            .await
            .expect("deleted-agent eligibility change must match a full rebuild");
    }

    /// Live task projections: id to payload.
    fn live_task_projection(connection: &Connection) -> BTreeMap<String, Option<String>> {
        window_rows(connection, "task")
            .into_iter()
            .filter(|(_, (_, left))| !left)
            .map(|(id, (payload, _))| (id, payload))
            .collect()
    }

    /// Writes one task as `put_task` does and applies the incremental window.
    fn write_task(transaction: &Transaction<'_>, task: &Value) -> StoreResult<()> {
        let id = task["id"].as_str().expect("task id");
        let previous: Option<Value> = transaction
            .query_row(
                "SELECT record FROM runtime_tasks WHERE id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|raw| studio_json::from_str(&raw))
            .transpose()?;
        transaction.execute(
            "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2) \
             ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            rusqlite::params![id, task.to_string()],
        )?;
        maintain_task_window_after_write(
            transaction,
            previous.as_ref(),
            &<studio_runtime_store::TaskRecord as serde::Deserialize>::deserialize(task)?,
            &|_, _, _, _| true,
        )
    }

    async fn task_window_store() -> (tempfile::TempDir, studio_runtime_store::RuntimeStore) {
        let root = tempfile::tempdir().expect("isolated state directory");
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .expect("open isolated runtime store");
        store
            .transaction(|transaction| {
                for id in ["a", "b"] {
                    let agent = studio_runtime_store::AgentRecord::from_value(
                        json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
                            "id": id, "rootId": id, "epoch": 0, "status": "idle", "accountKey": "default", "isLead": true
                        }),
                    )?;
                    studio_runtime_store::AgentRepository::put(transaction, &agent)?;
                }
                maintain_task_window(transaction, &|_, _, _, _| true)
            })
            .await
            .expect("seed agents and mark the window migrated");
        (root, store)
    }

    #[tokio::test]
    async fn incremental_task_window_equals_full_rebuild_over_random_writes() {
        let (_root, store) = task_window_store().await;
        store
            .transaction(|transaction| {
                // Start at the production history boundary so every completed insert
                // exercises eviction, without hundreds of below-boundary rebuilds.
                for index in 0..TASK_WINDOW_HISTORY_ROWS {
                    let id = format!("history-{index}");
                    transaction.execute(
                        "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                        rusqlite::params![
                            id,
                            json!({"id":id,"agent":"a","status":"completed","created":index,"itemId":id,"turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"})
                                .to_string()
                        ],
                    )?;
                }
                maintain_task_window(transaction, &|_, _, _, _| true)?;
                // Deterministic xorshift sequence; `created` is unique per task.
                let mut state = 0x9E37_79B9_7F4A_7C15_u64;
                let mut next = move |bound: u64| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state % bound
                };
                let mut created = BTreeMap::<u64, u64>::new();
                for step in 0..32_u64 {
                    let index = next(12);
                    let created = *created
                        .entry(index)
                        .or_insert_with(|| TASK_WINDOW_HISTORY_ROWS as u64 + step * 7 + 1);
                    let status = ["running", "completed", "failed", "completed"][next(4) as usize];
                    let agent = ["a", "b"][next(2) as usize];
                    let task = json!({
                        "id": format!("t{index}"), "agent": agent,
                        "status": status, "created": created, "itemId":format!("t{index}"), "turnId":"turn", "type":"commandExecution", "kind":"command", "name":"commandExecution"
                    });
                    write_task(transaction, &task)?;
                    let incremental = live_task_projection(transaction);
                    maintain_task_window(transaction, &|_, _, _, _| true)?;
                    let full = live_task_projection(transaction);
                    assert_eq!(full, incremental, "step {step} write {task}");
                }
                assert!(
                    !live_task_projection(transaction).contains_key("history-0"),
                    "random completed writes evict the oldest history row"
                );
                Ok(())
            })
            .await
            .expect("random task writes");
    }

    #[tokio::test]
    async fn task_write_cost_does_not_grow_with_history_size() {
        let (_root, store) = task_window_store().await;
        let mut statements = Vec::new();
        for history in [150_i64, 1500] {
            let count = store
                .transaction(move |transaction| {
                    for index in 0..history {
                        transaction.execute(
                            "INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                            rusqlite::params![
                                format!("h{history}-{index}"),
                                json!({"id": format!("h{history}-{index}"), "agent": "a",
                                       "status": "completed", "created": index, "itemId":format!("h{history}-{index}"), "turnId":"turn", "type":"commandExecution", "kind":"command", "name":"commandExecution"})
                                .to_string()
                            ],
                        )?;
                    }
                    maintain_task_window(transaction, &|_, _, _, _| true)?;
                    let running = json!({"id": format!("run{history}"), "agent": "a", "status": "running",
                                         "created": 1_000_000 + history, "itemId":format!("run{history}"), "turnId":"turn", "type":"commandExecution", "kind":"command", "name":"commandExecution"});
                    TRACED_SQL.with_borrow_mut(Vec::clear);
                    transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                    write_task(transaction, &running)?;
                    let mut done = running.clone();
                    done["status"] = json!("completed");
                    write_task(transaction, &done)?;
                    transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                    Ok(TRACED_SQL.with_borrow_mut(std::mem::take))
                })
                .await
                .expect("traced task writes");
            assert!(
                !count.iter().any(|sql| sql.contains("FROM runtime_tasks t")),
                "a task write must not scan the task history: {count:?}"
            );
            statements.push(count.len());
        }
        assert_eq!(
            statements[0], statements[1],
            "statements per write are independent of history"
        );
    }

    #[test]
    fn sync_vault_snapshot_skips_malformed_persisted_projects() {
        let connection = Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch("CREATE TABLE runtime_projects (id TEXT, record TEXT);")
            .expect("create source table");
        connection
            .execute(
                "INSERT INTO runtime_projects (id, record) VALUES ('broken-project', '{')",
                [],
            )
            .expect("insert malformed project row");

        let snapshot = ProjectVaultSnapshot::capture(
            Path::new("/unused"),
            &ProjectVaultSnapshot::stored_projects(&connection).expect("project rows"),
        );
        let mut connection = connection;
        let transaction = connection.transaction().expect("begin sync transaction");
        snapshot
            .recheck(&transaction)
            .expect("malformed row stays outside the valid project snapshot");
    }

    #[test]
    fn draft_identity_matches_row_key_and_allows_extension_members() {
        let validate_draft = |id: &str, payload: &Value| {
            studio_runtime_schema::drafts::DraftVersion::from_stored_value(id, payload.clone())
        };
        // A chat has one draft: its id is the chat id.
        let payload =
            json!({"device":"phone","session":"chat","id":"chat","text":"draft","extra":true});
        assert!(validate_draft("chat", &payload).is_ok());
        assert!(
            validate_draft(
                "phone:tab:chat",
                &json!({"device":"phone","session":"chat","id":"phone:tab:chat","text":"draft"})
            )
            .is_err()
        );
        assert!(validate_draft("chat", &json!({})).is_err());
        assert!(validate_draft("chat", &Value::Null).is_err());
        assert!(validate_draft("other", &json!({"device":"phone","session":"chat"})).is_err());
    }

    #[test]
    fn preference_rank_clamps_future_clock_and_keeps_other_fields() {
        let scope = PreferenceScope::User;
        let current = Map::new();
        let incoming = parse_preferences(&json!({
            "[\"codex-studio-preferences-v1\",\"theme\"]": {"value":"dark","timestamp":900000,"writer":"a"},
            "[\"codex-studio-preferences-v1\",\"mainFontSize\"]": {"value":18,"timestamp":100,"writer":"a"}
        }), scope).expect("valid preference fields");
        let merged = merge_preferences(&current, &incoming, 1_000).expect("merge succeeds");
        assert_eq!(
            merged["[\"codex-studio-preferences-v1\",\"theme\"]"]["timestamp"],
            1_000
        );
        assert_eq!(
            merged["[\"codex-studio-preferences-v1\",\"mainFontSize\"]"]["value"],
            18
        );
    }

    #[test]
    fn preference_validation_rejects_unknown_fields_and_scope_leaks() {
        assert!(parse_preferences(&json!({
            "[\"codex-studio-preferences-v1\",\"theme\"]": {"value":"blue","timestamp":1,"writer":"a"}
        }), PreferenceScope::User).is_err());
        let key = r#"["codex-studio-preferences-v1","showTerminals"]"#;
        for value in [json!(true), json!(false)] {
            let fields = Value::Object(preference_field(key, value.clone(), 1));
            let parsed = parse_preferences(&fields, PreferenceScope::User)
                .expect("terminal visibility is a user preference");
            assert_eq!(parsed[key]["value"], value);
            assert!(parse_preferences(&fields, PreferenceScope::Server).is_err());
        }
        for value in [Value::Null, json!("false"), json!(0)] {
            assert!(
                parse_preferences(
                    &Value::Object(preference_field(key, value, 1)),
                    PreferenceScope::User,
                )
                .is_err()
            );
        }
        assert!(parse_preferences(&json!({
            "[\"codex-project-tree:/state\",\"/repo\"]": {"value":true,"timestamp":1,"writer":"a"}
        }), PreferenceScope::User).is_err());
    }

    #[test]
    fn model_choices_preference_validates_scope_and_value() {
        let key = r#"["studio-model-choices-v1"]"#;
        for value in [
            json!({}),
            json!({"codex:gpt-6-astra": "whitelisted", "claude:opus": "blacklisted"}),
            json!({"codex:new": "untouched"}),
        ] {
            let fields = Value::Object(preference_field(key, value, 1));
            assert!(parse_preferences(&fields, PreferenceScope::Server).is_ok());
            assert!(parse_preferences(&fields, PreferenceScope::User).is_err());
        }
        for value in [
            Value::Null,
            json!(true),
            json!(["codex:a"]),
            json!({"astra": "blacklisted"}),
            json!({":opus": "blacklisted"}),
            json!({"codex:": "blacklisted"}),
            json!({"codex:a": "hidden"}),
        ] {
            assert!(
                parse_preferences(
                    &Value::Object(preference_field(key, value, 1)),
                    PreferenceScope::Server
                )
                .is_err()
            );
        }
    }

    fn preference_field(name: &str, value: Value, timestamp: i64) -> Map<String, Value> {
        Map::from_iter([(
            name.to_owned(),
            json!({"value":value,"timestamp":timestamp,"writer":"sync-policy-test"}),
        )])
    }

    #[tokio::test]
    async fn model_choices_persist_retries_and_preserves_other_fields_on_reopen() {
        let root = tempfile::tempdir().expect("isolated state");
        let store = RuntimeStore::open(Some(root.path()))
            .await
            .expect("open store");
        let key = r#"["studio-model-choices-v1"]"#;
        let other = r#"["codex-project-tree:/state","/repo"]"#;
        let user = r#"["codex-studio-preferences-v1","theme"]"#;
        let apply = |value, version| {
            parse_preferences(
                &Value::Object(preference_field(key, value, version)),
                PreferenceScope::Server,
            )
            .expect("valid model choices")
        };
        push_preferences(
            &store,
            PreferenceScope::Server,
            preference_field(other, json!(true), 1),
            |_| true,
            || {},
        )
        .await
        .expect("other preference");
        push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(user, json!("dark"), 1),
            |_| true,
            || {},
        )
        .await
        .expect("user preference");
        let first = apply(json!({"codex:astra": "whitelisted"}), 2);
        let merged = push_preferences(
            &store,
            PreferenceScope::Server,
            first.clone(),
            |_| true,
            || {},
        )
        .await
        .expect("first apply");
        let duplicate = push_preferences(&store, PreferenceScope::Server, first, |_| true, || {})
            .await
            .expect("retry after response loss");
        assert_eq!(duplicate, merged);
        let changed = push_preferences(
            &store,
            PreferenceScope::Server,
            apply(json!({"codex:astra": "blacklisted"}), 3),
            |_| true,
            || {},
        )
        .await
        .expect("new edit");
        assert_eq!(changed[key]["value"], json!({"codex:astra": "blacklisted"}));
        let stale = push_preferences(
            &store,
            PreferenceScope::Server,
            apply(json!({}), 1),
            |_| true,
            || {},
        )
        .await
        .expect("stale base");
        assert_eq!(stale[key]["value"], json!({"codex:astra": "blacklisted"}));
        assert_eq!(stale[other]["value"], true);
        drop(store);
        let reopened = RuntimeStore::open(Some(root.path()))
            .await
            .expect("reopen SQLite");
        let reader = reopened.test_reader().expect("reader");
        let persisted: Value = studio_json::from_str(
            &SyncDocumentsRepository::preference_payload(&reader, "server")
                .expect("read")
                .expect("server row"),
        )
        .expect("JSON");
        assert_eq!(
            persisted["value"]["fields"][key]["value"],
            json!({"codex:astra": "blacklisted"})
        );
        assert_eq!(persisted["value"]["fields"][other]["value"], true);
        let user_row: Value = studio_json::from_str(
            &SyncDocumentsRepository::preference_payload(&reader, "user")
                .expect("read user")
                .expect("user row"),
        )
        .expect("JSON");
        assert_eq!(user_row["value"]["fields"][user]["value"], "dark");
        assert!(user_row["value"]["fields"].get(key).is_none());
        drop(reader);
        let all = push_preferences(
            &reopened,
            PreferenceScope::Server,
            apply(json!({}), 4),
            |_| true,
            || {},
        )
        .await
        .expect("all including future models");
        assert_eq!(all[key]["value"], json!({}));
    }

    #[tokio::test]
    async fn provider_update_preference_notifies_only_after_effective_user_value_change() {
        let root = tempfile::tempdir().expect("isolated state directory");
        let store = RuntimeStore::open(Some(root.path()))
            .await
            .expect("open isolated runtime store");
        let notifications = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let notification = |notifications: &std::sync::Arc<std::sync::atomic::AtomicUsize>| {
            let notifications = std::sync::Arc::clone(notifications);
            move || {
                notifications.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        };
        let key = crate::provider_versions::PROVIDER_UPDATE_PREFERENCE_KEY;

        let merged = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(key, json!(false), 1),
            |_| true,
            notification(&notifications),
        )
        .await
        .expect("provider preference writes");
        assert_eq!(
            merged[key]["value"], false,
            "the existing preference field map is returned"
        );
        assert_eq!(
            notifications.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "absent/default-enabled to disabled commits and wakes once"
        );

        let _ = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(key, json!(false), 2),
            |_| true,
            notification(&notifications),
        )
        .await
        .expect("same effective value with newer metadata writes");
        let _ = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(key, json!(false), 2),
            |_| true,
            notification(&notifications),
        )
        .await
        .expect("identical preference update is a no-op");
        let _ = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(
                r#"["codex-studio-preferences-v1","theme"]"#,
                json!("dark"),
                3,
            ),
            |_| true,
            notification(&notifications),
        )
        .await
        .expect("unrelated appearance preference writes");
        assert_eq!(
            notifications.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "metadata-only and unrelated changes do not wake the provider monitor"
        );
        let _ = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(key, json!(true), 4),
            |_| true,
            notification(&notifications),
        )
        .await
        .expect("provider preference can be re-enabled");
        assert_eq!(
            notifications.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "enabled transition also wakes after commit"
        );

        let malformed_fields = Map::from_iter([(
            key.to_owned(),
            json!({"value":"invalid","timestamp":4,"writer":"legacy"}),
        )]);
        let malformed = canonical_json(&json!({
            "collection":"uiPreferences",
            "id":"user",
            "value":{"fields":malformed_fields}
        }));
        let malformed_for_write = malformed.clone();
        store
            .transaction(move |transaction| {
                SyncDocumentsRepository::put_preferences(
                    transaction,
                    "user",
                    &malformed_for_write,
                )?;
                Ok(())
            })
            .await
            .expect("malformed persisted fixture seeds");
        let result = push_preferences(
            &store,
            PreferenceScope::User,
            preference_field(key, json!(true), 5),
            |_| true,
            notification(&notifications),
        )
        .await;
        assert!(
            matches!(result, Err(SyncPolicyError::InvalidStoredObject { .. })),
            "malformed existing preference remains refused"
        );
        assert_eq!(
            notifications.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "a rolled-back malformed stored preference does not wake"
        );
        let payload = store.test_reader().expect("preference reader");
        assert_eq!(
            SyncDocumentsRepository::preference_payload(&payload, "user")
                .expect("stored payload query"),
            Some(malformed),
            "the refused update leaves malformed stored data untouched"
        );
    }

    #[test]
    fn cursor_limits_match_scope_contract() {
        assert_eq!(effective_limit(SYNC_ENTITY_SCOPE, 900), 500);
        assert_eq!(effective_limit("drafts", 900), 100);
        assert_eq!(effective_limit("drafts", 0), 1);
    }

    #[test]
    fn overflowing_numbers_are_rejected_by_json_parser() {
        assert!(studio_json::from_str::<Value>(r#"{"nested":[1e309]}"#).is_err());
    }

    #[test]
    fn stored_preference_envelope_identity_must_match_key() {
        let payload =
            Some(r#"{"collection":"agent","id":"wrong","value":{"fields":{}}}"#.to_owned());
        assert!(matches!(
            stored_preference_fields(payload, "user", &|_| true),
            Err(SyncPolicyError::InvalidStoredObject { scope, id })
                if scope == "uiPreferences" && id == "user"
        ));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod work_parity_tests {
    use crate::sync_views::{WorkView, work_views};
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use studio_json::{OwnedRawJson as RawValue, Value, json};
    use studio_runtime_store::{RuntimeStore, StoreError, WorkItemsRepository};

    #[tokio::test]
    async fn read_time_work_rows_equal_work_views() {
        let root = tempfile::tempdir().expect("isolated state directory");
        let store = RuntimeStore::open(Some(root.path()))
            .await
            .expect("open isolated runtime store");
        let raw = json!({
            "id": "w", "rootId": "lead", "title": "work", "status": "review",
            "results": [{"id": "r0"}, {"id": "r1"}],
            "decisions": [{"id": "d0"}],
            "releases": [{"id": "x0"}],
        })
        .to_string();
        let source = raw.clone();
        let published = store
            .transaction(move |transaction| {
                transaction.execute(
                    "INSERT INTO runtime_work(id,record) VALUES('w',?1)",
                    [&source],
                )?;
                studio_runtime_store::work_history::reconcile(
                    transaction,
                    "w",
                    &studio_json::from_str::<studio_json::Value>(&source)?,
                )?;
                #[derive(Deserialize)]
                struct Stored {
                    value: RawValue,
                }
                let collections = studio_json::to_string(studio_runtime_store::WORK_COLLECTIONS)?;
                let page = crate::entity_scope::resume(
                    transaction,
                    None,
                    Some(&collections),
                    None,
                    None,
                    true,
                    512,
                )?;
                let mut out = BTreeMap::new();
                for row in page.rows {
                    let stored: Stored =
                        studio_json::from_str(row.payload.as_deref().unwrap_or("{}"))?;
                    out.insert((row.collection, row.id), stored.value.as_str().to_owned());
                }
                Ok::<_, StoreError>(out)
            })
            .await
            .expect("read work rows");

        let expected = store
            .transaction(move |transaction| {
                let record: Value = studio_json::from_str(&raw)?;
                let item =
                    WorkItemsRepository::work_record(transaction, "w")?.expect("stored work item");
                let refusal = crate::work_write::current_submission_refusal(transaction, &record)
                    .expect("submission refusal");
                let mut out = BTreeMap::new();
                for (collection, view) in work_views(&item, &record, refusal, &BTreeMap::new())? {
                    let id = match &view {
                        WorkView::Entry(entry) => entry["id"].as_str().unwrap().to_owned(),
                        WorkView::Summary(_) | WorkView::Detail(_) => "w".to_owned(),
                    };
                    out.insert((collection.to_owned(), id), studio_json::to_string(&view)?);
                }
                Ok::<_, StoreError>(out)
            })
            .await
            .expect("work views");

        assert_eq!(expected.len(), 6);
        assert_eq!(published, expected);
    }
}
