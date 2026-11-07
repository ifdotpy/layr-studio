//! Sidebar facts from the canonical agent/project/process owners. The pure
//! projector lives in studio-read-models; entity_scope owns cursor delivery.
use rusqlite::{Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};
use studio_read_models::sidebar::{SidebarContext, SidebarFacts, WaitActivity};
use studio_runtime_schema::{
    approvals::{AgentId, ApprovalState},
    process_controls::BackgroundTaskKind,
    sidebar::SidebarChatRow,
};
use studio_runtime_store::{ProjectsRepository, StoreError, StoreResult};

fn facts(
    connection: &Connection,
    id: &str,
    server: Option<&str>,
) -> StoreResult<Option<SidebarFacts>> {
    let Some(documents) = crate::sync_policy::agent_view_documents(connection, id, server)? else {
        return Ok(None);
    };
    let Some(mut detail) = documents.detail else {
        return Ok(None);
    };
    if let (Some(detail), Some(studio_json::Value::Object(summary))) = (detail.as_object_mut(), documents.summary) {
        for (key, value) in summary {
            detail.entry(key).or_insert(value);
        }
    }
    studio_json::from_value(detail)
        .map(Some)
        .map_err(Into::into)
}

pub(crate) fn row(
    connection: &Connection,
    id: &str,
    server: Option<&str>,
) -> StoreResult<Option<SidebarChatRow>> {
    let Some(source) = facts(connection, id, server)? else {
        return Ok(None);
    };
    if !studio_read_models::sidebar::visible(&source) {
        return Ok(None);
    }
    let mut context = SidebarContext::default();
    for (_, project) in ProjectsRepository::list_records(connection)? {
        if Some(project.path.as_str()) == source.project_path.as_deref() {
            context.project_name = project.name.as_ref().cloned().unwrap_or_default();
            context.folders = project.folders.as_ref().cloned().unwrap_or_default();
        }
    }
    let team: Option<String> = connection.query_row(
        "SELECT json_extract(payload,'$.value') FROM sync_entities WHERE collection='peerTeam' AND deleted=0 AND EXISTS(SELECT 1 FROM json_each(payload,'$.value.members') WHERE value=?1) ORDER BY seq LIMIT 1",
        [id],|r|r.get(0)).optional()?;
    if let Some(team) = team {
        let team: crate::sync_policy::PeerTeamProjection = studio_json::from_str(&team)?;
        context.team_same_project =
            Some(team.project_path.as_str()) == source.project_path.as_deref();
        context.team_id = Some(team.id);
        context.team_name = team.name;
    }
    #[derive(serde::Deserialize)]
    struct Order {
        groups: Option<BTreeMap<String, Vec<String>>>,
    }
    let order: Order = studio_json::from_value(ProjectsRepository::sidebar_order(connection)?)?;
    context.order = order.groups.unwrap_or_default();
    let ids = connection.prepare_cached("SELECT id FROM runtime_agents WHERE (root_id=?1 OR id=?1) AND deleted_at IS NULL ORDER BY rowid")?
        .query_map([id], |r| r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
    let mut members = Vec::new();
    let mut activities = Vec::new();
    let mut inputs = BTreeMap::new();
    let mut deferred = BTreeSet::new();
    for member_id in ids {
        let Some(member) = (if member_id == source.id {
            Some(source.clone())
        } else {
            facts(connection, &member_id, server)?
        }) else {
            continue;
        };
        for approval in
            studio_runtime_store::approvals::open_for_agent(connection, &AgentId(member_id.clone()))
                .map_err(|_| StoreError::RecordContract("sidebar approval read failed"))?
        {
            if !matches!(approval.state, ApprovalState::Pending)
                || approval
                    .epoch
                    .is_some_and(|epoch| i64::try_from(epoch).ok() != Some(member.epoch))
            {
                continue;
            }
            if approval.deferral.as_ref().is_some_and(|d| d.active) {
                deferred.insert(member_id.clone());
            } else {
                *inputs.entry(member_id.clone()).or_insert(0_u32) += 1;
            }
        }
        let mut statement = connection.prepare_cached("SELECT record FROM runtime_tasks WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') IN ('starting','running','approval')")?;
        for raw in statement.query_map([&member_id], |r| r.get::<_, String>(0))? {
            let task: studio_runtime_store::TaskRecord = studio_json::from_str(&raw?)?;
            let display = crate::process_controls::task_record_display(&task);
            activities.push(WaitActivity {
                owner: member_id.clone(),
                command: display.kind == BackgroundTaskKind::Command,
                monitor: false,
                background: !member.in_flight
                    || member
                        .turn_id
                        .as_deref()
                        .is_some_and(|turn| turn != task.turn_id()),
            });
        }
        let mut statement = connection.prepare_cached("SELECT record FROM runtime_monitors WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') IN ('starting','running','approval')")?;
        for raw in statement.query_map([&member_id], |r| r.get::<_, String>(0))? {
            let monitor: studio_runtime_store::MonitorRecord = studio_json::from_str(&raw?)?;
            let display = crate::process_controls::monitor_record_display(&monitor);
            if display.active {
                activities.push(WaitActivity {
                    owner: member_id.clone(),
                    command: false,
                    monitor: true,
                    background: true,
                });
            }
        }
        members.push(member);
    }
    studio_read_models::sidebar::status_context(
        &mut context,
        &source,
        &members,
        &activities,
        &inputs,
        &deferred,
    );
    Ok(studio_read_models::sidebar::project(&source, &context))
}

/// Reuse agent view marks for every fact that a sidebar row reads. Ordinary
/// agent writes already mark themselves; child attention/work also marks roots.
pub(crate) fn publish_changes(
    transaction: &rusqlite::Transaction<'_>,
    pending: &studio_runtime_store::PendingEntities,
) -> StoreResult<()> {
    let mut owners = pending.team_roots.clone();
    owners.extend(pending.sidebar_rows.iter().cloned());
    for id in &pending.agents {
        let root: Option<String> = transaction
            .query_row(
                "SELECT root_id FROM runtime_agents WHERE id=?1 AND root_id<>id",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        owners.extend(root);
    }
    for (table, ids) in [
        ("runtime_tasks", &pending.tasks),
        ("runtime_monitors", &pending.studio_errors.monitors),
    ] {
        for id in ids {
            let owner: Option<String> = transaction
                .query_row(
                    &format!("SELECT json_extract(record,'$.agent') FROM {table} WHERE id=?1"),
                    [id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            if let Some(owner) = owner {
                owners.insert(owner.clone());
                owners.extend(
                    transaction
                        .query_row(
                            "SELECT root_id FROM runtime_agents WHERE id=?1",
                            [owner],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()?,
                );
            }
        }
    }
    if pending.sidebar_structure_changed {
        invalidate_all(transaction)?;
    }
    for id in owners {
        studio_runtime_store::durable_seq::mark(
            transaction,
            studio_runtime_store::durable_seq::AGENT_VIEW_MARK,
            &id,
        )?;
    }
    Ok(())
}

/// A changed peer-team projection can move a surviving chat's group when a
/// different lead disappears. The canonical peer-team publisher calls this.
pub(crate) fn invalidate_all(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    let mut statement = transaction.prepare_cached("SELECT id FROM runtime_agents WHERE deleted_at IS NULL AND COALESCE(json_extract(record,'$.isLead'),0)=1")?;
    for id in statement.query_map([], |r| r.get::<_, String>(0))? {
        studio_runtime_store::durable_seq::mark(
            transaction,
            studio_runtime_store::durable_seq::AGENT_VIEW_MARK,
            &id?,
        )?;
    }
    Ok(())
}
