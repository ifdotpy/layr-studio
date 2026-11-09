use crate::interaction_tables::{GraphEdgesRow, GroupsRow, MessagesRow};
use rusqlite::Transaction;
use studio_db_api::Table;

use crate::StoreResult;

// These tables are installed by Runtime and feature constructors, or on first
// feature use, in canvas.sqlite3. The Rust feature schema owns their current DDL.
// `runtime_budget` retains historical state and replay diagnostics.
const FEATURE_SCHEMA: &str = r#"
            CREATE TABLE IF NOT EXISTS runtime_work (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_work_root_status
                ON runtime_work(json_extract(record,'$.rootId'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_work_owner_status
                ON runtime_work(json_extract(record,'$.owner'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_work_owner_root_status_id ON runtime_work(
                json_extract(record,'$.owner'),json_extract(record,'$.rootId'),
                json_extract(record,'$.status'),id);
            CREATE INDEX IF NOT EXISTS runtime_work_archive_due ON runtime_work(
                COALESCE(json_extract(record,'$.archiveIntent.nextAttemptAt'),0),id)
                WHERE json_extract(record,'$.archiveIntent.status')='pending';
            CREATE TABLE IF NOT EXISTS runtime_plans (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_annotations (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_analytics_projection (seq INTEGER PRIMARY KEY AUTOINCREMENT, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_event_meta (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_item_fulltext (id TEXT PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_item_blobs (id TEXT PRIMARY KEY, hash TEXT NOT NULL, size INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_search_next_meta (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, kind TEXT NOT NULL,
                search_rowid INTEGER NOT NULL UNIQUE);
            CREATE TABLE IF NOT EXISTS runtime_search_rollout (
                id INTEGER PRIMARY KEY CHECK(id=1), phase TEXT NOT NULL,
                cursor INTEGER NOT NULL DEFAULT 0, updated REAL NOT NULL);
            CREATE VIRTUAL TABLE IF NOT EXISTS runtime_search_next USING fts5(body,content='',contentless_delete=1,tokenize='unicode61');
            CREATE TABLE IF NOT EXISTS runtime_checkpoints (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_projects (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_lead_requests (id TEXT PRIMARY KEY, agent TEXT NOT NULL, signature TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_sidebar_order (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_workspace_operations (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_workspace_operation_phase ON runtime_workspace_operations(
                json_extract(record,'$.phase'), json_extract(record,'$.agent'));
            CREATE TABLE IF NOT EXISTS runtime_tool_requests (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_tool_request_actor
                ON runtime_tool_requests(json_extract(record,'$.agent'), json_extract(record,'$.updated') DESC);
            CREATE TABLE IF NOT EXISTS runtime_rules (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            -- What the last publication of each agent's views saw: the publisher compares
            -- it with the new row to find team, room and liveness changes, and keeps a
            -- lead's team counts so a member change marks the lead only when they
            -- change. The views themselves are built at read time.
            -- Owner, created time and running state of each task in the client task
            -- window, so a task write can place it without its document.
            CREATE TABLE IF NOT EXISTS task_window_state (id TEXT PRIMARY KEY,
                agent TEXT NOT NULL, created REAL, running INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS task_window_state_history
                ON task_window_state(created DESC) WHERE NOT running;
            -- sync_policy::TASK_WINDOW_EVICT_SQL scans live members, excluding retained tombstones.
            CREATE INDEX IF NOT EXISTS durable_rows_task_window_live
                ON durable_rows(id) WHERE tbl='task_window' AND deleted=0;
            CREATE TABLE IF NOT EXISTS agent_publication_state (id TEXT PRIMARY KEY,
                root_id TEXT, is_lead INTEGER, live INTEGER NOT NULL, in_flight INTEGER NOT NULL,
                team_counts TEXT);
            CREATE INDEX IF NOT EXISTS runtime_rule_ready_tick
                ON runtime_rules(json_extract(record,'$.status'))
                WHERE NOT COALESCE(json_extract(record,'$.inFlight'),0);
            CREATE TABLE IF NOT EXISTS runtime_budget (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_execution_runs (id TEXT PRIMARY KEY, agent TEXT NOT NULL, account TEXT NOT NULL, epoch INTEGER NOT NULL, thread TEXT, turn TEXT, created REAL NOT NULL, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_execution_run_agent ON runtime_execution_runs(agent,created DESC);
            CREATE INDEX IF NOT EXISTS runtime_execution_run_created ON runtime_execution_runs(created DESC);
            CREATE INDEX IF NOT EXISTS runtime_execution_run_message ON runtime_execution_runs(agent,thread,turn);
            CREATE INDEX IF NOT EXISTS runtime_execution_run_native ON runtime_execution_runs(agent,account,thread,turn);
            CREATE TABLE IF NOT EXISTS runtime_execution_attempts (id TEXT PRIMARY KEY, run TEXT NOT NULL, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_execution_attempt_run ON runtime_execution_attempts(run);
            CREATE TABLE IF NOT EXISTS runtime_execution_inputs (run TEXT NOT NULL, event TEXT NOT NULL, PRIMARY KEY(run,event));
            CREATE INDEX IF NOT EXISTS runtime_execution_input_event ON runtime_execution_inputs(event);
            CREATE TABLE IF NOT EXISTS runtime_execution_effects (id TEXT PRIMARY KEY, run TEXT NOT NULL, kind TEXT NOT NULL, reference TEXT NOT NULL, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_execution_effect_run ON runtime_execution_effects(run);
            CREATE TABLE IF NOT EXISTS runtime_execution_nodes (id TEXT PRIMARY KEY, run TEXT NOT NULL, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_execution_node_run ON runtime_execution_nodes(run);
        CREATE TABLE IF NOT EXISTS runtime_orchestration_settings (id TEXT PRIMARY KEY, record TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS runtime_federation_settings (
          id TEXT PRIMARY KEY, record TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS runtime_federation_inbox (
          peer_id TEXT NOT NULL, message_id TEXT NOT NULL, record TEXT NOT NULL,
          PRIMARY KEY(peer_id,message_id));
            CREATE INDEX IF NOT EXISTS runtime_workspace_operation_agent_phase ON runtime_workspace_operations(json_extract(record,'$.agent'),json_extract(record,'$.phase'));
            CREATE INDEX IF NOT EXISTS runtime_request_agent_status ON runtime_requests(json_extract(record,'$.agent'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_rule_agent_inflight ON runtime_rules(json_extract(record,'$.agent'),json_extract(record,'$.inFlight'));
            CREATE INDEX IF NOT EXISTS runtime_conversion_rules_rootId ON runtime_rules(json_extract(record,'$.rootId')) WHERE json_extract(record,'$.rootId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_conversion_plans_rootId ON runtime_plans(json_extract(record,'$.rootId')) WHERE json_extract(record,'$.rootId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_conversion_annotations_rootId ON runtime_annotations(json_extract(record,'$.rootId')) WHERE json_extract(record,'$.rootId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_conversion_complaints_leadId ON runtime_complaints(json_extract(record,'$.leadId')) WHERE json_extract(record,'$.leadId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_conversion_event_meta_rootId ON runtime_event_meta(json_extract(record,'$.rootId')) WHERE json_extract(record,'$.rootId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_conversion_event_meta_leadId ON runtime_event_meta(json_extract(record,'$.leadId')) WHERE json_extract(record,'$.leadId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_room_root ON runtime_rooms(json_extract(record,'$.rootId'));
            CREATE INDEX IF NOT EXISTS runtime_room_member_first ON runtime_rooms(json_extract(record,'$.members[0]'));
            CREATE INDEX IF NOT EXISTS runtime_room_member_second ON runtime_rooms(json_extract(record,'$.members[1]'));
            CREATE INDEX IF NOT EXISTS runtime_room_many_members ON runtime_rooms(json_array_length(record,'$.members')) WHERE json_array_length(record,'$.members')>2;
            CREATE TABLE IF NOT EXISTS user_terminals (id TEXT PRIMARY KEY, record TEXT NOT NULL, output TEXT NOT NULL, offset INTEGER NOT NULL, output_generation INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE IF NOT EXISTS user_terminal_output (terminal TEXT NOT NULL, start INTEGER NOT NULL, end INTEGER NOT NULL, text TEXT NOT NULL, PRIMARY KEY (terminal,start));
            CREATE INDEX IF NOT EXISTS user_terminal_output_end ON user_terminal_output (terminal,end);
            CREATE TABLE IF NOT EXISTS user_terminal_event_cursor (terminal TEXT PRIMARY KEY, sequence INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS user_terminal_decoder_state (terminal TEXT PRIMARY KEY, pending BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_access_settings(id INTEGER PRIMARY KEY CHECK(id=1), auto_pair INTEGER NOT NULL);
            INSERT OR IGNORE INTO runtime_access_settings VALUES(1,1);
            CREATE TABLE IF NOT EXISTS runtime_access_local_alias (
          id INTEGER PRIMARY KEY CHECK(id=1), alias TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_native_notices (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_agent_sweep_busy_v2 ON runtime_agents(account_key) WHERE provider='codex' AND (context_repair_phase IN ('preparing','submitted','unknown','ready') OR COALESCE(native_tool_refresh_id,'')!='');
            CREATE INDEX IF NOT EXISTS runtime_agent_sweep_cleanup_v2 ON runtime_agents(source_cleanup_account_key,source_cleanup_thread_id) WHERE source_cleanup_phase IN ('planned','submitted');
            CREATE INDEX IF NOT EXISTS runtime_agent_unsettled_legacy ON runtime_agents(id) WHERE legacy_unsettled=1;
            CREATE INDEX IF NOT EXISTS runtime_idle_compaction_deadline ON runtime_agents(idle_episode_type,idle_episode_fire_at,id,idle_episode_id) WHERE idle_episode_type='object';
            CREATE TABLE IF NOT EXISTS runtime_process_actions (id TEXT PRIMARY KEY, signature TEXT NOT NULL, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_safety_retries (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_account_transfers(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_account_transfer_status ON runtime_account_transfers(json_extract(record,'$.status'));
            CREATE TABLE IF NOT EXISTS runtime_capacity_retries(id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_limit_reset_attempts (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_limit_reset_requests (id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_agent_moves(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_usage_resumes(id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_server_inbox (
                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, state TEXT NOT NULL,
                    result TEXT, created_at REAL NOT NULL DEFAULT 0, completed_at REAL NOT NULL DEFAULT 0, principal TEXT, actor TEXT);
"#;

/// Progress records of resumable passes, in Canvas and, for compacted
/// analytics columns, in analytics.
pub(crate) const PAYLOAD_MIGRATIONS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS runtime_payload_migrations (name TEXT PRIMARY KEY,cursor INTEGER NOT NULL DEFAULT 0,complete INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',updated REAL,error TEXT);
";

/// Every statement that [`prepare`] runs, in order.
fn feature_statements() -> [&'static str; 3] {
    [
        FEATURE_SCHEMA,
        PAYLOAD_MIGRATIONS_SCHEMA,
        crate::stored_text_compaction::BATCHES_SCHEMA,
    ]
}

pub(crate) fn prepare(transaction: &Transaction<'_>) -> StoreResult<()> {
    for descriptor in [
        GroupsRow::descriptor(),
        MessagesRow::descriptor(),
        GraphEdgesRow::descriptor(),
    ] {
        descriptor.install_if_absent(transaction)?;
    }
    crate::voice_storage::prepare(transaction)?;
    crate::access_tables::prepare_features(transaction)?;
    for descriptor in [
        crate::context_settings::AssetsStorageRow::descriptor(),
        crate::context_settings::ProfilesStorageRow::descriptor(),
        crate::context_settings::ModelStatusStorageRow::descriptor(),
    ] {
        descriptor.install_if_absent(transaction)?;
    }
    for schema in feature_statements() {
        transaction.execute_batch(schema)?;
    }
    transaction.execute_batch("CREATE INDEX IF NOT EXISTS runtime_receipt_transcript_item ON runtime_event_meta(json_extract(record,'$.transcriptItemId'))")?;
    // A deleted row is no longer readable by the writer journal. Its source
    // trigger owns the corresponding summary tombstone in the same transaction.
    for (table, collection) in [
        ("runtime_events", "messageReceipt"),
        ("runtime_tasks", "task"),
    ] {
        let tombstone =
            format!("json_object('collection','{collection}','id',OLD.id,'value',json('{{}}'))");
        transaction.execute_batch(&format!(
            "CREATE TRIGGER IF NOT EXISTS summary_{collection}_delete AFTER DELETE ON {table} BEGIN
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             SELECT '{collection}',OLD.id,(SELECT max(seq)+1 FROM (
                 SELECT COALESCE(MAX(seq),0) seq FROM sync_entities UNION ALL
                 SELECT MAX(COALESCE(MAX(seq),0),COALESCE((SELECT value FROM sync_generation WHERE id=1),0)) FROM sync_documents UNION ALL
                 SELECT COALESCE(MAX(seq),0) FROM durable_rows)),
                 sync_entity_hash({tombstone}),{tombstone},1
             WHERE EXISTS(SELECT 1 FROM sync_entities WHERE collection='{collection}' AND id=OLD.id AND deleted=0)
             ON CONFLICT(collection,id) DO UPDATE SET seq=excluded.seq,hash=excluded.hash,payload=excluded.payload,deleted=1;
             END;"
        ))?;
    }
    // Removing a transcript or its metadata can change materialization even
    // though the event itself is unchanged. Journal that event through its
    // source row; semantic event triggers ignore this value-preserving update.
    transaction.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS summary_receipt_meta_delete AFTER DELETE ON runtime_event_meta BEGIN
         UPDATE runtime_events SET id=id WHERE id=OLD.id AND kind='user'; END;
         CREATE TRIGGER IF NOT EXISTS summary_receipt_item_delete AFTER DELETE ON runtime_items BEGIN
         UPDATE runtime_events SET id=id WHERE kind='user' AND agent=OLD.agent AND (
            OLD.id=agent || ':' || id OR id IN (SELECT id FROM runtime_event_meta
                WHERE json_extract(record,'$.transcriptItemId')=OLD.id)); END;"
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::feature_statements;
    use crate::repositories::schema_registrations;
    use rusqlite::Connection;
    use std::collections::BTreeSet;

    #[test]
    fn terminal_generation_preserves_output_and_counts_resets() {
        use crate::TerminalsRepository;
        let mut db = crate::fresh_state::test_canvas();
        let record = studio_json::json!({"id":"terminal","agent":"a","title":"Terminal","cwd":std::env::temp_dir(),"status":"running","created":1});
        db.execute(
            "INSERT INTO user_terminals(id,record,output,offset) VALUES('terminal',?1,'a🙂',0)",
            [record.to_string()],
        )
        .expect("legacy output");
        let tx = db.transaction().expect("upgrade");
        super::prepare(&tx).expect("current schema");
        let row = TerminalsRepository::get(&tx, "terminal")
            .expect("read")
            .expect("terminal");
        assert_eq!(
            (row.output.as_str(), row.offset, row.output_generation),
            ("a🙂", 0, 0)
        );
        for (text, start, generation) in [
            ("a🙂中", 0, 0),
            ("中", 2, 0),
            ("x", 0, 1),
            ("", 0, 2),
            ("y", 0, 2),
        ] {
            TerminalsRepository::put(&tx, "terminal", &row.record, text, start).expect("write");
            let saved = TerminalsRepository::get(&tx, "terminal")
                .expect("read generation")
                .expect("row");
            assert_eq!(saved.output_generation, generation);
        }
        tx.commit().expect("persist resets");
    }

    #[test]
    fn feature_tables_are_not_core_repository_requirements() {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        crate::access_tables::prepare_features(&connection).expect("access feature owners");
        let mut table_statements = 0;
        for statement in feature_statements()
            .iter()
            .flat_map(|schema| schema.split(';'))
        {
            let statement = statement.trim();
            let uppercase = statement.to_ascii_uppercase();
            if uppercase.starts_with("CREATE TABLE")
                || uppercase.starts_with("CREATE VIRTUAL TABLE")
            {
                connection
                    .execute(statement, [])
                    .expect("feature schema table statement executes");
                table_statements += 1;
            }
        }
        assert!(table_statements > 0, "feature table DDL is discovered");

        let created_tables = connection
            .prepare(
                "SELECT name FROM sqlite_schema \
                 WHERE type='table' AND sql IS NOT NULL ORDER BY name",
            )
            .expect("SQLite schema query prepares")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("feature tables are queried")
            .map(|table| table.expect("feature table name is readable"))
            .collect::<BTreeSet<_>>();

        let mut core_violations = Vec::new();
        for registration in schema_registrations() {
            for table in registration.requirements.tables {
                if created_tables.contains(table.name) {
                    core_violations.push(format!(
                        "{} declares bootstrap-created table {} as core",
                        registration.repository, table.name
                    ));
                }
            }
            for table in registration.requirements.bootstrap_tables {
                if created_tables.contains(table.name) {
                    let exists: bool = connection
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                            [table.name],
                            |row| row.get(0),
                        )
                        .expect("bootstrap table existence is checked after creation");
                    assert!(exists, "{} bootstrap table is created", table.name);
                }
            }
        }

        assert!(
            core_violations.is_empty(),
            "feature schema tables must be bootstrap requirements, not core requirements: {core_violations:?}"
        );
    }
}
