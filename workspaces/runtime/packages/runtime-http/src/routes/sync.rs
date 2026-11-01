use axum::Json;
use axum::extract::State;

use crate::router::HttpState;

use super::manifest::{OperationStatus, RouteGroup, get as register_get};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![register_get(
        "/api/sync/identity",
        OperationStatus::Implemented,
        identity,
    )])
}

pub(crate) async fn identity(
    State(state): State<HttpState>,
) -> Json<crate::generated::SyncIdentityResponse> {
    Json(crate::generated::SyncIdentityResponse {
        workspace_id: state.workspace_id,
        sync_protocol: i64::from(
            studio_runtime_schema::sync_protocol::SYNC_IDENTITY_PROTOCOL_VERSION,
        ),
    })
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "sync",
        routes: Some(routes),
        dependencies: &[],
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, extract::ConnectInfo, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn identity_http_returns_workspace_and_protocol() {
        let state = crate::router::HttpState::new(
            "token".into(),
            "workspace".into(),
            String::new(),
            4620,
            None,
            None,
            None,
        );
        let response = crate::router(state)
            .expect("router")
            .oneshot(
                Request::builder()
                    .uri("/api/sync/identity")
                    .header("host", "127.0.0.1:4620")
                    .header("x-canvas-token", "token")
                    .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
                    .extension(ConnectInfo(std::net::SocketAddr::from((
                        [127, 0, 0, 1],
                        12345,
                    ))))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("JSON");
        assert_eq!(
            body,
            serde_json::json!({"workspaceId":"workspace", "syncProtocol":studio_runtime_schema::sync_protocol::SYNC_IDENTITY_PROTOCOL_VERSION})
        );
    }
}
