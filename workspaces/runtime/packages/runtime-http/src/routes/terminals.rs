//! Persisted terminal reads and session-dependent terminal operations.

use super::extractors::{ApiJson, TypedQuery};
#[cfg(not(unix))]
use super::manifest::Dependency;
use super::manifest::{OperationStatus, RouteGroup, get, post};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(test)]
use studio_runtime_schema::terminal_ingress::{
    TERMINAL_COLS_MAX, TERMINAL_COLS_MIN, TERMINAL_CREATE_AGENT_ID_MAX_CODEPOINTS,
    TERMINAL_CREATE_REQUEST_ID_MAX_CODEPOINTS, TERMINAL_ROWS_MAX, TERMINAL_ROWS_MIN,
};
use studio_runtime_schema::terminal_ingress::{
    TerminalAction, TerminalClose, TerminalCreateBody, TerminalInput, TerminalRename,
    TerminalResize,
};
#[cfg(unix)]
use studio_runtime_services::terminal_driver::TerminalCreate;
use studio_runtime_services::terminal_driver::{
    self, TERMINAL_COLS_DEFAULT, TERMINAL_ROWS_DEFAULT, TerminalError,
};
use studio_runtime_store::path_identity::CanonicalPath;
use studio_runtime_store::{AgentId, AgentRepository};

use crate::error::ApiError;
use crate::router::HttpState;

#[cfg(not(unix))]
const PLATFORM_DEPENDENCIES: [Dependency; 1] = [Dependency::new("runtime-process/local-pty-unix")];
#[cfg(unix)]
const ROUTE_DEPENDENCIES: &[&str] = &[];
#[cfg(not(unix))]
const ROUTE_DEPENDENCIES: &[&str] = &["runtime-process/local-pty-unix"];

fn live_operation_status() -> OperationStatus {
    #[cfg(unix)]
    {
        OperationStatus::Implemented
    }
    #[cfg(not(unix))]
    {
        OperationStatus::partial(&PLATFORM_DEPENDENCIES)
    }
}

pub(crate) fn routes() -> RouteGroup {
    let mut routes = vec![
        get("/api/terminals", OperationStatus::Implemented, listing),
        get(
            "/api/terminals/output",
            OperationStatus::Implemented,
            output,
        ),
    ];
    routes.extend([
        post("/api/terminals/input", live_operation_status(), input),
        post("/api/terminals/resize", live_operation_status(), resize),
        post("/api/terminals/rename", live_operation_status(), rename),
        post("/api/terminals/close", live_operation_status(), close),
    ]);
    routes.push(post(
        "/api/terminals/create",
        live_operation_status(),
        create,
    ));
    RouteGroup::new(routes)
}

#[cfg(unix)]
async fn require_reconciled_driver(state: &HttpState) -> Result<(), ApiError> {
    let Some(gate) = state.terminal_write_gate_handle() else {
        return Ok(());
    };
    let reconciled = gate().await;
    if reconciled {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Terminal restart cleanup is still pending",
        ))
    }
}

pub(super) async fn listing(
    State(state): State<HttpState>,
) -> Result<Json<crate::generated::TerminalList>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let result = terminal_driver::listing(&store)
        .await
        .map_err(terminal_error)?;
    Ok(Json(
        super::generated_json::<crate::generated::TerminalList>(result)?,
    ))
}

#[derive(serde::Deserialize)]
enum HistoryFlag {
    #[serde(rename = "1")]
    Enabled,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalOutputQuery {
    id: Option<
        studio_runtime_schema::bounded_text::BoundedText<
            0,
            { studio_runtime_schema::terminal_ingress::TERMINAL_ID_MAX_CODEPOINTS },
        >,
    >,
    offset: Option<
        studio_runtime_schema::bounded_integer::BoundedInteger<
            0,
            { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 },
        >,
    >,
    history: Option<HistoryFlag>,
    limit: Option<
        studio_runtime_schema::bounded_integer::BoundedInteger<
            1,
            { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 },
        >,
    >,
}

async fn output(
    TypedQuery(query): TypedQuery<TerminalOutputQuery>,
    State(state): State<HttpState>,
) -> Result<Json<crate::generated::TerminalOutput>, ApiError> {
    let id = query
        .id
        .as_ref()
        .map_or("", |value| value.as_str())
        .to_owned();
    let offset = query.offset.map_or(0, |value| value.get());
    let history = query.history.is_some();
    let limit = query.limit.map_or(65_536, |value| value.get());
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let result = if history {
        terminal_driver::history_output(Arc::clone(&store), &id, offset, limit)
            .await
            .map_err(terminal_error)?
    } else {
        terminal_driver::output(&store, &id, offset)
            .await
            .map_err(terminal_error)?
    };
    Ok(Json(super::generated_json::<
        crate::generated::TerminalOutput,
    >(result)?))
}

async fn create(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<TerminalCreateBody>,
) -> Result<Json<crate::generated::TerminalRecord>, ApiError> {
    #[cfg(unix)]
    {
        require_reconciled_driver(&state).await?;
        let store = state.store.clone().ok_or_else(ApiError::internal)?;
        let agent_id = body.agent.as_str().to_owned();
        let stored = store
            .read_async("runtime-http/routes/terminals", move |connection| {
                AgentRepository::get(connection, &AgentId::from(agent_id.as_str()))
                    .map_err(TerminalError::from)
            })
            .await
            .map_err(terminal_body_error)?;
        let (agent, agent_cwd, cwd) = studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-http/routes/terminals",
            move || checked_agent_cwd(stored),
        )
        .await
        .map_err(|_| ApiError::internal())?
        .map_err(terminal_body_error)?;
        let columns = body
            .cols
            .map_or(TERMINAL_COLS_DEFAULT, |value| value.get() as u64);
        let rows = body
            .rows
            .map_or(TERMINAL_ROWS_DEFAULT, |value| value.get() as u64);
        let request_id = body.id.as_str();
        let shell = terminal_shell().await;
        let environment = serde_json::json!({
            "TERM":"xterm-256color",
            "COLORTERM":"truecolor"
        });
        let driver = state
            .terminal_driver_handle()
            .ok_or_else(ApiError::internal)?;
        let result = driver
            .create(TerminalCreate {
                request_id,
                agent: &agent,
                agent_cwd: &agent_cwd,
                cwd: &cwd,
                columns,
                rows,
                shell: &shell,
                environment,
                submitted_request: &body,
            })
            .await
            .map_err(terminal_body_error)?;
        Ok(Json(super::generated_json::<
            crate::generated::TerminalRecord,
        >(result)?))
    }
    #[cfg(not(unix))]
    {
        let _ = (state, body);
        Err(super::manifest::dependency_unavailable(
            "POST",
            "/api/terminals/create",
            &PLATFORM_DEPENDENCIES,
        )
        .unwrap_or_else(|_| ApiError::internal()))
    }
}

async fn input(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<TerminalInput>,
) -> Result<Json<TerminalActionResponse>, ApiError> {
    action(state, TerminalAction::Input(body), "/api/terminals/input").await
}
async fn resize(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<TerminalResize>,
) -> Result<Json<TerminalActionResponse>, ApiError> {
    action(state, TerminalAction::Resize(body), "/api/terminals/resize").await
}
async fn rename(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<TerminalRename>,
) -> Result<Json<TerminalActionResponse>, ApiError> {
    action(state, TerminalAction::Rename(body), "/api/terminals/rename").await
}
async fn close(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<TerminalClose>,
) -> Result<Json<TerminalActionResponse>, ApiError> {
    action(state, TerminalAction::Close(body), "/api/terminals/close").await
}
async fn action(
    state: HttpState,
    body: TerminalAction,
    _path: &'static str,
) -> Result<Json<TerminalActionResponse>, ApiError> {
    #[cfg(unix)]
    {
        require_reconciled_driver(&state).await?;
        let driver = state
            .terminal_driver_handle()
            .ok_or_else(ApiError::internal)?;
        let is_input = matches!(body, TerminalAction::Input(_));
        let result = driver
            .perform_action(&body)
            .await
            .map_err(terminal_body_error)?;
        if is_input {
            Ok(Json(TerminalActionResponse::Input(
                super::generated_json::<crate::generated::TerminalInputResult>(result)?,
            )))
        } else {
            Ok(Json(TerminalActionResponse::Record(
                super::generated_json::<crate::generated::TerminalRecord>(result)?,
            )))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (state, body);
        Err(
            super::manifest::dependency_unavailable("POST", _path, &PLATFORM_DEPENDENCIES)
                .unwrap_or_else(|_| ApiError::internal()),
        )
    }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum TerminalActionResponse {
    Input(crate::generated::TerminalInputResult),
    Record(crate::generated::TerminalRecord),
}

/// Check a stored agent and canonicalize its working directory (filesystem work).
#[allow(
    clippy::disallowed_methods,
    reason = "terminal directory validation runs on the labeled terminal blocking worker"
)]
fn checked_agent_cwd(
    agent: Option<studio_runtime_store::AgentRecord>,
) -> Result<(String, String, PathBuf), TerminalError> {
    let agent = agent.ok_or(TerminalError::InvalidRequest)?;
    if agent.is_deleted() {
        return Err(TerminalError::InvalidRequest);
    }
    let agent_cwd = agent
        .cwd
        .as_deref()
        .ok_or(TerminalError::InvalidRequest)?
        .to_owned();
    let canonical = CanonicalPath::existing(Path::new(&agent_cwd))
        .map_err(|_| TerminalError::InvalidRequest)?;
    if !canonical.is_dir() {
        return Err(TerminalError::InvalidRequest);
    }
    Ok((agent.id.0, agent_cwd, canonical.into_path_buf()))
}

#[cfg(unix)]
async fn terminal_shell() -> String {
    if let Some(path) = std::env::var_os("SHELL") {
        use std::os::unix::fs::PermissionsExt as _;
        let path = PathBuf::from(path);
        if tokio::fs::metadata(&path)
            .await
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        {
            return path.to_string_lossy().into_owned();
        }
    }
    "/bin/sh".to_owned()
}

fn terminal_body_error(error: TerminalError) -> ApiError {
    match error {
        TerminalError::InvalidRequest
        | TerminalError::UnknownTerminal
        | TerminalError::NotRunning
        | TerminalError::ReceiptConflict => {
            ApiError::validation(vec![Value::String("body".to_owned())])
        }
        other @ TerminalError::InvalidStoredObject(_)
        | other @ TerminalError::InvalidCursor
        | other @ TerminalError::InvalidHistoryRange
        | other @ TerminalError::StartupRecovery(_)
        | other @ TerminalError::BlockingTask
        | other @ TerminalError::Store(_)
        | other @ TerminalError::Session(_) => terminal_error(other),
    }
}

pub(super) fn terminal_error(error: TerminalError) -> ApiError {
    match error {
        TerminalError::InvalidRequest
        | TerminalError::InvalidCursor
        | TerminalError::InvalidHistoryRange
        | TerminalError::NotRunning => {
            ApiError::validation(vec![Value::String("query".to_owned())])
        }
        TerminalError::UnknownTerminal => ApiError::validation(vec![
            Value::String("query".to_owned()),
            Value::String("id".to_owned()),
        ]),
        TerminalError::InvalidStoredObject(id) => ApiError::invalid_stored("terminals", &id),
        TerminalError::Store(studio_runtime_store::StoreError::InvalidStoredObject {
            table,
            id,
        }) => ApiError::invalid_stored(table, &id),
        TerminalError::ReceiptConflict
        | TerminalError::StartupRecovery(_)
        | TerminalError::BlockingTask
        | TerminalError::Store(_)
        | TerminalError::Session(_) => ApiError::internal(),
    }
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "terminals",
        routes: Some(routes),
        dependencies: ROUTE_DEPENDENCIES,
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::disallowed_methods)]
mod tests {
    #[test]
    fn output_query_enforces_history_and_limit() {
        fn valid(query: &str) -> bool {
            let uri: axum::http::Uri = format!("/api/terminals/output?{query}").parse().unwrap();
            axum::extract::Query::<super::TerminalOutputQuery>::try_from_uri(&uri).is_ok()
        }
        assert!(valid("id=term&history=1&limit=5"));
        assert!(!valid("history=0"));
        assert!(!valid("limit=0"));
        assert!(!valid("extra=1"));
    }
    use std::sync::Arc;

    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use studio_runtime_store::{RuntimeStore, TerminalsRepository};
    use tower::ServiceExt;

    #[test]
    fn typed_create_preserves_receipt_identity_and_bounds() {
        for value in [
            json!({"id":"i", "agent":"a"}),
            json!({"id":"i", "agent":"a", "cols":100, "rows":28}),
        ] {
            let body: super::TerminalCreateBody = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                studio_runtime_schema::json_compat::python_json_serialized(&body).unwrap(),
                studio_runtime_schema::json_compat::python_json(&value)
            );
        }
        for value in [
            json!({"id":"i", "agent":"a", "cols":null}),
            json!({"id":"i", "agent":"a", "cols":1}),
            json!({"id":"i", "agent":"a", "extra":true}),
        ] {
            assert!(serde_json::from_value::<super::TerminalCreateBody>(value).is_err());
        }
        let doc: Value = serde_json::from_str(include_str!("../../openapi.json")).unwrap();
        let p = &doc["components"]["schemas"]["TerminalCreate"]["properties"];
        assert_eq!(
            p["id"]["maxLength"],
            super::TERMINAL_CREATE_REQUEST_ID_MAX_CODEPOINTS
        );
        assert_eq!(
            p["agent"]["maxLength"],
            super::TERMINAL_CREATE_AGENT_ID_MAX_CODEPOINTS
        );
        assert_eq!(p["cols"]["minimum"], super::TERMINAL_COLS_MIN);
        assert_eq!(p["cols"]["maximum"], super::TERMINAL_COLS_MAX);
        assert_eq!(p["cols"]["default"], super::TERMINAL_COLS_DEFAULT);
        assert_eq!(p["rows"]["minimum"], super::TERMINAL_ROWS_MIN);
        assert_eq!(p["rows"]["maximum"], super::TERMINAL_ROWS_MAX);
        assert_eq!(p["rows"]["default"], super::TERMINAL_ROWS_DEFAULT);
    }

    #[tokio::test]
    async fn pending_restart_cleanup_returns_retryable_error_without_terminal_writes() {
        let tmpfs = std::env::var_os("CS_TERMINAL_TEST_STATE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&tmpfs).expect("tmpfs state parent");
        let root = tempfile::Builder::new()
            .prefix("terminal-http-gate-")
            .tempdir_in(tmpfs)
            .expect("isolated terminal HTTP state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store"),
        );
        let before = TerminalsRepository::list(&store.test_reader().expect("read before request"))
            .expect("read terminal rows before request");
        let state = crate::router::HttpState::new(
            "token".into(),
            "workspace".into(),
            root.path().to_string_lossy().into_owned(),
            12345,
            None,
            Some(store.clone()),
            None,
        )
        .with_terminal_write_gate(Arc::new(|| Box::pin(async { false })));
        let app = crate::router::router(state).expect("HTTP router composes the terminal route");
        let body = json!({"id":"pending-recovery-create","agent":"agent"});
        let request = Request::builder()
            .method("POST")
            .uri("/api/terminals/create")
            .header("host", "127.0.0.1:12345")
            .header("content-type", "application/json")
            .header("x-canvas-token", "token")
            .header("x-canvas-workspace", "workspace")
            .header(
                crate::schema::API_SCHEMA_HASH_HEADER,
                crate::schema::api_schema_hash(),
            )
            .body(Body::from(body.to_string()))
            .expect("terminal request builds");
        let mut request = request;
        request
            .extensions_mut()
            .insert(ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                54321,
            ))));
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("terminal response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = to_bytes(response.into_body(), 4096)
            .await
            .expect("response body reads");
        let body: Value = serde_json::from_slice(&bytes).expect("error envelope parses");
        assert_eq!(
            body,
            json!({"error":"Terminal restart cleanup is still pending"})
        );
        for (path, body) in [
            (
                "input",
                json!({"id":"terminal", "text":"x", "request_id":"request"}),
            ),
            ("resize", json!({"id":"terminal", "cols":80, "rows":24})),
            ("rename", json!({"id":"terminal", "title":"Shell"})),
            ("close", json!({"id":"terminal"})),
        ] {
            let request = Request::builder()
                .method("POST")
                .uri(format!("/api/terminals/{path}"))
                .header("host", "127.0.0.1:12345")
                .header("content-type", "application/json")
                .header("x-canvas-token", "token")
                .header("x-canvas-workspace", "workspace")
                .header(
                    crate::schema::API_SCHEMA_HASH_HEADER,
                    crate::schema::api_schema_hash(),
                )
                .extension(ConnectInfo(std::net::SocketAddr::from((
                    [127, 0, 0, 1],
                    54321,
                ))))
                .body(Body::from(body.to_string()))
                .unwrap();
            assert_eq!(
                app.clone().oneshot(request).await.unwrap().status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{path}"
            );
        }
        for (path, body) in [
            ("create", json!({"id":"request", "agent":"agent", "cols":1})),
            (
                "input",
                json!({"id":"terminal", "text":"x", "request_id":""}),
            ),
            ("resize", json!({"id":"terminal", "cols":null})),
            ("rename", json!({"id":"terminal", "title":""})),
            ("close", json!({"id":"terminal", "extra":true})),
        ] {
            let request = Request::builder()
                .method("POST")
                .uri(format!("/api/terminals/{path}"))
                .header("host", "127.0.0.1:12345")
                .header("content-type", "application/json")
                .header("x-canvas-token", "token")
                .header(
                    crate::schema::API_SCHEMA_HASH_HEADER,
                    crate::schema::api_schema_hash(),
                )
                .extension(ConnectInfo(std::net::SocketAddr::from((
                    [127, 0, 0, 1],
                    54321,
                ))))
                .body(Body::from(body.to_string()))
                .unwrap();
            assert_eq!(
                app.clone().oneshot(request).await.unwrap().status(),
                StatusCode::BAD_REQUEST,
                "{path}"
            );
        }
        let after = TerminalsRepository::list(&store.test_reader().expect("read after request"))
            .expect("read terminal rows after request");
        assert_eq!(after.len(), before.len());
        assert_eq!(after, before);
    }
}
