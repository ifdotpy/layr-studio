use axum::Json;
use axum::extract::State;

use crate::router::HttpState;

use super::manifest::{OperationStatus, RouteGroup, get as register_get};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![register_get(
        "/api/session",
        OperationStatus::Implemented,
        get,
    )])
}

pub(crate) async fn get(State(state): State<HttpState>) -> Json<crate::generated::SessionResponse> {
    Json(crate::generated::SessionResponse { token: state.token })
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "session",
        routes: Some(routes),
        dependencies: &[],
    }
}
