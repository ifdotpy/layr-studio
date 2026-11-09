//! Retire re-parenting while retaining team insertion/deletion/restore visibility.
use rusqlite::Transaction;
use studio_db_api::Table;

use crate::StoreResult;

const LEGACY_VISIBILITY_PREFIX: &str = "entity_scope_visibility:";

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let old_publication_state: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('agent_publication_state') WHERE name IN ('root_id','is_lead'))",
        [],
        |row| row.get(0),
    )?;
    if old_publication_state {
        tx.execute_batch(
            "ALTER TABLE agent_publication_state RENAME TO reparent_publication_state",
        )?;
        crate::clock_schema::AgentPublicationState::descriptor().install_if_absent(tx)?;
        tx.execute_batch(
            "INSERT INTO agent_publication_state(id,live,in_flight,team_counts)
            SELECT id,live,in_flight,team_counts FROM reparent_publication_state;
            DROP TABLE reparent_publication_state;",
        )?;
    }
    for index in [
        "runtime_conversion_rules_rootId",
        "runtime_conversion_plans_rootId",
        "runtime_conversion_annotations_rootId",
        "runtime_conversion_event_meta_rootId",
        "runtime_conversion_event_meta_leadId",
    ] {
        tx.execute_batch(&format!("DROP INDEX IF EXISTS {index}"))?;
    }
    for trigger in [
        "sync_entity_scope_visibility_insert",
        "sync_entity_scope_visibility_update",
        "sync_entity_scope_tombstone_update",
        "sync_entity_scope_tombstone_delete",
        "durable_runtime_agents_insert",
        "durable_runtime_agents_update",
        "durable_runtime_agents_delete",
    ] {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger}"))?;
    }
    let prefix = crate::repositories::sync_documents::TEAM_MEMBERSHIP_KEY_PREFIX;
    let legacy_json_start = LEGACY_VISIBILITY_PREFIX.len() + 1;
    // Preserve outstanding pre-upgrade invalidations, collapsed to one revision per selector.
    tx.execute_batch(&format!(
        "INSERT INTO sync_entity_meta(key,value)
           SELECT '{prefix}' || json_quote(json_extract(substr(key,{legacy_json_start}),'$[1]')),
                  CAST(MAX(CAST(value AS INTEGER)) AS TEXT)
           FROM sync_entity_meta WHERE key LIKE '{LEGACY_VISIBILITY_PREFIX}%'
           GROUP BY json_extract(substr(key,{legacy_json_start}),'$[1]')
         ON CONFLICT(key) DO UPDATE SET value=CAST(MAX(CAST(sync_entity_meta.value AS INTEGER),CAST(excluded.value AS INTEGER)) AS TEXT);
         DELETE FROM sync_entity_meta WHERE key LIKE '{LEGACY_VISIBILITY_PREFIX}%'
            OR key LIKE 'entity_scope_tombstone:%';
         UPDATE runtime_agents SET record=json_remove(record,'$.convertedFromLead')
           WHERE json_valid(record) AND json_type(record,'$.convertedFromLead') IS NOT NULL;"
    ))?;
    for table in crate::durable_seq::DURABLE_TABLES {
        for operation in ["insert", "update", "delete"] {
            tx.execute_batch(&format!(
                "DROP TRIGGER IF EXISTS durable_{}_{operation}",
                table.name
            ))?;
        }
    }
    for name in ["summary_task_delete", "summary_messageReceipt_delete"] {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {name}"))?;
    }
    crate::feature_schema::install_summary_delete_triggers(tx)?;
    crate::durable_seq::install(tx)?;
    crate::entity_scopes::install(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_schema(db: &rusqlite::Connection) -> Vec<(String, String)> {
        db.prepare(
            "SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    #[test]
    fn upgraded_and_fresh_entity_scopes_have_identical_schema_and_rerun_is_idempotent() {
        let fresh = crate::fresh_state::test_canvas();
        let mut upgraded = crate::fresh_state::test_canvas();
        let tx = upgraded.transaction().unwrap();
        for (name, _) in owned_schema(&tx).into_iter().filter(|(name, _)| {
            name.starts_with("immutable_scope_") || name.starts_with("durable_")
        }) {
            tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {name}"))
                .unwrap();
        }
        tx.execute_batch("ALTER TABLE durable_rows DROP COLUMN scope;
            ALTER TABLE agent_publication_state ADD COLUMN root_id TEXT;
            ALTER TABLE agent_publication_state ADD COLUMN is_lead INTEGER;
            CREATE INDEX runtime_conversion_plans_rootId ON runtime_plans(json_extract(record,'$.rootId')) WHERE json_extract(record,'$.rootId') IS NOT NULL;
            CREATE TRIGGER sync_entity_scope_visibility_update AFTER UPDATE ON sync_entities BEGIN SELECT 1; END;
            INSERT INTO sync_entity_meta VALUES('entity_scope_visibility:[\"*\",\"lead\"]','123');
            INSERT INTO sync_entity_meta VALUES('entity_scope_tombstone:[\"task\",\"old\"]','[]');
            INSERT INTO runtime_agents(id,record) VALUES('old','{\"id\":\"old\",\"rootId\":\"lead\",\"parentId\":\"lead\",\"provider\":\"codex\",\"accountKey\":\"default\",\"epoch\":1,\"compactions\":0,\"status\":\"idle\",\"convertedFromLead\":{}}');").unwrap();
        apply(&tx).unwrap();
        assert_eq!(owned_schema(&tx), owned_schema(&fresh));
        let once = owned_schema(&tx);
        let high = crate::durable_seq::high(&tx).unwrap();
        apply(&tx).unwrap();
        assert_eq!(owned_schema(&tx), once);
        assert_eq!(crate::durable_seq::high(&tx).unwrap(), high);
        let legacy: i64 = tx
            .query_row(
                "SELECT count(*) FROM sync_entity_meta WHERE key LIKE 'entity_scope_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy, 0);
        let revision: String = tx
            .query_row(
                "SELECT value FROM sync_entity_meta WHERE key='team_membership:\"lead\"'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revision, "123");
        let identity: (String, String, Option<String>) = tx.query_row("SELECT json_extract(record,'$.rootId'),json_extract(record,'$.parentId'),json_extract(record,'$.convertedFromLead') FROM runtime_agents WHERE id='old'", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
        assert_eq!(identity, ("lead".into(), "lead".into(), None));
    }
}
