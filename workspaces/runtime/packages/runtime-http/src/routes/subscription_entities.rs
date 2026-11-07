//! Entity view delivery. OpenAPI resource extensions own every collection and
//! identity filter; publication remains independent in runtime-services.

use crate::error::ApiError;
use crate::generated::{ResourceRef, ResourceRevisionEntry, SubscriptionCursor};
use crate::router::HttpState;
use studio_json::json;
use studio_runtime_schema::shared_values::{
    SYNC_ENTITY_STREAM_MAX_DOCUMENTS, SYNC_STREAM_EVENT_MAX_BYTES,
};
use studio_runtime_schema::sync_protocol::{SubscriptionEntityScope, subscription_entity_scopes};
use studio_runtime_store::EntityPage;

/// OpenAPI owns collections; typed selectors keep payload probing out of delivery.
pub(super) fn selection(
    resource: &ResourceRef,
) -> Option<(&'static SubscriptionEntityScope, Option<&str>, Option<&str>)> {
    let (kind, id, agent) = match resource {
        ResourceRef::StateResource(_) => ("state", None, None),
        ResourceRef::CostsResource(_) => ("costs", None, None),
        ResourceRef::DesktopResource(_) => ("desktop", None, None),
        ResourceRef::TerminalsResource(_) => ("terminals", None, None),
        ResourceRef::AgentDetailResource(r) => ("agentDetail", Some(r.agent_id.as_str()), None),
        ResourceRef::SessionCostResource(r) => ("session-cost", Some(r.agent_id.as_str()), None),
        ResourceRef::TaskDetailResource(r) => ("taskDetail", Some(r.task_id.as_str()), None),
        ResourceRef::TaskResource(r) => ("task", Some(r.task_id.as_str()), None),
        ResourceRef::TerminalResource(r) => ("terminal", Some(r.terminal_id.as_str()), None),
        ResourceRef::TasksResource(r) => ("tasks", None, r.agent_id.as_deref()),
        ResourceRef::MonitorsResource(r) => ("monitors", None, r.agent_id.as_deref()),
        ResourceRef::RetainedWorktreesResource(r) => {
            ("retained-worktrees", None, Some(r.agent_id.as_str()))
        }
        ResourceRef::WorkResource(r) => ("work", None, Some(r.agent_id.as_str())),
        ResourceRef::AccountsResource(_)
        | ResourceRef::DraftsResource(_)
        | ResourceRef::LimitsResource(_)
        | ResourceRef::ModelsResource(_)
        | ResourceRef::MultiServerResource(_)
        | ResourceRef::NativeLayrSettingsResource(_)
        | ResourceRef::QueueResource(_)
        | ResourceRef::ReceiptsResource(_)
        | ResourceRef::RoomResource(_)
        | ResourceRef::TranscriptResource(_)
        | ResourceRef::TranscriptsResource(_)
        | ResourceRef::VoiceResource(_)
        | ResourceRef::WorkspaceResource(_)
        | ResourceRef::AuthProgressResource(_)
        | ResourceRef::RenameProgressResource(_)
        | ResourceRef::ProcessesResource(_)
        | ResourceRef::ComplaintResource(_)
        | ResourceRef::TerminalOutputResource(_) => return None,
    };
    Some((subscription_entity_scopes().get(kind)?, id, agent))
}

async fn read_page(
    state: &HttpState,
    resource: &ResourceRef,
    since: Option<i64>,
) -> Result<EntityPage, ApiError> {
    let (scope, id, agent) = selection(resource).ok_or_else(ApiError::internal)?;
    let id = id.map(str::to_owned);
    let agent = agent.map(str::to_owned);
    let collections =
        studio_json::to_string(&scope.collections).map_err(|_| ApiError::internal())?;
    state
        .store
        .as_ref()
        .ok_or_else(ApiError::internal)?
        .read_async("runtime-http/subscription-entities", move |tx| {
            studio_runtime_services::entity_scope::resume(
                tx,
                since,
                Some(&collections),
                id.as_deref(),
                agent.as_deref(),
                scope.include_team_members,
                SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
            )
            .map_err(|_| ApiError::internal())
        })
        .await
}

pub(super) async fn initial_snapshot(
    state: HttpState,
    resource: ResourceRef,
    since: Option<SubscriptionCursor>,
) -> Result<ResourceRevisionEntry, ApiError> {
    let after = since
        .filter(|cursor| cursor.workspace_id == state.workspace_id)
        .map(|cursor| cursor.seq);
    let page = read_page(&state, &resource, after).await?;
    let revision = page.max_sequence;
    // Existing global pull pagination owns oversized initial lists. Never
    // advertise a complete per-view cursor for a truncated interval.
    if page.checkpoint < page.max_sequence {
        return studio_json::from_value(json!({"revision":revision,"resource":resource}))
            .map_err(|_| ApiError::internal());
    }
    let documents = page
        .rows
        .iter()
        .map(super::sync_documents::entity_document)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(ApiError::internal)?;
    let entry: ResourceRevisionEntry = studio_json::from_value(json!({
        "revision":revision,
        "resource":resource,
        "cursor":{"workspaceId":state.workspace_id,"seq":revision},
        "replyKind":if page.reset {"snapshot"} else {"delta"},
        "entityChanges":{"after":if page.reset {0} else {after.unwrap_or(0)},"through":revision,"documents":documents}
    })).map_err(|_| ApiError::internal())?;
    if studio_json::to_vec(&entry)
        .map_err(|_| ApiError::internal())?
        .len()
        > SYNC_STREAM_EVENT_MAX_BYTES
    {
        return studio_json::from_value(json!({"revision":revision,"resource":entry.resource}))
            .map_err(|_| ApiError::internal());
    }
    Ok(entry)
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "subscription_entities",
        routes: None,
        dependencies: &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use studio_runtime_store::{
        AgentRecord, AgentRepository, RuntimeStore, StoreError, SyncDocumentsRepository,
    };

    #[test]
    fn typed_entity_selectors_match_every_openapi_scope() {
        for (kind, scope) in subscription_entity_scopes() {
            let mut value = json!({"kind":kind});
            for field in scope.id_field.iter().chain(&scope.agent_field) {
                value[field] = json!("selector");
            }
            let resource: ResourceRef =
                studio_json::from_value(value.clone()).expect("typed entity reference");
            let (selected, id, agent) = selection(&resource).expect("every OpenAPI entity scope");
            assert_eq!(selected.collections, scope.collections);
            assert_eq!(
                id,
                scope
                    .id_field
                    .as_deref()
                    .and_then(|field| value[field].as_str()),
                "{kind} id owner"
            );
            assert_eq!(
                agent,
                scope
                    .agent_field
                    .as_deref()
                    .and_then(|field| value[field].as_str()),
                "{kind} agent owner"
            );
        }
    }

    #[tokio::test]
    async fn retained_worktrees_include_deleted_members_and_stream_cleanup_deletes() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        studio_runtime_services::entity_publication::install(&store)
            .await
            .unwrap();
        store.transaction_with(|tx| {
            for (id, root, deleted) in [("lead", "lead", false), ("deleted-worker", "lead", true),
                ("hard-worker", "lead", true), ("other-worker", "other-lead", true),
                ("other-soft", "other-lead", true)] {
                let mut value = json!({"id":id,"rootId":root,"accountKey":"default","provider":"codex","epoch":1,"status":"completed","compactions":0,
                    "workspaceCleanupIntent":{"phase":"unknown","effect":{"kind":"remove_worktree","identity":{"cwd":"/fixture/retained"}},"receipt":{"reason":"Modified files"},"requestId":"private"}});
                if deleted { value["deletedAt"] = json!(1); }
                AgentRepository::put(tx, &AgentRecord::from_value(value)?)?;
            }
            Ok::<_, StoreError>(())
        }).await.unwrap();
        let workspace = store
            .test_read_transaction(|tx| SyncDocumentsRepository::workspace_id(tx))
            .unwrap();
        let state = HttpState::new(
            "token".into(),
            workspace,
            directory.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        let resource = ResourceRef::RetainedWorktreesResource(
            studio_runtime_schema::resource_ref::AgentResource {
                agent_id: String::from("lead").try_into().unwrap(),
            },
        );
        let initial = initial_snapshot(state.clone(), resource.clone(), None)
            .await
            .unwrap();
        let cursor = initial.cursor.clone();
        let frame = studio_json::to_value(initial).unwrap();
        let rows = frame["entityChanges"]["documents"].as_array().unwrap();
        let mut ids = rows
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, ["deleted-worker", "hard-worker", "lead"]);
        assert!(
            rows.iter()
                .all(|row| row["collection"] == "retainedWorktree")
        );
        assert!(!frame.to_string().contains("private"));
        store
            .transaction_with(|tx| {
                tx.execute(
                    "DELETE FROM runtime_agents WHERE id IN ('hard-worker','other-worker')",
                    [],
                )?;
                Ok::<_, StoreError>(())
            })
            .await
            .unwrap();
        let hard_delete = initial_snapshot(state.clone(), resource.clone(), cursor)
            .await
            .unwrap();
        let cursor = hard_delete.cursor.clone();
        let frame = studio_json::to_value(hard_delete).unwrap();
        let documents = frame["entityChanges"]["documents"].as_array().unwrap();
        assert!(
            !documents.iter().any(|row| row["id"] == "hard-worker"),
            "arbitrary hard deletes reconcile only by snapshot"
        );
        assert!(
            !documents.iter().any(|row| row["id"] == "other-worker"),
            "another root's hard-delete identity must never enter this feed"
        );
        let snapshot = initial_snapshot(state.clone(), resource.clone(), None)
            .await
            .unwrap();
        let snapshot = studio_json::to_value(snapshot).unwrap();
        assert!(
            !snapshot["entityChanges"]["documents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "hard-worker")
        );
        store.transaction_with(|tx| {
            tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.workspaceCleanupIntent.phase','confirmed') WHERE id IN ('deleted-worker','other-soft')", [])?;
            Ok::<_, StoreError>(())
        }).await.unwrap();
        let change = initial_snapshot(state, resource, cursor).await.unwrap();
        let frame = studio_json::to_value(change).unwrap();
        assert_eq!(
            frame["replyKind"], "delta",
            "valid reconnect cursor keeps the cleanup tombstone incremental"
        );
        assert!(
            !frame["entityChanges"]["documents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "other-soft")
        );
        assert!(
            frame["entityChanges"]["documents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "deleted-worker" && row["op"] == "delete")
        );
    }

    #[tokio::test]
    async fn task_detail_is_id_scoped_and_streams_existing_payload_and_delete() {
        let directory = tempfile::tempdir().expect("isolated state");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        studio_runtime_services::entity_publication::install(&store)
            .await
            .expect("publisher");
        store.transaction_with(|tx| {
            AgentRepository::put(tx, &AgentRecord::from_value(json!({"id":"lead","rootId":"lead","accountKey":"default","provider":"codex","epoch":1,"status":"idle","compactions":0}))?)?;
            for id in ["chosen", "other"] {
                tx.execute("INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)", rusqlite::params![id,json!({"id":id,"agent":"lead","kind":"tool","type":"dynamicToolCall","name":"tool","itemId":id,"turnId":"turn","status":"completed","created":1,"arguments":"private input","tail":"first"}).to_string()])?;
            }
            Ok::<_, StoreError>(())
        }).await.expect("tasks");
        let workspace = store
            .test_read_transaction(|tx| SyncDocumentsRepository::workspace_id(tx))
            .unwrap();
        let state = HttpState::new(
            "token".into(),
            workspace,
            directory.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        let resource =
            ResourceRef::TaskDetailResource(studio_runtime_schema::resource_ref::TaskResource {
                task_id: String::from("chosen").try_into().unwrap(),
            });
        let initial = initial_snapshot(state.clone(), resource.clone(), None)
            .await
            .expect("detail");
        let cursor = initial.cursor.clone();
        let frame = studio_json::to_value(initial).unwrap();
        let rows = frame["entityChanges"]["documents"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["collection"], "taskDetail");
        assert_eq!(rows[0]["id"], "chosen");
        let rest = store
            .test_read_transaction(|tx| {
                studio_runtime_store::WorkspaceViewsRepository::task(tx, "chosen")
            })
            .unwrap()
            .unwrap();
        assert_eq!(rows[0]["value"], rest, "same existing REST payload");
        assert_eq!(rows[0]["value"]["arguments"], "private input");
        for request in [
            json!({"kind":"state"}),
            json!({"kind":"tasks","agentId":"lead"}),
        ] {
            let frame = initial_snapshot(
                state.clone(),
                studio_json::from_value(request).unwrap(),
                None,
            )
            .await
            .unwrap();
            let frame = studio_json::to_value(frame).unwrap();
            for row in frame["entityChanges"]["documents"].as_array().unwrap() {
                assert_ne!(row["collection"], "taskDetail");
                assert!(
                    row["value"].get("arguments").is_none(),
                    "arguments stay outside summaries"
                );
            }
        }
        let mut owner_cursor = cursor.clone();
        for deleted in [true, false] {
            store.transaction_with(move |tx| {
                tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.deletedAt',?1) WHERE id='lead'", rusqlite::params![if deleted { Some(2.0) } else { None }])?;
                Ok::<_, StoreError>(())
            }).await.unwrap();
            let change = initial_snapshot(state.clone(), resource.clone(), owner_cursor)
                .await
                .unwrap();
            owner_cursor = change.cursor.clone();
            let value = studio_json::to_value(change).unwrap();
            let rows = value["entityChanges"]["documents"].as_array().unwrap();
            assert_eq!(
                rows.len(),
                1,
                "owner delete/restore must invalidate task detail"
            );
            if deleted {
                assert_eq!(rows[0]["op"], "delete");
            } else {
                assert_eq!(rows[0]["value"]["arguments"], "private input");
            }
        }
        let cursor = owner_cursor;
        store.transaction_with(|tx| {
            tx.execute("UPDATE runtime_tasks SET record=json_set(record,'$.tail','updated') WHERE id='chosen'", [])?;
            Ok::<_, StoreError>(())
        }).await.unwrap();
        let update = initial_snapshot(state.clone(), resource.clone(), cursor)
            .await
            .unwrap();
        let cursor = update.cursor.clone();
        assert_eq!(
            studio_json::to_value(update).unwrap()["entityChanges"]["documents"][0]["value"]["tail"],
            "updated"
        );
        store
            .transaction_with(|tx| {
                tx.execute("DELETE FROM runtime_tasks WHERE id='chosen'", [])?;
                Ok::<_, StoreError>(())
            })
            .await
            .unwrap();
        let removed = initial_snapshot(state, resource, cursor).await.unwrap();
        let frame = studio_json::to_value(removed).unwrap();
        assert_eq!(frame["entityChanges"]["documents"][0]["op"], "delete");
        assert_eq!(
            frame["entityChanges"]["documents"][0]["collection"],
            "taskDetail"
        );
    }

    #[tokio::test]
    async fn remote_project_binding_survives_overlapping_project_ids_in_server_frames() {
        let directory = tempfile::tempdir().expect("isolated server");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        store
            .set_local_server_id(Some("B".to_owned()))
            .expect("server identity");
        studio_runtime_services::entity_publication::install(&store)
            .await
            .expect("publisher");
        store
            .transaction_with(|tx| {
                studio_runtime_store::ProjectsRepository::put_project(
                    tx,
                    &json!({
                        "id":"P", "homeServerId":"B", "name":"Project B", "path":"/B/P"
                    }),
                )?;
                AgentRepository::put(
                    tx,
                    &AgentRecord::from_value(json!({
                        "compactions":0,"accountKey":"default","provider":"codex",
                        "id":"remote-chat","rootId":"remote-chat","epoch":1,
                        "status":"idle","isLead":true,"name":"Bound to A",
                        "cwd":"/B/alias","projectId":"P","projectServerId":"A"
                    }))?,
                )?;
                Ok::<_, StoreError>(())
            })
            .await
            .expect("published project and chat");
        let workspace = store
            .test_read_transaction(|tx| SyncDocumentsRepository::workspace_id(tx))
            .expect("workspace");
        let state = HttpState::new(
            "token".into(),
            workspace,
            directory.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store),
            None,
        );
        let resource =
            ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
        let frame = studio_json::to_value(
            initial_snapshot(state, resource, None)
                .await
                .expect("server snapshot"),
        )
        .expect("frame");
        let chat = frame["entityChanges"]["documents"]
            .as_array()
            .expect("documents")
            .iter()
            .find(|row| row["collection"] == "agentSummary" && row["id"] == "remote-chat")
            .expect("chat summary");
        assert_eq!(
            chat["value"]["projectServerId"], "A",
            "serving B must preserve the stored project home A"
        );
    }

    #[tokio::test]
    async fn view_scoped_entities_publish_list_and_detail_through_real_handlers() {
        let directory = tempfile::tempdir().expect("isolated state");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        studio_runtime_services::entity_publication::install(&store)
            .await
            .expect("publisher");
        store.transaction_with(|tx| {
            AgentRepository::put(tx, &AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"chat","rootId":"chat","epoch":1,"status":"completed","name":"List chat","threadId":"thread","lastCompletedTurn":"turn","lastCompletedTurnStatus":"completed","lastAnswer":"Detailed answer","tail":"Detailed output"}))?)
        }).await.expect("source commit publishes both views");
        let workspace = store
            .test_read_transaction(|tx| SyncDocumentsRepository::workspace_id(tx))
            .expect("workspace");
        let state = HttpState::new(
            "token".into(),
            workspace,
            directory.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        let list_ref: ResourceRef =
            studio_json::from_value(json!({"kind":"state"})).expect("state ref");
        let detail_ref: ResourceRef =
            studio_json::from_value(json!({"kind":"agentDetail","agentId":"chat"}))
                .expect("detail ref");
        store.transaction_with(|tx| {
            AgentRepository::put(tx,&AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"worker","rootId":"chat","parentId":"chat","epoch":1,"status":"idle"}))?)?;
            tx.execute("INSERT INTO runtime_tasks(id,record) VALUES(?1,?2)",rusqlite::params!["worker-command",json!({"id":"worker-command","agent":"worker","kind":"command","type":"commandExecution","name":"commandExecution","itemId":"command","turnId":"turn","status":"running","created":1}).to_string()])?;
            Ok::<_, StoreError>(())
        }).await.expect("worker command source");
        let tasks_ref: ResourceRef =
            studio_json::from_value(json!({"kind":"tasks","agentId":"chat"}))
                .expect("team task ref");
        let tasks = initial_snapshot(state.clone(), tasks_ref, None)
            .await
            .expect("team tasks");
        assert!(
            studio_json::to_value(tasks).expect("task frame")["entityChanges"]["documents"]
                .as_array()
                .expect("task records")
                .iter()
                .any(|row| row["id"] == "worker-command")
        );
        let list = initial_snapshot(state.clone(), list_ref.clone(), None)
            .await
            .expect("global snapshot");
        let before = list.cursor.clone();
        let list = studio_json::to_value(list).expect("typed entry");
        let documents = list["entityChanges"]["documents"]
            .as_array()
            .expect("complete list");
        assert!(
            documents
                .iter()
                .any(|row| row["collection"] == "agentSummary")
        );
        assert!(
            !documents
                .iter()
                .any(|row| row["collection"] == "agent" || row["collection"] == "agentDetail")
        );
        // Global state includes both agents, ordered by their durable changes.
        let summary = documents
            .iter()
            .find(|row| row["collection"] == "agentSummary" && row["id"] == "chat")
            .expect("summary");
        assert_eq!(summary["value"]["name"], "List chat");
        assert!(summary["value"].get("lastAnswer").is_none());
        let worker = documents
            .iter()
            .find(|row| row["collection"] == "agentSummary" && row["id"] == "worker")
            .expect("worker summary");
        assert_eq!(worker["value"]["id"], "worker");
        assert_eq!(worker["value"]["rootId"], "chat");
        assert_eq!(worker["value"]["status"], "idle");
        // This worker fixture has no name; its summary preserves that absence.
        assert!(worker["value"]["name"].is_null());
        let detail = initial_snapshot(state.clone(), detail_ref.clone(), None)
            .await
            .expect("detail snapshot");
        let detail = studio_json::to_value(detail).expect("typed detail");
        assert_eq!(
            detail["entityChanges"]["documents"][0]["value"]["lastAnswer"],
            "Detailed answer"
        );
        store
            .transaction_with(|tx| {
                let mut agent =
                    AgentRepository::get(tx, &studio_runtime_store::AgentId::from("chat"))?
                        .expect("agent")
                        .value();
                agent["lastAnswer"] = json!("Changed answer");
                AgentRepository::put(tx, &AgentRecord::from_value(agent.clone())?)?;
                studio_runtime_services::agents_create::publish_agent(tx, &agent)?;
                Ok::<_, StoreError>(())
            })
            .await
            .expect("detail output commit");
        let unchanged = initial_snapshot(state.clone(), list_ref, before)
            .await
            .expect("unchanged list");
        // The list view is built at read time, so a detail change sends the same
        // summary again; detail fields never reach the list.
        let unchanged = studio_json::to_value(unchanged).expect("typed list");
        for document in unchanged["entityChanges"]["documents"]
            .as_array()
            .expect("delta")
        {
            assert!(document["collection"] != "agent" && document["collection"] != "agentDetail");
            assert!(document["value"].get("lastAnswer").is_none());
        }
        let entry = initial_snapshot(state.clone(), detail_ref, None)
            .await
            .expect("changed detail");
        assert_eq!(
            studio_json::to_value(entry).expect("typed entry")["entityChanges"]["documents"][0]["value"]
                ["lastAnswer"],
            "Changed answer"
        );
    }
}
