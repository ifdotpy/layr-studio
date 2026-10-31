//! Stored room fields shared by room writers and projections.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stored room field marking retirement with a Unix timestamp in seconds.
pub const PROJECT_DELETED_AT: &str = "projectDeletedAt";

/// Keys retained when legacy open room records are migrated.
pub const FIELDS: &[&str] = &[
    "id",
    "kind",
    "members",
    "rootId",
    "projectId",
    "projectPath",
    "customName",
    "name",
    "created",
    "updated",
    "userHidden",
    PROJECT_DELETED_AT,
    "radio",
    "federation",
    "peerId",
    "peerLabel",
    "status",
    "localMembers",
    "peerTeamId",
    "peerTeamName",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoomKind {
    Private,
    Broadcast,
    Federated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoomStatus {
    Pending,
    Approved,
    Revoked,
}

pub type RoomRadioIdentity = (
    Option<String>,
    Option<crate::recovery_record::SafeInteger>,
    crate::recovery_record::SafeInteger,
);

fn radio_identity_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type":"array", "minItems":3, "maxItems":3,
        "prefixItems":[
            {"type":["string","null"]},
            {"type":["integer","null"],"minimum":0,"maximum":crate::shared_values::MAX_SAFE_INTEGER},
            {"type":"integer","minimum":0,"maximum":crate::shared_values::MAX_SAFE_INTEGER}
        ]
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RoomRadioSeen {
    #[schemars(schema_with = "radio_identity_schema")]
    pub identity: RoomRadioIdentity,
    #[schemars(with = "u64", range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub seq: crate::recovery_record::SafeInteger,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomLastMessage {
    pub seq: ChatSequence,
    pub text: String,
    pub sender: String,
    pub created: serde_json::Number,
}

/// Sequence of a persisted chat message; the first message has sequence 1.
pub const CHAT_SEQUENCE_MIN: i64 = 1;
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ChatSequence(crate::recovery_record::SafeInteger);
impl ChatSequence {
    pub fn from_i64(value: i64) -> Option<Self> {
        (value >= CHAT_SEQUENCE_MIN)
            .then(|| crate::recovery_record::SafeInteger::from_i64(value))
            .flatten()
            .map(Self)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Waiting,
    Speaking,
    Blocked,
    Stopping,
}
#[derive(Clone, Debug, Serialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Active {
    pub event_id: String,
    pub agent_id: String,
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub epoch: u64,
    pub thread_id: Option<String>,
    #[schemars(schema_with = "radio_identity_schema")]
    pub identity: RoomRadioIdentity,
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub through: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_requested: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_continuation_planned: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "InterruptState")]
    pub interrupt_state: Option<InterruptState>,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, serde_json::Value>")]
    pub extra: BTreeMap<String, Opaque>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InterruptState {
    Pending,
    Unknown,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Opaque(Box<serde_json::value::RawValue>);
impl PartialEq for Opaque {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}
impl<'de> Deserialize<'de> for Opaque {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Box::<serde_json::value::RawValue>::deserialize(d).map(Self)
    }
}
type Fields = BTreeMap<String, Opaque>;
struct StrictFields(Fields);
impl<'de> Deserialize<'de> for StrictFields {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StrictFields;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a radio object with unique fields")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut fields = Fields::new();
                while let Some((key, value)) = map.next_entry::<String, Opaque>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate radio field"));
                    }
                }
                Ok(StrictFields(fields))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
fn required<T: serde::de::DeserializeOwned, E: serde::de::Error>(
    fields: &mut Fields,
    key: &'static str,
) -> Result<T, E> {
    let raw = fields.remove(key).ok_or_else(|| E::missing_field(key))?;
    serde_json::from_str(raw.0.get()).map_err(E::custom)
}
fn optional<T: serde::de::DeserializeOwned, E: serde::de::Error>(
    fields: &mut Fields,
    key: &str,
) -> Result<Option<T>, E> {
    fields
        .remove(key)
        .map(|raw| serde_json::from_str(raw.0.get()).map_err(E::custom))
        .transpose()
}
fn required_safe<E: serde::de::Error>(fields: &mut Fields, key: &'static str) -> Result<u64, E> {
    let value: u64 = required(fields, key)?;
    if value > crate::shared_values::MAX_SAFE_INTEGER as u64 {
        return Err(E::custom("radio integer exceeds the safe range"));
    }
    Ok(value)
}
impl<'de> Deserialize<'de> for Active {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut fields = StrictFields::deserialize(d)?.0;
        Ok(Self {
            event_id: required(&mut fields, "eventId")?,
            agent_id: required(&mut fields, "agentId")?,
            epoch: required_safe(&mut fields, "epoch")?,
            thread_id: required(&mut fields, "threadId")?,
            identity: required(&mut fields, "identity")?,
            through: required_safe(&mut fields, "through")?,
            turn_id: optional(&mut fields, "turnId")?,
            interrupt_requested: optional(&mut fields, "interruptRequested")?,
            question_continuation_planned: optional(&mut fields, "questionContinuationPlanned")?,
            interrupt_state: optional(&mut fields, "interruptState")?,
            extra: fields,
        })
    }
}
#[derive(Clone, Debug, Serialize, schemars::JsonSchema, PartialEq)]
pub struct Radio {
    #[serde(rename = "teamId")]
    pub team_id: String,
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub revision: u64,
    pub status: Status,
    #[schemars(required)]
    pub speaker: Option<String>,
    pub next: Vec<String>,
    pub direct: bool,
    #[schemars(required)]
    pub active: Option<Active>,
    #[schemars(required)]
    pub error: Option<String>,
    pub seen: BTreeMap<String, RoomRadioSeen>,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, serde_json::Value>")]
    pub extra: Fields,
}
impl<'de> Deserialize<'de> for Radio {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut fields = StrictFields::deserialize(d)?.0;
        Ok(Self {
            team_id: required(&mut fields, "teamId")?,
            revision: required_safe(&mut fields, "revision")?,
            status: required(&mut fields, "status")?,
            speaker: required(&mut fields, "speaker")?,
            next: required(&mut fields, "next")?,
            direct: required(&mut fields, "direct")?,
            active: required(&mut fields, "active")?,
            error: required(&mut fields, "error")?,
            seen: required(&mut fields, "seen")?,
            extra: fields,
        })
    }
}
impl Radio {
    pub fn new(team_id: String, direct: bool) -> Self {
        Self {
            team_id,
            revision: 0,
            status: Status::Idle,
            speaker: None,
            next: Vec::new(),
            direct,
            active: None,
            error: None,
            seen: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}
/// Stored room fields shared by writers and projections.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "PersistedRoomRecord")]
pub struct RoomRecord {
    pub id: String,
    pub kind: RoomKind,
    #[serde(
        default,
        deserialize_with = "present_nonnull",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Vec<String>")]
    pub members: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_members: Option<Option<Vec<String>>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub peer_team_id: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub peer_team_name: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub root_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub project_path: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<serde_json::Number>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated: Option<Option<serde_json::Number>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub user_hidden: Option<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_deleted_at: Option<serde_json::Number>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub radio: Option<Option<Radio>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub federation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub peer_label: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RoomStatus>,
}

pub(crate) fn schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(RoomRecord).to_value();
    for name in ["speaker", "active", "error"] {
        let field = &mut schema["$defs"]["Radio"]["properties"][name];
        *field = serde_json::json!({"anyOf":[field.clone(),{"type":"null"}]});
    }
    // Active's generated object always contains the non-optional fields above.
    let required = schema["$defs"]["Radio"]["properties"]["active"]["anyOf"][0]["required"]
        .as_array_mut()
        .expect("generated active schema has required fields");
    required.retain(|field| field != "interruptState");
    required.push(serde_json::json!("threadId"));
    schema
}

pub(crate) fn check_schema_parity() -> Result<(), String> {
    let stored: serde_json::Value =
        serde_json::from_slice(include_bytes!("schemas/persisted-room-record.schema.json"))
            .map_err(|error| error.to_string())?;
    if stored != schema() {
        return Err("persisted room schema must be generated from room_record::RoomRecord".into());
    }
    Ok(())
}

impl RoomRecord {
    pub fn new(id: String, kind: RoomKind) -> Self {
        Self {
            id,
            kind,
            members: None,
            local_members: None,
            peer_team_id: None,
            peer_team_name: None,
            root_id: None,
            project_id: None,
            project_path: None,
            custom_name: None,
            name: None,
            created: None,
            updated: None,
            user_hidden: None,
            project_deleted_at: None,
            radio: None,
            federation: None,
            peer_id: None,
            peer_label: None,
            status: None,
        }
    }
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

fn present_nonnull<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_record_rejects_unknown_fields_and_preserves_radio() {
        let raw = r#"{"id":"radio:a","kind":"private","members":["a"],"radio":{"teamId":"a","revision":0,"status":"idle","speaker":null,"next":[],"direct":true,"active":null,"error":null,"seen":{},"future":{"x":1}}}"#;
        let room: RoomRecord = serde_json::from_str(raw).unwrap();
        let encoded = serde_json::to_string(&room).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            serde_json::from_str::<serde_json::Value>(raw).unwrap()
        );
        assert!(
            serde_json::from_str::<RoomRecord>(r#"{"id":"r","kind":"private","extra":1}"#).is_err()
        );
        assert!(serde_json::from_str::<RoomRecord>(r#"{"id":"r","kind":"other"}"#).is_err());
    }

    #[test]
    fn migration_keys_match_record_schema() {
        let schema = schema();
        let properties = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let fields = FIELDS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(properties, fields);
        check_schema_parity().unwrap();
        let chat_schema: serde_json::Value =
            serde_json::from_str(include_str!("schemas/persisted-chat-message.schema.json"))
                .unwrap();
        assert_eq!(
            chat_schema["properties"]["seq"]["minimum"],
            CHAT_SEQUENCE_MIN
        );
    }
}
