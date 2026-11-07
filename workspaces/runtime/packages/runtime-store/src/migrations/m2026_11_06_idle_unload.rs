//! Indexed projections for the scheduler's idle-unload decisions.
use crate::StoreResult;
use rusqlite::Transaction;

pub(crate) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for column in crate::agent_columns::AGENT_COLUMNS
        .iter()
        .filter(|column| !column.stored)
    {
        add_column(
            tx,
            "runtime_agents",
            column.name,
            &crate::agent_columns::column_sql(column),
        )?;
    }
    for (table, name, sql_type, expression) in [
        (
            "runtime_monitors",
            "source_account_key",
            "",
            "COALESCE(record->>'$.operation.accountKey',record->>'$.accountKey')",
        ),
        (
            "runtime_account_transfers",
            "target_account_key",
            "",
            "record->>'$.targetAccountKey'",
        ),
        ("runtime_monitors", "agent", "TEXT", "record->>'$.agent'"),
        ("runtime_work", "owner", "TEXT", "record->>'$.owner'"),
    ] {
        add_column(
            tx,
            table,
            name,
            &format!(
                "{name} {sql_type} GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN {expression} END) VIRTUAL"
            ),
        )?;
    }
    if crate::schema::table_exists(tx, "runtime_monitors")? {
        let active = crate::startup_recovery::active_monitor_predicate();
        for (index, field) in [
            ("runtime_monitor_idle_agent", "agent"),
            ("runtime_monitor_idle_account", "source_account_key"),
        ] {
            tx.execute_batch(&format!(
                "CREATE INDEX IF NOT EXISTS {index} ON runtime_monitors({field}) WHERE {active}"
            ))?;
        }
    }
    for (table, sql) in [
        (
            "runtime_agents",
            "CREATE INDEX IF NOT EXISTS runtime_agent_idle_unload ON runtime_agents(account_key,deleted_at,id,account_transfer_id,native_release_reset_pending,is_lead)",
        ),
        (
            "runtime_account_transfers",
            "CREATE INDEX IF NOT EXISTS runtime_transfer_target_account_key ON runtime_account_transfers(json_extract(record,'$.status'),target_account_key)",
        ),
        (
            "runtime_work",
            "CREATE INDEX IF NOT EXISTS runtime_work_idle_owner ON runtime_work(owner,json_extract(record,'$.status'))",
        ),
        (
            "runtime_events",
            "CREATE INDEX IF NOT EXISTS runtime_event_idle_agent_status ON runtime_events(agent,status)",
        ),
    ] {
        if crate::schema::table_exists(tx, table)? {
            tx.execute_batch(sql)?;
        }
    }
    Ok(())
}

fn add_column(tx: &Transaction<'_>, table: &str, name: &str, definition: &str) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, table)? {
        return Ok(());
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo(?1) WHERE name=?2)",
        [table, name],
        |row| row.get(0),
    )?;
    if !exists {
        tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {definition}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn projections_match_legacy_values_and_migration_repeats() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(crate::agent_columns::agent_table_sql())
            .unwrap();
        db.execute_batch("CREATE TABLE runtime_monitors(id TEXT PRIMARY KEY,record TEXT,monitor_status TEXT); CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY,record TEXT); CREATE TABLE runtime_work(id TEXT PRIMARY KEY,record TEXT); CREATE TABLE runtime_events(id TEXT,agent TEXT,status TEXT);").unwrap();
        for (id, record) in [
            ("absent", "{}"),
            (
                "null",
                r#"{"accountTransferId":null,"nativeRelease":{"resetPending":null},"isLead":null}"#,
            ),
            (
                "false",
                r#"{"accountTransferId":false,"nativeRelease":{"resetPending":false},"isLead":false}"#,
            ),
            (
                "true",
                r#"{"accountTransferId":true,"nativeRelease":{"resetPending":true},"isLead":true}"#,
            ),
            (
                "legacy",
                r#"{"accountTransferId":"t","nativeRelease":{"resetPending":1},"isLead":0}"#,
            ),
        ] {
            let agent = format!(r#"{{"provider":"codex",{}}}"#, &record[1..record.len() - 1]);
            let agent = agent.replace(",}", "}");
            db.execute(
                "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
                (id, agent),
            )
            .unwrap();
        }
        db.execute_batch("INSERT INTO runtime_monitors VALUES('legacy','{\"agent\":\"a\",\"accountKey\":\"old\"}','running'),('nested','{\"agent\":null,\"accountKey\":\"old\",\"operation\":{\"accountKey\":\"new\"}}','running'),('bad','{','lost'); INSERT INTO runtime_account_transfers VALUES('t','{\"status\":\"pending\",\"targetAccountKey\":\"x\"}');").unwrap();
        for (id, value) in [
            ("absent", "{}"),
            (
                "null",
                r#"{"agent":null,"accountKey":null,"targetAccountKey":null}"#,
            ),
            (
                "false",
                r#"{"agent":false,"accountKey":false,"targetAccountKey":false}"#,
            ),
            (
                "true",
                r#"{"agent":true,"accountKey":true,"targetAccountKey":true}"#,
            ),
        ] {
            db.execute(
                "INSERT INTO runtime_monitors(id,record,monitor_status) VALUES(?1,?2,'running')",
                (id, value),
            )
            .unwrap();
            db.execute(
                "INSERT INTO runtime_account_transfers(id,record) VALUES(?1,?2)",
                (id, value),
            )
            .unwrap();
        }
        for _ in 0..2 {
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            tx.commit().unwrap();
        }
        let mismatches: i64 = db.query_row("SELECT count(*) FROM runtime_agents WHERE account_transfer_id IS NOT json_extract(record,'$.accountTransferId') OR native_release_reset_pending IS NOT json_extract(record,'$.nativeRelease.resetPending') OR is_lead IS NOT json_extract(record,'$.isLead')", [], |r| r.get(0)).unwrap();
        assert_eq!(mismatches, 0);
        let mismatches: i64 = db.query_row("SELECT count(*) FROM runtime_monitors WHERE json_valid(record) AND source_account_key IS NOT COALESCE(json_extract(record,'$.operation.accountKey'),json_extract(record,'$.accountKey'))", [], |r| r.get(0)).unwrap();
        assert_eq!(mismatches, 0);
        let mismatches: i64 = db.query_row("SELECT count(*) FROM runtime_account_transfers WHERE target_account_key IS NOT json_extract(record,'$.targetAccountKey')", [], |r| r.get(0)).unwrap();
        assert_eq!(mismatches, 0);
        for (id, value) in [
            ("missing", "{}"),
            ("null", r#"{"agent":null,"owner":null}"#),
            ("false", r#"{"agent":false,"owner":false}"#),
            ("true", r#"{"agent":true,"owner":true}"#),
            ("string", r#"{"agent":"1","owner":"1"}"#),
        ] {
            db.execute(
                "INSERT INTO runtime_monitors(id,record,monitor_status) VALUES(?1,?2,'running')",
                (format!("parity-{id}"), value),
            )
            .unwrap();
            db.execute(
                "INSERT INTO runtime_work(id,record) VALUES(?1,?2)",
                (id, value),
            )
            .unwrap();
        }
        for id in ["0", "1", "missing", ""] {
            let monitor: (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM runtime_monitors WHERE monitor_status='running' AND json_extract(record,'$.agent')=a.id), (SELECT count(*) FROM runtime_monitors WHERE monitor_status='running' AND agent=a.id) FROM (SELECT CAST(?1 AS TEXT) AS id) a", [id], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
            let work: (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM runtime_work WHERE json_extract(record,'$.owner')=a.id), (SELECT count(*) FROM runtime_work WHERE owner=a.id) FROM (SELECT CAST(?1 AS TEXT) AS id) a", [id], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(monitor.0, monitor.1, "monitor {id}");
            assert_eq!(work.0, work.1, "work {id}");
        }
        // Legacy malformed monitor records are retained. The old raw extraction
        // errors if evaluated; the guarded projection returns NULL instead.
        assert!(
            db.query_row(
                "SELECT json_extract(record,'$.agent') FROM runtime_monitors WHERE id='bad'",
                [],
                |r| r.get::<_, Option<String>>(0)
            )
            .is_err()
        );
        let malformed: (bool, bool) = db.query_row("SELECT agent IS NULL, source_account_key IS NULL FROM runtime_monitors WHERE id='bad'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(malformed, (true, true));
        let old_active: i64 = db.query_row("SELECT count(*) FROM runtime_monitors WHERE id='bad' AND monitor_status IN ('running','starting','approval') AND json_extract(record,'$.agent')='a'", [], |r| r.get(0)).unwrap();
        let new_active: i64 = db.query_row("SELECT count(*) FROM runtime_monitors WHERE id='bad' AND monitor_status IN ('running','starting','approval') AND agent='a'", [], |r| r.get(0)).unwrap();
        assert_eq!((old_active, new_active), (0, 0));
        let target: String = db
            .query_row(
                "SELECT target_account_key FROM runtime_account_transfers WHERE id='t'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(target, "x");
        for (sql, index) in [
            (
                "SELECT id FROM runtime_agents INDEXED BY runtime_agent_idle_unload WHERE account_key='a' AND deleted_at IS NULL",
                "runtime_agent_idle_unload",
            ),
            (
                "SELECT id FROM runtime_monitors WHERE monitor_status IN ('running','starting','approval') AND source_account_key='a'",
                "runtime_monitor_idle_account",
            ),
            (
                "SELECT id FROM runtime_account_transfers WHERE json_extract(record,'$.status')='pending' AND target_account_key='a'",
                "runtime_transfer_target_account_key",
            ),
            (
                "SELECT id FROM runtime_events WHERE agent='a' AND status IN ('pending','reserved','dispatching','uncertain')",
                "runtime_event_idle_agent_status",
            ),
        ] {
            let plan: String = db
                .query_row(&format!("EXPLAIN QUERY PLAN {sql}"), [], |r| r.get(3))
                .unwrap();
            assert!(plan.contains(index), "{plan}");
        }
    }
}
