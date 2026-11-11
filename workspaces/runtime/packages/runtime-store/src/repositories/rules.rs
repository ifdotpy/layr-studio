//! Read persisted rule and owner records for the rules list.

use crate::agent_tables::AgentsStorageRow;
use rusqlite::{Connection, Transaction};

use crate::StoreResult;
use crate::json_record_codec::RawJsonRecord;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use crate::planning_work::RuleRow;
use crate::records::{AgentId, AgentRecord, RuleOwnerRecord, RuleRecord, Table};
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
    /// Owner fields that can invalidate a sampled rule decision.
    pub const OWNER_FIELDS: &[&str] = &[
        "epoch",
        "status",
        "archived",
        "accountKey",
        "threadId",
        "autoWake",
        "deletedAt",
        "restartRecovery",
        "disconnectRecovery",
        "nativeFailureHold",
        "approvalPolicy",
        "sandbox",
        "profile",
        "role",
    ];

    /// Decode only decision fields and preserve source duplicate-field errors.
    pub fn owner(connection: &Connection, id: &str) -> StoreResult<Option<RuleOwnerRecord>> {
        let raw = studio_db_api::db_query!(
            canvas_read(connection),
            AgentsStorageRow,
            scalar_raw(AgentsStorageRow::RECORD),
            filter(AgentsStorageRow::ID.eq_ref(id))
        )?;
        raw.map(|raw| {
            let owner: RuleOwnerRecord = crate::records::decode_agent_projection(id, &raw)?;
            if owner.id().as_ref() != id {
                return Err(crate::records::invalid_agent_field(
                    id,
                    "/id",
                    "row_id_mismatch",
                    None,
                ));
            }
            Ok(owner)
        })
        .transpose()
    }

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
    fn rule_owner_projection_matches_the_decision_fields_of_the_full_record() {
        let db = crate::fresh_state::test_canvas();
        for fields in [
            studio_json::json!({}),
            studio_json::json!({"deletedAt":0,"archived":true,"autoWake":false}),
            studio_json::json!({"deletedAt":1.5,"archived":false,"autoWake":true}),
            studio_json::json!({"threadId":null,"deletedAt":null,"sandbox":null,"restartRecovery":null}),
            studio_json::json!({"restartRecovery":{"epoch":1,"stage":"pending","autoWake":true,"threadId":null}}),
            studio_json::json!({"threadId":"thread","archived":null,"nativeFailureHold":true,
                "approvalPolicy":"never","sandbox":{"type":"dangerFullAccess"},
                "profile":{"id":"profile"},"role":"implementer",
                "disconnectRecovery":{"epoch":1,"stage":"pending","autoWake":true,"accountKey":"default","threadId":"thread"}}),
        ] {
            let mut raw = studio_json::json!({"id":"owner","rootId":"owner","epoch":1,"status":"idle","compactions":0,"accountKey":"default","provider":"codex","autoWake":true,"unrelatedHistory":{"text":"large transcript"}});
            raw.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            assert!(
                crate::AgentRecord::from_value(raw.clone()).is_err(),
                "unknown root keys are refused"
            );
            let retained = raw
                .as_object_mut()
                .unwrap()
                .remove("unrelatedHistory")
                .unwrap();
            raw["error"] = retained;
            let agent = crate::AgentRecord::from_value(raw).unwrap();
            crate::AgentRepository::put(&db, &agent).unwrap();
            let full = RulesRepository::agent(&db, "owner")
                .unwrap()
                .unwrap()
                .into_value();
            let scope = crate::agent_volatile::publication_work::scope();
            let owner = RulesRepository::owner(&db, "owner")
                .unwrap()
                .unwrap()
                .value();
            assert_eq!(
                crate::agent_volatile::publication_work::take_full_decodes(),
                0
            );
            drop(scope);
            assert!(owner.get("error").is_none());
            for field in RulesRepository::OWNER_FIELDS {
                assert_eq!(owner.get(field), full.get(field), "{field}");
            }
        }
        assert!(RulesRepository::owner(&db, "missing").unwrap().is_none());
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
