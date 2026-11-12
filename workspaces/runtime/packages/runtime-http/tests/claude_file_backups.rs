//! HTTP, retained account adapter and durable control settlement in one caller test.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]
use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::Request,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use studio_json::{Value, json};
use studio_runtime_http::{HttpState, router};
use studio_runtime_process::{
    EventCursor, Generation, OperationId, ProviderHandle, ProviderLaunchSpec, provider_seam::*,
};
use studio_runtime_services::{
    account_policy::AccountPolicyState,
    account_session_runtime::*,
    account_worker::{
        AccountHandle, StoreBridge, preparation::PreparationResources, start_account_worker,
    },
    claude_controls::service::ControlSession,
    provider_session_task::SessionTaskConfig,
};
use studio_runtime_store::{AgentId, AgentRepository, RuntimeStore};
use tokio::sync::{Notify, mpsc, watch};
use tower::ServiceExt;

#[derive(Default)]
pub(crate) struct Native {
    pub(crate) calls: Mutex<Vec<Value>>,
    pub(crate) repair_report: Mutex<Option<Value>>,
    ready: Notify,
    lost_reply: AtomicBool,
    reconnected: AtomicBool,
    not_sent: AtomicBool,
    unsupported: AtomicBool,
}
impl Native {
    fn response(&self, request: &Value) -> Value {
        self.calls.lock().unwrap().push(request.clone());
        let p = &request["params"];
        studio_runtime_services::internal_sender_witness::request(
            studio_runtime_providers::Method::from_wire(request["method"].as_str().unwrap())
                .unwrap(),
            p,
        );
        if request["method"] == "turn/settings/update" && self.unsupported.load(Ordering::SeqCst) {
            return json!({"id":request["id"],"error":{"code":-32601,"message":"unsupported settings"}});
        }
        let result = match request["method"].as_str().unwrap() {
            "claude/repairReport" => {
                self.repair_report.lock().unwrap().clone().unwrap_or_else(|| json!({"status":"needs_repair","skippedRows":3,"invalidSequence":true,"scopeConfirmed":true,"nativeHistoryConfirmed":false,"secret":"must-not-escape","transcript":"private transcript content","path":"/private/account"}))
            }
            "claude/state" => {
                json!({"tasks":[],"turns":[{"id":"one","status":"completed","canRestoreFiles":true},{"id":"two","status":"completed","canRestoreFiles":true}]})
            }
            "claude/settings" => json!({"settings":p["settings"]}),
            "turn/settings/update" => json!({"status":"applied"}),
            "claude/stopTask" => json!({"stopped":true}),
            "claude/fileBackups" => {
                let release = p["settledRequestIds"]
                    .as_array()
                    .is_some_and(|ids| !ids.is_empty());
                json!({"count":1,"bytes":500,"maxCount":100,"maxBytes":1073741824,"released":if release {1} else {0}})
            }
            method => panic!("unexpected provider method {method}"),
        };
        json!({"id":request["id"],"result":result})
    }
}
struct Factory(Arc<Native>);
impl ProviderTaskFactory for Factory {
    fn open(&self, handle: ProviderHandle, options: OpenOptions, stop: Shutdown) -> OpenFuture<'_> {
        let native = self.0.clone();
        Box::pin(async move {
            let generation = options.expected_generation.unwrap();
            let (commands, mut inbox) = mpsc::channel::<AdapterCommand>(16);
            let (entries, journal) = mpsc::channel(16);
            let (status, observed) = watch::channel(AttachmentStatus {
                handle: handle.clone(),
                open_attempted: true,
                info: Some(AttachmentInfo {
                    spawn_started_at: None,
                    initialized_at: None,
                    handle,
                    generation,
                    pid: 1,
                    acknowledged: EventCursor(0),
                    resumed: false,
                }),
                initialization: InitializationStatus::AwaitingReply,
            });
            let mut stopped = stop.subscribe();
            let task = tokio::spawn(async move {
                let _status = status;
                let mut cursor = 0;
                loop {
                    let command = tokio::select! {biased; _=Shutdown::cancelled(&mut stopped)=>break, command=inbox.recv()=>match command {Some(c)=>c,None=>break}};
                    match command {
                        AdapterCommand::Write {
                            write,
                            reply,
                            observed,
                        } => {
                            let method = write.frame().method().unwrap_or("");
                            if method == "turn/settings/update"
                                && native.not_sent.swap(false, Ordering::SeqCst)
                            {
                                drop(reply.send(CommandReceipt::NotSent));
                                continue;
                            }
                            if method == "initialize" {
                                drop(reply.send(CommandReceipt::CachedInitialize(json!({}))));
                            } else {
                                drop(reply.send(CommandReceipt::Written {
                                    written_at: None,
                                    remote_id: None,
                                    duplicate: false,
                                }));
                                if method == "initialized" {
                                    native.ready.notify_one();
                                } else {
                                    let response = native.response(&write.message());
                                    cursor += 1;
                                    entries
                                        .send(JournalEntry {
                                    boundary_at: None,
                                            received_at_ms: options.clock.now_ms(),
                                            cursor: EventCursor(cursor),
                                            generation,
                                            payload: JournalPayload::Frame(
                                                match studio_runtime_providers::decode_journal_entry_borrowed(1, 1, format!("{response}\n").as_bytes()).payload { studio_runtime_providers::DecodedJournalPayload::Frame(frame) => frame, other @ studio_runtime_providers::DecodedJournalPayload::RawStdout { .. } => panic!("fixture must decode: {other:?}"), },
                                            ),
                                        })
                                        .await
                                        .unwrap();
                                }
                            }
                            let _observed = observed.await;
                        }
                        AdapterCommand::Ack { cursor, reply } => {
                            drop(reply.send(CommandReceipt::Acknowledged(cursor)));
                        }
                    }
                }
                TaskExit {
                    result: Ok(()),
                    cleanup_error: None,
                    recovery_needed: None,
                    provider_retained: false,
                }
            });
            Ok(ProviderAttachment::from_task(
                commands, journal, observed, stop, task,
            ))
        })
    }
}
struct Broker(Arc<dyn ControlSession>, AtomicBool);
impl AccountSessionPort for Broker {
    fn ensure_session<'a>(&'a self, account: &'a str) -> AccountSessionFuture<'a, ConnectionScope> {
        Box::pin(async move {
            self.1.store(true, Ordering::SeqCst);
            Ok(ConnectionScope {
                account: account.into(),
                connection: "fixture".into(),
                generation: 1,
            })
        })
    }
    fn claude_control_session(
        &self,
        _: &str,
    ) -> Result<Arc<dyn ControlSession>, ClaudeControlSessionError> {
        if !self.1.load(Ordering::SeqCst) {
            return Err(ClaudeControlSessionError::Unavailable);
        }
        Ok(self.0.clone())
    }
}
async fn post(app: &axum::Router, path: &str, body: Value) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("host", "127.0.0.1:49171")
                .header("content-type", "application/json")
                .header("x-canvas-token", "fixture-token")
                .header(
                    studio_runtime_http::API_SCHEMA_HASH_HEADER,
                    studio_runtime_http::schema::api_schema_hash(),
                )
                .extension(ConnectInfo(
                    "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    (
        status,
        studio_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap(),
    )
}
pub(crate) async fn fixture() -> (
    tempfile::TempDir,
    Arc<RuntimeStore>,
    axum::Router,
    AccountHandle,
    Arc<Native>,
) {
    fixture_with_live_session(Some(true)).await
}
async fn fixture_with_live_session(
    live: Option<bool>,
) -> (
    tempfile::TempDir,
    Arc<RuntimeStore>,
    axum::Router,
    AccountHandle,
    Arc<Native>,
) {
    fixture_with_provider(live, studio_runtime_providers::ProviderKind::Claude).await
}
async fn fixture_with_provider(
    live: Option<bool>,
    provider: studio_runtime_providers::ProviderKind,
) -> (
    tempfile::TempDir,
    Arc<RuntimeStore>,
    axum::Router,
    AccountHandle,
    Arc<Native>,
) {
    let root = tempfile::tempdir().unwrap();
    let cwd = root.path().join("cwd");
    std::fs::create_dir(&cwd).unwrap();
    let state_dir = root.path().join("state");
    std::fs::create_dir(&state_dir).unwrap();
    let store = Arc::new(RuntimeStore::open(Some(&state_dir)).await.unwrap());
    let agent = json!({"compactions":0,"id":"agent","rootId":"agent","epoch":2,"cwd":cwd,"provider":provider,"accountKey":"account","threadId":"thread","turnId":"one","model":"old-model","status":"paused","inFlight":false,"autoWake":false});
    store
        .transaction(move |tx| {
            studio_runtime_store::AgentRepository::put(
                tx,
                &studio_runtime_store::AgentRecord::from_value(agent)?,
            )?;
            Ok(())
        })
        .await
        .unwrap();
    store
        .transaction(|tx| {
            studio_runtime_store::OrchestrationSettingsRepository::set(
                tx,
                studio_runtime_store::IDLE_SESSION_UNLOAD_SETTING,
                0,
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let native = Arc::new(Native::default());
    let clock = SessionClock::default();
    let mut resources = PreparationResources::bundled(
        String::new(),
        studio_runtime_services::model_catalog::ModelCatalogService::new(),
    )
    .unwrap();
    resources.claude_account_pin = json!({"provider":provider});
    let worker = start_account_worker(
        StoreBridge::new(store.clone()).await.unwrap(),
        "account".into(),
        Arc::new(Factory(native.clone())),
        OpenOptions {
            clock,
            launch: ProviderLaunchSpec::new(vec!["fixture".into()], BTreeMap::new(), cwd),
            expected_generation: Some(Generation(1)),
        },
        SessionTaskConfig {
            initialize_params: json!({}),
            generation: Generation(1),
            wire: provider.wire(),
            timeout_ms: 20000,
            initialize_operation: OperationId("init".into()),
            request_id_prefix: None,
            recovered: vec![],
        },
        resources,
    )
    .await
    .unwrap();
    native.ready.notified().await;
    if provider == studio_runtime_providers::ProviderKind::Codex {
        store
            .transaction(|tx| {
                let mut record = AgentRepository::get(tx, &AgentId::from("agent"))?.unwrap();
                record.status = studio_runtime_store::AgentStatus::Running;
                record.in_flight = Some(true);
                record.turn_id = Some("one".into());
                AgentRepository::put(tx, &record)
            })
            .await
            .unwrap();
    }
    let policy=AccountPolicyState::from_startup(Some(studio_json::json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"account","accounts":{"account":{"id":"account","provider":provider,"home":"/fixture","label":"Fixture","source":"Claude Code","status":"ready"}},"logins":{}}).to_string().as_bytes()),[]).unwrap();
    let mut state = HttpState::new(
        "fixture-token".into(),
        "fixture".into(),
        root.path().display().to_string(),
        49171,
        None,
        Some(store.clone()),
        Some(Arc::new(policy)),
    );
    state.account_session = live.map(|live| {
        if provider == studio_runtime_providers::ProviderKind::Claude {
            Arc::new(Broker(
                Arc::new(worker.claude_session().unwrap()),
                AtomicBool::new(live),
            )) as Arc<dyn AccountSessionPort>
        } else {
            Arc::new(CodexBroker(worker.retained_session(), native.clone()))
                as Arc<dyn AccountSessionPort>
        }
    });
    (root, store, router(state).unwrap(), worker, native)
}
fn agent(store: &RuntimeStore) -> Value {
    AgentRepository::get(&store.test_reader().unwrap(), &AgentId("agent".into()))
        .unwrap()
        .unwrap()
        .value()
}

#[tokio::test]
async fn claude_control_session_reads_and_stop_keep_dispatch() {
    let (_root, _store, app, mut worker, native) = fixture().await;
    let stop_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let missing = post(
        &app,
        "/api/claude/session",
        json!({"id":"agent","action":"stop_task","task_id":"task"}),
    )
    .await;
    assert_eq!(missing.0, 400, "{missing:?}");
    for (action, expected) in [("state", 200), ("commands", 409), ("stop_task", 200)] {
        let body = if action == "stop_task" {
            json!({"id":"agent","action":action,"task_id":"task","request_id":stop_id})
        } else {
            json!({"id":"agent","action":action})
        };
        let result = post(&app, "/api/claude/session", body).await;
        assert_eq!(result.0, expected, "{action}: {result:?}");
    }
    let replay = post(
        &app,
        "/api/claude/session",
        json!({"id":"agent","action":"stop_task","task_id":"task","request_id":stop_id}),
    )
    .await;
    assert_eq!(replay.0, 200, "{replay:?}");
    let methods: Vec<Value> = native
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|call| call["method"].clone())
        .collect();
    assert_eq!(
        methods,
        json!(["claude/state", "claude/stopTask"])
            .as_array()
            .unwrap()
            .clone()
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn claude_command_preserves_message_request_identity() {
    let (_root, _store, app, mut worker, _native) = fixture_with_live_session(Some(false)).await;
    let request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let body = json!({"id":"agent","action":"command","command":"/help","request_id":request_id});
    let first = post(&app, "/api/claude/session", body.clone()).await;
    assert_eq!(first.0, 200, "{first:?}");
    let replay = post(&app, "/api/claude/session", body).await;
    assert_eq!(replay, first);
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn claude_control_settings_ingress_and_exact_receipt_replay() {
    let (_root, store, app, mut worker, native) = fixture_with_live_session(Some(false)).await;
    for body in [
        json!({"id":"agent","action":"settings","settings":{}}),
        json!({"id":"agent","action":"settings","request_id":null,"settings":{}}),
        json!({"id":"agent","action":"settings","request_id":"","settings":{}}),
        json!({"id":"agent","action":"settings","request_id":"invalid","settings":{"thinking":null}}),
        json!({"id":"agent","action":"settings","request_id":"invalid","settings":{"permissionMode":null}}),
        json!({"id":"agent","action":"settings","request_id":"invalid","settings":{},"turn_id":"two"}),
        // The rollback action no longer exists.
        json!({"id":"agent","action":"rollback","request_id":"invalid","turn_id":"two"}),
    ] {
        let result = post(&app, "/api/claude/session", body).await;
        assert_eq!(result.0, 400, "{result:?}");
    }
    assert!(native.calls.lock().unwrap().is_empty());
    assert!(agent(&store)["workspaceReservationId"].is_null());

    let body = json!({"id":"agent","action":"settings","scope":{"scope":"session"},"request_id":studio_runtime_schema::persistence::RequestIdentity::issue_root(),"settings":{"permissionMode":"plan","thinking":false,"autoCompactWindow":null}});
    let result = post(&app, "/api/claude/session", body.clone()).await;
    assert_eq!(result.0, 200, "{result:?}");
    assert_eq!(
        agent(&store)["claudeOptions"],
        json!({"thinking":false,"autoCompactWindow":null})
    );
    assert_eq!(agent(&store)["permissionMode"], "plan");
    let calls = native.calls.lock().unwrap().clone();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "claude/settings")
            .count(),
        1
    );
    let replay = post(&app, "/api/claude/session", body.clone()).await;
    assert_eq!(replay, result);
    assert_eq!(*native.calls.lock().unwrap(), calls);

    let mut persistent = body.clone();
    persistent["scope"] = json!({"scope":"persistent"});
    assert_eq!(post(&app, "/api/claude/session", persistent).await.0, 400);
    let mut conflict = body;
    conflict["settings"]
        .as_object_mut()
        .unwrap()
        .remove("autoCompactWindow");
    let conflict = post(&app, "/api/claude/session", conflict).await;
    assert_eq!(conflict.0, 409, "{conflict:?}");
    assert_eq!(*native.calls.lock().unwrap(), calls);
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn claude_file_backups_usage_is_read_through_http() {
    let (_root, _store, app, mut worker, _native) = fixture_with_live_session(Some(false)).await;
    let usage = post(
        &app,
        "/api/claude/file-backups",
        json!({"id":"agent","release":false,"request_id":studio_runtime_schema::persistence::RequestIdentity::issue_root()}),
    )
    .await;
    assert_eq!(usage.0, 200, "{usage:?}");
    assert_eq!(usage.1["count"], 1);
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn claude_controls_start_missing_account_session() {
    let (_root, _store, app, mut worker, native) = fixture_with_live_session(Some(false)).await;
    let result = post(
        &app,
        "/api/claude/session",
        json!({"id":"agent","action":"state"}),
    )
    .await;
    assert_eq!(result.0, 200, "{result:?}");
    assert_eq!(native.calls.lock().unwrap()[0]["method"], "claude/state");
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn claude_controls_report_unavailable_and_keep_local_paths() {
    let (_root, store, app, mut worker, native) = fixture_with_live_session(None).await;
    let result = post(
        &app,
        "/api/claude/session",
        json!({"id":"agent","action":"state"}),
    )
    .await;
    assert_eq!(result.0, 503, "{result:?}");
    assert_eq!(
        result.1["error"],
        "The Claude account session is not running"
    );
    let mut record = agent(&store);
    record["threadId"] = Value::Null;
    store
        .transaction(move |tx| {
            AgentRepository::put(tx, &studio_runtime_store::AgentRecord::from_value(record)?)
        })
        .await
        .unwrap();
    let state = post(
        &app,
        "/api/claude/session",
        json!({"id":"agent","action":"state"}),
    )
    .await;
    assert_eq!(state.0, 200, "{state:?}");
    let settings = post(&app, "/api/claude/session", json!({"id":"agent","action":"settings","request_id":studio_runtime_schema::persistence::RequestIdentity::issue_root(),"settings":{"thinking":false}})).await;
    assert_eq!(settings.0, 200, "{settings:?}");
    assert!(native.calls.lock().unwrap().is_empty());
    worker.shutdown().await.unwrap();
}

struct CodexBroker(
    studio_runtime_services::account_worker::retained::RetainedSessionPort,
    Arc<Native>,
);
impl AccountSessionPort for CodexBroker {
    fn current_scope(&self, _: &str) -> Option<ConnectionScope> {
        let mut scope = self.0.scope().clone();
        if self.1.reconnected.load(Ordering::SeqCst) {
            scope.generation += 1;
        }
        Some(scope)
    }
    fn provider_request<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> AccountSessionFuture<'a, SessionReceipt> {
        Box::pin(async move {
            let receipt = self
                .0
                .request(
                    studio_runtime_services::account_worker::retained::SessionRequest {
                        root_request_id: request.root_request_id,
                        admitted_request: request.admitted_request,
                        scope: self.0.scope().clone(),
                        expected_scope: Some(self.0.scope().clone()),
                        operation: request.operation,
                        actor: request.actor,
                        transfer: None,
                        deleted_actor_session: None,
                        method: request.method,
                        params: request.params,
                        deadline: request.deadline,
                    },
                )
                .await
                .map_err(|error| {
                    if matches!(
                        error,
                        studio_runtime_services::account_worker::retained::PortError::Identity
                            | studio_runtime_services::account_worker::retained::PortError::Invalid
                    ) {
                        AccountSessionError::Refused("settings refused")
                    } else {
                        AccountSessionError::Unknown
                    }
                })?;
            if self.1.lost_reply.swap(false, Ordering::SeqCst) {
                return Err(AccountSessionError::Unknown);
            }
            Ok(receipt.into())
        })
    }
}
#[tokio::test]
async fn internal_sender_witness_current_turn_settings_http_worker_codec() {
    let (_root, store, app, mut worker, native) =
        fixture_with_provider(Some(true), studio_runtime_providers::ProviderKind::Codex).await;
    let body = current_settings_body();
    let result = post(&app, "/api/conversation", body.clone()).await;
    assert_eq!(result.0, 200, "{result:?}");
    assert_eq!(post(&app, "/api/conversation", body.clone()).await.0, 200);
    let calls = native.calls.lock().unwrap().clone();
    let settings: Vec<_> = calls
        .iter()
        .filter(|c| c["method"] == "turn/settings/update")
        .collect();
    assert_eq!(settings.len(), 1);
    assert_eq!(
        settings[0]["params"],
        json!({"threadId":"thread","turnId":"one","model":"new-model","effort":"high","serviceTier":"priority"})
    );
    assert_eq!(
        agent(&store)["model"],
        "old-model",
        "turn override must not overwrite session defaults"
    );
    let mut stale = body.clone();
    stale["settings_scope"]["turn_id"] = json!("different-turn");
    stale["request_id"] = json!(studio_runtime_schema::persistence::RequestIdentity::issue_root());
    assert_eq!(post(&app, "/api/conversation", stale).await.0, 400);
    let mut persistent = body;
    persistent["settings_scope"] = json!({"scope":"persistent"});
    assert_eq!(post(&app, "/api/conversation", persistent).await.0, 400);
    worker.shutdown().await.unwrap();
}

fn current_settings_body() -> Value {
    json!({"id":"agent", "expected_account_key":"account", "request_id":studio_runtime_schema::persistence::RequestIdentity::issue_root(), "settings_scope":{"scope":"current_turn","turn_id":"one"}, "model":"new-model", "effort":"high", "fast_mode":true})
}

#[tokio::test]
async fn live_settings_lost_receipt_replays_after_turn_or_generation_changes() {
    for ended in [true, false] {
        let (_root, store, app, mut worker, native) =
            fixture_with_provider(Some(true), studio_runtime_providers::ProviderKind::Codex).await;
        let body = current_settings_body();
        native.lost_reply.store(true, Ordering::SeqCst);
        assert_eq!(post(&app, "/api/conversation", body.clone()).await.0, 502);
        if ended {
            store
                .transaction(|tx| {
                    let mut record = AgentRepository::get(tx, &AgentId::from("agent"))?.unwrap();
                    record.in_flight = Some(false);
                    record.turn_id = None;
                    record.status = studio_runtime_store::AgentStatus::Idle;
                    AgentRepository::put(tx, &record)
                })
                .await
                .unwrap();
        } else {
            // Simulates current generation two with the retained checkpoint-verified generation-one receipt.
            native.reconnected.store(true, Ordering::SeqCst);
        }
        let result = post(&app, "/api/conversation", body).await;
        assert_eq!(result.0, 200, "ended={ended}: {result:?}");
        assert_eq!(native.calls.lock().unwrap().len(), 1);
        worker.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn live_settings_unsupported_and_not_sent_are_terminal_http_refusals() {
    let (_root, store, app, mut worker, native) =
        fixture_with_provider(Some(true), studio_runtime_providers::ProviderKind::Codex).await;
    native.not_sent.store(true, Ordering::SeqCst);
    let result = post(&app, "/api/conversation", current_settings_body()).await;
    assert_eq!(result.0, 400, "{result:?}");
    assert!(native.calls.lock().unwrap().is_empty());
    native.unsupported.store(true, Ordering::SeqCst);
    assert_eq!(
        post(&app, "/api/conversation", current_settings_body())
            .await
            .0,
        400
    );
    native.unsupported.store(false, Ordering::SeqCst);
    assert_eq!(
        post(&app, "/api/conversation", current_settings_body())
            .await
            .0,
        200
    );
    assert_eq!(agent(&store)["status"], "running");
    worker.shutdown().await.unwrap();
}
