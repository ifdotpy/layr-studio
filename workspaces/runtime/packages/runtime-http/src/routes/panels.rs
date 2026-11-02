//! Profile operations.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use studio_runtime_schema::http_bounded_ingress as bounds;
use studio_runtime_services::panels::{self, ProfileField};
use studio_runtime_store::{ProfileRepository, ProfileRole, ReasoningEffort, StoreError};

use super::extractors::ApiJson;
use super::manifest::{OperationStatus, RouteGroup, get, post};
use crate::error::ApiError;
use crate::router::HttpState;

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get("/api/profiles", OperationStatus::Implemented, profiles),
        post(
            "/api/profiles",
            OperationStatus::Implemented,
            update_profiles,
        ),
    ])
}

async fn profiles(
    State(state): State<HttpState>,
) -> Result<Json<crate::generated::ProfileList>, ApiError> {
    let store = state.store.ok_or_else(ApiError::internal)?;
    let records = store
        .read_async("runtime-http/routes/panels", |connection| {
            ProfileRepository::list(connection).map_err(profile_read_error)
        })
        .await?;
    let profiles = json!({"profiles": records});
    Ok(Json(
        super::generated_json::<crate::generated::ProfileList>(profiles)?,
    ))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileInput {
    #[serde(default, deserialize_with = "present")]
    action: Option<crate::generated::ProfileBodyAction>,
    effort: Option<crate::generated::ProfileBodyEffort>,
    id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    instructions: Option<
        studio_runtime_schema::bounded_text::BoundedText<
            0,
            { bounds::PROFILE_INSTRUCTIONS_MAX_CHARS },
        >,
    >,
    #[serde(rename = "isNew")]
    _is_new: Option<bool>,
    model: Option<
        studio_runtime_schema::bounded_text::BoundedText<0, { bounds::PROFILE_MODEL_MAX_CHARS }>,
    >,
    name: Option<
        studio_runtime_schema::bounded_text::BoundedText<0, { bounds::PROFILE_NAME_MAX_CHARS }>,
    >,
    #[serde(default, deserialize_with = "present")]
    role: Option<crate::generated::ProfileBodyRole>,
}
fn present<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

async fn update_profiles(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<ProfileInput>,
) -> Result<Json<crate::generated::ProfileMutation>, ApiError> {
    let action = body.action.unwrap_or_default();
    let requested_id = body.id.unwrap_or_default();
    let store = state.store.ok_or_else(ApiError::internal)?;
    let id = if requested_id.is_empty() {
        let id_store = store.clone();
        id_store
            .read_async("runtime-http/routes/panels", move |connection| {
                ProfileRepository::new_id(connection).map_err(|_| ApiError::internal())
            })
            .await?
    } else {
        requested_id
    };
    let result = if action == crate::generated::ProfileBodyAction::Delete {
        let deleted_id = id.clone();
        store
            .transaction_with(move |transaction| ProfileRepository::delete(transaction, &id))
            .await
            .map_err(|_| ApiError::internal())?;
        json!({"deleted": deleted_id})
    } else {
        let name = body.name.ok_or_else(|| validation_error("name"))?;
        let model = body.model.ok_or_else(|| validation_error("model"))?;
        let role = match body.role.unwrap_or_default() {
            crate::generated::ProfileBodyRole::Reviewer => ProfileRole::Reviewer,
            crate::generated::ProfileBodyRole::Implementer => ProfileRole::Implementer,
        };
        let effort = body.effort.map(|effort| match effort {
            crate::generated::ProfileBodyEffort::Low => ReasoningEffort::Low,
            crate::generated::ProfileBodyEffort::Medium => ReasoningEffort::Medium,
            crate::generated::ProfileBodyEffort::High => ReasoningEffort::High,
            crate::generated::ProfileBodyEffort::Xhigh => ReasoningEffort::Xhigh,
            crate::generated::ProfileBodyEffort::Max => ReasoningEffort::Max,
            crate::generated::ProfileBodyEffort::Ultra => ReasoningEffort::Ultra,
        });
        let profile = panels::make_profile(
            id,
            name.as_str(),
            role,
            model.as_str(),
            effort,
            body.instructions
                .as_ref()
                .map_or("", |value| value.as_str()),
        )
        .map_err(profile_field_error)?;
        let response = serde_json::to_value(&profile).map_err(|_| ApiError::internal())?;
        store
            .transaction_with(move |transaction| ProfileRepository::save(transaction, &profile))
            .await
            .map_err(profile_read_error)?;
        response
    };
    Ok(Json(super::generated_json::<
        crate::generated::ProfileMutation,
    >(result)?))
}

fn profile_field_error(field: ProfileField) -> ApiError {
    validation_error(match field {
        ProfileField::Name => "name",
        ProfileField::Model => "model",
    })
}

fn validation_error(field: &'static str) -> ApiError {
    ApiError::validation(vec![
        Value::String("body".to_owned()),
        Value::String(field.to_owned()),
    ])
}

fn profile_read_error(error: StoreError) -> ApiError {
    match error {
        StoreError::InvalidStoredObject { table, id } => {
            let object = table.strip_prefix("runtime_").unwrap_or(table);
            ApiError::invalid_stored(object, &id)
        }
        StoreError::SchemaMismatch(message)
            if message.starts_with("Invalid stored object profiles/") =>
        {
            let id = message
                .strip_prefix("Invalid stored object profiles/")
                .unwrap_or(&message);
            ApiError::invalid_stored("profiles", id)
        }
        StoreError::Sqlite(error)
            if error
                .to_string()
                .contains("no such table: runtime_profiles") =>
        {
            ApiError::invalid_stored("runtime_profiles", "<schema>")
        }
        StoreError::RequestResult(error) => error.into(),
        StoreError::Outbox(error) => error.into(),
        error @ (StoreError::TranscriptFullTextUnavailable { .. }
        | StoreError::TranscriptInputsIncomplete { .. }) => ApiError::stored(error),
        StoreError::StateDirectory(_)
        | StoreError::AlreadyOwned
        | StoreError::IntermediateReleaseRequired
        | StoreError::MissingDatabase(_)
        | StoreError::Migration { .. }
        | StoreError::UnsafeStateDirectory
        | StoreError::SchemaMismatch(_)
        | StoreError::InvalidSyncObject { .. }
        | StoreError::InvalidSyncObjectField { .. }
        | StoreError::InvalidStoredRecord { .. }
        | StoreError::SchemaObjectMismatch { .. }
        | StoreError::Sqlite(_)
        | StoreError::CommitOutcomeUnknown(_)
        | StoreError::Json(_)
        | StoreError::SchemaValidation(_)
        | StoreError::RecordContract(_)
        | StoreError::WriterPoisoned
        | StoreError::WriterActorInitialization(_)
        | StoreError::WriterActorControl(_)
        | StoreError::CommitActorOutcomeUnknown(_)
        | StoreError::ReadTaskFailed => ApiError::internal(),
    }
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "panels",
        routes: Some(routes),
        dependencies: &[],
    }
}

#[cfg(test)]
mod strict_profile_tests {
    #[tokio::test]
    async fn profile_http_accepts_omitted_and_rejects_null_fields() {
        use axum::{body::Body, extract::ConnectInfo, http::Request};
        use tower::ServiceExt;
        let root = tempfile::tempdir().expect("state");
        let store = std::sync::Arc::new(
            studio_runtime_store::RuntimeStore::open(Some(root.path()))
                .await
                .expect("store"),
        );
        let state = crate::router::HttpState::new(
            "token".into(),
            "workspace".into(),
            root.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store),
            None,
        );
        let app = crate::router(state).expect("router");
        for (body, expected) in [
            (r#"{"name":"Profile","model":"model"}"#, 200),
            (r#"{"name":"Profile","model":"model","role":null}"#, 400),
        ] {
            let request = Request::builder()
                .method("POST")
                .uri("/api/profiles")
                .header("host", "127.0.0.1:4620")
                .header("x-canvas-token", "token")
                .header("content-type", "application/json")
                .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
                .extension(ConnectInfo(std::net::SocketAddr::from((
                    [127, 0, 0, 1],
                    12345,
                ))))
                .body(Body::from(body))
                .expect("request");
            assert_eq!(
                app.clone()
                    .oneshot(request)
                    .await
                    .expect("response")
                    .status()
                    .as_u16(),
                expected
            );
        }
    }

    #[test]
    fn profile_input_is_closed_and_bounded() {
        assert!(serde_json::from_str::<super::ProfileInput>(r#"{"name":"n","model":"m"}"#).is_ok());
        assert!(serde_json::from_str::<super::ProfileInput>(r#"{"action":null}"#).is_err());
        assert!(serde_json::from_str::<super::ProfileInput>(r#"{"role":null}"#).is_err());
        assert!(serde_json::from_str::<super::ProfileInput>(r#"{"instructions":null}"#).is_err());
        assert!(
            serde_json::from_str::<super::ProfileInput>(
                r#"{"action":"delete","role":"reviewer","instructions":""}"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<super::ProfileInput>(&format!(
                r#"{{"name":"{}"}}"#,
                "a".repeat(101)
            ))
            .is_err()
        );
        assert!(serde_json::from_str::<super::ProfileInput>(r#"{"extra":1}"#).is_err());
    }
}
