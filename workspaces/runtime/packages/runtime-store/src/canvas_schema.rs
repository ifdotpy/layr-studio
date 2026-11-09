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

pub(crate) fn registry() -> [studio_db_api::Registration; 69] {
    db_schema![Canvas;
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
        WorkDecisionsStorageRow,
        WorkReleasesStorageRow,
        WorkResultsStorageRow,
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use studio_db_api::Table;

    #[derive(serde::Deserialize)]
    pub(crate) struct Catalog {
        #[serde(rename = "canvas.sqlite3")]
        pub(crate) canvas: Objects,
    }
    #[derive(serde::Deserialize)]
    pub(crate) struct Objects {
        pub(crate) sqlite_master: Vec<Entry>,
    }
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    pub(crate) struct Entry {
        pub(crate) create_order: usize,
        pub(crate) name: String,
        pub(crate) sql: String,
        pub(crate) table: String,
        pub(crate) r#type: String,
    }

    #[test]
    fn clocks_settings_descriptors_preserve_catalog_and_existing_state() {
        let snapshot: Catalog =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).expect("catalog");
        let db = crate::fresh_state::test_canvas();
        crate::clock_schema::prepare_features(&db).expect("clock features");
        crate::settings_schema::prepare_features(&db).expect("settings features");
        for registration in registry() {
            let descriptor = registration.descriptor;
            if let Some(entry) = snapshot
                .canvas
                .sqlite_master
                .iter()
                .find(|entry| entry.name == descriptor.name)
            {
                assert_eq!(descriptor.ddl, entry.sql, "{} DDL", descriptor.name);
                for entry in snapshot
                    .canvas
                    .sqlite_master
                    .iter()
                    .filter(|entry| entry.table == descriptor.name && entry.r#type == "index")
                {
                    let index = descriptor
                        .indexes
                        .iter()
                        .find(|index| index.name == entry.name)
                        .expect("owned index");
                    assert_eq!(index.sql, entry.sql, "{} index", entry.name);
                }
            }
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
            crate::SyncDocumentsRepository::put_entity_hash(&tx, "fixture", "row", "one", false)
                .expect("first publication")
        );
        let first = crate::durable_seq::high(&tx).expect("first counter");
        assert!(first > before);
        assert!(
            !crate::SyncDocumentsRepository::put_entity_hash(&tx, "fixture", "row", "one", false)
                .expect("no-op")
        );
        assert_eq!(crate::durable_seq::high(&tx).expect("no-op counter"), first);
        crate::peer_update_settings::canvas_tx(&tx)
            .upsert(&SyncEntities {
                collection: "fixture".into(),
                id: "row".into(),
                seq: first,
                hash: "one".into(),
                payload: Some(raw.into()),
                deleted: 0,
            })
            .expect("exact raw evidence");
        let stored = crate::peer_update_settings::canvas_tx(&tx)
            .get::<SyncEntities>(&("fixture".into(), "row".into()))
            .expect("read")
            .expect("row");
        assert_eq!(stored.payload.as_deref(), Some(raw));
        assert!(
            crate::SyncDocumentsRepository::put_entity_hash(&tx, "fixture", "row", "two", true)
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
                "SELECT count(*) FROM sync_entities WHERE collection='fixture'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("rollback"),
            0
        );
    }
}
