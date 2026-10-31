//! Checked radio records and transactional store projections.
use super::*;

pub(crate) use studio_runtime_schema::room_record::{Active, InterruptState, Radio, Status};
pub(super) struct Room {
    pub(super) record: studio_runtime_schema::room_record::RoomRecord,
    pub(super) radio: Radio,
}
impl Serialize for Room {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Stored<'a> {
            #[serde(flatten)]
            record: &'a studio_runtime_schema::room_record::RoomRecord,
            radio: &'a Radio,
        }
        Stored {
            record: &self.record,
            radio: &self.radio,
        }
        .serialize(serializer)
    }
}
impl Room {
    pub(super) fn checked(id: &str, raw: &str) -> Result<Self, ProjectsError> {
        let record: studio_runtime_schema::room_record::RoomRecord =
            serde_json::from_str(raw).map_err(|_| invalid(id))?;
        Self::from_record(id, record)
    }
    fn from_record(
        id: &str,
        mut record: studio_runtime_schema::room_record::RoomRecord,
    ) -> Result<Self, ProjectsError> {
        let radio = record.radio.take().flatten().ok_or_else(|| invalid(id))?;
        let room = Self { record, radio };
        if room.record.id != id {
            return Err(invalid(id));
        }
        if !id.starts_with("radio:") || room.radio.next.len() > 5 || room.radio.seen.len() > 2 {
            return Err(invalid(id));
        }
        Ok(room)
    }
    pub(super) fn id(&self) -> &str {
        &self.record.id
    }
    pub(super) fn members(&self) -> Vec<String> {
        self.record.members.clone().unwrap_or_default()
    }
    pub(super) fn blocked(&mut self, text: &str) {
        self.radio.status = Status::Blocked;
        self.radio.error = Some(text.to_owned());
        self.radio.next.clear();
    }
}
pub(super) fn invalid(id: &str) -> ProjectsError {
    ProjectsError::InvalidStored {
        table: "runtime_rooms",
        id: id.to_owned(),
    }
}
pub(super) fn rooms(db: &Connection) -> Result<Vec<Room>, ProjectsError> {
    if !records::exists(db, "runtime_rooms")? {
        return Ok(Vec::new());
    }
    let mut stmt = db.prepare("SELECT id,record FROM runtime_rooms ORDER BY rowid")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut rooms = Vec::new();
    for row in rows {
        let (id, raw) = row?;
        let record: studio_runtime_schema::room_record::RoomRecord =
            serde_json::from_str(&raw).map_err(|_| invalid(&id))?;
        if record.id != id {
            return Err(invalid(&id));
        }
        if id.starts_with("radio:") && record.radio.as_ref().and_then(Option::as_ref).is_some() {
            rooms.push(Room::from_record(&id, record)?);
        }
    }
    Ok(rooms)
}
pub(super) fn agents(db: &Connection) -> Result<BTreeMap<String, Value>, ProjectsError> {
    records::all(db, "runtime_agents")?
        .into_iter()
        .map(|a| {
            let id = a["id"]
                .as_str()
                .ok_or(ProjectsError::Invalid("stored agent id is invalid"))?
                .to_owned();
            Ok((id, a))
        })
        .collect()
}
pub(super) fn policy(
    db: &Connection,
    room: &Room,
    prepared: &PreparedHost,
    recipient: &str,
) -> Result<RadioAuthorizationSnapshot, ProjectsError> {
    let value = serde_json::to_string(room).map_err(studio_runtime_store::StoreError::from)?;
    let inputs = RadioAuthorizationRecords {
        recipient_id: if recipient.is_empty() {
            room.members()
                .first()
                .cloned()
                .ok_or_else(|| invalid(room.id()))?
        } else {
            recipient.to_owned()
        },
        rooms: BTreeMap::from([(room.id().to_owned(), value)]),
        agents: agents(db)?
            .into_iter()
            .map(|(id, a)| (id, a.to_string()))
            .collect(),
        projects: ProjectsRepository::list(db)?
            .into_iter()
            .map(|(id, p)| (id, p.to_string()))
            .collect(),
        resolved_paths: prepared.resolved_paths().clone(),
    };
    RadioAuthorizationSnapshot::from_records(&inputs).map_err(|e| ProjectsError::InvalidStored {
        table: "radio",
        id: e.location,
    })
}
pub(super) fn denial_text(denial: RadioDenial) -> &'static str {
    match denial {
        RadioDenial::ParticipantsNoLongerAvailable => {
            "The shared chat participants are no longer available in this project."
        }
        RadioDenial::MembershipChanged => {
            "The shared chat membership changed. Its history stays with its original members."
        }
        RadioDenial::ParticipantUnavailable => {
            "A participant is being transferred or its native chat is unavailable."
        }
        RadioDenial::ParticipantStopped => {
            "A participant is stopped. Resume that chat before sending."
        }
        RadioDenial::NoActiveRoomReceipt => "The shared turn has no active room receipt",
        RadioDenial::AmbiguousRoomReceipt => "The shared turn has multiple room receipts",
        RadioDenial::TurnNoLongerAuthorized => "The shared turn is no longer authorized",
    }
}
pub(super) fn validate(
    db: &Connection,
    room: &Room,
    prepared: &PreparedHost,
    ready: bool,
) -> Result<Option<&'static str>, ProjectsError> {
    Ok(policy(db, room, prepared, "")?
        .validate_room(room.id(), ready)
        .map(denial_text))
}
pub(super) fn save(
    db: &Transaction<'_>,
    room: &mut Room,
    prepared: &PreparedHost,
    now: f64,
) -> Result<(), ProjectsError> {
    room.record.updated = Some(serde_json::Number::from_f64(now));
    let raw = serde_json::to_string(&*room).map_err(studio_runtime_store::StoreError::from)?;
    db.execute("INSERT INTO runtime_rooms(id,record) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",params![room.id(),raw])?;
    publish_radio_room(db, room, prepared)
}
fn agent_projection(
    value: &Value,
) -> Result<crate::submit_gates::AgentAuthorityRow, ProjectsError> {
    use crate::submit_gates::{AgentAuthorityRow, Deletion, StoredField};
    let invalid = || ProjectsError::InvalidStored {
        table: "runtime_agents",
        id: value["id"].as_str().unwrap_or_default().to_owned(),
    };
    let string = |key: &str| match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        _ => Err(invalid()),
    };
    let deleted_at = match value.get("deletedAt") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(Deletion::Bool(*value)),
        Some(Value::Number(value)) => Some(Deletion::Number(value.as_f64().ok_or_else(invalid)?)),
        _ => return Err(invalid()),
    };
    let is_lead = match value.get("isLead") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(*value),
        _ => return Err(invalid()),
    };
    let name = match value.get("name") {
        None => StoredField::Missing,
        Some(Value::Null) => StoredField::Null,
        Some(Value::String(name)) => StoredField::Value(name.clone()),
        _ => return Err(invalid()),
    };
    Ok(AgentAuthorityRow {
        id: string("id")?,
        root_id: string("rootId")?,
        parent_id: string("parentId")?,
        is_lead,
        deleted_at,
        cwd: string("cwd")?,
        name,
        shared_room_id: string("sharedRoomId")?,
        execution_mode: string("executionMode")?,
    })
}
fn team_projection(value: &Value) -> Result<crate::submit_gates::PeerTeamRow, ProjectsError> {
    let invalid = || ProjectsError::InvalidStored {
        table: "runtime_projects",
        id: value["id"].as_str().unwrap_or_default().to_owned(),
    };
    Ok(crate::submit_gates::PeerTeamRow {
        id: value["id"].as_str().ok_or_else(invalid)?.to_owned(),
        name: value["name"].as_str().ok_or_else(invalid)?.to_owned(),
        members: value["members"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or_else(invalid))
            .collect::<Result<_, _>>()?,
    })
}
pub(super) fn publish_room(
    db: &Transaction<'_>,
    value: &studio_runtime_schema::room_record::RoomRecord,
    prepared: &PreparedHost,
) -> Result<(), ProjectsError> {
    let context = room_context(db, &value.id, prepared)?;
    let view = crate::room_projection::project_room(value, &context)
        .map_err(studio_runtime_store::StoreError::from)?;
    put_room_view(db, &value.id, view)
}
fn publish_radio_room(
    db: &Transaction<'_>,
    room: &Room,
    prepared: &PreparedHost,
) -> Result<(), ProjectsError> {
    let context = room_context(db, room.id(), prepared)?;
    let view = crate::room_projection::project_radio_room(&room.record, &context, &room.radio)
        .map_err(studio_runtime_store::StoreError::from)?;
    put_room_view(db, room.id(), view)
}
fn room_context(
    db: &Transaction<'_>,
    id: &str,
    prepared: &PreparedHost,
) -> Result<RoomViewContext, ProjectsError> {
    let all_agents = project_agents(db)?;
    let mut teams = Vec::new();
    for (_, p) in ProjectsRepository::list(db)? {
        if let (Some(path), Some(raw)) = (p["path"].as_str(), p["peerTeams"].as_array()) {
            teams.extend(live_teams(
                raw,
                &all_agents,
                prepared.resolve(path)?,
                prepared,
            )?);
        }
    }
    let last = db.query_row("SELECT seq,text,created,sender FROM runtime_chat_messages WHERE room=?1 ORDER BY seq DESC LIMIT 1",[id],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,f64>(2)?,r.get::<_,String>(3)?))).optional()?;
    let last = last
        .map(|(seq, text, created, sender)| {
            Ok::<_, ProjectsError>(studio_runtime_schema::room_record::RoomLastMessage {
                seq: studio_runtime_schema::room_record::ChatSequence::from_i64(seq)
                    .ok_or_else(|| invalid(id))?,
                text: text.chars().take(180).collect(),
                created: serde_json::Number::from_f64(created).ok_or_else(|| invalid(id))?,
                sender,
            })
        })
        .transpose()?;
    let agents = all_agents
        .into_values()
        .map(|value| agent_projection(&value))
        .collect::<Result<_, _>>()?;
    let teams = teams
        .into_iter()
        .map(|value| team_projection(&value))
        .collect::<Result<_, _>>()?;
    let mut context = RoomViewContext::new(agents, teams);
    context.last_message = last;
    Ok(context)
}
fn put_room_view(
    db: &Transaction<'_>,
    id: &str,
    view: Option<crate::room_projection::RoomProjection>,
) -> Result<(), ProjectsError> {
    match view {
        Some(view) => {
            SyncDocumentsRepository::put_entity(db, "room", id, view, false)?;
        }
        None => {
            SyncDocumentsRepository::put_entity(db, "room", id, json!({}), true)?;
        }
    }
    Ok(())
}
pub(super) fn message(
    db: &Transaction<'_>,
    room: &Room,
    key: &str,
    sender: &str,
    text: &str,
    now: f64,
) -> Result<(), ProjectsError> {
    db.execute("INSERT INTO runtime_chat_messages(id,room,sender,text,created,deliveries) VALUES(?1,?2,?3,?4,?5,'{}') ON CONFLICT(id) DO UPDATE SET text=excluded.text",params![key,room.id(),sender,text,now])?;
    Ok(())
}
pub(super) fn event(db: &Connection, active: &Active) -> Result<Option<QueueEvent>, ProjectsError> {
    Ok(QueueEventRepository::get(
        db,
        &QueueEventId::from(active.event_id.as_str()),
    )?)
}
pub(super) fn turn(agent: &Value, active: &Active, event: Option<&QueueEvent>) -> Option<String> {
    let event = event?;
    if event.agent.0 != agent["id"] || u64::try_from(event.epoch).ok() != Some(active.epoch) {
        return None;
    }
    if let Some(id) = &event.turn_id {
        return Some(id.clone());
    }
    let attempt = &agent["startAttempt"];
    if attempt["events"] == json!([active.event_id])
        && attempt["epoch"] == active.epoch
        && truthy(attempt.get("submitted"))
        && attempt["threadId"] == agent["threadId"]
    {
        return attempt["turnId"]
            .as_str()
            .or_else(|| attempt["observedTurnId"].as_str())
            .map(str::to_owned);
    }
    None
}
pub(super) fn completed(db: &Connection, agent: &str, turn: &str) -> Result<bool, ProjectsError> {
    Ok(studio_runtime_store::domain::turn_completed(
        db, agent, turn,
    )?)
}
pub(super) fn pending_question(
    db: &Connection,
    agent: &str,
    turn: &str,
) -> Result<bool, ProjectsError> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM runtime_requests WHERE json_extract(record,'$.agent')=?1 AND json_extract(record,'$.turnId')=?2 AND json_extract(record,'$.method')='agent/asyncQuestion' AND json_extract(record,'$.status')='pending')",params![agent,turn],|r|r.get(0))?)
}
pub(super) fn unsent_cancelled(db: &Connection, active: &Active) -> Result<bool, ProjectsError> {
    let Some(event) = event(db, active)? else {
        return Ok(false);
    };
    if event.status != "cancelled"
        || event.turn_id.is_some()
        || active
            .turn_id
            .as_ref()
            .and_then(|v| v.as_deref())
            .is_some_and(|v| !v.is_empty())
    {
        return Ok(false);
    }
    let a = records::get(db, "runtime_agents", &active.agent_id)?.unwrap_or(Value::Null);
    let attempt = &a["startAttempt"];
    Ok(!attempt["events"]
        .as_array()
        .is_some_and(|ids| ids.contains(&json!(active.event_id)))
        || !["submitted", "observedTurnId", "turnId"]
            .iter()
            .any(|k| truthy(attempt.get(k))))
}
pub(super) fn cancel_pending(db: &Transaction<'_>, active: &Active) -> Result<bool, ProjectsError> {
    db.execute("UPDATE runtime_events SET status='cancelled',error='Shared chat stopped' WHERE id=?1 AND status='pending'",[&active.event_id])?;
    unsent_cancelled(db, active)
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    #[test]
    fn radio_requires_canonical_direct_and_active_fields() {
        let canonical = r#"{"teamId":"t","revision":0,"status":"idle","speaker":null,"next":[],"active":null,"error":null,"seen":{},"direct":false}"#;
        assert!(serde_json::from_str::<Radio>(canonical).is_ok());
        assert!(
            serde_json::from_str::<Radio>(&canonical.replace(",\"direct\":false", "")).is_err()
        );
        assert!(serde_json::from_str::<Radio>(&canonical.replace("\"active\":null,", "")).is_err());
    }

    #[test]
    fn projection_rows_match_typed_decoding_and_reject_invalid_fields() {
        for raw in [
            r#"{"id":"a","name":null,"deletedAt":0,"cwd":"/p"}"#,
            r#"{"id":"b","name":"B","isLead":true,"deletedAt":false}"#,
        ] {
            let value: Value = serde_json::from_str(raw).expect("projection JSON");
            let expected: crate::submit_gates::AgentAuthorityRow =
                serde_json::from_str(raw).expect("typed agent");
            assert_eq!(
                agent_projection(&value).expect("agent projection"),
                expected
            );
        }
        assert!(agent_projection(&json!({"id":"a","isLead":"yes"})).is_err());
        let raw = r#"{"id":"t","name":"Team","members":["a","b"]}"#;
        let value: Value = serde_json::from_str(raw).expect("projection JSON");
        let expected: crate::submit_gates::PeerTeamRow =
            serde_json::from_str(raw).expect("typed team");
        assert_eq!(team_projection(&value).expect("team projection"), expected);
        assert!(team_projection(&json!({"id":"t","name":"T","members":[null]})).is_err());
    }
}
