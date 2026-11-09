//! Wave-two interaction descriptors. Legacy SQL retains the physical format.
use crate::json_record_codec::{ExactJson, ExactJsonCodec, JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use crate::{ComplaintRecord, RequestRecord, StoreResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_db_api::DbTable;
use studio_runtime_schema::room_record::RoomRecord;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupsMembersPayload(pub Vec<String>);

/// Recipient keys are open; historical non-status entries remain original evidence.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatDelivery {
    Status(studio_runtime_schema::persistence::DeliveryStatus),
    Recipient(studio_runtime_schema::persistence::RecipientDelivery),
    Legacy(studio_runtime_schema::raw_json::RawJson),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatDeliveriesPayload {
    Recipients(BTreeMap<String, ChatDelivery>),
    Legacy(studio_runtime_schema::raw_json::RawJson),
}

#[derive(DbTable)]
#[db(table = "graph_edges", database = Canvas, legacy_ddl = "CREATE TABLE graph_edges (\n                  id TEXT PRIMARY KEY, source TEXT NOT NULL, target TEXT NOT NULL,\n                  kind TEXT NOT NULL, UNIQUE(source, target, kind))")]
#[db(state_check = crate::state_check::count_legacy_rows::<GraphEdgesRow>)]
pub struct GraphEdgesRow {
    #[db(key)]
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
}

#[derive(DbTable)]
#[db(table = "groups", database = Canvas, legacy_ddl = "CREATE TABLE groups (\n                  id TEXT PRIMARY KEY, name TEXT NOT NULL, members TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<GroupsRow>)]
pub struct GroupsRow {
    #[db(key)]
    pub id: String,
    pub name: String,
    #[db(codec = ExactJsonCodec<GroupsMembersPayload>)]
    pub members: ExactJson<GroupsMembersPayload>,
}

#[derive(DbTable)]
#[db(table = "messages", database = Canvas, legacy_ddl = "CREATE TABLE messages (\n                  id TEXT PRIMARY KEY, room TEXT NOT NULL, author TEXT NOT NULL,\n                  text TEXT NOT NULL, at REAL NOT NULL, deliveries TEXT NOT NULL)")]
#[db(legacy_index = "CREATE INDEX messages_room ON messages(room, at)")]
#[db(state_check = crate::state_check::count_legacy_rows::<MessagesRow>)]
pub struct MessagesRow {
    #[db(key)]
    pub id: String,
    pub room: String,
    pub author: String,
    pub text: String,
    pub at: f64,
    #[db(codec = ExactJsonCodec<ChatDeliveriesPayload>)]
    pub deliveries: ExactJson<ChatDeliveriesPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_chat_messages", database = Canvas, legacy_ddl = "CREATE TABLE runtime_chat_messages (\n                  seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,\n                  room TEXT NOT NULL, sender TEXT NOT NULL, text TEXT NOT NULL,\n                  created REAL NOT NULL, deliveries TEXT NOT NULL, rev INTEGER NOT NULL DEFAULT 1)")]
#[db(legacy_index = "CREATE INDEX runtime_chat_room ON runtime_chat_messages(room, seq)")]
#[db(state_check = crate::state_check::check_legacy_chat)]
pub struct ChatMessagesRow {
    #[db(generated)]
    pub seq: i64,
    #[db(key)]
    pub id: String,
    pub room: String,
    pub sender: String,
    pub text: String,
    pub created: f64,
    #[db(codec = ExactJsonCodec<ChatDeliveriesPayload>)]
    pub deliveries: ExactJson<ChatDeliveriesPayload>,
    #[db(generated)]
    pub rev: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_requests", database = Canvas, legacy_ddl = "CREATE TABLE runtime_requests (id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_runtime_requests)]
#[db(
    legacy_index = "CREATE INDEX runtime_request_agent_status ON runtime_requests(json_extract(record,'$.agent'),json_extract(record,'$.status'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_request_native_question ON runtime_requests(\n                    json_extract(record,'$.agent'),json_extract(record,'$.params.itemId'),\n                    json_extract(record,'$.status'))\n                    WHERE json_valid(record) AND json_extract(record,'$.method')='item/tool/requestUserInput'"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_request_startup_unsettled ON runtime_requests(id) WHERE CASE WHEN json_valid(record) THEN json_extract(record,'$.status') IN ('answering','pending') ELSE 0 END"
)]
#[db(state_check = crate::state_check::check_legacy_json::<RequestRecord>)]
pub struct RequestRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<RequestRecord>)]
    pub record: RawJsonRecord<RequestRecord>,
}

fn check_runtime_requests(row: &RequestRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_rooms", database = Canvas, legacy_ddl = "CREATE TABLE runtime_rooms (id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_runtime_rooms)]
#[db(
    legacy_index = "CREATE INDEX runtime_room_many_members ON runtime_rooms(json_array_length(record,'$.members')) WHERE json_array_length(record,'$.members')>2"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_room_member_first ON runtime_rooms(json_extract(record,'$.members[0]'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_room_member_second ON runtime_rooms(json_extract(record,'$.members[1]'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_room_root ON runtime_rooms(json_extract(record,'$.rootId'))"
)]
#[db(state_check = crate::state_check::check_legacy_rooms)]
pub struct RoomRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<RoomRecord>)]
    pub record: RawJsonRecord<RoomRecord>,
}

fn check_runtime_rooms(row: &RoomRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

#[derive(DbTable)]
#[db(table = "runtime_complaints", database = Canvas, legacy_ddl = "CREATE TABLE runtime_complaints (id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_runtime_complaints)]
#[db(
    legacy_index = "CREATE INDEX runtime_conversion_complaints_leadId ON runtime_complaints(json_extract(record,'$.leadId')) WHERE json_extract(record,'$.leadId') IS NOT NULL"
)]
#[db(state_check = crate::state_check::check_legacy_json::<ComplaintRecord>)]
pub struct ComplaintRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<ComplaintRecord>)]
    pub record: RawJsonRecord<ComplaintRecord>,
}

fn check_runtime_complaints(row: &ComplaintRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

impl ChatDelivery {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Status(status) => Some(status.as_str()),
            Self::Recipient(_) | Self::Legacy(_) => None,
        }
    }
}
impl ChatDeliveriesPayload {
    pub fn get(&self, recipient: &str) -> Option<&ChatDelivery> {
        match self {
            Self::Recipients(values) => values.get(recipient),
            Self::Legacy(_) => None,
        }
    }
    pub fn get_mut(&mut self, recipient: &str) -> Option<&mut ChatDelivery> {
        match self {
            Self::Recipients(values) => values.get_mut(recipient),
            Self::Legacy(_) => None,
        }
    }
    pub fn set(&mut self, recipient: String, delivery: ChatDelivery) {
        if let Self::Recipients(values) = self {
            values.insert(recipient, delivery);
        }
    }
}

use studio_runtime_schema::approvals::{Answer, Delivery, Detail, Principal, Request};
#[derive(DbTable)]
#[db(table = "approvals", database = Canvas, legacy_ddl = "CREATE TABLE approvals (\n            id TEXT PRIMARY KEY, agent TEXT NOT NULL, turn TEXT, created REAL NOT NULL, epoch INTEGER,\n            kind TEXT NOT NULL, request TEXT NOT NULL, delivery TEXT, detail TEXT NOT NULL,\n            state TEXT NOT NULL,\n            answer TEXT, command TEXT, answered_by TEXT, at REAL, since REAL, reason TEXT,\n            deferral_active INTEGER, deferral_at REAL, deferral_by TEXT,\n            fire_at REAL, hidden_at REAL, hidden_by TEXT)")]
#[db(legacy_index = "CREATE INDEX approvals_turn ON approvals(turn,state)")]
#[db(
    legacy_index = "CREATE INDEX approvals_agent_open ON approvals(agent) WHERE state IN ('pending','answering')"
)]
#[db(
    legacy_index = "CREATE INDEX approvals_account_open ON approvals(json_extract(delivery,'$.account')) WHERE state IN ('pending','answering')"
)]
#[db(
    legacy_index = "CREATE INDEX approvals_fire_at ON approvals(fire_at) WHERE fire_at IS NOT NULL"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<ApprovalsRow>)]
pub struct ApprovalsRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    pub turn: Option<String>,
    pub created: f64,
    pub epoch: Option<i64>,
    pub kind: String,
    #[db(codec = ExactJsonCodec<Request>)]
    pub request: ExactJson<Request>,
    #[db(codec = ExactJsonCodec<Delivery>)]
    pub delivery: Option<ExactJson<Delivery>>,
    #[db(codec = ExactJsonCodec<Detail>)]
    pub detail: ExactJson<Detail>,
    pub state: String,
    #[db(codec = ExactJsonCodec<Answer>)]
    pub answer: Option<ExactJson<Answer>>,
    pub command: Option<String>,
    #[db(codec = ExactJsonCodec<Principal>)]
    pub answered_by: Option<ExactJson<Principal>>,
    pub at: Option<f64>,
    pub since: Option<f64>,
    pub reason: Option<String>,
    pub deferral_active: Option<i64>,
    pub deferral_at: Option<f64>,
    #[db(codec = ExactJsonCodec<Principal>)]
    pub deferral_by: Option<ExactJson<Principal>>,
    pub fire_at: Option<f64>,
    pub hidden_at: Option<f64>,
    #[db(codec = ExactJsonCodec<Principal>)]
    pub hidden_by: Option<ExactJson<Principal>>,
}

impl<'de> Deserialize<'de> for ChatDelivery {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = studio_json::OwnedRawJson::deserialize(d)?;
        if let Ok(status) = studio_json::from_str::<
            studio_runtime_schema::persistence::DeliveryStatus,
        >(raw.as_str())
        {
            return Ok(Self::Status(status));
        }
        if studio_json::from_str::<BTreeMap<String, studio_json::OwnedRawJson>>(raw.as_str())
            .is_ok_and(|fields| fields.contains_key("error"))
            && let Ok(recipient) = studio_json::from_str(raw.as_str())
        {
            return Ok(Self::Recipient(recipient));
        }
        Ok(Self::Legacy(studio_runtime_schema::raw_json::RawJson(raw)))
    }
}
impl<'de> Deserialize<'de> for ChatDeliveriesPayload {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = studio_json::OwnedRawJson::deserialize(d)?;
        match studio_json::from_str(raw.as_str()) {
            Ok(recipients) => Ok(Self::Recipients(recipients)),
            Err(_) => Ok(Self::Legacy(studio_runtime_schema::raw_json::RawJson(raw))),
        }
    }
}

impl Serialize for ChatDelivery {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Status(status) => status.serialize(serializer),
            Self::Legacy(raw) => raw.serialize(serializer),
            Self::Recipient(recipient) => {
                // Value-based delivery writers emitted object fields in lexical order.
                use serde::ser::SerializeMap;
                let mut object = serializer.serialize_map(Some(2))?;
                object.serialize_entry("error", &recipient.error)?;
                object.serialize_entry("status", &recipient.status)?;
                object.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    #[test]
    fn interaction_descriptors_cover_repository_validator_columns() {
        for descriptor in [
            GraphEdgesRow::descriptor(),
            GroupsRow::descriptor(),
            MessagesRow::descriptor(),
            ChatMessagesRow::descriptor(),
            RequestRow::descriptor(),
            RoomRow::descriptor(),
            ComplaintRow::descriptor(),
        ] {
            // Every repository validator consumes physical names owned by this descriptor.
            for registration in crate::repositories::schema_registrations() {
                let requirements = registration.requirements;
                for table in requirements
                    .tables
                    .iter()
                    .chain(requirements.bootstrap_tables)
                    .chain(
                        requirements
                            .conditional_tables
                            .iter()
                            .flat_map(|conditional| conditional.tables),
                    )
                {
                    if table.name == descriptor.name {
                        for column in table.columns {
                            assert!(
                                descriptor.columns.iter().any(|owner| owner.name == *column),
                                "{} validator references {}.{} outside its descriptor owner",
                                registration.repository,
                                descriptor.name,
                                column
                            );
                        }
                    }
                }
            }
        }
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::approvals::create_schema(&db).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT sql FROM sqlite_schema WHERE name='approvals'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            ApprovalsRow::DDL
        );
        for index in ApprovalsRow::INDEXES {
            assert_eq!(
                db.query_row(
                    "SELECT sql FROM sqlite_schema WHERE name=?1",
                    [index.name],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                index.sql
            );
        }
    }

    #[test]
    fn interaction_room_callers_keep_record_bytes_and_rowid_order() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(RoomRow::DDL).unwrap();
        let tx = db.transaction().unwrap();
        let first = RoomRecord::new(
            "z".into(),
            studio_runtime_schema::room_record::RoomKind::Private,
        );
        crate::Table::new(&tx).put(&first).unwrap();
        let legacy: String = tx
            .query_row("SELECT record FROM runtime_rooms WHERE id='z'", [], |row| {
                row.get(0)
            })
            .unwrap();
        RoomRow::put(&tx, &first).unwrap();
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_rooms WHERE id='z'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            legacy
        );
        RoomRow::put(
            &tx,
            &RoomRecord::new(
                "a".into(),
                studio_runtime_schema::room_record::RoomKind::Private,
            ),
        )
        .unwrap();
        assert_eq!(
            RoomRow::list(&tx)
                .unwrap()
                .into_iter()
                .map(|room| room.id)
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
        assert_eq!(RoomRow::get(&tx, "z").unwrap().unwrap().id, "z");
    }

    #[test]
    fn interaction_registry_keeps_previously_unchecked_tables_count_only() {
        for (descriptor, fixture) in [
            (
                GraphEdgesRow::descriptor(),
                "INSERT INTO graph_edges VALUES(NULL,'a','b','historical')",
            ),
            (
                GroupsRow::descriptor(),
                "INSERT INTO groups VALUES(NULL,'g','not json')",
            ),
            (
                MessagesRow::descriptor(),
                "INSERT INTO messages VALUES(NULL,'r','a','body',1.125,'not json')",
            ),
            (
                ApprovalsRow::descriptor(),
                "INSERT INTO approvals(id,agent,created,kind,request,detail,state) VALUES(NULL,'a',1.125,'historical','not json','not json','historical')",
            ),
        ] {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch(descriptor.ddl).unwrap();
            db.execute_batch(fixture).unwrap();
            let registration = crate::canvas_schema::registry()
                .into_iter()
                .find(|owner| owner.descriptor.name == descriptor.name)
                .unwrap();
            let report = (registration.check)(&db).unwrap();
            assert_eq!(report.counts(), (1, 0), "{}", descriptor.name);
            assert!(report.failures.is_empty());
        }
    }

    #[test]
    fn interaction_delivery_projection_keeps_legacy_value_encoding() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(ChatMessagesRow::DDL).unwrap();
        db.execute_batch("CREATE TABLE runtime_events(id TEXT PRIMARY KEY, agent TEXT, kind TEXT, text TEXT, status TEXT, created REAL, epoch INTEGER, turn_id TEXT, error TEXT)").unwrap();
        let tx = db.transaction().unwrap();
        let raw = r#"{"agent":"queued","other":{"status":"pending","error":null}}"#;
        let message = crate::ChatMessage {
            sequence: 0,
            id: "m".into(),
            room: "r".into(),
            sender: "s".into(),
            text: "body".into(),
            created: 1.25,
            deliveries: ExactJson::from_stored(raw.into()).unwrap(),
        };
        crate::MessageRepository::put(&tx, &message).unwrap();
        crate::QueueEventRepository::put(
            &tx,
            &crate::QueueEvent {
                id: "chat:m:agent".into(),
                agent: "agent".into(),
                kind: "agent_message".into(),
                text: r#"{"message_id":"m","room":"r","sender":"s","text":"body"}"#.into(),
                status: "delivered".into(),
                created: 1.0,
                epoch: 1,
                turn_id: Some("t".into()),
                error: None,
            },
        )
        .unwrap();
        let mut legacy: studio_json::Value = studio_json::from_str(raw).unwrap();
        legacy["agent"] = studio_json::json!("delivered");
        crate::SchedulerRepository::project_delivered_chat_events(
            &tx,
            "agent",
            1,
            "t",
            &["chat:m:agent".into()],
        )
        .unwrap();
        let stored = crate::MessageRepository::get(&tx, &"m".into())
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.deliveries.raw(),
            studio_json::to_string(&legacy).unwrap()
        );
    }

    #[test]
    fn interaction_chat_crud_keeps_sequence_revision_and_fractional_seconds() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(ChatMessagesRow::DDL).unwrap();
        let catalog: studio_json::Value =
            studio_json::from_str(include_str!("runtime-state-schema.json")).unwrap();
        let revision = catalog["canvas.sqlite3"]["sqlite_master"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "runtime_chat_message_revision")
            .unwrap();
        db.execute_batch(revision["sql"].as_str().unwrap()).unwrap();
        let tx = db.transaction().unwrap();
        let mut message = crate::ChatMessage {
            sequence: 0,
            id: "m".into(),
            room: "r".into(),
            sender: "a".into(),
            text: "first".into(),
            created: 1.125,
            deliveries: ExactJson::from_stored(
                "  { \"a\": \"queued\", \"future\": true } \n".into(),
            )
            .unwrap(),
        };
        crate::MessageRepository::put(&tx, &message).unwrap();
        let first = crate::MessageRepository::get(&tx, &message.id)
            .unwrap()
            .unwrap();
        assert_eq!(first.sequence, 1);
        assert_eq!(first.created, 1.125);
        let original = first.deliveries.raw().to_owned();
        crate::MessageRepository::put(&tx, &first).unwrap();
        assert_eq!(
            crate::MessageRepository::get(&tx, &message.id)
                .unwrap()
                .unwrap()
                .deliveries
                .raw(),
            original
        );
        message.text = "second".into();
        crate::MessageRepository::put(&tx, &message).unwrap();
        assert_eq!(
            crate::MessageRepository::get(&tx, &message.id)
                .unwrap()
                .unwrap()
                .sequence,
            1
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*),rev FROM runtime_chat_messages",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            )
            .unwrap(),
            (1, 2)
        );
    }
}
