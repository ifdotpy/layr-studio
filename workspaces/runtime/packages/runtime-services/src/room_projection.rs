//! Typed public room view, derived from the stored room and its directory.
use crate::submit_gates::{AgentAuthorityRow, Deletion};
use crate::sync_views::{RoomViewContext, SYNC_ARRAY_ITEMS, bound_sync_text};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message: Option<Option<RoomLastMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) local_members: Option<Option<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peer_team_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peer_team_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peer_label: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) root_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated: Option<Option<serde_json::Number>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_hidden: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radio: Option<Option<RadioProjection>>,
}

impl RoomProjection {
    pub(crate) fn omit_last_message(&mut self) {
        self.last_message = None;
    }
    pub(crate) fn direct_radio(&self) -> bool {
        self.radio
            .as_ref()
            .and_then(Option::as_ref)
            .and_then(|radio| radio.direct)
            .flatten()
            == Some(true)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interrupt_requested: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    question_continuation_planned: Option<Option<bool>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    direct: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seen: Option<Option<BTreeMap<String, RoomRadioSeen>>>,
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
        Some(Some(state)) => project_radio_room(record, context, state),
        Some(None) => Ok(project_room_view(record, context, Some(None))),
        None => Ok(project_room_view(record, context, None)),
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
                interrupt_requested: active.interrupt_requested,
                question_continuation_planned: active.question_continuation_planned,
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
        direct: Some(Some(state.direct)),
        seen: Some(Some(state.seen.clone())),
    };
    Ok(project_room_view(
        record,
        context,
        Some(Some(bound_radio(radio))),
    ))
}

fn project_room_view(
    record: &RoomRecord,
    context: &RoomViewContext,
    radio: Option<Option<RadioProjection>>,
) -> Option<RoomProjection> {
    let custom = record
        .custom_name
        .as_ref()
        .and_then(|name| name.as_deref())
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
            local_members = Some(Some(members.clone()));
            peer_label = Some(
                federation
                    .get("peerLabel")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            );
            let name = custom.map(str::to_owned).or_else(|| {
                federation
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            (members, name)
        }
        RoomKind::Broadcast => {
            let root = record.root_id.as_ref()?.as_deref()?;
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
                    peer_team_id = Some(Some(team.id.clone()));
                    peer_team_name = Some(Some(team.name.clone()));
                } else {
                    peer_team_id = None;
                    peer_team_name = None;
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
    let optional = |v: &Option<Option<String>>, field| {
        v.as_ref().map(|v| v.as_deref().map(|s| bounded(s, field)))
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
        last_message: Some(last_message),
        local_members: local_members.map(|v| v.map(bounded_members)),
        peer_team_id: optional(&peer_team_id, "peerTeamId"),
        peer_team_name: optional(&peer_team_name, "peerTeamName"),
        peer_label: optional(&peer_label, "peerLabel"),
        root_id: optional(&record.root_id, "rootId"),
        project_path: optional(&record.project_path, "projectPath"),
        updated: record.updated.clone(),
        user_hidden: record.user_hidden,
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
        active.turn_id = active
            .turn_id
            .take()
            .map(|s| s.map(|s| bounded(&s, "turnId")));
        active.identity.0 = active.identity.0.take().map(|s| bounded(&s, ""));
    }
    if let Some(Some(seen)) = &mut state.seen {
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
            if let Some(Some(state)) = &record.radio {
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
