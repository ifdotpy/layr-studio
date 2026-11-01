//! HTTP handlers for persisted work-item, plan and queue reads.

use super::extractors::{ApiJson, TypedQuery};
use super::manifest::{OperationStatus, RouteGroup, get, post};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use studio_runtime_schema::annotation_command::{
    AnnotationCommandResponse, AnnotationData, AnnotationEntityRow, AnnotationRowOperation,
};
use studio_runtime_schema::persistence::{RequestReferenceKind, RequestResultStatus};
use studio_runtime_services::event_write::{self, AnnotationWrite, EventWriteError};
use studio_runtime_services::work_policy::{self, WorkPolicyError};
use studio_runtime_store::{
    AgentId, AgentRecord, AgentRepository, Connection, WorkItemsRepository,
};

use crate::error::ApiError;
use crate::router::HttpState;
use crate::validation::Registry;

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get("/api/work", OperationStatus::Implemented, work),
        get("/api/queue", OperationStatus::Implemented, queue),
        post(
            "/api/queue",
            OperationStatus::Implemented,
            super::queue_write::queue_action,
        ),
        post("/api/annotation", OperationStatus::Implemented, annotation),
        post(
            "/api/organization",
            OperationStatus::Implemented,
            organization,
        ),
    ])
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkQuery {
    agent: Option<String>,
}

pub(crate) async fn work(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<WorkQuery>,
) -> Result<Json<Value>, ApiError> {
    let agent_id = query
        .agent
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Invalid request"))?;
    work_for_agent(state, agent_id).await.map(Json)
}

pub(crate) async fn work_for_agent(state: HttpState, agent_id: String) -> Result<Value, ApiError> {
    read_agent(state, agent_id, |connection, agent, _validators| {
        let records = WorkItemsRepository::work(connection).map_err(ApiError::stored)?;
        let mut list = work_policy::work_list(&agent.root_id.0, &records);
        let lead = AgentRepository::get(connection, &agent.root_id)?
            .ok_or_else(|| invalid_stored("runtime_agents", &agent.root_id.0))?;
        list["rootId"] = json!(agent.root_id.0);
        list["taskDefaultGate"] = lead
            .extra
            .get("taskDefaultGate")
            .cloned()
            .unwrap_or(Value::Null);
        let records_by_id: std::collections::BTreeMap<_, _> = records
            .iter()
            .map(|record| (record.id().0.as_str(), record))
            .collect();
        for item in list["items"].as_array_mut().expect("work list") {
            let id = item["id"].as_str().unwrap_or_default();
            let record = records_by_id
                .get(id)
                .ok_or_else(|| invalid_stored("runtime_work", id))?;
            item["submissionRefusal"] =
                studio_runtime_services::work_write::current_submission_refusal(
                    connection,
                    &record.value(),
                )
                .map_err(super::work_write::write_error)?;
        }
        list["tasks"] = list["items"].clone();
        Ok(list)
    })
    .await
}

pub(crate) async fn queue(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<WorkQuery>,
) -> Result<Json<Value>, ApiError> {
    let agent_id = query
        .agent
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Invalid request"))?;
    queue_for_agent(state, agent_id).await.map(Json)
}

pub(super) async fn queue_for_agent(state: HttpState, agent_id: String) -> Result<Value, ApiError> {
    read_agent(state, agent_id, |connection, agent, _validators| {
        let snapshot = super::queue_read::queue_snapshot(connection, agent)
            .map_err(super::queue_read::api_error)?;
        Ok(json!({
            "items": snapshot.items,
            "revision": snapshot.revision,
            "capabilities": {
                "reorder": studio_runtime_services::queue_write::QUEUE_REORDER_ENABLED,
                "receipts": studio_runtime_services::queue_write::QUEUE_RECEIPTS_ENABLED,
            },
        }))
    })
    .await
}

async fn read_agent<T, F>(state: HttpState, id: String, read: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &AgentRecord, &Registry) -> Result<T, ApiError> + Send + 'static,
{
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let validators = state.validators.clone();
    store
        .read_async("runtime-http/routes/work_items", move |connection| {
            let agent = AgentRepository::get(connection, &AgentId::from(id.as_str()))?
                .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Invalid request"))?;
            if !super::queue_read::is_live(&agent) {
                return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid request"));
            }
            read(connection, &agent, &validators)
        })
        .await
}

fn invalid_stored(object: &'static str, id: &str) -> ApiError {
    stored_error(WorkPolicyError::InvalidStoredObject {
        object,
        id: id.to_owned(),
    })
}

fn stored_error(error: WorkPolicyError) -> ApiError {
    match error {
        WorkPolicyError::InvalidStoredObject { object, id } => {
            ApiError::invalid_stored(object, &id)
        }
    }
}

const ANNOTATION_PATH_MAX_CODEPOINTS: usize = 4096;
const ANNOTATION_TEXT_MAX_CODEPOINTS: usize = 32000;
const ANNOTATION_TURN_ID_MAX_CODEPOINTS: usize = 200;
const ANNOTATION_LINE_MAX: u64 = studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AnnotationBody {
    #[serde(default)]
    agent: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    id: studio_runtime_schema::queue_command::Field<String>,
    path: studio_runtime_schema::bounded_text::BoundedText<0, ANNOTATION_PATH_MAX_CODEPOINTS>,
    line: studio_runtime_schema::bounded_integer::BoundedInteger<1, ANNOTATION_LINE_MAX>,
    text: studio_runtime_schema::bounded_text::BoundedText<1, ANNOTATION_TEXT_MAX_CODEPOINTS>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    turn_id: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::bounded_text::BoundedText<0, ANNOTATION_TURN_ID_MAX_CODEPOINTS>,
    >,
}

pub(crate) async fn annotation(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<AnnotationBody>,
) -> Result<Json<AnnotationCommandResponse>, ApiError> {
    let signature = studio_runtime_schema::json_compat::python_json_serialized(&body)
        .map(|canonical| studio_runtime_store::sha256_hex(canonical.as_bytes()))
        .map_err(|_| ApiError::internal())?;
    let input = AnnotationWrite {
        agent_id: body
            .agent
            .value()
            .cloned()
            .unwrap_or_else(|| state.workspace_id.clone()),
        id: body.id.value().cloned(),
        path: body.path.as_str().to_owned(),
        line: body.line.get(),
        text: body.text.as_str().to_owned(),
        turn_id: body.turn_id.value().map(|v| v.as_str().to_owned()),
        signature,
    };
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let wake = state.event_wake.clone();
    let result = store
        .transaction_with_error(|connection| event_write::annotate(connection, input))
        .await
        .map_err(write_error)?;
    if result.event_inserted {
        wake.notify_one();
    }
    let reference = result.reference;
    let id = reference.id.clone();
    let rows = store.read_async("runtime-http/annotation-response", move |connection| {
        #[derive(serde::Deserialize)]
        struct Payload {
            collection: RequestReferenceKind,
            id: String,
            value: AnnotationData,
        }
        let row: Option<(i64, String)> = connection.query_row(
            "SELECT seq,payload FROM sync_entities WHERE collection='annotation' AND id=?1 AND deleted=0",
            [&id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        let Some((seq, payload)) = row else { return Ok::<_, studio_runtime_store::StoreError>(Vec::new()) };
        let payload: Payload = serde_json::from_str(&payload)?;
        if payload.collection != RequestReferenceKind::Annotation || payload.id != id || payload.value.id != id {
            return Err(studio_runtime_store::StoreError::InvalidStoredObject { table: "sync_entities", id });
        }
        Ok(vec![AnnotationEntityRow { collection: payload.collection, id: payload.id, op: AnnotationRowOperation::Upsert, seq, value: payload.value }])
    }).await.map_err(ApiError::stored)?;
    Ok(Json(AnnotationCommandResponse {
        status: RequestResultStatus::Applied,
        ref_kind: reference.kind,
        ref_id: reference.id,
        rev: reference.revision,
        rows,
    }))
}
pub(crate) async fn organization(
    State(state): State<HttpState>,
    ApiJson(body): ApiJson<studio_runtime_services::organization_effects::OrganizationRequest>,
) -> Result<Json<Value>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let result = store
        .transaction_with_error(move |tx| {
            studio_runtime_services::organization_effects::apply(tx, &body)
        })
        .await
        .map_err(write_error)?;
    Ok(Json(result))
}

pub(crate) fn work_request_signature<T: serde::Serialize>(
    agent: &str,
    actor: Option<&str>,
    body: &T,
) -> Result<String, ApiError> {
    let canonical = serde_json::to_vec(&(agent, actor, body)).map_err(ApiError::json)?;
    Ok(studio_runtime_store::sha256_hex(&canonical))
}

pub(crate) fn write_error(error: EventWriteError) -> ApiError {
    match error {
        EventWriteError::RequestResult(error) => error.into(),
        EventWriteError::InvalidResultSchema(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
        EventWriteError::RemoteEventsUnavailable => ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "Not implemented: requires cross-server/remote-events",
        ),
        EventWriteError::WorkerAssignmentUnavailable => ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "Not implemented: requires worker-driver/work-assignment",
        ),
        EventWriteError::InvalidRequest(message) | EventWriteError::Conflict(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
        EventWriteError::InvalidStoredObject { object, id } => invalid_stored(object, &id),
        EventWriteError::Store(error) => ApiError::stored(error),
        EventWriteError::Clock => ApiError::internal(),
    }
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "work_items",
        routes: Some(routes),
        dependencies: &["event-driver/work-items"],
    }
}

#[cfg(test)]
mod annotation_body_tests {
    use super::AnnotationBody;
    use serde_json::{Value, json};

    #[test]
    fn annotation_limits_match_openapi() {
        let doc: Value = serde_json::from_str(include_str!("../../openapi.json")).unwrap();
        let properties = &doc["components"]["schemas"]["AnnotationBody"]["properties"];
        assert_eq!(
            properties["path"]["maxLength"],
            super::ANNOTATION_PATH_MAX_CODEPOINTS
        );
        assert_eq!(
            properties["text"]["maxLength"],
            super::ANNOTATION_TEXT_MAX_CODEPOINTS
        );
        assert_eq!(
            properties["turnId"]["anyOf"][0]["maxLength"],
            super::ANNOTATION_TURN_ID_MAX_CODEPOINTS
        );
        assert_eq!(properties["line"]["maximum"], super::ANNOTATION_LINE_MAX);
    }

    #[test]
    fn annotation_identity_preserves_nullable_field_presence() {
        for mut value in [
            json!({"path":"é", "line":1, "text":"hello"}),
            json!({"agent":null, "id":null, "path":"p", "line":2, "text":"x", "turnId":null}),
            json!({"agent":"a", "id":"i", "path":"p", "line":3, "text":"x", "turnId":"t"}),
        ] {
            let body: AnnotationBody = serde_json::from_value(value.clone()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .entry("agent")
                .or_insert(Value::Null);
            assert_eq!(
                studio_runtime_schema::json_compat::python_json_serialized(&body).unwrap(),
                studio_runtime_schema::json_compat::python_json(&value)
            );
        }
    }

    #[test]
    fn annotation_body_enforces_schema_bounds_and_unknown_fields() {
        for value in [
            json!({"path":"p", "line":0, "text":"x"}),
            json!({"path":"p", "line":1, "text":""}),
            json!({"path":"p", "line":1, "text":"x", "extra":true}),
        ] {
            assert!(serde_json::from_value::<AnnotationBody>(value).is_err());
        }
    }
}

#[cfg(test)]
mod typed_query_tests {
    use axum::extract::Query;
    use serde::de::DeserializeOwned;

    fn accepts<T: DeserializeOwned>(query: &str) -> bool {
        let uri = format!("/?{query}").parse().unwrap();
        Query::<T>::try_from_uri(&uri).is_ok()
    }

    #[test]
    fn query_contracts() {
        assert!(accepts::<super::WorkQuery>("agent=a"));
        assert!(!accepts::<super::WorkQuery>("extra=x"));
        assert!(!accepts::<super::WorkQuery>("agent=a&unknown=x"));
    }
}
