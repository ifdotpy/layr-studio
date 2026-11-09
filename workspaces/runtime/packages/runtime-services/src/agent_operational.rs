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
    root_id: Option<String>,
    source: Option<String>,
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
    fn managed(&self) -> bool {
        self.source
            .as_deref()
            .is_none_or(|source| source == "managed")
    }
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
    pub monitors: u32,
}
#[derive(Default, Clone, Copy)]
struct Progress {
    work: bool,
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
        if !agent.managed() {
            continue;
        }
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
        if facts.get(&monitor.agent).is_some_and(Fact::managed)
            && crate::process_controls::monitor_record_display(&monitor).active
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
        if !agent.managed()
            || approval.hidden.is_some()
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
    let mut root_activity: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for f in facts.values().filter(|f| f.managed()) {
        if let Some(root) = f.root_id.as_ref().filter(|root| !root.is_empty()) {
            let flags = root_activity.entry(root.clone()).or_default();
            flags.0 |= own[&f.id].inputs > 0;
            flags.1 |= f.active() || own[&f.id].process;
        }
    }
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in facts.values() {
        if f.managed()
            && let Some(parent) = &f.parent_id
        {
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
        let p = Progress { work };
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
                answer: o.inputs > 0 || root_activity.get(id).is_some_and(|flags| flags.0),
                active: f.active()
                    || o.process
                    || root_activity.get(id).is_some_and(|flags| flags.1),
                deferred: o.deferred,
                monitors: o.monitors,
            },
        );
    }
    Ok(result)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    status: Option<String>,
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

/// Combine the typed agent projection with the request-local operational context.
pub(crate) fn summary(
    full: &studio_runtime_schema::generated::AgentEntityDto,
    context: Option<&Context>,
) -> StoreResult<studio_runtime_schema::generated::AgentSummaryEntityDto> {
    let source = Source {
        status: full.status.clone().flatten(),
        auto_wake: full.auto_wake.flatten(),
        empty: full.empty.as_ref().and_then(Option::as_ref).and_then(Value::as_bool),
        in_flight: full.in_flight.flatten(),
        thread_id: full.thread_id.clone().flatten(),
        last_completed_turn: full.last_completed_turn.clone().flatten(),
        last_completed_turn_status: full.last_completed_turn_status.clone().flatten(),
        read_state: full.read_state.as_ref().and_then(Option::as_ref).map(Option::<studio_runtime_schema::agent_entity_nested_extra::ReadStateDto>::deserialize).transpose()?.flatten(),
        read_state_supported: full.read_state_supported.flatten(),
        has_question: full.has_question.flatten(),
        has_approval: full.has_approval.flatten(),
        has_background_work: full.has_background_work,
        account_key: full.account_key.clone().flatten(),
        epoch: full.epoch.flatten(),
        capacity_retry: full.capacity_retry.as_ref().and_then(Option::as_ref).map(Option::<studio_runtime_schema::agent_entity_nested_extra::AgentEntityCapacityRetryDto>::deserialize).transpose()?.flatten(),
    };
    let (status, mark, eligible) = operational(&source, context);
    Ok(
        studio_runtime_schema::generated::AgentSummaryEntityDto::from_agent(
            full, status, mark, eligible,
        ),
    )
}

fn operational(
    source: &Source,
    context: Option<&Context>,
) -> (
    studio_runtime_schema::chat_operational_status::AgentOperationalStatus,
    Option<studio_runtime_schema::agent_entity_nested_extra::ReadStateDto>,
    MarkUnreadEligibility,
) {
    let context = context.cloned().unwrap_or_default();
    let mark = if source.read_state_supported == Some(true) {
        studio_read_models::chat_status::mark_read_target(
            source.thread_id.as_deref(),
            source.last_completed_turn.as_deref(),
            source.last_completed_turn_status.as_deref(),
            source.read_state.as_ref(),
        )
    } else {
        None
    };
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
    let status = source.status.as_ref().map(|status| {
        AgentStatus::deserialize(Value::String(status.clone())).unwrap_or(AgentStatus::Unknown)
    });
    let facts = studio_read_models::chat_status::ChatStatusFacts {
        status: status.as_ref(),
        auto_wake: source.auto_wake,
        empty: source.empty == Some(true),
        in_flight: source.in_flight == Some(true),
        answer: context.answer
            || source.has_question == Some(true)
            || source.has_approval == Some(true),
        deferred: context.deferred,
        background_work: context.monitors > 0 || source.has_background_work == Some(true),
        active: context.active || source.has_background_work == Some(true),
        disk_full,
        unread,
        wait: context.wait.as_ref(),
    };
    let operational = studio_read_models::chat_status::derive_chat_status(&facts);
    (operational, mark, eligible)
}

// Corrupt-record repair alone retains the tolerant JSON projection.
pub(crate) fn decorate(summary: &mut Value, context: Option<&Context>) -> StoreResult<()> {
    let source = Source::deserialize(&*summary)?;
    let (operational, mark, eligible) = operational(&source, context);
    summary["operationalStatus"] =
        serde::Serialize::serialize(&operational, studio_json::value::Serializer)?;
    summary["markRead"] = serde::Serialize::serialize(&mark, studio_json::value::Serializer)?;
    summary["markUnread"] = serde::Serialize::serialize(&eligible, studio_json::value::Serializer)?;
    summary["inFlight"] = Value::Bool(source.in_flight == Some(true));
    Ok(())
}

/// Existing journal owner/root fanout only. Projects/order never invalidate agents.
pub(crate) fn affected_owners(
    tx: &rusqlite::Transaction<'_>,
    pending: &studio_runtime_store::PendingEntities,
) -> StoreResult<BTreeSet<String>> {
    let mut owners = pending.team_roots.clone();
    owners.extend(pending.agents.iter().cloned());
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
    if owners.is_empty() {
        return Ok(owners);
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

#[cfg(test)]
mod owner_matrix_tests {
    fn put_task(db: &rusqlite::Connection, task: &studio_runtime_schema::task_record::TaskRecord) {
        let tx = db.unchecked_transaction().expect("fixture transaction");
        studio_runtime_store::TaskRepository::put(&tx, task).expect("task");
        tx.commit().expect("fixture commit");
    }

    use super::*;
    use studio_json::json;
    fn put(db: &Connection, id: &str, parent: Option<&str>, status: &str, extra: &Value) {
        let mut record = json!({"id":id,"rootId":"root","parentId":parent,"isLead":parent.is_none(),"provider":"codex","accountKey":"default","epoch":1,"status":status,"autoWake":true,"source":"managed"});
        record
            .as_object_mut()
            .expect("record")
            .extend(extra.as_object().expect("extra").clone());
        db.execute(
            "INSERT INTO runtime_agents(id,record) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            [id, &record.to_string()],
        )
        .expect("fact");
    }
    #[test]
    fn v5_child_status_and_parked_event_matrix() {
        for (status, extra, expected) in [
            ("queued", json!({}), true),
            ("starting", json!({}), true),
            ("running", json!({}), true),
            ("approval", json!({}), true),
            ("completed", json!({}), false),
            ("idle", json!({}), false),
            ("completed", json!({"inFlight":true}), true),
            ("waiting", json!({}), false),
            ("waiting_on_monitor", json!({}), false),
            ("paused", json!({"inFlight":true}), false),
            ("failed", json!({}), false),
            ("interrupted", json!({}), false),
            ("running", json!({"archived":true}), false),
            ("waiting", json!({"autoWake":false}), false),
            (
                "parked",
                json!({"autoWake":false,"parkedEvent":"ready"}),
                true,
            ),
        ] {
            let db = studio_runtime_store::test_canvas();
            put(&db, "root", None, "waiting", &json!({}));
            put(&db, "child", Some("root"), status, &extra);
            let contexts = load(&db, &["root".into()]).expect("facts");
            let count = contexts["root"]
                .wait
                .as_ref()
                .map_or(0, |wait| wait.facts().agents);
            assert_eq!(count, u32::from(expected), "{status} {extra}");
        }
    }
    #[test]
    fn v5_unsupported_summary_has_no_read_action() {
        let mut summary = json!({"id":"root","status":"completed","threadId":"thread","lastCompletedTurn":"turn","lastCompletedTurnStatus":"completed","readStateSupported":false,"autoWake":true});
        decorate(&mut summary, None).expect("summary");
        assert!(
            summary["markRead"].is_null(),
            "unsupported receipts cannot create an actionable proof"
        );
        assert_eq!(summary["markUnread"], "unsupported");
    }
    #[test]
    fn v5_wait_detail_retirement_matrix() {
        use studio_runtime_schema::task_record::{NonnegativeNumber, TaskRecord, TaskType};
        let count = |context: &Context| context.wait.as_ref().map_or(0, |wait| wait.facts().agents);
        for kind in ["command", "monitor", "tool"] {
            for process_status in ["starting", "running", "approval"] {
                for (status, extra, live) in [
                    ("waiting", json!({}), true),
                    ("waiting_on_monitor", json!({}), true),
                    ("paused", json!({"autoWake":false}), false),
                    ("failed", json!({}), false),
                    ("interrupted", json!({}), false),
                    ("completed", json!({}), false),
                    ("idle", json!({}), false),
                    ("waiting", json!({"archived":true}), false),
                    ("waiting", json!({"autoWake":false}), false),
                    ("waiting", json!({"source":"local"}), false),
                ] {
                    let db = studio_runtime_store::test_canvas();
                    put(&db, "root", None, "waiting", &json!({}));
                    put(&db, "child", Some("root"), status, &extra);
                    if kind == "monitor" {
                        let monitor = crate::monitor_policy::complete_test_monitor(
                            json!({"id":"work","agent":"child","status":process_status,"epoch":999}),
                        );
                        db.execute(
                            "INSERT INTO runtime_monitors(id,record) VALUES('work',?1)",
                            [monitor.to_string()],
                        )
                        .expect("monitor");
                    } else {
                        let mut task = TaskRecord::new(
                            "child".into(),
                            "work".into(),
                            "turn".into(),
                            NonnegativeNumber::from_f64(0.0).expect("time"),
                            if kind == "command" {
                                TaskType::CommandExecution
                            } else {
                                TaskType::DynamicToolCall
                            },
                            String::new(),
                        );
                        task.set_status(
                            studio_json::from_str(&format!("\"{process_status}\""))
                                .expect("status"),
                        );
                        put_task(&db, &task);
                    }
                    let contexts = load(&db, &["root".into()]).expect("facts");
                    assert_eq!(
                        count(&contexts["root"]),
                        u32::from(live && kind != "tool"),
                        "{kind} {process_status} {status} {extra}"
                    );
                }
            }
        }
        for (status, extra, live) in [
            ("parked", json!({"parkedEvent":"ready"}), true),
            ("parked", json!({"parkedEvent":null}), false),
            ("parked", json!({"parkedEvent":""}), false),
            ("waiting", json!({"parkedEvent":"ready"}), false),
            ("paused", json!({"parkedEvent":"ready"}), false),
            ("failed", json!({"parkedEvent":"ready"}), false),
            ("interrupted", json!({"parkedEvent":"ready"}), false),
            (
                "parked",
                json!({"parkedEvent":"ready","archived":true}),
                false,
            ),
        ] {
            let db = studio_runtime_store::test_canvas();
            put(&db, "root", None, "waiting", &json!({}));
            let mut fields = json!({"autoWake":false});
            fields
                .as_object_mut()
                .expect("fields")
                .extend(extra.as_object().expect("extra").clone());
            put(&db, "child", Some("root"), status, &fields);
            let monitor = crate::monitor_policy::complete_test_monitor(
                json!({"id":"watch","agent":"child","status":"running","epoch":1}),
            );
            db.execute(
                "INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",
                [monitor.to_string()],
            )
            .expect("monitor");
            let contexts = load(&db, &["root".into()]).expect("facts");
            assert_eq!(
                count(&contexts["root"]),
                u32::from(live),
                "{status} {extra}"
            );
            if live {
                assert_eq!(
                    contexts["child"]
                        .wait
                        .as_ref()
                        .expect("event")
                        .facts()
                        .event
                        .as_deref(),
                    Some("ready")
                );
            }
        }
        for (parent, status, extra, live) in [
            ("root", "waiting", json!({}), true),
            ("root", "paused", json!({"autoWake":false}), false),
            ("root", "failed", json!({}), false),
            ("root", "waiting", json!({"archived":true}), false),
            ("root", "waiting", json!({"autoWake":false}), false),
            ("root", "waiting", json!({"source":"local"}), false),
            ("other-root", "waiting", json!({}), false),
        ] {
            let db = studio_runtime_store::test_canvas();
            put(&db, "root", None, "waiting", &json!({}));
            put(&db, "middle", Some(parent), status, &extra);
            put(&db, "child", Some("middle"), "waiting", &json!({}));
            let monitor = crate::monitor_policy::complete_test_monitor(
                json!({"id":"watch","agent":"child","status":"running","epoch":1}),
            );
            db.execute(
                "INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",
                [monitor.to_string()],
            )
            .expect("monitor");
            let contexts = load(&db, &["root".into()]).expect("facts");
            assert_eq!(
                count(&contexts["root"]),
                u32::from(live),
                "{parent} {status} {extra}"
            );
        }
        let db = studio_runtime_store::test_canvas();
        put(&db, "root", None, "waiting", &json!({}));
        put(&db, "middle", Some("child"), "waiting", &json!({}));
        put(&db, "child", Some("middle"), "waiting", &json!({}));
        let monitor = crate::monitor_policy::complete_test_monitor(
            json!({"id":"watch","agent":"child","status":"running","epoch":1}),
        );
        db.execute(
            "INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",
            [monitor.to_string()],
        )
        .expect("monitor");

        let contexts = load(&db, &["middle".into()]).expect("cycle terminates");
        assert_eq!(
            count(&contexts["middle"]),
            1,
            "a parent cycle never counts the requesting agent itself"
        );
        let db = studio_runtime_store::test_canvas();
        put(&db, "root", None, "waiting", &json!({}));
        put(&db, "middle", Some("root"), "waiting", &json!({}));
        put(&db, "sibling", Some("root"), "waiting", &json!({}));
        let monitor = crate::monitor_policy::complete_test_monitor(
            json!({"id":"watch","agent":"sibling","status":"running","epoch":1}),
        );
        db.execute(
            "INSERT INTO runtime_monitors(id,record) VALUES('watch',?1)",
            [monitor.to_string()],
        )
        .expect("monitor");
        let contexts = load(&db, &["root".into()]).expect("facts");
        assert_eq!(
            count(&contexts["middle"]),
            0,
            "sibling activity stays outside the branch"
        );
        assert_eq!(count(&contexts["root"]), 1);
    }
    #[test]
    fn v5_wide_roster_1024_uses_one_batch() {
        let db = studio_runtime_store::test_canvas();
        db.execute_batch("BEGIN").expect("fixture transaction");
        put(&db, "root", None, "waiting", &json!({}));
        for index in 0..1024 {
            put(
                &db,
                &format!("worker-{index}"),
                Some("root"),
                "running",
                &json!({}),
            );
        }
        db.execute_batch("COMMIT").expect("fixtures");
        let contexts = load(&db, &["root".into()]).expect("one batch");
        assert_eq!(
            contexts["root"]
                .wait
                .as_ref()
                .expect("roster")
                .facts()
                .agents,
            1024
        );
        assert_eq!(contexts.len(), 1025);
    }
    #[test]
    fn v5_children_prune_unmanaged_and_stopped_branches_but_root_activity_uses_root_owner() {
        let db = studio_runtime_store::test_canvas();
        put(&db, "root", None, "waiting", &json!({}));
        put(
            &db,
            "unmanaged",
            Some("root"),
            "running",
            &json!({"source":"registered"}),
        );
        let contexts = load(&db, &["root".into()]).expect("facts");
        assert!(
            contexts["root"].wait.is_none(),
            "unmanaged children cannot create a wait trigger"
        );
        db.execute("DELETE FROM runtime_agents WHERE id='unmanaged'", [])
            .expect("remove");
        put(&db, "wrong-parent", Some("other"), "running", &json!({}));
        let contexts = load(&db, &["root".into()]).expect("facts");
        assert!(contexts["root"].wait.is_none());
        assert!(
            contexts["root"].active,
            "root-owned activity propagates even without a direct child edge"
        );
        put(&db, "stopped", Some("root"), "paused", &json!({}));
        put(&db, "grandchild", Some("stopped"), "running", &json!({}));
        let contexts = load(&db, &["root".into(), "stopped".into()]).expect("facts");
        assert!(
            !contexts["stopped"].active,
            "activity belongs to its owner and root, not every ancestor"
        );
        assert!(
            contexts["root"].wait.is_none(),
            "stopped branches prune descendant triggers"
        );
    }
    #[test]
    fn v5_process_owner_status_turn_and_kind_matrix() {
        use studio_runtime_schema::task_record::{
            NonnegativeNumber, TaskRecord, TaskStatus, TaskType,
        };
        let db = studio_runtime_store::test_canvas();
        put(
            &db,
            "root",
            None,
            "completed",
            &json!({"inFlight":false,"turnId":"current"}),
        );
        for (status, live) in [
            ("starting", true),
            ("running", true),
            ("approval", true),
            ("completed", false),
            ("failed", false),
            ("cancelled", false),
            ("lost", false),
        ] {
            for owner in ["root", "other"] {
                db.execute("DELETE FROM runtime_monitors", [])
                    .expect("clear");
                let id = format!("watch-{owner}");
                let monitor = crate::monitor_policy::complete_test_monitor(
                    json!({"id":id,"agent":owner,"status":status,"epoch":999}),
                );
                db.execute(
                    "INSERT INTO runtime_monitors(id,record) VALUES(?1,?2)",
                    [id, monitor.to_string()],
                )
                .expect("monitor");
                let contexts = load(&db, &["root".into()]).expect("facts");
                assert_eq!(
                    contexts["root"]
                        .wait
                        .as_ref()
                        .map_or(0, |w| w.facts().monitors),
                    u32::from(live && owner == "root"),
                    "{status} {owner}: monitor epoch is not an ownership condition"
                );
            }
        }
        db.execute("DELETE FROM runtime_monitors", [])
            .expect("clear");
        for (status, live) in [
            (TaskStatus::Starting, true),
            (TaskStatus::Running, true),
            (TaskStatus::Approval, true),
            (TaskStatus::Completed, false),
            (TaskStatus::Failed, false),
            (TaskStatus::Interrupted, false),
            (TaskStatus::Lost, false),
            (TaskStatus::Pending, false),
            (TaskStatus::Unknown, false),
        ] {
            db.execute("DELETE FROM runtime_tasks", []).expect("clear");
            let mut task = TaskRecord::new(
                "root".into(),
                "call".into(),
                "current".into(),
                NonnegativeNumber::from_f64(0.0).expect("time"),
                TaskType::CommandExecution,
                String::new(),
            );
            task.set_status(status);
            put_task(&db, &task);
            let contexts = load(&db, &["root".into()]).expect("facts");
            assert_eq!(
                contexts["root"]
                    .wait
                    .as_ref()
                    .map_or(0, |w| w.facts().commands),
                u32::from(live),
                "{status:?}: commandless command remains a trigger"
            );
        }
        for (kind, owner, turn, in_flight, expected) in [
            (TaskType::CommandExecution, "root", "current", true, 0),
            (TaskType::CommandExecution, "root", "old", true, 1),
            (TaskType::CommandExecution, "root", "", false, 1),
            (TaskType::DynamicToolCall, "root", "current", false, 0),
            (TaskType::CommandExecution, "other", "current", false, 0),
        ] {
            db.execute("DELETE FROM runtime_tasks", []).expect("clear");
            put(
                &db,
                "root",
                None,
                "completed",
                &json!({"inFlight":in_flight,"turnId":"current"}),
            );
            let task = TaskRecord::new(
                owner.into(),
                "call".into(),
                turn.into(),
                NonnegativeNumber::from_f64(0.0).expect("time"),
                kind,
                String::new(),
            );
            put_task(&db, &task);
            let contexts = load(&db, &["root".into()]).expect("facts");
            assert_eq!(
                contexts["root"]
                    .wait
                    .as_ref()
                    .map_or(0, |w| w.facts().commands),
                expected,
                "{kind:?} owner={owner} turn={turn} inFlight={in_flight}"
            );
        }
    }

    #[test]
    fn v5_request_epoch_state_and_empty_panel_attention_matrix() {
        use studio_runtime_schema::approvals::{
            Answer, ApprovalId, ApprovalState, Deferral, Principal, Timestamp,
        };
        let connection = studio_runtime_store::test_canvas();
        let db = connection
            .unchecked_transaction()
            .expect("fixture transaction");
        put(&db, "root", None, "approval", &json!({}));
        for epoch in [None, Some(1), Some(2)] {
            for state in ["pending", "answered", "deferred"] {
                db.execute("DELETE FROM approvals", []).expect("clear");
                let mut new = crate::entity_publication::tests::new_question("input");
                new.agent = studio_runtime_schema::approvals::AgentId("root".into());
                new.epoch = epoch;
                studio_runtime_store::approvals::create(&db, new, Timestamp(1.0)).expect("input");
                let mut approval =
                    studio_runtime_store::approvals::get(&db, &ApprovalId("input".into()))
                        .expect("read")
                        .expect("approval");
                if state == "answered" {
                    approval.state = ApprovalState::Answered {
                        answer: Answer::Answers { answers: vec![] },
                        by: Principal::Owner,
                        at: Timestamp(2.0),
                    };
                }
                if state == "deferred" {
                    approval.deferral = Some(Deferral {
                        active: true,
                        at: Timestamp(2.0),
                        by: Principal::Owner,
                    });
                }
                studio_runtime_store::approvals::put(&db, &approval).expect("state");
                let contexts = load(&db, &["root".into()]).expect("no client panels required");
                let current = epoch != Some(2);
                assert_eq!(
                    contexts["root"].answer,
                    current && state == "pending",
                    "epoch={epoch:?} {state}"
                );
                assert_eq!(contexts["root"].deferred, current && state == "deferred");
                assert_eq!(
                    contexts["root"]
                        .wait
                        .as_ref()
                        .map_or(0, |w| w.facts().inputs),
                    u32::from(current && state == "pending")
                );
            }
        }
    }
    #[test]
    fn v5_orphan_input_is_local_and_empty_root_never_propagates_to_an_empty_identity() {
        use studio_runtime_schema::approvals::{AgentId, Timestamp};
        let connection = studio_runtime_store::test_canvas();
        let db = connection
            .unchecked_transaction()
            .expect("fixture transaction");
        put(
            &db,
            "orphan",
            Some("missing-parent"),
            "paused",
            &json!({"rootId":"missing-root"}),
        );
        let mut input = crate::entity_publication::tests::new_question("orphan-input");
        input.agent = AgentId("orphan".into());
        input.epoch = Some(1);
        studio_runtime_store::approvals::create(&db, input, Timestamp(1.0)).expect("orphan input");
        assert!(load(&db, &["orphan".into()]).expect("facts")["orphan"].answer);
        put(&db, "", None, "idle", &json!({"rootId":""}));
        put(&db, "regular", None, "paused", &json!({"rootId":""}));
        let mut input = crate::entity_publication::tests::new_question("regular-input");
        input.agent = AgentId("regular".into());
        input.epoch = Some(1);
        studio_runtime_store::approvals::create(&db, input, Timestamp(1.0)).expect("regular input");
        let contexts = load(&db, &["regular".into(), "".into()]).expect("facts");
        assert!(contexts["regular"].answer);
        assert!(
            !contexts[""].answer,
            "empty root identity cannot route another actor's input"
        );
    }

    #[test]
    fn v5_root_activity_autowake_and_inflight_matrix() {
        let db = studio_runtime_store::test_canvas();
        for status in ["queued", "starting", "running"] {
            for auto_wake in [false, true] {
                for in_flight in [false, true] {
                    put(
                        &db,
                        "root",
                        None,
                        status,
                        &json!({"autoWake":auto_wake,"inFlight":in_flight}),
                    );
                    let contexts = load(&db, &["root".into()]).expect("facts");
                    let mut summary = json!({"id":"root","status":status,"autoWake":auto_wake,"inFlight":in_flight,"empty":false,"readStateSupported":true});
                    decorate(&mut summary, contexts.get("root")).expect("summary");
                    assert_eq!(
                        summary["operationalStatus"]["reason"],
                        if auto_wake || in_flight {
                            "working"
                        } else {
                            "stopped"
                        },
                        "{status} autoWake={auto_wake} inFlight={in_flight}"
                    );
                }
            }
        }
    }
    #[test]
    fn v5_stopped_child_process_and_input_keep_root_attention_without_child_wait() {
        use studio_runtime_schema::{
            approvals::{AgentId, Timestamp},
            task_record::{NonnegativeNumber, TaskRecord, TaskType},
        };
        let connection = studio_runtime_store::test_canvas();
        let db = connection
            .unchecked_transaction()
            .expect("fixture transaction");
        put(&db, "root", None, "waiting", &json!({}));
        put(
            &db,
            "child",
            Some("root"),
            "paused",
            &json!({"autoWake":false}),
        );
        let task = TaskRecord::new(
            "child".into(),
            "call".into(),
            String::new(),
            NonnegativeNumber::from_f64(0.0).expect("time"),
            TaskType::CommandExecution,
            String::new(),
        );
        studio_runtime_store::TaskRepository::put(&db, &task).expect("task");
        let contexts = load(&db, &["root".into()]).expect("facts");
        assert!(contexts["root"].active);
        assert!(contexts["root"].wait.is_none());
        let mut input = crate::entity_publication::tests::new_question("child-input");
        input.agent = AgentId("child".into());
        input.epoch = Some(1);
        studio_runtime_store::approvals::create(&db, input, Timestamp(1.0)).expect("input");
        let contexts = load(&db, &["root".into()]).expect("facts");
        let mut summary = json!({"id":"root","status":"waiting","autoWake":true});
        decorate(&mut summary, contexts.get("root")).expect("summary");
        assert_eq!(
            summary["operationalStatus"],
            json!({"reason":"needs_input","wait":null})
        );
    }
    #[test]
    fn v5_unrelated_complaints_and_work_are_not_operational_inputs() {
        let db = studio_runtime_store::test_canvas();
        put(&db, "root", None, "idle", &json!({}));
        let tx = db.unchecked_transaction().expect("fixture transaction");
        for collection in ["complaint", "work"] {
            studio_runtime_store::SyncDocumentsRepository::put_entity(
                &tx,
                collection,
                "unrelated",
                json!({"id":"unrelated","owner":"other","status":"pending"}),
                false,
            )
            .expect("unrelated entity");
        }
        tx.commit().expect("fixtures");
        let contexts = load(&db, &["root".into()]).expect("facts");
        assert!(!contexts["root"].answer);
        assert!(!contexts["root"].active);
        assert!(contexts["root"].wait.is_none());
    }
    #[test]
    fn v5_disk_retry_requires_current_thread_epoch_account_and_unclaimed_schedule() {
        for (change, expected) in [
            (json!({}), "disk_space"),
            (json!({"threadId":"other"}), "failed"),
            (json!({"epoch":2}), "failed"),
            (json!({"accountKey":"other"}), "failed"),
            (json!({"status":"finished"}), "failed"),
            (json!({"acceptedTurnId":"new"}), "failed"),
            (json!({"cause":"serverOverloaded"}), "failed"),
        ] {
            let mut retry = json!({"id":"retry","threadId":"thread","epoch":1,"accountKey":"default","status":"scheduled","updatedAt":0,"dueAt":null,"cause":"diskFull"});
            retry
                .as_object_mut()
                .expect("retry")
                .extend(change.as_object().expect("change").clone());
            let mut summary = json!({"id":"root","status":"failed","threadId":"thread","epoch":1,"accountKey":"default","autoWake":true,"capacityRetry":retry});
            decorate(&mut summary, None).expect("summary");
            assert_eq!(summary["operationalStatus"]["reason"], expected, "{change}");
        }
    }
}
