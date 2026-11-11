//! Transactional chat organization effects from `/api/organization`.
//!
//! Deliberate tightening: malformed persisted folder/read-state revisions return
//! a located `runtime_agents` stored-object error rather than a generic request
//! failure (or a default revision).

use rusqlite::OptionalExtension;
use studio_json::{self, Value};
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
            let project: Value = studio_json::from_str(&project).map_err(|_| {
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
        let current = stored_folder_revision(&agent.project_folder_revision, id)?;
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
            agent.project_folder_revision = Field::Value(studio_runtime_schema::raw_json::RawJson(
                studio_json::to_raw_value(&revision)
                    .map_err(studio_runtime_store::StoreError::from)?,
            ));
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
        agent.project = studio_runtime_schema::queue_command::Field::Value(
            studio_runtime_store::StoredTransferValue::Known(value.trim().to_owned()),
        );
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
    let previous = agent.read_state.as_ref().map(std::ops::Deref::deref);
    let current = previous.map_or(0, |state| state.revision.as_i64());
    let same = previous.is_some_and(|state| {
        state.thread_id == thread && state.turn_id == turn && state.read == read_value
    });
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
    let revision = current
        .checked_add(1)
        .and_then(studio_runtime_schema::AgentCounter::from_i64)
        .ok_or_else(|| invalid_agent(&agent))?;
    agent.read_state = studio_runtime_schema::queue_command::Field::Value(Box::new(
        studio_runtime_schema::agent_entity_nested_extra::ReadStateDto {
            thread_id: thread.to_owned(),
            turn_id: turn.to_owned(),
            read: read_value,
            revision,
        },
    ));
    AgentRepository::put(tx, &agent)?;
    publish(tx, agent)
}

fn stored_folder_revision(
    value: &Field<studio_runtime_schema::raw_json::RawJson>,
    id: &str,
) -> Result<i64, EventWriteError> {
    match value {
        Field::Missing => Ok(0),
        Field::Value(raw) => studio_json::from_str::<i64>(raw.0.as_str())
            .ok()
            .filter(|revision| *revision >= 0)
            .ok_or_else(|| EventWriteError::InvalidStoredObject {
                object: "runtime_agents",
                id: id.to_owned(),
            }),
        Field::Null => Err(EventWriteError::InvalidStoredObject {
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
    use studio_json::{Value, json};
    use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore, StoreError};

    const CHAT: &str = "organized-chat";

    #[test]
    fn folder_revision_preserves_missing_valid_and_malformed_behavior() {
        use studio_runtime_schema::{queue_command::Field, raw_json::RawJson};
        let decode = |raw: &str| {
            Field::Value(RawJson(
                studio_json::OwnedRawJson::from_string(raw.into()).unwrap(),
            ))
        };
        assert_eq!(
            super::stored_folder_revision(&Field::Missing, CHAT).unwrap(),
            0
        );
        assert_eq!(
            super::stored_folder_revision(&decode("1"), CHAT).unwrap(),
            1
        );
        for field in [
            Field::Null,
            decode("-1"),
            decode("1.0"),
            decode(r#""invalid""#),
        ] {
            assert!(matches!(
                super::stored_folder_revision(&field, CHAT),
                Err(super::EventWriteError::InvalidStoredObject {
                    object: "runtime_agents",
                    ..
                })
            ));
        }
    }

    #[test]
    fn organization_request_is_strict_and_bounds_revision() {
        use super::OrganizationRequest;
        use studio_runtime_schema::http_ingress::Object;
        assert!(
            studio_json::from_str::<Object<OrganizationRequest>>(
                r#"{"id":"agent","archived":true}"#
            )
            .is_ok()
        );
        for raw in [
            r#"{"id":"agent","extra":true}"#,
            r#"{"id":"agent","expected_revision":9007199254740992}"#,
            r#"{"id":"agent","read_state":["thread","turn",true,0]}"#,
        ] {
            assert!(studio_json::from_str::<Object<OrganizationRequest>>(raw).is_err());
        }
    }

    /// The client view of the chat, as the renderer pulls it.
    fn client_view(store: &RuntimeStore) -> Value {
        let payload = store
            .test_read_transaction(|tx| crate::entity_scope::direct_document(tx, "agent", CHAT))
            .expect("the chat entity reads")
            .and_then(|row| row.payload)
            .expect("the chat entity is live");
        studio_json::from_str::<Value>(&payload).expect("entity JSON")["value"].clone()
    }

    async fn organize(store: &RuntimeStore, body: &str) {
        let studio_runtime_schema::http_ingress::Object(body) = studio_json::from_str::<
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

        organize(
            &store,
            r#"{"id":"organized-chat","archived":true,"pinned":true}"#,
        )
        .await;
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
}
