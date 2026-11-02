//! Web Push of sign-in alerts to the owner's devices (docs/web-push.md). A device of the
//! owner reads the server's public key, subscribes, and can stop the pushes of any device. The
//! subscription endpoint and keys stay on the server; the routes return only IDs and names.

use axum::{Json, extract::State, http::StatusCode};
use studio_runtime_services::web_push;

use super::{
    extractors::ApiJson,
    manifest::{OperationStatus, RouteGroup, get, post},
};
use crate::{error::ApiError, generated, router::HttpState};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get("/api/web-push", OperationStatus::Implemented, push_state),
        post(
            "/api/web-push/subscriptions",
            OperationStatus::Implemented,
            subscribe,
        ),
        post("/api/web-push/remove", OperationStatus::Implemented, remove),
    ])
}

fn push_error(error: web_push::WebPushError) -> ApiError {
    match error {
        web_push::WebPushError::Invalid(message) => ApiError::new(StatusCode::BAD_REQUEST, message),
        web_push::WebPushError::Crypto | web_push::WebPushError::Io(_) => ApiError::internal(),
    }
}

fn device(row: &web_push::Subscription) -> generated::WebPushSubscription {
    generated::WebPushSubscription {
        id: row.id.clone(),
        device: row.device.clone(),
        created: row.created,
        endpoint_fingerprint: studio_runtime_store::sha256_hex(row.endpoint.as_bytes()),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, web_push::WebPushError> + Send + 'static,
) -> Result<T, ApiError> {
    studio_runtime_process::blocking_pool::spawn_blocking("runtime-http/routes/web_push", work)
        .await
        .map_err(|_| ApiError::internal())?
        .map_err(push_error)
}

async fn push_state(
    State(state): State<HttpState>,
) -> Result<Json<generated::WebPushState>, ApiError> {
    let root = std::path::PathBuf::from(&state.state_dir);
    blocking(move || {
        let key = web_push::ServerKey::load_or_create(&root)?;
        let rows = web_push::Subscriptions::new(&root).list()?;
        Ok(generated::WebPushState {
            public_key: key.public_key(),
            ready: web_push::subject(&root).is_some(),
            subscriptions: rows.iter().map(device).collect(),
        })
    })
    .await
    .map(Json)
}

async fn subscribe(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<generated::WebPushSubscribeRequest>,
) -> Result<Json<generated::WebPushSubscription>, ApiError> {
    let root = std::path::PathBuf::from(&state.state_dir);
    blocking(move || {
        let row = web_push::Subscriptions::new(&root).add(
            web_push::NewSubscription {
                endpoint: body.endpoint.as_str().to_owned(),
                p256dh: body.p256dh.as_str().to_owned(),
                auth: body.auth.as_str().to_owned(),
                device: body.device.as_str().to_owned(),
            },
            false,
            web_push::now_seconds(),
        )?;
        Ok(device(&row))
    })
    .await
    .map(Json)
}

async fn remove(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<generated::WebPushRemoveRequest>,
) -> Result<Json<generated::WebPushRemoveResponse>, ApiError> {
    let root = std::path::PathBuf::from(&state.state_dir);
    blocking(move || {
        let removed = web_push::Subscriptions::new(&root).remove(body.id.as_str())?;
        Ok(generated::WebPushRemoveResponse { removed })
    })
    .await
    .map(Json)
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "web_push", routes: Some(routes), dependencies: &[],
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn voice_records_accepts_integral_decimal_after() {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |super::ApiJson(_): super::ApiJson<generated::VoiceRecordsRequest>| async {},
            ),
        );
        let response = app
            .oneshot(request(
                "/",
                Method::POST,
                Some(json!({"agent":"a","after":1.0})),
            ))
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn project_delete_accepts_integral_decimal_sequence() {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |super::ApiJson(_): super::ApiJson<generated::ProjectDeleteRequest>| async {},
            ),
        );
        let response = app.oneshot(request("/", Method::POST, Some(json!({
            "id":"p", "request_id":"r", "expected_workspace_id":"w", "expected_project_seq":1.0
        })))).await.unwrap();
        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn native_action_accepts_integral_decimal_epoch() {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |super::ApiJson(_): super::ApiJson<generated::NativeActionRequest>| async {},
            ),
        );
        let response = app
            .oneshot(request(
                "/",
                Method::POST,
                Some(json!({
                    "id":"a", "action":"compact", "context":{"epoch":1.0}
                })),
            ))
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    #[tokio::test]
    async fn mcp_reconnect_accepts_integral_decimal_epoch() {
        assert_mcp_integral_decimal("epoch").await;
    }

    #[tokio::test]
    async fn mcp_reconnect_accepts_integral_decimal_generation() {
        assert_mcp_integral_decimal("generation").await;
    }

    async fn assert_mcp_integral_decimal(field: &str) {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |super::ApiJson(_): super::ApiJson<generated::McpReconnectRequest>| async {},
            ),
        );
        let mut body = json!({"id":"a", "name":"n", "request_id":"01234567-89ab-cdef-0123-456789abcdef", "session":{"account":"a","thread":"t","connection":"c","epoch":1,"generation":1}});
        body["session"][field] = json!(1.0);
        let response = app
            .oneshot(request("/", Method::POST, Some(body)))
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    #[test]
    fn opted_in_ingress_matches_schema() {
        use studio_runtime_schema::http_ingress::Object;
        let document = crate::schema::openapi_document();
        for (name, fixture) in [
            ("IdRequest", json!({"id":"a"})),
            (
                "NativeActionRequest",
                json!({"id":"a","action":"compact","context":{"epoch":1}}),
            ),
            (
                "NativeActionRequest",
                json!({"id":"a","action":{"safety":"wait","turnId":"t"}}),
            ),
            (
                "McpReconnectRequest",
                json!({"id":"a","name":"n","request_id":"01234567-89ab-cdef-0123-456789abcdef","session":{"account":"a","thread":"t","connection":"c","epoch":1,"generation":1}}),
            ),
            (
                "ClaudeAddStartRequest",
                json!({"login_id":"01234567-89ab-cdef-0123-456789abcdef"}),
            ),
            (
                "ClaudeCancelRequest",
                json!({"login_id":"01234567-89ab-cdef-0123-456789abcdef"}),
            ),
            (
                "ClaudeCodeRequest",
                json!({"login_id":"01234567-89ab-cdef-0123-456789abcdef","code":"code"}),
            ),
            (
                "ClaudeStartRequest",
                json!({"login_id":"01234567-89ab-cdef-0123-456789abcdef","account_key":"account"}),
            ),
            (
                "ProjectDeleteRequest",
                json!({"id":"p","request_id":"r","expected_workspace_id":"w","expected_project_seq":1}),
            ),
            ("ProjectSecretsListRequest", json!({"projectId":"p"})),
            (
                "ProjectSecretSetRequest",
                json!({"projectId":"p","name":"TOKEN","value":"v","requestId":"r"}),
            ),
            (
                "ProjectSecretRemoveRequest",
                json!({"projectId":"p","name":"TOKEN","requestId":"r"}),
            ),
            ("VaultLoginRequest", json!({"profile":"p","requestId":"r"})),
            ("VaultPullRequest", json!({"projectId":"p","requestId":"r"})),
            ("VaultSetupFactsRequest", json!({"projectId":"p"})),
            ("MacSyncConfigRequest", json!({"projectId":"p"})),
            (
                "MacSyncResolveRequest",
                json!({"projectId":"p","path":"file","choice":"vm"}),
            ),
            (
                "VaultSetupRequest",
                json!({"projectId":"p","requestId":"r","prefix":"p","profile":"p","region":"us-east-1"}),
            ),
            (
                "VaultUnlockRequest",
                json!({"projectId":"p","words":"word"}),
            ),
            ("CheckpointCaptureRequest", json!({"agent":"a"})),
            (
                "CheckpointPreviewRequest",
                json!({"agent":"a","checkpoint":"c"}),
            ),
            (
                "CheckpointRestoreRequest",
                json!({"agent":"a","expectedTree":"t"}),
            ),
            ("VoiceEndRequest", json!({"agent":"a","session_id":"s"})),
            (
                "VoiceStartRequest",
                json!({"agent":"a","session_id":"s","sdp":"x"}),
            ),
            ("VoiceStatusRequest", json!({"agent":"a"})),
            ("VoiceRecordsRequest", json!({"agent":"a"})),
            (
                "WebPushSubscribeRequest",
                json!({"endpoint":"e","p256dh":"p","auth":"a","device":"d"}),
            ),
        ] {
            let schema = json!({"$ref":format!("#/components/schemas/{name}"), "components":document["components"]});
            let validator = jsonschema::options()
                .should_validate_formats(true)
                .with_pattern_options(jsonschema::PatternOptions::regex())
                .build(&schema)
                .unwrap();
            let mut cases = vec![fixture.clone()];
            for key in document["components"]["schemas"][name]["properties"]
                .as_object()
                .unwrap()
                .keys()
            {
                let mut missing = fixture.clone();
                missing.as_object_mut().unwrap().remove(key);
                cases.push(missing);
                for value in [
                    json!(null),
                    json!(42),
                    json!(-1),
                    json!(1.0),
                    json!(1.5),
                    json!(9007199254740991_u64),
                    json!(9007199254740992_u64),
                    json!(true),
                    json!(""),
                    json!("a"),
                    json!("a\n"),
                    json!(" "),
                    json!("01234567-89ab-cdef-0123-456789abcdef"),
                    json!("_"),
                    json!("A_B1"),
                    json!("us-east-1"),
                    json!("é"),
                    json!("x".repeat(43)),
                    json!("x".repeat(44)),
                    json!("x".repeat(99)),
                    json!("x".repeat(100)),
                    json!("x".repeat(101)),
                    json!("x".repeat(64)),
                    json!("x".repeat(65)),
                    json!("x".repeat(80)),
                    json!("x".repeat(81)),
                    json!("x".repeat(128)),
                    json!("x".repeat(129)),
                    json!("x".repeat(200)),
                    json!("x".repeat(201)),
                    json!("x".repeat(2048)),
                    json!("x".repeat(2049)),
                    json!("x".repeat(4096)),
                    json!("x".repeat(4097)),
                    json!("x".repeat(2000)),
                    json!("x".repeat(2001)),
                    json!("x".repeat(16384)),
                    json!("x".repeat(16385)),
                ] {
                    let mut body = fixture.clone();
                    body[key] = value;
                    cases.push(body);
                }
            }
            let mut extra = fixture.clone();
            extra["extra"] = json!(true);
            cases.push(extra);
            if name == "NativeActionRequest" {
                for value in [
                    json!(null),
                    json!(-1),
                    json!(1.0),
                    json!(1.5),
                    json!(9007199254740992_u64),
                ] {
                    let mut body = fixture.clone();
                    body["context"] = json!({"epoch":value});
                    cases.push(body);
                }
                for value in [
                    json!(""),
                    json!(42),
                    json!("x".repeat(200)),
                    json!("x".repeat(201)),
                ] {
                    let mut body = fixture.clone();
                    body["action"] = json!({"safety":"wait","turnId":value});
                    cases.push(body);
                }
            }
            if name == "McpReconnectRequest" {
                for key in ["account", "thread", "connection", "epoch", "generation"] {
                    let mut missing = fixture.clone();
                    missing["session"].as_object_mut().unwrap().remove(key);
                    cases.push(missing);
                    for value in [
                        json!(null),
                        json!(""),
                        json!(1.0),
                        json!(1.5),
                        json!(-1),
                        json!(9007199254740992_u64),
                    ] {
                        let mut body = fixture.clone();
                        body["session"][key] = value;
                        cases.push(body);
                    }
                }
                let mut extra = fixture.clone();
                extra["session"]["extra"] = json!(true);
                cases.push(extra);
            }
            for body in cases {
                let decoded = match name {
                    "NativeActionRequest" => serde_json::from_value::<
                        Object<generated::NativeActionRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ClaudeAddStartRequest" => serde_json::from_value::<
                        Object<generated::ClaudeAddStartRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ClaudeCancelRequest" => serde_json::from_value::<
                        Object<generated::ClaudeCancelRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ClaudeCodeRequest" => {
                        serde_json::from_value::<Object<generated::ClaudeCodeRequest>>(body.clone())
                            .is_ok()
                    }
                    "ClaudeStartRequest" => serde_json::from_value::<
                        Object<generated::ClaudeStartRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ProjectSecretsListRequest" => serde_json::from_value::<
                        Object<generated::ProjectSecretsListRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ProjectSecretSetRequest" => serde_json::from_value::<
                        Object<generated::ProjectSecretSetRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ProjectSecretRemoveRequest" => serde_json::from_value::<
                        Object<generated::ProjectSecretRemoveRequest>,
                    >(body.clone())
                    .is_ok(),
                    "VaultLoginRequest" => {
                        serde_json::from_value::<Object<generated::VaultLoginRequest>>(body.clone())
                            .is_ok()
                    }
                    "VaultPullRequest" => {
                        serde_json::from_value::<Object<generated::VaultPullRequest>>(body.clone())
                            .is_ok()
                    }
                    "VaultSetupFactsRequest" => serde_json::from_value::<
                        Object<generated::VaultSetupFactsRequest>,
                    >(body.clone())
                    .is_ok(),
                    "MacSyncConfigRequest" => serde_json::from_value::<
                        Object<generated::MacSyncConfigRequest>,
                    >(body.clone())
                    .is_ok(),
                    "MacSyncResolveRequest" => serde_json::from_value::<
                        Object<generated::MacSyncResolveRequest>,
                    >(body.clone())
                    .is_ok(),
                    "VaultSetupRequest" => {
                        serde_json::from_value::<Object<generated::VaultSetupRequest>>(body.clone())
                            .is_ok()
                    }
                    "VaultUnlockRequest" => serde_json::from_value::<
                        Object<generated::VaultUnlockRequest>,
                    >(body.clone())
                    .is_ok(),
                    "IdRequest" => {
                        serde_json::from_value::<Object<generated::IdRequest>>(body.clone()).is_ok()
                    }
                    "McpReconnectRequest" => serde_json::from_value::<
                        Object<generated::McpReconnectRequest>,
                    >(body.clone())
                    .is_ok(),
                    "ProjectDeleteRequest" => serde_json::from_value::<
                        Object<generated::ProjectDeleteRequest>,
                    >(body.clone())
                    .is_ok(),
                    "CheckpointCaptureRequest" => serde_json::from_value::<
                        Object<generated::CheckpointCaptureRequest>,
                    >(body.clone())
                    .is_ok(),
                    "VoiceRecordsRequest" => serde_json::from_value::<
                        Object<generated::VoiceRecordsRequest>,
                    >(body.clone())
                    .is_ok(),
                    "CheckpointPreviewRequest" => serde_json::from_value::<
                        Object<generated::CheckpointPreviewRequest>,
                    >(body.clone())
                    .is_ok(),
                    "CheckpointRestoreRequest" => serde_json::from_value::<
                        Object<generated::CheckpointRestoreRequest>,
                    >(body.clone())
                    .is_ok(),
                    "VoiceEndRequest" => {
                        serde_json::from_value::<Object<generated::VoiceEndRequest>>(body.clone())
                            .is_ok()
                    }
                    "VoiceStartRequest" => {
                        serde_json::from_value::<Object<generated::VoiceStartRequest>>(body.clone())
                            .is_ok()
                    }
                    "VoiceStatusRequest" => serde_json::from_value::<
                        Object<generated::VoiceStatusRequest>,
                    >(body.clone())
                    .is_ok(),
                    _ => serde_json::from_value::<Object<generated::WebPushSubscribeRequest>>(
                        body.clone(),
                    )
                    .is_ok(),
                };
                assert_eq!(decoded, validator.is_valid(&body), "{name}: {body}");
            }
        }
    }

    #[tokio::test]
    async fn remove_ingress_matches_openapi() {
        let document = crate::schema::openapi_document();
        let schema = json!({
            "$ref":"#/components/schemas/WebPushRemoveRequest",
            "components":document["components"]
        });
        let validator = jsonschema::options()
            .should_validate_formats(true)
            .with_pattern_options(jsonschema::PatternOptions::regex())
            .build(&schema)
            .unwrap();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(
                |super::ApiJson(_): super::ApiJson<generated::WebPushRemoveRequest>| async {},
            ),
        );
        for body in [
            json!({"id":"a"}),
            json!({"id":"é".repeat(64)}),
            json!({"id":""}),
            json!({"id":"é".repeat(65)}),
            json!({}),
            json!({"id":null}),
            json!({"id":42}),
            json!({"id":"a", "extra":true}),
        ] {
            let response = app
                .clone()
                .oneshot(request("/", Method::POST, Some(body.clone())))
                .await
                .unwrap();
            assert_eq!(
                response.status().is_success(),
                validator.is_valid(&body),
                "{body}"
            );
        }
    }

    /// The P-256 base point: a valid uncompressed point for a fixture subscription.
    const POINT: &str =
        "BGsX0fLhLEJH-Lzm5WOkQPJ3A32BLeszoPShOUXYmMKWT-NC4v4af5uO5-tKfA-eFivOM1drMV7Oy7ZAaDe_UfU";

    fn request(path: &str, method: Method, body: Option<Value>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:49171")
            .header("x-canvas-token", "fixture-token")
            .header(
                crate::schema::API_SCHEMA_HASH_HEADER,
                crate::schema::api_schema_hash(),
            )
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:12345"
                    .parse::<std::net::SocketAddr>()
                    .expect("fixture peer address parses"),
            ));
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        builder
            .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
            .expect("request builds")
    }

    async fn call(
        app: &axum::Router,
        path: &str,
        method: Method,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(request(path, method, body))
            .await
            .expect("response");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("body");
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn a_device_subscribes_is_listed_without_its_keys_and_can_be_removed() {
        let state_dir = tempfile::tempdir().expect("temporary state directory");
        let state = crate::router::HttpState::new(
            "fixture-token".into(),
            "fixture".into(),
            state_dir.path().to_string_lossy().into_owned(),
            49171,
            None,
            None,
            None,
        );
        let app = crate::router::router(state).expect("valid router");
        let (status, first) = call(&app, "/api/web-push", Method::GET, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["subscriptions"], json!([]));
        assert_eq!(first["ready"], false, "no https remote access origin");
        assert!(
            first["publicKey"]
                .as_str()
                .is_some_and(|key| key.len() == 87)
        );
        let subscription = json!({
            "endpoint": "https://web.push.apple.com/fixture", "p256dh": POINT,
            "auth": "AAECAwQFBgcICQoLDA0ODw", "device": "iPhone"
        });
        let (status, row) = call(
            &app,
            "/api/web-push/subscriptions",
            Method::POST,
            Some(subscription.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{row}");
        assert_eq!(row["device"], "iPhone");
        assert_eq!(
            row["endpointFingerprint"],
            "88ebf617997e2dc82d3ce799e52579feb916c1c5092aa8292d04f6559b4191fe"
        );
        let (_, listed) = call(&app, "/api/web-push", Method::GET, None).await;
        assert_eq!(listed["publicKey"], first["publicKey"]);
        assert_eq!(
            listed["subscriptions"][0]["endpointFingerprint"],
            "88ebf617997e2dc82d3ce799e52579feb916c1c5092aa8292d04f6559b4191fe"
        );
        let listed = listed["subscriptions"].to_string();
        assert!(listed.contains("iPhone"));
        assert!(
            !listed.contains("web.push.apple.com") && !listed.contains(POINT),
            "the endpoint and keys stay on the server"
        );
        let mut bad = subscription;
        bad["endpoint"] = json!("http://10.0.0.5/push");
        let (status, _) = call(&app, "/api/web-push/subscriptions", Method::POST, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (_, removed) = call(
            &app,
            "/api/web-push/remove",
            Method::POST,
            Some(json!({"id": row["id"]})),
        )
        .await;
        assert_eq!(removed, json!({"removed": true}));
        let (_, retried) = call(
            &app,
            "/api/web-push/remove",
            Method::POST,
            Some(json!({"id": row["id"]})),
        )
        .await;
        assert_eq!(retried, json!({"removed": false}));
        let (_, listed) = call(&app, "/api/web-push", Method::GET, None).await;
        assert_eq!(listed["subscriptions"], json!([]));
    }
}
