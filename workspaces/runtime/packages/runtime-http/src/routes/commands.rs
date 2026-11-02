//! HTTP edge of the one command path: `POST /api/commands`.
//! Tracking: https://github.com/ifdotpy/layr-studio/issues/630
//!
//! The edge only reads the principal and calls `studio_runtime_services::commands`.
//! `withdraw` and `stop` run in one transaction. `send` runs `begin` (permission, id,
//! replay) before its I/O, and then posts its message and runs `finish` (reference and
//! watermark) in one transaction, unless the message is federated.

use base64::Engine;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::{Extension, Json, extract::State};
use studio_runtime_schema::command::{Command, CommandRequest, CommandResponse, CommandRow};
use studio_runtime_services::agent_lifecycle_runtime::LifecyclePorts;
use studio_runtime_services::commands::{self, CommandError, Start};
use studio_runtime_services::principal::Principal;

use super::extractors::{ApiJson, Caller};
use super::manifest::{OperationStatus, RouteGroup, post};
use crate::error::ApiError;
use crate::router::HttpState;

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![post(
        "/api/commands",
        OperationStatus::Implemented,
        command,
    )])
}

pub(crate) fn command_error(error: CommandError) -> ApiError {
    match error {
        CommandError::Forbidden => ApiError::new(
            StatusCode::FORBIDDEN,
            "This caller may not run this command",
        ),
        CommandError::TooOld => ApiError::receipt_expired(error.to_string()),
        CommandError::Invalid(message) => ApiError::new(StatusCode::BAD_REQUEST, message),
        CommandError::Conflict(message) => ApiError::new(StatusCode::CONFLICT, message),
        CommandError::NotFound => ApiError::new(StatusCode::NOT_FOUND, error.to_string()),
        CommandError::Missing(message) => ApiError::new(StatusCode::NOT_FOUND, message),
        CommandError::Entity(message) => ApiError::new(StatusCode::CONFLICT, message),
        CommandError::Outbox(error) => studio_runtime_store::StoreError::from(error).into(),
        CommandError::RequestResult(error) => error.into(),
        CommandError::Store(error) => error.into(),
        CommandError::Lifecycle(error) => super::agent_management::management_error(error),
        CommandError::Coordination(error) => super::messages::coordination_error(error),
    }
}

fn now_ms() -> Result<i64, ApiError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::internal())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ApiError::internal())
}

async fn command(
    State(state): State<HttpState>,
    ports: Option<Extension<Arc<dyn LifecyclePorts>>>,
    Caller(principal): Caller,
    ApiJson(request): ApiJson<CommandRequest>,
) -> Result<Json<CommandResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let now_ms = now_ms()?;
    let chat = request.command.target().to_owned();
    let mut extra_rows = Vec::new();
    let reference = match request.command.clone() {
        Command::Withdraw { .. } | Command::Stop { .. } | Command::Resolve { .. } => {
            let ports = ports.map(|Extension(ports)| ports);
            let (for_transaction, request) = (ports.clone(), request.clone());
            let now = now_ms as f64 / 1000.0;
            let executed = store
                .transaction_with_error(move |tx| {
                    commands::execute_sync(
                        tx,
                        &principal,
                        &request,
                        now_ms,
                        now,
                        for_transaction.as_deref(),
                    )
                })
                .await
                .map_err(command_error)?;
            if !executed.replayed {
                state.event_wake.notify_one();
                if let Some(ports) = ports {
                    // A failed wake cannot roll back the committed stop. The effect owner
                    // retries the staged effects.
                    if let Err(error) = ports.committed().await {
                        eprintln!("Lifecycle committed; effect delivery remains pending: {error}");
                    }
                }
            }
            executed.reference
        }
        Command::Send { target, body } => {
            match begin(&state, &principal, &request, now_ms).await? {
                Start::Replay(reference) => reference,
                Start::Fresh(begun) => {
                    let mut assets = body.assets.as_slice().to_vec();
                    let as_file = !begun.legacy_send() && (body.as_text_file
                        || body.text.as_str().chars().count()
                            > studio_runtime_schema::shared_values::MAX_MANAGED_MESSAGE_CODEPOINTS);
                    let text = if as_file {
                        if assets.len() >= studio_runtime_schema::command::COMMAND_SEND_MAX_ASSETS {
                            return Err(ApiError::new(
                                StatusCode::BAD_REQUEST,
                                "Too many attachments",
                            ));
                        }
                        let asset_id = uuid::Uuid::new_v5(
                            &uuid::Uuid::NAMESPACE_OID,
                            format!("text-file:{}", request.id.as_str()).as_bytes(),
                        )
                        .to_string();
                        // Asset storage commits before the message: a crash here can leave one
                        // unreferenced asset, and the same command retries the same asset id.
                        studio_runtime_services::payload_store::upload(
                            &store,
                            target.as_str(),
                            "message.txt",
                            Some(&asset_id),
                            &base64::engine::general_purpose::STANDARD
                                .encode(body.text.as_str().as_bytes()),
                        )
                        .await
                        .map_err(|error| {
                            if matches!(error, studio_runtime_services::payload_store::PayloadStoreError::UnknownAgent) {
                                ApiError::new(StatusCode::BAD_REQUEST, "Sending as a file is available only in managed chats")
                            } else {
                                super::workspace_files::map_payload_store_error(error)
                            }
                        })?;
                        assets.push(asset_id);
                        String::new()
                    } else {
                        body.text.as_str().to_owned()
                    };
                    // The command id is the message id: the message row is the receipt.
                    let message = crate::generated::ManagedMessageBody {
                        id: request.id.as_str().to_owned(),
                        room: target.as_str().to_owned(),
                        text,
                        assets: Some(assets),
                        delivery: body.delivery,
                    };
                    let id = request.id.as_str().to_owned();
                    let (Json(receipt), finished) =
                        super::messages::send_message_with(state.clone(), message, {
                            let (begun, id) = (begun.clone(), id.clone());
                            move |tx| finish_message(tx, &begun, &id)
                        })
                        .await
                        .map_err(same_id_conflict)?;
                    // The delivery receipt of the message, in the shape of
                    // `GET /api/messages/receipts`. It is not an entity row, so its seq is 0.
                    extra_rows.push(CommandRow {
                        collection: "messageReceipt".into(),
                        id: request.id.as_str().to_owned(),
                        seq: 0,
                        payload: serde_json::to_value(&receipt).map_err(ApiError::json)?,
                    });
                    match finished {
                        Some(reference) => reference,
                        // The federated branch does not post here, so it finishes in its own
                        // transaction.
                        None => store
                            .transaction_with_error(move |tx| finish_message(tx, &begun, &id))
                            .await
                            .map_err(command_error)?,
                    }
                }
            }
        }
    };
    let mut response = store
        .read_async("runtime-http/routes/commands", move |connection| {
            commands::response(connection, &reference, &chat)
        })
        .await
        .map_err(command_error)?;
    response.rows.extend(extra_rows);
    Ok(Json(response))
}

/// Record the reference of a sent message and raise the watermark. Call in the transaction
/// that posted the message.
fn finish_message(
    tx: &rusqlite::Transaction<'_>,
    begun: &commands::Begun,
    id: &str,
) -> Result<studio_runtime_store::request_results::RequestReference, CommandError> {
    let reference = commands::message_reference(tx, id)?;
    commands::finish(tx, begun, &reference)
}

async fn begin(
    state: &HttpState,
    principal: &Principal,
    request: &CommandRequest,
    now_ms: i64,
) -> Result<Start, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let (principal, request) = (principal.clone(), request.clone());
    store
        .transaction_with_error(move |tx| commands::begin(tx, &principal, &request, now_ms))
        .await
        .map_err(command_error)
}

/// The message route answers a reused id with other content as a bad request. The command
/// path answers it as a conflict, like every other command.
fn same_id_conflict(error: ApiError) -> ApiError {
    if error.status() == StatusCode::BAD_REQUEST && error.message() == commands::MESSAGE_ID_CONFLICT
    {
        return ApiError::new(StatusCode::CONFLICT, commands::MESSAGE_ID_CONFLICT);
    }
    error
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "commands",
        routes: Some(routes),
        dependencies: &[],
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::Request;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use studio_runtime_schema::persistence::{REQUEST_RESULT_TTL_MS, RequestIdentity};
    use studio_runtime_services::agent_lifecycle_runtime::LifecyclePorts;
    use studio_runtime_services::commands::CommandError;
    use studio_runtime_services::principal::{Grant, Principal};
    use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore};
    use tower::ServiceExt;

    use crate::router::HttpState;

    fn uuid_at(ms: i64) -> String {
        let bits = ((ms as u128) << 80) | (7 << 76) | (2 << 62) | 1;
        uuid::Uuid::from_u128(bits).to_string()
    }

    fn now_ms() -> i64 {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_millis(),
        )
        .expect("ms")
    }

    async fn post(app: &axum::Router, principal: Option<Principal>, body: Value) -> (u16, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/commands")
            .header("host", "127.0.0.1:4620")
            .header("content-type", "application/json")
            .header("x-canvas-token", "token")
            .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
            .extension(ConnectInfo(
                "127.0.0.1:12345"
                    .parse::<std::net::SocketAddr>()
                    .expect("peer"),
            ));
        if let Some(principal) = principal {
            builder = builder.extension(principal);
        }
        let response = app
            .clone()
            .oneshot(builder.body(Body::from(body.to_string())).expect("request"))
            .await
            .expect("response");
        let status = response.status().as_u16();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, serde_json::from_slice(&bytes).expect("JSON"))
    }

    fn withdraw(id: &str, number: u64, message: &str) -> Value {
        json!({"id":id,"client":"web-1","number":number,"kind":"withdraw","target":"workspace","body":{"message":message}})
    }

    async fn app_with_queue() -> (axum::Router, Arc<RuntimeStore>, tempfile::TempDir) {
        let directory = tempfile::tempdir().expect("isolated state");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        let cwd = directory.path().to_string_lossy().into_owned();
        store.transaction(move |tx| {
            for id in ["workspace", "other"] {
                AgentRepository::put(tx, &AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","id":id,"rootId":id,"epoch":0,"status":"idle","cwd":cwd,"provider":studio_runtime_schema::ProviderKind::Codex}))?)?;
            }
            tx.execute("INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch) VALUES('a','workspace','user','A','pending',1,0),('b','workspace','user','B','pending',2,0)", [])?;
            Ok(())
        }).await.expect("seed queue");
        let state = HttpState::new(
            "token".into(),
            "workspace".into(),
            directory.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        (crate::router(state).expect("router"), store, directory)
    }

    async fn status_of(store: &RuntimeStore, id: &'static str) -> String {
        store
            .transaction(move |tx| {
                Ok(tx.query_row(
                    "SELECT status FROM runtime_events WHERE id=?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .await
            .expect("status")
    }

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "test compares the external request schema compiler with the typed ingress"
    )]
    fn command_wire_matches_schema_boundaries() {
        use studio_runtime_schema::command::{
            COMMAND_CLIENT_ID_MAX_CHARS, COMMAND_SEND_MAX_ASSETS, COMMAND_TARGET_MAX_CHARS,
            CommandRequest,
        };
        let document = crate::schema::openapi_document();
        for name in [
            "CommandRequestSend",
            "CommandRequestWithdraw",
            "CommandRequestStop",
        ] {
            assert_eq!(
                document.pointer(&format!(
                    "/components/schemas/{name}/properties/client/maxLength"
                )),
                Some(&json!(COMMAND_CLIENT_ID_MAX_CHARS))
            );
            assert_eq!(
                document.pointer(&format!(
                    "/components/schemas/{name}/properties/target/maxLength"
                )),
                Some(&json!(COMMAND_TARGET_MAX_CHARS))
            );
            assert_eq!(
                document.pointer(&format!(
                    "/components/schemas/{name}/properties/number/maximum"
                )),
                Some(&json!(
                    studio_runtime_schema::shared_values::MAX_SAFE_INTEGER
                ))
            );
        }
        assert_eq!(
            document.pointer("/components/schemas/CommandSendBody/properties/assets/maxItems"),
            Some(&json!(COMMAND_SEND_MAX_ASSETS))
        );
        assert_eq!(
            document.pointer("/components/schemas/CommandSendBody/properties/text/maxLength"),
            Some(&json!(
                studio_runtime_schema::shared_values::MAX_DRAFT_TEXT_CODEPOINTS
            ))
        );
        let registry = studio_runtime_schema::registry_for_document(document)
            .expect("request schema registry");
        let schema = json!({"$ref":format!("{}#/components/schemas/CommandRequest", studio_runtime_schema::OPENAPI_URI)});
        let validator = jsonschema::options()
            .with_registry(&registry)
            .build(&schema)
            .expect("command schema");
        let base = withdraw("0192f3a0-0000-7000-8000-000000000001", 1, "m");
        let mut cases = vec![base.clone()];
        for (field, value) in [
            ("id", json!("bad")),
            ("client", json!("")),
            ("client", json!("x".repeat(129))),
            ("target", json!("")),
            ("target", json!("x".repeat(201))),
            ("number", json!(0)),
            ("number", json!(9007199254740992u64)),
            ("extra", json!(true)),
        ] {
            let mut case = base.clone();
            case[field] = value;
            cases.push(case);
        }
        let mut empty = base.clone();
        empty["body"]["message"] = json!("");
        cases.push(empty);
        let mut send = base;
        send["kind"] = json!("send");
        send["body"] = json!({"text":"hello"});
        cases.push(send.clone());
        send["body"]["assets"] = json!(vec!["a"; 9]);
        cases.push(send.clone());
        send["body"] = json!({"text":"hello", "delivery":null});
        cases.push(send);
        for case in cases {
            let schema = validator.is_valid(&case);
            let typed = serde_json::from_str::<CommandRequest>(&case.to_string()).is_ok();
            assert_eq!(typed, schema, "{case}");
        }
    }

    #[test]
    fn unflagged_send_serializes_without_changing_fingerprint() {
        let request: studio_runtime_schema::command::CommandRequest = serde_json::from_str(r#"{"id":"0192f3a0-0000-7000-8000-000000000001","client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello"}}"#).expect("request");
        assert_eq!(
            serde_json::to_string(&request.command).expect("serialize"),
            r#"{"kind":"send","target":"workspace","body":{"text":"hello","assets":[]}}"#
        );
    }

    #[tokio::test]
    async fn command_wire_rejects_unknown_and_out_of_range_fields() {
        let (app, _store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let base = withdraw(&id, 1, "a");
        let mut cases = Vec::new();
        let mut extra = base.clone();
        extra["extra"] = json!(true);
        cases.push(extra);
        let mut client = base.clone();
        client["client"] = json!("x".repeat(129));
        cases.push(client);
        let mut number = base.clone();
        number["number"] = json!(studio_runtime_schema::shared_values::MAX_SAFE_INTEGER + 1);
        cases.push(number);
        let mut body = base;
        body["body"]["extra"] = json!(true);
        cases.push(body);
        for case in cases {
            let (status, response) = post(&app, None, case).await;
            assert_eq!(status, 400, "{response}");
            assert_eq!(response["error"], "Invalid request");
            assert_eq!(response["location"][0], "body");
        }
    }

    #[tokio::test]
    async fn replay_returns_the_same_reference_and_runs_once() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let (status, first) = post(&app, None, withdraw(&id, 1, "a")).await;
        assert_eq!(status, 200, "{first}");
        assert_eq!(first["status"], "applied");
        assert_eq!(first["ref_kind"], "message");
        assert_eq!(first["ref_id"], "a");
        assert_eq!(first["rev"], 2);
        assert_eq!(status_of(&store, "a").await, "cancelled");
        let (status, again) = post(&app, None, withdraw(&id, 1, "a")).await;
        assert_eq!(status, 200, "{again}");
        for field in ["status", "ref_kind", "ref_id", "rev"] {
            assert_eq!(first[field], again[field], "{field}");
        }
        assert_eq!(status_of(&store, "b").await, "pending");
    }

    #[tokio::test]
    async fn pre_upgrade_send_skips_admission_and_file_conversion() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        store.transaction(|tx| { tx.execute("INSERT INTO runtime_orchestration_settings(id,record) VALUES('global',json_object('sendAdmissionCutoverMs',?1)) ON CONFLICT(id) DO UPDATE SET record=json_set(record,'$.sendAdmissionCutoverMs',?1)", [now_ms() + 60_000])?; Ok(()) }).await.expect("cutover");
        let (status, response) = post(&app, None, json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello","asTextFile":true}})).await;
        assert_eq!(status, 200, "{response}");
        let state = store
            .read_async("legacy-test", move |db| {
                Ok::<_, studio_runtime_store::StoreError>((
                    db.query_row(
                        "SELECT text FROM runtime_events WHERE id=?1",
                        [&id],
                        |row| row.get::<_, String>(0),
                    )?,
                    db.query_row("SELECT count(*) FROM command_send_admissions", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    db.query_row("SELECT count(*) FROM runtime_assets", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                ))
            })
            .await
            .expect("state");
        assert_eq!(state, ("hello".to_owned(), 0, 0));
    }

    #[tokio::test]
    async fn pre_upgrade_normalized_retry_never_reserves_fingerprint() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let request = json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello","delivery":"current_turn"}});
        assert_eq!(post(&app, None, request.clone()).await.0, 200);
        store.transaction(|tx| { tx.execute("DELETE FROM command_send_admissions", [])?; tx.execute("INSERT INTO runtime_orchestration_settings(id,record) VALUES('global',json_object('sendAdmissionCutoverMs',?1)) ON CONFLICT(id) DO UPDATE SET record=json_set(record,'$.sendAdmissionCutoverMs',?1)", [now_ms() + 60_000])?; Ok(()) }).await.expect("cutover");
        let mut omitted = request.clone();
        omitted["body"]
            .as_object_mut()
            .expect("body")
            .remove("delivery");
        assert_eq!(post(&app, None, omitted).await.0, 200);
        assert_eq!(post(&app, None, request).await.0, 200);
    }

    #[tokio::test]
    async fn pre_upgrade_send_conflict_does_not_poison_original_retry() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let request = json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello"}});
        assert_eq!(post(&app, None, request.clone()).await.0, 200);
        store
            .transaction(|tx| {
                tx.execute("DELETE FROM command_send_admissions", [])?;
                tx.execute("INSERT INTO runtime_orchestration_settings(id,record) VALUES('global',json_object('sendAdmissionCutoverMs',?1)) ON CONFLICT(id) DO UPDATE SET record=json_set(record,'$.sendAdmissionCutoverMs',?1)", [now_ms() + 60_000])?;
                Ok(())
            })
            .await
            .expect("pre-upgrade state");
        let mut changed = request.clone();
        changed["body"]["delivery"] = json!("next_turn");
        assert_eq!(post(&app, None, changed).await.0, 409);
        let mut changed = request.clone();
        changed["body"]["assets"] = json!(["7f4ed766-564a-4e1c-ae4f-d8e7f9b4d384"]);
        assert_eq!(post(&app, None, changed).await.0, 409);
        assert_eq!(post(&app, None, request).await.0, 200);
    }

    #[tokio::test]
    async fn send_fingerprint_is_reserved_before_upload_and_checked_after_completion() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let request = json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello","asTextFile":true}});
        let typed: studio_runtime_schema::command::CommandRequest =
            serde_json::from_str(&request.to_string()).expect("command");
        store
            .transaction_with_error(move |tx| {
                studio_runtime_services::commands::begin(tx, &Principal::Owner, &typed, now_ms())
            })
            .await
            .expect("admission");
        let asset_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            format!("text-file:{id}").as_bytes(),
        )
        .to_string();
        studio_runtime_services::payload_store::upload(
            &store,
            "workspace",
            "message.txt",
            Some(&asset_id),
            "aGVsbG8=",
        )
        .await
        .expect("uploaded before interrupted send");
        let mut changed = request.clone();
        changed["body"]["delivery"] = json!("next_turn");
        let (status, _) = post(&app, None, changed).await;
        assert_eq!(status, 409, "changed retry after admission");
        let mut changed = request.clone();
        changed["body"]["assets"] = json!(["other"]);
        let (status, _) = post(&app, None, changed).await;
        assert_eq!(status, 409, "changed attachments after admission");
        let (status, response) = post(&app, None, request.clone()).await;
        assert_eq!(status, 200, "{response}");
        let mut changed = request;
        changed["body"]["asTextFile"] = json!(false);
        let (status, _) = post(&app, None, changed).await;
        assert_eq!(status, 409, "changed retry after completion");
    }

    #[tokio::test]
    async fn send_command_uses_the_typed_message_path() {
        let (app, _store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let (status, response) = post(&app, None, json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello"}})).await;
        assert_eq!(status, 200, "{response}");
        assert_eq!(response["ref_id"], id);
    }

    async fn message_count(store: &RuntimeStore, id: &str) -> i64 {
        let id = id.to_owned();
        store
            .transaction(move |tx| {
                Ok(tx.query_row(
                    "SELECT COUNT(*) FROM runtime_events WHERE id=?1",
                    [id.as_str()],
                    |row| row.get::<_, i64>(0),
                )?)
            })
            .await
            .expect("message count")
    }

    #[tokio::test]
    async fn a_send_whose_record_fails_rolls_back_its_message_and_the_retry_succeeds_once() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        let send = json!({"id":id,"client":"web-1","number":1,"kind":"send","target":"workspace","body":{"text":"hello"}});
        // Injected failure in `finish`: the watermark write fails after the message is posted.
        store
            .transaction(|tx| {
                tx.execute_batch(
                    "CREATE TRIGGER fail_send_record_insert BEFORE INSERT ON client_command_watermarks
                     BEGIN SELECT RAISE(ABORT,'injected record failure'); END;
                     CREATE TRIGGER fail_send_record_update BEFORE UPDATE ON client_command_watermarks
                     BEGIN SELECT RAISE(ABORT,'injected record failure'); END;",
                )?;
                Ok(())
            })
            .await
            .expect("arm failure");
        let (status, response) = post(&app, None, send.clone()).await;
        assert!((500..600).contains(&status), "{status} {response}");
        assert_eq!(message_count(&store, &id).await, 0);
        store
            .transaction(|tx| {
                tx.execute_batch(
                    "DROP TRIGGER fail_send_record_insert; DROP TRIGGER fail_send_record_update;",
                )?;
                Ok(())
            })
            .await
            .expect("disarm failure");
        // The retry with the same id is not a conflict and posts the message once.
        let (status, response) = post(&app, None, send.clone()).await;
        assert_eq!(status, 200, "{response}");
        assert_eq!(response["ref_id"], id);
        assert_eq!(message_count(&store, &id).await, 1);
        let (status, response) = post(&app, None, send).await;
        assert_eq!(status, 200, "{response}");
        assert_eq!(response["ref_id"], id);
        assert_eq!(message_count(&store, &id).await, 1);
    }

    #[tokio::test]
    async fn group_text_file_send_is_refused_clearly() {
        let (app, store, _directory) = app_with_queue().await;
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO groups(id,name,members) VALUES('group','Group','[]')",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("group");
        let (status, response) = post(&app, None, json!({"id":RequestIdentity::issue_root(),"client":"web-1","number":1,"kind":"send","target":"group","body":{"text":"hello","asTextFile":true}})).await;
        assert_eq!(status, 400);
        assert_eq!(
            response["error"],
            "Sending as a file is available only in managed chats"
        );
    }

    #[tokio::test]
    async fn text_file_send_and_long_paste_are_idempotent() {
        for (number, flagged) in [(1, true), (2, false)] {
            let (app, store, _directory) = app_with_queue().await;
            let id = RequestIdentity::issue_root();
            let text = if flagged {
                "short text".to_owned()
            } else {
                "x".repeat(studio_runtime_schema::shared_values::MAX_MANAGED_MESSAGE_CODEPOINTS + 1)
            };
            let mut body = json!({"text":text});
            if flagged {
                body["asTextFile"] = json!(true);
            }
            let request = json!({"id":id,"client":"web-1","number":number,"kind":"send","target":"workspace","body":body});
            let (status, first) = post(&app, None, request.clone()).await;
            assert_eq!(status, 200, "{first}");
            let (status, second) = post(&app, None, request).await;
            assert_eq!(status, 200, "{second}");
            assert_eq!(first["ref_id"], second["ref_id"]);
            let id = id.to_string();
            let (counts, path, stored_assets) = store
                .transaction(move |tx| {
                    let path: String = tx.query_row(
                        "SELECT json_extract(record,'$.path') FROM runtime_assets",
                        [],
                        |r| r.get(0),
                    )?;
                    let stored_assets: String = tx.query_row(
                        "SELECT record FROM runtime_event_meta WHERE id=?1",
                        [&id],
                        |r| r.get(0),
                    )?;
                    Ok((
                        (
                            tx.query_row("SELECT count(*) FROM runtime_assets", [], |r| {
                                r.get::<_, i64>(0)
                            })?,
                            tx.query_row(
                                "SELECT count(*) FROM runtime_events WHERE id=?1",
                                [&id],
                                |r| r.get::<_, i64>(0),
                            )?,
                        ),
                        path,
                        stored_assets,
                    ))
                })
                .await
                .expect("counts");
            assert_eq!(counts, (1, 1));
            assert_eq!(
                tokio::fs::read_to_string(path)
                    .await
                    .expect("asset content"),
                text
            );
            assert!(stored_assets.contains("assets"));
        }
    }

    #[tokio::test]
    async fn same_id_with_another_body_is_a_conflict() {
        let (app, store, _directory) = app_with_queue().await;
        let id = RequestIdentity::issue_root();
        assert_eq!(post(&app, None, withdraw(&id, 1, "a")).await.0, 200);
        let (status, _) = post(&app, None, withdraw(&id, 1, "b")).await;
        assert_eq!(status, 409);
        assert_eq!(status_of(&store, "b").await, "pending");
    }

    #[tokio::test]
    async fn a_command_older_than_the_window_is_rejected_as_too_old() {
        let (app, store, _directory) = app_with_queue().await;
        let fresh = RequestIdentity::issue_root();
        assert_eq!(post(&app, None, withdraw(&fresh, 5, "a")).await.0, 200);
        let old = uuid_at(now_ms() - REQUEST_RESULT_TTL_MS - 1_000);
        let (status, body) = post(&app, None, withdraw(&old, 3, "b")).await;
        assert_eq!(status, 410, "{body}");
        assert!(
            body["error"]
                .as_str()
                .expect("error")
                .contains("already applied")
        );
        assert_eq!(status_of(&store, "b").await, "pending");
        let (status, body) = post(&app, None, withdraw(&old, 6, "b")).await;
        assert_eq!(status, 410, "{body}");
        assert!(body["error"].as_str().expect("error").contains("expired"));
        assert_eq!(status_of(&store, "b").await, "pending");
    }

    #[tokio::test]
    async fn an_observe_device_cannot_send() {
        let (app, store, _directory) = app_with_queue().await;
        let observe = Principal::Device {
            id: "phone".into(),
            grant: Grant::Observe,
        };
        let (status, _) = post(
            &app,
            Some(observe),
            json!({"id":RequestIdentity::issue_root(),"client":"p","number":1,"kind":"send","target":"workspace","body":{"text":"hi"}}),
        )
        .await;
        assert_eq!(status, 403);
        let count: i64 = store
            .transaction(|tx| {
                Ok(tx.query_row("SELECT count(*) FROM runtime_events", [], |row| row.get(0))?)
            })
            .await
            .expect("count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn an_agent_outside_its_descendants_is_refused() {
        let (app, store, _directory) = app_with_queue().await;
        let agent = Principal::Agent {
            id: "other".into(),
            epoch: 0,
        };
        let (status, _) = post(
            &app,
            Some(agent),
            withdraw(&RequestIdentity::issue_root(), 1, "a"),
        )
        .await;
        assert_eq!(status, 403);
        assert_eq!(status_of(&store, "a").await, "pending");
    }

    fn stop(id: &str, number: u64) -> Value {
        json!({"id":id,"client":"web-1","number":number,"kind":"stop","target":"workspace","body":{"descendants":true}})
    }

    async fn agent_status(store: &RuntimeStore) -> String {
        store
            .transaction(|tx| {
                let agent = AgentRepository::get(tx, &"workspace".into())?.expect("agent");
                Ok(format!("{:?}", agent.status))
            })
            .await
            .expect("status")
    }

    async fn request_result_count(store: &RuntimeStore) -> i64 {
        store
            .transaction(|tx| {
                Ok(tx.query_row("SELECT count(*) FROM request_results", [], |row| row.get(0))?)
            })
            .await
            .expect("count")
    }

    /// Seed an outbox entry of `kind` for the agent `workspace` with `status`.
    async fn seed_entry(store: &RuntimeStore, kind: &'static str, status: &'static str) {
        store
            .transaction(move |tx| {
                tx.execute(
                    "INSERT INTO runtime_outbox(id,kind,owner_kind,owner_id,request_id,signature,status,account,stream_id,generation,ordinal,next_at,payload,created,target) VALUES('entry',?1,'agent','workspace',NULL,'sig',?2,NULL,NULL,NULL,NULL,NULL,'{}',1,NULL)",
                    [kind, status],
                )?;
                Ok(())
            })
            .await
            .expect("seed outbox entry");
    }

    async fn entry_row(store: &RuntimeStore) -> (String, Option<i64>, String) {
        store
            .transaction(|tx| {
                Ok(tx.query_row(
                    "SELECT status,next_at,payload FROM runtime_outbox WHERE id='entry'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?)
            })
            .await
            .expect("entry row")
    }

    fn resolve(id: &str, number: u64, action: &str) -> Value {
        json!({"id":id,"client":"web-1","number":number,"kind":"resolve","target":"entry","body":{"action":action}})
    }

    #[tokio::test]
    async fn resolve_retry_queues_the_same_entry_with_its_body() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "unknown").await;
        let (status, body) = post(
            &app,
            None,
            resolve(&RequestIdentity::issue_root(), 1, "retry"),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["ref_kind"], "outbox");
        assert_eq!(body["ref_id"], "entry");
        let (status, next_at, payload) = entry_row(&store).await;
        assert_eq!(status, "pending");
        assert!(next_at.is_some(), "a retry is due");
        assert_eq!(payload, "{}", "the body is unchanged");
    }

    #[tokio::test]
    async fn resolve_delivered_commits_the_entry() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "unknown").await;
        let (status, body) = post(
            &app,
            None,
            resolve(&RequestIdentity::issue_root(), 1, "delivered"),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let (status, next_at, _) = entry_row(&store).await;
        assert_eq!(status, "committed");
        assert_eq!(next_at, None);
    }

    #[tokio::test]
    async fn resolve_discard_cancels_the_entry_and_keeps_its_payload() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "unknown").await;
        let (status, body) = post(
            &app,
            None,
            resolve(&RequestIdentity::issue_root(), 1, "discard"),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let (status, next_at, payload) = entry_row(&store).await;
        assert_eq!(status, "cancelled");
        assert_eq!(next_at, None);
        assert_eq!(payload, "{}", "the row keeps its payload");
    }

    #[tokio::test]
    async fn resolve_refuses_an_entry_that_is_not_unknown() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "pending").await;
        let (status, _) = post(
            &app,
            None,
            resolve(&RequestIdentity::issue_root(), 1, "delivered"),
        )
        .await;
        assert_eq!(status, 409);
        assert_eq!(entry_row(&store).await.0, "pending");
    }

    #[tokio::test]
    async fn resolve_refuses_a_steer_retry_and_leaves_it_unknown() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "steer", "unknown").await;
        let (status, _) = post(
            &app,
            None,
            resolve(&RequestIdentity::issue_root(), 1, "retry"),
        )
        .await;
        assert_eq!(status, 400);
        assert_eq!(entry_row(&store).await.0, "unknown");
    }

    #[tokio::test]
    async fn resolve_replay_returns_the_first_reference_and_changes_nothing_again() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "unknown").await;
        let id = RequestIdentity::issue_root();
        let (status, first) = post(&app, None, resolve(&id, 1, "delivered")).await;
        assert_eq!(status, 200, "{first}");
        let (status, again) = post(&app, None, resolve(&id, 1, "delivered")).await;
        assert_eq!(status, 200, "{again}");
        assert_eq!(first["ref_id"], again["ref_id"]);
        assert_eq!(entry_row(&store).await.0, "committed");
    }

    #[tokio::test]
    async fn an_observe_device_cannot_resolve() {
        let (app, store, _directory) = app_with_queue().await;
        seed_entry(&store, "provider_call", "unknown").await;
        let observe = Principal::Device {
            id: "phone".into(),
            grant: Grant::Observe,
        };
        let (status, _) = post(
            &app,
            Some(observe),
            resolve(&RequestIdentity::issue_root(), 1, "delivered"),
        )
        .await;
        assert_eq!(status, 403);
        assert_eq!(entry_row(&store).await.0, "unknown");
    }

    /// Records the lifecycle effects that a command stages in its transaction.
    struct StagingPorts(
        Arc<
            std::sync::Mutex<
                Vec<studio_runtime_services::agent_lifecycle_runtime::LifecycleEffectKind>,
            >,
        >,
    );

    impl LifecyclePorts for StagingPorts {
        fn stage(
            &self,
            _: &rusqlite::Transaction<'_>,
            effect: &studio_runtime_services::agent_lifecycle_runtime::LifecycleEffect,
        ) -> Result<(), studio_runtime_services::agent_management::ManagementError> {
            self.0.lock().expect("staged effects").push(effect.kind());
            Ok(())
        }

        fn committed(
            &self,
        ) -> studio_runtime_services::agent_lifecycle_runtime::LifecycleCommitFuture<'_> {
            Box::pin(async { Ok(()) })
        }
    }

    /// The owner withdraws message `a`, which a running turn of `workspace` holds.
    async fn hold_a_and_withdraw_it(
        store: &RuntimeStore,
        staged: Arc<
            std::sync::Mutex<
                Vec<studio_runtime_services::agent_lifecycle_runtime::LifecycleEffectKind>,
            >,
        >,
        forced_rollback: bool,
    ) -> Result<studio_runtime_services::commands::Executed, CommandError> {
        use studio_runtime_services::commands::execute_sync;
        store
            .transaction(|tx| {
                tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.status','running','$.inFlight',json('true'),'$.turnId','turn-1') WHERE id='workspace'", [])?;
                tx.execute("UPDATE runtime_events SET status='reserved' WHERE id='a'", [])?;
                Ok(())
            })
            .await
            .expect("hold a");
        // The owner's command id must be a UUIDv7, as the HTTP edge requires.
        let id = RequestIdentity::issue_root();
        let request: studio_runtime_schema::command::CommandRequest =
            serde_json::from_value(withdraw(&id, 1, "a")).expect("request");
        let now = now_ms();
        store
            .transaction_with_error(move |tx| {
                let ports = StagingPorts(staged);
                let dynamic: &dyn LifecyclePorts = &ports;
                let executed = execute_sync(
                    tx,
                    &Principal::Owner,
                    &request,
                    now,
                    now as f64 / 1000.0,
                    Some(dynamic),
                )?;
                if forced_rollback {
                    return Err(CommandError::Invalid("forced rollback"));
                }
                Ok(executed)
            })
            .await
    }

    #[tokio::test]
    async fn withdrawing_a_held_message_writes_the_draft_and_stages_the_stop_in_one_commit() {
        use studio_runtime_services::agent_lifecycle_runtime::LifecycleEffectKind;
        let (_app, store, _directory) = app_with_queue().await;
        let staged = Arc::new(std::sync::Mutex::new(Vec::new()));
        let executed = hold_a_and_withdraw_it(&store, staged.clone(), false)
            .await
            .expect("withdraw held");
        assert!(!executed.replayed);
        assert_eq!(executed.reference.id, "a");
        assert_eq!(
            *staged.lock().expect("staged effects"),
            vec![LifecycleEffectKind::Interrupt]
        );
        assert_eq!(status_of(&store, "a").await, "cancelled");
        let draft: String = store
            .transaction(|tx| {
                Ok(tx.query_row(
                    "SELECT payload FROM sync_documents WHERE scope='drafts' AND id='server:withdraw:workspace'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .await
            .expect("server draft");
        let draft: Value = serde_json::from_str(&draft).expect("draft JSON");
        assert_eq!(draft["text"], "A");
    }

    #[tokio::test]
    async fn a_held_withdraw_that_rolls_back_leaves_the_message_the_draft_and_the_turn_as_they_were()
     {
        let (_app, store, _directory) = app_with_queue().await;
        let staged = Arc::new(std::sync::Mutex::new(Vec::new()));
        let outcome = hold_a_and_withdraw_it(&store, staged, true).await;
        assert!(matches!(
            outcome,
            Err(CommandError::Invalid("forced rollback"))
        ));
        assert_eq!(status_of(&store, "a").await, "reserved");
        let drafts: i64 = store
            .transaction(|tx| {
                Ok(tx.query_row(
                    "SELECT count(*) FROM sync_documents WHERE scope='drafts' AND id='server:withdraw:workspace'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .await
            .expect("draft count");
        assert_eq!(drafts, 0);
    }

    #[tokio::test]
    async fn withdraw_returns_the_text_as_a_server_draft() {
        let (app, store, _directory) = app_with_queue().await;
        let (status, body) =
            post(&app, None, withdraw(&RequestIdentity::issue_root(), 1, "a")).await;
        assert_eq!(status, 200, "{body}");
        let draft: String = store
            .transaction(|tx| {
                Ok(tx.query_row(
                    "SELECT payload FROM sync_documents WHERE scope='drafts' AND id='server:withdraw:workspace'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .await
            .expect("server draft");
        let draft: Value = serde_json::from_str(&draft).expect("draft JSON");
        assert_eq!(draft["text"], "A");
        assert_eq!(draft["session"], "workspace");
        assert_eq!(draft["device"], "server");
    }

    #[tokio::test]
    async fn stop_and_its_reference_commit_together_and_a_replay_does_not_stop_again() {
        let (app, store, _directory) = app_with_queue().await;
        let before = agent_status(&store).await;
        let id = RequestIdentity::issue_root();
        let (status, first) = post(&app, None, stop(&id, 1)).await;
        assert_eq!(status, 200, "{first}");
        assert_eq!(first["ref_kind"], "agent");
        assert_eq!(
            agent_status(&store).await,
            before,
            "the stop does not pause the chat"
        );
        assert_eq!(
            status_of(&store, "a").await,
            "pending",
            "the stop does not clear the queue"
        );
        assert_eq!(request_result_count(&store).await, 1);
        let (status, again) = post(&app, None, stop(&id, 1)).await;
        assert_eq!(status, 200, "{again}");
        assert_eq!(first["rev"], again["rev"]);
        assert_eq!(request_result_count(&store).await, 1);
    }

    #[tokio::test]
    async fn a_stop_that_cannot_record_its_reference_changes_nothing() {
        let (_app, store, _directory) = app_with_queue().await;
        let before = agent_status(&store).await;
        let request: studio_runtime_schema::command::CommandRequest =
            serde_json::from_value(stop(&RequestIdentity::issue_root(), 1)).expect("stop");
        let now = now_ms();
        let outcome = store
            .transaction_with_error(move |tx| {
                studio_runtime_services::commands::execute_sync(
                    tx,
                    &Principal::Owner,
                    &request,
                    now,
                    now as f64 / 1000.0,
                    None,
                )?;
                // The commit fails after the stop: both the stop and the reference go.
                Err::<(), _>(studio_runtime_services::commands::CommandError::Invalid(
                    "forced rollback",
                ))
            })
            .await;
        assert!(outcome.is_err());
        assert_eq!(agent_status(&store).await, before);
        assert_eq!(request_result_count(&store).await, 0);
    }

    #[tokio::test]
    async fn an_agent_withdraws_from_its_descendant_with_its_tool_call_id() {
        use studio_runtime_services::commands::{agent_request, execute_sync};
        let (_app, store, _directory) = app_with_queue().await;
        store
            .transaction(|tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","id":"kid","rootId":"workspace","parentId":"workspace","epoch":0,"status":"idle","provider":studio_runtime_schema::ProviderKind::Codex}))?)?;
                tx.execute("INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch) VALUES('k','kid','user','K','pending',3,0)", [])?;
                Ok(())
            })
            .await
            .expect("seed child");
        let parent = Principal::Agent {
            id: "workspace".into(),
            epoch: 0,
        };
        // A tool call id is not a UUIDv7. The server numbers the commands of an agent.
        let request = agent_request(
            "workspace",
            "call_123",
            studio_runtime_schema::command::Command::Withdraw {
                target: "kid".to_owned().try_into().expect("target"),
                body: studio_runtime_schema::command::WithdrawBody {
                    message: "k".to_owned().try_into().expect("message"),
                },
            },
        )
        .expect("agent request");
        let now = now_ms();
        let run = |request: studio_runtime_schema::command::CommandRequest| {
            let parent = parent.clone();
            let store = store.clone();
            async move {
                store
                    .transaction_with_error(move |tx| {
                        execute_sync(tx, &parent, &request, now, now as f64 / 1000.0, None)
                    })
                    .await
            }
        };
        let first = run(request.clone()).await.expect("first call");
        assert!(!first.replayed);
        assert_eq!(first.reference.id, "k");
        let again = run(request).await.expect("repeated call");
        assert!(again.replayed);
        assert_eq!(first.reference, again.reference);
    }
}
