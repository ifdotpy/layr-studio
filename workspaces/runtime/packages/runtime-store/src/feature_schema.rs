use crate::interaction_tables::{GraphEdgesRow, GroupsRow, MessagesRow};
use rusqlite::Transaction;
use studio_db_api::Table;

use crate::StoreResult;

// These tables are installed by Runtime and feature constructors, or on first
// feature use, in canvas.sqlite3. The Rust feature schema owns their current DDL.
const FEATURE_SCHEMA_BEFORE_AGENT_INDEXES: &str = r#"
            CREATE TABLE IF NOT EXISTS runtime_work (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_work_root_status
                ON runtime_work(json_extract(record,'$.rootId'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_work_owner_status
                ON runtime_work(json_extract(record,'$.owner'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_work_owner_root_status_id ON runtime_work(
                json_extract(record,'$.owner'),json_extract(record,'$.rootId'),
                json_extract(record,'$.status'),id);
            CREATE TABLE IF NOT EXISTS runtime_plans (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_annotations (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_search_next_meta (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, kind TEXT NOT NULL,
                search_rowid INTEGER NOT NULL UNIQUE);

            CREATE TABLE IF NOT EXISTS runtime_checkpoints (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runtime_projects (id TEXT PRIMARY KEY, record TEXT NOT NULL);

            CREATE TABLE IF NOT EXISTS runtime_workspace_operations (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runtime_workspace_operation_phase ON runtime_workspace_operations(
                json_extract(record,'$.phase'), json_extract(record,'$.agent'));
            CREATE TABLE IF NOT EXISTS runtime_rules (id TEXT PRIMARY KEY, record TEXT NOT NULL);
            -- What the last publication of each agent's views saw: the publisher compares
            -- it with the new row to find team, room and liveness changes, and keeps a
            -- lead's team counts so a member change marks the lead only when they
            -- change. The views themselves are built at read time.
            -- Owner, created time and running state of each task in the client task
            -- window, so a task write can place it without its document.

            -- sync_policy::TASK_WINDOW_EVICT_SQL scans live members, excluding retained tombstones.

            CREATE INDEX IF NOT EXISTS runtime_rule_ready_tick
                ON runtime_rules(json_extract(record,'$.status'))
                WHERE NOT COALESCE(json_extract(record,'$.inFlight'),0);
        CREATE TABLE IF NOT EXISTS runtime_federation_inbox (
          peer_id TEXT NOT NULL, message_id TEXT NOT NULL, record TEXT NOT NULL,
          PRIMARY KEY(peer_id,message_id));
            CREATE INDEX IF NOT EXISTS runtime_workspace_operation_agent_phase ON runtime_workspace_operations(json_extract(record,'$.agent'),json_extract(record,'$.phase'));
            CREATE INDEX IF NOT EXISTS runtime_request_agent_status ON runtime_requests(json_extract(record,'$.agent'),json_extract(record,'$.status'));
            CREATE INDEX IF NOT EXISTS runtime_rule_agent_inflight ON runtime_rules(json_extract(record,'$.agent'),json_extract(record,'$.inFlight'));
            CREATE INDEX IF NOT EXISTS runtime_conversion_complaints_leadId ON runtime_complaints(json_extract(record,'$.leadId')) WHERE json_extract(record,'$.leadId') IS NOT NULL;
            CREATE INDEX IF NOT EXISTS runtime_room_root ON runtime_rooms(json_extract(record,'$.rootId'));
            CREATE INDEX IF NOT EXISTS runtime_room_member_first ON runtime_rooms(json_extract(record,'$.members[0]'));
            CREATE INDEX IF NOT EXISTS runtime_room_member_second ON runtime_rooms(json_extract(record,'$.members[1]'));
            CREATE INDEX IF NOT EXISTS runtime_room_many_members ON runtime_rooms(json_array_length(record,'$.members')) WHERE json_array_length(record,'$.members')>2;
"#;
const FEATURE_SCHEMA_AFTER_AGENT_INDEXES: &str = r#"
            CREATE TABLE IF NOT EXISTS runtime_server_inbox (
                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, state TEXT NOT NULL,
                    result TEXT, created_at REAL NOT NULL DEFAULT 0, completed_at REAL NOT NULL DEFAULT 0, principal TEXT, actor TEXT);
"#;

/// The FTS owner shared by feature installation and catalog export.
pub(crate) const SEARCH_SCHEMA: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS runtime_search_next USING fts5(body,content='',contentless_delete=1,tokenize='unicode61');";

/// Progress records of resumable passes, in Canvas and, for compacted
/// analytics columns, in analytics.
pub(crate) const PAYLOAD_MIGRATIONS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS runtime_payload_migrations (name TEXT PRIMARY KEY,cursor INTEGER NOT NULL DEFAULT 0,complete INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',updated REAL,error TEXT);
";

/// Every statement that [`prepare`] runs, in order.
pub(crate) fn feature_schema_sql() -> String {
    let mut sql = FEATURE_SCHEMA_BEFORE_AGENT_INDEXES.to_owned();
    // Preserve the original feature-installation point and descriptor index order.
    for index in crate::agent_tables::AgentsStorageRow::INDEXES
        .iter()
        .filter(|index| {
            matches!(
                index.name,
                "runtime_agent_sweep_busy_v2"
                    | "runtime_agent_sweep_cleanup_v2"
                    | "runtime_agent_unsettled_legacy"
                    | "runtime_idle_compaction_deadline"
            )
        })
    {
        sql.push_str(
            &index
                .sql
                .replacen("CREATE INDEX ", "CREATE INDEX IF NOT EXISTS ", 1),
        );
        sql.push(';');
    }
    sql.push_str(FEATURE_SCHEMA_AFTER_AGENT_INDEXES);
    sql
}

fn feature_statements() -> [String; 3] {
    [
        feature_schema_sql(),
        SEARCH_SCHEMA.to_owned(),
        PAYLOAD_MIGRATIONS_SCHEMA.to_owned(),
    ]
}

pub(crate) fn prepare(transaction: &Transaction<'_>) -> StoreResult<()> {
    crate::receipt_tables::prepare_features(transaction)?;
    crate::clock_schema::prepare_features(transaction)?;
    crate::settings_schema::prepare_features(transaction)?;
    for descriptor in [
        crate::agent_tables::LeadRequestsStorageRow::descriptor(),
        crate::agent_tables::AgentMovesStorageRow::descriptor(),
        GroupsRow::descriptor(),
        MessagesRow::descriptor(),
        GraphEdgesRow::descriptor(),
    ] {
        descriptor.install_if_absent(transaction)?;
    }
    crate::item_monitor_terminal::prepare(transaction)?;
    crate::voice_storage::prepare(transaction)?;
    crate::queue_schema::prepare_features(transaction)?;
    crate::access_tables::prepare_features(transaction)?;
    for descriptor in [
        crate::context_settings::AssetsStorageRow::descriptor(),
        crate::context_settings::ProfilesStorageRow::descriptor(),
        crate::context_settings::ModelStatusStorageRow::descriptor(),
    ] {
        descriptor.install_if_absent(transaction)?;
    }
    for schema in feature_statements() {
        transaction.execute_batch(&schema)?;
    }
    crate::peer_update_settings::canvas_tx(transaction).insert_new(
        &crate::settings_schema::AccessSettings {
            id: 1,
            auto_pair: 1,
        },
    )?;
    crate::stored_text_compaction::prepare_batches(transaction)?;
    install_summary_delete_triggers(transaction)?;
    // Removing a transcript or its metadata can change materialization even
    // though the event itself is unchanged. Journal that event through its
    // source row; semantic event triggers ignore this value-preserving update.
    transaction.execute_batch(RECEIPT_TRANSCRIPT_TRIGGERS)?;
    Ok(())
}

/// Shared owner of feature summary tombstones and their catalog definitions.
pub(crate) fn install_summary_delete_triggers(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<()> {
    // A deleted row is no longer readable by the writer journal. Its source
    // trigger owns the corresponding summary tombstone in the same transaction.
    for (table, collection) in [
        ("runtime_events", "messageReceipt"),
        ("runtime_tasks", "task"),
    ] {
        let function = crate::entity_scopes::SCOPE_TOMBSTONE_FUNCTION;
        let tombstone = format!(
            "{function}('{collection}',OLD.id,(SELECT payload FROM sync_entities WHERE collection='{collection}' AND id=OLD.id),(SELECT deleted FROM sync_entities WHERE collection='{collection}' AND id=OLD.id))"
        );
        connection.execute_batch(&format!(
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
    Ok(())
}

pub(crate) const RECEIPT_TRANSCRIPT_TRIGGERS: &str = "CREATE TRIGGER IF NOT EXISTS summary_receipt_meta_delete AFTER DELETE ON runtime_event_meta BEGIN
         UPDATE runtime_events SET id=id WHERE id=OLD.id AND kind='user'; END;
         CREATE TRIGGER IF NOT EXISTS summary_receipt_item_delete AFTER DELETE ON runtime_items BEGIN
         UPDATE runtime_events SET id=id WHERE kind='user' AND agent=OLD.agent AND (
            OLD.id=agent || ':' || id OR id IN (SELECT id FROM runtime_event_meta
                WHERE json_extract(record,'$.transcriptItemId')=OLD.id)); END;";

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
        crate::clock_schema::prepare_features(&connection).expect("clock feature owners");
        crate::settings_schema::prepare_features(&connection).expect("settings feature owners");
        crate::stored_text_compaction::prepare_batches(&connection).expect("batch owner");
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
