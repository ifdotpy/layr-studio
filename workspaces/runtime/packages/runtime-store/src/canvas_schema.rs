//! Canvas table registry; each descriptor has one family owner.
use crate::clock_schema::*;
use crate::peer_update_settings::{Canvas, PeerUpdateSettings};
use crate::settings_schema::*;
use crate::work_history::{WorkDecisionsStorageRow, WorkReleasesStorageRow, WorkResultsStorageRow};
use studio_db_api::db_schema;

/// Install only the explicitly requested legacy owners, preserving existing rows.
pub(crate) fn install(
    connection: &rusqlite::Connection,
    descriptors: &[studio_db_api::Descriptor],
) -> rusqlite::Result<()> {
    for descriptor in descriptors {
        descriptor.install_if_absent(connection)?;
    }
    Ok(())
}

pub(crate) fn registry() -> [studio_db_api::Registration; 116] {
    db_schema![Canvas;
        crate::queue_schema::ToolRequestAliasIndex,
        crate::agent_tables::AgentsStorageRow,
        crate::agent_tables::AgentVolatileStorageRow,
        crate::agent_tables::AgentMovesStorageRow,
        crate::agent_tables::SessionsStorageRow,
        crate::agent_tables::TurnsStorageRow,
        crate::agent_tables::ChatsStorageRow,
        crate::agent_tables::LeadRequestsStorageRow,
        crate::agent_tables::SendAdmissionsStorageRow,
        crate::access_tables::AccessAliasRequestsRow,
        crate::access_tables::AccessAuditRow,
        crate::access_tables::AccessAutoAttemptsRow,
        crate::access_tables::AccessClientsRow,
        crate::access_tables::AccessDiscoveredRow,
        crate::access_tables::AccessInvitesRow,
        AccessLocalAlias,
        crate::access_tables::AccessNameRequestsRow,
        crate::access_tables::AccessNameTargetsRow,
        crate::access_tables::AccessNoncesRow,
        AccessSettings,
        crate::access_tables::AccessUnrevokesRow,
        crate::item_monitor_terminal::ItemsRow,
        crate::item_monitor_terminal::ItemBlobsRow,
        crate::item_monitor_terminal::ItemFulltextRow,
        crate::item_monitor_terminal::MonitorsRow,
        crate::item_monitor_terminal::TerminalsRow,
        crate::item_monitor_terminal::TerminalOutputRow,
        crate::item_monitor_terminal::TerminalDecoderRow,
        crate::item_monitor_terminal::TerminalCursorRow,
        AgentPublicationState,
        AgentRecordGeneration,
        crate::interaction_tables::ApprovalsRow,
        crate::interaction_tables::ChatMessagesRow,
        ClientCommandWatermarks,
        crate::interaction_tables::ComplaintRow,
        DurableFloor,
        DurableRows,
        crate::access_tables::FederationIdentityRow,
        crate::access_tables::FederationInboxRow,
        crate::access_tables::FederationInvitesRow,
        crate::access_tables::FederationPeersRow,
        crate::access_tables::FederationRoomsRow,
        FederationSettings,
        crate::project_grants::GrantRow,
        crate::interaction_tables::GraphEdgesRow,
        crate::interaction_tables::GroupsRow,
        crate::interaction_tables::MessagesRow,
        OnlineMigrationDirty,
        OrchestrationSettings,
        PayloadMigrations,
        PeerUpdateSettings,
        crate::receipt_tables::RequestResultsRow,
        crate::receipt_tables::OutboxStorageRow,
        crate::receipt_tables::AccountStateRow,
        crate::receipt_tables::AccountTransferRow,
        crate::receipt_tables::AnalyticsProjectionRow,
        crate::receipt_tables::CapacityRetryRow,
        crate::receipt_tables::SafetyRetryRow,
        crate::receipt_tables::UsageResumeRow,
        crate::receipt_tables::LimitResetAttemptRow,
        crate::receipt_tables::LimitResetRequestRow,
        crate::receipt_tables::ErrorIncidentRow,
        crate::interaction_tables::RequestRow,
        crate::interaction_tables::RoomRow,
        SchedulerAgentChanges,
        SchedulerAgentChangeState,
        SearchRollout,
        crate::access_tables::ServerExecAuditRow,
        crate::access_tables::ServerExecInputRow,
        crate::access_tables::ServerExecRow,
        crate::access_tables::ServerInboxRow,
        crate::access_tables::ServerLinksRow,
        crate::access_tables::ServerOutputToolsRow,
        crate::access_tables::ServerRetiredRow,
        crate::context_settings::AssetsStorageRow,
        crate::context_settings::CompactionsStorageRow,
        crate::context_settings::ContextManifestsStorageRow,
        crate::context_settings::ContextRemindersStorageRow,
        crate::context_settings::ModelStatusStorageRow,
        crate::context_settings::ProfilesStorageRow,
        crate::context_settings::TranscriptRevisionsStorageRow,
        SidebarOrder,
        StoredTextBatches,
        SyncDocuments,
        SyncEntities,
        SyncEntityMeta,
        SyncGeneration,
        SyncIdentity,
        TaskWindowState,
        crate::voice_storage::VoiceApprovalRow,
        crate::voice_storage::VoiceDeliveryRow,
        crate::voice_storage::VoiceRecordRow,
        crate::voice_storage::VoiceSessionRow,
        crate::voice_storage::VoiceSpeechRequestRow,
        crate::planning_work::AnnotationRow,
        crate::planning_work::CheckpointRow,
        crate::planning_work::PlanRow,
        crate::planning_work::ProjectRow,
        crate::planning_work::RuleRow,
        crate::planning_work::TaskRow,
        crate::planning_work::WorkRow,
        crate::queue_schema::EventsRow,
        crate::queue_schema::EventMetaRow,
        crate::queue_schema::EventRevisionsRow,
        crate::queue_schema::NativeNoticesRow,
        crate::queue_schema::ExecutionRunsRow,
        crate::queue_schema::ExecutionNodesRow,
        crate::queue_schema::ExecutionEffectsRow,
        crate::queue_schema::ExecutionAttemptsRow,
        crate::queue_schema::ExecutionInputsRow,
        crate::queue_schema::ProcessActionsRow,
        crate::queue_schema::ToolRequestsRow,
        WorkDecisionsStorageRow,
        WorkReleasesStorageRow,
        WorkResultsStorageRow,
        crate::planning_work::WorkspaceOperationRow,
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use studio_db_api::Table;

    #[derive(serde::Deserialize)]
    struct GeneratedCatalog {
        #[serde(rename = "canvas.sqlite3")]
        canvas: GeneratedDatabase,
        #[serde(rename = "analytics.sqlite3")]
        analytics: GeneratedDatabase,
        pragmas: Option<std::collections::BTreeMap<String, GeneratedPragmas>>,
    }
    #[derive(serde::Deserialize)]
    struct GeneratedDatabase {
        sqlite_master: Vec<GeneratedObject>,
    }
    #[derive(serde::Deserialize)]
    struct GeneratedObject {
        name: String,
        sql: String,
    }
    #[derive(serde::Deserialize)]
    struct GeneratedPragmas {
        journal_mode: String,
        synchronous: i64,
    }

    #[test]
    fn generated_analytics_migration_table_preserves_previous_ddl_and_statuses() {
        let catalog: GeneratedCatalog =
            studio_json::from_str(&crate::storage_catalog::generate().unwrap()).unwrap();
        let table = catalog
            .analytics
            .sqlite_master
            .iter()
            .find(|entry| entry.name == "runtime_payload_migrations")
            .unwrap();
        // Independent pre-generator Analytics layout; Canvas has a different contract.
        assert_eq!(
            table.sql,
            "CREATE TABLE runtime_payload_migrations (name TEXT PRIMARY KEY,cursor INTEGER NOT NULL DEFAULT 0,complete INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',updated REAL,error TEXT)"
        );
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&table.sql).unwrap();
        db.execute(
            "INSERT INTO runtime_payload_migrations(name,status) VALUES('legacy','paused')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn generated_catalog_preserves_previous_durability_pragmas() {
        let catalog: GeneratedCatalog =
            studio_json::from_str(&crate::storage_catalog::generate().unwrap()).unwrap();
        let pragmas = catalog.pragmas.expect("retained pragma metadata");
        assert_eq!(pragmas.len(), 2);
        for name in ["analytics", "main"] {
            assert_eq!(pragmas[name].journal_mode, "wal");
            assert_eq!(pragmas[name].synchronous, 1);
        }
    }

    #[test]
    fn generated_summary_triggers_match_feature_preparation() {
        let catalog: GeneratedCatalog =
            studio_json::from_str(&crate::storage_catalog::generate().unwrap()).unwrap();
        let mut db = crate::fresh_state::test_canvas();
        db.execute_batch(
            "DROP TRIGGER summary_messageReceipt_delete; DROP TRIGGER summary_task_delete;",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        crate::feature_schema::prepare(&tx).unwrap();
        for name in ["summary_messageReceipt_delete", "summary_task_delete"] {
            let actual: String = tx
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name=?1",
                    [name],
                    |row| row.get(0),
                )
                .unwrap();
            let expected = catalog
                .canvas
                .sqlite_master
                .iter()
                .find(|entry| entry.name == name)
                .unwrap();
            assert_eq!(actual, expected.sql, "{name}");
        }
    }

    #[test]
    fn generated_storage_catalog_matches_committed_json() {
        let generated = crate::storage_catalog::generate().expect("registry export");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime-state-schema.json");
        if std::env::var_os("STUDIO_WRITE_STORAGE_CATALOG").is_some() {
            std::fs::write(&path, &generated).expect("write generated catalog");
        }
        assert!(
            std::fs::read_to_string(path).unwrap() == generated,
            "run this test with STUDIO_WRITE_STORAGE_CATALOG=1 to regenerate"
        );
    }

    #[test]
    fn clocks_settings_descriptors_preserve_existing_state() {
        let db = crate::fresh_state::test_canvas();
        crate::clock_schema::prepare_features(&db).expect("clock features");
        crate::settings_schema::prepare_features(&db).expect("settings features");
        for registration in registry() {
            let descriptor = registration.descriptor;
            let (rows, rejected) = (registration.check)(&db).expect(descriptor.name).counts();
            assert_eq!(
                rejected, 0,
                "{} rejects {rejected}/{rows} existing rows",
                descriptor.name
            );
        }
        // Lazy table owners are installed by the real feature preparation path.
        for name in [TaskWindowState::NAME, AgentPublicationState::NAME] {
            let wanted = registry()
                .into_iter()
                .find(|entry| entry.descriptor.name == name)
                .expect("registered")
                .descriptor;
            let sql: String = db
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name=?1",
                    [name],
                    |row| row.get(0),
                )
                .expect("installed feature");
            assert_eq!(sql, wanted.ddl, "{name} lazy DDL");
        }
        let scratch = rusqlite::Connection::open_in_memory().expect("scratch");
        install(&scratch, &[OnlineMigrationDirty::descriptor()]).expect("dirty owner");
        let sql: String = scratch
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name=?1",
                [OnlineMigrationDirty::NAME],
                |row| row.get(0),
            )
            .expect("installed dirty");
        assert_eq!(sql, OnlineMigrationDirty::DDL);
    }

    #[test]
    fn federation_settings_preserves_unknown_extension_evidence() {
        let raw = r#" { "enabled": true, "updated": 1.5, "future": { "nested": [1,2] } } "#;
        let record = crate::json_record_codec::ExactJson::<FederationSettingsPayload>::from_stored(
            raw.to_owned(),
        )
        .expect("known settings and raw extensions");
        assert!(record.value().fields.enabled);
        assert_eq!(record.raw(), raw);
        assert_eq!(
            record.value().extensions["future"].as_str(),
            r#"{ "nested": [1,2] }"#
        );
    }

    #[test]
    fn federation_settings_writer_keeps_historical_field_order() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("settings transaction");
        crate::FederationRepository::set_enabled(&tx, true).expect("write settings");
        let row = crate::peer_update_settings::canvas_tx(&tx)
            .get::<FederationSettings>(&("global".to_owned(),))
            .expect("read settings")
            .expect("row");
        let updated = row.record.value().extensions["updated"].as_str();
        assert_eq!(
            row.record.raw(),
            format!(r#"{{"enabled":true,"updated":{updated}}}"#)
        );
        tx.rollback().expect("caller owns rollback");
    }

    #[test]
    fn metadata_marker_query_keeps_sql_comparison_semantics() {
        let db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch(SyncEntityMeta::DDL).expect("legacy table");
        db.execute_batch("INSERT INTO sync_entity_meta VALUES('text','1'),('blob',x'31'),('other','0'),(NULL,'1')").expect("history");
        for key in ["text", "blob", "other", "missing"] {
            let previous: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key=?1 AND value='1')",
                    [key],
                    |row| row.get(0),
                )
                .expect("legacy predicate");
            assert_eq!(
                SyncEntityMeta::matches(&db, key, "1").expect("typed predicate"),
                previous,
                "{key}"
            );
        }
    }

    #[test]
    fn generated_publication_crud_keeps_sequences_noops_and_rollback() {
        let mut db = crate::fresh_state::test_canvas();
        let before = crate::durable_seq::high(&db).expect("counter");
        let tx = db.transaction().expect("publication transaction");
        let raw = r#"{ "unknown": [1,2], "id": "row" }"#;
        assert!(
            crate::SyncDocumentsRepository::put_entity_hash(
                &tx,
                "transcript:fixture",
                "row",
                "one",
                false
            )
            .expect("first publication")
        );
        let first = crate::durable_seq::high(&tx).expect("first counter");
        assert!(first > before);
        assert!(
            !crate::SyncDocumentsRepository::put_entity_hash(
                &tx,
                "transcript:fixture",
                "row",
                "one",
                false
            )
            .expect("no-op")
        );
        assert_eq!(crate::durable_seq::high(&tx).expect("no-op counter"), first);
        crate::peer_update_settings::canvas_tx(&tx)
            .upsert(&SyncEntities {
                collection: "transcript:fixture".into(),
                id: "row".into(),
                seq: first,
                hash: "one".into(),
                payload: Some(raw.into()),
                deleted: 0,
            })
            .expect("exact raw evidence");
        let stored = crate::peer_update_settings::canvas_tx(&tx)
            .get::<SyncEntities>(&("transcript:fixture".into(), "row".into()))
            .expect("read")
            .expect("row");
        assert_eq!(stored.payload.as_deref(), Some(raw));
        assert!(
            crate::SyncDocumentsRepository::put_entity_hash(
                &tx,
                "transcript:fixture",
                "row",
                "two",
                true
            )
            .expect("tombstone")
        );
        assert!(crate::durable_seq::high(&tx).expect("tombstone counter") > first);
        tx.rollback().expect("outer rollback");
        assert_eq!(
            crate::durable_seq::high(&db).expect("rolled back counter"),
            before
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM sync_entities WHERE collection='transcript:fixture'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("rollback"),
            0
        );
    }
}
