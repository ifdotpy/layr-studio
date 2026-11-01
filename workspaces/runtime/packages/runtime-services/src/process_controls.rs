//! Process identities and durable action receipts shared with the Python backend.
//! Listing is read-only; native effects are driven only after an action is claimed.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use studio_runtime_providers::ProviderKind;
use studio_runtime_schema::process_controls::{
    BackgroundTaskAction, BackgroundTaskDisplay, BackgroundTaskIndicator, BackgroundTaskKind,
    BackgroundTaskKindLabel, BackgroundTaskLabel, BackgroundTaskView, ProcessActionResult,
    ProcessActionStatus, ProcessControlRequest, ProcessEntry, ProcessListing, ProcessOutcome,
    ProcessReceipt, ProcessResponseTarget, ProcessSourceKind, ProcessStatus, ProcessStdinState,
    ProcessView, ProcessWakeBudget, TaskProjection,
};
use studio_runtime_store::{
    ProcessActionKind, ProcessActionPhase, ProcessActionRecord, StoreError, Transaction,
};

use crate::account_session_runtime::ConnectionScope;
use crate::event_write::{CallerAdmission, RequestKey, begin_request};

/// Public view plus native identity evidence, retained together until dispatch.
#[derive(Clone, Debug)]
pub struct ProcessRecord {
    pub view: ProcessView,
    pub source_account: Option<String>,
    pub source_connection: Option<String>,
    pub source_thread: Option<String>,
    pub source_generation: Option<u64>,
    pub monitor_status: Option<studio_runtime_schema::studio_errors::MonitorStatus>,
    pub reattached_connection: Option<String>,
    pub item_id: Option<String>,
    pub process_id: Option<studio_runtime_schema::task_record::ProcessId>,
}

fn display(
    kind: BackgroundTaskKind,
    kind_label: BackgroundTaskKindLabel,
    title: String,
    status: ProcessStatus,
    cancelling: bool,
    in_background_list: bool,
    controls: (Vec<BackgroundTaskAction>, Option<ProcessStdinState>),
) -> BackgroundTaskDisplay {
    let (mut actions, stdin_state) = controls;
    use BackgroundTaskIndicator as Indicator;
    use BackgroundTaskLabel as Label;
    use studio_runtime_schema::studio_errors::MonitorStatus as Monitor;
    use studio_runtime_schema::task_record::TaskStatus as Task;
    let status_label = match status {
        ProcessStatus::Task(Task::Starting) | ProcessStatus::Monitor(Monitor::Starting) => {
            Label::Starting
        }
        ProcessStatus::Task(Task::Approval) | ProcessStatus::Monitor(Monitor::Approval) => {
            Label::Approval
        }
        ProcessStatus::Task(Task::Running) | ProcessStatus::Monitor(Monitor::Running) => {
            Label::Running
        }
        ProcessStatus::Task(Task::Completed) | ProcessStatus::Monitor(Monitor::Completed) => {
            Label::Completed
        }
        ProcessStatus::Task(Task::Failed) | ProcessStatus::Monitor(Monitor::Failed) => {
            Label::Failed
        }
        ProcessStatus::Task(Task::Lost) | ProcessStatus::Monitor(Monitor::Lost) => Label::Lost,
        ProcessStatus::Task(Task::Pending) => Label::Pending,
        ProcessStatus::Task(Task::Unknown) => Label::Unknown,
        ProcessStatus::Task(Task::Interrupted) => Label::Interrupted,
        ProcessStatus::Monitor(Monitor::Cancelled) => Label::Cancelled,
    };
    let active = matches!(
        status_label,
        Label::Starting | Label::Approval | Label::Running | Label::Pending
    );
    let indicator = match status_label {
        Label::Approval => Indicator::Answer,
        Label::Failed | Label::Lost => Indicator::Error,
        Label::Completed => Indicator::Unread,
        Label::Starting | Label::Running | Label::Pending => Indicator::Working,
        Label::Stopping
        | Label::Cancelled
        | Label::Interrupted
        | Label::WaitingForExit
        | Label::Unknown => Indicator::Paused,
    };
    let label = if active && cancelling {
        Label::WaitingForExit
    } else {
        status_label
    };
    actions.retain(|action| match action {
        BackgroundTaskAction::Stop => active && !cancelling,
        BackgroundTaskAction::Input => {
            !cancelling
                && matches!(status_label, Label::Running)
                && stdin_state == Some(ProcessStdinState::Open)
        }
        BackgroundTaskAction::Resize => !cancelling && matches!(status_label, Label::Running),
        BackgroundTaskAction::Approve | BackgroundTaskAction::ReadOutput => true,
    });
    if matches!(status_label, Label::Approval) {
        actions.push(BackgroundTaskAction::Approve);
    }
    BackgroundTaskDisplay {
        kind,
        kind_label,
        title,
        active,
        indicator,
        label,
        in_background_list,
        actions,
        stdin_state,
    }
}

fn task_title(command: Option<&str>, query: Option<&str>, name: Option<&str>) -> String {
    if let Some(value) = command
        .filter(|value| !value.is_empty())
        .or(query.filter(|value| !value.is_empty()))
    {
        return value.to_owned();
    }
    match name.filter(|value| !value.is_empty()) {
        Some("commandExecution") => "Command".into(),
        Some("webSearch") => "Web search".into(),
        Some("mcpToolCall") => "Connected tool".into(),
        Some("dynamicToolCall") => "Tool call".into(),
        Some("contextCompaction") => "Context compaction".into(),
        Some("fileChange") => "File changes".into(),
        Some(value) => value.replace('_', " "),
        None => "Tool call".into(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request id expired")]
    Expired,
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Store(#[from] StoreError),
}
/// Separate the current model connection from live source-process scopes.
/// These are cached observations, not permission to start or replay a command.
#[derive(Clone, Default)]
pub struct Connections {
    owners: BTreeMap<String, String>,
    sources: BTreeMap<String, ConnectionScope>,
    scopes: BTreeMap<String, ConnectionScope>,
    responses: BTreeMap<String, HeldResponseProof>,
}
impl FromIterator<(String, String)> for Connections {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self {
            owners: iter.into_iter().collect(),
            sources: BTreeMap::new(),
            scopes: BTreeMap::new(),
            responses: BTreeMap::new(),
        }
    }
}
impl<const N: usize> From<[(String, String); N]> for Connections {
    fn from(value: [(String, String); N]) -> Self {
        value.into_iter().collect()
    }
}
impl Connections {
    pub fn get(&self, owner: &str) -> Option<&String> {
        self.owners.get(owner)
    }
    pub fn observe_source(&mut self, scope: ConnectionScope) {
        self.sources.insert(scope.account.clone(), scope);
    }
    pub fn observe_owner(&mut self, owner: String, scope: ConnectionScope) {
        self.owners.insert(owner.clone(), scope.connection.clone());
        self.scopes.insert(owner, scope);
    }
    pub fn observe_response(&mut self, proof: HeldResponseProof) {
        self.responses
            .insert(text(&proof.owner, "id", "").into(), proof);
    }
}

/// Candidate metadata is not native proof. Only a fresh, exact-scope compact
/// thread read can authorize Stop, and its turn remains conditional at native dispatch.
pub fn held_response_candidate(owner: &Value) -> bool {
    owner["provider"] == "claude"
        && owner["executionMode"] != "vm"
        && owner["status"] == "interrupted"
        && owner["inFlight"] == false
        && owner["autoWake"] == false
        && owner["deleted"] != true
        && !studio_runtime_store::truthy(owner.get("deletedAt"))
        && owner["archived"] != true
        && !studio_runtime_store::truthy(owner.get("archivedAt"))
        && owner["accountTransferId"].is_null()
        && owner["executionMove"].is_null()
        && owner["epoch"].as_u64().is_some()
        && !text(owner, "id", "").is_empty()
        && !text(owner, "threadId", "").is_empty()
        && !text(owner, "turnId", "").is_empty()
        && owner["nativeResponseTurn"] == owner["turnId"]
        && owner["restartRecovery"]["stage"] == "held"
        && owner["disconnectRecovery"]["source"] == "restart"
        && ["restartRecovery", "disconnectRecovery"].iter().all(|key| {
            owner[key]["epoch"] == owner["epoch"]
                && owner[key]["accountKey"] == text(owner, "accountKey", "default")
                && owner[key]["threadId"] == owner["threadId"]
        })
}

/// Ephemeral observation, never a stored lifecycle transition or delivery permit.
/// Revalidation binds the unchanged actor and complete current native scope.
#[derive(Clone)]
pub struct HeldResponseProof {
    owner: Value,
    scope: ConnectionScope,
}
impl HeldResponseProof {
    pub fn observe(owner: Value, scope: ConnectionScope, reply: &Value) -> Option<Self> {
        (held_response_candidate(&owner)
            && scope.account == text(&owner, "accountKey", "default")
            && !scope.connection.is_empty()
            && reply["thread"]["id"] == owner["threadId"]
            && reply["thread"]["status"]["type"] == "active")
            .then_some(Self { owner, scope })
    }
    fn matches(&self, owner: &Value, scope: &ConnectionScope) -> bool {
        self.owner == *owner && self.scope == *scope && held_response_candidate(owner)
    }
}

fn retained_monitor_scope(
    tx: &Transaction<'_>,
    monitor: &studio_runtime_store::MonitorRecord,
    owner: &Value,
) -> Result<Option<ConnectionScope>, Error> {
    if monitor.status != studio_runtime_schema::studio_errors::MonitorStatus::Running
        || monitor.agent != text(owner, "id", "")
        || Some(monitor.epoch.get()) != owner["epoch"].as_i64()
        || owner["deleted"] == true
        || studio_runtime_store::truthy(owner.get("deletedAt"))
    {
        return Ok(None);
    }
    let Some(operation) = &monitor.operation else {
        return Ok(None);
    };
    if operation.agent.as_str() != text(owner, "id", "")
        || Some(operation.epoch.get()) != owner["epoch"].as_i64()
    {
        return Ok(None);
    }
    match crate::account_worker::retained::vm_actor(tx, operation.agent.as_str()) {
        Ok(false) => {}
        Err(crate::account_worker::WorkerError::Store(error)) => return Err(Error::Store(error)),
        _ => return Ok(None),
    }
    match crate::account_worker::retained::monitor_command_scope(tx, &monitor.id, operation) {
        Ok(scope) => Ok(scope),
        Err(crate::account_worker::WorkerError::Store(error)) => Err(Error::Store(error)),
        _ => Ok(None),
    }
}

/// The source accounts of proven running host monitors. The HTTP caller only
/// observes those accounts' existing sessions; it never starts another session.
pub fn monitor_sources(
    tx: &Transaction<'_>,
    rows: &[(&Value, ProcessSource)],
) -> Result<Vec<ConnectionScope>, Error> {
    let mut scopes = BTreeMap::new();
    for (owner, source) in rows {
        if let ProcessSource::Monitor(monitor) = source
            && let Some(scope) = retained_monitor_scope(tx, monitor, owner)?
        {
            scopes.insert(scope.account.clone(), scope);
        }
    }
    Ok(scopes.into_values().collect())
}

/// Projection of an existing command, authorized by its original command ledger
/// and an observed live source scope, never by the actor's new model thread.
pub fn project_monitor(
    tx: &Transaction<'_>,
    raw: &studio_runtime_store::MonitorRecord,
    owner: &Value,
    owner_connection: Option<&str>,
    source: Option<&ConnectionScope>,
) -> Result<ProcessRecord, Error> {
    let proven = match (retained_monitor_scope(tx, raw, owner)?, source) {
        (Some(original), Some(live))
            if live.account == original.account
                && live.generation == original.generation
                && !live.connection.is_empty()
                && (live.connection == original.connection
                    || raw.reattached_connection_id.as_deref()
                        == Some(live.connection.as_str())) =>
        {
            Some(live)
        }
        _ => None,
    };
    Ok(project_monitor_bound(
        raw,
        owner,
        proven
            .map(|scope| scope.connection.as_str())
            .or(owner_connection),
        proven.map(|scope| scope.generation),
    ))
}
fn project_monitor_bound(
    monitor: &studio_runtime_store::MonitorRecord,
    owner: &Value,
    connection: Option<&str>,
    generation: Option<u64>,
) -> ProcessRecord {
    use studio_runtime_schema::studio_errors::MonitorStatus;
    let agent = monitor.agent.as_str();
    let account = text(owner, "accountKey", "default");
    let pending = matches!(
        monitor.status,
        MonitorStatus::Approval | MonitorStatus::Starting
    );
    let operation = monitor.operation.as_ref();
    let source_account = operation
        .map(|op| op.account_key.as_str())
        .or(pending.then_some(account));
    let mut source_connection = operation.map(|op| op.connection_id.as_str());
    let mut source_thread = operation.and_then(|op| op.evidence.thread_id.as_deref());
    if generation.is_some() {
        source_connection = connection;
    } else if monitor.status == MonitorStatus::Running
        && connection.is_some()
        && monitor.reattached_connection_id.as_deref() == connection
    {
        source_connection = connection;
        source_thread = owner["threadId"].as_str();
    }
    if generation.is_none()
        && source_thread.is_none_or(str::is_empty)
        && operation.map(|op| op.connection_id.as_str()) == connection
    {
        source_thread = owner["threadId"].as_str();
    }
    let epoch = monitor.epoch.get();
    let mut native = json!([
        monitor.id,
        epoch,
        source_account,
        source_connection,
        source_thread
    ]);
    if let Some(generation) = generation {
        native
            .as_array_mut()
            .expect("identity")
            .push(json!(generation));
    }
    let same = (generation.is_some() || source_account == Some(account))
        && owner["epoch"].as_i64() == Some(epoch)
        && connection.is_some_and(|c| !c.is_empty())
        && source_connection == connection
        && (generation.is_some()
            || (source_thread.is_some_and(|v| !v.is_empty())
                && source_thread == owner["threadId"].as_str()));
    let output = monitor.tail.as_str();
    let bytes = monitor.bytes.get() as u64;
    let command = monitor.command.as_str();
    let name = if command.is_empty() {
        "Command monitor"
    } else {
        command
    };
    let mut actions = vec![BackgroundTaskAction::ReadOutput];
    actions.extend(crate::monitor_policy::input_actions(
        monitor,
        same && owner["autoWake"] == true,
    ));
    if same
        || (pending && owner["epoch"].as_i64() == Some(epoch) && source_account == Some(account))
    {
        actions.push(BackgroundTaskAction::Stop);
    }
    let cancelled = monitor.cancel_requested == Some(true);
    let status = ProcessStatus::Monitor(monitor.status);
    let view = ProcessView {
        id: monitor.id.clone(),
        key: URL_SAFE_NO_PAD.encode(json!(["monitor", agent, monitor.id]).to_string()),
        generation: digest(&json!(["monitor", agent, native])),
        agent: agent.to_owned(),
        account_key: account.to_owned(),
        epoch: owner["epoch"].as_u64().expect("checked owner epoch"),
        source: ProcessSourceKind::Monitor,
        status,
        cancel_requested: cancelled,
        created: monitor.created.seconds(),
        name: name.to_owned(),
        command: Some(command.to_owned()),
        cwd: Some(monitor.cwd.clone()),
        log: Some(monitor.log.clone()),
        tail: output.to_owned(),
        duration_ms: None,
        finished: None,
        exit_code: None,
        process_id: None,
        timeout_ms: None,
        error: monitor.error.as_ref().map(|v| v.as_str().to_owned()),
        stdin_error: monitor.stdin_error.clone(),
        bytes,
        output_truncated: monitor
            .output_truncated
            .unwrap_or(bytes > output.len() as u64),
        output_file: None,
        output_patterns: monitor.output_patterns.clone().unwrap_or_default(),
        output_wake_budget: monitor
            .output_wake_budget
            .as_ref()
            .map(|budget| ProcessWakeBudget {
                total_suppressed_matches: budget.total_suppressed_matches,
                paused: budget.paused,
                tokens: budget.tokens,
            }),
        display: display(
            BackgroundTaskKind::Monitor,
            BackgroundTaskKindLabel::Monitor,
            task_title(Some(command), None, Some(name)),
            status,
            cancelled,
            true,
            (actions, monitor_stdin_state(monitor)),
        ),
    };
    ProcessRecord {
        view,
        source_account: source_account.map(str::to_owned),
        source_connection: source_connection.map(str::to_owned),
        source_thread: source_thread.map(str::to_owned),
        source_generation: generation,
        monitor_status: Some(monitor.status),
        reattached_connection: monitor.reattached_connection_id.clone(),
        item_id: None,
        process_id: None,
    }
}

fn raw(connection: &Connection, table: &str, id: &str) -> Result<Option<Value>, Error> {
    let text: Option<String> = connection
        .query_row(
            &format!("SELECT record FROM {table} WHERE id=?"),
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)?;
    text.map(|text| {
        serde_json::from_str(&text)
            .map_err(StoreError::from)
            .map_err(Error::from)
    })
    .transpose()
}
pub fn owner(connection: &Connection, scope: &str) -> Result<Value, Error> {
    let root =
        raw(connection, "runtime_agents", scope)?.ok_or(Error::Invalid("Unknown process owner"))?;
    if studio_runtime_store::truthy(root.get("deletedAt")) {
        return Err(Error::Invalid("This process owner was deleted"));
    }
    Ok(root)
}
pub fn owners(connection: &Connection, scope: &str) -> Result<Vec<Value>, Error> {
    let root = owner(connection, scope)?;
    if root["id"].as_str().is_none() {
        return Err(Error::Invalid("Invalid process owner"));
    }
    let mut statement = connection
        .prepare(
            "WITH RECURSIVE descendants(id) AS (
             SELECT ?1 UNION SELECT child.id FROM runtime_agents child
             JOIN descendants parent ON child.parent_id=parent.id
             WHERE child.deleted_at IS NULL
         ) SELECT agent.record FROM descendants JOIN runtime_agents agent ON agent.id=descendants.id
           WHERE descendants.id != ?1",
        )
        .map_err(StoreError::from)?;
    let rows = statement
        .query_map([scope], |row| row.get::<_, String>(0))
        .map_err(StoreError::from)?;
    let mut result = vec![root];
    for row in rows {
        result
            .push(serde_json::from_str(&row.map_err(StoreError::from)?).map_err(StoreError::from)?);
    }
    Ok(result)
}

/// A process source retains its owning record type through selection.
pub enum ProcessSource {
    Monitor(Box<studio_runtime_store::MonitorRecord>),
    Task(Box<studio_runtime_store::TaskRecord>),
}

/// Scoped active commands paired with their already-loaded owners.
pub fn process_rows<'a>(
    connection: &Connection,
    owners: &'a [Value],
) -> Result<Vec<(&'a Value, ProcessSource)>, Error> {
    let mut result = Vec::new();
    for table in ["runtime_monitors", "runtime_tasks"] {
        let (index, status) = if table == "runtime_monitors" {
            ("runtime_monitor_agent_status", "IN (?2,?3,?4)")
        } else {
            ("runtime_task_agent_created_id", "= ?2")
        };
        let sql = format!(
            "SELECT id,record FROM {table} INDEXED BY {index} WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') {status}"
        );
        let mut statement = connection.prepare_cached(&sql).map_err(StoreError::from)?;
        for owner in owners {
            let id = text(owner, "id", "");
            let statuses = studio_runtime_store::startup_recovery::ACTIVE_MONITOR_STATUSES;
            let args = [id, statuses[0], statuses[1], statuses[2]];
            let args = if table == "runtime_monitors" {
                &args[..]
            } else {
                &args[..2]
            };
            let rows = statement
                .query_map(rusqlite::params_from_iter(args), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(StoreError::from)?;
            for row in rows {
                let (row_id, raw) = row.map_err(StoreError::from)?;
                let source = if table == "runtime_monitors" {
                    ProcessSource::Monitor(Box::new(
                        serde_json::from_str(&raw).map_err(StoreError::from)?,
                    ))
                } else {
                    let record: studio_runtime_store::TaskRecord =
                        serde_json::from_str(&raw).map_err(StoreError::from)?;
                    if record.id().as_ref() != row_id {
                        return Err(StoreError::InvalidStoredObject {
                            table: "runtime_tasks",
                            id: row_id,
                        }
                        .into());
                    }
                    ProcessSource::Task(Box::new(record))
                };
                result.push((owner, source));
            }
        }
    }
    Ok(result)
}

fn text<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or(default)
}
pub fn digest(value: &Value) -> String {
    studio_runtime_store::sha256_hex(value.to_string().as_bytes())
}
fn request_digest(request: &ProcessControlRequest) -> Result<String, Error> {
    let bytes = serde_json::to_vec(request).map_err(StoreError::from)?;
    Ok(studio_runtime_store::sha256_hex(&bytes))
}
pub fn project(
    source: ProcessSourceKind,
    raw: &studio_runtime_store::TaskRecord,
    owner: &Value,
    connection: Option<&str>,
) -> ProcessRecord {
    let agent = raw.agent();
    let account = text(owner, "accountKey", "default");
    let source_account = raw.source_account_key.as_deref();
    let source_connection = raw.source_connection_id.as_deref();
    let source_thread = raw.source_thread_id.as_deref();
    let epoch = raw.owner_epoch;
    let claude = source == ProcessSourceKind::ClaudeTask;
    let item = raw.item_id();
    let opaque = if claude {
        item.strip_prefix("task:").unwrap_or(item)
    } else {
        raw.process_id()
            .and_then(|id| id.text())
            .filter(|id| !id.is_empty())
            .unwrap_or(item)
    };
    let native = if claude {
        json!([
            opaque,
            epoch,
            source_account,
            source_connection,
            source_thread,
            raw.turn_id()
        ])
    } else {
        json!([
            opaque,
            item,
            epoch,
            source_account,
            source_connection,
            source_thread,
            raw.turn_id()
        ])
    };
    let same = source_account == Some(account)
        && epoch == owner["epoch"].as_i64()
        && connection.is_some_and(|c| !c.is_empty())
        && source_connection == connection
        && source_thread.is_some_and(|thread| !thread.is_empty())
        && source_thread == owner["threadId"].as_str();
    let can_input = same
        && owner["autoWake"] == true
        && !claude
        && raw
            .process_id()
            .and_then(|id| id.text())
            .is_some_and(|id| !id.is_empty());
    let output = if claude {
        ""
    } else {
        raw.tail.as_deref().filter(|s| !s.is_empty()).unwrap_or("")
    };
    let bytes = output.len() as u64;
    let default_name = if claude {
        "Claude background task"
    } else {
        "Background command"
    };
    let name = if claude {
        Some(raw.name.as_str())
            .filter(|s| !s.is_empty())
            .or(raw.command.as_deref().filter(|s| !s.is_empty()))
    } else {
        raw.command
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(Some(raw.name.as_str()).filter(|s| !s.is_empty()))
    }
    .unwrap_or(default_name);
    let mut actions = Vec::new();
    if !claude || raw.output_file.is_some() {
        actions.push(BackgroundTaskAction::ReadOutput);
    }
    if can_input {
        actions.push(BackgroundTaskAction::Input);
    }
    if same {
        actions.push(BackgroundTaskAction::Stop);
    }
    let status = ProcessStatus::Task(raw.status());
    let view = ProcessView {
        id: raw.id().as_ref().to_owned(),
        key: URL_SAFE_NO_PAD.encode(json!([source.as_str(), agent, opaque]).to_string()),
        generation: digest(&json!([source.as_str(), agent, native])),
        agent: agent.to_owned(),
        account_key: account.to_owned(),
        epoch: owner["epoch"].as_u64().expect("checked owner epoch"),
        source,
        status,
        cancel_requested: false,
        created: raw.created(),
        name: name.into(),
        command: raw.command.clone(),
        cwd: raw.cwd.clone(),
        log: None,
        tail: output.into(),
        duration_ms: None,
        finished: None,
        exit_code: None,
        process_id: None,
        timeout_ms: None,
        error: raw.error.clone(),
        stdin_error: None,
        bytes,
        output_truncated: raw.output_truncated.unwrap_or(false),
        output_file: if claude {
            raw.output_file.clone()
        } else {
            None
        },
        output_patterns: Vec::new(),
        output_wake_budget: None,
        display: display(
            if claude {
                BackgroundTaskKind::BackgroundTask
            } else {
                BackgroundTaskKind::Command
            },
            if claude {
                BackgroundTaskKindLabel::Tool
            } else {
                BackgroundTaskKindLabel::BackgroundCommand
            },
            task_title(raw.command.as_deref(), raw.query.as_deref(), Some(name)),
            status,
            false,
            true,
            (
                actions,
                (!claude && raw.process_id().is_some()).then_some(ProcessStdinState::Open),
            ),
        ),
    };
    ProcessRecord {
        view,
        source_account: source_account.map(str::to_owned),
        source_connection: source_connection.map(str::to_owned),
        source_thread: source_thread.map(str::to_owned),
        source_generation: None,
        monitor_status: None,
        reattached_connection: None,
        item_id: Some(item.to_owned()),
        process_id: raw.process_id().cloned(),
    }
}

pub fn task_record_display(task: &studio_runtime_store::TaskRecord) -> BackgroundTaskDisplay {
    use studio_runtime_schema::task_record::TaskKind;
    let kind = if task.kind() == TaskKind::Command {
        BackgroundTaskKind::Command
    } else if task.item_id().starts_with("task:") {
        BackgroundTaskKind::BackgroundTask
    } else {
        BackgroundTaskKind::Tool
    };
    let in_background_list = matches!(
        kind,
        BackgroundTaskKind::Command | BackgroundTaskKind::BackgroundTask
    );
    let actions = vec![BackgroundTaskAction::ReadOutput];
    display(
        kind,
        if kind == BackgroundTaskKind::Command {
            BackgroundTaskKindLabel::Command
        } else {
            BackgroundTaskKindLabel::Tool
        },
        task_title(
            task.command.as_deref(),
            task.query.as_deref(),
            Some(&task.name),
        ),
        ProcessStatus::Task(task.status()),
        false,
        in_background_list,
        (actions, None),
    )
}

fn monitor_stdin_state(monitor: &studio_runtime_store::MonitorRecord) -> Option<ProcessStdinState> {
    if !monitor.interactive {
        None
    } else if monitor.stdin_closed == Some(true) {
        Some(ProcessStdinState::Closed)
    } else if monitor.stdin_close_requested.is_some() {
        Some(ProcessStdinState::CloseRequested)
    } else {
        Some(ProcessStdinState::Open)
    }
}

pub fn monitor_record_display(
    monitor: &studio_runtime_store::MonitorRecord,
) -> BackgroundTaskDisplay {
    let actions = vec![BackgroundTaskAction::ReadOutput];
    display(
        BackgroundTaskKind::Monitor,
        BackgroundTaskKindLabel::Monitor,
        task_title(Some(monitor.command.as_str()), None, None),
        ProcessStatus::Monitor(monitor.status),
        monitor.cancel_requested == Some(true),
        true,
        (actions, monitor_stdin_state(monitor)),
    )
}

pub fn processes(
    connection: &Transaction<'_>,
    rows: &[(&Value, ProcessSource)],
    connections: &Connections,
) -> Result<Vec<ProcessRecord>, Error> {
    let mut output = Vec::new();
    for (owner, row) in rows {
        let owner_connection = connections.get(text(owner, "id", "")).map(String::as_str);
        match row {
            ProcessSource::Monitor(monitor) => {
                let account = monitor
                    .operation
                    .as_ref()
                    .map(|op| op.account_key.as_str())
                    .unwrap_or("default");
                output.push(project_monitor(
                    connection,
                    monitor,
                    owner,
                    owner_connection,
                    connections.sources.get(account),
                )?);
            }
            ProcessSource::Task(raw) => {
                let provider = raw
                    .source_provider
                    .map(|provider| provider.key())
                    .unwrap_or_else(|| text(owner, "provider", ProviderKind::Codex.key()));
                let source = if raw.kind() == studio_runtime_schema::task_record::TaskKind::Command
                    && raw.process_id().is_some_and(|id| id.truthy())
                {
                    ProcessSourceKind::CodexTask
                } else if provider == ProviderKind::Claude.key()
                    && raw.kind() == studio_runtime_schema::task_record::TaskKind::Tool
                    && raw
                        .item_id()
                        .strip_prefix("task:")
                        .is_some_and(|v| !v.is_empty())
                {
                    ProcessSourceKind::ClaudeTask
                } else {
                    continue;
                };
                output.push(project(source, raw, owner, owner_connection));
            }
        }
    }
    output.sort_by(|a, b| {
        b.view
            .created
            .partial_cmp(&a.view.created)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.view.key.cmp(&b.view.key))
    });
    Ok(output)
}
pub fn response_target(owner: &Value, connection: Option<&str>) -> Option<ProcessResponseTarget> {
    if owner["inFlight"] == true {
        response_identity(owner, connection?)
    } else {
        None
    }
}
fn stop_target(owner: &Value, connections: &Connections) -> Option<ProcessResponseTarget> {
    let id = text(owner, "id", "");
    response_target(owner, connections.get(id).map(String::as_str)).or_else(|| {
        match (connections.responses.get(id), connections.scopes.get(id)) {
            (Some(proof), Some(scope)) if proof.matches(owner, scope) => {
                response_identity(owner, &scope.connection)
            }
            _ => None,
        }
    })
}
fn response_identity(owner: &Value, connection: &str) -> Option<ProcessResponseTarget> {
    Some(ProcessResponseTarget {
        agent: owner["id"].as_str()?.to_owned().try_into().ok()?,
        account_key: text(owner, "accountKey", "default")
            .to_owned()
            .try_into()
            .ok()?,
        epoch: owner["epoch"].as_u64()?,
        connection_id: connection.to_owned().try_into().ok()?,
        thread_id: owner["threadId"].as_str()?.to_owned().try_into().ok()?,
        turn_id: owner["turnId"].as_str()?.to_owned().try_into().ok()?,
    })
}

pub fn listing(
    connection: &Transaction<'_>,
    owners: &[Value],
    rows: &[(&Value, ProcessSource)],
    connections: &Connections,
) -> Result<ProcessListing, Error> {
    let processes: Vec<_> = processes(connection, rows, connections)?
        .into_iter()
        .map(|process| process.view)
        .collect();
    #[derive(serde::Deserialize)]
    struct TaskEntity {
        value: TaskProjection,
    }
    let owner_ids: HashSet<_> = owners
        .iter()
        .filter_map(|owner| owner["id"].as_str())
        .collect();
    let mut statement = connection
        .prepare("SELECT payload FROM sync_entities WHERE collection='task' AND deleted=0")
        .map_err(StoreError::from)?;
    let entities = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(StoreError::from)?;
    let mut tasks = Vec::new();
    for entity in entities {
        let entity: TaskEntity =
            serde_json::from_str(&entity.map_err(StoreError::from)?).map_err(StoreError::from)?;
        if owner_ids.contains(entity.value.agent.as_str()) && entity.value.display.active {
            tasks.push(entity.value);
        }
    }
    let activities = background_activities(&processes, &tasks);
    Ok(ProcessListing {
        response_target: stop_target(&owners[0], connections),
        processes,
        activities,
    })
}
fn background_activities(
    processes: &[ProcessView],
    tasks: &[TaskProjection],
) -> Vec<BackgroundTaskView> {
    let process_ids: HashSet<_> = processes
        .iter()
        .filter(|row| row.display.active)
        .map(|row| (row.agent.as_str(), row.id.as_str()))
        .collect();
    let mut activities: Vec<_> = processes
        .iter()
        .filter(|row| row.display.active)
        .cloned()
        .map(BackgroundTaskView::Process)
        .collect();
    activities.extend(
        tasks
            .iter()
            .filter(|row| {
                row.display.active && !process_ids.contains(&(row.agent.as_str(), row.id.as_str()))
            })
            .cloned()
            .map(BackgroundTaskView::Task),
    );
    activities
}
pub fn matches(process: &ProcessRecord, identity: &ProcessRecord) -> bool {
    process.view.key == identity.view.key
        && process.view.generation == identity.view.generation
        && process.view.agent == identity.view.agent
        && process.view.account_key == identity.view.account_key
        && process.view.epoch == identity.view.epoch
}
fn response_matches(
    expected: &Option<ProcessResponseTarget>,
    target: &ProcessResponseTarget,
) -> bool {
    expected.as_ref() == Some(target)
}
pub fn validate_response(
    owner: &Value,
    target: &ProcessResponseTarget,
    connection: Option<&str>,
) -> bool {
    response_matches(&response_target(owner, connection), target)
}
pub fn validate_stop_response(
    owner: &Value,
    target: &ProcessResponseTarget,
    scope: &ConnectionScope,
    proof: Option<&HeldResponseProof>,
) -> bool {
    validate_response(owner, target, Some(&scope.connection))
        || proof.is_some_and(|proof| {
            proof.matches(owner, scope)
                && response_matches(&response_identity(owner, &scope.connection), target)
        })
}

pub enum Claim {
    Replay(ProcessActionResult),
    New {
        record: Box<ProcessActionRecord>,
        targets: Vec<ProcessRecord>,
        result: ProcessActionResult,
    },
}

pub fn claim(
    tx: &Transaction<'_>,
    request: &ProcessControlRequest,
    connections: &Connections,
    now: f64,
) -> Result<Claim, Error> {
    if let Some(result) = replay(tx, request)? {
        return Ok(Claim::Replay(result));
    }
    let command = request;
    let command_scope = command.scope();
    let id = command_scope.request_id.as_str();
    let signature = request_digest(request)?;
    let scope = command_scope.agent.as_str();
    let owners = owners(tx, scope)?;
    let rows = process_rows(tx, &owners)?;
    let processes = processes(tx, &rows, connections)?;
    let owner = &owners[0];
    if text(owner, "accountKey", "default") != command_scope.account_key.as_str()
        || owner["epoch"].as_u64() != Some(command_scope.epoch)
    {
        return Err(Error::Invalid(
            "The process owner changed. Refresh the process list",
        ));
    }
    let mut targets = Vec::new();
    let mut entries = Vec::new();
    let response = if let ProcessControlRequest::StopResponse {
        response_target, ..
    }
    | ProcessControlRequest::BackgroundCommand {
        response_target, ..
    } = command
    {
        let expected = if matches!(command, ProcessControlRequest::StopResponse { .. }) {
            stop_target(owner, connections)
        } else {
            self::response_target(owner, connections.get(scope).map(String::as_str))
        };
        let valid = response_matches(&expected, response_target);
        if !valid {
            return Err(Error::Invalid(
                "The response changed. Refresh the process list",
            ));
        }
        Some(serde_json::value::to_raw_value(response_target).map_err(StoreError::from)?)
    } else {
        let (selected, include_descendants) = match command {
            ProcessControlRequest::StopSelected {
                process,
                include_descendants,
                ..
            } => (std::slice::from_ref(process), *include_descendants),
            ProcessControlRequest::StopOwned {
                processes,
                include_descendants,
                ..
            } => (processes.as_slice(), *include_descendants),
            ProcessControlRequest::StopResponse { .. }
            | ProcessControlRequest::BackgroundCommand { .. } => {
                unreachable!("response commands handled above")
            }
        };
        let mut seen = HashSet::new();
        for identity in selected {
            let found = processes.iter().find(|p| {
                p.view.key == identity.key.as_str()
                    && p.view.generation == identity.generation.as_str()
                    && p.view.agent == identity.agent.as_str()
                    && p.view.account_key == identity.account_key.as_str()
                    && p.view.epoch == identity.epoch
            });
            let error = if !seen.insert(identity.key.as_str()) {
                Some("A process identity was repeated")
            } else if !include_descendants && identity.agent.as_str() != scope {
                Some("Worker processes are outside this stop scope")
            } else if found.is_none() {
                Some("This process changed or finished. Refresh the process list")
            } else if found
                .is_some_and(|p| !p.view.display.actions.contains(&BackgroundTaskAction::Stop))
            {
                Some("The process owner changed. Refresh the process list")
            } else {
                None
            };
            if let Some(error) = error {
                entries.push(ProcessEntry {
                    key: identity.key.as_str().to_owned(),
                    agent: identity.agent.as_str().to_owned(),
                    source: None,
                    generation: None,
                    outcome: ProcessOutcome::Stale {
                        error: error.into(),
                    },
                    extra: Default::default(),
                });
            } else if let Some(process) = found {
                entries.push(ProcessEntry {
                    key: process.view.key.clone(),
                    agent: process.view.agent.clone(),
                    source: Some(process.view.source.as_str().to_owned()),
                    generation: Some(process.view.generation.clone()),
                    outcome: ProcessOutcome::Pending,
                    extra: Default::default(),
                });
                targets.push(process.clone());
            }
        }
        None
    };
    let result = ProcessActionResult {
        request_id: id.into(),
        status: ProcessActionStatus::Uncertain,
        processes: entries,
        response: None,
        extra: Default::default(),
    };
    let root_request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let admitted = match begin_request::<ProcessActionResult>(
        tx,
        RequestKey::new(
            "process-action",
            &[
                scope,
                command_scope.account_key.as_str(),
                &command_scope.epoch.to_string(),
            ],
            &root_request_id,
        )
        .operation(&[id]),
        &signature,
        chrono::Utc::now().timestamp_millis(),
    )
    .map_err(StoreError::from)?
    {
        CallerAdmission::Fresh(admitted) => {
            admitted.derive_result("process-action", &[id], &signature)
        }
        CallerAdmission::Replay(_) => {
            return Err(Error::Store(StoreError::RequestResult(
                studio_runtime_store::request_results::RequestResultError::Conflict {
                    id: root_request_id,
                },
            )));
        }
    };
    let record = ProcessActionRecord {
        request_id: id.into(),
        root_request_id,
        admitted_request: admitted,
        signature: signature.clone(),
        phase: ProcessActionPhase::Dispatching,
        scope_agent: scope.into(),
        scope_account_key: command_scope.account_key.as_str().to_owned(),
        scope_epoch: command_scope.epoch.into(),
        action: match command {
            ProcessControlRequest::StopSelected { .. } => ProcessActionKind::StopSelected,
            ProcessControlRequest::StopOwned { .. } => ProcessActionKind::StopOwned,
            ProcessControlRequest::StopResponse { .. } => ProcessActionKind::StopResponse,
            ProcessControlRequest::BackgroundCommand { .. } => ProcessActionKind::BackgroundCommand,
        },
        response_target: response,
        processes: result.processes.clone(),
        created: now,
        result: result.clone(),
    };
    tx.execute(
        "INSERT INTO runtime_process_actions(id,signature,record) VALUES(?,?,?)",
        params![
            id,
            signature,
            serde_json::to_string(&record).map_err(StoreError::from)?
        ],
    )
    .map_err(StoreError::from)?;
    Ok(Claim::New {
        record: Box::new(record),
        targets,
        result,
    })
}
/// Reuse a durable action before any fresh native observation. Changed content
/// under the same request ID remains an error, including after a lost response.
pub fn replay(
    tx: &Transaction<'_>,
    request: &ProcessControlRequest,
) -> Result<Option<ProcessActionResult>, Error> {
    let prior: Option<(String, String)> = tx
        .query_row(
            "SELECT signature,record FROM runtime_process_actions WHERE id=?",
            [request.scope().request_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(StoreError::from)?;
    let Some((signature, raw)) = prior else {
        return Ok(None);
    };
    if signature != request_digest(request)? {
        return Err(Error::Invalid(
            "This process action ID has different content",
        ));
    }
    let record: ProcessActionRecord = serde_json::from_str(&raw).map_err(StoreError::from)?;
    Ok(Some(record.result))
}
pub fn save(tx: &Transaction<'_>, record: &ProcessActionRecord) -> Result<(), Error> {
    tx.execute(
        "UPDATE runtime_process_actions SET record=? WHERE id=?",
        params![
            serde_json::to_string(record).map_err(StoreError::from)?,
            record.request_id
        ],
    )
    .map_err(StoreError::from)?;
    Ok(())
}
pub struct ReceiptIdentity {
    pub request_id: String,
    pub agent: String,
    pub account_key: String,
    pub epoch: i64,
}

pub fn receipt(connection: &Connection, query: &ReceiptIdentity) -> Result<ProcessReceipt, Error> {
    let exists:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_process_actions')",[],|r|r.get(0)).map_err(StoreError::from)?;
    if !exists {
        return input_receipt(connection, query);
    }
    let record: Option<String> = connection
        .query_row(
            "SELECT record FROM runtime_process_actions WHERE id=?",
            [&query.request_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)?;
    let record: Option<ProcessActionRecord> = record
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(StoreError::from)?;
    if let Some(record) = record
        && record.scope_agent == query.agent
        && record.scope_account_key == query.account_key
        && Some(query.epoch as f64) == record.scope_epoch.as_f64()
    {
        return Ok(ProcessReceipt::found(record.result));
    }
    input_receipt(connection, query)
}
fn input_receipt(
    connection: &Connection,
    query: &ReceiptIdentity,
) -> Result<ProcessReceipt, Error> {
    crate::monitor_http_input::read(connection, query).map_err(|error| match error {
        crate::monitor_policy::MonitorError::Expired => Error::Expired,
        crate::monitor_policy::MonitorError::Store(error) => Error::Store(error),
        crate::monitor_policy::MonitorError::InvalidStored { .. }
        | crate::monitor_policy::MonitorError::InvalidResultReceipt(_)
        | crate::monitor_policy::MonitorError::InvalidDeliveryIdentity(_)
        | crate::monitor_policy::MonitorError::InvalidOutputReceipt(_)
        | crate::monitor_policy::MonitorError::TaskFailed
        | crate::monitor_policy::MonitorError::Refused(_)
        | crate::monitor_policy::MonitorError::InputOutcomeUnknown
        | crate::monitor_policy::MonitorError::Unavailable(_)
        | crate::monitor_policy::MonitorError::ProviderRequest { .. }
        | crate::monitor_policy::MonitorError::ConfigurationRejected { .. }
        | crate::monitor_policy::MonitorError::ConfigurationReceipt { .. }
        | crate::monitor_policy::MonitorError::Event(_)
        | crate::monitor_policy::MonitorError::Rules(_)
        | crate::monitor_policy::MonitorError::Io(_) => {
            Error::Invalid("Invalid monitor input receipt")
        }
    })
}

#[cfg(test)]
mod claim_identity_tests {
    use super::*;
    use studio_runtime_schema::persistence::RequestIdentity;
    use studio_runtime_store::request_results::RequestResults;

    #[test]
    fn activity_snapshot_prefers_process_then_restores_task_after_exit() {
        let raw = r#"{"id":"command","agent":"owner","created":1,"itemId":"item","turnId":"turn","kind":"command","status":"running","type":"commandExecution","name":"commandExecution","processId":"pid"}"#;
        let task: studio_runtime_store::TaskRecord = serde_json::from_str(raw).unwrap();
        let owner = json!({"id":"owner","epoch":1,"accountKey":"default","threadId":"thread","autoWake":true});
        let process = project(
            ProcessSourceKind::CodexTask,
            &task,
            &owner,
            Some("connection"),
        )
        .view;
        let task = crate::sync_views::project_task_record(&task).unwrap();
        let before = background_activities(&[], std::slice::from_ref(&task));
        let running =
            background_activities(std::slice::from_ref(&process), std::slice::from_ref(&task));
        let after = background_activities(&[], std::slice::from_ref(&task));
        assert_eq!(before.len(), 1);
        assert!(matches!(&before[0], BackgroundTaskView::Task(_)));
        assert_eq!(running.len(), 1);
        assert!(matches!(&running[0], BackgroundTaskView::Process(_)));
        assert_eq!(after.len(), 1);
        assert!(matches!(&after[0], BackgroundTaskView::Task(_)));
    }

    #[test]
    fn monitor_actions_match_input_admission_for_pending_and_closed_stdin() {
        let owner = json!({"id":"owner","rootId":"owner","compactions":0,"epoch":1,"accountKey":"default","threadId":"thread","autoWake":true,"provider":"codex","status":"idle","cwd":std::env::temp_dir()});
        let fixture = format!(
            r#"{{"id":"watch","agent":"owner","epoch":1,"status":"running","requestId":"request","created":1,"activityAt":1,"activityGeneration":0,"bytes":0,"command":"cat","cwd":{},"interactive":true,"livenessCommand":"","log":{},"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"","timeout_ms":3600000,"wakeOn":"exit","operation":{{"agent":"owner","epoch":1,"accountKey":"default","connectionId":"connection","threadId":"thread"}}}}"#,
            serde_json::to_string(&std::env::temp_dir()).unwrap(),
            serde_json::to_string(&std::env::temp_dir().join("watch.log")).unwrap()
        );
        let record: studio_runtime_store::MonitorRecord = serde_json::from_str(&fixture).unwrap();
        for state in ["open", "closed", "pending"] {
            let mut record = record.clone();
            if state == "closed" {
                record.stdin_closed = Some(true);
            }
            if state == "pending" {
                record.input_attempt = Some(serde_json::from_str(r#"{"id":"old","agent":"owner","epoch":1,"accountKey":"default","connectionId":"connection","threadId":"thread","method":"command/exec/write","params":{"processId":"watch","deltaBase64":"YQ==","closeStdin":false},"status":"submitted","created":1}"#).unwrap());
            }
            let snapshot =
                crate::monitor_policy::MonitorSnapshot::checked_stored(&record, owner.clone())
                    .unwrap();
            let operation = snapshot.operation().unwrap();
            let projected = project_monitor_bound(&record, &owner, Some("connection"), None);
            for (action, resize) in [
                (BackgroundTaskAction::Input, false),
                (BackgroundTaskAction::Resize, true),
            ] {
                let input = crate::monitor_policy::Input {
                    text: "x".into(),
                    rows: resize.then_some(24),
                    cols: resize.then_some(80),
                    close_stdin: false,
                };
                let admitted =
                    crate::monitor_policy::begin_input(&snapshot, operation, "new", &input, 2.0)
                        .is_ok();
                assert_eq!(
                    projected.view.display.actions.contains(&action),
                    admitted,
                    "{state}: {action:?}"
                );
            }
        }
    }

    #[test]
    fn task_record_projection_publishes_background_display_policy() {
        let raw = json!({"id":"a:task:1","agent":"a","created":1,"itemId":"task:1","kind":"tool","status":"running","turnId":"turn","type":"dynamicToolCall","name":"job"});
        let task: studio_runtime_store::TaskRecord = serde::Deserialize::deserialize(&raw).unwrap();
        let display = task_record_display(&task);
        assert_eq!(display.kind, BackgroundTaskKind::BackgroundTask);
        assert!(display.active);
        assert!(display.in_background_list);
        assert_eq!(display.stdin_state, None);
        assert!(!display.actions.contains(&BackgroundTaskAction::Input));
        assert!(
            !serde_json::to_string(&display)
                .unwrap()
                .contains("stdinState")
        );
    }

    #[test]
    fn process_projection_publishes_display_policy() {
        let owner = json!({"id":"owner","epoch":1,"accountKey":"default","threadId":"thread","autoWake":true});
        let raw = json!({"id":"task","agent":"owner","ownerEpoch":1,"itemId":"task:123","status":"running","created":1,"kind":"tool","turnId":"turn","type":"dynamicToolCall","name":"job"});
        let task: studio_runtime_store::TaskRecord = serde::Deserialize::deserialize(&raw).unwrap();
        let projected = project(ProcessSourceKind::ClaudeTask, &task, &owner, None);
        assert_eq!(
            projected.view.display.kind,
            BackgroundTaskKind::BackgroundTask
        );
        assert!(projected.view.display.active);
    }

    #[test]
    fn typed_monitor_process_projection_preserves_identity_and_capabilities() {
        let owner = json!({"id":"owner","epoch":1,"accountKey":"default","threadId":"thread","autoWake":true});
        for status in ["starting", "running"] {
            let fixture = format!(
                r#"{{"id":"watch","agent":"owner","epoch":1,"status":"{status}","requestId":"request","created":1,"activityAt":1,"activityGeneration":0,"bytes":12,"command":"printf x","cwd":{},"interactive":true,"livenessCommand":"","log":{},"stallTimeoutSeconds":1800,"stallWakeGeneration":-1,"successExitCodes":[0],"tail":"x","timeout_ms":3600000,"wakeOn":"exit","operation":{{"agent":"owner","epoch":1,"accountKey":"default","connectionId":"connection","threadId":"thread"}}}}"#,
                serde_json::to_string(&std::env::temp_dir()).unwrap(),
                serde_json::to_string(&std::env::temp_dir().join("watch.log")).unwrap()
            );
            let record: studio_runtime_store::MonitorRecord =
                serde_json::from_str(&fixture).unwrap();
            let projected = project_monitor_bound(&record, &owner, Some("connection"), None);
            assert_eq!(projected.source_connection.as_deref(), Some("connection"));
            assert_eq!(projected.view.id, "watch");
            assert_eq!(projected.view.tail, "x");
            assert_eq!(projected.view.bytes, 12);
            assert!(
                projected
                    .view
                    .display
                    .actions
                    .contains(&BackgroundTaskAction::ReadOutput)
            );
        }
    }

    #[test]
    fn process_generation_digest_keeps_canonical_nested_keys() {
        let value = json!({"z":[{"b":2,"a":1}],"a":{"y":3,"b":2}});
        assert_eq!(
            digest(&value),
            "72ddf54a7a1042b66ae85c35ecc5c9b2d5fea1cce275577cf0502e1f1965da22"
        );
    }

    #[test]
    fn normalized_signature_replays_and_refuses_preupgrade_receipt() {
        let raw = r#"{"action":"stop_owned","requestId":"request","agent":"owner","accountKey":"acct","epoch":1.0,"processes":[{"key":"key","generation":"gen","agent":"owner","accountKey":"acct","epoch":1.0}],"responseTarget":null}"#;
        let request: ProcessControlRequest = serde_json::from_str(raw).expect("request");
        let normalized = raw
            .replace("1.0", "1")
            .replace(",\"responseTarget\":null", "");
        let retry: ProcessControlRequest = serde_json::from_str(&normalized).expect("retry");
        let signature = request_digest(&request).expect("signature");
        assert_eq!(signature, request_digest(&retry).expect("retry signature"));
        let old = "b16654521805af453f0fc6e0b9f42fb54887c3b20eb1fb103b50e05a645497f8";
        assert_ne!(signature, old);
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(
            "CREATE TABLE runtime_process_actions(id TEXT PRIMARY KEY,signature TEXT,record TEXT);",
        )
        .expect("table");
        db.execute(
            "INSERT INTO runtime_process_actions VALUES('request',?1,'{}')",
            [old],
        )
        .expect("old receipt");
        let tx = db.transaction().expect("transaction");
        assert!(matches!(
            replay(&tx, &retry),
            Err(Error::Invalid(
                "This process action ID has different content"
            ))
        ));
    }

    #[test]
    fn process_claim_reuses_its_admitted_token_after_expiry_without_another_effect() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT,parent_id TEXT,deleted_at REAL);
            CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY,record TEXT, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
            CREATE INDEX runtime_monitor_agent_status ON runtime_monitors(id);
            CREATE TABLE runtime_tasks(id TEXT PRIMARY KEY,record TEXT);
            CREATE INDEX runtime_task_agent_created_id ON runtime_tasks(id);
            CREATE TABLE runtime_process_actions(id TEXT PRIMARY KEY,signature TEXT,record TEXT);
            CREATE TABLE runtime_orchestration_settings(id TEXT PRIMARY KEY,record TEXT);
            CREATE TABLE effects(count INTEGER);
            INSERT INTO effects VALUES(0);")
            .expect("claim tables");
        studio_runtime_store::receipt_schema::create(&db).expect("receipt schema");
        let owner = json!({"id":"owner","epoch":3,"accountKey":"account","inFlight":true,
            "threadId":"thread","turnId":"turn"});
        db.execute(
            "INSERT INTO runtime_agents(id,record) VALUES('owner',?1)",
            [owner.to_string()],
        )
        .expect("owner");
        let raw = serde_json::value::RawValue::from_string(
            json!({"action":"stop_response","requestId":"opaque-action-id",
            "agent":"owner","accountKey":"account","epoch":3,"responseTarget":{
                "agent":"owner","accountKey":"account","epoch":3,"connectionId":"connection",
                "threadId":"thread","turnId":"turn"}})
            .to_string(),
        )
        .expect("fixture JSON");
        let request: ProcessControlRequest = serde_json::from_str(raw.get()).expect("request");
        let connections = Connections::from([("owner".into(), "connection".into())]);
        let tx = db.transaction().expect("transaction");
        let Claim::New { record, .. } =
            claim(&tx, &request, &connections, 1.0).expect("fresh claim")
        else {
            panic!("first claim must be new");
        };
        let root = &record.root_request_id;
        RequestIdentity::new("process-action", &[], root, &[]).expect("UUIDv7 root");
        assert_eq!(record.request_id, "opaque-action-id");
        tx.execute("UPDATE effects SET count=count+1", [])
            .expect("first dispatch");
        let token = record.admitted_request.clone();
        let after_ttl =
            token.created() + studio_runtime_schema::persistence::REQUEST_RESULT_TTL_MS + 1;
        RequestResults::gc(&tx, after_ttl).expect("expire fresh ingress");
        assert!(
            RequestIdentity::new("process-action", &[], root, &[])
                .expect("identity")
                .validate(
                    after_ttl,
                    RequestResults::expired_before_ms(&tx).expect("floor")
                )
                .is_err()
        );
        assert!(matches!(
            claim(&tx, &request, &connections, after_ttl as f64 / 1000.0)
                .expect("admitted replay never reenters ingress"),
            Claim::Replay(_)
        ));
        let persisted: String = tx
            .query_row(
                "SELECT record FROM runtime_process_actions WHERE id='opaque-action-id'",
                [],
                |row| row.get(0),
            )
            .expect("owner record");
        let persisted: ProcessActionRecord = serde_json::from_str(&persisted).expect("record");
        assert_eq!(&persisted.root_request_id, root);
        assert_eq!(persisted.admitted_request, token);
        assert_eq!(
            tx.query_row("SELECT count FROM effects", [], |row| row.get::<_, i64>(0))
                .expect("effects"),
            1
        );
    }
}
#[test]
fn typed_task_keeps_numeric_process_identity_projection() {
    let raw = r#"{"id":"a:i","agent":"a","created":1,"itemId":"i","turnId":"turn","kind":"command","status":"running","type":"commandExecution","name":"commandExecution","processId":42}"#;
    let typed: studio_runtime_store::TaskRecord = serde_json::from_str(raw).expect("task fixture");
    let owner = serde_json::json!({"id":"a","epoch":1,"accountKey":"default","threadId":"thread","autoWake":true});
    let after = project(
        ProcessSourceKind::CodexTask,
        &typed,
        &owner,
        Some("connection"),
    );
    assert_eq!(after.process_id, typed.process_id().cloned());
    assert_eq!(after.item_id.as_deref(), Some("i"));
    assert_eq!(after.view.source, ProcessSourceKind::CodexTask);
}
