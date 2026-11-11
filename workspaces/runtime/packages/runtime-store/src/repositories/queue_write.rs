//! Queue event mutations used by the queue-write service.

use rusqlite::{Transaction, params};
use studio_json::Value;

use crate::records::{EventMetadataRecord, QueueEventId, Table};
use crate::{
    Connection, StoreResult,
    schema::{SchemaRequirements, TableRequirement},
};

const EVENT_TABLE: &[TableRequirement] = &[TableRequirement {
    name: "runtime_events",
    columns: &[
        "id", "agent", "kind", "text", "status", "created", "epoch", "turn_id", "error",
    ],
}];
const META_TABLE: &[TableRequirement] = &[TableRequirement {
    name: "runtime_event_meta",
    columns: &["id", "record"],
}];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[EVENT_TABLE[0]],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[META_TABLE[0]],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Debug, PartialEq)]
pub struct PendingEvent {
    pub id: String,
    pub kind: String,
    pub created: f64,
    pub valid: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct QueueWriteRepository;

/// Only the durable fields consumed by the queue projection.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueProjectionOwner {
    pub id: crate::AgentId,
    #[serde(deserialize_with = "crate::records::deserialize_agent_epoch")]
    pub epoch: i64,
    pub deleted_at: Option<f64>,
    #[serde(
        default,
        deserialize_with = "crate::records::deserialize_optional_safe_counter"
    )]
    pub queue_mutation_revision: Option<u64>,
}

impl QueueWriteRepository {
    pub fn projection_owner(
        connection: &Connection,
        id: &str,
    ) -> StoreResult<Option<QueueProjectionOwner>> {
        let raw = crate::peer_update_settings::canvas_read(connection).named(
            &queries::ProjectionOwner::QUERY,
            &queries::ProjectionOwner { id },
        )?;
        raw.map(|raw| {
            let owner: QueueProjectionOwner = studio_json::from_str(&raw).map_err(|_| {
                crate::StoreError::InvalidStoredObject {
                    table: "runtime_agents",
                    id: id.to_owned(),
                }
            })?;
            if owner.id.0 != id {
                return Err(crate::StoreError::InvalidStoredObject {
                    table: "runtime_agents",
                    id: id.to_owned(),
                });
            }
            Ok(owner)
        })
        .transpose()
    }
    pub fn pending_events(
        transaction: &Transaction<'_>,
        agent: &str,
        epoch: i64,
    ) -> StoreResult<Vec<PendingEvent>> {
        let mut statement = transaction.prepare_cached(
            "SELECT id,kind,created FROM runtime_events WHERE agent=?1 AND epoch=?2 AND status='pending' ORDER BY created,rowid",
        )?;
        let rows = statement.query_map(params![agent, epoch], |row| {
            let (id, id_valid) = row.get::<_, String>(0).map_or_else(
                |_| ("<invalid id>".to_owned(), false),
                |value| (value, true),
            );
            let (kind, kind_valid) = row
                .get::<_, String>(1)
                .map_or_else(|_| (String::new(), false), |value| (value, true));
            let (created, created_valid) = row
                .get::<_, f64>(2)
                .map_or_else(|_| (f64::NAN, false), |value| (value, value.is_finite()));
            Ok(PendingEvent {
                id,
                kind,
                created,
                valid: id_valid && kind_valid && created_valid,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_created(
        transaction: &Transaction<'_>,
        id: &str,
        created: f64,
    ) -> StoreResult<()> {
        transaction.execute(
            "UPDATE runtime_events SET created=?1 WHERE id=?2",
            params![created, id],
        )?;
        Ok(())
    }

    pub fn update_status(transaction: &Transaction<'_>, id: &str, status: &str) -> StoreResult<()> {
        transaction.execute(
            "UPDATE runtime_events SET status=?1 WHERE id=?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn update_text(transaction: &Transaction<'_>, id: &str, text: &str) -> StoreResult<()> {
        transaction.execute(
            "UPDATE runtime_events SET text=?1 WHERE id=?2",
            params![text, id],
        )?;
        Ok(())
    }

    pub fn metadata(transaction: &Transaction<'_>, id: &str) -> StoreResult<Option<Value>> {
        Ok(Self::typed_metadata(transaction, id)?.map(EventMetadataRecord::into_value))
    }

    pub fn typed_metadata(
        transaction: &Transaction<'_>,
        id: &str,
    ) -> StoreResult<Option<EventMetadataRecord>> {
        Table::<EventMetadataRecord>::new(transaction).get(&QueueEventId::from(id))
    }

    pub fn put_typed_metadata(
        transaction: &Transaction<'_>,
        id: &str,
        metadata: &EventMetadataRecord,
    ) -> StoreResult<()> {
        Table::<EventMetadataRecord>::new(transaction).put_at(&QueueEventId::from(id), metadata)
    }

    pub fn put_metadata(
        transaction: &Transaction<'_>,
        id: &str,
        metadata: &Value,
    ) -> StoreResult<()> {
        let key = QueueEventId::from(id);
        let record = EventMetadataRecord::from_value(metadata.clone()).map_err(|_| {
            crate::StoreError::InvalidStoredObject {
                table: "runtime_event_meta",
                id: id.to_owned(),
            }
        })?;
        Table::<EventMetadataRecord>::new(transaction).put_at(&key, &record)
    }
}

#[studio_db_api::db_queries]
mod queries {
    use crate::agent_tables::AgentsStorageRow;
    use crate::peer_update_settings::Canvas;
    use studio_db_api::DbQuery;

    #[derive(DbQuery)]
    #[db_query(database=Canvas, mode=read, cardinality=optional, result=String,
        decode=AgentsStorageRow::ID, tables(agents=AgentsStorageRow),
        columns(id=AgentsStorageRow::ID, record=AgentsStorageRow::RECORD),
        sql="SELECT json_object('id',{record}->'$.id','epoch',{record}->'$.epoch',
            'deletedAt',{record}->'$.deletedAt','queueMutationRevision',{record}->'$.queueMutationRevision') AS {id}
            FROM {agents} WHERE {id}=?1")]
    pub struct ProjectionOwner<'a> {
        #[db(column=AgentsStorageRow::ID)]
        pub id: &'a str,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_projection_fields_preserve_numeric_contracts() {
        let mut db = crate::test_canvas();
        queries::validate_queries(&db).unwrap();
        for (field, value, valid) in [
            ("epoch", Some("1"), true),
            ("epoch", Some("-1"), false),
            ("epoch", Some("1.0"), false),
            ("epoch", Some("true"), false),
            ("epoch", Some("null"), false),
            ("epoch", None, false),
            ("queueMutationRevision", Some("2"), true),
            ("queueMutationRevision", Some("-1"), false),
            ("queueMutationRevision", Some("9007199254740992"), false),
            ("queueMutationRevision", Some("false"), false),
            ("queueMutationRevision", Some("null"), true),
            ("queueMutationRevision", None, true),
            ("deletedAt", Some("1"), true),
            ("deletedAt", Some("true"), false),
            ("deletedAt", Some("null"), true),
            ("deletedAt", None, true),
        ] {
            let mut record = studio_json::json!({"id":"agent","rootId":"agent","epoch":1,"provider":"codex","accountKey":"default","status":"idle","compactions":0});
            if let Some(raw) = value {
                record[field] = studio_json::from_str::<Value>(raw).unwrap();
            } else {
                record.as_object_mut().unwrap().remove(field);
            }
            let tx = db.transaction().unwrap();
            crate::peer_update_settings::canvas_tx(&tx)
                .named(
                    &RawAgent::QUERY,
                    &RawAgent {
                        id: "agent",
                        record: &record.to_string(),
                    },
                )
                .unwrap();
            assert_eq!(
                QueueWriteRepository::projection_owner(&tx, "agent").is_ok(),
                valid,
                "{record}"
            );
        }
    }

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database=crate::peer_update_settings::Canvas,mode=write,cardinality=none,result=(),
        tables(agents=crate::agent_tables::AgentsStorageRow),
        columns(id=crate::agent_tables::AgentsStorageRow::ID,record=crate::agent_tables::AgentsStorageRow::RECORD),
        sql="INSERT INTO {agents}({id},{record}) VALUES(?1,?2)")]
    struct RawAgent<'a> {
        #[db(column=crate::agent_tables::AgentsStorageRow::ID)]
        id: &'a str,
        #[db(column=crate::agent_tables::AgentsStorageRow::ID)]
        record: &'a str,
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "queue_write",
        requirements: &REQUIREMENTS,
    }
}
