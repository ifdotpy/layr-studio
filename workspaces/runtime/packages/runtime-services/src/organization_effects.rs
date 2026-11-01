//! Transactional chat organization effects from `/api/organization`.
//!
//! Deliberate tightening: malformed persisted folder/read-state revisions return
//! a located `runtime_agents` stored-object error rather than a generic request
//! failure (or a default revision).

use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};
use studio_runtime_schema::{
    bounded_integer::BoundedInteger, bounded_text::BoundedText, http_ingress::Object,
    queue_command::Field,
};
use studio_runtime_store::{AgentRecord, AgentRepository, Transaction};

use crate::event_write::{self, EventWriteError};

/// Maximum request identity or path length, in Unicode code points.
const ORGANIZATION_ID_MAX_CHARS: usize = 4096;
type Text = BoundedText<0, ORGANIZATION_ID_MAX_CHARS>;
type Revision =
    BoundedInteger<0, { studio_runtime_schema::shared_values::MAX_SAFE_INTEGER as u64 }>;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationRequest {
    id: Text,
    #[serde(default)]
    pinned: Field<bool>,
    #[serde(default)]
    archived: Field<bool>,
    #[serde(default)]
    project: Field<Text>,
    #[serde(default)]
    project_folder: Field<Text>,
    #[serde(default)]
    project_path: Field<Text>,
    #[serde(default)]
    expected_folder: Field<Text>,
    #[serde(default)]
    expected_revision: Field<Revision>,
    #[serde(default)]
    read_state: Field<Object<ReadStateRequest>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadStateRequest {
    thread_id: Text,
    turn_id: Text,
    read: bool,
    expected_revision: Revision,
}

pub fn apply(tx: &Transaction<'_>, body: &OrganizationRequest) -> Result<Value, EventWriteError> {
    let id = body.id.as_str();
    let mut agent = event_write::checked_agent(tx, id)?;
    match &body.read_state {
        Field::Value(Object(read)) => return apply_read_state(tx, agent, read),
        Field::Null => {
            return Err(EventWriteError::InvalidRequest(
                "Supply a nonempty result identity",
            ));
        }
        Field::Missing => {}
    }
    if !body.project_folder.is_missing() {
        if agent.is_lead != Some(true)
            || body.project_path.value().map(Text::as_str) != agent.cwd.as_deref()
        {
            return Err(EventWriteError::InvalidRequest(
                "Select a chat in this project",
            ));
        }
        let desired = if matches!(&body.project_folder, Field::Null) {
            None
        } else {
            let folder_id = body
                .project_folder
                .value()
                .map(Text::as_str)
                .ok_or(EventWriteError::InvalidRequest("Select a project folder"))?;
            let path = agent.cwd.as_deref().unwrap_or_default();
            let project = tx
                .query_row(
                    "SELECT record FROM runtime_projects WHERE id=?1",
                    [path],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(studio_runtime_store::StoreError::from)?
                .unwrap_or_else(|| "{}".to_owned());
            let project: Value = serde_json::from_str(&project).map_err(|_| {
                EventWriteError::InvalidStoredObject {
                    object: "runtime_projects",
                    id: path.to_owned(),
                }
            })?;
            let exists = project
                .get("folders")
                .and_then(Value::as_array)
                .is_some_and(|folders| {
                    folders
                        .iter()
                        .any(|f| f.get("id").and_then(Value::as_str) == Some(folder_id))
                });
            if !exists {
                return Err(EventWriteError::InvalidRequest(
                    "This folder is not in the selected project",
                ));
            }
            Some(folder_id.to_owned())
        };
        let current = stored_revision(agent.extra.get("projectFolderRevision"), id)?;
        let revision = body
            .expected_revision
            .value()
            .copied()
            .map(Revision::get)
            .ok_or(EventWriteError::InvalidRequest(
                "Supply the current chat folder revision",
            ))?;
        let current_folder = agent.project_folder.value().cloned();
        let replay = current_folder == desired
            && (revision == current || current.checked_sub(1) == Some(revision));
        if !replay
            && (revision != current
                || body.expected_folder.value().map(Text::as_str) != current_folder.as_deref())
        {
            return Err(EventWriteError::InvalidRequest(
                "The chat folder changed. Reload it before moving",
            ));
        }
        if current_folder != desired {
            let revision = current
                .checked_add(1)
                .ok_or(EventWriteError::InvalidRequest(
                    "The chat folder revision is invalid",
                ))?;
            agent
                .extra
                .insert("projectFolderRevision".into(), json!(revision));
        }
        match desired {
            Some(folder) => {
                agent.project_folder = studio_runtime_schema::queue_command::Field::Value(folder);
            }
            None => {
                agent.project_folder = studio_runtime_schema::queue_command::Field::Null;
            }
        }
    }
    for (field, value) in [("pinned", &body.pinned), ("archived", &body.archived)] {
        if !value.is_missing() {
            let enabled = value.value().copied().unwrap_or_default();
            if field == "archived"
                && enabled
                && (agent.in_flight == Some(true) || active_monitor(tx, id)?)
            {
                return Err(EventWriteError::InvalidRequest(
                    "Stop active work before archiving this conversation",
                ));
            }
            match field {
                "archived" => {
                    agent.archived = studio_runtime_schema::queue_command::Field::Value(enabled)
                }
                "pinned" => {
                    agent.pinned = studio_runtime_schema::queue_command::Field::Value(enabled)
                }
                _ => unreachable!(),
            }
        }
    }
    if !body.project.is_missing() {
        let value = body.project.value().map(Text::as_str).unwrap_or_default();
        if value.chars().count() > 100 {
            return Err(EventWriteError::InvalidRequest("Supply a project name"));
        }
        agent.extra.insert("project".into(), json!(value.trim()));
    }
    AgentRepository::put(tx, &agent)?;
    publish(tx, agent)
}

/// The renderer reads chats only from their sync entity: publish the stored change with it.
fn publish(tx: &Transaction<'_>, agent: AgentRecord) -> Result<Value, EventWriteError> {
    let value = agent.value();
    crate::agents_create::publish_agent(tx, &value)?;
    Ok(value)
}

fn active_monitor(tx: &Transaction<'_>, id: &str) -> Result<bool, EventWriteError> {
    Ok(tx.query_row("SELECT EXISTS(SELECT 1 FROM runtime_monitors WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.status') IN ('running','approval','starting'))", [id], |r| r.get::<_, bool>(0)).map_err(studio_runtime_store::StoreError::from)?)
}

fn apply_read_state(
    tx: &Transaction<'_>,
    mut agent: AgentRecord,
    read: &ReadStateRequest,
) -> Result<Value, EventWriteError> {
    let thread = read.thread_id.as_str();
    let turn = read.turn_id.as_str();
    if thread.trim().is_empty() || turn.trim().is_empty() {
        return Err(EventWriteError::InvalidRequest(
            "Supply a nonempty result identity",
        ));
    }
    let read_value = read.read;
    let revision = read.expected_revision.get();
    if agent.thread_id.as_deref() != Some(thread)
        || agent.last_completed_turn.value().map(String::as_str) != Some(turn)
        || agent.last_completed_turn_status.value().map(String::as_str) != Some("completed")
    {
        return Err(EventWriteError::InvalidRequest(
            "The completed result changed. Reload the chat",
        ));
    }
    let previous = match agent.extra.get("readState") {
        None => json!({}),
        Some(Value::Object(_)) => agent.extra["readState"].clone(),
        Some(_) => return Err(invalid_agent(&agent)),
    };
    let current = stored_revision(previous.get("revision"), &agent.id.0)?;
    let same = previous.get("threadId").and_then(Value::as_str) == Some(thread)
        && previous.get("turnId").and_then(Value::as_str) == Some(turn)
        && previous.get("read").and_then(Value::as_bool) == Some(read_value);
    if same && (revision == current || current.checked_sub(1) == Some(revision)) {
        // A replay writes nothing, but the client that sends it may hold a view that a
        // build without this publication left behind; publish the stored state again.
        return publish(tx, agent);
    }
    if revision != current {
        return Err(EventWriteError::InvalidRequest(
            "The read state changed. Reload the chat",
        ));
    }
    let mut state = Map::new();
    state.insert("threadId".into(), json!(thread));
    state.insert("turnId".into(), json!(turn));
    state.insert("read".into(), json!(read_value));
    let next = current
        .checked_add(1)
        .ok_or_else(|| invalid_agent(&agent))?;
    state.insert("revision".into(), json!(next));
    agent.extra.insert("readState".into(), Value::Object(state));
    AgentRepository::put(tx, &agent)?;
    publish(tx, agent)
}

fn stored_revision(value: Option<&Value>, id: &str) -> Result<i64, EventWriteError> {
    match value {
        None => Ok(0),
        Some(value) => value
            .as_i64()
            .filter(|revision| *revision >= 0)
            .ok_or_else(|| EventWriteError::InvalidStoredObject {
                object: "runtime_agents",
                id: id.to_owned(),
            }),
    }
}

fn invalid_agent(agent: &AgentRecord) -> EventWriteError {
    EventWriteError::InvalidStoredObject {
        object: "runtime_agents",
        id: agent.id.0.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use serde_json::{Value, json};
    use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore, StoreError};

    const CHAT: &str = "organized-chat";

    #[test]
    fn organization_request_is_strict_and_bounds_revision() {
        use super::OrganizationRequest;
        use studio_runtime_schema::http_ingress::Object;
        assert!(
            serde_json::from_str::<Object<OrganizationRequest>>(
                r#"{"id":"agent","archived":true}"#
            )
            .is_ok()
        );
        for raw in [
            r#"{"id":"agent","extra":true}"#,
            r#"{"id":"agent","expected_revision":9007199254740992}"#,
            r#"{"id":"agent","read_state":["thread","turn",true,0]}"#,
        ] {
            assert!(serde_json::from_str::<Object<OrganizationRequest>>(raw).is_err());
        }
    }

    /// The client view of the chat, as the renderer pulls it.
    fn client_view(store: &RuntimeStore) -> Value {
        let payload: String = store
            .test_read_transaction(|tx| {
                Ok(tx.query_row(
                    "SELECT payload FROM sync_entities WHERE collection='agent' AND id=?1 AND deleted=0",
                    [CHAT],
                    |row| row.get(0),
                )?)
            })
            .expect("the chat entity reads");
        serde_json::from_str::<Value>(&payload).expect("entity JSON")["value"].clone()
    }

    async fn organize(store: &RuntimeStore, body: &str) {
        let studio_runtime_schema::http_ingress::Object(body) = serde_json::from_str::<
            studio_runtime_schema::http_ingress::Object<super::OrganizationRequest>,
        >(body)
        .unwrap();
        store
            .transaction_with_error(move |tx| apply(tx, &body))
            .await
            .expect("organization write succeeds");
    }

    #[tokio::test]
    async fn organization_writes_reach_the_client_view_of_the_chat() {
        let dir = tempfile::tempdir().expect("isolated state");
        let store = RuntimeStore::open(Some(dir.path()))
            .await
            .expect("runtime store opens");
        let agent = AgentRecord::from_value(json!({"compactions":0,
            "id":CHAT,"rootId":CHAT,"epoch":1,"status":"idle","name":"Organized chat",
            "provider":"codex","model":"gpt-6","prompt":"Start","cwd":"/fixture",
            "accountKey":"default","isLead":true,"autoWake":false,"threadId":"thread-1","projectFolder":"folder-a",
            "lastCompletedTurn":"turn-1","lastCompletedTurnStatus":"completed"
        }))
        .expect("valid persisted agent");
        store
            .transaction_with(move |tx| {
                AgentRepository::put(tx, &agent)?;
                crate::agents_create::publish_agent(tx, &agent.value())?;
                Ok::<_, StoreError>(())
            })
            .await
            .expect("the chat is seeded and published");
        assert_ne!(client_view(&store)["archived"], true);

        let clear_folder = r#"{"id":"organized-chat","project_path":"/fixture","project_folder":null,"expected_folder":"folder-a","expected_revision":0}"#;
        organize(&store, clear_folder).await;
        organize(&store, clear_folder).await;
        let view = client_view(&store);
        assert_eq!(view["projectFolder"], Value::Null);
        assert_eq!(view["projectFolderRevision"], 1);

        organize(&store, r#"{"id":"organized-chat","archived":true,"pinned":true}"#).await;
        let view = client_view(&store);
        assert_eq!(view["archived"], true, "an archive reaches the client");
        assert_eq!(view["pinned"], true, "a pin reaches the client");

        organize(
            &store,
            r#"{"id":"organized-chat","read_state":{"thread_id":"thread-1","turn_id":"turn-1","read":true,"expected_revision":0}}"#,
        )
        .await;
        assert_eq!(
            client_view(&store)["readState"]["read"],
            true,
            "a read state reaches the client"
        );
    }

    /// A read state stored by a build that did not publish it leaves the client one
    /// revision behind. The client then repeats the read with its old revision, which
    /// the server accepts as a replay; the replay must repair the client view.
    #[tokio::test]
    async fn a_read_replay_repairs_a_stale_client_view() {
        let dir = tempfile::tempdir().expect("isolated state");
        let store = RuntimeStore::open(Some(dir.path()))
            .await
            .expect("runtime store opens");
        let stale = json!({"compactions":0,
            "id":CHAT,"rootId":CHAT,"epoch":1,"status":"completed","name":"Organized chat",
            "provider":"codex","model":"gpt-6","prompt":"Start","cwd":"/fixture",
            "accountKey":"default","isLead":true,"autoWake":true,"threadId":"thread-1",
            "lastCompletedTurn":"turn-1","lastCompletedTurnStatus":"completed",
            "readState":{"read":true,"revision":2,"threadId":"thread-1","turnId":"turn-1"}
        });
        let mut stored = stale.clone();
        stored["lastCompletedTurn"] = json!("turn-2");
        stored["readState"] =
            json!({"read":true,"revision":3,"threadId":"thread-1","turnId":"turn-2"});
        store
            .transaction_with(move |tx| {
                let published = AgentRecord::from_value(stale)?;
                AgentRepository::put(tx, &published)?;
                crate::agents_create::publish_agent(tx, &published.value())?;
                // The read of turn-2 was stored without a publication.
                AgentRepository::put(tx, &AgentRecord::from_value(stored)?)?;
                Ok::<_, StoreError>(())
            })
            .await
            .expect("the chat is seeded with a stale client view");
        assert_eq!(client_view(&store)["readState"]["turnId"], "turn-1");

        // The client sees turn-2 unread and marks it read with the revision it knows.
        organize(
            &store,
            r#"{"id":"organized-chat","read_state":{"thread_id":"thread-1","turn_id":"turn-2","read":true,"expected_revision":2}}"#,
        )
        .await;
        let read = &client_view(&store)["readState"];
        assert_eq!(
            read["turnId"], "turn-2",
            "the replay repairs the client view"
        );
        assert_eq!(read["revision"], 3);
    }
}
