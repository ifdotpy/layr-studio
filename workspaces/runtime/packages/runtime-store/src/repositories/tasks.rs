use rusqlite::Connection;

use crate::StoreResult;
use crate::json_record_codec::RawJsonRecord;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use crate::planning_work::TaskRow;
use crate::records::{Table, TaskId, TaskRecord, TaskRepository};
use crate::schema::{SchemaRequirements, TableRequirement};

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[TableRequirement {
        name: "runtime_tasks",
        columns: &["id", "record"],
    }],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

impl TaskRepository {
    /// Create the task indexes Rust readers rely on. The newest-tasks-per-agent
    /// order serves storage retention and agent task history; Python startup
    /// creates the same index, so existing state directories keep theirs.
    pub(crate) fn prepare_indexes(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
        for definition in Self::index_definitions() {
            transaction.execute_batch(&definition.create_sql)?;
        }
        Ok(())
    }

    /// Canonical DDL for the task repository's owned indexes.
    #[doc(hidden)]
    pub fn index_definitions() -> Vec<crate::OwnedRuntimeIndexDefinition> {
        use studio_db_api::Table as _;
        TaskRow::INDEXES
            .iter()
            .filter(|index| index.name != "runtime_task_agent_updated_id")
            .map(|index| crate::OwnedRuntimeIndexDefinition {
                name: index.name,
                table: TaskRow::NAME,
                create_sql: index
                    .sql
                    .replacen("CREATE INDEX ", "CREATE INDEX IF NOT EXISTS ", 1),
            })
            .collect()
    }

    /// Schema-checked running tasks for one agent, in stable row order.
    pub fn running_for_agent(connection: &Connection, agent: &str) -> StoreResult<Vec<TaskRecord>> {
        let mut statement = connection.prepare_cached(
            // JSON expression selectors retain the existing SQL semantics.
            "SELECT id,record FROM runtime_tasks WHERE json_extract(record,'$.agent')=?1 \
             AND json_extract(record,'$.status')='running' ORDER BY id",
        )?;
        statement
            .query_map([agent], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (id, raw) = row?;
                Table::<TaskRecord>::new(connection).decode_stored(&id, &raw)
            })
            .collect()
    }

    pub fn get(connection: &Connection, id: &TaskId) -> StoreResult<Option<TaskRecord>> {
        Ok(canvas_read(connection)
            .get::<TaskRow>(&(id.as_ref().to_owned(),))?
            .map(|row| row.record.record().clone()))
    }

    pub fn put(connection: &rusqlite::Transaction<'_>, record: &TaskRecord) -> StoreResult<()> {
        canvas_tx(connection).upsert(&TaskRow {
            id: record.id().as_ref().to_owned(),
            record: RawJsonRecord::from_record(connection, record)?,
        })?;
        Ok(())
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "tasks",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod typed_tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn task_rows_preserve_presence_extensions_and_check_identity() {
        let mut db = Connection::open_in_memory().unwrap();
        let db = db.transaction().unwrap();
        db.execute_batch("CREATE TABLE runtime_tasks(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
            .unwrap();
        for index in TaskRepository::index_definitions() {
            db.execute_batch(&index.create_sql).unwrap();
        }
        let value = json!({"id":"a","agent":"agent","created":1.0,"status":"running","kind":"command","itemId":"item","turnId":"turn","type":"commandExecution","name":"commandExecution"});
        let record: TaskRecord = studio_json::from_str(&value.to_string()).unwrap();
        TaskRepository::put(&db, &record).unwrap();
        assert_eq!(
            crate::WorkspaceViewsRepository::task(&db, "a").unwrap(),
            Some(value.clone())
        );
        let page =
            crate::WorkspaceViewsRepository::task_page(&db, &["agent".into()], None, None, 10)
                .unwrap();
        assert_eq!(
            page.into_iter().next().unwrap().decode(&db).unwrap(),
            record
        );
        assert_eq!(
            crate::projection_value(
                &TaskRepository::get(&db, &TaskId::from("a"))
                    .unwrap()
                    .unwrap()
            ),
            value
        );
        db.execute(
            "UPDATE runtime_tasks SET record=?1",
            [r#"{"id":"b","status":"running"}"#],
        )
        .unwrap();
        assert!(TaskRepository::get(&db, &TaskId::from("a")).is_err());
        db.execute(
            "UPDATE runtime_tasks SET record=?1",
            [r#"{"id":"a","created":-1}"#],
        )
        .unwrap();
        assert!(TaskRepository::get(&db, &TaskId::from("a")).is_err());
    }
}
