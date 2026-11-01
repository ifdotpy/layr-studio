//! The owner's list of agent work that ended because of a Studio error.

use axum::{Json, extract::State};

use super::{
    extractors::ApiJson,
    manifest::{OperationStatus, RouteGroup, get, post},
};
use crate::{error::ApiError, generated, router::HttpState};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get("/api/studio-errors", OperationStatus::Implemented, read),
        post(
            "/api/studio-errors/dismiss",
            OperationStatus::Implemented,
            dismiss,
        ),
    ])
}

fn now() -> Result<f64, ApiError> {
    studio_runtime_services::event_write::unix_time().map_err(|_| ApiError::internal())
}

async fn read(
    State(state): State<HttpState>,
) -> Result<Json<generated::StudioErrorsResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let now = now()?;
    let value = store
        .read_async("runtime-http/routes/studio_errors", move |tx| {
            studio_runtime_services::studio_errors::list(tx, now)
        })
        .await
        .map_err(ApiError::stored)?;
    Ok(Json(super::generated_json(value)?))
}

type ErrorId = studio_runtime_schema::bounded_text::BoundedText<1, { usize::MAX }>;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SingleDismiss {
    id: ErrorId,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MultipleDismiss {
    #[serde(deserialize_with = "dismiss_ids")]
    ids: Vec<ErrorId>,
}
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DismissRequest {
    Single(SingleDismiss),
    Multiple(MultipleDismiss),
}
fn dismiss_ids<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<ErrorId>, D::Error> {
    let ids = <Vec<ErrorId> as serde::Deserialize>::deserialize(d)?;
    if ids.is_empty()
        || ids.len() > studio_runtime_schema::shared_values::STUDIO_ERRORS_DISMISS_MAX_IDS
    {
        Err(serde::de::Error::custom("invalid error id count"))
    } else {
        Ok(ids)
    }
}

async fn dismiss(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<DismissRequest>,
) -> Result<Json<generated::StudioErrorsDismissResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let now = now()?;
    let ids: Vec<String> = match body {
        DismissRequest::Single(body) => vec![body.id.as_str().to_owned()],
        DismissRequest::Multiple(body) => body
            .ids
            .into_iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    };
    let value = store
        .transaction(move |tx| studio_runtime_services::studio_errors::dismiss_many(tx, &ids, now))
        .await
        .map_err(ApiError::stored)?;
    Ok(Json(super::generated_json(value)?))
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "studio_errors", routes: Some(routes), dependencies: &[],
    }
}

#[cfg(test)]
mod strict_dismiss_tests {
    #[test]
    fn dismiss_body_is_closed_and_bounded() {
        assert!(serde_json::from_str::<super::DismissRequest>(r#"{"id":"a"}"#).is_ok());
        assert!(serde_json::from_str::<super::DismissRequest>(r#"{"ids":["a"]}"#).is_ok());
        assert!(serde_json::from_str::<super::DismissRequest>(r#"{"ids":[]}"#).is_err());
        assert!(serde_json::from_str::<super::DismissRequest>(r#"{"id":"a","extra":1}"#).is_err());
        assert!(
            serde_json::from_str::<super::DismissRequest>(r#"{"id":"a","ids":["b"]}"#).is_err()
        );
    }
}
