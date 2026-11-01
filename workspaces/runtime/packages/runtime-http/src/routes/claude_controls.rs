//! Claude session reads and durable settings controls.
use super::{
    extractors::{ApiJson, TypedQuery},
    manifest::{OperationStatus, RouteGroup, get, post},
};
use crate::{error::ApiError, router::HttpState};
use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use studio_runtime_providers::ProviderKind;
use studio_runtime_schema::claude_controls::{ClaudeControlCommand, ClaudeSessionRead};
use studio_runtime_services::claude_controls::{
    ControlError,
    service::{
        ControlBinding, ControlFuture, ControlInvocation, ControlReply, ControlSession, Controls,
    },
    store::StoredControls,
};
use studio_runtime_store::{
    AgentId, AgentRepository, RuntimeStore, workspace_operations::WorkspaceOperationsRepository,
};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get(
            "/api/claude/repair-report",
            OperationStatus::Implemented,
            repair_report,
        ),
        post("/api/claude/session", OperationStatus::Implemented, session),
        post(
            "/api/claude/file-backups",
            OperationStatus::Implemented,
            file_backups,
        ),
    ])
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RepairReportQuery {
    id: studio_runtime_schema::bounded_text::BoundedText<1, { usize::MAX }>,
}
async fn repair_report(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<RepairReportQuery>,
) -> Result<Json<crate::generated::ClaudeRepairReport>, ApiError> {
    use studio_runtime_services::claude_controls::service::ControlScope;
    let id = query.id.as_str();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let (agent, _) = load_agent(store, id.to_owned()).await?;
    require_claude_account(&state, &agent).await?;
    if agent.deleted_at.is_some() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "This conversation was deleted",
        ));
    }
    let Some(thread) = state_thread(&agent) else {
        return super::generated_json(json!({"status":"none","skippedRows":0,"invalidSequence":false,"scopeConfirmed":null,"nativeHistoryConfirmed":null})).map(Json);
    };
    let account = agent
        .lazy_account_transfer
        .value()
        .and_then(|transfer| transfer.pending())
        .map(|transfer| transfer.source_account_key.as_str())
        .unwrap_or(&agent.account_key);
    let session = state
        .account_session
        .as_ref()
        .ok_or_else(ApiError::internal)?
        .claude_control_session(account)
        .map_err(session_error)?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        session.call_scoped(
            ControlScope {
                agent: id.to_owned(),
                epoch: u64::try_from(agent.epoch).unwrap_or(0),
                request_id: None,
            },
            ControlInvocation::RepairReport {
                thread_id: thread.to_owned(),
            },
        ),
    )
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "Could not read the saved repair report",
        )
    })?
    .map_err(control_error)?;
    let ControlReply::RepairReport(result) = result else {
        return Err(ApiError::internal());
    };
    // The provider reply may carry paths or text. Only the report fields leave.
    let Value::Object(mut reply) = result else {
        return Err(ApiError::internal());
    };
    reply.retain(|name, _| {
        matches!(
            name.as_str(),
            "status"
                | "skippedRows"
                | "invalidSequence"
                | "scopeConfirmed"
                | "nativeHistoryConfirmed"
        )
    });
    let evidence: crate::generated::ClaudeRepairReport =
        serde::Deserialize::deserialize(Value::Object(reply)).map_err(|_| ApiError::internal())?;
    if !(0..=studio_runtime_schema::shared_values::MAX_SAFE_INTEGER)
        .contains(&evidence.skipped_rows)
    {
        return Err(ApiError::internal());
    }
    Ok(Json(evidence))
}

fn nonempty_text<'de, D: serde::Deserializer<'de>, const MAX: usize>(
    deserializer: D,
) -> Result<String, D::Error> {
    let value = <studio_runtime_schema::bounded_text::BoundedText<1, MAX> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(value.as_str().to_owned())
}
fn command_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    nonempty_text::<D, { usize::MAX }>(d)
}
fn backup_agent_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    nonempty_text::<D, { studio_runtime_schema::http_bounded_ingress::BACKUP_AGENT_ID_MAX_CHARS }>(
        d,
    )
}
fn backup_request_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    nonempty_text::<D, { studio_runtime_schema::http_bounded_ingress::BACKUP_REQUEST_ID_MAX_CHARS }>(
        d,
    )
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionCommand {
    #[serde(rename = "action")]
    _action: crate::generated::ClaudeSessionCommandRequestAction,
    id: String,
    command: String,
    #[serde(deserialize_with = "command_id")]
    request_id: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupsRequest {
    #[serde(deserialize_with = "backup_agent_id")]
    id: String,
    release: bool,
    #[serde(deserialize_with = "backup_request_id")]
    request_id: String,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum SessionRequest {
    Settings(ClaudeControlCommand),
    Command(SessionCommand),
    Read(ClaudeSessionRead),
}

#[cfg(test)]
mod session_request_tests {
    use super::SessionRequest;

    #[test]
    fn session_request_rejects_extra_fields_and_unbounded_stop_identity() {
        assert!(
            serde_json::from_str::<SessionRequest>(
                r#"{"id":"agent","action":"command","command":"/help","request_id":"id"}"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<SessionRequest>(
                r#"{"id":"agent","action":"command","command":"/help","request_id":""}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<super::BackupsRequest>(
                r#"{"id":"a","release":true,"request_id":"r"}"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<super::BackupsRequest>(
                r#"{"id":"a","release":true,"request_id":"r","extra":1}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<super::BackupsRequest>(
                r#"{"id":"a","release":true,"request_id":""}"#
            )
            .is_err()
        );
        for body in [
            r#"{"id":"agent","action":"state","extra":true}"#.to_owned(),
            r#"{"id":"agent","action":"command","command":"/help","request_id":"id","extra":true}"#
                .to_owned(),
            format!(
                r#"{{"id":"agent","action":"stop_task","task_id":"{}","request_id":"id"}}"#,
                "x".repeat(1025)
            ),
        ] {
            assert!(
                serde_json::from_str::<SessionRequest>(&body).is_err(),
                "{body}"
            );
        }
    }
}

async fn session(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<SessionRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = match &body {
        SessionRequest::Settings(request) => request.id(),
        SessionRequest::Command(request) => &request.id,
        SessionRequest::Read(request) => request.id(),
    };
    if id.is_empty() {
        return Err(ApiError::validation(vec![json!("body"), json!("id")]));
    }
    let id = id.to_owned();
    let is_state = matches!(&body, SessionRequest::Read(ClaudeSessionRead::State { .. }));
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let (agent, operation) = load_agent(store.clone(), id).await?;
    require_claude_account(&state, &agent).await?;
    let snapshot_operation = match &body {
        SessionRequest::Command(_) => None,
        SessionRequest::Settings(_) | SessionRequest::Read(_) => operation.clone(),
    };
    let snapshot = studio_runtime_services::claude_controls::ControlSnapshot::from_stored(
        agent.value(),
        snapshot_operation.as_ref(),
    )
    .map_err(control_error)?;
    let account = if is_state {
        agent
            .lazy_account_transfer
            .value()
            .and_then(|lazy| lazy.pending())
            .map(|lazy| lazy.source_account_key.as_str())
            .unwrap_or(&agent.account_key)
    } else {
        &agent.account_key
    };
    let no_thread = agent.thread_id.as_deref().is_none_or(str::is_empty);
    let local_only = (is_state && state_thread(&agent).is_none())
        || (matches!(&body, SessionRequest::Settings(_)) && no_thread);
    if matches!(
        &body,
        SessionRequest::Read(ClaudeSessionRead::StopTask { .. })
    ) && no_thread
    {
        return Err(control_error(ControlError::Request));
    }
    // Native commands use the message service and need no provider control session.
    let controls = || -> Result<Controls, ApiError> {
        let session: Arc<dyn ControlSession> = if local_only {
            Arc::new(LocalControlSession {
                binding: ControlBinding::new(account.to_owned(), "local-no-thread".into(), 1)
                    .map_err(control_error)?,
            })
        } else {
            let broker = state
                .account_session
                .as_ref()
                .ok_or_else(|| ApiError::dependency("provider-session/claude-controls"))?;
            broker
                .claude_control_session(account)
                .map_err(session_error)?
        };
        Ok(Controls::new(
            Arc::new(StoredControls::new(store.clone(), Arc::new(|_| {}))),
            session,
        ))
    };
    let result = match body {
        SessionRequest::Command(request) => {
            return command(state, request).await;
        }
        SessionRequest::Settings(request) => controls()?.execute(request).await,
        SessionRequest::Read(request) => {
            controls()?.read_or_stop_snapshot(&snapshot, &request).await
        }
    };
    state.event_wake.notify_one();
    let result = result.map_err(control_error)?;
    Ok(Json(result))
}

async fn file_backups(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<BackupsRequest>,
) -> Result<Json<crate::generated::ClaudeFileBackupsResponse>, ApiError> {
    tokio::spawn(async move {
        use studio_runtime_services::claude_controls::service::ControlScope;
        use studio_runtime_store::ClaudeControlRepository;
        let store = state.store.clone().ok_or_else(ApiError::internal)?;
        let _execution = studio_runtime_services::checkpoints::RestoreExecution::acquire(format!(
            "{}:{}",
            store.state_dir().display(),
            body.id
        ))
        .map_err(|_| control_error(ControlError::Busy))?;
        let id = body.id.clone();
        let (agent, settled) = store
            .read_async("claude/file-backups", move |tx| {
                let agent = AgentRepository::get(tx, &AgentId(id))?.ok_or_else(|| {
                    studio_runtime_store::StoreError::InvalidStoredRecord {
                        object: "agent",
                        id: "missing".into(),
                    }
                })?;
                let settled = ClaudeControlRepository::settled_file_rewinds(tx, &agent.value())?;
                Ok::<_, studio_runtime_store::StoreError>((agent, settled))
            })
            .await
            .map_err(|_| ApiError::internal())?;
        require_claude_account(&state, &agent).await?;
        let snapshot = studio_runtime_services::claude_controls::ControlSnapshot::from_records(
            agent.value(),
            None,
        )
        .map_err(control_error)?;
        let thread = agent
            .thread_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| control_error(ControlError::Request))?;
        let account = &agent.account_key;
        let session = state
            .account_session
            .as_ref()
            .ok_or_else(ApiError::internal)?
            .claude_control_session(account)
            .map_err(session_error)?;
        let result = session
            .call_scoped(
                ControlScope {
                    agent: body.id,
                    epoch: snapshot.epoch(),
                    request_id: Some(body.request_id.clone()),
                },
                ControlInvocation::FileBackups {
                    thread_id: thread.to_owned(),
                    settled_request_ids: if body.release { Some(settled) } else { None },
                    request_id: body.request_id,
                },
            )
            .await
            .map_err(control_error)?;
        let ControlReply::FileBackups(result) = result else {
            return Err(ApiError::internal());
        };
        super::generated_json::<crate::generated::ClaudeFileBackupsResponse>(result).map(Json)
    })
    .await
    .map_err(|_| ApiError::internal())?
}

async fn require_claude_account(
    state: &HttpState,
    agent: &studio_runtime_store::AgentRecord,
) -> Result<(), ApiError> {
    let account_key = agent.account_key.clone();
    let driver = state.account_driver.clone();
    let policy = state.account_policy.clone();
    let accounts = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/routes/claude_controls/account",
        move || match driver {
            Some(driver) => driver.snapshot().map_err(|_| ApiError::internal()),
            None => policy
                .map(|policy| policy.accounts())
                .ok_or_else(ApiError::internal),
        },
    )
    .await
    .map_err(|_| ApiError::internal())??;
    let provider = accounts
        .accounts
        .iter()
        .find(|account| account.id.as_str() == Some(account_key.as_str()))
        .and_then(|account| account.provider.as_str());
    if provider != Some(ProviderKind::Claude.key()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Select a Claude Code chat",
        ));
    }
    Ok(())
}

fn state_thread(agent: &studio_runtime_store::AgentRecord) -> Option<&str> {
    let transfer = agent.lazy_account_transfer.value();
    if let Some(transfer) = transfer {
        transfer
            .pending()
            .and_then(|transfer| transfer.source_thread_id.value())
            .map(String::as_str)
            .filter(|thread| !thread.is_empty())
    } else {
        agent
            .thread_id
            .as_deref()
            .filter(|thread| !thread.is_empty())
    }
}

struct LocalControlSession {
    binding: ControlBinding,
}

impl ControlSession for LocalControlSession {
    fn binding(&self) -> ControlBinding {
        self.binding.clone()
    }

    fn call_scoped(
        &self,
        _scope: studio_runtime_services::claude_controls::service::ControlScope,
        _call: ControlInvocation,
    ) -> ControlFuture<'_, ControlReply> {
        Box::pin(async { Err(ControlError::Request) })
    }
}

async fn load_agent(
    store: Arc<RuntimeStore>,
    id: String,
) -> Result<
    (
        studio_runtime_store::AgentRecord,
        Option<studio_runtime_store::WorkspaceOperationRecord>,
    ),
    ApiError,
> {
    let loaded = store
        .read_async("runtime-http/routes/claude_controls", move |tx| {
            let Some(agent) = AgentRepository::get(tx, &AgentId::from(id.as_str()))? else {
                return Ok(None);
            };
            let value = agent.value();
            let operation = value
                .get("workspaceReservationId")
                .and_then(Value::as_str)
                .filter(|id| id.starts_with("claude-control:"))
                .map(|id| WorkspaceOperationsRepository::operation(tx, id))
                .transpose()?
                .flatten();
            Ok::<_, studio_runtime_store::StoreError>(Some((agent, operation)))
        })
        .await
        .map_err(|_| ApiError::internal())?;
    loaded.ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Unknown managed agent"))
}

async fn command(state: HttpState, body: SessionCommand) -> Result<Json<Value>, ApiError> {
    let command = studio_runtime_services::claude_controls::service::native_command(&body.command)
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Supply one explicit Claude command",
            )
        })?;
    let message = crate::generated::ManagedMessageBody {
        id: body.request_id,
        room: body.id,
        text: command.into(),
        assets: None,
        delivery: Some(studio_runtime_schema::DeliveryMoment::CurrentTurn),
    };
    let receipt = super::messages::send_message(state, message).await?;
    serde_json::to_value(receipt.0)
        .map(Json)
        .map_err(|_| ApiError::internal())
}

fn control_error(error: ControlError) -> ApiError {
    match error {
        ControlError::Invalid(_) | ControlError::Unavailable => ApiError::internal(),
        ControlError::Request => {
            ApiError::new(StatusCode::BAD_REQUEST, "Invalid Claude control request")
        }
        ControlError::Busy => ApiError::new(
            StatusCode::BAD_REQUEST,
            "Pause Claude before changing its session",
        ),
        ControlError::Changed => ApiError::new(
            StatusCode::CONFLICT,
            "Claude session changed before the action",
        ),
        ControlError::Conflict => ApiError::new(
            StatusCode::CONFLICT,
            "This request identity has different content",
        ),
        ControlError::Unknown => ApiError::new(
            StatusCode::CONFLICT,
            "Claude control outcome is unknown; inspect workspace recovery",
        ),
        ControlError::RewindNotApplied(reason) | ControlError::SettingsNotApplied(reason) => {
            ApiError::new(StatusCode::BAD_REQUEST, reason)
        }
        ControlError::RewindRejected(reason) => ApiError::new(
            StatusCode::CONFLICT,
            format!("Claude file restore requires recovery: {reason}"),
        ),
        ControlError::NotSent => ApiError::new(
            StatusCode::BAD_REQUEST,
            "Claude control request was not sent",
        ),
    }
}

fn session_error(
    error: studio_runtime_services::account_session_runtime::AccountSessionError,
) -> ApiError {
    use studio_runtime_services::account_session_runtime::AccountSessionError as Error;
    match error {
        Error::Expired => ApiError::new(StatusCode::GONE, error.to_string()),
        Error::Requires(dependency) => ApiError::dependency(dependency),
        Error::Refused(message) => ApiError::new(StatusCode::BAD_REQUEST, message),
        Error::InvalidResponse(_) | Error::Persistence | Error::Unknown => ApiError::internal(),
    }
}

inventory::submit! {crate::routes::manifest::RouteModuleRegistration{name:"claude_controls",routes:Some(routes),dependencies:&[]}}

#[cfg(test)]
mod typed_query_contract_tests {
    use super::*;
    #[test]
    fn repairreportquery_decodes_and_rejects_invalid() {
        fn decode(query: &str) -> bool {
            let uri: axum::http::Uri = format!("/?{query}").parse().unwrap();
            axum::extract::Query::<RepairReportQuery>::try_from_uri(&uri).is_ok()
        }
        assert!(decode("id=x"));
        assert!(!decode("id=&extra=x"));
        assert!(!decode("extra=x"));
    }
}
