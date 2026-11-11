pub mod account_state;
pub(crate) mod account_transfer;
pub(crate) mod agent_indexed_reads;
pub mod agent_management;
pub(crate) mod agent_move;
pub(crate) mod agents;
pub mod analytics;
pub mod analytics_capture;
pub mod capability_reads;
pub(crate) mod checkpoints;
pub(crate) mod claude_controls;
pub(crate) mod claude_records;
pub(crate) mod claude_retry;
pub mod command_watermarks;
pub(crate) mod context_repair;
pub(crate) mod continuation;
pub(crate) mod coordination;
pub mod cost_aggregate;
pub(crate) mod event_write;
pub(crate) mod events;
pub(crate) mod execution_history;
pub(crate) mod federation;
pub(crate) mod federation_credentials;
pub mod federation_messaging;
pub(crate) mod messages;
pub mod monitor_runtime;
pub(crate) mod monitors;
pub(crate) mod multi_server;
pub mod multi_server_delivery;
pub(crate) mod notification_dispatch;
pub(crate) mod orchestration_settings;
pub mod outbox;
pub mod process_actions;
pub(crate) mod profiles;
pub(crate) mod project_locations;
pub(crate) mod projects;
pub(crate) mod questions_reads;
pub(crate) mod questions_write;
pub(crate) mod queue_write;
pub mod remote_outbox;
pub(crate) mod remote_transport;
pub mod request_results;
pub(crate) mod reviewer_archive;
pub(crate) mod rules;
pub mod rules_tick;
pub(crate) mod scheduler;
pub mod startup_recovery;
pub(crate) mod sync_documents;
pub mod system_reads;
pub(crate) mod tasks;
pub mod terminals;
pub(crate) mod transcript;
pub(crate) mod usage;
pub(crate) mod voice;
pub(crate) mod work_items;
pub(crate) mod workspace_files;
pub mod workspace_operations;
pub mod workspace_ready;
pub(crate) mod workspace_views;

use crate::schema::SchemaRequirements;

pub(crate) struct SchemaRegistration {
    pub(crate) repository: &'static str,
    pub(crate) requirements: &'static SchemaRequirements,
}

inventory::collect!(SchemaRegistration);

pub(crate) fn schema_registrations() -> Vec<&'static SchemaRegistration> {
    let mut registrations = inventory::iter::<SchemaRegistration>
        .into_iter()
        .collect::<Vec<_>>();
    registrations.sort_unstable_by_key(|registration| registration.repository);
    registrations
}

#[cfg(test)]
mod tests {
    use super::{account_state, agents, events, messages, sync_documents, tasks};
    use crate::records::{
        AgentId, AgentRecord, AgentRepository, ChatMessage, ChatMessageId, MessageRepository,
        QueueEvent, QueueEventId, QueueEventRepository, TaskRecord, TaskRepository,
    };
    use rusqlite::Connection;
    use std::fs;
    use std::path::Path;
    use studio_db_api::Table;
    use studio_json::json;
    use sync_documents::SyncDocumentsRepository;

    fn database_with(requirements: &crate::schema::SchemaRequirements) -> Connection {
        let connection = Connection::open_in_memory().expect("memory SQLite opens");
        let mut tables = std::collections::BTreeMap::<&str, Vec<&str>>::new();
        for table in requirements
            .tables
            .iter()
            .chain(requirements.bootstrap_tables)
        {
            let columns = tables.entry(table.name).or_default();
            for column in table.columns {
                if !columns.contains(column) {
                    columns.push(column);
                }
            }
        }
        for (name, columns) in tables {
            if name == crate::publication_schema::Publications::NAME {
                crate::publication_schema::Publications::descriptor()
                    .install_if_absent(&connection)
                    .expect("declared publication owner");
                continue;
            }
            let columns = columns
                .iter()
                .map(|column| {
                    if *column == "id" {
                        format!("\"{column}\" TEXT PRIMARY KEY")
                    } else if matches!(*column, "seq" | "rev") {
                        format!("\"{column}\" INTEGER NOT NULL DEFAULT 1")
                    } else {
                        format!("\"{column}\"")
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            connection
                .execute_batch(&format!("CREATE TABLE \"{name}\" ({columns})"))
                .expect("declared repository table creates");
        }
        connection
    }

    fn declared_modules(source: &str) -> Vec<String> {
        source
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                ["pub(crate) mod ", "pub mod ", "mod "]
                    .into_iter()
                    .find_map(|prefix| line.strip_prefix(prefix))
                    .and_then(|declaration| declaration.strip_suffix(';'))
                    .map(str::to_owned)
            })
            .collect()
    }

    #[test]
    fn repository_modules_match_the_source_directory_in_alphabetical_order() {
        let directory = Path::new(
            &std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
        )
        .join("src/repositories");
        let mut source_modules = fs::read_dir(&directory)
            .expect("repositories directory is readable")
            .map(|entry| {
                entry
                    .expect("repositories directory entry is readable")
                    .path()
            })
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?;
                (stem != "mod").then(|| stem.to_owned())
            })
            .collect::<Vec<_>>();
        source_modules.sort_unstable();

        assert_eq!(declared_modules(include_str!("mod.rs")), source_modules);

        let mut requirement_modules = fs::read_dir(&directory)
            .expect("repositories directory is readable")
            .map(|entry| {
                entry
                    .expect("repositories directory entry is readable")
                    .path()
            })
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .filter_map(|path| {
                if path.file_stem().is_some_and(|stem| stem == "mod") {
                    return None;
                }
                let source = fs::read_to_string(&path).expect("repository source is readable");
                source
                    .contains(
                        "pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements",
                    )
                    .then(|| path.file_stem()?.to_str().map(str::to_owned))
                    .flatten()
            })
            .collect::<Vec<_>>();
        requirement_modules.sort_unstable();

        let registrations = super::schema_registrations();
        let registered_modules = registrations
            .iter()
            .map(|registration| registration.repository.to_owned())
            .collect::<Vec<_>>();
        assert_eq!(registered_modules, requirement_modules);
        assert_eq!(registrations.len(), requirement_modules.len());
        assert!(registered_modules.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn repository_sql_uses_only_declared_tables() {
        let db = database_with(&agents::REQUIREMENTS);
        let agent = AgentRecord::from_value(json!({"compactions":0,"accountKey":"default",
            "id":"a", "provider":"codex", "rootId":"a", "epoch":0, "status":"idle"
        }))
        .expect("agent JSON has a string id");
        AgentRepository::put(&db, &agent).expect("agent write uses declared objects");
        AgentRepository::get(&db, agent.id()).expect("agent read uses declared objects");

        let db = database_with(&tasks::REQUIREMENTS);
        let task: TaskRecord = studio_json::from_str(&json!({
            "id":"t", "agent":"a", "kind":"command", "status":"completed",
            "created":0, "itemId":"i", "turnId":"turn", "type":"commandExecution", "name":"commandExecution", "interactive":false, "exitCode":0
        }).to_string())
        .expect("task JSON has a string id");
        {
            let tx = db.unchecked_transaction().expect("fixture transaction");
            TaskRepository::put(&tx, &task).expect("task write uses declared objects");
            tx.commit().expect("fixture commit");
        }
        TaskRepository::get(&db, task.id()).expect("task read uses declared objects");

        let db = database_with(&events::REQUIREMENTS);
        let event = QueueEvent {
            id: QueueEventId::from("e"),
            agent: AgentId::from("a"),
            kind: String::new(),
            text: String::new(),
            status: String::new(),
            created: 0.0,
            epoch: 0,
            turn_id: None,
            error: None,
        };
        QueueEventRepository::put(&db, &event).expect("event write uses declared objects");
        QueueEventRepository::get(&db, &event.id).expect("event read uses declared objects");

        let db = database_with(&messages::REQUIREMENTS);
        let message = ChatMessage {
            sequence: 0,
            id: ChatMessageId::from("m"),
            room: String::new(),
            sender: String::new(),
            text: String::new(),
            created: 0.0,
            deliveries: crate::json_record_codec::ExactJson::from_stored("null".into()).unwrap(),
        };
        let message_tx = db.unchecked_transaction().unwrap();
        MessageRepository::put(&message_tx, &message).expect("message write uses declared objects");
        message_tx.commit().unwrap();
        MessageRepository::get(&db, &message.id).expect("message read uses declared objects");

        let db = database_with(&account_state::REQUIREMENTS);
        let tx = db.unchecked_transaction().expect("publication fixture");
        crate::HttpPublicationQueries::test_seed_entity(
            &tx,
            "workspace",
            "current",
            1,
            0,
            Some(
                &json!({"collection":"workspace","id":"current","value": {"rateLimitsByAccount": {
                    "alpha": {"accountKey": "alpha", "data": {"status": "ready"}}
                }}})
                .to_string(),
            ),
        )
        .expect("fixture row uses declared columns");
        tx.commit().expect("publication fixture commits");
        db.execute_batch(
            "CREATE TABLE runtime_usage_resumes(id TEXT PRIMARY KEY,agent TEXT,record TEXT)",
        )
        .expect("observation receipts fixture table");
        account_state::limits_snapshots(&db).expect("account limits read uses only declared table");
        assert!(
            account_state::authentication_recovery_account_keys(&db, 2_000.0)
                .expect("declared resume table query succeeds")
                .is_empty()
        );
    }

    #[test]
    fn sync_repository_sql_uses_only_declared_tables() {
        use studio_db_api::Table;
        let mut db = Connection::open_in_memory().expect("memory SQLite opens");
        for descriptor in [
            crate::metadata_schema::Metadata::descriptor(),
            crate::change_schema::Changes::descriptor(),
            crate::publication_schema::Publications::descriptor(),
        ] {
            descriptor
                .install_if_absent(&db)
                .expect("declared canonical owner");
        }
        let tx = db.transaction().expect("identity fixture");
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<crate::metadata_schema::SyncIdentity>()
            .put(&crate::metadata_schema::SyncIdentity {
                identity: crate::consolidation_codec::nullable_text_identity(
                    &Some("workspace-a".into()),
                    1,
                ),
                source_rowid: 1,
                text_key: Some("workspace-a".into()),
            })
            .expect("workspace identity");
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<crate::metadata_schema::SyncGeneration>()
            .put(&crate::metadata_schema::SyncGeneration {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                sync_generation: 0,
            })
            .expect("publication clock");
        tx.commit().expect("identity fixture commits");
        assert_eq!(
            SyncDocumentsRepository::workspace_id(&db).expect("identity reads"),
            "workspace-a"
        );

        let tx = db.transaction().expect("draft fixture");
        SyncDocumentsRepository::put_draft(
            &tx,
            "phone:chat",
            "{\"device\":\"phone\",\"session\":\"chat\"}",
            false,
        )
        .expect("draft fixture inserts");
        tx.commit().expect("draft fixture commits");
        let drafts = SyncDocumentsRepository::pull_drafts(&db, 0, 100).expect("drafts read");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, "phone:chat");
        let transaction = db.transaction().expect("preference transaction begins");
        assert!(
            SyncDocumentsRepository::put_preferences(
                &transaction,
                "user",
                "{\"collection\":\"uiPreferences\",\"id\":\"user\",\"value\":{\"fields\":{}}}",
            )
            .expect("preferences write")
        );
        transaction.commit().expect("preference commits");
        assert!(
            SyncDocumentsRepository::preference_payload(&db, "user")
                .expect("preferences read")
                .is_some()
        );
    }
}
