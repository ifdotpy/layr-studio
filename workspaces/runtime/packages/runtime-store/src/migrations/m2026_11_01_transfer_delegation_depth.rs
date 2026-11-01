use crate::StoreResult;
use rusqlite::{Transaction, params};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for (table, paths) in [
        (
            "runtime_agents",
            "(j.path='$.workerDefaults' OR j.path='$.lazyAccountTransfer.sourceState.workerDefaults')",
        ),
        (
            "runtime_account_transfers",
            "(j.path GLOB '$.members.*.settings.workerDefaults' OR j.path GLOB '$.members.*.targetSettings.workerDefaults')",
        ),
    ] {
        if !crate::schema::table_exists(tx, table)? {
            continue;
        }
        let sql = format!(
            "SELECT a.id,j.fullkey,j.type,j.atom FROM {table} a,json_tree(CASE WHEN json_valid(a.record) THEN a.record ELSE '{{}}' END) j WHERE j.key='delegationDepth' AND {paths}"
        );
        let mut query = tx.prepare(&sql)?;
        let rows = query
            .query_map([], |row| {
                let kind: String = row.get(2)?;
                let valid = kind == "integer" && match row.get_ref(3)? {
                    rusqlite::types::ValueRef::Integer(depth) => {
                        studio_runtime_schema::agent_entity_nested::WorkerDelegationDepth::try_from(
                            depth,
                        )
                        .is_ok()
                    }
                    _ => false,
                };
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, valid))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, path, valid) in rows {
            if !valid {
                tx.execute(
                    &format!("UPDATE {table} SET record=json_remove(record,?2) WHERE id=?1"),
                    params![id, path],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_delegation_depth_inherits_without_dropping_records() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_agents VALUES('agent','{"workerDefaults":{"model":"m","delegationDepth":4},"future":42}');
            INSERT INTO runtime_agents VALUES('overflow','{"workerDefaults":{"delegationDepth":9223372036854775808}}');
            INSERT INTO runtime_account_transfers VALUES('move','{"members":{"agent":{"settings":{"workerDefaults":{"delegationDepth":0,"model":"m"}},"targetSettings":{"workerDefaults":{"delegationDepth":2}}}}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let agent: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='agent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let transfer: String = tx
            .query_row(
                "SELECT record FROM runtime_account_transfers WHERE id='move'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let overflow: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='overflow'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&overflow).unwrap(),
            serde_json::json!({"workerDefaults":{}})
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&agent).unwrap(),
            serde_json::json!({"workerDefaults":{"model":"m"},"future":42})
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&transfer).unwrap(),
            serde_json::json!({"members":{"agent":{"settings":{"workerDefaults":{"model":"m"}},"targetSettings":{"workerDefaults":{"delegationDepth":2}}}}})
        );
    }
}
