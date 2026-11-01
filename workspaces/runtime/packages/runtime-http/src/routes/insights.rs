//! HTTP adapters for analytics and signature-checked session-cost snapshots.

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;
use studio_runtime_services::analytics::{self, AnalyticsOptions};
use studio_runtime_services::costs::{self, CostsError};

use super::extractors::TypedQuery;
use super::manifest::{Dependency, OperationStatus, RouteGroup, get};
use crate::error::{ApiError, InternalErrorKind};
use crate::router::HttpState;

const SESSION_COST_DEPENDENCIES: [Dependency; 1] = [Dependency::new("costs/pricing-refresh")];

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get(
            "/api/analytics",
            OperationStatus::Implemented,
            analytics_read,
        ),
        get(
            "/api/session-cost",
            OperationStatus::Implemented,
            session_cost,
        ),
    ])
}

enum ReadBody {
    Json(Value),
    Export(Vec<u8>),
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionCostQuery {
    agent: Option<String>,
}
/// Analytics time in nonnegative finite seconds.
struct AnalyticsTime(f64);
impl<'de> serde::Deserialize<'de> for AnalyticsTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <f64 as serde::Deserialize>::deserialize(deserializer)?;
        if value.is_finite() && value >= 0.0 {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("invalid analytics time"))
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum AnalyticsScope {
    Agent,
    Team,
    All,
}
#[derive(serde::Deserialize)]
enum ExportFlag {
    #[serde(rename = "1")]
    One,
}
type AnalyticsLimit = studio_runtime_schema::bounded_integer::BoundedInteger<
    0,
    { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 },
>;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyticsQuery {
    agent: Option<String>,
    scope: Option<AnalyticsScope>,
    from_time: Option<AnalyticsTime>,
    to_time: Option<AnalyticsTime>,
    limit: Option<AnalyticsLimit>,
    offset: Option<AnalyticsLimit>,
    export: Option<ExportFlag>,
    tool: Option<String>,
    timing: Option<String>,
    view: Option<String>,
    detail: Option<String>,
    item: Option<String>,
    turn: Option<String>,
    thread: Option<String>,
    turns: Option<String>,
}

async fn analytics_read(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<AnalyticsQuery>,
) -> Result<Response, ApiError> {
    let options = AnalyticsOptions {
        agent: query.agent,
        scope: query.scope.map(|scope| {
            match scope {
                AnalyticsScope::Agent => "agent",
                AnalyticsScope::Team => "team",
                AnalyticsScope::All => "all",
            }
            .to_owned()
        }),
        from: query.from_time.map(|value| value.0),
        to: query.to_time.map(|value| value.0),
        limit: query.limit.map(|value| value.get()),
        offset: query.offset.map(|value| value.get()),
        export: query.export.is_some(),
        tool: query.tool,
        timing: query.timing,
        view: query.view,
        detail: query.detail,
        item: query.item,
        turn: query.turn,
        thread: query.thread,
        turns: query.turns,
    };
    let export = options.export;
    let timing = options.timing.as_deref() == Some("1") && !export;
    let started = Instant::now();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let result = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/routes/insights",
        move || {
            let value = analytics::read(&store, &options)?;
            if export {
                Ok(ReadBody::Export(serde_json::to_vec(&value)?))
            } else {
                Ok(ReadBody::Json(value))
            }
        },
    )
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(map_analytics_error)?;
    let mut response = match result {
        ReadBody::Json(value) => {
            Json(super::generated_json::<crate::generated::AnalyticsResponse>(value)?)
                .into_response()
        }
        ReadBody::Export(body) => {
            let mut response = Response::new(Body::from(body));
            for (name, value) in [
                (header::CONTENT_TYPE, "application/json; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"codex-studio-analytics.json\"",
                ),
                (header::CACHE_CONTROL, "no-store"),
                (HeaderName::from_static("x-content-type-options"), "nosniff"),
            ] {
                response
                    .headers_mut()
                    .insert(name, HeaderValue::from_static(value));
            }
            response
        }
    };
    if timing {
        let value = format!(
            "analytics-total;dur={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        if let Ok(value) = HeaderValue::from_str(&value) {
            response.headers_mut().insert("server-timing", value);
        }
    }
    Ok(response)
}

async fn session_cost(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<SessionCostQuery>,
) -> Result<Json<costs::SessionCost>, ApiError> {
    let agent = query.agent.unwrap_or_default();
    session_cost_for_agent(state, agent).await
}

pub(super) async fn session_cost_for_agent(
    state: HttpState,
    agent: String,
) -> Result<Json<costs::SessionCost>, ApiError> {
    if agent.is_empty() || !valid_agent_id(&agent) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Select a chat"));
    }
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let accounts = state
        .account_policy
        .as_ref()
        .map(|policy| {
            let snapshot = policy.accounts();
            snapshot
                .accounts
                .into_iter()
                .chain(snapshot.archived_accounts)
                .filter_map(|account| Some((account.id.as_str()?.to_owned(), account)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let value = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/routes/insights",
        move || costs::session_cost_with_account_profiles(&store, &agent, &accounts),
    )
    .await
    .map_err(|_| ApiError::internal_with_cause(InternalErrorKind::StoreConcurrency))?
    .map_err(map_costs_error)?;
    Ok(Json(value))
}

fn cost_dependency(dependency: &'static str) -> ApiError {
    debug_assert!(
        SESSION_COST_DEPENDENCIES
            .iter()
            .any(|candidate| candidate.name() == dependency)
    );
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        format!("Not implemented: requires {dependency}"),
    )
}

fn map_costs_error(error: CostsError) -> ApiError {
    match error {
        CostsError::Dependency(dependency) => cost_dependency(dependency),
        CostsError::Analytics(error) => map_analytics_error(error),
        CostsError::Store(error) => ApiError::stored(error),
        CostsError::Repository(_) | CostsError::Sqlite(_) => {
            ApiError::internal_with_cause(InternalErrorKind::StoreDatabase)
        }
        CostsError::Computation => {
            ApiError::internal_with_cause(InternalErrorKind::CostsComputation)
        }
        CostsError::InvalidSessionState | CostsError::AccountProfilesRequired => {
            ApiError::internal_with_cause(InternalErrorKind::CostsState)
        }
        CostsError::InvalidPricing => cost_dependency("costs/pricing-refresh"),
    }
}

fn map_analytics_error(error: analytics::AnalyticsError) -> ApiError {
    match error {
        analytics::AnalyticsError::InvalidRequest(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
        analytics::AnalyticsError::UnknownAgent => {
            ApiError::new(StatusCode::BAD_REQUEST, "Select an agent for analytics")
        }
        analytics::AnalyticsError::InvalidStoredObject(object) => {
            ApiError::invalid_stored(object, "<record>")
        }
        analytics::AnalyticsError::InvalidStoredRow { table, id } => {
            ApiError::invalid_stored(table, &id)
        }
        analytics::AnalyticsError::Read(_) | analytics::AnalyticsError::Sqlite(_) => {
            ApiError::internal_with_cause(InternalErrorKind::StoreDatabase)
        }
        analytics::AnalyticsError::Serialization(_) => {
            ApiError::internal_with_cause(InternalErrorKind::StoreData)
        }
    }
}

fn valid_agent_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "insights",
        routes: Some(routes),
        dependencies: &["costs/pricing-refresh"],
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, extract::ConnectInfo, http::Request};
    use serde_json::{Value, json};
    use std::{
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
    };
    use studio_runtime_store::{
        RuntimeStore, StoreError,
        analytics_capture::{AnalyticsCaptureRepository, CaptureTable},
    };
    use tower::ServiceExt;

    /// Persisted rows exactly as typed usage capture writes them: a response,
    /// a Claude query cumulative and a gap carry no legacy counter fields.
    #[tokio::test]
    async fn analytics_route_serves_typed_usage_rows_within_its_contract() {
        let root = tempfile::tempdir().expect("temporary state");
        let store = Arc::new(RuntimeStore::open(Some(root.path())).await.expect("store"));
        let tokens = json!({"cacheWriteInputTokens":25647,"cachedInputTokens":0,"inputTokens":25649,
            "outputTokens":19,"reasoningOutputTokens":0,"totalTokens":25668});
        let typed = json!({"cacheReadInput":0,"cacheWriteInput":25647,"output":19,"reasoningOutput":0,"uncachedInput":2});
        let base = |id: &str, at: f64| {
            json!({"id":id,"agentId":"typed","rootId":"typed","threadId":"thread",
            "turnId":"turn","accountKey":"claude","accountHistory":[],"at":at,"recordedAt":at,"source":"live",
            "timestampSource":"observed","provider":"claude","model":"claude-haiku-5-5","fingerprint":id,
            "last":{},"total":{},"delta":{}})
        };
        // Capture sets modelContextWindow for responses and cumulatives only.
        let mut response = base("typed-response", 1.0);
        response["counterDomain"] = json!("response");
        response["modelContextWindow"] = Value::Null;
        response["responseId"] = json!("msg-1");
        response["last"] = tokens.clone();
        response["delta"] = tokens.clone();
        response["studioUsage"] = json!({"kind":"response","evidence":"live","model":"claude-haiku-5-5",
            "responseId":"msg-1","turnId":"turn","tokens":typed});
        let mut cumulative = base("typed-cumulative", 2.0);
        cumulative["counterDomain"] = json!("claudeQuery");
        cumulative["reset"] = json!(false);
        cumulative["baselineMissing"] = json!(false);
        cumulative["modelContextWindow"] = json!(1_000_000);
        cumulative["total"] = tokens;
        cumulative["studioUsage"] = json!({"kind":"cumulative","scope":"claudeQuery","turnId":"turn",
            "modelContextWindow":1_000_000,"total":typed,"models":[{"model":"claude-haiku-5-5",
            "providerCostUsd":0.0051391,"tokens":typed}]});
        let mut gap = base("typed-gap", 3.0);
        gap["counterDomain"] = json!("gap");
        gap["responseId"] = json!("msg-2");
        gap["studioUsage"] = json!({"kind":"gap","responseId":"msg-2","turnId":"turn",
            "model":"claude-haiku-5-5","reason":"queryClosed"});
        store
            .analytics_transaction::<_, StoreError>(move |tx, _| {
                AnalyticsCaptureRepository::write(tx, CaptureTable::Agents, &json!({"id":"typed",
                    "rootId":"typed","provider":"claude","accountKey":"claude","threadId":"thread"}))?;
                for row in [response, cumulative, gap] {
                    AnalyticsCaptureRepository::write(tx, CaptureTable::Usage, &row)?;
                }
                Ok(())
            })
            .await
            .expect("typed analytics rows");
        let state = crate::router::HttpState::new(
            "token".into(),
            "fixture".into(),
            root.path().display().to_string(),
            49173,
            None,
            Some(store),
            None,
        );
        let app = crate::router::router(state).expect("router");
        let request = Request::builder()
            .uri("/api/analytics?agent=typed&scope=agent&limit=500")
            .header("host", "127.0.0.1:49173")
            .header("x-canvas-token", "token")
            .extension(ConnectInfo(SocketAddr::from((Ipv4Addr::LOCALHOST, 12345))))
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(request).await.expect("route");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .expect("bounded body");
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        // Only the response is a usage sample; the cumulative stays provisional
        // and the gap is unknown usage, not a zero sample.
        assert_eq!(value["summary"]["usageSamples"], json!(1), "{value}");
        assert_eq!(value["summary"]["tokens"]["totalTokens"], json!(25668.0));
        assert_eq!(value["summary"]["provisionalUsageSamples"], json!(1));
    }
}

#[cfg(test)]
mod typed_query_contract_tests {
    use super::*;
    #[test]
    fn sessioncostquery_decodes_and_rejects_invalid() {
        fn decode(query: &str) -> bool {
            let uri: axum::http::Uri = format!("/?{query}").parse().unwrap();
            axum::extract::Query::<SessionCostQuery>::try_from_uri(&uri).is_ok()
        }
        assert!(decode("agent=x"));
        assert!(!decode("extra=x"));
        assert!(!decode("extra=x"));
    }
}

#[cfg(test)]
mod analyticsquery_tests {
    #[test]
    fn query_contract() {
        fn decode(query: &str) -> bool {
            let uri: axum::http::Uri = format!("/?{query}").parse().unwrap();
            axum::extract::Query::<super::AnalyticsQuery>::try_from_uri(&uri).is_ok()
        }
        assert!(decode("limit=1&from_time=0"));
        assert!(!decode("limit=-1"));
        assert!(!decode("limit=9007199254740992"));
        assert!(!decode("from_time=-1"));
        assert!(!decode("from_time=NaN"));
        assert!(!decode("scope=invalid"));
        assert!(!decode("export=0"));
        assert!(!decode("extra=x"));
    }
}
