//! Status-only HTTP service while the state is being opened and migrated.
use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use studio_runtime_store::migrations::MigrationProgress;

#[derive(Clone)]
pub struct StartupGate(Arc<Mutex<MigrationProgress>>);

impl Default for StartupGate {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(MigrationProgress {
            id: "startup".into(),
            done: 0,
            total: 0,
        })))
    }
}

impl StartupGate {
    pub fn update(&self, progress: MigrationProgress) {
        if let Ok(mut current) = self.0.lock() {
            *current = progress;
        }
    }
}

#[derive(Serialize)]
struct Status {
    status: &'static str,
    #[serde(flatten)]
    progress: MigrationProgress,
}

async fn health(State(gate): State<StartupGate>) -> Result<Json<Status>, StatusCode> {
    let progress = gate
        .0
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .clone();
    Ok(Json(Status {
        status: "migrating",
        progress,
    }))
}

pub async fn ready(
    axum::Extension(monitor): axum::Extension<tokio::task::AbortHandle>,
) -> Result<Json<&'static str>, StatusCode> {
    if monitor.is_finished() {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    } else {
        Ok(Json("ready"))
    }
}

pub fn router(gate: StartupGate) -> Router {
    Router::new()
        .route("/health", get(health))
        .fallback(|| async { (StatusCode::SERVICE_UNAVAILABLE, "migrating") })
        .with_state(gate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn monitor_runtime_failure_marks_health_unavailable() {
        let task = tokio::spawn(async { Err::<(), _>("fixture monitor store failure") });
        let health = Router::new().route(
            "/health",
            get(ready).layer(axum::Extension(task.abort_handle())),
        );
        let response = health
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        task.await.unwrap().unwrap_err();
        let response = health
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn startup_status_is_readable_and_commands_are_refused() {
        let gate = StartupGate::default();
        gate.update(MigrationProgress {
            id: "family:source".into(),
            done: 3,
            total: 10,
        });
        let status = router(gate.clone())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let body = axum::body::to_bytes(status.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            body.as_ref(),
            br#"{"status":"migrating","id":"family:source","done":3,"total":10}"#
        );
        let command = router(gate)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/commands")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(command.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
