use super::*;
use crate::event_write::{CallerAdmission, RequestKey, begin_request};
use std::sync::Arc;
use studio_runtime_schema::peer_team_ingress::TeamMutation;

pub async fn peer_team_write(
    store: Arc<RuntimeStore>,
    body: TeamMutation,
    default_account: String,
    known_accounts: Vec<crate::account_policy::PublicAccount>,
) -> Result<Value, ProjectsError> {
    let path_value = Some(body.path().to_owned());
    let revision = body
        .expected_revision()
        .as_u64()
        .ok_or(ProjectsError::Invalid("peer team revision is invalid"))?;
    let request_id = python_strip(body.request_id()).to_owned();
    if request_id.is_empty() || request_id.chars().count() > 255 {
        return Err(ProjectsError::Invalid("peer team request ID is invalid"));
    }
    let team_id = body
        .team_id()
        .map(|value| {
            Uuid::parse_str(value)
                .map(|id| id.to_string())
                .map_err(|_| ProjectsError::Invalid("peer team ID is invalid"))
        })
        .transpose()?;
    if !matches!(&body, TeamMutation::Move(_)) && team_id.is_none() {
        return Err(ProjectsError::Invalid("peer team ID is invalid"));
    }
    let (name, members) = match &body {
        TeamMutation::Save(v) => {
            let name = python_strip(&v.name);
            if name.is_empty() || name.chars().count() > 80 {
                return Err(ProjectsError::Invalid("peer team name is invalid"));
            }
            if v.members.len() < 2
                || v.members.iter().any(String::is_empty)
                || v.members
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != v.members.len()
            {
                return Err(ProjectsError::Invalid(
                    "select at least two different lead chats",
                ));
            }
            (Some(name.to_owned()), v.members.clone())
        }
        TeamMutation::Delete(_) | TeamMutation::Move(_) => (None, Vec::new()),
    };
    let signature = peer_team_signature(&body)?;
    let key = format!("peer-team:{request_id}");
    let (prepared, path) = super::prepare_path(store.clone(), path_value, false).await?;
    store.transaction_with(move |tx| {
        prepared.recheck(tx)?;
        let server = RuntimeStore::transaction_local_server_id(tx)?.ok_or(ProjectsError::Invalid("The local server ID is unavailable"))?;
        let admitted = match begin_request::<Box<serde_json::value::RawValue>>(tx,RequestKey::new("peer-team", &[&server,"owner-api"], &request_id).raw(&key).operation(&["write"]), &signature,(now_seconds()*1000.0) as i64).map_err(StoreError::from)? {
            CallerAdmission::Fresh(admitted) => admitted,
            CallerAdmission::Replay(reply) => return serde_json::from_str(reply.get()).map_err(StoreError::from).map_err(Into::into),
        };
        let mut project = match load_project(tx, &path)? {
            Some(project) => project,
            None if path.starts_with("project:") => return Err(ProjectsError::Invalid("select an existing project")),
            None => { let account = project_account(&prepared, tx, &path, &default_account)?; new_project(&prepared, &path, &project_name(None, &path)?, &account, &known_accounts)? }
        };
        let current = revision_of(Some(&project), "peerTeamsRevision", &path)?;
        if revision != current { return Err(ProjectsError::Conflict("Peer teams changed. Reload before saving")); }
        let raw_teams = project.get("peerTeams").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut previous_ids = raw_teams.iter().map(|team| {
            let value = team.get("id").and_then(Value::as_str)
                .ok_or_else(|| ProjectsError::InvalidStored { table: "runtime_projects", id: path.clone() })?;
            Uuid::parse_str(value).map(|uuid| uuid.to_string())
                .map_err(|_| ProjectsError::InvalidStored { table: "runtime_projects", id: path.clone() })
        }).collect::<Result<std::collections::HashSet<_>,_>>()?;
        let agents = project_agents(tx)?;
        let mut teams = live_teams(&raw_teams, &agents, &path, &prepared)?;
        let mut statement = tx.prepare_cached("SELECT id FROM sync_entities WHERE collection='peerTeam' AND json_extract(payload,'$.value.projectPath')=?1")?;
        previous_ids.extend(statement.query_map([&path], |row| row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?);
        drop(statement);
        match body {
            TeamMutation::Save(_) => {
                let team_id = team_id.as_deref().ok_or(ProjectsError::Invalid("peer team ID is invalid"))?;
                for member in &members {
                    let id = member.as_str();
                    if !agents.get(id).is_some_and(|agent| is_project_lead(agent, &path, &prepared)) { return Err(ProjectsError::Invalid("select live lead chats from this project only")); }
                }
                if teams.iter().any(|team| team.get("id").and_then(Value::as_str) != Some(team_id)
                    && team.get("members").and_then(Value::as_array).is_some_and(|existing| existing.iter().any(|member| member.as_str().is_some_and(|member| members.iter().any(|selected| selected == member))))) {
                    return Err(ProjectsError::Invalid("a chat already belongs to another peer team"));
                }
                let selected = json!({"id":team_id,"name":name,"members":members});
                if let Some(old) = teams.iter_mut().find(|team| team.get("id").and_then(Value::as_str) == Some(team_id)) { *old = selected; } else { teams.push(selected); }
            }
            TeamMutation::Delete(_) => teams.retain(|team| team.get("id").and_then(Value::as_str) != team_id.as_deref()),
            TeamMutation::Move(movement) => {
                let member = python_strip(&movement.member);
                if member.is_empty() || member.chars().count() > 255 || !agents.get(member).is_some_and(|agent| is_project_lead(agent, &path, &prepared)) { return Err(ProjectsError::Invalid("select a live lead chat from this project only")); }
                let source = teams.iter().position(|team| team.get("members").and_then(Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some(member))));
                let target = team_id.as_deref().and_then(|id| teams.iter().position(|team| team.get("id").and_then(Value::as_str) == Some(id)));
                if team_id.is_some() && target.is_none() { return Err(ProjectsError::Invalid("the destination team is no longer available")); }
                if source == target { return Err(ProjectsError::Invalid("the chat already has this team membership")); }
                if let Some(index) = source
                    && let Some(list) = teams[index]["members"].as_array_mut()
                { list.retain(|item| item.as_str() != Some(member)); }
                if let Some(index) = target
                    && let Some(list) = teams[index]["members"].as_array_mut()
                { list.push(json!(member)); }
                teams.retain(|team| team.get("members").and_then(Value::as_array).is_some_and(|items| items.len() >= 2));
            }
        }
        project["peerTeams"] = json!(teams);
        project["peerTeamsRevision"] = json!(current + 1);
        project["updated"] = json!(now_seconds());
        ProjectsRepository::put_project(tx, &project)?;
        let team_values = project.get("peerTeams").and_then(Value::as_array).cloned().unwrap_or_default();
        let live = live_teams(&team_values, &agents, &path, &prepared)?;
        let mut live_ids = std::collections::HashSet::new();
        for team in &live {
            let id = team.get("id").and_then(Value::as_str).ok_or(ProjectsError::InvalidStored { table: "runtime_projects", id: path.clone() })?;
            live_ids.insert(id.to_owned());
            SyncDocumentsRepository::put_entity(tx, "peerTeam", id, peer_team_projection(team, &path, current + 1)?, false)?;
        }
        for id in previous_ids.difference(&live_ids) { SyncDocumentsRepository::put_entity(tx, "peerTeam", id, json!({}), true)?; }
        refresh_project_rooms(tx, &path, &prepared)?;
        let reply = serde_json::value::to_raw_value(&project).map_err(StoreError::from)?;
        crate::event_write::settle_request(tx, &admitted, None, &reply).map_err(StoreError::from)?;
        Ok(project)
    }).await
}

pub(crate) fn project_agents(
    tx: &studio_runtime_store::Transaction<'_>,
) -> Result<std::collections::HashMap<String, Value>, ProjectsError> {
    ProjectsRepository::agents(tx)?
        .into_iter()
        .map(|(id, agent)| {
            check_id(&agent, "runtime_agents", &id)?;
            Ok((id, agent))
        })
        .collect()
}

pub(crate) fn is_project_lead(agent: &Value, path: &str, prepared: &PreparedHost) -> bool {
    agent.get("isLead").and_then(Value::as_bool) == Some(true)
        && agent.get("deletedAt").is_none_or(Value::is_null)
        && agent.get("parentId").is_none_or(Value::is_null)
        && agent.get("rootId").and_then(Value::as_str) == agent.get("id").and_then(Value::as_str)
        && agent
            .get("cwd")
            .and_then(Value::as_str)
            .is_some_and(|cwd| prepared.resolve(cwd).is_ok_and(|resolved| resolved == path))
}

pub(crate) fn live_teams(
    raw: &[Value],
    agents: &std::collections::HashMap<String, Value>,
    path: &str,
    prepared: &PreparedHost,
) -> Result<Vec<Value>, ProjectsError> {
    raw.iter()
        .map(|team| {
            let invalid = || ProjectsError::InvalidStored {
                table: "runtime_projects",
                id: path.to_owned(),
            };
            let id = team.get("id").and_then(Value::as_str).ok_or_else(invalid)?;
            let id = Uuid::parse_str(id).map_err(|_| invalid())?.to_string();
            let name = team
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            let members = team
                .get("members")
                .and_then(Value::as_array)
                .ok_or_else(invalid)?;
            let members = members
                .iter()
                .filter_map(Value::as_str)
                .filter(|member| {
                    agents
                        .get(*member)
                        .is_some_and(|agent| is_project_lead(agent, path, prepared))
                })
                .collect::<Vec<_>>();
            Ok((members.len() >= 2).then(|| json!({"id":id,"name":name,"members":members})))
        })
        .filter_map(|result| match result {
            Ok(Some(team)) => Some(Ok(team)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub(crate) fn peer_team_projection(
    team: &Value,
    path: &str,
    revision: u64,
) -> Result<Value, ProjectsError> {
    Ok(
        json!({"id":team.get("id").and_then(Value::as_str).ok_or(ProjectsError::Invalid("stored peer team is invalid"))?,
        "name":team.get("name").and_then(Value::as_str).ok_or(ProjectsError::Invalid("stored peer team is invalid"))?,
        "projectPath":path,"members":team.get("members").ok_or(ProjectsError::Invalid("stored peer team is invalid"))?,"revision":revision}),
    )
}

pub(crate) fn refresh_project_rooms(
    tx: &studio_runtime_store::Transaction<'_>,
    path: &str,
    prepared: &PreparedHost,
) -> Result<(), ProjectsError> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_rooms')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(());
    }
    let mut statement = tx.prepare_cached("SELECT id FROM runtime_rooms WHERE json_extract(record,'$.kind')='private' AND json_extract(record,'$.projectPath')=?1")?;
    let ids = statement
        .query_map([path], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for id in ids {
        ProjectsRepository::refresh_private_room(tx, &id)?;
        let room =
            studio_runtime_store::Table::<studio_runtime_schema::room_record::RoomRecord>::new(tx)
                .get(&id)?
                .ok_or_else(|| ProjectsError::InvalidStored {
                    table: "runtime_rooms",
                    id: id.clone(),
                })?;
        crate::radio_runtime::publish_room(tx, &room, prepared)?;
    }
    Ok(())
}

pub async fn reorder(
    store: &RuntimeStore,
    body: studio_runtime_schema::project_ingress::SidebarReorder,
) -> Result<Value, ProjectsError> {
    let request_id = python_strip(body.request_id.as_str());
    if request_id.is_empty() {
        return Err(ProjectsError::Invalid("request ID is invalid"));
    }
    let expected = body
        .expected_revision
        .as_u64()
        .ok_or(ProjectsError::Invalid("sidebar revision is invalid"))?;
    let groups = &body.groups;
    let total = groups.values().map(Vec::len).sum::<usize>();
    if groups.len() > 500
        || total > 10_000
        || serde_json::to_string(groups).map_or(true, |value| value.len() > 5_000_000)
        || groups.iter().any(|(key, items)| {
            key.chars().count() > 4096
                || items.len() > 10_000
                || items.iter().any(|item| item.chars().count() > 4096)
                || items.iter().collect::<std::collections::HashSet<_>>().len() != items.len()
        })
    {
        return Err(ProjectsError::Invalid("sidebar order is invalid"));
    }
    // Keep the existing receipt identity when request bodies are typed.
    let signature = sidebar_signature(&body)?;
    let key = format!("sidebar-order:{request_id}");
    let request_id = request_id.to_owned();
    let migration = body.migration.value() == Some(&true);
    let groups = body.groups;
    store
        .transaction_with(move |tx| -> Result<Value, ProjectsError> {
            let server = RuntimeStore::transaction_local_server_id(tx)?
                .ok_or(ProjectsError::Invalid("The local server ID is unavailable"))?;
            let admitted = match begin_request::<Box<serde_json::value::RawValue>>(
                tx,
                RequestKey::new("sidebar-order", &[&server, "owner-api"], &request_id)
                    .raw(&key)
                    .operation(&["write"]),
                &signature,
                (now_seconds() * 1000.0) as i64,
            )
            .map_err(StoreError::from)?
            {
                CallerAdmission::Fresh(admitted) => admitted,
                CallerAdmission::Replay(reply) => {
                    return serde_json::from_str(reply.get())
                        .map_err(StoreError::from)
                        .map_err(Into::into);
                }
            };
            let current = ProjectsRepository::sidebar_order(tx)?;
            let revision = current.get("revision").and_then(Value::as_u64).ok_or(
                ProjectsError::InvalidStored {
                    table: "runtime_sidebar_order",
                    id: "current".to_owned(),
                },
            )?;
            if revision != expected {
                return Err(ProjectsError::Conflict(
                    "Sidebar order changed. Refreshed from the server",
                ));
            }
            if current
                .get("groups")
                .is_some_and(|groups| !groups.is_null())
                && migration
            {
                return Err(ProjectsError::Conflict(
                    "Sidebar order changed. Refreshed from the server",
                ));
            }
            let result = json!({"revision":revision.saturating_add(1),"groups":groups});
            ProjectsRepository::put_sidebar_order(tx, &result)?;
            let reply = serde_json::value::to_raw_value(&result).map_err(StoreError::from)?;
            crate::event_write::settle_request(tx, &admitted, None, &reply)
                .map_err(StoreError::from)?;
            Ok(result)
        })
        .await
}

fn peer_team_signature(
    body: &studio_runtime_schema::peer_team_ingress::TeamMutation,
) -> Result<String, ProjectsError> {
    operation_signature("peer-team", body)
}

fn sidebar_signature(
    body: &studio_runtime_schema::project_ingress::SidebarReorder,
) -> Result<String, ProjectsError> {
    operation_signature("sidebar-order", body)
}

fn operation_signature(
    operation: &str,
    body: &impl serde::Serialize,
) -> Result<String, ProjectsError> {
    #[derive(serde::Serialize)]
    struct SignedOperation<'a, T> {
        operation: &'a str,
        body: &'a T,
    }
    let bytes =
        studio_runtime_schema::json_compat::python_json_compact_serialized(&SignedOperation {
            operation,
            body,
        })
        .map_err(StoreError::from)?;
    Ok(sha256_hex(bytes.as_bytes()))
}

#[cfg(test)]
mod sidebar_signature_tests {
    use super::{peer_team_signature, sidebar_signature};
    use studio_runtime_schema::project_ingress::SidebarReorder;

    #[test]
    fn equivalent_requests_have_the_same_signature() {
        let a: SidebarReorder = serde_json::from_str(r#"{"action":"reorder","request_id":"r","expected_revision":1,"groups":{"b":["2"],"a":["1"]}}"#).unwrap();
        let b: SidebarReorder = serde_json::from_str(r#"{"groups":{"a":["1"],"b":["2"]},"expected_revision":1,"request_id":"r","action":"reorder"}"#).unwrap();
        let changed: SidebarReorder = serde_json::from_str(r#"{"action":"reorder","request_id":"r","expected_revision":2,"groups":{"a":["1"],"b":["2"]}}"#).unwrap();
        assert_eq!(
            sidebar_signature(&a).unwrap(),
            sidebar_signature(&b).unwrap()
        );
        assert_ne!(
            sidebar_signature(&a).unwrap(),
            sidebar_signature(&changed).unwrap()
        );
    }

    #[test]
    fn peer_team_signature_keeps_legacy_bytes() {
        let body = serde_json::from_str(r#"{"action":"delete","path":"/雪","expected_revision":1,"request_id":"r","team_id":"a"}"#).unwrap();
        assert_eq!(
            peer_team_signature(&body).unwrap(),
            "bc5059bb0ff81794b0b058e3d29d06f81fff2ad19c7a4cca8f33e1c2b7d77c63"
        );
    }

    #[test]
    fn sidebar_signature_keeps_legacy_bytes() {
        let body = serde_json::from_str(r#"{"action":"reorder","request_id":"r","expected_revision":1,"groups":{"b":["2"],"a":["1"]}}"#).unwrap();
        assert_eq!(
            sidebar_signature(&body).unwrap(),
            "b253b17fedf5317dd24bcccca6f1d6eb2166326039b3f14fa442c55de2a17f14"
        );
    }
}
