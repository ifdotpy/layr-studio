use crate::CachedSql;
use crate::agent_tables::AgentsStorageRow;
use crate::peer_update_settings::canvas_read;
use rusqlite::Connection;
use studio_runtime_schema::persistence::HoldStatus;

use crate::StoreResult;
use crate::records::{AgentId, AgentRecord, AgentRepository, Table};
use crate::schema::{SchemaRequirements, TableRequirement};

const TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: "runtime_agents",
        columns: &["id", "record"],
    },
    crate::clock_schema::AGENT_GENERATION_TABLE_REQUIREMENT,
    crate::clock_schema::SCHEDULER_STATE_TABLE_REQUIREMENT,
    crate::clock_schema::SCHEDULER_CHANGES_TABLE_REQUIREMENT,
];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: TABLES,
    conditional_tables: &[],
    triggers: crate::clock_schema::AGENT_CLOCK_TRIGGERS,
    conditional_triggers: crate::clock_schema::AGENT_TRANSCRIPT_TRIGGER_REQUIREMENTS,
    connection_functions: &[],
    bootstrap_tables: &[
        crate::agent_volatile::TABLE_REQUIREMENT,
        crate::domain::TABLE_REQUIREMENTS[0],
        crate::domain::TABLE_REQUIREMENTS[1],
        crate::domain::TABLE_REQUIREMENTS[2],
        TableRequirement {
            name: "runtime_agents",
            columns: &["hold_status", "hold_incident"],
        },
    ],
    bootstrap_triggers: &crate::agent_volatile::TRIGGER_REQUIREMENTS,
    bootstrap_connection_functions: &[],
};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentDeletion {
    id: AgentId,
    deleted_at: Option<f64>,
}

impl AgentRepository {
    /// Seed the singleton rows the change-journal triggers update when a
    /// schema-only state lacks them; without them no change is journaled.
    pub(crate) fn prepare_change_journal(
        transaction: &rusqlite::Transaction<'_>,
    ) -> StoreResult<()> {
        crate::clock_schema::prepare_agent_change_journal(transaction)
    }

    pub fn get_raw(connection: &Connection, id: &AgentId) -> StoreResult<Option<String>> {
        Ok(Table::<AgentRecord>::new(connection)
            .get_with_raw(id)?
            .map(|(_, raw)| raw))
    }

    pub fn get(connection: &Connection, id: &AgentId) -> StoreResult<Option<AgentRecord>> {
        Table::<AgentRecord>::new(connection).get(id)
    }

    pub fn exists(connection: &Connection, id: &str) -> StoreResult<bool> {
        Ok(studio_db_api::db_query!(
            canvas_read(connection),
            AgentsStorageRow,
            exists(),
            filter(AgentsStorageRow::ID.eq_ref(id))
        )?)
    }

    /// Missing, live (including a zero deletion time), or deleted, by primary key.
    pub fn deletion_status(connection: &Connection, id: &str) -> StoreResult<Option<bool>> {
        studio_db_api::db_query!(
            canvas_read(connection),
            AgentsStorageRow,
            scalar_raw(AgentsStorageRow::RECORD),
            filter(AgentsStorageRow::ID.eq_ref(id))
        )?
        .map(|row| {
            let agent: AgentDeletion = crate::records::decode_agent_projection(id, &row)?;
            if agent.id.0 != id {
                return Err(crate::records::invalid_agent_field(
                    id,
                    "/id",
                    "row_id_mismatch",
                    None,
                ));
            }
            Ok(agent.deleted_at.is_some_and(|at| at != 0.0))
        })
        .transpose()
    }

    pub fn put(connection: &Connection, record: &AgentRecord) -> StoreResult<()> {
        Table::<AgentRecord>::new(connection).put(record)?;
        Self::hold(connection, &record.id.0, HoldStatus::Clear, None)?;
        Ok(())
    }

    /// Quarantine is independent of record decoding, including malformed JSON.
    pub fn hold(
        connection: &Connection,
        id: &str,
        status: HoldStatus,
        incident: Option<&str>,
    ) -> StoreResult<()> {
        connection.execute_cached(
            "UPDATE runtime_agents SET hold_status=?2,hold_incident=?3 WHERE id=?1 AND (hold_status IS NOT ?2 OR hold_incident IS NOT ?3)",
            rusqlite::params![id, status, incident],
        )?;
        Ok(())
    }

    pub fn event_fired(connection: &Connection, root: &str, event: &str) -> StoreResult<bool> {
        let agent = Self::get(connection, &AgentId::from(root))?
            .ok_or(crate::StoreError::RecordContract("missing event root"))?;
        Ok(agent.fired_events.contains(event))
    }

    /// The root owns every fired name for its lifetime; first insertion wins.
    pub fn fire_event(
        tx: &rusqlite::Transaction<'_>,
        root: &str,
        event: &str,
    ) -> StoreResult<bool> {
        let mut agent = Self::get(tx, &AgentId::from(root))?
            .ok_or(crate::StoreError::RecordContract("missing event root"))?;
        if !agent.fired_events.insert(event.to_owned()) {
            return Ok(false);
        }
        Self::put(tx, &agent)?;
        Ok(true)
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "agents",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod invalid_agent_repair_tests {
    use super::*;
    use studio_json::json;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
        cardinality = none, result = (), tables(agents = AgentsStorageRow),
        columns(id = AgentsStorageRow::ID, record = AgentsStorageRow::RECORD),
        sql = "UPDATE {agents} SET {record}=json_set({record},'$.deletedAt',json({id})) WHERE {id} IN ('true','false')")]
    struct BooleanDeletion;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
        cardinality = none, result = (), tables(agents = AgentsStorageRow),
        columns(id = AgentsStorageRow::ID, record = AgentsStorageRow::RECORD),
        sql = "UPDATE {agents} SET {record}=json_set({record},'$.deletedAt',NULL,'$.epoch',json({id})) WHERE {id} IN ('true','false')")]
    struct BooleanEpoch;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
        cardinality = none, result = (), tables(agents = AgentsStorageRow),
        columns(record = AgentsStorageRow::RECORD),
        sql = "UPDATE {agents} SET {record}=substr({record},1,length({record})-1)||',\"deletedAt\":0,\"deletedAt\":true'||char(125)")]
    struct DuplicateDeletion;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
        cardinality = none, result = (), tables(agents = AgentsStorageRow),
        columns(record = AgentsStorageRow::RECORD),
        sql = "UPDATE {agents} SET {record}=substr({record},1,length({record})-1)||',\"epoch\":false'||char(125)")]
    struct DuplicateEpoch;

    #[derive(studio_db_api::DbQuery)]
    #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
        cardinality = none, result = (), tables(agents = AgentsStorageRow),
        columns(id = AgentsStorageRow::ID, record = AgentsStorageRow::RECORD),
        sql = "UPDATE {agents} SET {record}=json_set({record},'$.epoch',CASE {id} WHEN 'negative' THEN -1 ELSE 9007199254740992 END)")]
    struct InvalidEpochRange;

    #[test]
    fn agent_narrow_reads_preserve_duplicate_keys_and_epoch_bounds() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let seed = |id: &str| {
            let agent = AgentRecord::from_value(json!({"id":id,"rootId":id,
                "provider":"codex","accountKey":"default","epoch":1,
                "status":"idle","compactions":0,"autoWake":true}))
            .unwrap();
            AgentRepository::put(&tx, &agent).unwrap();
        };
        seed("owner");
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&DuplicateDeletion::QUERY, &DuplicateDeletion)
            .unwrap();
        assert!(AgentRepository::get(&tx, &"owner".into()).is_err());
        assert!(AgentRepository::deletion_status(&tx, "owner").is_err());
        assert!(crate::RulesRepository::owner(&tx, "owner").is_err());
        seed("owner");
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&DuplicateEpoch::QUERY, &DuplicateEpoch)
            .unwrap();
        assert!(AgentRepository::get(&tx, &"owner".into()).is_err());
        assert!(crate::RulesRepository::owner(&tx, "owner").is_err());
        seed("negative");
        seed("unsafe");
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&InvalidEpochRange::QUERY, &InvalidEpochRange)
            .unwrap();
        for id in ["negative", "unsafe"] {
            assert!(AgentRepository::get(&tx, &id.into()).is_err());
            assert!(crate::RulesRepository::owner(&tx, id).is_err());
        }
    }

    #[test]
    fn agent_narrow_reads_reject_sql_boolean_numbers() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        for id in ["true", "false"] {
            let agent = AgentRecord::from_value(json!({"id":id,"rootId":id,
                "provider":"codex","accountKey":"default","epoch":1,
                "status":"idle","compactions":0,"autoWake":true}))
            .unwrap();
            AgentRepository::put(&tx, &agent).unwrap();
        }
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&BooleanDeletion::QUERY, &BooleanDeletion)
            .unwrap();
        for id in ["true", "false"] {
            assert!(AgentRepository::get(&tx, &id.into()).is_err());
            assert!(
                AgentRepository::deletion_status(&tx, id).is_err(),
                "deletedAt={id}"
            );
            assert!(
                crate::RulesRepository::owner(&tx, id).is_err(),
                "deletedAt={id}"
            );
            assert!(AgentRepository::exists(&tx, id).unwrap());
        }
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&BooleanEpoch::QUERY, &BooleanEpoch)
            .unwrap();
        for id in ["true", "false"] {
            assert!(AgentRepository::get(&tx, &id.into()).is_err());
            assert!(
                crate::RulesRepository::owner(&tx, id).is_err(),
                "epoch={id}"
            );
        }
    }

    #[test]
    fn agent_liveness_reads_distinguish_zero_deleted_and_missing_rows() {
        let db = crate::fresh_state::test_canvas();
        assert!(!AgentRepository::exists(&db, "missing").unwrap());
        assert_eq!(
            AgentRepository::deletion_status(&db, "missing").unwrap(),
            None
        );
        for (id, deletion, deleted) in [
            ("live", json!(null), false),
            ("zero", json!(0), false),
            ("deleted", json!(1.5), true),
        ] {
            let mut agent = AgentRecord::from_value(json!({
                "compactions":0,"accountKey":"default","id":id,"rootId":id,
                "provider":"codex","epoch":1,"status":"idle","deletedAt":deletion
            }))
            .unwrap();
            // Archived state does not determine AgentRecord::is_deleted.
            agent.archived = studio_runtime_schema::queue_command::Field::Value(true);
            AgentRepository::put(&db, &agent).unwrap();
            let scope = crate::agent_volatile::publication_work::scope();
            assert!(AgentRepository::exists(&db, id).unwrap());
            assert_eq!(
                AgentRepository::deletion_status(&db, id).unwrap(),
                Some(deleted)
            );
            assert_eq!(
                crate::agent_volatile::publication_work::take_full_decodes(),
                0
            );
            drop(scope);
            assert_eq!(
                AgentRepository::get(&db, &id.into())
                    .unwrap()
                    .unwrap()
                    .is_deleted(),
                deleted
            );
        }
    }

    #[test]
    fn receipt_domain_quarantine_does_not_decode_malformed_agent() {
        let db = rusqlite::Connection::open_in_memory().expect("database");
        db.execute_batch(&format!(
            "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT,hold_status TEXT {},hold_incident TEXT); INSERT INTO runtime_agents(id,record,hold_status) VALUES('broken','malformed','clear')",
            studio_runtime_schema::persistence::enum_check::<HoldStatus>("hold_status")
        )).expect("owner shape");
        AgentRepository::hold(&db, "broken", HoldStatus::Invalid, Some("incident"))
            .expect("independent quarantine");
        let state: (HoldStatus, String, String) = db
            .query_row(
                "SELECT hold_status,hold_incident,record FROM runtime_agents WHERE id='broken'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("scalar authority");
        assert_eq!(
            state,
            (HoldStatus::Invalid, "incident".into(), "malformed".into())
        );
    }

    #[test]
    fn valid_agent_repair_clears_the_invalid_record_hold() {
        let mut connection = crate::fresh_state::test_canvas();
        let agent = AgentRecord::from_value(json!({"compactions":0,"accountKey":"default",
            "id":"agent", "provider":"codex", "rootId":"agent", "epoch":1, "status":"idle"
        }))
        .expect("valid repair record");
        AgentRepository::put(&connection, &agent).expect("seed agent");
        AgentRepository::hold(&connection, "agent", HoldStatus::Invalid, Some("incident"))
            .expect("seed hold");
        let tx = connection.transaction().expect("repair transaction");
        AgentRepository::put(&tx, &agent).expect("typed repair saves and clears hold");
        tx.commit().expect("repair commits atomically");
        let status: HoldStatus = connection
            .query_row(
                "SELECT hold_status FROM runtime_agents WHERE id='agent'",
                [],
                |row| row.get(0),
            )
            .expect("hold");
        assert_eq!(status, HoldStatus::Clear);
    }
    #[test]
    fn receipt_domain_fired_event_set_retains_all_names_and_rolls_back() {
        let mut db = crate::fresh_state::test_canvas();
        let root = AgentRecord::from_value(
            json!({"compactions":0,"accountKey":"default","id":"root","rootId":"root","provider":"codex","epoch":1,"status":"idle"}),
        )
        .expect("root");
        AgentRepository::put(&db, &root).expect("save");
        let tx = db.transaction().expect("tx");
        assert!(AgentRepository::fire_event(&tx, "root", "one").expect("first"));
        assert!(AgentRepository::fire_event(&tx, "root", "two").expect("second"));
        assert!(!AgentRepository::fire_event(&tx, "root", "one").expect("replay"));
        tx.commit().expect("commit");
        assert!(AgentRepository::event_fired(&db, "root", "one").expect("one"));
        let tx = db.transaction().expect("tx");
        assert!(AgentRepository::fire_event(&tx, "root", "three").expect("three"));
        tx.rollback().expect("rollback");
        assert!(!AgentRepository::event_fired(&db, "root", "three").expect("not fired"));
    }
}
