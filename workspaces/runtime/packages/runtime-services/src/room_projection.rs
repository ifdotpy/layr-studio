//! Typed public room view, derived from the stored room and its directory.
use crate::submit_gates::{AgentAuthorityRow, Deletion};
use crate::sync_views::{RoomViewContext, SYNC_ARRAY_ITEMS, bound_sync_text};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_schema::recovery_record::SafeInteger;
use studio_runtime_schema::room_record::{
    Radio, RoomKind, RoomLastMessage, RoomRadioIdentity, RoomRadioSeen, RoomRecord, Status,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoomProjection {
    pub(crate) id: String,
    pub(crate) kind: RoomKind,
    pub(crate) members: Vec<String>,
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    last_message: Field<RoomLastMessage>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    pub(crate) local_members: Field<Vec<String>>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    pub(crate) peer_team_id: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    pub(crate) peer_team_name: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    pub(crate) peer_label: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    pub(crate) root_id: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    project_path: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    updated: Field<serde_json::Number>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    user_hidden: Field<bool>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    radio: Field<RadioProjection>,
}

impl RoomProjection {
    pub(crate) fn omit_last_message(&mut self) {
        self.last_message = Field::Missing;
    }
    pub(crate) fn direct_radio(&self) -> bool {
        self.radio.as_ref().and_then(|radio| radio.direct.as_ref()) == Some(&true)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RadioActive {
    identity: RoomRadioIdentity,
    event_id: String,
    agent_id: String,
    epoch: SafeInteger,
    thread_id: Option<String>,
    through: SafeInteger,
    #[serde(skip_serializing_if = "Field::is_missing")]
    turn_id: Field<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    interrupt_requested: Field<bool>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    question_continuation_planned: Field<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RadioProjection {
    team_id: String,
    revision: SafeInteger,
    status: Status,
    speaker: Option<String>,
    next: Vec<String>,
    active: Option<RadioActive>,
    error: Option<String>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    direct: Field<bool>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    seen: Field<BTreeMap<String, RoomRadioSeen>>,
}

fn bounded(text: &str, field: &str) -> String {
    bound_sync_text(text, field, false)
}
fn active_agent<'a>(context: &'a RoomViewContext, id: &str) -> Option<&'a AgentAuthorityRow> {
    context
        .active_agent_indices
        .get(id)
        .and_then(|index| context.agents.get(*index))
}

pub(crate) fn project_room(
    record: &RoomRecord,
    context: &RoomViewContext,
) -> Result<Option<RoomProjection>, serde_json::Error> {
    if record.id.is_empty() || record.project_deleted_at.is_some() {
        return Ok(None);
    }
    match &record.radio {
        Field::Value(state) => project_radio_room(record, context, state),
        Field::Null => Ok(project_room_view(record, context, Field::Null)),
        Field::Missing => Ok(project_room_view(record, context, Field::Missing)),
    }
}

pub(crate) fn project_radio_room(
    record: &RoomRecord,
    context: &RoomViewContext,
    state: &Radio,
) -> Result<Option<RoomProjection>, serde_json::Error> {
    if record.id.is_empty() || record.project_deleted_at.is_some() {
        return Ok(None);
    }
    let safe = |value: u64| {
        i64::try_from(value)
            .ok()
            .and_then(SafeInteger::from_i64)
            .ok_or_else(|| serde_json::Error::io(std::io::Error::other("invalid radio integer")))
    };
    let active = state
        .active
        .as_ref()
        .map(|active| -> Result<RadioActive, serde_json::Error> {
            Ok(RadioActive {
                identity: active.identity.clone(),
                event_id: active.event_id.clone(),
                agent_id: active.agent_id.clone(),
                epoch: safe(active.epoch)?,
                thread_id: active.thread_id.clone(),
                through: safe(active.through)?,
                turn_id: active.turn_id.clone(),
                interrupt_requested: active.interrupt_requested.clone(),
                question_continuation_planned: active.question_continuation_planned.clone(),
            })
        })
        .transpose()?;
    let radio = RadioProjection {
        team_id: state.team_id.clone(),
        revision: safe(state.revision)?,
        status: state.status,
        speaker: state.speaker.clone(),
        next: state.next.clone(),
        active,
        error: state.error.clone(),
        direct: Field::Value(state.direct),
        seen: Field::Value(state.seen.clone()),
    };
    Ok(project_room_view(
        record,
        context,
        Field::Value(bound_radio(radio)),
    ))
}

fn project_room_view(
    record: &RoomRecord,
    context: &RoomViewContext,
    radio: Field<RadioProjection>,
) -> Option<RoomProjection> {
    let custom = record
        .custom_name
        .as_ref()
        .map(String::as_str)
        .filter(|s| !s.is_empty());
    let mut local_members = record.local_members.clone();
    let mut peer_team_id = record.peer_team_id.clone();
    let mut peer_team_name = record.peer_team_name.clone();
    let mut peer_label = record.peer_label.clone();
    let (members, name) = match record.kind {
        RoomKind::Federated => {
            let federation = context.federation.as_ref()?;
            if federation.get("status")?.as_str()? != "approved"
                || !federation.get("localApproved")?.as_bool()?
                || !federation.get("remoteApproved")?.as_bool()?
            {
                return None;
            }
            let mut members = federation
                .get("localMembers")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            members.sort_unstable();
            members.dedup();
            if members.is_empty() || members.iter().any(|id| active_agent(context, id).is_none()) {
                return None;
            }
            local_members = Field::Value(members.clone());
            peer_label = match federation.get("peerLabel").and_then(Value::as_str) {
                Some(label) => Field::Value(label.to_owned()),
                None => Field::Null,
            };
            let name = custom.map(str::to_owned).or_else(|| {
                federation
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            (members, name)
        }
        RoomKind::Broadcast => {
            let root = record.root_id.as_ref()?.as_str();
            let members = context
                .agents
                .iter()
                .filter(|agent| !agent.deleted_at.as_ref().is_some_and(Deletion::is_deleted))
                .filter(|agent| root == "all" || agent.root_id.as_deref() == Some(root))
                .filter_map(|agent| agent.id.clone())
                .collect();
            let fallback = if root == "all" {
                "All agents".to_owned()
            } else {
                format!(
                    "{} · Broadcast",
                    active_agent(context, root)?.name.value_or(None)?
                )
            };
            (members, Some(custom.unwrap_or(&fallback).to_owned()))
        }
        RoomKind::Private => {
            let members = record.members.clone()?;
            if members.is_empty() || members.iter().any(|id| active_agent(context, id).is_none()) {
                return None;
            }
            let fallback = members
                .iter()
                .filter_map(|id| active_agent(context, id))
                .filter_map(|agent| agent.name.value_or(None).map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ↔ ");
            if members.len() == 2 {
                if let Some(team) = context
                    .peer_teams
                    .iter()
                    .find(|team| members.iter().all(|member| team.members.contains(member)))
                {
                    peer_team_id = Field::Value(team.id.clone());
                    peer_team_name = Field::Value(team.name.clone());
                } else {
                    peer_team_id = Field::Missing;
                    peer_team_name = Field::Missing;
                }
            }
            (members, Some(custom.unwrap_or(&fallback).to_owned()))
        }
    };
    let last_message = context
        .last_message
        .as_ref()
        .map(|message| RoomLastMessage {
            seq: message.seq.clone(),
            text: bounded(&message.text, "text"),
            sender: bounded(&message.sender, "sender"),
            created: message.created.clone(),
        });
    let optional = |v: &Field<String>, field| match v {
        Field::Missing => Field::Missing,
        Field::Null => Field::Null,
        Field::Value(v) => Field::Value(bounded(v, field)),
    };
    let bounded_members = |v: Vec<String>| {
        v.into_iter()
            .take(SYNC_ARRAY_ITEMS)
            .map(|s| bounded(&s, ""))
            .collect()
    };
    Some(RoomProjection {
        id: bounded(&record.id, "id"),
        kind: record.kind,
        members: bounded_members(members),
        name: name.map(|s| bounded(&s, "name")),
        last_message: match last_message {
            Some(v) => Field::Value(v),
            None => Field::Null,
        },
        local_members: match local_members {
            Field::Missing => Field::Missing,
            Field::Null => Field::Null,
            Field::Value(v) => Field::Value(bounded_members(v)),
        },
        peer_team_id: optional(&peer_team_id, "peerTeamId"),
        peer_team_name: optional(&peer_team_name, "peerTeamName"),
        peer_label: optional(&peer_label, "peerLabel"),
        root_id: optional(&record.root_id, "rootId"),
        project_path: optional(&record.project_path, "projectPath"),
        updated: record.updated.clone(),
        user_hidden: record.user_hidden.clone(),
        radio,
    })
}

fn bound_radio(mut state: RadioProjection) -> RadioProjection {
    state.team_id = bounded(&state.team_id, "teamId");
    state.speaker = state.speaker.map(|s| bounded(&s, "speaker"));
    state.error = state.error.map(|s| bounded(&s, "error"));
    state.next = state
        .next
        .into_iter()
        .take(SYNC_ARRAY_ITEMS)
        .map(|s| bounded(&s, ""))
        .collect();
    if let Some(active) = &mut state.active {
        active.event_id = bounded(&active.event_id, "eventId");
        active.agent_id = bounded(&active.agent_id, "agentId");
        active.thread_id = active.thread_id.take().map(|s| bounded(&s, "threadId"));
        if let Field::Value(turn_id) = &mut active.turn_id {
            *turn_id = bounded(turn_id, "turnId");
        }
        active.identity.0 = active.identity.0.take().map(|s| bounded(&s, ""));
    }
    if let Field::Value(seen) = &mut state.seen {
        for cursor in seen.values_mut() {
            cursor.identity.0 = cursor.identity.0.take().map(|s| bounded(&s, ""));
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn room_projection_matches_existing_view() {
        let agents = ["a", "b"]
            .into_iter()
            .map(|id| serde_json::from_value(json!({"id":id,"name":id,"rootId":"a"})).unwrap())
            .collect();
        let mut context = RoomViewContext::new(agents, vec![]);
        context.last_message = Some(RoomLastMessage {
            seq: studio_runtime_schema::room_record::ChatSequence::from_i64(1).unwrap(),
            text: "hello".into(),
            sender: "a".into(),
            created: 1.into(),
        });
        context.federation = Some(
            json!({"status":"approved","localApproved":true,"remoteApproved":true,"localMembers":["b","a","a"],"name":"Remote chat","peerLabel":"Peer"}),
        );
        for raw in [
            json!({"id":"private:a:b","kind":"private","members":["a","b"],"customName":"hello","projectPath":null,"updated":1,"userHidden":false}),
            json!({"id":"broadcast:a","kind":"broadcast","rootId":"a"}),
            json!({"id":"federated:x","kind":"federated","members":["a"],"customName":null}),
            json!({"id":"radio:x","kind":"private","members":["a","b"],"radio":{"teamId":"x","revision":0,"status":"idle","speaker":null,"next":[],"active":null,"error":null,"direct":false,"seen":{}}}),
            json!({"id":"radio:y","kind":"private","members":["a","b"],"radio":{"teamId":"x","revision":1,"status":"speaking","speaker":"a","next":[],"active":{"identity":["thread",1,2],"eventId":"event","agentId":"a","epoch":1,"threadId":"thread","through":3,"turnId":null},"error":null,"direct":false,"seen":{"a":{"identity":["thread",1,2],"seq":3}}}}),
        ] {
            let record: RoomRecord = serde_json::from_str(&raw.to_string()).unwrap();
            let expected = crate::sync_views::room_entity_view(&raw, &context).unwrap();
            let actual =
                serde_json::to_value(project_room(&record, &context).unwrap().unwrap()).unwrap();
            assert_eq!(actual, expected);
            if let Field::Value(state) = &record.radio {
                let direct = serde_json::to_value(
                    project_radio_room(&record, &context, state)
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(direct, expected);
            }
        }
    }

    #[test]
    fn malformed_radio_is_an_error_not_a_deleted_room() {
        assert!(
            serde_json::from_str::<RoomRecord>(
                r#"{"id":"r","kind":"private","radio":{"teamId":"broken"}}"#
            )
            .is_err()
        );
    }
}
