//! Retire the generated agent `status`, `in_flight` and `turn_id` columns. The chat
//! and turn rows own these facts (`domain`). Each index that names a retired column
//! goes first. Its owner recreates the current definition at startup.
use crate::{StoreError, StoreResult};
use rusqlite::{OptionalExtension, Transaction};

pub(super) const RETIRED: [&str; 3] = ["status", "in_flight", "turn_id"];

/// Whether `sql` names `column` as a whole identifier. `hold_status` does not name `status`.
fn names_column(sql: &str, column: &str) -> bool {
    sql.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|token| token == column)
}

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for column in RETIRED {
        let kind: Option<i64> = tx
            .query_row(
                "SELECT hidden FROM pragma_table_xinfo('runtime_agents') WHERE name=?1",
                [column],
                |row| row.get(0),
            )
            .optional()?;
        match kind {
            None => {}
            Some(3) => {
                let indexes: Vec<(String, String)> = tx
                    .prepare(
                        "SELECT name,sql FROM sqlite_master WHERE type='index' AND tbl_name='runtime_agents' AND sql IS NOT NULL",
                    )?
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<_, _>>()?;
                for (index, _) in indexes
                    .into_iter()
                    .filter(|(_, sql)| names_column(sql, column))
                {
                    tx.execute_batch(&format!("DROP INDEX IF EXISTS \"{index}\""))?;
                }
                tx.execute_batch(&format!("ALTER TABLE runtime_agents DROP COLUMN {column}"))?
            }
            Some(_) => {
                return Err(StoreError::RecordContract(
                    "retired agent column is not stored generated",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    fn fixture() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL,\
             account_key TEXT NOT NULL DEFAULT 'default', deleted_at TEXT, role TEXT, parent_id TEXT,\
             agent_archived INTEGER NOT NULL DEFAULT 0, review_archive_failed INTEGER NOT NULL DEFAULT 0,\
             review_archive_scheduled INTEGER NOT NULL DEFAULT 0, hold_status TEXT NOT NULL DEFAULT 'clear',\
             status GENERATED ALWAYS AS (json_extract(record,'$.status')) STORED,\
             in_flight GENERATED ALWAYS AS (json_extract(record,'$.inFlight')) STORED,\
             turn_id GENERATED ALWAYS AS (json_extract(record,'$.turnId')) STORED);\
             CREATE TABLE changes(id TEXT);\
             CREATE TRIGGER agent_change AFTER UPDATE ON runtime_agents BEGIN INSERT INTO changes VALUES(new.id); END;\
             CREATE INDEX runtime_agent_account ON runtime_agents(account_key,deleted_at);\
             CREATE INDEX runtime_agent_account_wake_candidates_v6 ON runtime_agents(account_key,id) WHERE status='queued' AND in_flight=0;\
             CREATE INDEX runtime_agent_deleted_unsettled_v2 ON runtime_agents(id) WHERE COALESCE(turn_id,0)!=0;\
             CREATE INDEX runtime_agent_reviewer_archive_pending_v2 ON runtime_agents(parent_id,id) WHERE role='reviewer' AND status='completed';\
             CREATE INDEX runtime_agent_callback_context_v3 ON runtime_agents(account_key,turn_id) WHERE status='running';\
             CREATE INDEX runtime_agent_holds ON runtime_agents(id) WHERE hold_status!='clear';\
             INSERT INTO runtime_agents(id,record,hold_status) VALUES('a','{\"status\":\"queued\",\"turnId\":\"t\"}','blocked');",
        )
        .unwrap();
        db
    }

    #[test]
    fn retired_columns_drop_their_indexes_and_keep_rows_and_triggers() {
        let mut db = fixture();
        for _ in 0..2 {
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            tx.commit().unwrap();
        }
        let columns: i64 = db
            .query_row(
                "SELECT count(*) FROM pragma_table_xinfo('runtime_agents') WHERE name IN ('status','in_flight','turn_id')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(columns, 0);
        let indexes: Vec<String> = db
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='runtime_agents' AND sql IS NOT NULL ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(indexes, ["runtime_agent_account", "runtime_agent_holds"]);
        let row: (String, String) = db
            .query_row(
                "SELECT record,hold_status FROM runtime_agents WHERE id='a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row.0, "{\"status\":\"queued\",\"turnId\":\"t\"}");
        assert_eq!(row.1, "blocked");
        db.execute("UPDATE runtime_agents SET record='{}' WHERE id='a'", [])
            .unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM changes", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn a_plain_column_with_a_retired_name_is_refused_and_rolled_back() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_agents(id TEXT, record TEXT, status TEXT);\
             CREATE INDEX runtime_agent_account_wake_candidates_v6 ON runtime_agents(id);",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        assert!(super::apply(&tx).is_err());
        tx.rollback().unwrap();
        let indexes: i64 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='runtime_agent_account_wake_candidates_v6'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 1);
    }

    #[test]
    fn names_match_whole_identifiers_only() {
        assert!(super::names_column("WHERE status='x'", "status"));
        assert!(super::names_column("json(record,'$.status')", "status"));
        assert!(!super::names_column("WHERE hold_status!='clear'", "status"));
        assert!(!super::names_column(
            "(steer_rejected_turn_id,'')",
            "turn_id"
        ));
    }
}
