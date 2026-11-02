//! Differential witnesses for generated HTTP ingress contracts.
use crate::{generated, ingress_pattern::*, routes::extractors::ApiJson};
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde_json::{Value, json};
use tower::ServiceExt;

// Named validators do not expose format assertions or production pattern options.
#[allow(
    clippy::disallowed_methods,
    reason = "parity witnesses require production pattern options and format assertions for an inline schema"
)]
fn parity_validator(schema: &Value) -> jsonschema::Validator {
    jsonschema::options()
        .should_validate_formats(true)
        .with_pattern_options(jsonschema::PatternOptions::regex())
        .build(schema)
        .expect("parity schema compiles")
}

fn request(path: &str, method: Method, body: Option<Value>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .expect("request builds")
}

#[tokio::test]
async fn voice_records_accepts_integral_decimal_after() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|ApiJson(_): ApiJson<generated::VoiceRecordsRequest>| async {}),
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
        axum::routing::post(|ApiJson(_): ApiJson<generated::ProjectDeleteRequest>| async {}),
    );
    let response = app
        .oneshot(request(
            "/",
            Method::POST,
            Some(json!({
                "id":"p", "request_id":"r", "expected_workspace_id":"w", "expected_project_seq":1.0
            })),
        ))
        .await
        .unwrap();
    assert!(response.status().is_success());
}

#[tokio::test]
async fn native_action_accepts_integral_decimal_epoch() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|ApiJson(_): ApiJson<generated::NativeActionRequest>| async {}),
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
        axum::routing::post(|ApiJson(_): ApiJson<generated::McpReconnectRequest>| async {}),
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
        let validator = parity_validator(&schema);
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
                "NativeActionRequest" => {
                    serde_json::from_value::<Object<generated::NativeActionRequest>>(body.clone())
                        .is_ok()
                }
                "ClaudeAddStartRequest" => {
                    serde_json::from_value::<Object<generated::ClaudeAddStartRequest>>(body.clone())
                        .is_ok()
                }
                "ClaudeCancelRequest" => {
                    serde_json::from_value::<Object<generated::ClaudeCancelRequest>>(body.clone())
                        .is_ok()
                }
                "ClaudeCodeRequest" => {
                    serde_json::from_value::<Object<generated::ClaudeCodeRequest>>(body.clone())
                        .is_ok()
                }
                "ClaudeStartRequest" => {
                    serde_json::from_value::<Object<generated::ClaudeStartRequest>>(body.clone())
                        .is_ok()
                }
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
                "MacSyncConfigRequest" => {
                    serde_json::from_value::<Object<generated::MacSyncConfigRequest>>(body.clone())
                        .is_ok()
                }
                "MacSyncResolveRequest" => {
                    serde_json::from_value::<Object<generated::MacSyncResolveRequest>>(body.clone())
                        .is_ok()
                }
                "VaultSetupRequest" => {
                    serde_json::from_value::<Object<generated::VaultSetupRequest>>(body.clone())
                        .is_ok()
                }
                "VaultUnlockRequest" => {
                    serde_json::from_value::<Object<generated::VaultUnlockRequest>>(body.clone())
                        .is_ok()
                }
                "IdRequest" => {
                    serde_json::from_value::<Object<generated::IdRequest>>(body.clone()).is_ok()
                }
                "McpReconnectRequest" => {
                    serde_json::from_value::<Object<generated::McpReconnectRequest>>(body.clone())
                        .is_ok()
                }
                "ProjectDeleteRequest" => {
                    serde_json::from_value::<Object<generated::ProjectDeleteRequest>>(body.clone())
                        .is_ok()
                }
                "CheckpointCaptureRequest" => serde_json::from_value::<
                    Object<generated::CheckpointCaptureRequest>,
                >(body.clone())
                .is_ok(),
                "VoiceRecordsRequest" => {
                    serde_json::from_value::<Object<generated::VoiceRecordsRequest>>(body.clone())
                        .is_ok()
                }
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
                "VoiceStatusRequest" => {
                    serde_json::from_value::<Object<generated::VoiceStatusRequest>>(body.clone())
                        .is_ok()
                }
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
    let validator = parity_validator(&schema);
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|ApiJson(_): ApiJson<generated::WebPushRemoveRequest>| async {}),
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

#[test]
fn named_patterns_match_openapi() {
    let document = crate::schema::openapi_document();
    for (schema, field, baseline) in [
        ("ProjectSecretsListRequest", "projectId", "project"),
        ("ProjectSecretSetRequest", "name", "TOKEN"),
        ("ProjectSecretSetRequest", "requestId", "request"),
        ("VaultLoginRequest", "requestId", "request"),
        ("VaultLoginRequest", "profile", "profile"),
        ("VaultSetupRequest", "prefix", "prefix"),
        ("VaultSetupRequest", "region", "us-east-1"),
        ("AccountGroupRequest", "group", "team_1"),
        ("RenameServer", "label", "Server Name"),
        (
            "ClaudeCancelRequest",
            "login_id",
            "01234567-89ab-cdef-0123-456789abcdef",
        ),
        ("ClaudeAddStartRequest", "label", "label"),
    ] {
        let spec = document["components"]["schemas"][schema]["properties"][field].clone();
        let validator = parity_validator(&spec);
        let mut values = vec![
            baseline.to_owned(),
            String::new(),
            " ".into(),
            "é".into(),
            "\u{85}".into(),
            "\u{a0}".into(),
            "\u{200b}".into(),
            "\u{2028}".into(),
            "\u{feff}".into(),
            "a\n".into(),
            "x".repeat(44),
            "x".repeat(100),
            "x".repeat(128),
            "us-east-1".into(),
        ];
        for c in 0..=127u8 {
            let mut s = baseline.to_owned();
            s.replace_range(..1, &(c as char).to_string());
            values.push(s);
        }
        for value in values {
            let wire = json!(value);
            let ok = match (schema, field) {
                ("ProjectSecretsListRequest", _) => {
                    serde_json::from_value::<ProjectId>(wire.clone()).is_ok()
                }
                ("ProjectSecretSetRequest", "name") => {
                    serde_json::from_value::<SecretName>(wire.clone()).is_ok()
                }
                ("ProjectSecretSetRequest", _) => {
                    serde_json::from_value::<SecretRequestId>(wire.clone()).is_ok()
                }
                ("VaultLoginRequest", "requestId") => {
                    serde_json::from_value::<VaultRequestId>(wire.clone()).is_ok()
                }
                ("VaultLoginRequest", _) => {
                    serde_json::from_value::<AwsProfileName>(wire.clone()).is_ok()
                }
                ("VaultSetupRequest", "prefix") => serde_json::from_value::<
                    studio_runtime_schema::resource_ref::VaultPrefix,
                >(wire.clone())
                .is_ok(),
                ("VaultSetupRequest", _) => serde_json::from_value::<Region>(wire.clone()).is_ok(),
                ("AccountGroupRequest", _) => serde_json::from_value::<
                    Option<studio_runtime_schema::project_ingress::AccountGroup>,
                >(wire.clone())
                .is_ok(),
                ("RenameServer", _) => serde_json::from_value::<
                    BoundedPattern<studio_runtime_schema::multi_server_ingress::ServerLabel, 1, 80>,
                >(wire.clone())
                .is_ok(),
                ("ClaudeCancelRequest", _) => {
                    serde_json::from_value::<BoundedPattern<Uuid, 36, 36>>(wire.clone()).is_ok()
                }
                _ => serde_json::from_value::<Option<BoundedPattern<NonWhitespace, 1, 32>>>(
                    wire.clone(),
                )
                .is_ok(),
            };
            assert_eq!(ok, validator.is_valid(&wire), "{schema}.{field}: {wire:?}");
        }
    }
}
