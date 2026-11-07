//! Index the durable cursor used to finish account-wide authentication proof fanout.
use crate::{StoreResult, agent_columns};
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, agent_columns::AGENT_TABLE)? {
        return Ok(());
    }
    let column = &agent_columns::AUTH_RESUME_PROOF_COLUMN;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo('runtime_agents') WHERE name=?1)",
        [column.name],
        |row| row.get(0),
    )?;
    if !exists {
        tx.execute_batch(&format!(
            "ALTER TABLE runtime_agents ADD COLUMN {}",
            agent_columns::column_sql(column)
        ))?;
    }
    tx.execute_batch(&format!(
        "CREATE INDEX IF NOT EXISTS runtime_agent_auth_resume_proof ON runtime_agents(id) WHERE {}",
        agent_columns::AUTH_RESUME_PROOF_PENDING_PREDICATE
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, StatementStatus};

    #[test]
    fn auth_resume_proof_migration_tolerates_corrupt_records_and_tracks_writes() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_agents VALUES('bad','{'),('empty','{\"authResumeProofPending\":{}}'),
            ('pending','{\"authResumeProofPending\":{\"acct\":{\"proofAt\":1}}}');").unwrap();
        for (id, field) in [
            ("missing", "{}"),
            ("null", "null"),
            ("string", "\"text\""),
            ("number", "1"),
            ("array", "[]"),
        ] {
            let record = if id == "missing" {
                "{}".to_owned()
            } else {
                format!("{{\"authResumeProofPending\":{field}}}")
            };
            db.execute(
                "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
                [id, &record],
            )
            .unwrap();
        }
        for _ in 0..2 {
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            tx.commit().unwrap();
        }
        let pending = || {
            db.prepare(&format!(
                "SELECT id FROM runtime_agents WHERE {} ORDER BY id",
                crate::agent_columns::AUTH_RESUME_PROOF_PENDING_PREDICATE
            ))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };
        let legacy = "json_type(record,'$.authResumeProofPending')='object' AND json(json_extract(record,'$.authResumeProofPending')) <> '{}'";
        assert!(
            db.prepare(&format!("SELECT id FROM runtime_agents WHERE {legacy}"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .is_err(),
            "the legacy unguarded scan rejects corrupt JSON"
        );
        let legacy_ids = db
            .prepare(&format!(
                "SELECT id FROM runtime_agents WHERE json_valid(record) AND {legacy} ORDER BY id"
            ))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(pending(), legacy_ids);
        assert_eq!(pending(), ["pending"]);
        db.execute(
            "UPDATE runtime_agents SET record='{\"authResumeProofPending\":{}}' WHERE id='pending'",
            [],
        )
        .unwrap();
        assert!(pending().is_empty());
        db.execute("UPDATE runtime_agents SET record='{\"authResumeProofPending\":{\"other\":{}}}' WHERE id='empty'", []).unwrap();
        assert_eq!(pending(), ["empty"]);
    }

    #[test]
    fn auth_resume_proof_write_overhead_is_bounded() {
        let mut db = Connection::open_in_memory().unwrap();
        let columns = crate::agent_columns::AGENT_COLUMNS
            .iter()
            .filter(|column| column.name != crate::agent_columns::AUTH_RESUME_PROOF_COLUMN.name)
            .map(crate::agent_columns::column_sql)
            .collect::<Vec<_>>()
            .join(",");
        db.execute_batch(&format!(
            "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,{columns},record TEXT NOT NULL)"
        ))
        .unwrap();
        // A large unrelated payload makes JSON parsing representative of retained agents.
        let record = studio_json::json!({"provider":"codex","id":"agent","status":"completed","notes":"x".repeat(16_384)}).to_string();
        let write = |db: &Connection| {
            let mut stmt = db.prepare("INSERT INTO runtime_agents(id,record) VALUES('agent',?1) ON CONFLICT(id) DO UPDATE SET record=excluded.record").unwrap();
            stmt.execute([&record]).unwrap();
            stmt.get_status(StatementStatus::VmStep)
        };
        write(&db);
        let before = write(&db);
        let started = std::time::Instant::now();
        for _ in 0..128 {
            write(&db);
        }
        let before_ns = started.elapsed().as_nanos() / 128;
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        tx.commit().unwrap();
        let after = write(&db);
        let started = std::time::Instant::now();
        for _ in 0..128 {
            write(&db);
        }
        let after_ns = started.elapsed().as_nanos() / 128;
        eprintln!("agent record UPSERT average ns: {before_ns} -> {after_ns} (128 writes each)");
        eprintln!(
            "agent record UPSERT VM steps: {before} -> {after} (delta {})",
            after - before
        );
        assert!(
            after - before < 50,
            "proof projection adds bounded write work: {before} -> {after}"
        );
    }
}
