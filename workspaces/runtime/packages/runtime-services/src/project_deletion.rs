//! Atomic deletion of a registered project lifetime. The lifecycle owner stages
//! every stop; this owner selects project membership and retires its navigation.
use crate::{
    agent_lifecycle_runtime::{self, LifecyclePorts},
    agent_management::{Action, ManagementError},
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use studio_json::{Value, json};
use studio_runtime_store::{
    AgentRecord, ProjectsRepository, RuntimeStore, StoreError, SyncDocumentsRepository, Transaction,
};
use thiserror::Error;

/// Internal command supplied by the generated HTTP request. No caller-supplied
/// agent lists or paths participate in membership selection.
pub struct Request {
    pub id: String,
    pub request_id: String,
    pub expected_workspace_id: String,
    pub expected_project_seq: i64,
}
#[derive(Serialize, Deserialize)]
pub struct DeleteReply {
    id: String,
    request_id: String,
    outcome: DeleteOutcome,
    #[serde(rename = "deletedAgentIds")]
    deleted_agent_ids: BTreeSet<String>,
    #[serde(rename = "deletedRoomIds")]
    deleted_room_ids: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DeleteOutcome {
    Deleted,
}
#[derive(Debug, Error)]
pub enum Error {
    #[error("The project no longer exists")]
    NotFound,
    #[error("The project belongs to a different workspace")]
    Workspace,
    #[error("The project changed; confirm deletion again using current state")]
    Sequence,
    #[error("Project deletion requires one resolved owning server")]
    Remote,
    #[error("Project {project} conflicts with project {other} at the same location")]
    ProjectLocationConflict { project: String, other: String },
    #[error("Agent {agent} has incomplete or invalid project binding")]
    InvalidBinding { agent: String },
    #[error("Agent {agent} has ambiguous membership in project {project}")]
    AmbiguousAgent { agent: String, project: String },
    #[error("Agent {agent} has conflicting membership in projects {project} and {other}")]
    AgentProjectConflict {
        agent: String,
        project: String,
        other: String,
    },
    #[error("Agent {agent} has invalid ancestry: {reason}")]
    InvalidAncestry { agent: String, reason: &'static str },
    #[error("Project {project} has an invalid location")]
    InvalidLocation { project: String },
    #[error("Sidebar order for project {project} has an invalid revision")]
    InvalidSidebar { project: String },
    #[error(transparent)]
    Lifecycle(#[from] ManagementError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Request(#[from] studio_runtime_store::request_results::RequestResultError),
}
impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

pub async fn execute(
    store: Arc<RuntimeStore>,
    request: Request,
    now: f64,
    ports: Arc<dyn LifecyclePorts>,
) -> Result<DeleteReply, Error> {
    let adapter = ports.clone();
    let (reply, applied) = store
        .transaction_with(move |tx| delete_in_transaction(tx, &request, now, adapter.as_ref()))
        .await?;
    if applied && let Err(error) = ports.committed().await {
        // A failed wake cannot change the committed receipt. Existing durable
        // lifecycle owners drain the same intents after restart.
        eprintln!("Project deletion committed; effect delivery remains pending: {error}");
    }
    Ok(reply)
}
fn delete_in_transaction(
    tx: &Transaction<'_>,
    request: &Request,
    now: f64,
    ports: &dyn LifecyclePorts,
) -> Result<(DeleteReply, bool), Error> {
    let workspace = SyncDocumentsRepository::workspace_id(tx)?;
    let server = RuntimeStore::transaction_local_server_id(tx)?
        .filter(|s| !s.is_empty())
        .ok_or(Error::Remote)?;
    // The authenticated owner API is the only ingress; paired agents have no
    // project-delete authority. Include actual server/workspace in the receipt.
    let signature = json!([
        "owner-api",
        server,
        workspace,
        request.id,
        request.request_id,
        request.expected_workspace_id,
        request.expected_project_seq
    ])
    .to_string();
    let receipt_id = format!("project-delete:{}", request.request_id);
    let now_ms = (now * 1000.0) as i64;
    if workspace != request.expected_workspace_id {
        return Err(Error::Workspace);
    }
    let authority = [&server[..], &workspace[..], "owner-api"];
    let key =
        crate::event_write::RequestKey::new("project-delete", &authority, &request.request_id)
            .raw(&receipt_id)
            .operation(&["delete"]);
    let admitted =
        match crate::event_write::begin_request::<DeleteReply>(tx, key, &signature, now_ms)? {
            crate::event_write::CallerAdmission::Replay(reply) => return Ok((reply, false)),
            crate::event_write::CallerAdmission::Fresh(admitted) => admitted,
        };
    let (_, raw_project) = ProjectsRepository::get(tx, &request.id)?.ok_or(Error::NotFound)?;
    let project = decode_project(&request.id, &raw_project)?;
    let seq = ProjectsRepository::document_version(tx, &request.id)?
        .filter(|(_, live)| *live)
        .map(|(seq, _)| seq);
    if seq != Some(request.expected_project_seq) {
        return Err(Error::Sequence);
    }
    let (project_ids, paths, overlaps) = project_scope(tx, &project, &server)?;
    let agents = associated_agents(
        ProjectsRepository::agents(tx)?,
        &project_ids,
        &server,
        &paths,
    )?;
    let selected = membership(
        &agents,
        &project.id,
        &project_ids,
        &server,
        &paths,
        &overlaps,
    )?;
    if let Some(other) = overlaps.keys().next() {
        return Err(Error::ProjectLocationConflict {
            project: project.id,
            other: other.clone(),
        });
    }
    refuse_pending_remote(tx, &project_ids, &paths, &selected)?;
    if studio_runtime_store::AgentMoveRepository::retains_agent_ownership(tx, &selected)? {
        return Err(Error::Remote);
    }
    let roots = selected
        .iter()
        .filter(|id| {
            agents[*id]
                .parent_id
                .as_deref()
                .is_none_or(|parent| !selected.contains(parent))
        })
        .cloned()
        .collect::<Vec<_>>();
    // Each selected tree goes through the existing transaction-only lifecycle
    // owner, under this ONE writer. Bound orphans are roots for this purpose.
    for root in roots {
        agent_lifecycle_runtime::execute_in_transaction_for_scope(
            tx,
            &Action::Delete { target: root },
            now,
            ports,
            &selected,
        )?;
    }
    let rooms = retire_rooms(tx, &project_ids, &paths, &selected, now)?;
    for id in &project_ids {
        if ProjectsRepository::get(tx, id)?.is_some() {
            ProjectsRepository::remove(tx, id)?;
            ProjectsRepository::record_destructive_removal(tx, id, &request.request_id, now_ms)?;
        }
        prune_sidebar(tx, id, &paths, &selected)?;
    }
    let reply = DeleteReply {
        id: request.id.clone(),
        request_id: request.request_id.clone(),
        outcome: DeleteOutcome::Deleted,
        deleted_agent_ids: selected,
        deleted_room_ids: rooms,
    };
    crate::event_write::settle_request(tx, &admitted, None, &reply)?;
    Ok((reply, true))
}
// Alias names were not required by historical records. Their identity is
// inspected separately; all other fields use the shared stored record type.
fn decode_project(
    id: &str,
    raw: &Value,
) -> Result<studio_runtime_schema::project_record::ProjectLive, Error> {
    let mut value = raw.clone();
    let fields = value
        .as_object_mut()
        .ok_or_else(|| Error::InvalidLocation {
            project: id.to_owned(),
        })?;
    fields.remove("projectAliases");
    fields.entry("recordKind").or_insert(json!("live"));
    let parsed =
        studio_runtime_schema::project_record::ProjectRecord::deserialize(value).map_err(|_| {
            Error::InvalidLocation {
                project: id.to_owned(),
            }
        })?;
    match parsed {
        studio_runtime_schema::project_record::ProjectRecord::Live(project) if project.id == id => {
            Ok(*project)
        }
        studio_runtime_schema::project_record::ProjectRecord::Live(_)
        | studio_runtime_schema::project_record::ProjectRecord::Removal(_) => {
            Err(Error::InvalidLocation {
                project: id.to_owned(),
            })
        }
    }
}
type ProjectScope = (
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeMap<String, BTreeSet<String>>,
);
fn project_scope(
    tx: &Transaction<'_>,
    project: &studio_runtime_schema::project_record::ProjectLive,
    server: &str,
) -> Result<ProjectScope, Error> {
    let mut ids = BTreeSet::from([project.id.clone()]);
    let records = ProjectsRepository::owned(tx)?;
    let mut paths = BTreeSet::new();
    loop {
        let count = ids.len();
        for (id, record) in &records {
            let linked = ids.contains(id)
                || record
                    .get("projectAliases")
                    .and_then(Value::as_array)
                    .is_some_and(|aliases| {
                        aliases.iter().any(|alias| {
                            alias.get("serverId").and_then(Value::as_str) == Some(server)
                                && alias
                                    .get("projectId")
                                    .and_then(Value::as_str)
                                    .is_some_and(|id| ids.contains(id))
                        })
                    });
            if !linked {
                continue;
            }
            let typed = decode_project(id, record)?;
            paths.extend(project_paths(&typed, server)?);
            ids.insert(id.clone());
            if let Some(aliases) = record
                .get("projectAliases")
                .filter(|value| !value.is_null())
            {
                let aliases = aliases.as_array().ok_or_else(|| Error::InvalidLocation {
                    project: id.clone(),
                })?;
                for alias in aliases {
                    let alias_server = alias.get("serverId").and_then(Value::as_str);
                    let alias_id = alias
                        .get("projectId")
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty());
                    if alias_server != Some(server) || alias_id.is_none() {
                        return Err(Error::Remote);
                    }
                    ids.insert(alias_id.expect("checked alias").to_owned());
                }
            }
            let record = typed;
            if let Some(locations) = record.locations.value() {
                ids.extend(locations.iter().map(|location| location.project_id.clone()));
            }
        }
        if count == ids.len() {
            break;
        }
    }
    let mut overlaps = BTreeMap::new();
    for (id, record) in &records {
        if !ids.contains(id) {
            let shared: BTreeSet<_> = record
                .get("locations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|location| location.get("serverId").and_then(Value::as_str) == Some(server))
                .filter_map(|location| location.get("path").and_then(Value::as_str))
                .filter(|path| paths.contains(*path))
                .map(str::to_owned)
                .collect();
            if !shared.is_empty() {
                overlaps.insert(id.clone(), shared);
            }
        }
    }
    Ok((ids, paths, overlaps))
}
fn project_paths(
    project: &studio_runtime_schema::project_record::ProjectLive,
    server: &str,
) -> Result<BTreeSet<String>, Error> {
    if project
        .location_intents
        .value()
        .is_some_and(|intents| !intents.is_empty())
        || project.home_server_id.value().map(String::as_str) != Some(server)
    {
        return Err(Error::Remote);
    }
    let locations = project.locations.value().ok_or(Error::Remote)?;
    let mut paths = BTreeSet::new();
    for location in locations {
        if location.server_id != server {
            return Err(Error::Remote);
        }
        if location.path.is_empty() {
            return Err(Error::InvalidLocation {
                project: project.id.clone(),
            });
        }
        paths.insert(location.path.clone());
    }
    if paths.is_empty() {
        return Err(Error::Remote);
    }
    Ok(paths)
}
fn refuse_pending_remote(
    tx: &Transaction<'_>,
    projects: &BTreeSet<String>,
    paths: &BTreeSet<String>,
    agents: &BTreeSet<String>,
) -> Result<(), Error> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_outbox')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "SELECT request_id FROM runtime_outbox WHERE kind='remote_delivery' AND json_extract(payload,'$.effect')='server_request'",
    )?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for operation in ids {
        let value = studio_runtime_store::ProjectLocationsRepository::outbox(tx, &operation)?
            .ok_or(Error::Remote)?;
        let payload = &value["body"]["payload"];
        if payload
            .get("project")
            .and_then(Value::as_str)
            .is_some_and(|id| projects.contains(id))
            || payload
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| paths.contains(path))
            || payload
                .get("agent")
                .and_then(Value::as_str)
                .is_some_and(|id| agents.contains(id))
        {
            return Err(Error::Remote);
        }
    }
    Ok(())
}
// Tolerant inspection determines relevance before strict record decoding. A
// malformed unrelated record must not prevent deletion of another project.
fn associated_agents(
    records: Vec<(String, Value)>,
    projects: &BTreeSet<String>,
    server: &str,
    paths: &BTreeSet<String>,
) -> Result<BTreeMap<String, AgentRecord>, Error> {
    let records: BTreeMap<_, _> = records.into_iter().collect();
    let mut relevant = BTreeSet::new();
    for (id, value) in &records {
        let project = value
            .get("projectId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let owner = value
            .get("projectServerId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let other =
            matches!((project, owner), (Some(p), Some(s)) if !projects.contains(p) || s != server);
        if project.is_some_and(|p| projects.contains(p)) && !other
            || !other
                && ["hostProjectPath", "cwd"].iter().any(|key| {
                    value
                        .get(*key)
                        .and_then(Value::as_str)
                        .is_some_and(|path| paths.contains(path))
                })
        {
            relevant.insert(id.clone());
        }
    }
    loop {
        let count = relevant.len();
        for (id, value) in &records {
            let parent = value.get("parentId").and_then(Value::as_str);
            let root = value.get("rootId").and_then(Value::as_str);
            if parent.is_some_and(|p| relevant.contains(p))
                || root.is_some_and(|r| relevant.contains(r))
            {
                relevant.insert(id.clone());
            }
            if relevant.contains(id) {
                for relative in [parent, root].into_iter().flatten() {
                    if records.contains_key(relative) {
                        relevant.insert(relative.to_owned());
                    }
                }
            }
        }
        if relevant.len() == count {
            break;
        }
    }
    records
        .into_iter()
        .filter(|(id, _)| relevant.contains(id))
        .map(|(id, value)| {
            AgentRecord::from_value(value)
                .map(|record| (id.clone(), record))
                .map_err(|_| {
                    Error::Store(StoreError::InvalidStoredRecord {
                        object: "runtime_agents",
                        id,
                    })
                })
        })
        .collect()
}
fn binding(agent: &AgentRecord) -> Result<Option<(&str, &str)>, Error> {
    match (
        agent.project_id.as_deref(),
        agent.project_server_id.as_deref(),
    ) {
        (None, None) => Ok(None),
        (Some(project), Some(server)) if !project.is_empty() && !server.is_empty() => {
            Ok(Some((project, server)))
        }
        _ => Err(Error::InvalidBinding {
            agent: agent.id.0.clone(),
        }),
    }
}
fn membership(
    agents: &BTreeMap<String, AgentRecord>,
    project: &str,
    projects: &BTreeSet<String>,
    server: &str,
    paths: &BTreeSet<String>,
    overlaps: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<String>, Error> {
    let mut selected = BTreeSet::new();
    for (id, agent) in agents {
        let path = agent.host_project_path.value().or(agent.cwd.as_ref());
        let owned = match binding(agent) {
            Ok(Some((p, s))) => projects.contains(p) && s == server,
            Ok(None) => {
                if let Some(other) = overlaps.iter().find_map(|(other, shared)| {
                    path.filter(|path| shared.contains(*path)).map(|_| other)
                }) {
                    return Err(Error::AgentProjectConflict {
                        agent: id.clone(),
                        project: project.to_owned(),
                        other: other.clone(),
                    });
                }
                agent.parent_id.is_none() && path.is_some_and(|p| paths.contains(p))
            }
            Err(error) => {
                if agent
                    .project_id
                    .as_ref()
                    .is_some_and(|p| projects.contains(p))
                    || agent.cwd.as_ref().is_some_and(|p| paths.contains(p))
                    || agent
                        .host_project_path
                        .value()
                        .is_some_and(|p| paths.contains(p))
                {
                    return Err(error);
                }
                false
            }
        };
        if owned {
            selected.insert(id.clone());
        }
    }
    loop {
        let count = selected.len();
        for (id, agent) in agents {
            if agent
                .parent_id
                .as_ref()
                .is_some_and(|p| selected.contains(p))
            {
                selected.insert(id.clone());
            }
        }
        if count == selected.len() {
            break;
        }
    }
    for (id, agent) in agents {
        let explicitly_other =
            matches!(binding(agent), Ok(Some((p, s))) if !projects.contains(p) || s != server);
        let associated = selected.contains(agent.root_id.as_ref())
            || (!explicitly_other
                && (agent.parent_id.is_none() || !agents.contains_key(agent.root_id.as_ref()))
                && agent
                    .host_project_path
                    .value()
                    .or(agent.cwd.as_ref())
                    .is_some_and(|p| paths.contains(p)))
            || (!explicitly_other
                && agent
                    .project_id
                    .as_ref()
                    .is_some_and(|p| projects.contains(p)));
        if associated && !selected.contains(id) {
            return Err(Error::AmbiguousAgent {
                agent: id.clone(),
                project: project.to_owned(),
            });
        }
    }
    for id in &selected {
        let agent = &agents[id];
        let invalid = |reason| Error::InvalidAncestry {
            agent: id.clone(),
            reason,
        };
        if agent.id.as_ref() != id || agent.root_id.as_ref().is_empty() {
            return Err(invalid("invalid identity or root"));
        }
        if agents.contains_key(agent.root_id.as_ref()) && !selected.contains(agent.root_id.as_ref())
        {
            return Err(invalid("root belongs to another project"));
        }
        if let Some((p, s)) = binding(agent)?
            && (!projects.contains(p) || s != server)
        {
            return Err(Error::AgentProjectConflict {
                agent: id.clone(),
                project: project.to_owned(),
                other: p.to_owned(),
            });
        }
        if [
            "movedTo",
            "remoteWorker",
            "remoteParent",
            "remoteOrigin",
            "relocation",
            "teleportPending",
        ]
        .iter()
        .any(|key| studio_runtime_store::truthy(agent.extra.get(*key)))
        {
            return Err(Error::Remote);
        }
        for field in ["serverId", "localServerId"] {
            if agent
                .extra
                .get(field)
                .filter(|v| !v.is_null())
                .is_some_and(|v| v.as_str() != Some(server))
            {
                return Err(Error::Remote);
            }
        }
        let mut chain = BTreeSet::new();
        let mut current = id.as_str();
        while let Some(record) = agents.get(current) {
            if !chain.insert(current) {
                return Err(invalid("parent cycle"));
            }
            if current != id && !selected.contains(current) {
                return Err(invalid("parent belongs to another project"));
            }
            let Some(parent) = record.parent_id.as_deref() else {
                break;
            };
            if parent.is_empty() {
                return Err(invalid("empty parent"));
            }
            current = parent;
        }
    }
    Ok(selected)
}
fn retire_rooms(
    tx: &Transaction<'_>,
    projects: &BTreeSet<String>,
    paths: &BTreeSet<String>,
    agents: &BTreeSet<String>,
    now: f64,
) -> Result<BTreeSet<String>, Error> {
    let mut stmt = tx.prepare_cached("SELECT id,record FROM runtime_rooms")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut retired = BTreeSet::new();
    for (id, raw) in rows {
        let mut room: studio_runtime_schema::room_record::RoomRecord =
            studio_json::from_str(&raw).map_err(StoreError::from)?;
        if room.id != id {
            return Err(StoreError::InvalidStoredRecord {
                object: "runtime_rooms",
                id,
            }
            .into());
        }
        let bound = room
            .project_id
            .as_deref()
            .is_some_and(|id| projects.contains(id))
            || room
                .project_path
                .as_ref()
                .map(String::as_str)
                .is_some_and(|p| projects.contains(p) || paths.contains(p));
        let broadcast = room.kind == studio_runtime_schema::room_record::RoomKind::Broadcast
            && room
                .root_id
                .as_ref()
                .map(String::as_str)
                .is_some_and(|root| root != "all" && agents.contains(root));
        if bound || broadcast {
            room.project_deleted_at = Some(
                studio_json::Number::from_f64(now)
                    .ok_or(StoreError::RecordContract("invalid project deletion time"))?,
            );
            studio_runtime_store::Table::new(tx).put(&room)?;
            SyncDocumentsRepository::put_entity(tx, "room", &id, json!({}), true)?;
            retired.insert(id);
        }
    }
    Ok(retired)
}
fn prune_sidebar(
    tx: &Transaction<'_>,
    project: &str,
    paths: &BTreeSet<String>,
    agents: &BTreeSet<String>,
) -> Result<(), Error> {
    let mut order = ProjectsRepository::sidebar_order(tx)?;
    if let Some(groups) = order.get_mut("groups").and_then(Value::as_object_mut) {
        let before = groups.clone();
        groups.retain(|key, _| key != project && !paths.contains(key));
        for items in groups.values_mut().filter_map(Value::as_array_mut) {
            items.retain(|item| !item.as_str().is_some_and(|id| agents.contains(id)));
        }
        if *groups != before {
            let revision = order
                .get("revision")
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::InvalidSidebar {
                    project: project.to_owned(),
                })?;
            order["revision"] = json!(revision.saturating_add(1));
            ProjectsRepository::put_sidebar_order(tx, &order)?;
        }
    }
    Ok(())
}

/// Capture the project entity version before asynchronous creation preparation.
/// Rechecking it in the writer fences a delete/re-register ABA lifetime change.
pub(crate) fn admission_version(
    db: &Connection,
    id: &str,
) -> Result<Option<(i64, bool)>, StoreError> {
    let version = ProjectsRepository::document_version(db, id)?.map(|(seq, live)| (seq, !live));
    version
        .map(|(seq, deleted)| {
            Ok((
                seq,
                deleted && ProjectsRepository::is_destructive_removal(db, id, seq)?,
            ))
        })
        .transpose()
}

#[cfg(test)]
mod membership_tests {
    use super::*;

    #[test]
    fn explicitly_owned_agents_in_another_project_line_are_not_members() {
        let line = "/store/projects/fixture/lines/main";
        let agents = ["first", "second"].into_iter().map(|id| {
            (id.to_owned(), AgentRecord::from_value(json!({"id": id, "rootId": id, "epoch": 1, "status": "paused", "accountKey": "default", "projectId": "/source/project", "projectServerId": "owner", "cwd": line, "hostProjectPath": "/source/project"})).expect("agent"))
        }).collect();
        let selected = membership(
            &agents,
            line,
            &BTreeSet::from([line.to_owned()]),
            "owner",
            &BTreeSet::from([line.to_owned()]),
            &BTreeMap::new(),
        )
        .expect("other project owns both agents");
        assert!(selected.is_empty());
    }
    #[test]
    fn unbound_agent_at_shared_location_names_both_projects() {
        let agent = AgentRecord::from_value(json!({"id":"legacy","rootId":"legacy","epoch":1,"status":"paused","accountKey":"default","cwd":"/shared"})).expect("agent");
        let error = membership(
            &BTreeMap::from([("legacy".into(), agent)]),
            "project-a",
            &BTreeSet::from(["project-a".into()]),
            "owner",
            &BTreeSet::from(["/shared".into()]),
            &BTreeMap::from([("project-b".into(), BTreeSet::from(["/shared".into()]))]),
        )
        .unwrap_err();
        assert!(
            matches!(error, Error::AgentProjectConflict { agent, project, other } if agent == "legacy" && project == "project-a" && other == "project-b")
        );
    }
}
