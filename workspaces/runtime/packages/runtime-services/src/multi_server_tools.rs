//! The lead's `orchestration_servers` tool over the owner-paired server channel.
//!
//! Python `MultiServerService.tools` in codex_multi_server_orchestration.py
//! owns these rules, with `codex_project_locations.request` and `suggestions`.
//! Every remote action queues one durable envelope with a stable identity in
//! the shared outbox before it crosses the transport. A refusal before
//! that write is a not-applied tool result; the receipt of a sent envelope is
//! the tool result, whatever its outcome.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use studio_runtime_store::path_identity::CanonicalPath;

use base64::Engine;
use futures_util::future::BoxFuture;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use studio_runtime_schema::persistence::{OutboxStatus, ServerDeliveryState, ServerFetchState};
use studio_runtime_store::multi_server_delivery::{SavedRemoteReply, remote_operation_key};
use studio_runtime_store::remote_outbox::{RemoteEffect, RemoteEntry, RemoteOutbox};
use studio_runtime_store::request_results::{AdmittedRequest, RequestResultError};
use studio_runtime_store::{
    AgentId, AgentRepository, ProjectsRepository, RuntimeStore, StoreError,
};

use crate::event_write::request_signature;
use crate::managed_host_tools::{Actor, DispatchedOperation, Error, OperationResult};
use crate::multi_server_auth::AccessError;
use crate::multi_server_orchestration::{ExchangeOutcome, OrchestrationDriver};
use crate::multi_server_transport_limits::MAX_REQUEST_BYTES;
use crate::server_git::{
    self, CHUNK, ExportRequest, MAX_BUNDLE, commit_hash, git_command, git_read, git_run,
};

/// Seconds that `git bundle verify` may run (Python `_fetch`).
const BUNDLE_VERIFY_SECONDS: u64 = 30;
/// Seconds that the local `git fetch` of a bundle may run (Python `_fetch`).
const BUNDLE_FETCH_SECONDS: u64 = 60;
/// Command actions that the executing server owns (Python `_exec_tool`).
const EXEC_ACTIONS: &[&str] = &["exec", "exec_read", "exec_input", "exec_cancel"];
/// Payload fields of a command request (Python `_exec_tool`).
const EXEC_FIELDS: &[&str] = &[
    "cwd",
    "command",
    "env",
    "timeout",
    "output_limit",
    "handle",
    "input",
    "close_stdin",
    "stdout_offset",
    "stderr_offset",
];

/// The tool adapter that managed-tool dispatch calls after admission commits.
pub(crate) trait MultiServerToolPort: Send + Sync {
    fn servers<'a>(
        &'a self,
        actor: &'a Actor,
        args: &'a Value,
        key: &'a str,
        now: f64,
    ) -> BoxFuture<'a, Result<DispatchedOperation, Error>>;

    /// Python `worker_call`: a remote worker's tool call answered by its home
    /// server through the remote-parent link.
    fn worker_call<'a>(
        &'a self,
        actor: &'a Actor,
        action: &'a str,
        args: &'a Value,
        key: &'a str,
    ) -> BoxFuture<'a, Result<DispatchedOperation, Error>>;
}

/// Production adapter over the paired-server outbox driver.
pub(crate) struct ServerTools {
    store: Arc<RuntimeStore>,
    driver: Arc<OrchestrationDriver>,
    local_exec: bool,
}

impl ServerTools {
    async fn deliver_operation(
        &self,
        server: &str,
        operation: &[String],
        command: crate::orchestration_wire::OrchestrationCommand,
        key: &str,
    ) -> Result<ExchangeOutcome, Step> {
        let now = crate::event_write::unix_time()
            .map_err(|_| Step::Refused("The system clock is unavailable".into()))?
            as i64;
        self.driver
            .exchange_operation(
                server,
                &crate::orchestration_wire::OrchestrationRequest {
                    request_id: key.to_owned(),
                    operation: operation.to_vec(),
                    command,
                },
                now,
            )
            .await
            .map_err(|error| match error {
                AccessError::Store(error) => Step::Store(error),
                AccessError::Sqlite(error) => Step::Store(error.into()),
                other @ AccessError::Refused { .. } | other @ AccessError::Stored { .. } => {
                    Step::Refused(other.to_string())
                }
            })
    }
    pub(crate) fn new(
        store: Arc<RuntimeStore>,
        driver: Arc<OrchestrationDriver>,
        local_exec: bool,
    ) -> Self {
        Self {
            store,
            driver,
            local_exec,
        }
    }
}

/// A step result. A refusal proves that no envelope was queued; a failure
/// after a queued envelope or a local Git effect leaves the outcome unknown.
#[derive(Debug)]
enum Step {
    Refused(String),
    Failed(String),
    Store(StoreError),
}
impl From<StoreError> for Step {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
impl From<rusqlite::Error> for Step {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}
impl From<crate::project_locations::LocationError> for Step {
    fn from(error: crate::project_locations::LocationError) -> Self {
        match error {
            crate::project_locations::LocationError::Store(error) => Self::Store(error),
            other @ crate::project_locations::LocationError::InvalidStored { .. }
            | other @ crate::project_locations::LocationError::Refused(_)
            | other @ crate::project_locations::LocationError::Requires(_)
            | other @ crate::project_locations::LocationError::Sqlite(_) => {
                Self::Refused(other.to_string())
            }
        }
    }
}

impl From<RequestResultError> for Step {
    fn from(error: RequestResultError) -> Self {
        Self::Store(error.into())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchReply {
    request_id: String,
    server: String,
    branch: String,
    commit: String,
    bytes: u64,
    destination: String,
}

/// The record of a local Git fetch in the shared outbox (effect `server_fetch`).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchEntry {
    state: ServerFetchState,
    /// The saved fetch inputs as JSON text.
    export: Option<String>,
    admission: AdmittedRequest,
}

/// An unknown entry (after an unclean stop) counts as a fetch that may have started.
fn fetch_state(entry: &RemoteEntry<FetchEntry>) -> ServerFetchState {
    if entry.status == OutboxStatus::Unknown {
        ServerFetchState::Fetching
    } else {
        entry.record.state
    }
}

enum FetchPayload {
    Preparing(ExportRequest),
    Fetching(ExportRequest),
}
impl FetchPayload {
    fn decode(state: ServerFetchState, raw: Option<&str>) -> Result<Option<Self>, Step> {
        let Some(raw) = raw else {
            return Ok(None);
        };
        let request: ExportRequest = serde_json::from_str(raw).map_err(StoreError::from)?;
        match state {
            ServerFetchState::Preparing => Ok(Some(Self::Preparing(request))),
            ServerFetchState::Fetching => Ok(Some(Self::Fetching(request))),
            ServerFetchState::Complete => Err(Step::Refused(
                "A completed fetch must have a saved request result".into(),
            )),
        }
    }
    fn export(&self) -> &ExportRequest {
        match self {
            Self::Preparing(request) | Self::Fetching(request) => request,
        }
    }
}

fn task_admission(
    tx: &studio_runtime_store::Connection,
    key: &str,
    actor: &str,
) -> Result<AdmittedRequest, Step> {
    crate::tool_requests::resolve_admission(tx, actor, key)
        .map_err(|error| Step::Refused(error.to_string()))?
        .ok_or_else(|| Step::Refused("The admitted tool request is unavailable".into()))
}

/// A failure of the shared outbox as a tool step.
fn access_step(error: AccessError) -> Step {
    match error {
        AccessError::Store(error) => Step::Store(error),
        AccessError::Sqlite(error) => Step::Store(error.into()),
        other @ (AccessError::Refused { .. } | AccessError::Stored { .. }) => {
            Step::Refused(other.to_string())
        }
    }
}

fn refuse<T>(message: impl Into<String>) -> Result<T, Step> {
    Err(Step::Refused(message.into()))
}

/// A written tool result that carries the paired receipt or local value.
fn written(value: &Value) -> DispatchedOperation {
    let mut result = OperationResult::reply(value);
    if let Some(probe) = output_metadata(result.value.get()) {
        result.output_expires_at = probe.output_expires_at;
        result.handle = probe.handle;
        result.output_ref = probe.output_ref;
    }
    DispatchedOperation::Applied(result)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputProbe {
    #[serde(default, deserialize_with = "numeric_expiry")]
    output_expires_at: Option<f64>,
    #[serde(default, deserialize_with = "present")]
    handle: bool,
    #[serde(default, deserialize_with = "present")]
    output_ref: bool,
    value: Option<Box<serde_json::value::RawValue>>,
    result: Option<Box<serde_json::value::RawValue>>,
}

fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    serde::de::IgnoredAny::deserialize(deserializer).map(|_| true)
}

fn numeric_expiry<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
    Ok(serde_json::from_str(raw.get()).ok())
}

fn output_metadata(raw: &str) -> Option<OutputProbe> {
    let probe: OutputProbe = serde_json::from_str(raw).ok()?;
    if probe.output_expires_at.is_some() && (probe.handle || probe.output_ref) {
        return Some(probe);
    }
    [probe.value.as_ref(), probe.result.as_ref()]
        .into_iter()
        .flatten()
        .find_map(|nested| {
            let inner: OutputProbe = serde_json::from_str(nested.get()).ok()?;
            (inner.output_expires_at.is_some() && (inner.handle || inner.output_ref))
                .then_some(inner)
        })
}

#[cfg(test)]
mod output_metadata_tests {
    #[test]
    fn expiry_is_carried_from_the_receipt_or_its_value() {
        let top = super::output_metadata(r#"{"outputExpiresAt":12,"outputRef":null}"#).unwrap();
        assert_eq!(top.output_expires_at, Some(12.0));
        assert!(top.output_ref);
        let nested = super::output_metadata(
            r#"{"outcome":"applied","value":{"outputExpiresAt":34,"handle":"h"}}"#,
        )
        .unwrap();
        assert_eq!(nested.output_expires_at, Some(34.0));
        assert!(nested.handle);
        let result = super::output_metadata(
            r#"{"value":{},"result":{"outputExpiresAt":56,"outputRef":"page"}}"#,
        )
        .unwrap();
        assert_eq!(result.output_expires_at, Some(56.0));
        assert!(result.output_ref);
        assert!(super::output_metadata(r#"{"outputExpiresAt":"bad","handle":"h"}"#).is_none());
        assert!(super::output_metadata(r#"{"outputExpiresAt":34}"#).is_none());
    }
}

fn not_sent(message: &str) -> DispatchedOperation {
    DispatchedOperation::NotSent(OperationResult::reply(&message))
}

fn settle(step: Result<Value, Step>) -> Result<DispatchedOperation, Error> {
    match step {
        Ok(value) => Ok(written(&value)),
        Err(Step::Refused(message)) => Ok(not_sent(&message)),
        Err(Step::Failed(message)) => Ok(DispatchedOperation::Unknown(OperationResult::reply(
            &message,
        ))),
        Err(Step::Store(error)) => Err(Error::Store(error)),
    }
}

fn io(error: std::io::Error) -> Step {
    Step::Failed(error.to_string())
}

/// Python `text_field`: a stripped text of 1 to `maximum` characters.
fn text_field(value: Option<&Value>, name: &str, maximum: usize) -> Result<String, Step> {
    match value.and_then(Value::as_str) {
        Some(text) if text.chars().count() <= maximum && !text.trim().is_empty() => {
            Ok(text.trim().to_owned())
        }
        _ => refuse(format!("Supply {name} with 1 to {maximum} characters")),
    }
}

fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The receipt that the tool returns for one delivered envelope (Python `deliver`).
fn receipt(outcome: &ExchangeOutcome, key: &str) -> Value {
    match outcome {
        ExchangeOutcome::Applied(reply) => {
            // RawValue contains valid JSON from the paired transport.
            let value: Value = serde_json::from_str(reply.value.get()).expect("valid raw reply");
            json!({"requestId":key,"outcome":"applied","value":value})
        }
        ExchangeOutcome::NotApplied(error) => {
            json!({"requestId":key,"outcome":"not_applied","error":error})
        }
        ExchangeOutcome::Unknown => json!({"requestId":key,"outcome":"unknown"}),
        ExchangeOutcome::PreEffectWaiting => {
            json!({"requestId":key,"outcome":"unknown","waitingFor":"The stop or parent update"})
        }
    }
}

impl ServerTools {
    async fn read<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, Step> + Send + 'static,
    ) -> Result<T, Step> {
        self.store
            .read_async("runtime-services/multi_server_tools", operation)
            .await
    }

    async fn local_server(&self) -> Result<String, Step> {
        let store = Arc::clone(&self.store);
        let local = studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/multi_server_tools",
            move || store.local_server_id(),
        )
        .await
        .map_err(|_| Step::Store(StoreError::WriterPoisoned))??;
        match local {
            Some(local) if !local.is_empty() => Ok(local),
            _ => refuse("The local server identity is unavailable"),
        }
    }

    async fn paired_ids(&self) -> Result<Vec<String>, Step> {
        self.read(|tx| {
            Ok(crate::multi_server::paired_servers(tx)?
                .into_iter()
                .map(|server| server.id.as_str().to_owned())
                .collect())
        })
        .await
    }

    /// Python `queue` followed by `deliver`: the driver claims the envelope in
    /// the outbox before the transport and records its receipt after it. A
    /// refusal of the claim itself (different content, size) wrote nothing.
    async fn deliver(
        &self,
        server: &str,
        command: crate::orchestration_wire::OrchestrationCommand,
        key: &str,
    ) -> Result<Value, Step> {
        self.deliver_exchange(server, command, key)
            .await
            .map(|outcome| receipt(&outcome, key))
    }

    async fn deliver_exchange(
        &self,
        server: &str,
        command: crate::orchestration_wire::OrchestrationCommand,
        key: &str,
    ) -> Result<ExchangeOutcome, Step> {
        let now = crate::event_write::unix_time()
            .map_err(|_| Step::Refused("The system clock is unavailable".into()))?
            as i64;
        match self.driver.exchange(server, command, key, now).await {
            Ok(outcome) => Ok(outcome),
            Err(AccessError::Refused {
                code: "request_conflict" | "request_size" | "invalid_request" | "client_revoked",
                message,
                ..
            }) => {
                if message == "The remote request exceeds 256 KiB" {
                    return refuse("The remote request exceeds 256 KiB; use a smaller batch");
                }
                refuse(message)
            }
            Err(_) => Ok(ExchangeOutcome::Unknown),
        }
    }

    async fn run(&self, actor: &Actor, args: &Value, key: &str) -> Result<Value, Step> {
        if actor.record().is_lead != Some(true) {
            return refuse("Only the lead can use remote server tools");
        }
        let action = args.get("action").and_then(Value::as_str).unwrap_or("list");
        let local = self.local_server().await?;
        match action {
            "list" => return self.list(&local).await,
            "add_location" | "remove_location" => {
                return self.location_request(actor, args, key, &local).await;
            }
            "location_matches" => {
                return self.location_matches(args, key).await;
            }
            _ if EXEC_ACTIONS.contains(&action) => {
                return self.exec(actor, args, action, key, &local).await;
            }
            _ => {}
        }
        let server = match args.get("server") {
            None if action == "receipt" => Value::String(local.clone()),
            Some(Value::String(server)) if action == "receipt" && server == "local" => {
                Value::String(local.clone())
            }
            None => Value::Null,
            Some(value) => value.clone(),
        };
        let Some(server) = server.as_str() else {
            return refuse("Select a paired server");
        };
        let paired = self.paired_ids().await?;
        if !paired.iter().any(|id| id == server) && !(action == "receipt" && server == local) {
            return refuse("Select a paired server");
        }
        if action == "fetch" {
            return self.fetch(actor, args, key, server).await;
        }
        if !matches!(action, "projects" | "folders" | "git" | "receipt") {
            return refuse("Select list, projects, folders, git, fetch, or receipt");
        }
        if action == "receipt" {
            return self.receipt(actor, args, server).await;
        }
        let mut payload = Map::new();
        for name in ["cwd", "argv"] {
            if let Some(value) = args.get(name) {
                payload.insert(name.to_owned(), value.clone());
            }
        }
        let key = key.to_owned();
        let command = if action == "folders" {
            let cwd = match payload.remove("cwd") {
                None => studio_runtime_schema::queue_command::Field::Missing,
                Some(Value::Null) => studio_runtime_schema::queue_command::Field::Null,
                Some(Value::String(value)) => {
                    studio_runtime_schema::queue_command::Field::Value(value)
                }
                Some(_) => return refuse("Invalid folder path"),
            };
            crate::orchestration_wire::OrchestrationCommand::Folders(
                crate::orchestration_worker::FoldersPayload { cwd },
            )
        } else if action == "git" {
            let cwd = payload
                .remove("cwd")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    Step::Refused("Supply an existing absolute repository folder".into())
                })?;
            let argv: Vec<String> = <Vec<String> as serde::Deserialize>::deserialize(
                payload.remove("argv").unwrap_or(Value::Null),
            )
            .map_err(|_| Step::Refused("Supply a bounded Git argument list".into()))?;
            crate::orchestration_wire::OrchestrationCommand::Git(
                crate::server_git::GitReadRequest { cwd, argv },
            )
        } else {
            let kind = action
                .parse()
                .map_err(|_| Step::Refused("Invalid cross-server action".into()))?;
            crate::orchestration_wire::OrchestrationCommand::remaining(kind, Value::Object(payload))
                .ok_or_else(|| Step::Refused("Invalid cross-server action".into()))?
        };
        self.deliver(server, command, &key).await
    }

    /// Python `tools` action `list`: a server with a failed queued delivery is offline.
    async fn list(&self, local: &str) -> Result<Value, Step> {
        let (servers, pending) = self
            .read(|tx| {
                let servers = crate::multi_server::paired_servers(tx)?;
                let pending = crate::server_outbox::offline_servers(tx)?;
                Ok((servers, pending))
            })
            .await?;
        let servers = servers
            .into_iter()
            .map(|server| {
                let offline = pending.iter().any(|id| server.id.as_str() == id);
                let mut result = json!(server);
                if offline {
                    result["status"] = json!("offline");
                }
                result
            })
            .collect::<Vec<_>>();
        Ok(json!({"localServer": local, "servers": servers}))
    }

    /// Python `tools` action `receipt`.
    async fn receipt(&self, actor: &Actor, args: &Value, server: &str) -> Result<Value, Step> {
        let Some(request) = args
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return refuse("Supply request_id");
        };
        let actor_id = actor.record().id.0.clone();
        let server = server.to_owned();
        self.read(move |tx| {
            let now=crate::event_write::unix_time().map_err(|_|Step::Refused("The system clock is unavailable".into()))?;
            if let Some(saved)=crate::project_locations::tool_location_receipt(tx,&actor_id,&request,&server,now)? {
                return Ok(json!({"state":ServerDeliveryState::Complete,"result":saved,"error":null}));
            }
            let task=crate::tool_requests::resolve(tx,&actor_id,&request).map_err(|error|Step::Refused(error.to_string()))?;
            let root=task.as_ref().and_then(|task|task["id"].as_str()).unwrap_or(&request);
            let action=task.as_ref().and_then(|task|task.pointer("/arguments/action")).and_then(Value::as_str).unwrap_or("exec");
            if action=="fetch" {
                let parent=task_admission(tx,root,&actor_id)?;
                let admitted=parent.derive_result("server-fetch",&[],"");
                let row:Option<(String,String)>=tx.query_row("SELECT id,signature FROM request_results WHERE id IN (?1,?2)",params![root,admitted.id()],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
                if let Some((id,signature))=row && let Some(saved)=crate::event_write::replay_request::<FetchReply>(tx,&id,&signature,(now*1000.0) as i64)? {
                    if saved.reply.server!=server {return refuse("The receipt belongs to another server");}
                    return Ok(json!({"state":ServerFetchState::Complete,"result":saved.reply,"error":null}));
                }
                let state:Option<ServerFetchState>=RemoteOutbox::get::<FetchEntry>(tx,RemoteEffect::ServerFetch,&server,root).map_err(StoreError::from)?.map(|entry|fetch_state(&entry));
                return state.map(|state|json!({"state":state,"result":null,"error":null})).ok_or_else(||Step::Refused("The request ID expired".into()));
            }
            let canonical=remote_operation_key("server-outbox",&[&server],root,&[action]);
            let row:Option<(String,String)>=tx.query_row("SELECT id,signature FROM request_results WHERE id=?1",params![canonical],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
            if let Some((id,signature))=row && let Some(saved)=crate::event_write::replay_request::<SavedRemoteReply>(tx,&id,&signature,(now*1000.0) as i64)? {
                if saved.reply.server.as_deref()!=Some(&server) || (action.starts_with("exec") && saved.reply.actor.as_deref()!=Some(&actor_id)) {return refuse("The receipt belongs to another actor or server");}
                return Ok(json!({"state":ServerDeliveryState::Complete,"result":saved.reply.response,"error":null}));
            }
            let pending:Option<(ServerDeliveryState,Option<String>,String)>=match crate::server_outbox::find_for(tx,&server,root).map_err(access_step)? {
                Some(entry)=>Some(entry),
                None=>crate::server_outbox::find_for(tx,&server,&canonical).map_err(access_step)?,
            }.map(|entry|(crate::server_outbox::state(entry.status),entry.record.error,entry.record.body));
            if let Some((state,error,body))=pending {
                let envelope:Value=serde_json::from_str(&body).map_err(StoreError::from)?;
                if action.starts_with("exec") && envelope.pointer("/payload/actor").and_then(Value::as_str)!=Some(&actor_id) {return refuse("The receipt belongs to another lead");}
                return Ok(json!({"state":state,"result":null,"error":error}));
            }
            refuse("The request ID expired")
        }).await
    }

    /// Python `codex_project_locations.request` with the lead as the actor.
    async fn location_request(
        &self,
        actor: &Actor,
        args: &Value,
        key: &str,
        local: &str,
    ) -> Result<Value, Step> {
        let args = crate::project_locations::LocationIngress::deserialize(args)
            .map_err(|_| Step::Refused("Invalid project location request".into()))?;
        let request = crate::project_locations::LocationRequest::parse(args, Some(local))
            .map_err(|message| Step::Refused(message.into()))?;
        let (action, path) = match &request.op {
            crate::project_locations::LocationOp::Add { path } => {
                ("add_location", Some(path.clone()))
            }
            crate::project_locations::LocationOp::Remove => ("remove_location", None),
        };
        let project_id = request.project.clone();
        let server = request.server.clone();
        if server != local && !self.paired_ids().await?.contains(&server) {
            return refuse("The server is not paired or is revoked");
        }
        // Python passes `{**args, 'request_id': key}`: the tool request owns the identity.
        let request_id = text_field(Some(&json!(key)), "a request ID", 128)?;
        let outbox_key = request_id.clone();
        let signature = request.signature().map_err(StoreError::from)?;
        let actor_id = actor.record().id.0.clone();
        let epoch = actor.record().epoch;
        let local_for_write = local.to_owned();
        let key_for_write = outbox_key.clone();
        let server_for_write = server.clone();
        let action_for_write = action.to_owned();
        let prepared =
            self.store
                .transaction_with(
                    move |tx| -> Result<
                        Result<crate::project_locations::LocationPayload, Value>,
                        Step,
                    > {
                        let current = checked_actor(tx, &actor_id)?;
                        if current.is_lead != Some(true) || current.epoch != epoch {
                            return refuse("Only the active lead can change project locations");
                        }
                        let parent = task_admission(tx, &key_for_write, &actor_id)?;
                        let admitted =
                            parent.derive_result("project-location", &["location"], &signature);
                        if let Some(saved) =
                            crate::event_write::replay_request::<Box<serde_json::value::RawValue>>(
                                tx,
                                admitted.id(),
                                admitted.signature(),
                                (crate::event_write::unix_time().map_err(|_| {
                                    Step::Refused("The system clock is unavailable".into())
                                })? * 1000.0) as i64,
                            )?
                        {
                            let _: crate::project_locations::LocationResult =
                                serde_json::from_str(saved.reply.get())
                                    .map_err(StoreError::from)?;
                            return Ok(Err(serde_json::from_str(saved.reply.get())
                                .map_err(StoreError::from)?));
                        }
                        let Some((_, mut project)) = ProjectsRepository::get(tx, &project_id)?
                        else {
                            return refuse("Select an existing project");
                        };
                        crate::project_locations::normalize_project(
                            &mut project,
                            &local_for_write,
                        )?;
                        let scoped_key = remote_operation_key(
                            "server-outbox",
                            &[&server_for_write],
                            &key_for_write,
                            &[&action_for_write],
                        );
                        let old: Option<(String, String)> =
                            match crate::server_outbox::find(tx, &key_for_write)
                                .map_err(access_step)?
                            {
                                Some(entry) => Some(entry),
                                None => crate::server_outbox::find(tx, &scoped_key)
                                    .map_err(access_step)?,
                            }
                            .map(|entry| (entry.target, entry.record.body));
                        if let Some((old_server, body)) = old {
                            #[derive(Deserialize)]
                            struct SavedEnvelope {
                                payload: Box<serde_json::value::RawValue>,
                            }
                            let envelope: SavedEnvelope =
                                serde_json::from_str(&body).map_err(|_| {
                                    Step::Refused("Stored cross-server envelope is invalid".into())
                                })?;
                            if old_server != server_for_write
                                || ProjectsRepository::location_request(
                                    tx,
                                    &project_id,
                                    &key_for_write,
                                )?
                                .as_ref()
                                    != Some(&admitted)
                            {
                                return refuse("This request ID has different content");
                            }
                            return Ok(Ok(serde_json::from_str(envelope.payload.get())
                                .map_err(StoreError::from)?));
                        }
                        let busy = crate::server_outbox::project_action_pending(
                            tx,
                            "'add_location','remove_location'",
                            &project_id,
                        )?;
                        if busy {
                            return refuse(
                                "A location change is pending. Read its existing receipt",
                            );
                        }
                        let locations =
                            project["locations"].as_array().cloned().unwrap_or_default();
                        let location = locations
                            .iter()
                            .find(|row| row["serverId"].as_str() == Some(server_for_write.as_str()))
                            .cloned();
                        if action_for_write == "add_location"
                            && let Some(location) = &location
                            && location.get("requestedPath").unwrap_or(&location["path"])
                                != &json!(path)
                        {
                            return refuse("This project already has a location on that server");
                        }
                        if action_for_write == "remove_location"
                            && location.is_some()
                            && locations.len() == 1
                        {
                            return refuse("Keep at least one project location");
                        }
                        use studio_runtime_schema::queue_command::Field;
                        let typed_location = location
                            .as_ref()
                            .map(|location| {
                                let required = |key: &str| {
                                    location
                                        .get(key)
                                        .and_then(Value::as_str)
                                        .map(str::to_owned)
                                        .ok_or_else(|| {
                                            Step::Refused(
                                                "Stored project location is invalid".into(),
                                            )
                                        })
                                };
                                let optional = |key: &str| match location.get(key) {
                                    None => Ok(Field::Missing),
                                    Some(Value::Null) => Ok(Field::Null),
                                    Some(Value::String(value)) => Ok(Field::Value(value.clone())),
                                    Some(_) => Err(Step::Refused(
                                        "Stored project location is invalid".into(),
                                    )),
                                };
                                Ok::<_, Step>(crate::project_locations::LocationReference {
                                    server_id: required("serverId")?,
                                    path: required("path")?,
                                    project_id: required("projectId")?,
                                    git_origin: optional("gitOrigin")?,
                                    requested_path: optional("requestedPath")?,
                                })
                            })
                            .transpose()?;
                        ProjectsRepository::put_location_request(
                            tx,
                            &project_id,
                            &key_for_write,
                            &admitted,
                        )?;
                        let payload = crate::project_locations::LocationPayload {
                            project: project_id.clone(),
                            name: match project.get("name") {
                                None | Some(Value::Null) => Field::Null,
                                Some(Value::String(value)) => Field::Value(value.clone()),
                                Some(_) => return refuse("Stored project name is invalid"),
                            },
                            path: path.clone().map_or(Field::Null, Field::Value),
                            request: Field::Value(request.wire()),
                            signature: Field::Missing,
                            result_id: Field::Missing,
                            location: typed_location.map_or(Field::Missing, Field::Value),
                            admission: Field::Value(crate::project_locations::location_admission(
                                tx,
                                &project_id,
                                Some((&actor_id, epoch)),
                            )?),
                        };
                        Ok(Ok(payload))
                    },
                )
                .await?;
        let payload = match prepared {
            Err(previous) => return Ok(previous),
            Ok(payload) => payload,
        };
        let command = if action == "add_location" {
            crate::orchestration_wire::OrchestrationCommand::AddLocation(payload)
        } else {
            crate::orchestration_wire::OrchestrationCommand::RemoveLocation(payload)
        };
        let result = self.deliver(&server, command, &outbox_key).await?;
        Ok(result)
    }

    /// Python `codex_project_locations.suggestions`: never adds a location.
    async fn location_matches(&self, args: &Value, key: &str) -> Result<Value, Step> {
        let project_id = field(args, "project").to_owned();
        let project = self
            .read(move |tx| Ok(ProjectsRepository::get(tx, &project_id)?.map(|(_, value)| value)))
            .await?;
        let Some(project) = project else {
            return refuse("Select an existing project");
        };
        let server = field(args, "server");
        if !self.paired_ids().await?.iter().any(|id| id == server) {
            return refuse("The server is not paired or is revoked");
        }
        let mut origin = project["locations"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|row| {
                row.get("gitOrigin")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
            })
            .map(str::to_owned);
        if origin.is_none() {
            let path = field(&project, "path").to_owned();
            if !path.starts_with("project:") {
                origin = studio_runtime_process::blocking_pool::spawn_blocking(
                    "runtime-services/multi_server_tools",
                    move || crate::projects::peer_location_info(&path).ok(),
                )
                .await
                .ok()
                .flatten()
                .and_then(|info| info.git_origin);
            }
        }
        let Some(origin) = origin else {
            return Ok(json!({"matches": []}));
        };
        self.deliver(
            server,
            crate::orchestration_wire::OrchestrationCommand::LocationMatches(
                crate::orchestration_worker::LocationMatchesPayload {
                    origin,
                    cwd: studio_runtime_schema::queue_command::Field::Missing,
                },
            ),
            key,
        )
        .await
    }

    /// Python `_exec_tool`.
    async fn exec(
        &self,
        actor: &Actor,
        args: &Value,
        action: &str,
        key: &str,
        local: &str,
    ) -> Result<Value, Step> {
        let server = match args.get("server").and_then(Value::as_str) {
            None | Some("") | Some("local") => local.to_owned(),
            Some(server) => server.to_owned(),
        };
        if server != local && !self.paired_ids().await?.contains(&server) {
            return refuse("Select local or a paired server");
        }
        if server == local && !self.local_exec {
            return refuse("Not implemented: requires multi-server/local-exec");
        }
        let mut payload = Map::new();
        for name in EXEC_FIELDS {
            if let Some(value) = args.get(*name) {
                payload.insert((*name).to_owned(), value.clone());
            }
        }
        payload.insert("actor".to_owned(), json!(actor.record().id.0));
        use crate::orchestration_exec::{CancelRequest, ExecRequest, InputRequest, ReadRequest};
        use crate::orchestration_wire::OrchestrationCommand;
        let payload = Value::Object(payload);
        let invalid = |_| Step::Refused("Invalid command request".into());
        let (command, handle) = match action {
            "exec" => {
                let mut request = ExecRequest::deserialize(payload).map_err(invalid)?;
                validate_exec(&request, server != local).map_err(Step::Refused)?;
                request.normalize_defaults();
                (
                    OrchestrationCommand::Exec(crate::orchestration_exec::ExecPayload::Start(
                        request,
                    )),
                    None,
                )
            }
            "exec_read" => {
                let request = ReadRequest::deserialize(payload).map_err(invalid)?;
                let handle = Some(request.handle.clone());
                (OrchestrationCommand::ExecRead(request), handle)
            }
            "exec_input" => {
                let request = InputRequest::deserialize(payload).map_err(invalid)?;
                let handle = Some(request.handle.clone());
                (OrchestrationCommand::ExecInput(request), handle)
            }
            "exec_cancel" => {
                let request = CancelRequest::deserialize(payload).map_err(invalid)?;
                let handle = Some(request.handle.clone());
                (OrchestrationCommand::ExecCancel(request), handle)
            }
            _ => return refuse("Invalid command action"),
        };
        let key = key.to_owned();
        let encoded = serde_json::to_vec(&crate::orchestration_wire::OrchestrationRequest {
            request_id: key.clone(),
            operation: Vec::new(),
            command: command.clone(),
        })
        .map_err(|_| Step::Refused("Invalid cross-server request".into()))?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return refuse("The encoded command request exceeds 256 KiB");
        }
        let actor_id = actor.record().id.0.clone();
        let epoch = actor.record().epoch;
        let server_for_check = server.clone();
        let action_for_check = action.to_owned();
        self.store
            .transaction_with(move |tx| -> Result<(), Step> {
                let current = checked_actor(tx, &actor_id)?;
                if current.is_lead != Some(true) || current.epoch != epoch {
                    return refuse("Only the active lead can use server commands");
                }
                if action_for_check != "exec" {
                    let Some(handle) = handle.as_ref() else {
                        return refuse("Supply a command handle");
                    };
                    let owned = crate::multi_server_orchestration::exec_handle_owned(
                        tx,
                        &server_for_check,
                        &actor_id,
                        handle,
                    )
                    .map_err(|error| match error {
                        AccessError::Store(error) => Step::Store(error),
                        AccessError::Sqlite(error) => Step::Store(error.into()),
                        other @ AccessError::Refused { .. }
                        | other @ AccessError::Stored { .. } => Step::Refused(other.to_string()),
                    })?;
                    if !owned {
                        return refuse("The command handle belongs to another actor or server");
                    }
                }
                Ok(())
            })
            .await?;
        self.deliver(&server, command, &key).await
    }

    /// Python `_fetch`: copy a remote worker branch into FETCH_HEAD of a local
    /// repository through a verified Git bundle.
    async fn fetch(
        &self,
        actor: &Actor,
        args: &Value,
        key: &str,
        server: &str,
    ) -> Result<Value, Step> {
        let worker_id = field(args, "agent_id").to_owned();
        let worker = self
            .read(move |tx| {
                Ok(AgentRepository::get(
                    tx,
                    &AgentId::from(worker_id.as_str()),
                )?)
            })
            .await?;
        let Some(worker) = worker else {
            return refuse("Select a remote worker in this team");
        };
        let remote = worker.extra.get("remoteWorker");
        if worker.root_id != actor.record().root_id
            || !studio_runtime_store::truthy(remote)
            || remote
                .and_then(|remote| remote.get("server"))
                .and_then(Value::as_str)
                != Some(server)
        {
            return refuse("Select a remote worker in this team");
        }
        let branch = match args.get("branch").and_then(Value::as_str) {
            Some(branch) if !branch.is_empty() && !branch.starts_with('-') => branch.to_owned(),
            _ => return refuse("Supply a worker branch"),
        };
        let destination = args
            .get("destination")
            .and_then(Value::as_str)
            .or_else(|| actor.record().cwd.as_deref())
            .unwrap_or_default()
            .to_owned();
        let branch_for_checks = branch.clone();
        let base = worker
            .extra
            .get("workerBaseCommit")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let (destination, known) = blocking(move || -> Result<(String, Vec<String>), Step> {
            let destination = resolve(&destination);
            let path = Path::new(&destination);
            if git(path, &["check-ref-format", "--branch", &branch_for_checks])?.1 != 0 {
                return refuse("Invalid worker branch");
            }
            if git(path, &["rev-parse", "--git-dir"])?.1 != 0 {
                return refuse("Supply an existing local Git repository");
            }
            let mut known = Vec::new();
            for reference in ["HEAD", "origin/main", "FETCH_HEAD"] {
                let object = format!("{reference}^{{commit}}");
                let (output, code) = git(path, &["rev-parse", "--verify", &object])?;
                let commit = String::from_utf8_lossy(&output).trim().to_owned();
                if code == 0 && commit_hash(&commit) {
                    known.push(commit);
                }
            }
            if let Some(base) = base.filter(|base| commit_hash(base)) {
                let object = format!("{base}^{{commit}}");
                if git(path, &["cat-file", "-e", &object])?.1 == 0 {
                    known.push(base);
                }
            }
            Ok((destination, known))
        })
        .await?;
        let signature = request_signature(&json!({
            "worker": worker.id.0,
            "branch": branch,
            "destination": destination,
        }));
        let export_key = key.to_owned();
        let key_owned = key.to_owned();
        let server_owned = server.to_owned();
        let signature_for_claim = signature.clone();
        let worker_cwd = worker.cwd.as_deref().map(str::to_owned);
        let mut known_sorted = known;
        known_sorted.sort();
        known_sorted.dedup();
        let branch_for_claim = branch.clone();
        let actor_for_fetch = actor.record().id.0.clone();
        let claim = self
            .store
            .transaction_with(move |tx| -> Result<Result<FetchPayload, Value>, Step> {
                let parent=task_admission(tx,&key_owned,&actor_for_fetch)?;
                let admitted=parent.derive_result("server-fetch",&[],&signature_for_claim);
                let now=(crate::event_write::unix_time().map_err(|_|Step::Refused("The system clock is unavailable".into()))?*1000.0) as i64;
                if let Some(saved)=crate::event_write::replay_request::<Box<serde_json::value::RawValue>>(tx,&key_owned,&signature_for_claim,now)?.or(crate::event_write::replay_request::<Box<serde_json::value::RawValue>>(tx,admitted.id(),&signature_for_claim,now)?) {
                    let _: FetchReply = serde_json::from_str(saved.reply.get()).map_err(StoreError::from)?;
                    return Ok(Err(serde_json::from_str(saved.reply.get()).map_err(StoreError::from)?));
                }
                if let Some(prior)=RemoteOutbox::find::<FetchEntry>(tx,RemoteEffect::ServerFetch,&key_owned).map_err(StoreError::from)? {
                    if prior.signature!=signature_for_claim {return refuse("This request id has different content");}
                    let state=fetch_state(&prior);
                    return Ok(match FetchPayload::decode(state,prior.record.export.as_deref())? {
                        Some(saved)=>Ok(saved),
                        None=>Err(json!({"requestId":key_owned,"outcome":"unknown","detail":"The original fetch inputs are unavailable. The operation will not be sent again."})),
                    });
                }
                let export=ExportRequest{cwd:worker_cwd.map_or(studio_runtime_schema::queue_command::Field::Null, studio_runtime_schema::queue_command::Field::Value),branch:branch_for_claim,known:studio_runtime_schema::queue_command::Field::Value(known_sorted)};
                RemoteOutbox::put(tx,RemoteEffect::ServerFetch,&server_owned,&key_owned,&signature_for_claim,OutboxStatus::Pending,None,FetchEntry{state:ServerFetchState::Preparing,export:Some(serde_json::to_string(&export).map_err(StoreError::from)?),admission:admitted},now).map_err(StoreError::from)?;
                Ok(Ok(FetchPayload::Preparing(export)))
            })
            .await?;
        let preparation = match claim {
            Err(previous) => return Ok(previous),
            Ok(preparation) => preparation,
        };
        let export = preparation.export().clone();
        let outcome = self
            .deliver_exchange(
                server,
                crate::orchestration_wire::OrchestrationCommand::Export(export),
                &export_key,
            )
            .await?;
        if matches!(preparation, FetchPayload::Fetching(_)) {
            return Ok(
                json!({"requestId":key,"outcome":"unknown","detail":"The local Git fetch has no final receipt. Inspect FETCH_HEAD; do not repeat it."}),
            );
        }
        let ExchangeOutcome::Applied(reply) = outcome else {
            return Ok(receipt(&outcome, key));
        };
        let metadata: crate::server_git::ExportReply = serde_json::from_str(reply.value.get())
            .map_err(|_| Step::Refused("Invalid remote bundle metadata".into()))?;
        if metadata.branch != branch {
            return refuse("Invalid remote bundle branch");
        }
        let already = metadata
            .already_have
            .clone()
            .filter(|value| !value.is_empty());
        if let Some(already) = &already {
            let destination = destination.clone();
            let already = already.clone();
            let present = blocking(move || {
                Ok(commit_hash(&already)
                    && git(
                        Path::new(&destination),
                        &["cat-file", "-e", &format!("{already}^{{commit}}")],
                    )?
                    .1 == 0)
            })
            .await?;
            if !present {
                return refuse("The advertised commit is absent from the local repository");
            }
        }
        let bytes = match metadata.bytes {
            0 if already.is_some() => 0,
            bytes if bytes > 0 && bytes <= MAX_BUNDLE => bytes,
            _ => return refuse("Invalid remote bundle size"),
        };
        let folder = tempfile::Builder::new()
            .prefix("studio-server-fetch-")
            .tempdir()
            .map_err(io)?;
        let bundle = folder.path().join("worker.bundle");
        if crate::disk_space::free_bytes(folder.path())
            .is_some_and(|free| free < bytes.saturating_mul(2))
        {
            return Err(Step::Failed(
                "Insufficient disk space for the remote Git bundle".into(),
            ));
        }
        let mut digest = Sha256::new();
        let mut data_file = std::fs::File::create(&bundle).map_err(io)?;
        let mut offset: u64 = 0;
        while offset < bytes {
            let chunk_key = key.to_owned();
            let chunk_receipt = self
                .deliver_operation(
                    server,
                    &[offset.to_string()],
                    crate::orchestration_wire::OrchestrationCommand::Chunk(
                        crate::server_git::ChunkRequest {
                            export: metadata
                                .export
                                .as_deref()
                                .ok_or_else(|| {
                                    Step::Refused("Invalid remote bundle export".into())
                                })?
                                .to_owned(),
                            offset,
                        },
                    ),
                    &chunk_key,
                )
                .await?;
            let ExchangeOutcome::Applied(reply) = chunk_receipt else {
                return Ok(receipt(&chunk_receipt, &chunk_key));
            };
            let chunk: crate::server_git::ChunkReply = serde_json::from_str(reply.value.get())
                .map_err(|_| Step::Refused("Invalid remote bundle chunk".into()))?;
            let data = base64::engine::general_purpose::STANDARD
                .decode(&chunk.data)
                .ok()
                .unwrap_or_default();
            let length = data.len() as u64;
            if data.is_empty()
                || data.len() > CHUNK
                || chunk.offset != offset
                || chunk.next_offset != offset + length
                || offset + length > bytes
                || data_encoding::HEXLOWER.encode(&Sha256::digest(&data)) != chunk.sha256
            {
                return refuse("Invalid remote bundle chunk");
            }
            use std::io::Write;
            data_file.write_all(&data).map_err(io)?;
            digest.update(&data);
            offset += length;
        }
        data_file.sync_all().map_err(io)?;
        drop(data_file);
        if already.is_none()
            && Some(data_encoding::HEXLOWER.encode(&digest.finalize()).as_str())
                != metadata.sha256.as_deref()
        {
            return refuse("The remote bundle checksum differs");
        }
        if already.is_none() {
            let destination = destination.clone();
            let bundle = bundle.clone();
            blocking(move || {
                let command = git_command(
                    Path::new(&destination),
                    &["bundle", "verify", &bundle.to_string_lossy()],
                    false,
                )
                .map_err(git_step)?;
                git_run(&command, BUNDLE_VERIFY_SECONDS).map_err(git_step)
            })
            .await?;
        }
        let key_owned = key.to_owned();
        let claimed = self
            .store
            .transaction_with(move |tx| -> Result<usize, Step> {
                let Some(entry) =
                    RemoteOutbox::find::<FetchEntry>(tx, RemoteEffect::ServerFetch, &key_owned)
                        .map_err(StoreError::from)?
                        .filter(|entry| {
                            entry.status == OutboxStatus::Pending
                                && entry.record.state == ServerFetchState::Preparing
                        })
                else {
                    return Ok(0);
                };
                let record = FetchEntry {
                    state: ServerFetchState::Fetching,
                    ..entry.record
                };
                let changed = RemoteOutbox::update(
                    tx,
                    RemoteEffect::ServerFetch,
                    &entry.target,
                    &key_owned,
                    &entry.signature,
                    OutboxStatus::Submitted,
                    None,
                    record,
                )
                .map_err(StoreError::from)?;
                Ok(usize::from(changed))
            })
            .await?;
        if claimed == 0 {
            return Ok(json!({
                "requestId": key,
                "outcome": "unknown",
                "detail": "The original local Git fetch is already reserved.",
            }));
        }
        let source = match &already {
            Some(_) => destination.clone(),
            None => CanonicalPath::existing(&bundle)
                .map(CanonicalPath::into_path_buf)
                .unwrap_or(bundle.clone())
                .to_string_lossy()
                .into_owned(),
        };
        let reference = already
            .clone()
            .unwrap_or_else(|| format!("refs/heads/{branch}"));
        let destination_for_fetch = destination.clone();
        let commit = blocking(move || {
            let path = Path::new(&destination_for_fetch);
            let command = git_command(path, &["fetch", "--no-tags", &source, &reference], true)
                .map_err(git_step)?;
            git_run(&command, BUNDLE_FETCH_SECONDS).map_err(git_step)?;
            let (output, code) = git(path, &["rev-parse", "FETCH_HEAD"])?;
            let commit = String::from_utf8_lossy(&output).trim().to_owned();
            if code != 0 || !commit_hash(&commit) {
                return Err(Step::Failed(
                    "Git did not return a final fetch commit".into(),
                ));
            }
            Ok(commit)
        })
        .await?;
        drop(folder);
        let value = json!({
            "requestId": key,
            "server": server,
            "branch": branch,
            "commit": commit,
            "bytes": bytes,
            "destination": destination,
        });
        let reply = FetchReply {
            request_id: key.to_owned(),
            server: server.to_owned(),
            branch,
            commit,
            bytes,
            destination,
        };
        let release_key = key.to_owned();
        let key_owned = key.to_owned();
        let server_owned = server.to_owned();
        let release = if already.is_none() {
            Some(crate::server_git::ReleaseRequest {
                export: metadata
                    .export
                    .as_deref()
                    .ok_or_else(|| Step::Refused("Invalid remote bundle export".into()))?
                    .to_owned(),
            })
        } else {
            None
        };
        let release_for_write = release.clone();
        let release_key_for_write = release_key.clone();
        let now = crate::event_write::unix_time().unwrap_or_default() as i64;
        self.store
            .transaction_with(move |tx| -> Result<(), Step> {
                let entry =
                    RemoteOutbox::find::<FetchEntry>(tx, RemoteEffect::ServerFetch, &key_owned)
                        .map_err(StoreError::from)?
                        .ok_or_else(|| Step::Refused("The fetch reservation is missing".into()))?;
                crate::event_write::settle_request(tx, &entry.record.admission, None, &reply)?;
                RemoteOutbox::delete(
                    tx,
                    RemoteEffect::ServerFetch,
                    &entry.target,
                    &key_owned,
                    &entry.signature,
                )
                .map_err(StoreError::from)?;
                if let Some(release) = &release_for_write {
                    crate::multi_server_orchestration::enqueue_in_transaction(
                        tx,
                        &server_owned,
                        crate::orchestration_wire::OrchestrationCommand::Release(release.clone()),
                        &release_key_for_write,
                        now,
                    )
                    .map_err(|error| match error {
                        AccessError::Store(error) => Step::Store(error),
                        AccessError::Sqlite(error) => Step::Store(error.into()),
                        other @ AccessError::Refused { .. }
                        | other @ AccessError::Stored { .. } => Step::Refused(other.to_string()),
                    })?;
                }
                Ok(())
            })
            .await?;
        if let Some(release) = release {
            // The receipt above is final. A failed release stays queued in the
            // outbox, and the peer removes an unreleased export after a day.
            if self
                .deliver(
                    server,
                    crate::orchestration_wire::OrchestrationCommand::Release(release),
                    &release_key,
                )
                .await
                .is_err()
            {}
        }
        Ok(value)
    }
}

impl MultiServerToolPort for ServerTools {
    fn servers<'a>(
        &'a self,
        actor: &'a Actor,
        args: &'a Value,
        key: &'a str,
        _now: f64,
    ) -> BoxFuture<'a, Result<DispatchedOperation, Error>> {
        Box::pin(async move { settle(self.run(actor, args, key).await) })
    }

    fn worker_call<'a>(
        &'a self,
        actor: &'a Actor,
        action: &'a str,
        args: &'a Value,
        key: &'a str,
    ) -> BoxFuture<'a, Result<DispatchedOperation, Error>> {
        Box::pin(async move { settle(self.worker(actor, action, args, key).await) })
    }
}

impl ServerTools {
    /// Python `worker_call`. The home server answers with a receipt: a refusal
    /// is the tool's refusal, an unknown outcome stays unknown and saved.
    async fn worker(
        &self,
        actor: &Actor,
        action: &str,
        args: &Value,
        key: &str,
    ) -> Result<Value, Step> {
        let origin = actor.record().extra.get("remoteOrigin");
        let home = origin
            .and_then(|origin| origin.get("home"))
            .and_then(Value::as_str);
        let link = origin
            .and_then(|origin| origin.get("link"))
            .and_then(Value::as_str);
        let (Some(home), Some(link)) = (home.map(str::to_owned), link.map(str::to_owned)) else {
            return refuse("Unknown remote-parent link");
        };
        let key = key.to_owned();
        let actor_id = actor.record().id.0.clone();
        let epoch = actor.record().epoch;
        let link_for_read = link.clone();
        let parent_epoch = self
            .read(move |tx| {
                let current = checked_actor(tx, &actor_id)?;
                if current.epoch != epoch {
                    return refuse("The worker was stopped");
                }
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT record FROM runtime_server_links WHERE id=?1",
                        [&link_for_read],
                        |row| row.get(0),
                    )
                    .optional()?;
                let Some(raw) = raw else {
                    return refuse("Unknown remote-parent link");
                };
                let record: Value = serde_json::from_str(&raw)
                    .map_err(|_| Step::Refused("The remote-parent link is invalid".into()))?;
                record
                    .get("parentEpoch")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| Step::Refused("The remote-parent link is invalid".into()))
            })
            .await?;
        use crate::orchestration_wire::OrchestrationCommand;
        use crate::orchestration_worker::{MoveSend, WorkerCall, WorkerMessage};
        let command = match action {
            "message" => OrchestrationCommand::Message(WorkerCall {
                link,
                worker: actor.record().id.0.clone(),
                epoch,
                parent_epoch,
                args: <WorkerMessage as serde::Deserialize>::deserialize(args)
                    .map_err(|_| Step::Refused("Invalid message arguments".into()))?,
            }),
            "move_send" => OrchestrationCommand::MoveSend(WorkerCall {
                link,
                worker: actor.record().id.0.clone(),
                epoch,
                parent_epoch,
                args: <MoveSend as serde::Deserialize>::deserialize(args)
                    .map_err(|_| Step::Refused("Invalid move-send request".into()))?,
            }),
            _ => {
                let kind = action
                    .parse()
                    .map_err(|_| Step::Refused("Invalid cross-server action".into()))?;
                OrchestrationCommand::remaining(kind, json!({"link":link,"worker":actor.record().id.0,"epoch":epoch,"parentEpoch":parent_epoch,"args":args}))
                    .ok_or_else(|| Step::Refused("Invalid cross-server action".into()))?
            }
        };
        let mut receipt = self.deliver(&home, command, &key).await?;
        match receipt["outcome"].as_str() {
            Some("not_applied") => refuse(field(&receipt, "error")),
            Some("applied") => Ok(receipt["value"].take()),
            _ => {
                receipt["detail"] = json!("The request is saved. Do not submit another identity.");
                Ok(receipt)
            }
        }
    }
}

/// Python `Runtime.dynamic` and `chat_message`: the tool calls of a remote
/// worker (`remoteOrigin`) that its home server answers, as (worker-call
/// action, call arguments). `None` runs the tool on this server. `Err` is a
/// refusal before any write. Arguments are never resolved on this server.
pub(crate) fn worker_route(
    tool: &str,
    args: &Value,
    actor: &studio_runtime_store::AgentRecord,
) -> Option<Result<(&'static str, Value), &'static str>> {
    if !studio_runtime_store::truthy(actor.extra.get("remoteOrigin")) {
        return None;
    }
    let moved = studio_runtime_store::truthy(actor.extra.get("movedFrom"));
    match tool {
        "orchestration_agent_manage" if moved && args.get("action") != Some(&json!("park")) => {
            Some(Err("The home server does not support this action"))
        }
        "orchestration_context"
            if matches!(
                args.get("topic").and_then(Value::as_str),
                Some("plan" | "complaints")
            ) =>
        {
            Some(Err("The home server does not support this action"))
        }
        "orchestration_task" | "orchestration_complaint" => {
            Some(Err("The home server does not support this action"))
        }
        "orchestration_interrupt" | "orchestration_spawn" if moved => {
            Some(Err("The home server does not support this action"))
        }
        "orchestration_send" if moved => Some(Ok(("move_send", args.clone()))),
        "orchestration_status" | "orchestration_peers" => {
            Some(Err("The home server does not support this action"))
        }
        "orchestration_chat_read" => {
            let room = args
                .get("room_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (!room.starts_with("remote:") && !room.starts_with("federated:"))
                .then_some(Err("The home server does not support this action"))
        }
        "orchestration_message" => {
            let target = args.get("target").and_then(Value::as_str)?;
            if target == "user" || target.starts_with("federated:") || target.starts_with("remote:")
            {
                return None;
            }
            Some(worker_message(target, args))
        }
        _ => None,
    }
}

/// Python `chat_message` checks before its `worker_call('message', ...)`.
pub(crate) fn worker_message(
    target: &str,
    args: &Value,
) -> Result<(&'static str, Value), &'static str> {
    let importance = match args.get("importance") {
        None => "message",
        Some(value) => value.as_str().unwrap_or_default(),
    };
    if !matches!(
        importance,
        "message" | "progress" | "question" | "blocker" | "result"
    ) {
        return Err("Unknown message importance");
    }
    let delivery = studio_runtime_schema::DeliveryMoment::for_message(args)
        .map_err(|_| "Invalid delivery moment")?;
    let progress_key = args.get("progress_key").filter(|value| !value.is_null());
    let progress_version = args
        .get("progress_version")
        .filter(|value| !value.is_null());
    if (progress_key.is_some() || progress_version.is_some())
        && (importance != "progress"
            || !progress_key
                .and_then(Value::as_str)
                .is_some_and(|key| (1..=200).contains(&key.chars().count()))
            || !progress_version.is_some_and(|version| version.is_u64()))
    {
        return Err(
            "Versioned progress requires a progress_key and nonnegative integer progress_version",
        );
    }
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .map(crate::event_write::python_strip)
        .filter(|text| (1..=12_000).contains(&text.chars().count()))
        .ok_or("Message must have 1 to 12000 characters")?;
    Ok((
        "message",
        json!({
            "target": target,
            "text": text,
            "importance": importance,
            "progress_key": progress_key,
            "progress_version": progress_version,
            "delivery": delivery,
        }),
    ))
}

/// Python `checked_actor(db, actor, actor)`.
fn checked_actor(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<studio_runtime_store::AgentRecord, Step> {
    let Some(agent) = AgentRepository::get(tx, &AgentId::from(id))? else {
        return refuse("Unknown agent");
    };
    if agent.is_deleted() {
        return refuse("This agent was deleted");
    }
    if agent.auto_wake != Some(true) {
        return refuse("The caller is stopped");
    }
    Ok(agent)
}

/// Python `expired_receipt`.
fn resolve(path: &str) -> String {
    let expanded = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest.trim_start_matches('/')))
            .unwrap_or_else(|| PathBuf::from(path)),
        _ => PathBuf::from(path),
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&expanded))
            .unwrap_or(expanded)
    };
    studio_runtime_store::path_identity::CanonicalPath::ancestor_nonstrict(&absolute)
        .map(CanonicalPath::into_path_buf)
        .unwrap_or(absolute)
        .to_string_lossy()
        .into_owned()
}

/// A Git failure after the fetch claim leaves its outcome unknown.
fn git_step(error: server_git::GitError) -> Step {
    match error {
        server_git::GitError::Refused(message) => Step::Refused(message),
        server_git::GitError::Failed(message) => Step::Failed(message),
    }
}

/// A Git read before the fetch claim: any failure proves nothing was fetched.
fn git(path: &Path, arguments: &[&str]) -> Result<(Vec<u8>, i32), Step> {
    git_read(path, arguments).map_err(|error| Step::Refused(error.to_string()))
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, Step> + Send + 'static,
) -> Result<T, Step> {
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/multi_server_tools",
        operation,
    )
    .await
    .map_err(|_| Step::Store(StoreError::WriterPoisoned))?
}

/// Python `codex_server_exec.validate`.
pub(crate) fn validate_exec(
    payload: &crate::orchestration_exec::ExecRequest,
    allow_foreign_windows_path: bool,
) -> Result<(), String> {
    use crate::orchestration_exec::CommandInput;
    let cwd = &payload.cwd;
    if cwd.contains('\0')
        || !(Path::new(cwd).is_absolute() || (allow_foreign_windows_path && windows_absolute(cwd)))
    {
        return Err("Supply an absolute cwd on the selected server".into());
    }
    if cwd.split(['/', '\\']).any(|part| part == "..") {
        return Err("The cwd must not contain parent traversal".into());
    }
    match &payload.command {
        CommandInput::Shell(command)
            if !command.is_empty() && !command.contains('\0') && command.len() <= 128 * 1024 => {}
        CommandInput::Argv(argv)
            if (1..=256).contains(&argv.len())
                && !argv[0].is_empty()
                && argv.iter().all(|value| !value.contains('\0'))
                && argv.iter().map(String::len).sum::<usize>() <= 128 * 1024 => {}
        CommandInput::Shell(_) => return Err("Supply a command of 1 to 128 KiB without NUL".into()),
        CommandInput::Argv(_) => {
            return Err("Supply command argv of 1 to 256 strings, at most 128 KiB".into());
        }
    }
    if payload.env.as_ref().is_some_and(|env| {
        env.iter()
            .any(|(key, value)| !valid_environment_name(key) || value.contains('\0'))
            || serde_json::to_vec(env).map_or(true, |bytes| bytes.len() > 128 * 1024)
    }) {
        return Err(
            "Supply env additions with valid names and string values, at most 128 KiB".into(),
        );
    }
    if !(1..=1800).contains(
        &payload
            .timeout
            .unwrap_or_else(crate::orchestration_exec::default_timeout),
    ) {
        return Err("Supply timeout seconds from 1 to 1800".into());
    }
    if !(2..=4 * 1024 * 1024).contains(
        &payload
            .output_limit
            .unwrap_or_else(crate::orchestration_exec::default_output_limit),
    ) {
        return Err("Supply output_limit bytes from 2 to 4194304".into());
    }
    Ok(())
}

/// Python `re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', key)`.
fn valid_environment_name(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Python `PureWindowsPath(cwd).is_absolute()`: a drive and a root, or a UNC share.
fn windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    let separator = |byte: u8| byte == b'\\' || byte == b'/';
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && separator(bytes[2])
    {
        return true;
    }
    if bytes.len() >= 2 && separator(bytes[0]) && separator(bytes[1]) {
        let rest = &path[2..];
        let mut parts = rest.split(['\\', '/']);
        return matches!((parts.next(), parts.next()), (Some(host), Some(share)) if !host.is_empty() && !share.is_empty());
    }
    false
}

#[cfg(test)]
#[path = "multi_server_tools/tests.rs"]
mod tests;
