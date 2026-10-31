use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    const FLAGS: &[&str] = &[
        "compactionsObservedOnly",
        "fastMode",
        "daybreakEnabled",
        "needsTitle",
        "manualName",
        "maxAgentsExplicit",
        "agentModeSupported",
    ];
    let flags = FLAGS
        .iter()
        .map(|field| format!("'{field}'"))
        .collect::<Vec<_>>()
        .join(",");
    let mut query = tx.prepare(&format!("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key IN ('nativeNameSynced','agentModeChangedAt','agentModeChangedBy','tokenBudget') OR (j.key IN ({flags}) AND (j.type NOT IN ('true','false') OR (SELECT count(*) FROM json_each(record) d WHERE d.key=j.key)>1)))"))?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: serde_json::Value = serde_json::from_str(&raw)?;
        let Some(object) = record.as_object_mut() else {
            continue;
        };
        for field in [
            "nativeNameSynced",
            "agentModeChangedAt",
            "agentModeChangedBy",
            "tokenBudget",
        ] {
            object.remove(field);
        }
        for field in FLAGS {
            if object.get(*field).is_some_and(|value| !value.is_boolean()) {
                object.remove(*field);
            }
        }
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn removes_unused_fields_and_normalizes_compaction_flag() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
        INSERT INTO runtime_agents VALUES
          ('legacy','{"id":"legacy","nativeNameSynced":true,"agentModeChangedAt":1,"agentModeChangedBy":"user","tokenBudget":1000,"compactionsObservedOnly":"bad","fastMode":"bad","daybreakEnabled":1,"needsTitle":[],"manualName":{},"maxAgentsExplicit":null,"agentModeSupported":"bad"}'),
          ('duplicate','{"id":"duplicate","compactionsObservedOnly":false,"compactionsObservedOnly":true}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let legacy: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy, r#"{"id":"legacy"}"#);
        let duplicate: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='duplicate'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(duplicate.matches("compactionsObservedOnly").count(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&duplicate).unwrap()["compactionsObservedOnly"],
            true
        );
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn real_agent_core_migrates_without_refusal() {
        let path = std::env::var_os("STUDIO_AGENT_CORE_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_agents").unwrap() {
            println!("agent table absent; rows=0 refused=0");
            return;
        }
        super::super::m2026_10_31_native_status_error::apply(&tx).unwrap();
        super::super::m2026_10_31_agent_permissions::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let mut query = tx.prepare("SELECT id,record FROM runtime_agents").unwrap();
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut total = 0;
        let mut refused = 0;
        for row in rows {
            let (id, raw) = row.unwrap();
            total += 1;
            if crate::records::parse_agent_record(&id, &raw).is_err() {
                refused += 1;
            }
        }
        println!("agent rows={total} refused={refused}");
        assert_eq!(refused, 0);
        drop(query);
        tx.commit().unwrap();
    }
}
