//! HTTP adapters for persisted transcript item reads.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use studio_runtime_services::transcript::{self, TranscriptError};
use studio_runtime_services::transcript_runtime::{self, PageRequest};

use super::extractors::TypedQuery;
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
/// Maximum search query length, in Unicode code points.
const TRANSCRIPT_SEARCH_MAX_CHARS: usize = 500;
type QueryId = studio_runtime_schema::bounded_text::BoundedText<0, TRANSCRIPT_AGENT_ID_MAX_CHARS>;
type QueryLimit = studio_runtime_schema::bounded_integer::BoundedInteger<
    0,
    { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 },
>;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptItemQuery {
    id: Option<QueryId>,
    message_id: Option<QueryId>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptBlobQuery {
    hash: Option<QueryId>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptSearchQuery {
    id: Option<QueryId>,
    q: studio_runtime_schema::bounded_text::BoundedText<1, TRANSCRIPT_SEARCH_MAX_CHARS>,
    limit: Option<QueryLimit>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    q: studio_runtime_schema::bounded_text::BoundedText<1, TRANSCRIPT_SEARCH_MAX_CHARS>,
    limit: Option<QueryLimit>,
}

fn query_id(value: Option<QueryId>) -> String {
    value.map_or_else(String::new, |value| value.as_str().to_owned())
}

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
    TypedQuery(query): TypedQuery<TranscriptItemQuery>,
) -> Result<Json<crate::generated::TranscriptItemResponse>, ApiError> {
    let agent = query_id(query.id);
    let identity = query_id(query.message_id);
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
    TypedQuery(query): TypedQuery<TranscriptBlobQuery>,
) -> Result<Json<crate::generated::TextBlobResponse>, ApiError> {
    let hash = query_id(query.hash);
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
    Ok(Json(crate::generated::TextBlobResponse {
        hash,
        size: i64::try_from(text.len()).map_err(|_| ApiError::internal())?,
        text,
    }))
}

async fn transcript_search(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<TranscriptSearchQuery>,
) -> Result<Json<crate::generated::TranscriptSearchResponse>, ApiError> {
    let agent = query_id(query.id);
    let search = query.q.as_str().to_owned();
    let limit = query.limit.map_or(100, |value| value.get());
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
    TypedQuery(query): TypedQuery<TranscriptItemQuery>,
) -> Result<Json<crate::generated::SearchItemResponse>, ApiError> {
    let identity = query_id(query.id);
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
    TypedQuery(query): TypedQuery<SearchQuery>,
) -> Result<Json<crate::generated::SearchResponse>, ApiError> {
    let search = query.q.as_str().to_owned();
    let limit = query.limit.map_or(50, |value| value.get());
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
    TypedQuery(query): TypedQuery<TranscriptPageQuery>,
) -> Result<Json<crate::generated::TranscriptPageResponse>, ApiError> {
    let agent = query_id(query.id);
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
    #[tokio::test]
    async fn blob_http_returns_text_and_utf8_byte_length() {
        use axum::{body::Body, extract::ConnectInfo, http::Request};
        use tower::ServiceExt;
        let root = tempfile::tempdir().expect("state");
        let store = std::sync::Arc::new(
            studio_runtime_store::RuntimeStore::open(Some(root.path()))
                .await
                .expect("store"),
        );
        let text = "hello 世界";
        let (blob, bytes) = studio_runtime_store::text_blobs::encode_blob(text);
        let path = studio_runtime_store::text_blobs::layout(&root.path().join("blobs"), &blob.hash);
        tokio::fs::create_dir_all(path.parent().expect("parent"))
            .await
            .expect("blob directory");
        tokio::fs::write(path, bytes).await.expect("blob");
        let state = HttpState::new(
            "token".into(),
            "workspace".into(),
            root.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store),
            None,
        );
        let request = Request::builder()
            .uri(format!("/api/transcript/blob?hash={}", blob.hash))
            .header("host", "127.0.0.1:4620")
            .header("x-canvas-token", "token")
            .header(crate::API_SCHEMA_HASH_HEADER, crate::api_schema_hash())
            .extension(ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                12345,
            ))))
            .body(Body::empty())
            .expect("request");
        let response = crate::router(state)
            .expect("router")
            .oneshot(request)
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
        assert_eq!(
            value,
            serde_json::json!({"hash":blob.hash,"size":text.len(),"text":text})
        );
    }
    #[test]
    fn transcript_queries_reject_unknown_and_out_of_range_inputs() {
        fn decode<T: serde::de::DeserializeOwned>(query: &str) -> bool {
            let uri: axum::http::Uri = format!("/api/transcript/search?{query}").parse().unwrap();
            axum::extract::Query::<T>::try_from_uri(&uri).is_ok()
        }
        assert!(decode::<TranscriptSearchQuery>("id=agent&q=hello&limit=5"));
        assert!(!decode::<TranscriptSearchQuery>("q=hello&limit=-1"));
        assert!(!decode::<TranscriptSearchQuery>("q=hello&unknown=1"));
        assert!(!decode::<TranscriptSearchQuery>("id=agent"));
        assert!(decode::<TranscriptItemQuery>("id=agent&message_id=item"));
    }
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
