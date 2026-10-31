use crate::StoreResult;
use rusqlite::Transaction;
use studio_runtime_schema::creation_intent::AgentRole;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let roles = [
        AgentRole::Orchestrator,
        AgentRole::Implementer,
        AgentRole::Reviewer,
    ]
    .map(|role| format!("'{}'", role.as_str()))
    .join(",");
    let mut query = tx.prepare(&format!("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key='role' AND (j.type NOT IN ('text','null') OR (j.type='text' AND j.value NOT IN ({roles})) OR (SELECT count(*) FROM json_each(record) d WHERE d.key=j.key)>1))"))?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&raw)?;
        if record
            .get("role")
            .is_some_and(|value| serde_json::from_str::<Option<AgentRole>>(value.get()).is_err())
        {
            record.remove("role");
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
    fn normalizes_bad_and_duplicate_roles() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
          INSERT INTO runtime_agents VALUES('bad','{"id":"bad","role":"future"}'),('duplicate','{"id":"duplicate","role":"reviewer","role":"implementer"}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let bad: Option<String> = tx
            .query_row(
                "SELECT json_type(record,'$.role') FROM runtime_agents WHERE id='bad'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bad, None);
        let (role, raw): (String, String) = tx.query_row("SELECT json_extract(record,'$.role'),record FROM runtime_agents WHERE id='duplicate'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(role, "implementer");
        assert_eq!(raw.matches("\"role\"").count(), 1);
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn real_agent_roles_migrate_without_refusal() {
        let path = std::env::var_os("STUDIO_AGENT_ROLE_COPY").unwrap();
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
        super::super::m2026_10_31_agent_text::apply(&tx).unwrap();
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
