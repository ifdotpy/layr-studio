use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    use studio_runtime_schema::ProviderKind;
    let providers = ProviderKind::ALL
        .map(|provider| format!("'{}'", provider.key()))
        .join(",");
    let max_revision = studio_runtime_schema::shared_values::MAX_SAFE_INTEGER;
    let mut query = tx.prepare(&format!("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key='complaintMisses' OR (j.key='layrReady' AND j.type NOT IN ('true','false')) OR (j.key IN ('lastCompletedTurn','hostProjectPath','executionSettingsAccountKey','lastCompletedTurnStatus') AND j.type NOT IN ('text','null')) OR (j.key='provider' AND (j.type NOT IN ('text','null') OR (j.type='text' AND j.value NOT IN ({providers})))) OR (j.key='created' AND (j.type NOT IN ('integer','real','null') OR j.atom < 0)) OR (j.key='queueMutationRevision' AND (j.type != 'integer' OR j.atom < 0 OR j.atom > {max_revision})) OR (j.key IN ('layrReady','lastCompletedTurn','hostProjectPath','executionSettingsAccountKey','lastCompletedTurnStatus','provider','created','queueMutationRevision') AND (SELECT count(*) FROM json_each(record) d WHERE d.key=j.key)>1))"))?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&raw)?;
        record.remove("complaintMisses");
        if record
            .get("layrReady")
            .is_some_and(|v| serde_json::from_str::<bool>(v.get()).is_err())
        {
            record.remove("layrReady");
        }
        for key in [
            "lastCompletedTurn",
            "hostProjectPath",
            "executionSettingsAccountKey",
            "lastCompletedTurnStatus",
        ] {
            if record
                .get(key)
                .is_some_and(|v| serde_json::from_str::<Option<String>>(v.get()).is_err())
            {
                record.remove(key);
            }
        }
        if record
            .get("provider")
            .is_some_and(|v| serde_json::from_str::<Option<ProviderKind>>(v.get()).is_err())
        {
            record.remove("provider");
        }
        if record.get("created").is_some_and(|v| {
            serde_json::from_str::<Option<studio_runtime_schema::NonnegativeNumber>>(v.get())
                .is_err()
        }) {
            record.remove("created");
        }
        if record.get("queueMutationRevision").is_some_and(|v| {
            serde_json::from_str::<u64>(v.get())
                .map_or(true, |revision| revision > max_revision as u64)
        }) {
            record.remove("queueMutationRevision");
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
    fn agent_batch_55_normalizes_all_owned_fields() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
        INSERT INTO runtime_agents VALUES('legacy','{"id":"legacy","provider":"future","created":-1,"complaintMisses":9,"layrReady":"bad","queueMutationRevision":-1,"lastCompletedTurn":42,"hostProjectPath":42,"executionSettingsAccountKey":42,"lastCompletedTurnStatus":42}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row("SELECT record FROM runtime_agents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(raw, r#"{"id":"legacy"}"#);
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn real_agent_batch_55_migrates_without_refusal() {
        let path = std::env::var_os("STUDIO_AGENT_BATCH_55_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_agents").unwrap() {
            println!("agent table absent; rows=0 refused=0");
            return;
        }
        super::super::m2026_10_31_native_status_error::apply(&tx).unwrap();
        super::super::m2026_10_31_agent_permissions::apply(&tx).unwrap();
        super::super::m2026_10_31_agent_core::apply(&tx).unwrap();
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
