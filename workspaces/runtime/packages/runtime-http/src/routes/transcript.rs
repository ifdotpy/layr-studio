//! HTTP adapters for persisted transcript item reads.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::Value;
use studio_runtime_services::transcript::{self, TranscriptError};
use studio_runtime_services::transcript_runtime::{self, PageRequest};

use super::extractors::{QueryParams, TypedQuery};
use super::manifest::{Dependency, OperationStatus, RouteGroup, get};
use crate::error::ApiError;
use crate::router::HttpState;

/// Retain a newest-items baseline while preserving the existing history route's
/// exclusive before cursor. Rust owns the default item count; OpenAPI checks it.
pub(crate) fn window_snapshot(
    mut page: crate::generated::TranscriptPageResponse,
    tail: Option<i64>,
) -> crate::generated::TranscriptPageResponse {
    use studio_runtime_schema::shared_values::{
        SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS, SYNC_TRANSCRIPT_STREAM_MAX_ITEMS,
    };
    let tail = tail
        .unwrap_or(SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS as i64)
        .clamp(1, SYNC_TRANSCRIPT_STREAM_MAX_ITEMS as i64) as usize;
    if page.items.len() > tail {
        let start = page.items.len() - tail;
        let before = if page.items[start].role == "reasoning" {
            page.items[..start].iter().rev().find(|item| item.role != "reasoning").map(|item|
                format!("{}{}", studio_runtime_schema::shared_values::TRANSCRIPT_BEFORE_INCLUSIVE_CURSOR_PREFIX, item.id))
                .or_else(|| page.next_cursor.clone().flatten())
        } else {
            Some(page.items[start].id.clone())
        };
        page.items.drain(..start);
        page.truncated = true;
        page.next_cursor = Some(before);
    }
    page
}

const CANVAS_DEPENDENCIES: &[Dependency] = &[Dependency::new("transcript/workspace-state")];

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![
        get(
            "/api/transcript/page",
            OperationStatus::Implemented,
            transcript_page,
        ),
        get(
            "/api/transcript/item",
            OperationStatus::Implemented,
            transcript_item,
        ),
        get(
            "/api/transcript/blob",
            OperationStatus::Implemented,
            transcript_blob,
        ),
        get(
            "/api/transcript/search",
            OperationStatus::Implemented,
            transcript_search,
        ),
        get(
            "/api/search/item",
            OperationStatus::Implemented,
            search_item,
        ),
        get("/api/search", OperationStatus::Implemented, search),
        get(
            "/api/transcript",
            OperationStatus::Implemented,
            canvas_transcript,
        ),
    ])
}

/// Maximum length of a transcript agent ID, in Unicode code points.
const TRANSCRIPT_AGENT_ID_MAX_CHARS: usize = 4096;
/// Maximum length of a transcript cursor, in Unicode code points.
const TRANSCRIPT_CURSOR_MAX_CHARS: usize = 4096;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptPageQuery {
    id: Option<studio_runtime_schema::bounded_text::BoundedText<0, TRANSCRIPT_AGENT_ID_MAX_CHARS>>,
    before:
        Option<studio_runtime_schema::bounded_text::BoundedText<0, TRANSCRIPT_CURSOR_MAX_CHARS>>,
    around:
        Option<studio_runtime_schema::bounded_text::BoundedText<0, TRANSCRIPT_CURSOR_MAX_CHARS>>,
    after: Option<studio_runtime_schema::bounded_text::BoundedText<0, TRANSCRIPT_CURSOR_MAX_CHARS>>,
    limit: Option<
        studio_runtime_schema::bounded_integer::BoundedInteger<
            0,
            { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 },
        >,
    >,
}

async fn transcript_page(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<TranscriptPageQuery>,
) -> Result<Json<crate::generated::TranscriptPageResponse>, ApiError> {
    let request = PageRequest {
        before: query.before.as_ref().map(|value| value.as_str().to_owned()),
        around: query.around.as_ref().map(|value| value.as_str().to_owned()),
        after: query.after.as_ref().map(|value| value.as_str().to_owned()),
        limit: query.limit.map(|value| value.get()),
    };
    let agent = query
        .id
        .as_ref()
        .map_or("", |value| value.as_str())
        .to_owned();
    read_page(state, agent, request, false).await
}

async fn read_page(
    state: HttpState,
    agent: String,
    request: PageRequest,
    native_fallback: bool,
) -> Result<Json<crate::generated::TranscriptPageResponse>, ApiError> {
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let value = match transcript_runtime::page(&store, agent, request).await {
        Err(TranscriptError::UnknownMessage) if native_fallback => Err(
            TranscriptError::RequiresDependency(CANVAS_DEPENDENCIES[0].name()),
        ),
        result => result,
    }
    .map_err(map_transcript_error)?;
    Ok(Json(super::generated_json::<
        crate::generated::TranscriptPageResponse,
    >(value)?))
}

async fn transcript_item(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::TranscriptItemResponse>, ApiError> {
    let agent = query
        .typed_value("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let identity = query
        .typed_value("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let value = transcript::item(&store, agent, identity)
        .await
        .map_err(map_transcript_error)?;
    Ok(Json(super::generated_json::<
        crate::generated::TranscriptItemResponse,
    >(value)?))
}

/// Full text of a cut item, by the hash that the item page carries. The
/// client calls it when the user opens the full output, and caches by hash.
#[allow(clippy::wildcard_enum_match_arm)]
async fn transcript_blob(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::TextBlobResponse>, ApiError> {
    let hash = query
        .typed_value("hash")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let text = store
        .read_async("runtime-http/transcript_blob", {
            let hash = hash.clone();
            move |transaction| studio_runtime_store::text_blobs::read(transaction, &hash)
        })
        .await
        .map_err(|error| match error {
            studio_runtime_store::StoreError::InvalidStoredRecord { .. } => {
                ApiError::new(StatusCode::NOT_FOUND, "Text blob not found")
            }
            studio_runtime_store::StoreError::StateDirectory(io)
                if io.kind() == std::io::ErrorKind::NotFound =>
            {
                ApiError::new(StatusCode::NOT_FOUND, "Text blob not found")
            }
            other => ApiError::stored(other),
        })?;
    Ok(Json(super::generated_json::<
        crate::generated::TextBlobResponse,
    >(
        serde_json::json!({"hash": hash, "size": text.len(), "text": text}),
    )?))
}

async fn transcript_search(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::TranscriptSearchResponse>, ApiError> {
    let agent = query
        .typed_value("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let search = query
        .typed_value("q")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let limit = query
        .typed_value("limit")
        .and_then(Value::as_i64)
        .unwrap_or(100);
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let value = transcript::search(&store, agent, search, limit)
        .await
        .map_err(map_transcript_error)?;
    Ok(Json(super::generated_json::<
        crate::generated::TranscriptSearchResponse,
    >(value)?))
}

async fn search_item(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::SearchItemResponse>, ApiError> {
    let identity = query
        .typed_value("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let value = transcript::search_item(&store, identity)
        .await
        .map_err(map_transcript_error)?;
    Ok(Json(super::generated_json::<
        crate::generated::SearchItemResponse,
    >(value)?))
}

async fn search(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::SearchResponse>, ApiError> {
    let search = query
        .typed_value("q")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let limit = query
        .typed_value("limit")
        .and_then(Value::as_i64)
        .unwrap_or(50);
    let store = state.store.clone().ok_or_else(ApiError::internal)?;
    let value = transcript::search_work(&store, search, None, limit)
        .await
        .map_err(map_transcript_error)?;
    Ok(Json(super::generated_json::<
        crate::generated::SearchResponse,
    >(value)?))
}

async fn canvas_transcript(
    State(state): State<HttpState>,
    query: QueryParams,
) -> Result<Json<crate::generated::TranscriptPageResponse>, ApiError> {
    let agent = query
        .typed_value("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    read_page(state, agent, PageRequest::default(), true).await
}

pub(crate) fn map_transcript_error(error: TranscriptError) -> ApiError {
    match error {
        TranscriptError::DeletedAssetOwner => {
            ApiError::new(StatusCode::BAD_REQUEST, "This agent was deleted")
        }
        TranscriptError::UnknownAssetOwner => {
            ApiError::new(StatusCode::BAD_REQUEST, "Unknown managed agent")
        }
        TranscriptError::SnapshotChanged => ApiError::new(
            StatusCode::CONFLICT,
            "Search context changed; retry the read",
        ),
        TranscriptError::UnknownMessage => ApiError::new(
            StatusCode::BAD_REQUEST,
            "Unknown message in this conversation",
        ),
        TranscriptError::DeletedConversation => {
            ApiError::new(StatusCode::BAD_REQUEST, "This conversation was deleted")
        }
        TranscriptError::MissingContent => ApiError::new(
            StatusCode::BAD_REQUEST,
            "Full transcript content is unavailable",
        ),
        TranscriptError::SearchResultUnavailable => {
            ApiError::new(StatusCode::BAD_REQUEST, "This search result is unavailable")
        }
        TranscriptError::ChatUnavailable => {
            ApiError::new(StatusCode::BAD_REQUEST, "This chat is unavailable")
        }
        TranscriptError::HistoryCursor => {
            ApiError::new(StatusCode::BAD_REQUEST, "Use one history cursor")
        }
        TranscriptError::InvalidStoredState { table, id } => ApiError::invalid_stored(&table, &id),
        TranscriptError::InvalidSearch => {
            ApiError::new(StatusCode::BAD_REQUEST, "Supply a valid search query")
        }
        TranscriptError::RequiresDependency(dependency) => ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            format!("Not implemented: requires {dependency}"),
        ),
        TranscriptError::InvalidStoredObject { table, id } => ApiError::invalid_stored(table, &id),
        TranscriptError::Store => ApiError::internal(),
        TranscriptError::ReadOnlyRepair => ApiError::internal(),
    }
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "transcript",
        routes: Some(routes),
        dependencies: &["transcript/workspace-state"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transcript_window_reasoning_boundary_retains_a_resolvable_older_cursor() {
        let page = serde_json::from_value(serde_json::json!({"items":[
            {"id":"parent","role":"assistant","text":"Earlier"},
            {"id":"reasoning:synthetic","role":"reasoning","text":""},
            {"id":"newest","role":"assistant","text":"Latest"}
        ],"truncated":false}))
        .expect("page");
        let window = window_snapshot(page, Some(2));
        assert_eq!(window.items.len(), 2);
        assert_eq!(window.items[0].id, "reasoning:synthetic");
        assert_eq!(
            window.next_cursor.flatten(),
            Some(format!(
                "{}parent",
                studio_runtime_schema::shared_values::TRANSCRIPT_BEFORE_INCLUSIVE_CURSOR_PREFIX
            ))
        );
    }

    #[test]
    fn transcript_route_is_implemented_with_only_a_conditional_workspace_refusal() {
        let operations =
            super::super::manifest::operation_manifest().expect("registered operation manifest");
        let operation = operations
            .iter()
            .find(|operation| operation.key() == ("GET".to_owned(), "/api/transcript".to_owned()))
            .expect("transcript operation");
        assert_eq!(operation.status(), OperationStatus::Implemented);
    }
}
