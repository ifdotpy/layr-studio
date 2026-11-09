//! Persistence for provider notification dispatch.

use crate::CachedSql;
use crate::schema::SchemaRequirements;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use studio_json::Value;

use crate::repositories::transcript::{TranscriptItem, TranscriptRepository, TranscriptStoreError};
use crate::{StoreError, StoreResult};

const EVENT_META_TABLE: &[crate::schema::TableRequirement] = &[crate::schema::TableRequirement {
    name: "runtime_event_meta",
    columns: &["id", "record"],
}];

#[derive(Clone, Copy, Debug, Default)]
pub struct NotificationDispatchRepository;

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[
        crate::schema::TableRequirement {
            name: "runtime_agents",
            columns: &["id", "record"],
        },
        crate::schema::TableRequirement {
            name: "runtime_events",
            columns: &["id", "agent", "kind", "text", "created"],
        },
        crate::schema::TableRequirement {
            name: "runtime_items",
            columns: &["id", "agent", "record", "created"],
        },
    ],
    conditional_tables: &[
        crate::schema::ConditionalTableRequirement {
            table_exists: "runtime_event_meta",
            tables: EVENT_META_TABLE,
        },
        crate::schema::ConditionalTableRequirement {
            table_exists: "runtime_item_fulltext",
            tables: &[crate::schema::TableRequirement {
                name: "runtime_item_fulltext",
                columns: &["id", "body"],
            }],
        },
    ],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

impl NotificationDispatchRepository {
    /// The input event owns publication identity even after its replay body expires.
    pub fn submission_owner(
        tx: &Transaction<'_>,
        agent: &str,
        event: &str,
    ) -> StoreResult<Option<studio_json::OwnedRawJson>> {
        if Self::user_event(tx, agent, event)?.is_none() {
            return Err(invalid_record("runtime_events", event));
        }
        Ok(Self::read_event_metadata::<
            std::collections::BTreeMap<String, studio_json::OwnedRawJson>,
        >(tx, event)?
        .and_then(|mut metadata| metadata.remove("providerSubmission")))
    }

    pub fn put_submission_owner<T: Serialize>(
        tx: &Transaction<'_>,
        attempt: Option<&str>,
        events: &[&str],
        owner: &T,
    ) -> StoreResult<()> {
        let raw = studio_json::to_string(owner)?;
        for event in events {
            Self::event_metadata(tx, event)?;
            tx.execute_cached(
                "INSERT INTO runtime_event_meta(id,record) VALUES(?1,json_object('providerSubmission',json(?2))) ON CONFLICT(id) DO UPDATE SET record=json_set(record,'$.providerSubmission',json(?2))",
                params![event, raw],
            )?;
        }
        if let Some(attempt) = attempt {
            let changed = tx.execute_cached(
                "UPDATE runtime_execution_attempts SET record=json_set(record,'$.providerSubmission',json(?2)) WHERE id=?1",
                params![attempt, raw],
            )?;
            if changed != 1 {
                return Err(invalid_record("runtime_execution_attempts", attempt));
            }
        }
        Ok(())
    }

    pub fn agent_record(
        transaction: &Transaction<'_>,
        agent: &str,
    ) -> StoreResult<Option<crate::AgentRecord>> {
        TranscriptRepository::agent(transaction, agent)
            .map_err(|error| map_transcript_error(error, "runtime_agents", agent))
    }

    pub fn related_record(
        transaction: &Transaction<'_>,
        table: &'static str,
        id: &str,
    ) -> StoreResult<Option<Value>> {
        if !crate::schema::table_exists(transaction, table)? {
            return Ok(None);
        }
        let raw: Option<String> = transaction
            .query_row(
                &format!("SELECT record FROM {table} WHERE id=?1"),
                [id],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|raw| {
            let value: Value =
                studio_json::from_str(&raw).map_err(|_| invalid_record(table, id))?;
            if !value.is_object() || value.get("id").and_then(Value::as_str) != Some(id) {
                return Err(invalid_record(table, id));
            }
            Ok(value)
        })
        .transpose()
    }

    pub fn put_agent_record(
        transaction: &Transaction<'_>,
        agent: &str,
        record: &crate::AgentRecord,
    ) -> StoreResult<()> {
        if record.id.0 != agent
            || !crate::Table::<crate::AgentRecord>::new(transaction).update(record)?
        {
            return Err(invalid_record("runtime_agents", agent));
        }
        Ok(())
    }

    pub fn transcript_item(
        transaction: &Transaction<'_>,
        agent: &str,
        id: &str,
    ) -> StoreResult<Option<(Value, f64)>> {
        Self::read_item(transaction, agent, id, false)
    }

    /// Read a native notice or tool item through the same checked storage boundary.
    pub fn notification_item(
        transaction: &Transaction<'_>,
        agent: &str,
        id: &str,
    ) -> StoreResult<Option<(Value, f64)>> {
        Self::read_item(transaction, agent, id, true)
    }

    fn read_item(
        transaction: &Transaction<'_>,
        agent: &str,
        id: &str,
        include_notices: bool,
    ) -> StoreResult<Option<(Value, f64)>> {
        let item = TranscriptRepository::item(transaction, agent, id)
            .map_err(|error| map_transcript_error(error, "runtime_items", id))?;
        item.map(|item| {
            validate_item(&item, agent, include_notices)?;
            let created = transaction
                .query_row_cached(
                    "SELECT created FROM runtime_items WHERE id=?1 AND agent=?2",
                    params![id, agent],
                    |row| row.get::<_, f64>(0),
                )
                .map_err(|_| invalid_record("runtime_items", id))?;
            if !created.is_finite() || created < 0.0 {
                return Err(invalid_record("runtime_items", id));
            }
            Ok((item.record.into_value(), created))
        })
        .transpose()
    }

    pub fn full_text(
        transaction: &Transaction<'_>,
        id: &str,
        record: &Value,
    ) -> StoreResult<Option<String>> {
        let _: String = transaction.query_row_cached(
            "SELECT agent FROM runtime_items WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        TranscriptRepository::full_text_raw(transaction, id, record)
            .map_err(|error| map_transcript_error(error, "runtime_items", id))
    }

    pub fn put_transcript_item(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        index_search: bool,
    ) -> StoreResult<()> {
        TranscriptRepository::put_item(transaction, id, agent, record, created, text, index_search)
            .map_err(|error| map_transcript_error(error, "runtime_items", id))
    }

    /// Native notifications share transcript persistence and search projection rules.
    pub fn put_notification_item(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        index_search: bool,
    ) -> StoreResult<()> {
        TranscriptRepository::put_notification_item(
            transaction,
            id,
            agent,
            record,
            created,
            text,
            index_search,
        )
        .map_err(|error| map_transcript_error(error, "runtime_items", id))
    }

    pub fn link_transcript_item_to_event(
        transaction: &Transaction<'_>,
        agent: &str,
        event_id: &str,
        item_id: &str,
    ) -> StoreResult<()> {
        let event_exists: bool = transaction.query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM runtime_events WHERE id=?1 AND agent=?2)",
            params![event_id, agent],
            |row| row.get(0),
        )?;
        if !event_exists {
            return Err(invalid_record("runtime_events", event_id));
        }
        let meta: Option<String> = transaction
            .query_row_cached(
                "SELECT record FROM runtime_event_meta WHERE id=?1",
                [event_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(meta) = meta {
            studio_json::from_str::<
                studio_runtime_schema::json_object::JsonObject<serde::de::IgnoredAny>,
            >(&meta)
            .map_err(|_| invalid_record("runtime_event_meta", event_id))?;
            transaction.execute_cached(
                "UPDATE runtime_event_meta SET record=json_set(record,'$.transcriptItemId',?1,'$.modelEventProjection',1) WHERE id=?2",
                params![item_id, event_id],
            )?;
        } else {
            let record = studio_json::json!({"transcriptItemId":item_id,"modelEventProjection":1});
            transaction.execute_cached(
                "INSERT INTO runtime_event_meta(id,record) VALUES(?1,?2)",
                params![event_id, record.to_string()],
            )?;
        }
        Ok(())
    }

    // rusqlite errors are non-exhaustive; preserve every unrelated database failure.
    #[allow(clippy::wildcard_enum_match_arm)]
    pub fn user_event(
        transaction: &Transaction<'_>,
        agent: &str,
        event_id: &str,
    ) -> StoreResult<Option<(String, String, f64)>> {
        let event = transaction
            .query_row_cached(
                "SELECT kind,text,created FROM runtime_events WHERE id=?1 AND agent=?2",
                params![event_id, agent],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| match error {
                rusqlite::Error::InvalidColumnType(..)
                | rusqlite::Error::FromSqlConversionFailure(..) => {
                    invalid_record("runtime_events", event_id)
                }
                error => StoreError::from(error),
            })?;
        if event
            .as_ref()
            .is_some_and(|(_, _, created)| !created.is_finite() || *created < 0.0)
        {
            return Err(invalid_record("runtime_events", event_id));
        }
        Ok(event)
    }

    pub fn event_metadata(
        transaction: &Transaction<'_>,
        event_id: &str,
    ) -> StoreResult<Option<Value>> {
        Self::read_event_metadata(transaction, event_id)
    }

    fn read_event_metadata<T: serde::de::DeserializeOwned>(
        transaction: &Transaction<'_>,
        event_id: &str,
    ) -> StoreResult<Option<T>> {
        if !crate::schema::table_exists(transaction, "runtime_event_meta")? {
            return Err(invalid_record("runtime_event_meta", event_id));
        }
        let raw: Option<String> = transaction
            .query_row_cached(
                "SELECT record FROM runtime_event_meta WHERE id=?1",
                [event_id],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|raw| {
            studio_json::from_str::<studio_runtime_schema::json_object::JsonObject<T>>(&raw)
                .map(|value| value.0)
                .map_err(|_| invalid_record("runtime_event_meta", event_id))
        })
        .transpose()
    }
}

fn validate_item(item: &TranscriptItem, agent: &str, include_notices: bool) -> StoreResult<()> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_schema::transcript_record::TranscriptRole;
    let record = &item.record;
    let role = record.role();
    let valid_inputs = match record.inputs() {
        Field::Missing => true,
        Field::Null => false,
        Field::Value(inputs) => inputs.iter().all(|input| {
            input.id().is_some()
                && input.at().is_some_and(f64::is_finite)
                && input.kind().is_some()
                && input.text().is_some()
                && input.truncated().is_some()
                && matches!(input.assets(), Field::Value(_))
        }),
    };
    if matches!(record.truncated_field(), Field::Null)
        || matches!(record.streaming(), Field::Null)
        || record.id().as_ref() != item.id
        || !(matches!(role, TranscriptRole::Assistant | TranscriptRole::User)
            || include_notices && matches!(role, TranscriptRole::System | TranscriptRole::Output))
        || matches!(
            record.text(),
            studio_runtime_schema::transcript_record::TranscriptText::InFulltextNull
        )
        || matches!(record.assets(), Field::Null)
        || !valid_inputs
    {
        return Err(invalid_record("runtime_items", &item.id));
    }
    let expected_prefix = format!("{agent}:");
    if !item.id.starts_with(&expected_prefix) {
        return Err(invalid_record("runtime_items", &item.id));
    }
    Ok(())
}

fn map_transcript_error(error: TranscriptStoreError, table: &'static str, id: &str) -> StoreError {
    match error {
        TranscriptStoreError::Store(error) => error,
        TranscriptStoreError::Sqlite(error) => StoreError::from(error),
        TranscriptStoreError::InvalidStoredObject { .. } => invalid_record(table, id),
    }
}

fn invalid_record(table: &str, id: &str) -> StoreError {
    StoreError::InvalidSyncObject {
        scope: table.into(),
        id: id.into(),
    }
}

inventory::submit! { crate::repositories::SchemaRegistration { repository: "notification_dispatch", requirements: &REQUIREMENTS } }

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_invalid<T: std::fmt::Debug>(result: StoreResult<T>, table: &str, id: &str) {
        assert!(
            matches!(result, Err(StoreError::InvalidSyncObject { scope, id: actual }) if scope == table && actual == id)
        );
    }

    #[test]
    fn transcript_metadata_reads_preserve_absence_errors_and_raw_owner() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_events(id TEXT,agent TEXT,kind TEXT,text TEXT,created REAL); INSERT INTO runtime_events VALUES('event','agent','user','input',1)").unwrap();
        let tx = db.transaction().unwrap();
        assert_invalid(
            NotificationDispatchRepository::event_metadata(&tx, "event"),
            "runtime_event_meta",
            "event",
        );
        assert_invalid(
            NotificationDispatchRepository::submission_owner(&tx, "agent", "event"),
            "runtime_event_meta",
            "event",
        );
        tx.execute_batch(
            "CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL)",
        )
        .unwrap();
        assert!(
            NotificationDispatchRepository::event_metadata(&tx, "event")
                .unwrap()
                .is_none()
        );
        assert!(
            NotificationDispatchRepository::submission_owner(&tx, "agent", "event")
                .unwrap()
                .is_none()
        );
        for raw in ["[1]", "null", "{broken"] {
            tx.execute(
                "INSERT OR REPLACE INTO runtime_event_meta VALUES('event',?1)",
                [raw],
            )
            .unwrap();
            assert_invalid(
                NotificationDispatchRepository::event_metadata(&tx, "event"),
                "runtime_event_meta",
                "event",
            );
            assert_invalid(
                NotificationDispatchRepository::submission_owner(&tx, "agent", "event"),
                "runtime_event_meta",
                "event",
            );
        }
        let owner = r#"{ "opaque":9007199254740993, "duplicate":1,"duplicate":2 }"#;
        tx.execute(
            "UPDATE runtime_event_meta SET record=?1",
            [format!(r#"{{"providerSubmission":{owner},"future":true}}"#)],
        )
        .unwrap();
        assert_eq!(
            NotificationDispatchRepository::submission_owner(&tx, "agent", "event")
                .unwrap()
                .unwrap()
                .as_str(),
            owner
        );
        assert_invalid(
            NotificationDispatchRepository::submission_owner(&tx, "other", "event"),
            "runtime_events",
            "event",
        );
        tx.execute("UPDATE runtime_event_meta SET record='{}'", [])
            .unwrap();
        assert!(
            NotificationDispatchRepository::submission_owner(&tx, "agent", "event")
                .unwrap()
                .is_none()
        );
        tx.execute(
            r#"UPDATE runtime_event_meta SET record='{"providerSubmission":null}'"#,
            [],
        )
        .unwrap();
        assert_eq!(
            NotificationDispatchRepository::submission_owner(&tx, "agent", "event")
                .unwrap()
                .unwrap()
                .as_str(),
            "null"
        );
    }

    #[test]
    fn transcript_notification_reads_keep_physical_owner_and_notice_eligibility() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_items(id TEXT PRIMARY KEY,agent TEXT,record TEXT,created REAL)",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        for role in ["assistant", "user", "system", "output"] {
            let raw = format!(
                r#"{{"id":"agent:x","role":"{role}","text":"notice","afterRestore":"restored"}}"#
            );
            tx.execute(
                "INSERT OR REPLACE INTO runtime_items VALUES('agent:x','agent',?1,7.5)",
                [&raw],
            )
            .unwrap();
            assert!(
                NotificationDispatchRepository::notification_item(&tx, "other", "agent:x")
                    .unwrap()
                    .is_none()
            );
            let (_, created) =
                NotificationDispatchRepository::notification_item(&tx, "agent", "agent:x")
                    .unwrap()
                    .unwrap();
            assert_eq!(created, 7.5);
            let ordinary = NotificationDispatchRepository::transcript_item(&tx, "agent", "agent:x");
            if matches!(role, "system" | "output") {
                assert_invalid(ordinary, "runtime_items", "agent:x");
            } else {
                assert!(ordinary.unwrap().is_some());
            }
            assert!(
                TranscriptRepository::items_for_agent(&tx, "agent")
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                TranscriptRepository::items_for_agent_including_restored(&tx, "agent")
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                TranscriptRepository::item_any_agent(&tx, "agent:x")
                    .unwrap()
                    .unwrap()
                    .agent,
                "agent"
            );
        }
        assert!(
            TranscriptRepository::item_any_agent(&tx, "missing")
                .unwrap()
                .is_none()
        );
        tx.execute("UPDATE runtime_items SET record='{broken'", [])
            .unwrap();
        assert_invalid(
            NotificationDispatchRepository::notification_item(&tx, "agent", "agent:x"),
            "runtime_items",
            "agent:x",
        );
    }
}
