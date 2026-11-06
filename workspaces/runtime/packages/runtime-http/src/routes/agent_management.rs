//! Thin HTTP lifecycle edge.
use super::{
    extractors::ApiJson,
    manifest::{OperationStatus, RouteGroup, post},
};
use crate::{error::ApiError, generated, router::HttpState};
use axum::{Extension, Json, extract::State, http::StatusCode};
use std::sync::Arc;
use studio_json::Value;
use studio_runtime_services::{
    agent_lifecycle_runtime::LifecyclePorts,
    agent_management::ManagementError,
    message_admission::MessageAdmission,
    native_action_ports,
    native_actions::{self, Admission, NativeError},
};

#[cfg(test)]
use studio_runtime_store::path_identity::CanonicalPath;
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![post(
        "/api/action",
        OperationStatus::Implemented,
        native_action,
    )])
}

pub(super) fn native_action_provider(
    agent: &AgentRecord,
    host_port: Option<
        Arc<dyn studio_runtime_services::account_session_runtime::AccountSessionPort>,
    >,
) -> Result<Arc<dyn studio_runtime_services::account_session_runtime::AccountSessionPort>, ApiError>
{
    if studio_runtime_services::workspace_ready::legacy_vm_record(agent) {
        return Err(ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "The workspace is unavailable",
        ));
    }
    host_port.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "Not implemented: requires provider-session/native-request",
        )
    })
}

async fn native_action(
    State(state): State<HttpState>,
    lifecycle: Option<Extension<Arc<dyn LifecyclePorts>>>,
    preparation: Option<
        Extension<Arc<studio_runtime_services::account_worker::preparation::PreparationResources>>,
    >,
    repair: Option<Extension<Arc<dyn NativeContextRepairPort>>>,
    ApiJson(body): ApiJson<generated::NativeActionRequest>,
) -> Result<Json<Value>, ApiError> {
    if let Some(root) = body.request_id.as_ref().and_then(Option::as_ref) {
        let root = root.as_str().to_owned();
        let agent_id = body.id.as_str().to_owned();
        let reserved = state
            .store
            .as_ref()
            .ok_or_else(ApiError::internal)?
            .read_async("runtime-http/native-action-purpose", move |connection| {
                let agent =
                    studio_runtime_services::event_write::checked_agent(connection, &agent_id)
                        .map_err(super::work_items::write_error)?;
                let episode =
                    agent
                        .extra
                        .get("idleCompactionEpisode")
                        .filter(|value| !value.is_null())
                        .cloned()
                        .map(
                            studio_json::from_value::<
                                studio_runtime_services::idle_compaction::Episode,
                            >,
                        )
                        .transpose()
                        .map_err(ApiError::json)?;
                Ok::<_, ApiError>(
                    episode.is_some_and(|episode| episode.id == root)
                        || native_actions::idle_provenance(connection, &root)
                            .map_err(native_error)?
                            .is_some(),
                )
            })
            .await?;
        if reserved {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "This request ID is reserved for idle compaction.",
            ));
        }
    }
    run_native_action(
        State(state),
        lifecycle,
        preparation,
        repair,
        ApiJson(body),
        None,
    )
    .await
}

async fn run_native_action(
    State(state): State<HttpState>,
    lifecycle: Option<Extension<Arc<dyn LifecyclePorts>>>,
    preparation: Option<
        Extension<Arc<studio_runtime_services::account_worker::preparation::PreparationResources>>,
    >,
    repair: Option<Extension<Arc<dyn NativeContextRepairPort>>>,
    ApiJson(body): ApiJson<generated::NativeActionRequest>,
    idle: Option<(
        studio_runtime_services::idle_compaction::Episode,
        studio_runtime_schema::usage_limits::UsageLimitsResponse,
    )>,
) -> Result<Json<Value>, ApiError> {
    let agent = body.id.as_str().to_owned();
    let action = match body.action {
        generated::NativeActionRequestAction::SafetyActionRequest(data) => {
            let port = state.account_session.clone().ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_IMPLEMENTED,
                    "Not implemented: requires provider-session/native-request",
                )
            })?;
            let store = state.store.clone().ok_or_else(ApiError::internal)?;
            let account_driver = state.account_driver.clone();
            let lifecycle = lifecycle.map(|Extension(ports)| ports);
            let resources = preparation
                .clone()
                .map(|Extension(resources)| resources)
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_IMPLEMENTED,
                        "Not implemented: requires native-action/preparation-and-settings",
                    )
                })?;
            let safety_action = data.safety.as_str();
            let turn_id = data.turn_id.as_str();
            let read_agent = agent.clone();
            let (account, epoch) = store
                .read_async("runtime-http/routes/agent_management", move |tx| {
                    let record = AgentRepository::get(tx, &AgentId::from(read_agent.as_str()))?
                        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Agent not found"))?;
                    let account = record.account_key.as_str().to_owned();
                    Ok::<_, ApiError>((account, record.epoch))
                })
                .await?;
            let actor = studio_runtime_services::account_worker::retained::RequestActor {
                agent: agent.clone(),
                epoch,
            };
            let account_driver = account_driver.ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_IMPLEMENTED,
                    "Not implemented: requires account-driver/provider-home",
                )
            })?;
            let accounts = account_driver
                .snapshot()
                .map_err(|_| ApiError::internal())?;
            let provider_home = accounts
                .accounts
                .iter()
                .find(|row| row.id.as_str() == Some(account.as_str()))
                .and_then(|row| row.home.as_str())
                .ok_or_else(ApiError::internal)?;
            let mut resources = resources.as_ref().clone();
            resources.account_home = provider_home.to_owned();
            let resources = Arc::new(resources);
            let result = studio_runtime_services::safety_buffering::execute_action(
                store,
                port.as_ref(),
                &resources,
                lifecycle,
                studio_runtime_services::safety_buffering::SafetyAction {
                    agent_id: &agent,
                    actor,
                    turn_id,
                    choice: safety_action,
                },
            )
            .await
            .map_err(safety_error)?;
            return Ok(Json(result));
        }
        generated::NativeActionRequestAction::NativeAction(action) => match action {
            generated::NativeAction::Compact => native_actions::Action::Compact,
            generated::NativeAction::Review => native_actions::Action::Review,
        },
    };
    let Some(request_id) = body.request_id.flatten() else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Reload Studio before Review or Compact. This client has no durable action request ID.",
        ));
    };
    let context = studio_json::to_value(body.context.flatten().unwrap_or_default())
        .map_err(|_| ApiError::internal())?;
    let request = native_actions::Request {
        request_id: request_id.as_str().to_owned(),
        agent,
        action,
        context,
    };
    // Invalid local state must be rejected before checking whether the provider
    // request consumer is connected, matching Python's read-only preconditions.
    let preflight_store = state.store.clone().ok_or_else(ApiError::internal)?;
    let preflight_request = request.clone();
    let preflight_result = preflight_store
        .read_async("runtime-http/routes/agent_management", move |tx| {
            if let Some(replayed) =
                native_actions::replay_if_settled(tx, &preflight_request).map_err(native_error)?
            {
                return Ok::<_, ApiError>(Some(replayed));
            }
            let agent = AgentRepository::get(tx, &AgentId::from(preflight_request.agent.as_str()))?
                .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Unknown managed agent"))?;
            if agent.thread_id.as_deref().is_none_or(str::is_empty) {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "This agent has no active conversation",
                ));
            }
            if agent.auto_wake != Some(true)
                || agent.in_flight == Some(true)
                || matches!(
                    agent.status,
                    studio_runtime_store::AgentStatus::Queued
                        | studio_runtime_store::AgentStatus::Starting
                        | studio_runtime_store::AgentStatus::Running
                        | studio_runtime_store::AgentStatus::Approval
                )
            {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "This agent is not ready for a native action",
                ));
            }
            Ok(None)
        })
        .await?;
    if let Some(replayed) = preflight_result {
        return Ok(Json(replayed));
    }
    let repair = repair.map(|Extension(port)| port);
    let resources = preparation
        .map(|Extension(resources)| resources)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Not implemented: requires native-action/preparation-and-settings",
            )
        })?;
    let host_port = state.account_session.clone();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let execute_store = Arc::clone(&store);
    let execute_request = request.clone();
    let account_driver = state.account_driver.clone();
    let admission_override = state.message_admission.clone();
    let read_agent = request.agent.clone();
    let use_prepared = admission_override.is_some();
    let (agent, prepared) = store
        .read_async("runtime-http/routes/agent_management", move |tx| {
            let agent = AgentRepository::get(tx, &AgentId::from(read_agent.as_str()))?
                .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Agent not found"))?;
            let prepared: studio_runtime_services::message_admission::AdmissionSnapshot =
                if use_prepared {
                    studio_runtime_services::message_admission::AdmissionSnapshot::Workspace(
                        studio_runtime_services::message_admission::PreparedAdmission::read(
                            tx,
                            &read_agent,
                        )
                        .map_err(|_| ApiError::internal())?,
                    )
                } else {
                    studio_runtime_services::message_admission::AdmissionSnapshot::Ingress(
                        studio_runtime_services::message_admission::MessageIngressAdmission::read(
                            tx,
                            &read_agent,
                        )
                        .map_err(|_| ApiError::internal())?,
                    )
                };
            Ok::<_, ApiError>((Box::new(agent), prepared))
        })
        .await?;
    let prepared = prepared.prepare().await.map_err(|_| ApiError::internal())?;
    let provider = native_action_provider(&agent, host_port)?;
    let admission: Arc<dyn MessageAdmission> =
        admission_override.unwrap_or_else(|| Arc::from(prepared));
    let bridge: Arc<dyn Admission> =
        Arc::new(native_action_ports::MessageNativeAdmission::new(admission));
    let bridge: Arc<dyn Admission> = match idle {
        Some((episode, limits)) => {
            Arc::new(studio_runtime_services::idle_compaction::IdleAdmission {
                inner: bridge,
                episode,
                limits,
            })
        }
        None => bridge,
    };
    let preparation_run = NativeActionPreparationRun {
        store: store.clone(),
        provider: provider.clone(),
        account_driver,
        repair,
        preparation: resources,
    };
    let now = unix_time();
    let result = native_actions::execute_provider_prepared(
        &execute_store,
        provider.as_ref(),
        &execute_request,
        now,
        bridge,
        Arc::new(native_action_ports::NativeActionAcceptance),
        &preparation_run,
    )
    .await
    .map_err(native_error)?;
    Ok(Json(result))
}

pub trait NativeContextRepairPort: Send + Sync {
    fn repair_idle(
        &self,
        store: Arc<RuntimeStore>,
        agent_id: String,
        provider_home: std::path::PathBuf,
        broker: Arc<dyn studio_runtime_services::account_session_runtime::AccountSessionPort>,
        deadline: std::time::Instant,
    ) -> futures_util::future::BoxFuture<'static, native_actions::Result<Value>>;
}

#[derive(Clone)]
struct NativeActionPreparationRun {
    store: Arc<RuntimeStore>,
    provider: Arc<dyn studio_runtime_services::account_session_runtime::AccountSessionPort>,
    account_driver: Option<Arc<studio_runtime_services::account_driver::AccountDriver>>,
    repair: Option<Arc<dyn NativeContextRepairPort>>,
    preparation: Arc<studio_runtime_services::account_worker::preparation::PreparationResources>,
}
impl NativeActionPreparationRun {
    async fn agent(
        &self,
        request: &native_actions::Request,
    ) -> native_actions::Result<AgentRecord> {
        let agent_id = AgentId::from(request.agent.as_str());
        self.store
            .read_async(
                "runtime-http/routes/agent_management/preparation",
                move |tx| AgentRepository::get(tx, &agent_id)?.ok_or(NativeError::Changed),
            )
            .await
    }

    /// The provider home of a host agent, or `None` for a VM agent.
    fn host_provider_home(
        &self,
        agent: &AgentRecord,
    ) -> native_actions::Result<Option<std::path::PathBuf>> {
        if studio_runtime_services::workspace_ready::legacy_vm_record(agent) {
            return Ok(None);
        }
        let account = agent.account_key.as_str();
        let account_driver = self
            .account_driver
            .as_ref()
            .ok_or(NativeError::Unavailable("account-driver/provider-home"))?;
        let accounts = account_driver
            .snapshot()
            .map_err(|_| NativeError::Unavailable("account-driver/provider-home"))?;
        accounts
            .accounts
            .iter()
            .find(|row| row.id.as_str() == Some(account))
            .and_then(|row| row.home.as_str())
            .map(|home| Some(std::path::PathBuf::from(home)))
            .ok_or(NativeError::Unavailable("account-driver/provider-home"))
    }

    fn provider_home(&self, agent: &AgentRecord) -> native_actions::Result<std::path::PathBuf> {
        self.host_provider_home(agent)?
            .ok_or(NativeError::Unavailable("workspace-state/unavailable"))
    }

    async fn repair_before_reserve(
        &self,
        request: &native_actions::Request,
    ) -> native_actions::Result<()> {
        let agent = self.agent(request).await?;
        let Some(provider_home) = self.host_provider_home(&agent)? else {
            return Ok(());
        };
        self.repair
            .as_ref()
            .ok_or(NativeError::Unavailable(
                "context-repair/repair-idle-broker",
            ))?
            .repair_idle(
                self.store.clone(),
                request.agent.clone(),
                provider_home,
                self.provider.clone(),
                std::time::Instant::now() + std::time::Duration::from_secs(20),
            )
            .await?;
        Ok(())
    }
}
impl native_actions::Preparation for NativeActionPreparationRun {
    fn before_reserve<'a>(
        &'a self,
        _store: &'a RuntimeStore,
        _provider: &'a dyn studio_runtime_services::account_session_runtime::AccountSessionPort,
        request: &'a native_actions::Request,
        _scope: &'a studio_runtime_services::account_session_runtime::ConnectionScope,
    ) -> studio_runtime_services::native_actions::PreparationFuture<'a> {
        Box::pin(async move { self.repair_before_reserve(request).await })
    }

    fn prepare<'a>(
        &'a self,
        _store: &'a RuntimeStore,
        _provider: &'a dyn studio_runtime_services::account_session_runtime::AccountSessionPort,
        request: &'a native_actions::Request,
        ticket: &'a studio_runtime_services::workspace_operations::ProviderTicket,
        scope: &'a studio_runtime_services::account_session_runtime::ConnectionScope,
    ) -> studio_runtime_services::native_actions::PreparationFuture<'a> {
        Box::pin(async move {
            let preparation = self.clone();
            let request_for_prepare = request.clone();
            let ticket_for_prepare = ticket.clone();
            let provider_home = preparation.provider_home(&preparation.agent(request).await?)?;
            let mut resources = preparation.preparation.as_ref().clone();
            resources.account_home = provider_home.to_string_lossy().into_owned();
            let prepared =
                studio_runtime_services::account_worker::preparation::prepare_native_action(
                    Arc::clone(&preparation.store),
                    &resources,
                    &request_for_prepare.agent,
                    &ticket_for_prepare.operation,
                    ticket_for_prepare.generation,
                )
                .await
                .map_err(|_| NativeError::Unavailable("thread-preparation/native-action"))?;
            if Some(prepared.thread_id.as_str()) != ticket.thread.as_deref() {
                return Err(NativeError::Changed);
            }
            let mut params = prepared.settings;
            if let Some(params) = params.as_object_mut() {
                params.remove("cwd");
                params.remove("cyberAccessProgram");
            }
            native_actions::update_action_settings(
                &self.store,
                self.provider.as_ref(),
                request,
                ticket,
                scope,
                params,
            )
            .await?;
            let idle_root = request.request_id.clone();
            let idle = self
                .store
                .read_async("runtime-http/native-action-purpose", move |connection| {
                    native_actions::idle_provenance(connection, &idle_root)
                })
                .await?;
            if idle.is_some() {
                let port =
                    self.preparation
                        .account_session
                        .as_ref()
                        .ok_or(NativeError::Unavailable(
                            "provider-session/account-rate-limits",
                        ))?;
                port.read_limits_for_resume(&scope.account, unix_time())
                    .await
                    .map_err(|_| {
                        NativeError::Unavailable("provider-session/account-rate-limits")
                    })?;
            }
            Ok(())
        })
    }
}

fn native_error(error: NativeError) -> ApiError {
    match error {
        NativeError::Expired => ApiError::new(StatusCode::GONE, error.to_string()),
        NativeError::Unavailable(_) => {
            ApiError::new(StatusCode::NOT_IMPLEMENTED, error.to_string())
        }
        NativeError::Conflict | NativeError::Invalid(_) | NativeError::Changed => {
            ApiError::new(StatusCode::BAD_REQUEST, error.to_string())
        }
        NativeError::Unknown => ApiError::new(StatusCode::CONFLICT, error.to_string()),
        NativeError::Store(error) => ApiError::stored(error),
        NativeError::Json(error) => ApiError::json(error),
        NativeError::Sql(error) => ApiError::stored(error.into()),
        NativeError::InvalidStored { table, id, .. } => ApiError::invalid_stored(table, &id),
    }
}

fn safety_error(
    error: studio_runtime_services::safety_buffering::SafetyBufferingError,
) -> ApiError {
    use studio_runtime_services::safety_buffering::SafetyBufferingError as SafetyError;
    match error {
        SafetyError::Expired => ApiError::new(StatusCode::GONE, error.to_string()),
        SafetyError::Requires(name) => ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            format!("Not implemented: requires {name}"),
        ),
        SafetyError::ProviderUnknown => ApiError::new(StatusCode::CONFLICT, error.to_string()),
        SafetyError::Persistence | SafetyError::InvalidStoredState => ApiError::internal(),
        SafetyError::StaleRetry
        | SafetyError::Inactive
        | SafetyError::InvalidChoice
        | SafetyError::NoRetryToCancel
        | SafetyError::StartAlreadySubmitted
        | SafetyError::ThreadNotOpen
        | SafetyError::MissingFasterModel
        | SafetyError::Preparation(_)
        | SafetyError::ProviderRejected => {
            ApiError::new(StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

fn unix_time() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}
pub(super) fn management_error(error: ManagementError) -> ApiError {
    match error {
        ManagementError::Expired => ApiError::new(StatusCode::GONE, error.to_string()),
        ManagementError::Unavailable(_) => {
            ApiError::new(StatusCode::NOT_IMPLEMENTED, error.to_string())
        }
        ManagementError::Invalid(_) => ApiError::new(StatusCode::BAD_REQUEST, error.to_string()),
        ManagementError::Stored { table, id } => ApiError::invalid_stored(&table, &id),
        ManagementError::Store(error) => ApiError::stored(error),
    }
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "agent_management", routes: Some(routes), dependencies: &[
            "account-worker/agent-interrupt", "native-safety/retry-driver",
            "agent-management/snapshot-limit", "cross-server/agent-management",
            "event-driver/child-stop-notification", "monitor-driver/agent-stop",
            "account-worker/deleted-settlement", "usage-policy/agent-stop", "voice-driver/agent-stop", "work-driver/deletion-release",
            "workspace-state/agent-lifecycle", "cross-server/native-review",
        ],
    }
}

/// The idle scheduler enters the same preparation, admission, and receipt path
/// as the Compact control. Its admission rechecks the episode under the writer.
pub struct IdleCompactionHost {
    pub state: HttpState,
    pub resources: Arc<studio_runtime_services::account_worker::preparation::PreparationResources>,
    pub repair: Arc<dyn NativeContextRepairPort>,
}
impl studio_runtime_services::idle_compaction::Host for IdleCompactionHost {
    fn compact<'a>(
        &'a self,
        agent: &'a str,
        episode: studio_runtime_services::idle_compaction::Episode,
        limits: studio_runtime_schema::usage_limits::UsageLimitsResponse,
    ) -> studio_runtime_services::idle_compaction::HostFuture<'a> {
        Box::pin(async move {
            let body = studio_json::from_value(studio_json::json!({
                "id":agent, "action":"compact", "request_id":episode.id, "context":episode.identity,
            }))
            .map_err(|error| error.to_string())?;
            let request_id = episode.id.clone();
            let result = run_native_action(
                State(self.state.clone()),
                None,
                Some(Extension(self.resources.clone())),
                Some(Extension(self.repair.clone())),
                ApiJson(body),
                Some((episode, limits)),
            )
            .await;
            if let Err(error) = &result
                && let Some(store) = &self.state.store
            {
                studio_runtime_services::idle_compaction::release_unsubmitted(
                    store,
                    agent.to_owned(),
                    request_id,
                    error.message().to_owned(),
                )
                .await
                .map_err(|error| error.to_string())?;
            }
            result
                .map(|_| ())
                .map_err(|error| error.message().to_owned())
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::disallowed_methods)]
mod native_review_preflight_tests {
    use super::*;
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use studio_json::json;
    use studio_runtime_services::{
        account_session_runtime::{
            AccountSessionError, AccountSessionPort, ConnectionScope, ProviderRequest,
            SessionReceipt,
        },
        account_worker::preparation::PreparationResources,
        model_catalog::ModelCatalogService,
    };
    struct ProviderSpy(AtomicUsize);
    impl AccountSessionPort for ProviderSpy {
        fn provider_request<'a>(
            &'a self,
            _request: ProviderRequest,
        ) -> futures_util::future::BoxFuture<'a, Result<SessionReceipt, AccountSessionError>>
        {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err(AccountSessionError::Requires("test/provider"))
            })
        }
    }

    struct RepairSpy(Arc<AtomicUsize>);
    impl NativeContextRepairPort for RepairSpy {
        fn repair_idle(
            &self,
            _store: Arc<RuntimeStore>,
            _agent_id: String,
            _provider_home: std::path::PathBuf,
            _broker: Arc<dyn AccountSessionPort>,
            _deadline: std::time::Instant,
        ) -> futures_util::future::BoxFuture<'static, native_actions::Result<Value>> {
            let calls = Arc::clone(&self.0);
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(Value::Null)
            })
        }
    }

    #[tokio::test]
    async fn native_action_http_wire_and_replay_equivalence() {
        use axum::{body::Body, http::Request, routing::post};
        use studio_runtime_schema::persistence::RequestIdentity;
        use studio_runtime_store::request_results::{Admission, RequestResults, ResultEnvelope};
        use tower::ServiceExt;

        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let agent = AgentRecord::from_value(json!({"compactions":0,
            "id":"agent", "rootId":"agent", "epoch":1, "status":"idle", "provider":"codex", "accountKey":"default",
            "idleCompactionEpisode": {"id":"reserved", "cache":{"requestId":"cache", "ttl":"1h", "warm":true,
                "lastActivityAt":1.0, "contextTokens":1, "prefix":"prefix"}, "fireAt":2.0, "expiresAt":3.0,
                "identity":{}, "prefixInputs":[], "attempt":{}}
        })).unwrap();
        let root = RequestIdentity::issue_root();
        let identity = RequestIdentity::new("native-action", &[], &root, &[]).unwrap();
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &agent)?;
                let signature = studio_runtime_store::python_json_with_separators(
                    &json!({"agent":"agent","action":"compact","context":{}}),
                    ",",
                    ":",
                );
                let Admission::Fresh(admitted) =
                    RequestResults::admit(tx, &identity, &signature, (unix_time() * 1000.0) as i64)
                        .unwrap()
                else {
                    panic!("fresh")
                };
                RequestResults::settle(
                    tx,
                    &admitted,
                    &ResultEnvelope {
                        actor: Some("agent".into()),
                        reply: json!({"result":"saved"}),
                        references: vec![],
                    },
                )
                .unwrap();
                Ok(())
            })
            .await
            .unwrap();
        let state = HttpState::new(
            "token".into(),
            "fixture".into(),
            directory.path().display().to_string(),
            49172,
            None,
            Some(store.clone()),
            None,
        );
        let app = axum::Router::new()
            .route("/api/action", post(native_action))
            .with_state(state);
        async fn send(app: &axum::Router, body: Value) -> (StatusCode, String) {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/api/action")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, String::from_utf8(bytes.to_vec()).unwrap())
        }
        let invalid = (
            StatusCode::BAD_REQUEST,
            "{\"error\":\"Invalid request\",\"location\":[\"body\",\"action\"]}".to_owned(),
        );
        for action in [
            json!("other"),
            json!({"safety":"other","turnId":"t"}),
            json!({"safety":"wait"}),
            json!({"safety":"wait","turnId":null}),
            json!({"safety":"wait","turnId":1}),
            json!({"safety":"wait","turnId":""}),
            json!({"safety":"wait","turnId":"é".repeat(201)}),
        ] {
            assert_eq!(
                send(&app, json!({"id":"agent","action":action})).await,
                invalid
            );
        }
        for n in [1, 200] {
            assert_eq!(
                send(
                    &app,
                    json!({"id":"agent","action":{"safety":"wait","turnId":"é".repeat(n)}})
                )
                .await,
                (
                    StatusCode::NOT_IMPLEMENTED,
                    "{\"error\":\"Not implemented: requires provider-session/native-request\"}"
                        .into()
                )
            );
        }
        for body in [
            json!({"id":"agent","action":"compact"}),
            json!({"id":"agent","action":"compact","request_id":null}),
        ] {
            assert_eq!(send(&app,body).await, (StatusCode::BAD_REQUEST,"{\"error\":\"Reload Studio before Review or Compact. This client has no durable action request ID.\"}".into()));
        }
        assert_eq!(
            send(
                &app,
                json!({"id":"agent","action":"compact","request_id":"reserved"})
            )
            .await,
            (
                StatusCode::BAD_REQUEST,
                "{\"error\":\"This request ID is reserved for idle compaction.\"}".into()
            )
        );
        let body = json!({"id":"agent","action":"compact","request_id":root});
        let expected = (StatusCode::OK, "{\"result\":\"saved\"}".into());
        assert_eq!(send(&app, body.clone()).await, expected);
        assert_eq!(send(&app, body).await, expected);
        let read = store.test_reader().unwrap();
        let count: i64 = read
            .query_row("SELECT COUNT(*) FROM request_results", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn native_action_rejects_reserved_idle_request_and_missing_id() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let agent = AgentRecord::from_value(json!({"compactions":0,
            "id":"agent", "rootId":"agent", "epoch":1, "status":"idle", "provider":"codex", "accountKey":"default",
            "idleCompactionEpisode": {"id":"reserved", "cache":{"requestId":"cache", "ttl":"1h", "warm":true,
                "lastActivityAt":1.0, "contextTokens":1, "prefix":"prefix"}, "fireAt":2.0, "expiresAt":3.0,
                "identity":{}, "prefixInputs":[], "attempt":{}}
        })).unwrap();
        store
            .transaction(move |tx| AgentRepository::put(tx, &agent))
            .await
            .unwrap();
        let state = HttpState::new(
            "token".into(),
            "fixture".into(),
            directory.path().display().to_string(),
            49172,
            None,
            Some(store),
            None,
        );
        for (request_id, expected) in [
            (Some("reserved".to_owned()), "reserved for idle compaction"),
            (None, "no durable action request ID"),
        ] {
            let body = generated::NativeActionRequest {
                action: generated::NativeActionRequestAction::NativeAction(
                    generated::NativeAction::Compact,
                ),
                id: "agent".to_owned().try_into().unwrap(),
                context: None,
                request_id: Some(request_id.map(|id| id.try_into().unwrap())),
            };
            let error = Box::pin(native_action(
                State(state.clone()),
                None,
                None,
                None,
                ApiJson(body),
            ))
            .await
            .unwrap_err();
            assert!(error.message().contains(expected), "{}", error.message());
        }
        for action in ["compact", "review"] {
            for request_id in [None, Some(Value::Null)] {
                let mut body = json!({"id":"agent", "action":action, "context":null});
                if let Some(request_id) = request_id {
                    body["request_id"] = request_id;
                }
                let body = studio_json::from_value(body).unwrap();
                let error = Box::pin(native_action(
                    State(state.clone()),
                    None,
                    None,
                    None,
                    ApiJson(body),
                ))
                .await
                .unwrap_err();
                assert!(error.message().contains("no durable action request ID"));
            }
        }
        for safety in [
            generated::SafetyAction::Wait,
            generated::SafetyAction::Retry,
            generated::SafetyAction::Cancel,
        ] {
            let body = generated::NativeActionRequest::new(
                generated::NativeActionRequestAction::SafetyActionRequest(
                    generated::SafetyActionRequest {
                        safety,
                        turn_id: "turn".to_owned().try_into().unwrap(),
                    },
                ),
                "agent".to_owned().try_into().unwrap(),
            );
            let error = Box::pin(native_action(
                State(state.clone()),
                None,
                None,
                None,
                ApiJson(body),
            ))
            .await
            .unwrap_err();
            assert!(error.message().contains("provider-session/native-request"));
        }
    }

    #[tokio::test]
    async fn remote_origin_review_preflight_uses_the_provider_home_owner() {
        let directory = tempfile::tempdir().expect("isolated store directory");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("open store"),
        );
        let original = AgentRecord::from_value(json!({"compactions":0,
            "id":"remote",
            "rootId":"remote",
            "epoch":7,
            "status":"running",
            "provider":"codex",
            "remoteOrigin":true,
            "threadId":"thread-1",
            "accountKey":"default",
            "startAttempt":{"submitted":true}
        }))
        .expect("valid agent record");
        let original_for_write = original.clone();
        store
            .transaction(move |tx| AgentRepository::put(tx, &original_for_write))
            .await
            .expect("save agent");
        let before_agent = {
            let connection = store.test_reader().expect("read store");
            AgentRepository::get(&connection, &original.id)
                .expect("read agent")
                .expect("agent exists")
                .value()
        };
        let before_receipts: i64 = {
            let connection = store.test_reader().expect("read store");
            connection
                .query_row(
                    "SELECT COUNT(*) FROM runtime_outbox WHERE kind='native_action'",
                    [],
                    |row| row.get(0),
                )
                .expect("count receipts")
        };

        let root = CanonicalPath::existing(
            Path::new(
                &std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
            )
            .join("../../../.."),
        )
        .expect("repository root");
        let resources = PreparationResources::bundled_at(
            String::new(),
            ModelCatalogService::new(),
            root.clone().to_path_buf(),
        )
        .expect("preparation resources");
        let provider = Arc::new(ProviderSpy(AtomicUsize::new(0)));
        let repair = Arc::new(RepairSpy(Arc::new(AtomicUsize::new(0))));
        let preparation = NativeActionPreparationRun {
            store: store.clone(),
            provider: provider.clone(),
            account_driver: None,
            repair: Some(repair.clone()),
            preparation: Arc::new(resources),
        };
        let request = native_actions::Request {
            request_id: "review-request".into(),
            agent: "remote".into(),
            action: native_actions::Action::Review,
            context: Value::Null,
        };
        let scope = ConnectionScope {
            account: "default".into(),
            connection: "connection-1".into(),
            generation: 3,
        };
        let result = native_actions::Preparation::before_reserve(
            &preparation,
            store.as_ref(),
            provider.as_ref(),
            &request,
            &scope,
        )
        .await;

        assert!(matches!(
            result,
            Err(NativeError::Unavailable("account-driver/provider-home"))
        ));
        assert_eq!(repair.0.load(Ordering::SeqCst), 0);
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);
        let connection = store.test_reader().expect("read unchanged store");
        let after_agent = AgentRepository::get(&connection, &original.id)
            .expect("read unchanged agent")
            .expect("agent remains")
            .value();
        let after_receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runtime_outbox WHERE kind='native_action'",
                [],
                |row| row.get(0),
            )
            .expect("count unchanged receipts");
        assert_eq!(before_agent["remoteOrigin"], true);
        assert_eq!(after_agent, original.value());
        assert_eq!(after_receipts, before_receipts);
    }

    #[test]
    fn legacy_vm_native_action_is_refused() {
        let agent = AgentRecord::from_value(json!({"id":"vm", "rootId":"vm", "epoch":1, "status":"idle", "workspaceMode":"layr", "accountKey":"default"})).unwrap();
        let error =
            native_action_provider(&agent, Some(Arc::new(ProviderSpy(AtomicUsize::new(0)))))
                .err()
                .expect("legacy VM refused");
        assert_eq!(error.status(), StatusCode::NOT_IMPLEMENTED);
        assert_eq!(error.message(), "The workspace is unavailable");
    }
}
