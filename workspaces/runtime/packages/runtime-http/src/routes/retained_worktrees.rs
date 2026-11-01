//! The worktrees that cleanup kept for workers of a team, including deleted workers.
use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use studio_runtime_store::{AgentId, AgentIndexedReads, AgentRepository};

use super::{
    extractors::TypedQuery,
    manifest::{OperationStatus, RouteGroup, get},
};
use crate::{error::ApiError, router::HttpState};

pub(crate) fn routes() -> RouteGroup {
    RouteGroup::new(vec![get(
        "/api/retained-worktrees",
        OperationStatus::Implemented,
        retained_worktrees,
    )])
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedQuery {
    agent: studio_runtime_schema::bounded_text::BoundedText<1, 200>,
}

async fn retained_worktrees(
    State(state): State<HttpState>,
    TypedQuery(query): TypedQuery<RetainedQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = query.agent.as_str().to_owned();
    let store = state.store.ok_or_else(ApiError::internal)?;
    let result = store
        .read_async("runtime-http/retained-worktrees", move |connection| {
            let root_id = AgentRepository::get(connection, &AgentId::from(id.as_str()))
                .map_err(ApiError::stored)?
                .map(|agent| agent.root_id.0)
                .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Agent not found"))?;
            // Cleanup ends after deletion. A deleted worker has no live sync row, so the
            // team view reads the stored records.
            let members =
                AgentIndexedReads::team_records(connection, &root_id).map_err(ApiError::stored)?;
            let retained_worktrees = members
                .iter()
                .filter_map(|(_, raw)| serde_json::from_str::<Value>(raw).ok())
                .filter_map(|member| {
                    let mut value =
                        studio_runtime_services::sync_views::retained_worktree_entity(&member)?;
                    value.as_object_mut()?.remove("rootId");
                    Some(value)
                })
                .collect::<Vec<_>>();
            Ok::<_, ApiError>(json!({"rootId": root_id, "retainedWorktrees": retained_worktrees}))
        })
        .await?;
    Ok(Json(result))
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "retained_worktrees",
        routes: Some(routes),
        dependencies: &[],
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
        assert!(accepts::<super::RetainedQuery>("agent=a"));
        assert!(!accepts::<super::RetainedQuery>("agent="));
        assert!(!accepts::<super::RetainedQuery>("agent=a&unknown=x"));
    }
}
