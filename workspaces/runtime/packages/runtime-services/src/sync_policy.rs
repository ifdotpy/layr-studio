//! Pure sync document, cursor and preference decisions.
use studio_runtime_schema::shared_values::MODEL_CHOICES_PREFERENCE_KEY;
use studio_runtime_store::{SyncPolicyQueries, clock_schema::SyncEntityMeta};

use crate::event_write::truthy;
pub use crate::sync_views::{
    AgentViewContext, ComplaintViewContext, RoomViewContext, agent_entity_view, openapi_document,
    project_persisted_record,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use studio_json::OwnedRawJson as RawValue;
use studio_json::{Map, Value, json};
use studio_runtime_schema::SYNC_ENTITY_SCOPE;
pub use studio_runtime_schema::shared_values::{SYNC_BATCH_LIMIT, SYNC_ENTITY_PAGE_LIMIT};
use studio_runtime_store::{
    RuntimeStore, StoreError, StoreResult, SyncDocumentsRepository, Table, TranscriptItemId,
    TranscriptItemRecord, agent_volatile,
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
    let seeded = SyncEntityMeta::value(connection, SYNC_SEEDED_MARKER)?;
    let version = SyncEntityMeta::value(connection, AGENT_ORGANIZATION_FIELDS_MARKER)?;
    let mut complete = seeded.as_deref() == Some("1")
        && version
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|value| value >= AGENT_ORGANIZATION_FIELDS_VERSION);
    if !complete {
        return Ok(false);
    }
    complete &= SyncEntityMeta::matches(connection, SUMMARY_ENTITIES_SEED_MARKER, "1")?;
    complete &= SyncEntityMeta::matches(connection, CONVERSATION_DETAILS_SEED_MARKER, "1")?;
    complete &= SyncEntityMeta::matches(connection, AGENT_PROJECTIONS_SEED_MARKER, "1")?;
    complete &= SyncEntityMeta::matches(connection, WORK_DETAILS_SEED_MARKER, "1")?;
    let table_exists = |name: &str| {
        connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
    };
    if table_exists("runtime_tasks")? && table_exists("runtime_agents")? {
        let task_window = SyncEntityMeta::value(connection, "task_window_migrated")?;
        complete &= task_window.as_deref() == Some("1");
    }
    if table_exists("runtime_events")? {
        if table_exists("runtime_agents")? {
            complete &= !SyncPolicyQueries::missing_live_queue(connection)?;
        }
        let event_window = SyncEntityMeta::value(connection, "event_window_seq")?;
        let projected_sequence = SyncPolicyQueries::collection_sequence(connection, "event")?;
        complete &= event_window
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|sequence| sequence == projected_sequence);
    }
    Ok(complete)
}

/// Rebuild renderer entity projections from persisted records in the caller's
/// transaction. Re-running this operation preserves unchanged entity
/// sequences because the repository compares canonical payloads before write.
pub fn maintain_entity_projections(
    transaction: &Transaction<'_>,
    _state_dir: &str,
    rooms: &crate::sync_entity_context::RoomDirectory,
    validate_projection: impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let project_server_id =
        studio_runtime_store::RuntimeStore::transaction_local_server_id(transaction)?;
    maintain_entity_projections_with_server_id(
        transaction,
        _state_dir,
        project_server_id.as_deref(),
        rooms,
        validate_projection,
    )
}

pub fn maintain_entity_projections_with_server_id(
    transaction: &Transaction<'_>,
    state_dir: &str,
    project_server_id: Option<&str>,
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
    if table_exists(transaction, "user_terminals")? {
        for terminal in studio_runtime_store::TerminalsRepository::list(transaction)? {
            if let Some(id) = terminal.get("id").and_then(Value::as_str) {
                crate::terminal_driver::publish_terminal(transaction, id)?;
            }
        }
    }
    for (id, _) in peer_agents {
        crate::queue_write::publish_queue(transaction, id)?;
    }
    let mut current_rooms = BTreeSet::new();
    crate::diagnostics::measure_runtime_stage("projection.direct_entities", || {
        for (table, collection) in [
            ("runtime_rooms", "room"),
            ("runtime_complaints", "complaint"),
            ("runtime_projects", "project"),
        ] {
            let records = source_records.get(table).ok_or(StoreError::RecordContract(
                "entity projection source snapshot is incomplete",
            ))?;
            for (id, raw) in records {
                if table == "runtime_projects" && !valid_stored_record(table, id, raw) {
                    eprintln!("ERROR project projection skipped invalid row id={id}");
                    continue;
                }
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
                    continue;
                }
                let record: studio_runtime_schema::project_record::ProjectRecord =
                    match studio_json::from_str(raw) {
                        Ok(record) => record,
                        Err(error) => {
                            eprintln!("ERROR project projection skipped row id={id}: {error}");
                            continue;
                        }
                    };
                let studio_runtime_schema::project_record::ProjectRecord::Live(project) = record
                else {
                    retire_if_live(transaction, "project", id, &validate_projection)?;
                    continue;
                };
                let peer_leads =
                    crate::project_projection::peer_leads_of(transaction, &project.id)?;
                if let Some(projected) =
                    crate::project_projection::project_record(*project, &peer_leads)
                {
                    SyncDocumentsRepository::put_entity(
                        transaction,
                        "project",
                        id,
                        projected,
                        false,
                    )?;
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
    crate::diagnostics::measure_runtime_stage("projection.event_window", || {
        maintain_event_window(transaction, &validate_projection)
    })?;
    SyncPolicyQueries::replace_meta_marker(transaction, AGENT_PROJECTIONS_SEED_MARKER, "1")?;
    seed_summary_entities(transaction)?;
    SyncPolicyQueries::replace_meta_marker(transaction, CONVERSATION_DETAILS_SEED_MARKER, "1")?;
    SyncPolicyQueries::replace_meta_marker(transaction, WORK_DETAILS_SEED_MARKER, "1")?;
    crate::diagnostics::measure_runtime_stage("projection.canvas_chat_entities", || {
        maintain_canvas_chat_entities(transaction, &validate_projection)
    })?;

    // Python refreshes workspace/current after entity mutations only when a
    // workspace projection already exists. Initial creation belongs to seed
    // and explicit workspace publishers, not unrelated lifecycle mutations.
    let prior = SyncPolicyQueries::live_entity_payload(transaction, "workspace", "current")?;
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
        let was_in_flight = SyncPolicyQueries::was_in_flight(transaction, &row.id)?;
        if !was_in_flight {
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
        let projected_live = SyncPolicyQueries::projected_live(transaction, id)?;
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
    publish_agents(transaction, &agents, &[], project_server_id, validate)?;
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
    for id in events {
        let agent = transaction
            .query_row(
                "SELECT e.agent FROM runtime_events e JOIN runtime_agents a ON a.id=e.agent \
             WHERE e.id=?1 AND e.kind='user' AND a.deleted_at IS NULL",
                [&id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(agent) = agent else {
            continue;
        };
        let response = crate::coordination_reads::delivery_receipts(
            transaction,
            &agent,
            std::slice::from_ref(&id),
        )
        .map_err(|_| StoreError::RecordContract("message receipt projection failed"))?;
        for mut receipt in response.items {
            #[derive(serde::Serialize)]
            struct ReceiptProjection {
                agent: String,
                #[serde(flatten)]
                receipt: crate::coordination_reads::MessageReceipt,
            }
            receipt.id = crate::sync_views::bound_sync_text(&receipt.id, "id");
            receipt.error = receipt
                .error
                .map(|error| crate::sync_views::bound_sync_text(&error, "error"));
            SyncDocumentsRepository::put_entity(
                transaction,
                "messageReceipt",
                &id,
                ReceiptProjection {
                    agent: crate::sync_views::bound_sync_text(&agent, "agent"),
                    receipt,
                },
                false,
            )?;
        }
    }
    Ok(())
}

/// Seed pre-existing rows once after the summary contract is installed. Normal
/// pulls and unrelated writes never reconstruct these collections.
fn seed_summary_entities(transaction: &Transaction<'_>) -> StoreResult<()> {
    if SyncEntityMeta::matches(transaction, SUMMARY_ENTITIES_SEED_MARKER, "1")? {
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
    SyncEntityMeta::insert(transaction, SUMMARY_ENTITIES_SEED_MARKER, "1")?;
    Ok(())
}

/// The writer-local agent record generation, advanced by runtime_agents triggers.
pub(crate) fn agent_record_generation(transaction: &Transaction<'_>) -> StoreResult<i64> {
    Ok(
        studio_runtime_store::clock_schema::AgentRecordGeneration::get(transaction)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?
            .value,
    )
}

/// Existing agent rows the trigger journal records as changed after
/// `generation`, as (rowid, id, record).
pub(crate) fn agents_changed_since(
    transaction: &Transaction<'_>,
    generation: i64,
) -> StoreResult<Vec<(i64, String, String)>> {
    Ok(SyncPolicyQueries::agents_changed_since(
        transaction,
        generation,
    )?)
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
    if ids.is_empty() {
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
            && valid_stored_record(table, id, &raw)
        {
            let raw = if table == agent_volatile::AGENT_TABLE {
                agent_volatile::merged_text(transaction, id, raw)?
            } else {
                raw
            };
            rows.push((rowid, id.clone(), raw));
        }
    }
    rows.sort_unstable_by_key(|(rowid, _, _)| *rowid);
    Ok(rows)
}

/// Mark the views of the team leads in `roots` whose team counts changed since their
/// last publication. A lead in `skip` is published by the caller already.
fn publish_team_roots(
    transaction: &Transaction<'_>,
    roots: &BTreeSet<String>,
    skip: &BTreeSet<String>,
) -> StoreResult<()> {
    for root in roots.difference(skip) {
        let current = crate::sync_entity_context::team_counts_text(transaction, root)?;
        let published = SyncPolicyQueries::published_team_counts(transaction, root)?;
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
    if agents.is_empty() {
        return Ok(());
    }
    let mut limits = crate::account_session_runtime::limits::publication_views(transaction)?;
    for &(id, record) in agents {
        let deleted = truthy(record.get("deletedAt"));
        crate::account_session_runtime::limits::publish_agent_change_with_views(
            transaction,
            id,
            deleted,
            &mut limits,
        )?;
        mark_agent_projection(transaction, id, deleted)?;
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
    mark_agent_projection(transaction, id, deleted)
}

fn mark_agent_projection(
    transaction: &Transaction<'_>,
    id: &str,
    deleted: bool,
) -> StoreResult<()> {
    studio_runtime_store::durable_seq::mark(
        transaction,
        studio_runtime_store::durable_seq::AGENT_VIEW_MARK,
        id,
    )?;
    let team_counts = crate::sync_entity_context::team_counts_text(transaction, id)?;
    SyncPolicyQueries::mark_agent_projection(transaction, id, deleted, team_counts.as_deref())?;
    Ok(())
}

/// The documents of one agent at read time: `agentSummary`, `agentDetail`, the legacy
/// `agent` DTO and `retainedWorktree`, each a value or None for a delete. None when the
/// agent row is gone.
pub(crate) struct AgentViewDocuments {
    pub summary: Option<RawValue>,
    pub detail: Option<RawValue>,
    pub agent: Option<RawValue>,
    pub retained_worktree: Option<RawValue>,
}

pub(crate) fn agent_view_documents(
    connection: &Connection,
    id: &str,
    project_server_id: Option<&str>,
    include_summary: bool,
    operational: Option<&crate::agent_operational::Context>,
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
            let legacy_vm_workspace = crate::workspace_ready::legacy_vm_record(&record);
            let context = crate::sync_entity_context::agent_view_context(
                connection,
                &record,
                project_server_id,
            )?;
            let retained = crate::sync_views::retained_worktree_from_record(&record)
                .map(|value| studio_json::to_raw_value(&value))
                .transpose()?;
            if record.is_deleted() {
                return Ok(Some(AgentViewDocuments {
                    summary: None,
                    detail: None,
                    agent: None,
                    retained_worktree: retained,
                }));
            }
            let Some(mut full) = crate::sync_views::agent_entity_from_record(record, &context)
            else {
                return Ok(Some(AgentViewDocuments {
                    summary: None,
                    detail: None,
                    agent: None,
                    retained_worktree: retained,
                }));
            };
            let summary = if include_summary {
                let mut summary = crate::agent_operational::summary(&full, operational)?;
                summary.legacy_vm_workspace = Some(legacy_vm_workspace);
                if summary.project_server_id.is_none() {
                    summary.project_server_id = project_server_id.map(str::to_owned);
                }
                Some(studio_json::to_raw_value(&summary)?)
            } else {
                None
            };
            let detail = studio_runtime_schema::generated::AgentDetailEntityDto::from(&full);
            full.team_counts = None;
            full.has_question = None;
            full.has_approval = None;
            full.has_background_work = None;
            return Ok(Some(AgentViewDocuments {
                summary,
                detail: Some(studio_json::to_raw_value(&detail)?),
                agent: Some(studio_json::to_raw_value(&full)?),
                retained_worktree: retained,
            }));
        }
        // Repair-only exception: arbitrary corrupt records require the tolerant Value fallback.
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
            retained_worktree: retained_worktree
                .map(|value| studio_json::to_raw_value(&value))
                .transpose()?,
        }));
    };
    let mut summary = if include_summary {
        project_persisted_record("agentSummary", &view)
    } else {
        None
    };
    if let Some(summary) = summary.as_mut() {
        if summary["projectServerId"].is_null() {
            summary["projectServerId"] =
                serde::Serialize::serialize(&project_server_id, studio_json::value::Serializer)?;
        }
        crate::agent_operational::decorate(summary, operational)?;
    }
    let detail = project_persisted_record("agentDetail", &view);
    let mut agent = view;
    if let Some(fields) = agent.as_object_mut() {
        fields.remove("teamCounts");
        for field in crate::sync_views::AGENT_SUMMARY_ATTENTION_FIELDS {
            fields.remove(field);
        }
    }
    Ok(Some(AgentViewDocuments {
        summary: summary
            .map(|value| studio_json::to_raw_value(&value))
            .transpose()?,
        detail: detail
            .map(|value| studio_json::to_raw_value(&value))
            .transpose()?,
        agent: Some(studio_json::to_raw_value(&agent)?),
        retained_worktree: retained_worktree
            .map(|value| studio_json::to_raw_value(&value))
            .transpose()?,
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
    if SyncEntityMeta::matches(transaction, WORK_DETAILS_SEED_MARKER, "1")? {
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
    SyncEntityMeta::put(transaction, WORK_DETAILS_SEED_MARKER, "1")?;
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
    let exists = SyncPolicyQueries::entity_is_live(transaction, "workspace", "current")?;
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
    agent_volatile::merged_rows(
        connection,
        live_agent_record_rows(connection)?
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
    let live = SyncPolicyQueries::entity_is_live(transaction, collection, id)?;
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
    studio_runtime_store::clock_schema::TaskWindowState {
        id: task.id().as_ref().to_owned(),
        agent: task.agent().to_owned(),
        created: task.created_number().number().as_f64(),
        running: i64::from(
            task.status() == studio_runtime_schema::task_record::TaskStatus::Running,
        ),
    }
    .upsert(transaction)?;
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
    let row = SyncPolicyQueries::task_window_prior(transaction, id)?;
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
        let migrated = SyncEntityMeta::value(transaction, "task_window_migrated")?;
        if migrated.as_deref() != Some("1") {
            SyncEntityMeta::put(transaction, "task_window_migrated", "1")?;
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
    let migration_complete: bool =
        SyncEntityMeta::matches(transaction, "task_window_migrated", "1")?;
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
        let unchanged_finished_position = previous_finished
            && !task_running
            && previous.is_some_and(|previous| {
                previous.get("created").and_then(Value::as_f64)
                    == task.created_number().number().as_f64()
            });
        if unchanged_finished_position
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
            let evicted = SyncPolicyQueries::task_window_eviction_candidate(
                transaction,
                TASK_WINDOW_HISTORY_ROWS as i64,
            )?;
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
        let mut statement = transaction.prepare(&format!("SELECT id,record FROM runtime_monitors WHERE {} AND json_extract(record,'$.status') IN ({placeholders})", studio_runtime_store::monitor_unheld_sql("runtime_monitors.id")))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(MONITOR_ACTIVE_STATUSES), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (id, raw) in rows {
            if let Some(record) = decode_or_hold_monitor(transaction, &id, &raw)? {
                kept.insert(id, record);
            }
        }
        let mut terminals = Vec::new();
        for status in MONITOR_TERMINAL_STATUSES {
            let mut statement = transaction.prepare(&format!("SELECT id,record FROM runtime_monitors WHERE {} AND json_extract(record,'$.status')=?1 ORDER BY json_extract(record,'$.created') DESC LIMIT ?2", studio_runtime_store::monitor_unheld_sql("runtime_monitors.id")))?;
            let rows = statement
                .query_map(
                    rusqlite::params![status, MONITOR_TERMINAL_WINDOW as i64],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for (id, raw) in rows {
                if let Some(record) = decode_or_hold_monitor(transaction, &id, &raw)? {
                    terminals.push(record);
                }
            }
        }
        terminals.sort_by(compare_created_desc);
        for record in terminals.into_iter().take(MONITOR_FINAL_WINDOW) {
            kept.insert(record.id.clone(), record);
        }
    } else {
        let mut statement = transaction.prepare(&format!(
            "SELECT id,record FROM runtime_monitors WHERE {} ORDER BY rowid",
            studio_runtime_store::monitor_unheld_sql("runtime_monitors.id")
        ))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let records = rows
            .into_iter()
            .map(|(id, raw)| decode_or_hold_monitor(transaction, &id, &raw))
            .collect::<StoreResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
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

fn maintain_event_window(
    transaction: &Transaction<'_>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    if !table_exists(transaction, "runtime_events")? {
        return Ok(());
    }
    // The generated runtime_events triggers use the same marker fast path as
    // Python's sync_event_window(full=False). Avoid reprojecting the entire
    // window after a trigger has already recorded its current high sequence.
    let changed = SyncPolicyQueries::collection_sequence(transaction, "event")?;
    let marker = SyncEntityMeta::value(transaction, "event_window_seq")?;
    if marker
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
        == Some(changed)
    {
        return Ok(());
    }
    #[derive(serde::Serialize)]
    struct EventProjection {
        id: String,
        agent: Option<String>,
        kind: Option<String>,
        status: Option<String>,
        created: Option<f64>,
        error: Option<String>,
    }
    let mut statement = transaction.prepare(
        &format!("SELECT id,agent,kind,status,created,error FROM runtime_events ORDER BY created DESC,id LIMIT {EVENT_WINDOW_ROWS}"),
    )?;
    let events = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let bound = |value: Option<String>, field| {
                value.map(|text| crate::sync_views::bound_sync_text(&text, field))
            };
            Ok((
                id.clone(),
                EventProjection {
                    id: crate::sync_views::bound_sync_text(&id, "id"),
                    agent: bound(row.get(1)?, "agent"),
                    kind: bound(row.get(2)?, "kind"),
                    status: bound(row.get(3)?, "status"),
                    created: row.get(4)?,
                    error: bound(row.get(5)?, "error"),
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut kept = BTreeSet::new();
    for (id, record) in events {
        if !id.is_empty() {
            kept.insert(id.clone());
            SyncDocumentsRepository::put_entity(transaction, "event", &id, record, false)?;
        }
    }
    retire_unkept(
        transaction,
        "event",
        &kept,
        Some("event_window_seq"),
        validate,
    )
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

fn decode_or_hold_monitor(
    tx: &Transaction<'_>,
    id: &str,
    raw: &str,
) -> StoreResult<Option<studio_runtime_store::MonitorRecord>> {
    let location = if let Ok(record) = decode_monitor(id, raw) {
        // Use the publication owner's full validation before selecting a row,
        // including retained matcher state that serde alone cannot validate.
        if project_monitor_record(&record).is_ok() {
            return Ok(Some(record));
        }
        crate::monitor_matches::validate(&record)
            .err()
            .unwrap_or("id")
    } else {
        "/record"
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StoreError::RecordContract("invalid clock during monitor quarantine"))?
        .as_secs_f64();
    crate::monitor_quarantine::hold_in(tx, id, location, "monitor_window", now)?;
    Ok(None)
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
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
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
    let mut kept = BTreeSet::new();
    for (id, record) in current {
        kept.insert(id.clone());
        SyncDocumentsRepository::put_entity(transaction, "peerTeam", &id, record, false)?;
    }
    retire_unkept(transaction, "peerTeam", &kept, None, validate)
}

fn retire_unkept(
    transaction: &Transaction<'_>,
    collection: &str,
    kept: &BTreeSet<String>,
    marker: Option<&str>,
    validate: &impl Fn(&str, &str, &Value, bool) -> bool,
) -> StoreResult<()> {
    let stale = SyncPolicyQueries::live_entity_ids(transaction, collection)?;
    for id in stale {
        if !kept.contains(&id) {
            put_projection(transaction, collection, &id, json!({}), true, validate)?;
        }
    }
    if let Some(marker) = marker {
        let sequence = SyncPolicyQueries::collection_sequence(transaction, collection)?;
        let sequence = sequence.to_string();
        let current = SyncEntityMeta::value(transaction, marker)?;
        if current.as_deref() != Some(sequence.as_str()) {
            SyncEntityMeta::put(transaction, marker, &sequence)?;
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
    #[tokio::test]
    async fn f1_n_canonical_catalog_has_no_source_table() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        let reader = store.test_reader().unwrap();
        assert!(
            !studio_runtime_store::system_reads::exists(&reader, "sync_entities").unwrap(),
            "fresh reader recreates sync_entities"
        );
        drop(reader);
        store
            .transaction(|tx| {
                assert!(
                    !studio_runtime_store::system_reads::exists(tx, "sync_entities")?,
                    "writer recreates sync_entities"
                );
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn f1_n_canonical_seed_completeness_checks_queue_and_event_sequence() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                for marker in [
                    SYNC_SEEDED_MARKER,
                    SUMMARY_ENTITIES_SEED_MARKER,
                    CONVERSATION_DETAILS_SEED_MARKER,
                    AGENT_PROJECTIONS_SEED_MARKER,
                    WORK_DETAILS_SEED_MARKER,
                    "task_window_migrated",
                ] {
                    SyncEntityMeta::put(tx, marker, "1")?;
                }
                SyncEntityMeta::put(tx, AGENT_ORGANIZATION_FIELDS_MARKER, "3")?;
                SyncEntityMeta::put(tx, "event_window_seq", "0")?;
                assert!(has_complete_entity_seed(tx)?);
                SyncEntityMeta::put(tx, "event_window_seq", "1")?;
                assert!(!has_complete_entity_seed(tx)?);
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn f1_n_canonical_workspace_refresh_preserves_envelope() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                seed_workspace_projection(tx, "before", |_, _, _, _| true)?;
                let rooms = crate::sync_entity_context::RoomDirectory::load(tx)?;
                maintain_entity_projections_with_server_id(
                    tx,
                    "after",
                    None,
                    &rooms,
                    |_, _, _, _| true,
                )?;
                let raw = SyncPolicyQueries::live_entity_payload(tx, "workspace", "current")?
                    .flatten()
                    .unwrap();
                let payload: Value = studio_json::from_str(&raw)?;
                assert_eq!(payload["value"]["stateDir"], "after");
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn f1_n_canonical_workspace_seed_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                seed_workspace_projection(tx, "first", |_, _, _, _| true)?;
                let high = SyncDocumentsRepository::entity_sequence(tx)?;
                seed_workspace_projection(tx, "second", |_, _, _, _| {
                    panic!("live row must skip validation")
                })?;
                assert_eq!(SyncDocumentsRepository::entity_sequence(tx)?, high);
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn f1_n_canonical_retirement_and_collection_marker() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(tx, "future", "keep", json!({}), false)?;
                SyncDocumentsRepository::put_entity(tx, "future", "stale", json!({}), false)?;
                retire_if_live(tx, "future", "missing", &|_, _, _, _| panic!("missing row"))?;
                retire_if_live(tx, "future", "stale", &|_, _, _, deleted| deleted)?;
                let high = SyncDocumentsRepository::entity_sequence(tx)?;
                retire_if_live(tx, "future", "stale", &|_, _, _, _| panic!("tombstone"))?;
                retire_unkept(
                    tx,
                    "future",
                    &BTreeSet::from(["keep".into()]),
                    Some("n_marker"),
                    &|_, _, _, _| panic!("kept row"),
                )?;
                assert_eq!(
                    SyncEntityMeta::value(tx, "n_marker")?,
                    Some(high.to_string())
                );
                assert_eq!(SyncDocumentsRepository::entity_sequence(tx)?, high);
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn f1_n_canonical_event_marker_fast_path() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                SyncEntityMeta::put(tx, "event_window_seq", "0")?;
                maintain_event_window(tx, &|_, _, _, _| panic!("current event window"))?;
                assert_eq!(
                    SyncEntityMeta::value(tx, "event_window_seq")?,
                    Some("0".into())
                );
                Ok(())
            })
            .await
            .unwrap();
    }

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
                SyncDocumentsRepository::put_entity(tx, "project", "bad", json!({"id":"bad","name":"last known"}), false)?;
                SyncDocumentsRepository::put_entity(tx, "peerTeam", "team", json!({"id":"team","name":"last known","projectPath":"/bad","members":["a","b"]}), false)?;
                Ok(())
            })
            .await
            .unwrap();
        store
            .transaction(move |tx| {
                let rooms = crate::sync_entity_context::RoomDirectory::load(tx)?;
                maintain_entity_projections_with_server_id(tx, "", None, &rooms, |_, _, _, _| {
                    true
                })?;
                let count: i64 = i64::from(
                    studio_runtime_store::ProjectionPublicationQueries::entity_version(
                        tx, "project", "good",
                    )?
                    .is_some_and(|(_, deleted)| !deleted),
                );
                assert_eq!(count, 1);
                for (collection, id) in [("project", "bad"), ("peerTeam", "team")] {
                    let payload: String =
                        studio_runtime_store::ProjectionPublicationQueries::raw_payload(
                            tx, collection, id, true,
                        )?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                    let payload: Value = studio_json::from_str(&payload)?;
                    assert_eq!(payload["value"]["name"], "last known");
                }
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn event_window_publishes_bounded_typed_event() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store.transaction_with(|tx| {
            let error = "é".repeat(2_100);
            tx.execute("INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch,error) VALUES('event','agent','user','text','pending',1,1,?1)", [&error])?;
            studio_runtime_store::test_fixtures::canvas::set_meta(tx, "event_window_seq", None)?;
            maintain_event_window(tx, &|_, _, _, _| true)?;
            let payload: String = studio_runtime_store::ProjectionPublicationQueries::raw_payload(tx, "event", "event", false).and_then(|row| row.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into()))?;
            let actual: Value = studio_json::from_str(&payload)?;
            assert_eq!(actual["value"], json!({"id":"event","agent":"agent","kind":"user","status":"pending","created":1.0,"error":"é".repeat(2_000)}));
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
                let payload: String = studio_runtime_store::ProjectionPublicationQueries::raw_payload(tx, "peerTeam", "team", false).and_then(|row| row.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into()))?;
                let value: Value = studio_json::from_str(&payload)?;
                assert_eq!(value["value"]["revision"], expected);
                assert_eq!(value["value"]["members"], json!(["a", "b"]));
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

    #[test]
    fn event_window_triggers_use_the_event_window_owner() {
        let sql = studio_runtime_store::test_fixtures::canvas::event_publication_trigger_sql();
        let limits = sql.match_indices("LIMIT ").count();
        assert!(limits > 0, "event window triggers declare a bound");
        assert_eq!(
            sql.matches(&format!("LIMIT {EVENT_WINDOW_ROWS})")).count(),
            limits,
            "every sync_schema.sql LIMIT is EVENT_WINDOW_ROWS"
        );
    }

    use super::*;
    use rusqlite::Connection;
    use rusqlite::trace::{TraceEvent, TraceEventCodes};
    use std::cell::RefCell;
    use studio_runtime_store::ProjectsRepository;

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
        let entries = studio_runtime_store::test_fixtures::canvas::changes(connection)
            .expect("window changes")
            .into_iter()
            .filter(|row| row.source_label == window)
            .map(|row| (row.durable_id, row.deleted != 0))
            .collect::<Vec<_>>();
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
                        repository::put(tx, &terminal)?;
                    }
                    maintain_monitor_window(tx, &|_, _, _, _| true)?;
                    let members = studio_runtime_store::durable_seq::window_members(tx, studio_runtime_store::durable_seq::MONITOR_WINDOW)?;
                    let retired = (0..=MONITOR_TERMINAL_WINDOW)
                        .map(|index| format!("terminal-{index}"))
                        .find(|id| !members.contains(id))
                        .expect("an unretained terminal");
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
                let window = || -> StoreResult<Vec<(String, i64, i64)>> {
                    Ok(studio_runtime_store::EntityScopeQueries::window_changes(
                        tx,
                        studio_runtime_store::durable_seq::MONITOR_WINDOW,
                        "runtime_monitors",
                        (0, i64::MAX),
                        0,
                        Some(&output_id),
                        2,
                    )?)
                };
                let before_window = window()?;
                assert_eq!(before_window.len(), 1);
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
                let published_window = window()?;
                assert_eq!(published_window.len(), 1, "one member has one publication");
                assert_eq!(published_window[0].0, output_id);
                assert_eq!(published_window[0].2, 0);
                assert!(
                    published_window[0].1 > before_window[0].1,
                    "changed content advances the client publication sequence"
                );
                assert!(
                    !statements.iter().any(|sql| sql
                        .starts_with("SELECT id,record FROM runtime_monitors")
                        && !sql.contains("json_extract(record,'$.agent')")),
                    "output must not run the window selector: {statements:?}"
                );
                let projected = monitor_entity(tx, &output_id)?.expect("updated projection");
                assert_eq!(projected["tail"], "fragment");
                assert_eq!(projected["bytes"], b"fragment".len());
                let (unchanged, actor) = monitor_runtime::snapshot(tx, &output_id)?;
                monitor_runtime::apply(tx, &unchanged, &actor, &unchanged.decision(), 2.0)?;
                assert_eq!(
                    window()?,
                    published_window,
                    "unchanged content retains the member and sequence without duplication"
                );
                assert_eq!(
                    monitor_entity(tx, &output_id)?,
                    Some(projected),
                    "unchanged content retains the publication"
                );
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
                let members = studio_runtime_store::durable_seq::window_members(
                    tx,
                    studio_runtime_store::durable_seq::MONITOR_WINDOW,
                )?;
                let excluded = (0..=MONITOR_TERMINAL_WINDOW)
                    .map(|index| format!("terminal-{index}"))
                    .find(|id| !members.contains(id))
                    .expect("an unretained terminal");
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
        studio_runtime_store::test_fixtures::canvas::install_canonical(&connection)
            .expect("canonical storage");
        connection.execute_batch(<studio_runtime_store::receipt_tables::ErrorIncidentRow as studio_db_api::Table>::DDL).unwrap();
        // Keep a record-only legacy source table so non-object corruption reaches the reader.
        // The current provider column rejects that corruption at write time.
        connection
            .execute_batch(
                "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY, record TEXT NOT NULL, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
                 CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
                 
                 
                   
                 
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
                            json!({"id":id,"agent":agent,"epoch":0,"status":status,"created":created,"requestId":format!("request-{id}"),"activityAt":created,"activityGeneration":0,"bytes":0,"command":"","cwd":std::env::temp_dir(),"interactive":false,"livenessCommand":"","log":std::env::temp_dir().join("watch.log"),"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit"})
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

        let count = studio_runtime_store::durable_seq::window_members(
            &transaction,
            studio_runtime_store::durable_seq::MONITOR_WINDOW,
        )
        .expect("live projected monitors")
        .len() as i64;
        assert_eq!(count, 205);
        for id in ["deleted-owner", "missing-owner"] {
            let exists: bool = studio_runtime_store::durable_seq::in_window(
                &transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                id,
            )
            .expect("owner-filtered projection query");
            assert!(!exists, "inactive or missing owner must not project {id}");
        }
        for id in ["active-0", "active-104", "invalid-owner"] {
            let exists: bool = studio_runtime_store::durable_seq::in_window(
                &transaction,
                studio_runtime_store::durable_seq::MONITOR_WINDOW,
                id,
            )
            .expect("retained monitor query");
            assert!(exists, "active or malformed source must retain {id}");
        }
        let actual_terminal_ids = studio_runtime_store::durable_seq::window_members(
            &transaction,
            studio_runtime_store::durable_seq::MONITOR_WINDOW,
        )
        .expect("projected terminal ids")
        .into_iter()
        .filter(|id| id.starts_with("terminal-"))
        .collect::<BTreeSet<_>>();
        let expected_active_terminal_ids = expected_terminal_ids
            .into_iter()
            .filter(|id| id != "terminal-50")
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_terminal_ids, expected_active_terminal_ids);
        let stale_deleted: bool =
            studio_runtime_store::test_fixtures::canvas::changes(&transaction)
                .map(|rows| {
                    rows.into_iter()
                        .find(|row| {
                            row.source_label == "monitor_window" && row.durable_id == "stale"
                        })
                        .is_none_or(|row| row.deleted == 1)
                })
                .expect("stale canonical tombstone");
        assert!(stale_deleted, "unselected canonical rows still tombstone");
        let invalid_deleted: bool =
            studio_runtime_store::test_fixtures::canvas::changes(&transaction)
                .map(|rows| {
                    rows.into_iter()
                        .find(|row| {
                            row.source_label == "monitor_window"
                                && row.durable_id == "invalid-monitor"
                        })
                        .is_none_or(|row| row.deleted == 1)
                })
                .expect("unselected monitor tombstone");
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
        maintain_monitor_window(&transaction, &|_, _, _, _| true).expect("malformed rows are held");
        assert!(
            studio_runtime_store::MonitorsRepository::is_held(&transaction, "invalid-monitor")
                .unwrap()
        );
    }

    #[test]
    fn indexed_monitor_window_matches_python_tie_boundary_and_filters_owner_after_selection() {
        let mut connection = Connection::open_in_memory().expect("database");
        studio_runtime_store::test_fixtures::canvas::install_canonical(&connection)
            .expect("canonical storage");
        connection.execute_batch(<studio_runtime_store::receipt_tables::ErrorIncidentRow as studio_db_api::Table>::DDL).unwrap();
        // Keep a record-only legacy source table so non-object corruption reaches the reader.
        // The current provider column rejects that corruption at write time.
        connection
            .execute_batch(
                "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY,record TEXT NOT NULL, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
                 CREATE INDEX runtime_monitor_status ON runtime_monitors(
                     json_extract(record,'$.status'),json_extract(record,'$.created'));
                 CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
                 
                 
                   
                 
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
                        json!({"id":id,"agent":agent,"epoch":0,"status":"completed","created":10,"requestId":format!("request-{id}"),"activityAt":10,"activityGeneration":0,"bytes":0,"command":"","cwd":std::env::temp_dir(),"interactive":false,"livenessCommand":"","log":std::env::temp_dir().join("watch.log"),"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit"})
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
        let actual_ids = studio_runtime_store::durable_seq::window_members(
            &transaction,
            studio_runtime_store::durable_seq::MONITOR_WINDOW,
        )
        .expect("projected terminal ids")
        .into_iter()
        .collect::<BTreeSet<_>>();
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
        let statements = store
            .transaction(move |transaction| {
                TRACED_SQL.with_borrow_mut(Vec::clear);
                transaction.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(trace_sql));
                let rooms = crate::sync_entity_context::RoomDirectory::load(transaction)?;
                maintain_entity_projections_with_server_id(
                    transaction,
                    &state_dir,
                    None,
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
                    &rooms,
                    |_, _, _, _| true,
                )?;
                let deleted: bool = studio_runtime_store::test_fixtures::canvas::changes(transaction)?.into_iter().find(|row| row.source_label == "task_window" && row.durable_id == "task").is_none_or(|row| row.deleted != 0);
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
    async fn task_window_refresh_repairs_legacy_position() {
        let (_root, store) = task_window_store().await;
        store.transaction(|tx| {
            tx.execute("INSERT INTO runtime_tasks(id,record) VALUES('legacy-task',?1)", [r#"{"id":"legacy-task","agent":"b","status":"running","created":12,"itemId":"legacy-task","turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"}"#])?;
            studio_runtime_store::durable_seq::join_window(tx, studio_runtime_store::durable_seq::TASK_WINDOW, "legacy-task")?;
            studio_runtime_store::clock_schema::TaskWindowState { id:"legacy-task".into(), agent:"b".into(), created:Some(0.0), running:1 }.upsert(tx)?;
            maintain_task_window(tx, &|_, _, _, _| true)?;
            let row = studio_runtime_store::SyncPolicyQueries::task_window_prior(tx, "legacy-task")?.expect("repaired task window position");
            assert_eq!(row, ("b".to_owned(), Some(12.0), true));
            assert!(studio_runtime_store::durable_seq::in_window(tx, studio_runtime_store::durable_seq::TASK_WINDOW, "legacy-task")?);
            Ok(())
        }).await.expect("refresh repairs legacy window position");
    }

    #[tokio::test]
    async fn summary_publication_preserves_pending_bounded_task_retirement() {
        let (_old_dir, old) = task_window_store().await;
        let (_new_dir, new) = task_window_store().await;
        for store in [&old, &new] {
            store.transaction(|tx| {
                for (prefix, agent, status, count, offset) in [
                    ("00-running", "a", "running", 101, 300),
                    ("10-finished", "a", "completed", 100, 200),
                    ("20-older", "b", "completed", 100, 0),
                ] {
                    for index in 0..count {
                        let id = format!("{prefix}-{index:03}");
                        let task = json!({"id":id,"agent":agent,"status":status,"created":offset+index,
                            "itemId":id,"turnId":"turn","type":"commandExecution","kind":"command","name":"commandExecution"});
                        tx.execute("INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",
                            rusqlite::params![id, task.to_string()])?;
                    }
                }
                maintain_task_window(tx, &|_, _, _, _| true)?;
                tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.deletedAt',1) WHERE id='a'", [])?;
                maintain_task_window(tx, &|_, _, _, _| true)?;
                let mut finished = 0;
                for id in studio_runtime_store::durable_seq::window_members(tx, studio_runtime_store::durable_seq::TASK_WINDOW)? {
                    if studio_runtime_store::SyncPolicyQueries::task_window_prior(tx, &id)?.is_some_and(|(_, _, running)| !running) { finished += 1; }
                }
                assert_eq!(finished, 200, "bounded retirement leaves finished members behind");
                Ok(())
            }).await.expect("retirement boundary fixture");
        }
        for (store, always_maintain) in [(&old, true), (&new, false)] {
            store.transaction(move |tx| {
                let id = "20-older-000";
                tx.execute("UPDATE runtime_tasks SET record=json_set(record,'$.tail','new output') WHERE id=?1", [id])?;
                if always_maintain {
                    let raw: String = tx.query_row("SELECT record FROM runtime_tasks WHERE id=?1", [id], |row| row.get(0))?;
                    let task = studio_json::from_str(&raw)?;
                    let prior = task_window_prior(tx, id)?;
                    maintain_task_window_after_write(tx, prior.as_ref(), &task, &|_, _, _, _| true)?;
                } else {
                    publish_summary_changes(tx, &studio_runtime_store::PendingEntities {
                        tasks: BTreeSet::from([id.to_owned()]), ..Default::default()
                    })?;
                }
                Ok(())
            }).await.expect("stable member output write");
        }
        let snapshot = |store: &RuntimeStore| {
            store
                .test_read_transaction(|tx| {
                    Ok((
                        window_rows(tx, "task"),
                        studio_runtime_store::durable_seq::high_and_floor(tx)?,
                    ))
                })
                .expect("window snapshot")
        };
        assert_eq!(
            snapshot(&new),
            snapshot(&old),
            "pending retirement must keep eviction and its sequence"
        );
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
                    let agent = ["a", "b"][(index % 2) as usize];
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
    fn draft_identity_matches_row_key_and_allows_extension_members() {
        let validate_draft = |id: &str, payload: &Value| {
            studio_runtime_schema::drafts::DraftVersion::from_stored_value(id, payload.clone())
        };
        let payload = json!({"device":"phone","session":"chat","id":"phone:tab:chat","text":"draft","extra":true});
        assert!(validate_draft("phone:tab:chat", &payload).is_ok());
        assert!(validate_draft("phone:chat", &json!({})).is_err());
        assert!(validate_draft("phone:chat", &Value::Null).is_err());
        assert!(validate_draft("phone:chat", &json!({"device":"other","session":"chat"})).is_err());
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

    #[tokio::test]
    async fn legacy_project_projection_drops_vm_fields() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                let project = json!({"recordKind":"live","id":"p","path":"/p","peerTeams":[]});
                ProjectsRepository::put_project(tx, &project)?;
                let mut prior = project;
                prior["layrReadiness"] = json!({"unexpected":"legacy"});
                prior["layrSync"] = json!({"unexpected":"legacy"});
                prior["vault"] = json!({"unexpected":"legacy"});
                SyncDocumentsRepository::put_entity(tx, "project", "p", prior, false)?;
                let rooms = crate::sync_entity_context::RoomDirectory::load(tx)?;
                maintain_entity_projections_with_server_id(tx, "", None, &rooms, |_, _, _, _| {
                    true
                })?;
                let payload: String =
                    studio_runtime_store::ProjectionPublicationQueries::raw_payload(
                        tx, "project", "p", false,
                    )
                    .and_then(|row| {
                        row.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
                    })?;
                let payload: Value = studio_json::from_str(&payload)?;
                assert_eq!(payload["value"]["id"], "p");
                for field in ["layrReadiness", "layrSync", "vault"] {
                    assert!(payload["value"].get(field).is_none(), "{field}");
                }
                Ok(())
            })
            .await
            .unwrap();
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
                let refusal = crate::work_write::current_submission_refusal(transaction, &item)
                    .expect("submission refusal");
                let mut out = BTreeMap::new();
                for (collection, view) in work_views(&item, &record, refusal, &BTreeMap::new())? {
                    let id = match &view {
                        WorkView::Entry(entry) => entry.id().to_owned(),
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
