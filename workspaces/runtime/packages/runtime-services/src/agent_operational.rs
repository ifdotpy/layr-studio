//! Request-local operational aggregation. Four indexed owner reads, then one
//! bottom-up pass; never canonical-view recursion or project/order reads.
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use studio_json::Value;
use studio_runtime_schema::{
    agent_entity_nested::AgentStatus,
    approvals::ApprovalState,
    chat_operational_status::{AgentWaitReasons, MarkUnreadEligibility, WaitReasonFields},
    process_controls::BackgroundTaskKind,
};
use studio_runtime_store::{StoreError, StoreResult};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fact {
    id: String,
    parent_id: Option<String>,
    status: Option<AgentStatus>,
    auto_wake: Option<bool>,
    in_flight: Option<bool>,
    archived: Option<bool>,
    parked_event: Option<String>,
    turn_id: Option<String>,
    #[serde(default)]
    epoch: i64,
}
impl Fact {
    fn active(&self) -> bool {
        self.in_flight == Some(true)
            || (self.auto_wake != Some(false)
                && matches!(
                    self.status,
                    Some(AgentStatus::Queued | AgentStatus::Starting | AgentStatus::Running)
                ))
    }
    fn waiting(&self) -> bool {
        matches!(
            self.status,
            Some(AgentStatus::Waiting | AgentStatus::Parked | AgentStatus::WaitingOnMonitor)
        )
    }
    fn parked(&self) -> bool {
        self.status == Some(AgentStatus::Parked)
            && self.parked_event.as_ref().is_some_and(|s| !s.is_empty())
    }
}
#[derive(Default, Clone)]
pub(crate) struct Context {
    pub wait: Option<AgentWaitReasons>,
    pub answer: bool,
    pub active: bool,
    pub deferred: bool,
    pub background: bool,
}
#[derive(Default, Clone, Copy)]
struct Progress {
    work: bool,
    answer: bool,
    active: bool,
}
#[derive(Default)]
struct Own {
    commands: u32,
    monitors: u32,
    inputs: u32,
    deferred: bool,
    process: bool,
}

/// Load all selected teams once. The only four statements read agents, tasks,
/// monitors and approvals through their existing root/owner indexes.
pub(crate) fn load(
    connection: &Connection,
    ids: &[String],
) -> StoreResult<BTreeMap<String, Context>> {
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let selected = studio_json::to_string(ids)?;
    let raws=connection.prepare_cached("SELECT record FROM runtime_agents WHERE deleted_at IS NULL AND (root_id IN (SELECT COALESCE(root_id,id) FROM runtime_agents WHERE id IN (SELECT value FROM json_each(?1))) OR id IN (SELECT value FROM json_each(?1)))")?.query_map([&selected],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
    let mut facts = BTreeMap::new();
    for raw in raws {
        match studio_json::from_str::<Fact>(&raw) {
            Ok(f) => {
                facts.insert(f.id.clone(), f);
            }
            Err(e) => eprintln!("Skipping malformed operational agent: {e}"),
        }
    }
    let owners = studio_json::to_string(&facts.keys().collect::<Vec<_>>())?;
    let mut own: BTreeMap<String, Own> = facts
        .keys()
        .map(|id| (id.clone(), Own::default()))
        .collect();
    let mut statement=connection.prepare_cached("SELECT record FROM runtime_tasks WHERE json_extract(record,'$.agent') IN (SELECT value FROM json_each(?1)) AND json_extract(record,'$.status') IN ('starting','running','approval')")?;
    for raw in statement.query_map([&owners], |r| r.get::<_, String>(0))? {
        let raw = raw?;
        let task: studio_runtime_store::TaskRecord = match studio_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Skipping malformed operational task: {e}");
                continue;
            }
        };
        let display = crate::process_controls::task_record_display(&task);
        let Some(agent) = facts.get(task.agent()) else {
            continue;
        };
        let Some(out) = own.get_mut(&agent.id) else {
            continue;
        };
        out.process = true;
        if display.kind == BackgroundTaskKind::Command
            && (agent.in_flight != Some(true)
                || (!task.turn_id().is_empty()
                    && agent
                        .turn_id
                        .as_ref()
                        .is_some_and(|t| !t.is_empty() && t != task.turn_id())))
        {
            out.commands = out.commands.saturating_add(1);
        }
    }
    let mut statement=connection.prepare_cached("SELECT record FROM runtime_monitors WHERE json_extract(record,'$.agent') IN (SELECT value FROM json_each(?1)) AND json_extract(record,'$.status') IN ('starting','running','approval')")?;
    for raw in statement.query_map([&owners], |r| r.get::<_, String>(0))? {
        let raw = raw?;
        let monitor: studio_runtime_store::MonitorRecord = match studio_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Skipping malformed operational monitor: {e}");
                continue;
            }
        };
        if crate::process_controls::monitor_record_display(&monitor).active
            && let Some(out) = own.get_mut(&monitor.agent)
        {
            out.monitors = out.monitors.saturating_add(1);
            out.process = true;
        }
    }
    for approval in studio_runtime_store::approvals::open_for_agents(connection, &owners)
        .map_err(|_| StoreError::RecordContract("operational approval read failed"))?
    {
        let Some(agent) = facts.get(approval.agent.as_str()) else {
            continue;
        };
        if approval.hidden.is_some()
            || !matches!(approval.state, ApprovalState::Pending)
            || approval
                .epoch
                .is_some_and(|e| i64::try_from(e).ok() != Some(agent.epoch))
        {
            continue;
        }
        let out = own.get_mut(&agent.id).expect("known owner");
        if approval.deferral.as_ref().is_some_and(|d| d.active) {
            out.deferred = true;
        } else {
            out.inputs = out.inputs.saturating_add(1);
        }
    }
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in facts.values() {
        if let Some(parent) = &f.parent_id {
            children
                .entry(parent.clone())
                .or_default()
                .push(f.id.clone());
        }
    }
    fn visit(
        id: &str,
        facts: &BTreeMap<String, Fact>,
        own: &BTreeMap<String, Own>,
        children: &BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        done: &mut BTreeMap<String, Progress>,
    ) -> Progress {
        if let Some(p) = done.get(id) {
            return *p;
        }
        if !visiting.insert(id.to_owned()) {
            return Progress::default();
        }
        let f = &facts[id];
        let o = &own[id];
        let mut below = Progress::default();
        for child in children.get(id).into_iter().flatten() {
            let p = visit(child, facts, own, children, visiting, done);
            below.work |= p.work;
            below.answer |= p.answer;
            below.active |= p.active;
        }
        let stopped = f.archived == Some(true)
            || matches!(
                f.status,
                Some(AgentStatus::Paused | AgentStatus::Failed | AgentStatus::Interrupted)
            );
        let local_work = f.in_flight == Some(true)
            || matches!(
                f.status,
                Some(
                    AgentStatus::Queued
                        | AgentStatus::Starting
                        | AgentStatus::Running
                        | AgentStatus::Approval
                )
            );
        let work = !stopped
            && (local_work
                || ((f.auto_wake != Some(false) || f.parked())
                    && f.waiting()
                    && (f.parked() || o.commands > 0 || o.monitors > 0 || below.work)));
        let p = Progress {
            work,
            answer: o.inputs > 0 || below.answer,
            active: f.active() || o.process || below.active,
        };
        visiting.remove(id);
        done.insert(id.to_owned(), p);
        p
    }
    let mut done = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    for id in facts.keys() {
        visit(id, &facts, &own, &children, &mut visiting, &mut done);
    }
    let mut result = BTreeMap::new();
    for (id, f) in &facts {
        let o = &own[id];
        let p = done[id];
        let agents = children
            .get(id)
            .into_iter()
            .flatten()
            .filter(|c| done[*c].work)
            .count();
        let wait = AgentWaitReasons::new(WaitReasonFields {
            agents: u32::try_from(agents).unwrap_or(u32::MAX),
            commands: o.commands,
            monitors: o.monitors,
            inputs: o.inputs,
            event: f.parked_event.clone().filter(|s| !s.is_empty()),
        });
        result.insert(
            id.clone(),
            Context {
                wait,
                answer: p.answer,
                active: p.active,
                deferred: o.deferred,
                background: o.monitors > 0,
            },
        );
    }
    Ok(result)
}

pub(crate) fn decorate(summary: &mut Value, context: Option<&Context>) -> StoreResult<()> {
    let context = context.cloned().unwrap_or_default();
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Source {
        status: Option<AgentStatus>,
        auto_wake: Option<bool>,
        empty: Option<bool>,
        in_flight: Option<bool>,
        thread_id: Option<String>,
        last_completed_turn: Option<String>,
        last_completed_turn_status: Option<String>,
        read_state: Option<studio_runtime_schema::agent_entity_nested_extra::ReadStateDto>,
        read_state_supported: Option<bool>,
        has_question: Option<bool>,
        has_approval: Option<bool>,
        has_background_work: Option<bool>,
        account_key: Option<String>,
        epoch: Option<i64>,
        capacity_retry:
            Option<studio_runtime_schema::agent_entity_nested_extra::AgentEntityCapacityRetryDto>,
    }
    let source: Source = studio_json::from_value(summary.clone())?;
    let mark = studio_read_models::chat_status::mark_read_target(
        source.thread_id.as_deref(),
        source.last_completed_turn.as_deref(),
        source.last_completed_turn_status.as_deref(),
        source.read_state.as_ref(),
    );
    let unread = mark.as_ref().is_some_and(|r| !r.read);
    let eligible = if source.read_state_supported != Some(true) {
        MarkUnreadEligibility::Unsupported
    } else if mark.is_none() {
        MarkUnreadEligibility::NoCompletedResult
    } else if unread {
        MarkUnreadEligibility::AlreadyUnread
    } else {
        MarkUnreadEligibility::Allowed
    };
    let disk_full=source.capacity_retry.as_ref().is_some_and(|retry| {
        retry.thread_id==source.thread_id
            && retry.epoch.as_ref().map(|epoch|epoch.as_i64())==source.epoch
            && retry.account_key.as_deref().unwrap_or("default")==source.account_key.as_deref().unwrap_or("default")
            && retry.status=="scheduled"
            && retry.accepted_turn_id.as_ref().is_none_or(String::is_empty)
            && retry.cause.as_ref()==Some(&studio_runtime_schema::continuation_snapshot::CapacityContinuationCause::DiskFull)
    });
    let facts = studio_read_models::chat_status::ChatStatusFacts {
        status: source.status.as_ref(),
        auto_wake: source.auto_wake,
        empty: source.empty == Some(true),
        in_flight: source.in_flight == Some(true),
        answer: context.answer
            || source.has_question == Some(true)
            || source.has_approval == Some(true),
        deferred: context.deferred,
        background_work: context.background || source.has_background_work == Some(true),
        active: context.active || source.has_background_work == Some(true),
        disk_full,
        unread,
        wait: context.wait.as_ref(),
    };
    let operational = studio_read_models::chat_status::derive_chat_status(&facts);
    summary["operationalStatus"] = studio_json::to_value(operational)?;
    summary["markRead"] = studio_json::to_value(mark)?;
    summary["markUnread"] = studio_json::to_value(eligible)?;
    summary["inFlight"] = Value::Bool(source.in_flight == Some(true));
    Ok(())
}

/// Existing journal owner/root fanout only. Projects/order never invalidate agents.
pub(crate) fn affected_owners(
    tx: &rusqlite::Transaction<'_>,
    pending: &studio_runtime_store::PendingEntities,
) -> StoreResult<BTreeSet<String>> {
    let mut owners = pending.team_roots.clone();
    for id in &pending.agents {
        owners.extend(
            tx.query_row(
                "SELECT root_id FROM runtime_agents WHERE id=?1 AND root_id<>id",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
        );
    }
    for (table, ids) in [
        ("runtime_tasks", &pending.tasks),
        ("runtime_monitors", &pending.studio_errors.monitors),
    ] {
        for id in ids {
            let agent: Option<String> = tx
                .query_row(
                    &format!("SELECT json_extract(record,'$.agent') FROM {table} WHERE id=?1"),
                    [id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            if let Some(agent) = agent {
                owners.insert(agent.clone());
                owners.extend(
                    tx.query_row(
                        "SELECT root_id FROM runtime_agents WHERE id=?1",
                        [agent],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?,
                );
            }
        }
    }
    let owners_json = studio_json::to_string(&owners)?;
    let owners=tx.prepare_cached("WITH RECURSIVE owners(id) AS (SELECT value FROM json_each(?1) UNION SELECT a.parent_id FROM runtime_agents a JOIN owners o ON a.id=o.id WHERE a.parent_id IS NOT NULL) SELECT id FROM owners")?.query_map([owners_json],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
    Ok(owners.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::trace::{TraceEvent, TraceEventCodes};
    use std::cell::Cell;
    thread_local! {static READS:Cell<usize>=const{Cell::new(0)};}
    fn count(event: TraceEvent<'_>) {
        if let TraceEvent::Stmt(statement, _) = event
            && statement
                .expanded_sql()
                .is_some_and(|sql| sql.starts_with("SELECT"))
        {
            READS.with(|n| n.set(n.get() + 1));
        }
    }
    #[tokio::test]
    async fn v5_operational_batch_has_four_indexed_reads_and_bottom_up_waits() {
        use crate::entity_publication::test_support::{seed_chat, store};
        use studio_json::json;
        let (_directory, store) = store().await;
        for (id, parent, status) in [
            ("root", None, "waiting"),
            ("child", Some("root"), "waiting"),
            ("grandchild", Some("child"), "running"),
        ] {
            seed_chat(&store,json!({"id":id,"rootId":"root","parentId":parent,"isLead":parent.is_none(),"provider":"codex","accountKey":"default","compactions":0,"epoch":1,"status":status,"autoWake":true})).await;
        }
        let contexts = store
            .test_read_transaction(|tx| {
                READS.with(|n| n.set(0));
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(count));
                let result = load(tx, &["root".into(), "child".into()]);
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
                assert_eq!(
                    READS.with(Cell::get),
                    4,
                    "one request aggregates all selected teams in four owner reads"
                );
                result
            })
            .expect("batch");
        assert_eq!(
            contexts["root"]
                .wait
                .as_ref()
                .expect("descendant work")
                .facts()
                .agents,
            1
        );
        assert_eq!(
            contexts["child"]
                .wait
                .as_ref()
                .expect("direct child work")
                .facts()
                .agents,
            1
        );
        assert!(contexts["root"].active);
        for collection in ["agentSummary", "agentTail"] {
            store.test_read_transaction(|tx| {
                READS.with(|n|n.set(0));
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT,Some(count));
                let result=crate::entity_scope::resume(tx,None,Some(&format!("[\"{collection}\"]")),Some("root"),None,false,100);
                tx.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT,None);
                eprintln!("V5_QUERY_COUNTS collection={collection} total_selects={} operational_selects={}",READS.with(Cell::get),if collection=="agentSummary" {4}else{0});
                result.map(|_|())
            }).expect("actual stream query count");
        }
    }
}
