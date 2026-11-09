//! Atomic persistence primitives for event requests.

use crate::CachedSql;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use studio_json::Value;

use crate::records::{AnnotationRecord, EventMetadataRecord, JsonRecord, QueueEventId, Table};
use crate::schema::{SchemaRequirements, TableRequirement};
use crate::{QueueEvent, StoreError, StoreResult};

const TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: "runtime_annotations",
        columns: &["id", "record"],
    },
    TableRequirement {
        name: "request_results",
        columns: &["id", "signature", "result", "created"],
    },
    TableRequirement {
        name: "runtime_events",
        columns: &[
            "id", "agent", "kind", "text", "status", "created", "epoch", "turn_id", "error",
        ],
    },
    TableRequirement {
        name: "runtime_event_meta",
        columns: &["id", "record"],
    },
];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[TABLES[2]],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[TABLES[0], TABLES[1], TABLES[3]],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "event_write",
        requirements: &REQUIREMENTS,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EventWriteRepository;

impl EventWriteRepository {
    /// Finish the active capacity retry when a manual input replaces it.
    pub fn finish_capacity_retry(
        transaction: &Transaction<'_>,
        agent_id: &str,
        retry_id: &str,
    ) -> StoreResult<Option<bool>> {
        let exists: bool = transaction.query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_capacity_retries')",
            [],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(None);
        }
        let raw = transaction
            .query_row_cached(
                "SELECT record FROM runtime_capacity_retries WHERE id=?1 AND agent=?2",
                params![retry_id, agent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(Some(false));
        };
        let mut record: Value =
            studio_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredObject {
                table: "runtime_capacity_retries",
                id: retry_id.to_owned(),
            })?;
        if record.get("id").and_then(Value::as_str) != Some(retry_id)
            || !studio_runtime_schema::validate("continuation/CapacityContinuation", &record)
                .map_err(StoreError::SchemaValidation)?
        {
            return Err(StoreError::InvalidStoredObject {
                table: "runtime_capacity_retries",
                id: retry_id.to_owned(),
            });
        }
        record["status"] = Value::String("finished".to_owned());
        record["dueAt"] = Value::Null;
        record["reason"] = Value::String("A new instruction replaces this retry.".to_owned());
        transaction.execute_cached(
            "UPDATE runtime_capacity_retries SET record=?1 WHERE id=?2 AND agent=?3",
            params![record.to_string(), retry_id, agent_id],
        )?;
        Ok(Some(true))
    }

    pub fn create_annotation(
        transaction: &Transaction<'_>,
        record: crate::json_record_codec::RawJsonRecord<AnnotationRecord>,
    ) -> StoreResult<i64> {
        let row = crate::planning_work::AnnotationRow {
            id: record.record().id().clone(),
            record,
            rev: 1,
        };
        let mut tx = crate::peer_update_settings::canvas_tx(transaction);
        tx.insert(&row)?;
        Ok(tx
            .get::<crate::planning_work::AnnotationRow>(&(row.id,))?
            .ok_or(StoreError::RecordContract("Inserted annotation is missing"))?
            .rev)
    }

    pub fn put_annotation(
        connection: &Transaction<'_>,
        id: &str,
        value: &Value,
    ) -> StoreResult<()> {
        let record = AnnotationRecord::from_value(value.clone())?;
        if record.id() != id {
            return Err(crate::StoreError::InvalidStoredObject {
                table: AnnotationRecord::TABLE,
                id: id.to_owned(),
            });
        }
        if !crate::peer_update_settings::canvas_tx(connection).insert_new(
            &crate::planning_work::AnnotationRow {
                id: id.to_owned(),
                record: crate::json_record_codec::RawJsonRecord::from_record(connection, &record)?,
                rev: 1,
            },
        )? {
            return Err(crate::StoreError::RecordContract(
                "annotation record identity already exists",
            ));
        }
        Ok(())
    }

    pub fn event_exists(connection: &Connection, id: &str) -> StoreResult<bool> {
        Ok(connection.query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM runtime_events WHERE id=?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    /// Insert an event. The id is the dedupe key: a repeated id is a no-op
    /// (`Ok(false)`) and keeps the first row. Any other refused row, such as an
    /// unknown kind, is an error.
    pub fn insert_event(connection: &Connection, event: &QueueEvent) -> StoreResult<bool> {
        let inserted = connection.execute_cached(
            "INSERT INTO runtime_events \
             (id,agent,kind,text,status,created,epoch,turn_id,error) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,NULL,NULL) \
             ON CONFLICT(id) DO NOTHING",
            params![
                event.id.0,
                event.agent.0,
                event.kind,
                event.text,
                event.status,
                event.created,
                event.epoch
            ],
        )?;
        Ok(inserted != 0)
    }

    pub fn mark_event_enqueued(
        connection: &Connection,
        id: &str,
        monotonic_ns: u128,
    ) -> StoreResult<()> {
        let key = QueueEventId::from(id);
        let table = Table::<EventMetadataRecord>::new(connection);
        let mut record = table.get(&key)?.unwrap_or_default();
        let timing = match &mut record.timing {
            studio_runtime_schema::queue_command::Field::Value(timing) => timing,
            timing @ (studio_runtime_schema::queue_command::Field::Missing
            | studio_runtime_schema::queue_command::Field::Null) => {
                *timing = studio_runtime_schema::queue_command::Field::Value(Default::default());
                let studio_runtime_schema::queue_command::Field::Value(timing) = timing else {
                    unreachable!()
                };
                timing
            }
        };
        timing.insert("enqueuedAt".to_owned(), studio_json::json!(monotonic_ns));
        table.put_at(&key, &record)
    }

    /// Mark the agent views after a scheduler status write. The views are built at read
    /// time from the agent row; the mark makes readers build them again.
    pub fn set_agent_sync_status(
        transaction: &Transaction<'_>,
        agent_id: &str,
        _status: &str,
    ) -> StoreResult<()> {
        Self::mark_agent_fields(transaction, agent_id)
    }

    /// Mark the agent views after a scheduler lifecycle transition.
    pub fn set_agent_sync_lifecycle(
        transaction: &Transaction<'_>,
        agent_id: &str,
        _status: &str,
        _in_flight: bool,
        _turn_id: Option<&str>,
    ) -> StoreResult<()> {
        Self::mark_agent_fields(transaction, agent_id)
    }

    /// Mark the agent views after a write of runtime-owned agent fields. Under the
    /// default publication ([`crate::entity_publication`]) the chat also gets a full
    /// publication before the commit.
    pub fn set_agent_sync_values(
        transaction: &Transaction<'_>,
        agent_id: &str,
        _fields: &[(&str, Value)],
    ) -> StoreResult<()> {
        if crate::entity_publication::active() {
            crate::entity_publication::mark_stale("agent", agent_id);
        }
        Self::mark_agent_views(transaction, agent_id)
    }

    /// Mark the agent views after a title write.
    pub fn set_agent_sync_name(
        transaction: &Transaction<'_>,
        agent_id: &str,
        _name: &str,
    ) -> StoreResult<()> {
        Self::mark_agent_fields(transaction, agent_id)
    }

    /// A status, lifecycle or title write: the fields are the projection of this write,
    /// so the commit needs no full publication of the chat.
    fn mark_agent_fields(transaction: &Transaction<'_>, agent_id: &str) -> StoreResult<()> {
        Self::mark_agent_views(transaction, agent_id)?;
        crate::entity_publication::mark_published("agent", agent_id);
        Ok(())
    }

    fn mark_agent_views(transaction: &Transaction<'_>, agent_id: &str) -> StoreResult<()> {
        crate::durable_seq::mark(transaction, crate::durable_seq::AGENT_VIEW_MARK, agent_id)
    }
}

#[cfg(test)]
mod tests {
    use super::EventWriteRepository;
    use crate::StoreError;
    use rusqlite::Connection;
    use studio_json::json;

    #[test]
    fn agent_sync_writes_mark_the_agent_views_at_a_new_sequence() {
        let mut connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(
                "CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,PRIMARY KEY(collection,id));
                 CREATE TABLE sync_documents(seq INTEGER,scope TEXT,id TEXT,payload TEXT,deleted INTEGER);
                 CREATE TABLE durable_rows(tbl TEXT,id TEXT,seq INTEGER,deleted INTEGER,at INTEGER,PRIMARY KEY(tbl,id));
                 CREATE TABLE sync_generation(id INTEGER PRIMARY KEY,value INTEGER NOT NULL); INSERT INTO sync_generation VALUES(1,4);",
            )
            .expect("counter fixture creates");
        let mark = |connection: &Connection| {
            connection
                .query_row(
                    "SELECT seq FROM durable_rows WHERE tbl='agent_views' AND id='a'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("agent views mark exists")
        };
        type Write = dyn Fn(&rusqlite::Transaction<'_>) -> Result<(), StoreError>;
        let writes: [&Write; 4] = [
            &|tx| EventWriteRepository::set_agent_sync_status(tx, "a", "queued"),
            &|tx| {
                EventWriteRepository::set_agent_sync_lifecycle(
                    tx,
                    "a",
                    "running",
                    true,
                    Some("turn-1"),
                )
            },
            &|tx| {
                EventWriteRepository::set_agent_sync_values(tx, "a", &[("lastAnswer", json!(""))])
            },
            &|tx| EventWriteRepository::set_agent_sync_name(tx, "a", "New"),
        ];
        for (expected, write) in (5..).zip(writes) {
            let transaction = connection.transaction().expect("transaction begins");
            write(&transaction).expect("agent views mark");
            transaction.commit().expect("transaction commits");
            assert_eq!(mark(&connection), expected);
        }
    }

    #[test]
    fn enqueue_timing_is_written_when_metadata_table_exists() {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(
                "CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL);",
            )
            .expect("metadata table creates");
        EventWriteRepository::mark_event_enqueued(&connection, "e", 42).expect("timing writes");
        let record: String = connection
            .query_row(
                "SELECT record FROM runtime_event_meta WHERE id='e'",
                [],
                |row| row.get(0),
            )
            .expect("metadata exists");
        assert_eq!(
            studio_json::from_str::<studio_json::Value>(&record).expect("metadata parses")["timing"]
                ["enqueuedAt"],
            42
        );
    }

    #[test]
    fn enqueue_timing_preserves_legacy_metadata_and_unknown_values() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL);",
            )
            .unwrap();
        let original = studio_json::json!({"delivery":"after_turn","requestedDelivery":null,"timing":{"prior":18446744073709551615u64},"assets":[null,{"extra":true}],"future":{"a":1}});
        connection
            .execute(
                "INSERT INTO runtime_event_meta VALUES('e',?1)",
                [original.to_string()],
            )
            .unwrap();
        EventWriteRepository::mark_event_enqueued(&connection, "e", 42).unwrap();
        let raw: String = connection
            .query_row(
                "SELECT record FROM runtime_event_meta WHERE id='e'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut expected = original;
        expected["timing"]["enqueuedAt"] = studio_json::json!(42);
        assert_eq!(
            studio_json::from_str::<studio_json::Value>(&raw).unwrap(),
            expected
        );
    }

    #[test]
    fn enqueue_refuses_scalar_timing_without_panicking_or_mutating_metadata() {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection
            .execute_batch(
                "CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL);
                 INSERT INTO runtime_event_meta VALUES('e','{\"timing\":true}');",
            )
            .expect("corrupt metadata fixture inserts");
        assert!(matches!(
            EventWriteRepository::mark_event_enqueued(&connection, "e", 42),
            Err(StoreError::InvalidStoredObject { table: "runtime_event_meta", id }) if id == "e"
        ));
        let stored: String = connection
            .query_row(
                "SELECT record FROM runtime_event_meta WHERE id='e'",
                [],
                |row| row.get(0),
            )
            .expect("metadata remains available");
        assert_eq!(stored, r#"{"timing":true}"#);
    }
}
