//! The agent views (`AGENT_VIEW_COLLECTIONS`) are built at read time from
//! `runtime_agents` (`entity_scope` in runtime-services). Delete their copies in
//! `sync_entities`, the view log that mirrored them, and the triggers that read team
//! membership from the `agentSummary` copy. The sync schema and the durable source then
//! create the new triggers, which read membership from `runtime_agents`. The counter
//! keeps every number already used.
use rusqlite::Transaction;

use crate::StoreResult;

/// Triggers whose SQL changes, or that wrote the removed copies.
const RETIRED_TRIGGERS: &[&str] = &[
    "sync_entity_scope_visibility_insert",
    "sync_entity_scope_visibility_update",
    "sync_entity_scope_tombstone_update",
    "summary_retainedWorktree_delete",
    "durable_view_log_insert",
    "durable_view_log_update",
    "durable_view_log_delete",
    "durable_runtime_agents_insert",
    "durable_runtime_agents_update",
    "durable_runtime_agents_delete",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    for trigger in RETIRED_TRIGGERS {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
    }
    tx.execute_batch("DROP TABLE IF EXISTS durable_view_log")?;
    let list = crate::AGENT_VIEW_COLLECTIONS
        .iter()
        .map(|collection| format!("'{collection}'"))
        .collect::<Vec<_>>()
        .join(",");
    let copies: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ({list}))"),
        [],
        |row| row.get(0),
    )?;
    // The publisher compares a new agent row with what it last published. Keep that
    // from the copies before they go, so the first publication sees no false change.
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_publication_state (id TEXT PRIMARY KEY,
             live INTEGER NOT NULL, in_flight INTEGER NOT NULL,
             team_counts TEXT);
         INSERT OR IGNORE INTO agent_publication_state(id,live,in_flight,team_counts)
           SELECT id,deleted=0,COALESCE(json_extract(payload,'$.value.inFlight'),0),
                  (SELECT json_extract(summary.payload,'$.value.teamCounts') FROM sync_entities AS summary
                    WHERE summary.collection='agentSummary' AND summary.id=sync_entities.id)
           FROM sync_entities WHERE collection='agent';",
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute_batch(&format!(
            "DELETE FROM sync_entities WHERE collection IN ({list})"
        ))?;
    }
    crate::bootstrap::prepare_sync_schema(tx)?;
    crate::durable_seq::install(tx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_copies_go_and_the_new_triggers_read_runtime_agents() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('agentSummary','a',999999,'h','{}',0),('workspace','current',5,'h','{}',0)",
            [],
        )
        .expect("old copies");
        apply(&tx).expect("migration applies");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='agentSummary'"),
            0
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='workspace'"),
            1
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM sqlite_master WHERE name='durable_view_log'"),
            0
        );
        // A new agent of a team moves the team fence at its own number.
        tx.execute(
            "INSERT INTO runtime_agents(id,record) VALUES('m','{\"id\":\"m\",\"rootId\":\"lead\",\"provider\":\"codex\",\"accountKey\":\"default\",\"epoch\":0,\"compactions\":0,\"status\":\"idle\"}')",
            [],
        )
        .expect("member");
        let seq = count("SELECT seq FROM durable_rows WHERE tbl='runtime_agents' AND id='m'");
        assert!(seq > 999_999);
        let fence: String = tx
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key='team_membership:' || json_quote('lead')",
                [],
                |row| row.get(0),
            )
            .expect("team fence");
        assert_eq!(fence, seq.to_string());
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
