//! Read persisted rule and owner records for the rules list.

use rusqlite::{Connection, Transaction};

use crate::StoreResult;
use crate::json_record_codec::RawJsonRecord;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use crate::planning_work::RuleRow;
use crate::records::{AgentId, AgentRecord, RuleRecord, Table};
use crate::schema::{SchemaRequirements, TableRequirement};

const RULE_TABLE: TableRequirement = TableRequirement {
    name: "runtime_rules",
    columns: &["id", "record"],
};

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[
        TableRequirement {
            name: "runtime_agents",
            columns: &["id", "record"],
        },
        TableRequirement {
            name: "runtime_monitors",
            columns: &["id", "record"],
        },
    ],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[RULE_TABLE],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Copy, Debug, Default)]
pub struct RulesRepository;

impl RulesRepository {
    pub fn list(connection: &Connection) -> StoreResult<(Vec<RuleRecord>, Vec<AgentRecord>)> {
        Ok((
            // The existing physical rowid order has no db_query ordering clause.
            Table::<RuleRecord>::new(connection).list()?,
            Table::<AgentRecord>::new(connection).list()?,
        ))
    }

    pub fn get(connection: &Connection, id: &str) -> StoreResult<Option<RuleRecord>> {
        Ok(canvas_read(connection)
            .get::<RuleRow>(&(id.to_owned(),))?
            .map(|row| row.record.record().clone()))
    }

    pub fn agent(connection: &Connection, id: &str) -> StoreResult<Option<AgentRecord>> {
        Table::<AgentRecord>::new(connection).get(&AgentId::from(id))
    }

    /// Store a rule. Its client view is built at read time (`entity_scope`).
    pub fn put(transaction: &Transaction<'_>, record: &RuleRecord) -> StoreResult<()> {
        canvas_tx(transaction).upsert(&RuleRow {
            id: record.id().clone(),
            record: RawJsonRecord::from_record(transaction, record)?,
        })?;
        Ok(())
    }

    pub fn delete(transaction: &Transaction<'_>, id: &str) -> StoreResult<()> {
        canvas_tx(transaction).delete::<RuleRow>(&(id.to_owned(),))?;
        Ok(())
    }

    pub fn active_check_ids(connection: &Connection, rule_id: &str) -> StoreResult<Vec<String>> {
        let mut statement = connection.prepare(
            "SELECT id FROM runtime_monitors WHERE \
             CASE WHEN json_valid(record) THEN \
             json_extract(record,'$.ruleId')=?1 AND \
             json_extract(record,'$.status') IN ('running','starting','approval') \
             ELSE 0 END ORDER BY rowid",
        )?;
        statement
            .query_map([rule_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::{REQUIREMENTS, RulesRepository};
    use rusqlite::Connection;

    #[test]
    fn rule_repository_reads_only_declared_tables_in_python_row_order() {
        let db = Connection::open_in_memory().expect("in-memory database opens");
        db.execute_batch(
            &crate::agent_columns::fixture_sql("CREATE TABLE runtime_rules(id TEXT PRIMARY KEY,record TEXT NOT NULL);
             CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
             INSERT INTO runtime_rules VALUES('r2','{\"id\":\"r2\",\"agent\":\"a1\",\"epoch\":0,\"name\":\"Rule\",\"kind\":\"interval\",\"status\":\"active\",\"intervalSeconds\":60,\"nextAt\":1,\"created\":1,\"command\":\"\",\"text\":\"\",\"checks\":0,\"wakes\":0,\"inFlight\":false,\"fileGeneration\":0,\"stallTimeoutSeconds\":1800,\"stallWakeGeneration\":-1,\"livenessCommand\":\"\",\"event\":\"\"}');
             INSERT INTO runtime_rules VALUES('r1','{\"id\":\"r1\",\"agent\":\"a1\",\"epoch\":0,\"name\":\"Rule\",\"kind\":\"interval\",\"status\":\"active\",\"intervalSeconds\":60,\"nextAt\":1,\"created\":1,\"command\":\"\",\"text\":\"\",\"checks\":0,\"wakes\":0,\"inFlight\":false,\"fileGeneration\":0,\"stallTimeoutSeconds\":1800,\"stallWakeGeneration\":-1,\"livenessCommand\":\"\",\"event\":\"\"}');
             INSERT INTO runtime_agents(id,record) VALUES('a1','{\"id\":\"a1\",\"provider\":\"codex\",\"accountKey\":\"default\",\"rootId\":\"a1\",\"epoch\":0,\"status\":\"idle\",\"compactions\":0}');"),
        )
        .expect("declared tables create");
        db.execute_batch(crate::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");

        let (rules, agents) = RulesRepository::list(&db).expect("records read");
        assert_eq!(
            rules
                .iter()
                .map(|item| item.id().as_str())
                .collect::<Vec<_>>(),
            ["r2", "r1"]
        );
        assert_eq!(agents[0].id().0, "a1");
        assert!(
            REQUIREMENTS
                .bootstrap_tables
                .iter()
                .any(|table| table.name == "runtime_rules")
        );
        assert!(
            REQUIREMENTS
                .tables
                .iter()
                .any(|table| table.name == "runtime_agents")
        );
    }

    #[test]
    fn active_check_lookup_is_filtered_and_ignores_unrelated_invalid_rows() {
        let db = Connection::open_in_memory().expect("in-memory database opens");
        db.execute_batch(
            "CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY,record TEXT NOT NULL, stream_pending INTEGER NOT NULL DEFAULT 0, monitor_status TEXT, monitor_error_candidate INTEGER NOT NULL DEFAULT 1, monitor_created REAL NOT NULL DEFAULT 0);
             INSERT INTO runtime_monitors(id,record) VALUES
                 ('m1','{\"id\":\"m1\",\"agent\":\"a1\",\"ruleId\":\"r1\",\"status\":\"running\",\"created\":1}'),
                 ('unrelated','{\"id\":\"unrelated\",\"status\":false}');",
        )
        .expect("monitor table creates");
        assert_eq!(
            RulesRepository::active_check_ids(&db, "r1").expect("active check is found"),
            ["m1"]
        );
        assert!(
            RulesRepository::active_check_ids(&db, "r2")
                .expect("other rule has no check")
                .is_empty()
        );
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "rules",
        requirements: &REQUIREMENTS,
    }
}
