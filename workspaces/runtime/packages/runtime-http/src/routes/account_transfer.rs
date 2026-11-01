//! Durable transfer requests, worker selection, and receipt replay.
use super::{
    extractors::ApiJson,
    manifest::{OperationStatus, RouteGroup, post},
};
use crate::{error::ApiError, generated, router::HttpState};
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::Value;
use studio_runtime_providers::ProviderKind;
use studio_runtime_services::{
    account_transfer::{Action, TransferError},
    account_transfer_driver,
};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        post(
            "/api/agents/account",
            OperationStatus::Implemented,
            selection,
        ),
        post(
            "/api/agents/account-transfer",
            OperationStatus::Implemented,
            transfer,
        ),
    ])
}
/// Maximum agent identity length for account transfer, in Unicode code points.
const TRANSFER_AGENT_MAX_CHARS: usize = 200;
/// Maximum transfer request identity length, in Unicode code points.
const TRANSFER_REQUEST_MAX_CHARS: usize = 200;
type TransferAgent = studio_runtime_schema::bounded_text::BoundedText<1, TRANSFER_AGENT_MAX_CHARS>;
type TransferAccount = studio_runtime_schema::account_key::AccountKey;
type TransferRequestId =
    studio_runtime_schema::bounded_text::BoundedText<1, TRANSFER_REQUEST_MAX_CHARS>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountSelectionRequest {
    id: TransferAgent,
    account_key: TransferAccount,
    cwd: Option<String>,
}

async fn selection(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<AccountSelectionRequest>,
) -> Result<Json<generated::AgentResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let accounts = crate::bounded_entities::accounts(&state)
        .await
        .map_err(|_| {
            error(TransferError::Stored {
                object: "account registry",
                pointer: "/".into(),
            })
        })?;
    let accounts_value = accounts.clone();
    let accounts = move || Ok::<_, TransferError>(accounts_value.clone());
    let value = studio_runtime_services::account_selection::select(
        &store,
        &accounts,
        body.id.as_str(),
        body.account_key.as_str(),
        body.cwd.as_deref(),
    )
    .await
    .map_err(error)?;
    state.event_wake.notify_one();
    state.continuation_wake.notify_one();
    super::generated_json::<generated::AgentResponse>(value).map(Json)
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Request {
    Action(TransferActionRequest),
    Start(TransferStartRequest),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferActionRequest {
    request_id: TransferRequestId,
    action: Action,
    id: Option<TransferAgent>,
    account_key: Option<TransferAccount>,
    scope: Option<generated::TransferScope>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferStartRequest {
    request_id: TransferRequestId,
    id: Option<TransferAgent>,
    account_key: Option<TransferAccount>,
    #[serde(default, deserialize_with = "present_scope")]
    scope: Option<generated::TransferScope>,
}
fn present_scope<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> Result<Option<generated::TransferScope>, D::Error> {
    generated::TransferScope::deserialize(input).map(Some)
}
async fn transfer(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<Request>,
) -> Result<Json<generated::TransferResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let (request_id, action, id, account_key, scope) = match body {
        Request::Action(body) => (
            body.request_id,
            Some(body.action),
            body.id,
            body.account_key,
            body.scope,
        ),
        Request::Start(body) => (body.request_id, None, body.id, body.account_key, body.scope),
    };
    let result = if let Some(action) = action {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ApiError::internal())?
            .as_secs_f64();
        account_transfer_driver::action(&store, request_id.as_str(), action, now)
            .await
            .map_err(error)?
    } else {
        let lead = id.as_ref().map(|value| value.as_str()).ok_or_else(|| {
            ApiError::validation(vec![
                Value::String("body".into()),
                Value::String("id".into()),
            ])
        })?;
        let target = account_key
            .as_ref()
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ApiError::validation(vec![
                    Value::String("body".into()),
                    Value::String("account_key".into()),
                ])
            })?;
        if let Some(saved) = account_transfer_driver::replay(
            &store,
            request_id.as_str(),
            lead,
            target,
            scope.as_ref().map_or("team", |value| value.as_str()),
        )
        .await
        .map_err(error)?
        {
            return super::generated_json::<generated::TransferResponse>(saved).map(Json);
        }
        let accounts = crate::bounded_entities::accounts(&state)
            .await
            .map_err(|_| ApiError::internal())?;
        if !accounts.known_account_keys.contains(target) {
            return Err(error(TransferError::Refused("Unknown account")));
        }
        if !accounts.accounts.iter().any(|account| {
            account.id.as_str() == Some(target)
                && matches!(
                    account.eligibility(),
                    studio_runtime_schema::agent_progress::AccountEligibility::Available
                )
        }) {
            return Err(error(TransferError::Refused(
                "This account is not available for transfer",
            )));
        }
        let target_provider = state
            .model_catalog
            .account_profile(target)
            .map(|(_, provider)| provider.key().to_owned())
            .or_else(|| {
                accounts
                    .accounts
                    .iter()
                    .find(|account| account.id.as_str() == Some(target))
                    .and_then(|account| account.provider.as_str().map(str::to_owned))
            })
            .or_else(|| (target == "default").then(|| ProviderKind::Codex.key().to_owned()))
            .ok_or_else(|| error(TransferError::Refused("Unknown account provider")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ApiError::internal())?
            .as_secs_f64();
        account_transfer_driver::request(
            &store,
            request_id.as_str(),
            lead,
            target,
            scope.as_ref().map_or("team", |value| value.as_str()),
            &target_provider,
            now,
        )
        .await
        .map_err(error)?
    };
    state.event_wake.notify_one();
    state.continuation_wake.notify_one();
    super::generated_json::<generated::TransferResponse>(result).map(Json)
}
fn error(error: TransferError) -> ApiError {
    let status = match error {
        TransferError::Expired => StatusCode::GONE,
        TransferError::Requires(_) => StatusCode::NOT_IMPLEMENTED,
        TransferError::Stored { object, pointer } => {
            return ApiError::invalid_stored(object, &pointer);
        }
        TransferError::Store(_) => return ApiError::internal(),
        TransferError::Refused(_) => StatusCode::BAD_REQUEST,
    };
    ApiError::new(status, error.to_string())
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "account_transfer", routes: Some(routes), dependencies: &[
            "account-transfer/usage-resume-cancellation",
            "account-transfer/provider-history-executor", "account-transfer/destination-settings",
            "store-bootstrap/account-transfer-schema",
        ],
    }
}

#[cfg(test)]
mod ingress_bounds_tests {
    use super::{AccountSelectionRequest, Request};

    #[test]
    fn transfer_body_bounds_are_enforced() {
        assert!(
            serde_json::from_str::<AccountSelectionRequest>(r#"{"id":"a","account_key":"b"}"#)
                .is_ok()
        );
        assert!(
            serde_json::from_str::<AccountSelectionRequest>(r#"{"id":"","account_key":"b"}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"request_id":""}"#).is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"request_id":"a","scope":"invalid"}"#).is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"request_id":"a","scope":"team"}"#).is_ok());
        assert!(serde_json::from_str::<Request>(r#"{"request_id":"a","action":"retry"}"#).is_ok());
        assert!(serde_json::from_str::<Request>(r#"{"request_id":"a","action":null}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"request_id":"a","scope":null}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"request_id":"a","unknown":1}"#).is_err());
        assert!(
            serde_json::from_str::<AccountSelectionRequest>(
                r#"{"id":"a","account_key":"b","unknown":1}"#
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod replay_tests {
    use axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::Request,
    };
    use serde_json::{Value, json};
    use std::sync::Arc;
    use studio_runtime_services::account_policy::AccountPolicyState;
    use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore};
    use tower::ServiceExt;

    #[tokio::test]
    async fn admission_uses_refreshed_driver_credentials_for_selection_and_transfer() {
        let root = tempfile::tempdir().unwrap();
        let signed_in = root.path().join("signed-in");
        let signed_out = root.path().join("signed-out");
        std::fs::create_dir_all(&signed_in).unwrap();
        std::fs::create_dir_all(&signed_out).unwrap();
        std::fs::write(
            signed_in.join("auth.json"),
            br#"{"OPENAI_API_KEY":"fixture"}"#,
        )
        .unwrap();
        let registry = root.path().join("accounts.json");
        std::fs::write(&registry, json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"signed-in","logins":{},"accounts":{
            "signed-in":{"id":"signed-in","provider":"codex","home":signed_in,"label":"In","source":"fixture","status":"signedOut"},
            "signed-out":{"id":"signed-out","provider":"codex","home":signed_out,"label":"Out","source":"fixture","status":"ready"}}}).to_string()).unwrap();
        let driver = Arc::new(
            studio_runtime_services::account_driver::AccountDriver::open(registry).unwrap(),
        );
        let stored = driver.snapshot().unwrap();
        let refreshed = driver.listing_snapshot().unwrap();
        for (key, before, after) in [
            ("signed-in", "signedOut", "ready"),
            ("signed-out", "ready", "signedOut"),
        ] {
            assert_eq!(
                stored
                    .accounts
                    .iter()
                    .find(|row| row.id.as_str() == Some(key))
                    .unwrap()
                    .status
                    .as_str(),
                Some(before)
            );
            assert_eq!(
                refreshed
                    .accounts
                    .iter()
                    .find(|row| row.id.as_str() == Some(key))
                    .unwrap()
                    .status
                    .as_str(),
                Some(after)
            );
        }
        let state_dir = root.path().join("state");
        std::fs::create_dir(&state_dir).unwrap();
        let store = Arc::new(RuntimeStore::open(Some(&state_dir)).await.unwrap());
        for id in ["select-in", "select-out", "move-in", "move-out"] {
            let agent = AgentRecord::from_value(json!({"compactions":0,"id":id,"rootId":id,"epoch":1,"status":"idle","isLead":true,"accountKey":"source","provider":"codex","serverId":"local","model":"gpt-6-astra","cwd":root.path()})).unwrap();
            store
                .transaction(move |tx| AgentRepository::put(tx, &agent))
                .await
                .unwrap();
        }
        let state = crate::HttpState::new(
            "token".into(),
            "fixture".into(),
            root.path().display().to_string(),
            49171,
            None,
            Some(store),
            None,
        )
        .with_account_driver(driver);
        let app = crate::router(state).unwrap();
        let mut results = Vec::new();
        for (route, id, account, expected) in [
            ("/api/agents/account", "select-in", "signed-in", 200),
            ("/api/agents/account", "select-out", "signed-out", 400),
            ("/api/agents/account-transfer", "move-in", "signed-in", 200),
            (
                "/api/agents/account-transfer",
                "move-out",
                "signed-out",
                400,
            ),
        ] {
            let request_id = if id == "move-out" {
                "5a300f72-4ea6-4d30-8a0e-9b2c13c4226c"
            } else {
                "5a300f72-4ea6-4d30-8a0e-9b2c13c4226b"
            };
            let body = json!({"id":id,"account_key":account,"request_id":request_id});
            let body = if route == "/api/agents/account" {
                json!({"id":id,"account_key":account})
            } else {
                body
            };
            let request = Request::builder()
                .method("POST")
                .uri(route)
                .header("host", "127.0.0.1:49171")
                .header("content-type", "application/json")
                .header("x-canvas-token", "token")
                .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
                .extension(ConnectInfo(
                    "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .body(Body::from(body.to_string()))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            let status = response.status();
            let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            if expected == 400 {
                assert!(
                    String::from_utf8_lossy(&bytes).contains("not available"),
                    "{route} {id}: {}",
                    String::from_utf8_lossy(&bytes)
                );
            }
            results.push((
                format!("{route} {id}: {}", String::from_utf8_lossy(&bytes)),
                status.as_u16(),
                expected,
            ));
        }
        assert!(
            results
                .iter()
                .all(|(_, actual, expected)| actual == expected),
            "{results:?}"
        );
    }

    #[tokio::test]
    async fn exact_transfer_retry_survives_destination_disconnect() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(root.path())).await.unwrap());
        let agent = AgentRecord::from_value(json!({"compactions":0,"id":"lead","rootId":"lead",
            "epoch":1,"status":"idle","isLead":true,"accountKey":"source","provider":"codex",
            "model":"gpt-6-astra","cwd":root.path()}))
        .unwrap();
        store
            .transaction(move |tx| AgentRepository::put(tx, &agent))
            .await
            .unwrap();
        let app = |disconnected, status| {
            let registry = json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,
                "defaultAccountKey":"target","logins":{},"accounts":{"target":{
                    "id":"target","provider":"codex","home":root.path(),"label":"Target",
                    "source":"fixture","status":status,"disconnected":disconnected}}});
            let policy =
                AccountPolicyState::from_startup(Some(&serde_json::to_vec(&registry).unwrap()), [])
                    .unwrap();
            crate::router(crate::HttpState::new(
                "token".into(),
                "fixture".into(),
                root.path().display().to_string(),
                49171,
                None,
                Some(store.clone()),
                Some(Arc::new(policy)),
            ))
            .unwrap()
        };
        let body = json!({"id":"lead","account_key":"target","request_id":"5a300f72-4ea6-4d30-8a0e-9b2c13c4226a"});
        let call = |app: axum::Router, body: Value| async move {
            let request = Request::builder()
                .method("POST")
                .uri("/api/agents/account-transfer")
                .header("host", "127.0.0.1:49171")
                .header("content-type", "application/json")
                .header("x-canvas-token", "token")
                .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
                .extension(ConnectInfo(
                    "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .body(Body::from(body.to_string()))
                .unwrap();
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let body = serde_json::from_slice::<Value>(
                &to_bytes(response.into_body(), 1024 * 1024).await.unwrap(),
            )
            .unwrap();
            (status, body)
        };
        let refused = call(app(false, "error"), body.clone()).await;
        assert_eq!(
            refused.0, 400,
            "unavailable account must not start a transfer"
        );
        let first = call(app(false, "ready"), body.clone()).await;
        assert_eq!(first.0, 200, "initial request: {:?}", first.1);
        let retry = call(app(true, "ready"), body.clone()).await;
        assert_eq!(
            retry, first,
            "exact retry returns the accepted result after disconnect"
        );
        let mut changed = body;
        changed["account_key"] = json!("another");
        let conflict = call(app(true, "ready"), changed).await;
        assert_eq!(conflict.0, 400);
        assert_eq!(
            conflict.1["error"],
            "This transfer id has different content"
        );
    }
}
