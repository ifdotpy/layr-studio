//! Shared canonical fixture operations. Never used by production startup.
#[cfg(test)]
mod tests {
    #[test]
    fn canonical_install_and_publication_snapshots() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        super::install_canonical(&db).unwrap();
        assert!(super::entities(&db).unwrap().is_empty());
        assert!(super::documents(&db).unwrap().is_empty());
        assert!(super::publications(&db).unwrap().is_empty());
        assert!(!crate::schema::table_exists(&db, "sync_entities").unwrap());
        assert!(!crate::schema::table_exists(&db, "runtime_agents").unwrap());
    }
}
use crate::StoreResult;
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::publication_schema::Publications;
use studio_db_api::CodecContext as Connection;
use studio_db_api::DbQuery;

/// Install the three canonical F1 tables and singleton clocks in a minimal fixture.
/// Call once, outside a transaction. Unrelated domain tables remain caller-owned.
/// Use `crate::test_canvas()` when a full current database and producers are required.
pub fn install_canonical(connection: &Connection) -> StoreResult<()> {
    use studio_db_api::Table;
    crate::schema::register_python_trigger_functions(connection)?;
    let transaction = connection.unchecked_transaction()?;
    crate::canvas_schema::install(
        &transaction,
        &[
            crate::metadata_schema::Metadata::descriptor(),
            crate::change_schema::Changes::descriptor(),
            Publications::descriptor(),
        ],
    )?;
    crate::clock_schema::seed_fresh_clock_rows(&transaction)?;
    transaction.commit()?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct EntitySnapshot {
    pub collection: String,
    pub id: String,
    pub seq: i64,
    pub hash: String,
    pub payload: Option<String>,
    pub deleted: i64,
    pub source_rowid: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub seq: i64,
    pub scope: String,
    pub id: String,
    pub payload: String,
    pub deleted: i64,
    pub source_rowid: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct PublicationSnapshot {
    pub kind: i64,
    /// Entity collection or document scope.
    pub collection: Option<String>,
    pub id: String,
    pub hash: Option<String>,
    pub payload: Option<Vec<u8>>,
    pub deleted: i64,
    pub source_rowid: i64,
    pub seq: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,
    result=(String,String,i64,String,Option<String>,i64,i64),
    tables(rows=Publications),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,hash=Publications::ENTITY_HASH,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY),predicates(entities_only=rows),
    sql="SELECT {collection},{id},{seq},{hash},{payload},{deleted},{rowid} FROM {rows} WHERE {entities_only} ORDER BY {collection},{id}")]
struct Entities {}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,
    result=(i64,String,String,String,i64,i64),
    tables(rows=Publications),
    columns(seq=Publications::PUBLICATION_SEQ,scope=Publications::DOCUMENT_SCOPE,id=Publications::DOCUMENT_ID,payload=Publications::DOCUMENT_PAYLOAD,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
    indexes(rows=Publications::INDEX_PUBLICATIONS_DOCUMENT_IDENTITY),predicates(documents_only=rows),
    sql="SELECT {seq},{scope},{id},{payload},{deleted},{rowid} FROM {rows} WHERE {documents_only} ORDER BY {seq},{rowid}")]
struct Documents {}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,
    result=(i64,Option<String>,String,Option<String>,Option<Vec<u8>>,i64,i64,i64),
    tables(rows=Publications),
    columns(kind=Publications::KIND,identity=Publications::IDENTITY,collection=Publications::COLLECTION_ORDER,scope=Publications::DOCUMENT_SCOPE,entity=Publications::ENTITY_ID,document=Publications::DOCUMENT_ID,hash=Publications::ENTITY_HASH,entity_payload=Publications::ENTITY_PAYLOAD,document_payload=Publications::DOCUMENT_PAYLOAD,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID,seq=Publications::PUBLICATION_SEQ),
    sql="SELECT {kind},coalesce({collection},{scope}),coalesce({entity},{document}),{hash},CAST(coalesce({entity_payload},{document_payload}) AS BLOB),{deleted},{rowid},{seq} FROM {rows} ORDER BY {kind},{identity}")]
struct AllPublications {}

pub fn entities(connection: &Connection) -> rusqlite::Result<Vec<EntitySnapshot>> {
    Ok(canvas_read(connection)
        .collect(&Entities::QUERY, &Entities {})?
        .into_iter()
        .map(
            |(collection, id, seq, hash, payload, deleted, source_rowid)| EntitySnapshot {
                collection,
                id,
                seq,
                hash,
                payload,
                deleted,
                source_rowid,
            },
        )
        .collect())
}
pub fn documents(connection: &Connection) -> rusqlite::Result<Vec<DocumentSnapshot>> {
    Ok(canvas_read(connection)
        .collect(&Documents::QUERY, &Documents {})?
        .into_iter()
        .map(
            |(seq, scope, id, payload, deleted, source_rowid)| DocumentSnapshot {
                seq,
                scope,
                id,
                payload,
                deleted,
                source_rowid,
            },
        )
        .collect())
}
pub fn publications(connection: &Connection) -> rusqlite::Result<Vec<PublicationSnapshot>> {
    Ok(canvas_read(connection)
        .collect(&AllPublications::QUERY, &AllPublications {})?
        .into_iter()
        .map(
            |(kind, collection, id, hash, payload, deleted, source_rowid, seq)| {
                PublicationSnapshot {
                    kind,
                    collection,
                    id,
                    hash,
                    payload,
                    deleted,
                    source_rowid,
                    seq,
                }
            },
        )
        .collect())
}

/// A frozen corpus caller supplies its original loader between this install and migration.
pub fn install_legacy_source_schema(connection: &Connection) -> StoreResult<()> {
    crate::schema::register_python_trigger_functions(connection)?;
    let transaction = connection.unchecked_transaction()?;
    crate::bootstrap::legacy_fixture::prepare_sync_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

/// Run admission checks, dated transforms, bounded consolidation and authority publication.
pub async fn migrate_legacy(root: &std::path::Path) -> StoreResult<crate::RuntimeStore> {
    crate::RuntimeStore::open(Some(root)).await
}

/// Restore the three untouched zstd database frames of a historical release.
/// The destination must be a disposable directory without database files.
pub async fn open_frozen_databases(
    root: &std::path::Path,
    frames: [&std::path::Path; 3],
) -> StoreResult<crate::RuntimeStore> {
    restore_frozen_databases(root, frames)?;
    migrate_legacy(root).await
}

/// Restore the supported release before applying a caller's untouched frozen seed.
/// Close all seed connections before calling `migrate_legacy`.
pub fn write_supported_fixture(root: &std::path::Path) -> StoreResult<()> {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/f832d09");
    let frames = [
        fixture.join("canvas.sqlite3.zst"),
        fixture.join("analytics.sqlite3.zst"),
        fixture.join("supervisor.sqlite3.zst"),
    ];
    restore_frozen_databases(root, [&frames[0], &frames[1], &frames[2]])
}

fn restore_frozen_databases(
    root: &std::path::Path,
    frames: [&std::path::Path; 3],
) -> StoreResult<()> {
    std::fs::create_dir_all(root)?;
    for (name, path) in ["canvas.sqlite3", "analytics.sqlite3", "supervisor.sqlite3"]
        .into_iter()
        .zip(frames)
    {
        let frame = std::fs::read(path)?;
        let size = zstd_safe::get_frame_content_size(&frame)
            .map_err(|_| crate::StoreError::RecordContract("invalid frozen database frame"))?
            .ok_or(crate::StoreError::RecordContract(
                "frozen database frame has no content size",
            ))?;
        let mut bytes = Vec::with_capacity(usize::try_from(size).map_err(|_| {
            crate::StoreError::RecordContract("frozen database frame is too large")
        })?);
        zstd_safe::decompress(&mut bytes, &frame)
            .map_err(|_| crate::StoreError::RecordContract("cannot decompress frozen database"))?;
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(name))?
            .write_all(&bytes)?;
    }
    Ok(())
}

/// Replace all identity evidence, optionally leaving identity absent.
pub fn reset_identity(tx: &rusqlite::Transaction<'_>, id: Option<&str>) -> StoreResult<()> {
    use crate::metadata_schema::SyncIdentity;
    let mut typed = crate::peer_update_settings::canvas_tx(tx);
    for row in studio_db_api::db_query!(typed.read(), SyncIdentity)? {
        typed.delete::<SyncIdentity>(&(row.identity,))?;
    }
    if let Some(id) = id {
        let text_key = Some(id.to_owned());
        typed.partition::<SyncIdentity>().put(&SyncIdentity {
            identity: crate::consolidation_codec::nullable_text_identity(&text_key, 1),
            source_rowid: 1,
            text_key,
        })?;
    }
    Ok(())
}

/// Seed or delete one exact business marker without changing its evidence codec.
pub fn set_meta(tx: &rusqlite::Transaction<'_>, key: &str, value: Option<&str>) -> StoreResult<()> {
    use crate::metadata_schema::SyncEntityMeta;
    let text_key = Some(key.to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&text_key, 0);
    let mut typed = crate::peer_update_settings::canvas_tx(tx);
    if let Some(value) = value {
        crate::clock_schema::SyncEntityMeta::put(tx, key, value)?;
    } else {
        typed.delete::<SyncEntityMeta>(&(identity,))?;
    }
    Ok(())
}

pub fn seed_orchestration_settings(tx: &rusqlite::Transaction<'_>, raw: &str) -> StoreResult<()> {
    use crate::metadata_schema::OrchestrationSettings;
    let text_key = Some("global".to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&text_key, 0);
    let source_rowid = canvas_read(tx)
        .get::<OrchestrationSettings>(&(identity.clone(),))?
        .map(|row| row.source_rowid)
        .map_or_else(
            || {
                crate::clock_schema::next_metadata_rowid(
                    tx,
                    crate::consolidation_kinds::CanvasMetadataKind::OrchestrationSettings,
                )
            },
            Ok,
        )?;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<OrchestrationSettings>()
        .put(&OrchestrationSettings {
            identity,
            source_rowid,
            text_key,
            orchestration_record:
                crate::metadata_values::OrchestrationSettingsEvidence::from_stored(raw.to_owned())?,
        })?;
    Ok(())
}

pub fn seed_federation_settings(tx: &rusqlite::Transaction<'_>, raw: &str) -> StoreResult<()> {
    use crate::metadata_schema::FederationSettings;
    let text_key = Some("global".to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&text_key, 0);
    let source_rowid = canvas_read(tx)
        .get::<FederationSettings>(&(identity.clone(),))?
        .map(|row| row.source_rowid)
        .map_or_else(
            || {
                crate::clock_schema::next_metadata_rowid(
                    tx,
                    crate::consolidation_kinds::CanvasMetadataKind::FederationSettings,
                )
            },
            Ok,
        )?;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<FederationSettings>()
        .put(&FederationSettings {
            identity,
            source_rowid,
            text_key,
            federation_record: crate::json_record_codec::ExactJson::from_stored(raw.to_owned())?,
        })?;
    Ok(())
}

pub fn seed_access_settings(tx: &rusqlite::Transaction<'_>, auto_pair: i64) -> StoreResult<()> {
    use crate::metadata_schema::AccessSettings;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<AccessSettings>()
        .put(&AccessSettings {
            identity: studio_db_api::migration::encode_key(&(1_i64,)),
            source_rowid: 1,
            integer_key: 1,
            auto_pair: crate::metadata_values::StoredBoolean::from_stored(auto_pair),
        })?;
    Ok(())
}

pub fn seed_migration_progress(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::settings_schema::PayloadMigrations,
) -> StoreResult<()> {
    use crate::metadata_schema::PayloadMigrations;
    let text_key = Some(row.name.clone());
    let identity = crate::consolidation_codec::nullable_text_identity(&text_key, 0);
    let mut typed = crate::peer_update_settings::canvas_tx(tx);
    let source_rowid = typed
        .get::<PayloadMigrations>(&(identity.clone(),))?
        .map(|row| row.source_rowid)
        .map_or_else(
            || {
                crate::clock_schema::next_metadata_rowid(
                    tx,
                    crate::consolidation_kinds::CanvasMetadataKind::PayloadMigrations,
                )
            },
            Ok,
        )?;
    typed
        .partition::<PayloadMigrations>()
        .put(&PayloadMigrations {
            identity,
            source_rowid,
            text_key,
            migration_cursor: row.cursor,
            migration_complete: crate::metadata_values::StoredBoolean::from_stored(row.complete),
            migration_status: row.status.parse().map_err(|_| {
                crate::StoreError::RecordContract("unknown fixture migration status")
            })?,
            migration_updated: row.updated,
            migration_error: row.error.clone(),
            migration_cutover_ms: row.cutover_ms,
            migration_cursor_key: row.cursor_key.clone(),
            migration_source_rows: row.copy_source_rows,
            migration_target_rows: row.copy_target_rows,
        })?;
    Ok(())
}

pub fn migration_progress(tx: &Connection, name: &str) -> StoreResult<Option<(i64, String)>> {
    crate::migration_progress::completion(&canvas_read(tx), name)
}

pub fn seed_entity(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::clock_schema::SyncEntities,
    source_rowid: i64,
) -> StoreResult<()> {
    use crate::publication_schema::{EntityValue, SyncEntities};
    let collection_order =
        crate::publication_collection::CollectionName::from_stored(row.collection.clone());
    let deleted = crate::metadata_values::StoredBoolean::from_stored(row.deleted);
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<SyncEntities>()
        .put(&SyncEntities {
            identity: studio_db_api::migration::encode_key(&(
                row.collection.clone(),
                row.id.clone(),
            )),
            source_rowid,
            publication_seq: row.seq,
            deleted,
            collection_kind: collection_order.kind(),
            collection_agent: collection_order.agent().cloned(),
            non_transcript: collection_order.non_transcript(),
            collection_order,
            entity_id: row.id.clone(),
            entity_hash: row.hash.clone(),
            entity_payload: EntityValue::from_stored(
                &row.collection,
                &row.id,
                deleted,
                row.payload.clone(),
            )?,
        })?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct ChangeSnapshot {
    pub kind: i64,
    pub source_label: String,
    pub durable_id: String,
    pub seq: i64,
    pub deleted: i64,
    pub deleted_at: i64,
    pub scope: Option<String>,
    pub source_rowid: i64,
}

use crate::change_schema::Changes;
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,
    result=(i64,String,String,i64,i64,i64,Option<String>,i64),
    tables(rows=Changes),columns(kind=Changes::KIND,label=Changes::SOURCE_LABEL,id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,at=Changes::DELETED_AT,scope=Changes::SCOPE,rowid=Changes::SOURCE_ROWID),
    indexes(rows=Changes::INDEX_CHANGES_DURABLE_IDENTITY),predicates(durable_only=rows),
    sql="SELECT {kind},{label},{id},{seq},{deleted},{at},{scope},{rowid} FROM {rows} WHERE {durable_only} ORDER BY {label},{id},{rowid}")]
struct DurableChanges {}

/// All durable-row changes, including tombstones and unrelated source labels.
pub fn changes(connection: &Connection) -> rusqlite::Result<Vec<ChangeSnapshot>> {
    Ok(canvas_read(connection)
        .collect(&DurableChanges::QUERY, &DurableChanges {})?
        .into_iter()
        .map(
            |(kind, source_label, durable_id, seq, deleted, deleted_at, scope, source_rowid)| {
                ChangeSnapshot {
                    kind,
                    source_label,
                    durable_id,
                    seq,
                    deleted,
                    deleted_at,
                    scope,
                    source_rowid,
                }
            },
        )
        .collect())
}

/// Registers the legacy producer functions before the caller loads an untouched corpus.
pub fn open_supported_canvas(root: &std::path::Path) -> StoreResult<Connection> {
    let db = Connection::open(root.join("canvas.sqlite3"))?;
    crate::schema::register_python_trigger_functions(&db)?;
    Ok(db)
}

pub fn event_publication_trigger_sql() -> String {
    crate::event_publication_triggers::trigger_sql()
}

#[derive(Clone, Copy)]
pub enum FailpointPartition {
    Metadata(crate::consolidation_kinds::CanvasMetadataKind),
    Changes(crate::consolidation_kinds::CanvasChangesKind),
    Publications(crate::consolidation_kinds::CanvasPublicationsKind),
}
#[derive(Clone, Copy)]
pub enum FailpointOperation {
    Insert,
    Update,
    Delete,
}

/// One test-only DDL site; identifiers and codes come exclusively from descriptor owners.
/// Each selected table/kind/operation has its own trigger, so independent faults coexist.
pub fn set_failpoint(
    connection: &Connection,
    partition: FailpointPartition,
    operation: FailpointOperation,
    enabled: bool,
) -> rusqlite::Result<()> {
    use studio_db_api::{Table, migration::identifier, query_column};
    let (table, kind_column, kind) = match partition {
        FailpointPartition::Metadata(kind) => (
            crate::metadata_schema::Metadata::NAME,
            query_column::<Canvas, _>(crate::metadata_schema::Metadata::KIND),
            kind as i64,
        ),
        FailpointPartition::Changes(kind) => (
            Changes::NAME,
            query_column::<Canvas, _>(Changes::KIND),
            kind as i64,
        ),
        FailpointPartition::Publications(kind) => (
            Publications::NAME,
            query_column::<Canvas, _>(Publications::KIND),
            kind as i64,
        ),
    };
    let (event, reference) = match operation {
        FailpointOperation::Insert => ("INSERT", "NEW"),
        FailpointOperation::Update => ("UPDATE", "NEW"),
        FailpointOperation::Delete => ("DELETE", "OLD"),
    };
    let name = identifier(&format!("test_support_fail_{table}_{kind}_{event}"));
    let table = identifier(table);
    let sql = if enabled {
        format!(
            "CREATE TEMP TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {table} WHEN {reference}.{kind_column}={kind} BEGIN SELECT RAISE(ABORT,'test-support failpoint'); END"
        )
    } else {
        format!("DROP TRIGGER IF EXISTS temp.{name}")
    };
    connection.execute_batch(&sql)
}

pub fn seed_agent_generation(tx: &rusqlite::Transaction<'_>, value: i64) -> StoreResult<()> {
    use crate::metadata_schema::AgentRecordGeneration;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<AgentRecordGeneration>()
        .put(&AgentRecordGeneration {
            identity: studio_db_api::migration::encode_key(&(1_i64,)),
            source_rowid: 1,
            integer_key: 1,
            agent_record_generation: value,
        })?;
    Ok(())
}

pub fn seed_scheduler_state(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::clock_schema::SchedulerAgentChangeState,
) -> StoreResult<()> {
    use crate::metadata_schema::SchedulerAgentChangeState;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<SchedulerAgentChangeState>()
        .put(&SchedulerAgentChangeState {
            identity: studio_db_api::migration::encode_key(&(row.id,)),
            source_rowid: row.id,
            integer_key: row.id,
            scheduler_floor: row.floor,
            scheduler_generation: row.generation,
            scheduler_entries: row.entries,
            scheduler_total: row.total,
        })?;
    Ok(())
}

pub fn seed_durable_floor(tx: &rusqlite::Transaction<'_>, floor: i64) -> StoreResult<()> {
    use crate::metadata_schema::DurableFloor;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<DurableFloor>()
        .put(&DurableFloor {
            identity: studio_db_api::migration::encode_key(&(1_i64,)),
            source_rowid: 1,
            integer_key: 1,
            durable_floor: floor,
        })?;
    Ok(())
}

pub fn seed_search_rollout(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::settings_schema::SearchRollout,
) -> StoreResult<()> {
    use crate::metadata_schema::SearchRollout;
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<SearchRollout>()
        .put(&SearchRollout {
            identity: studio_db_api::migration::encode_key(&(row.id,)),
            source_rowid: row.id,
            integer_key: row.id,
            search_phase: crate::metadata_values::SearchPhase::from_stored(row.phase.clone()),
            search_cursor: row.cursor,
            search_updated: row.updated,
        })?;
    Ok(())
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::peer_update_settings::canvas_tx;

    #[test]
    fn snapshots_preserve_raw_payloads_null_flags_source_order_and_rollback() {
        let mut db = Connection::open_in_memory().unwrap();
        install_canonical(&db).unwrap();
        let transaction = db.transaction().unwrap();
        for (id, rowid, payload, deleted) in [("b", -9, None, 0), ("a", 7, Some(" raw "), -3)] {
            seed_entity(
                &transaction,
                &crate::clock_schema::SyncEntities {
                    collection: "future".into(),
                    id: id.into(),
                    seq: 15,
                    hash: format!("hash-{id}"),
                    payload: payload.map(str::to_owned),
                    deleted,
                },
                rowid,
            )
            .unwrap();
        }
        let document = crate::publication_schema::SyncDocuments {
            identity: studio_db_api::migration::encode_key(&(8_i64,)),
            source_rowid: 8,
            publication_seq: 8,
            deleted: crate::metadata_values::StoredBoolean::from_stored(2),
            document_scope: "future-scope".into(),
            document_id: "document".into(),
            document_payload: crate::publication_schema::DocumentValue::from_stored(
                "future-scope",
                "document",
                " raw document ".into(),
            )
            .unwrap(),
        };
        canvas_tx(&transaction)
            .partition::<crate::publication_schema::SyncDocuments>()
            .put(&document)
            .unwrap();
        transaction.commit().unwrap();
        let entity = entities(&db).unwrap();
        assert_eq!(
            entity.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!((entity[0].source_rowid, entity[0].deleted), (7, -3));
        assert_eq!(entity[1].payload, None);
        assert_eq!(documents(&db).unwrap()[0].scope, "future-scope");
        let before = publications(&db).unwrap();
        assert_eq!(before.len(), 3);
        assert_eq!(
            before[0].payload.as_deref(),
            Some(b" raw document ".as_slice())
        );
        assert_eq!(before.iter().filter(|row| row.hash.is_some()).count(), 2);
        let transaction = db.transaction().unwrap();
        seed_entity(
            &transaction,
            &crate::clock_schema::SyncEntities {
                collection: "future".into(),
                id: "a".into(),
                seq: 17,
                hash: "changed".into(),
                payload: Some("changed".into()),
                deleted: 0,
            },
            7,
        )
        .unwrap();
        transaction.rollback().unwrap();
        assert_eq!(publications(&db).unwrap(), before);
    }

    #[test]
    fn typed_metadata_seeds_and_kind_failpoint_preserve_rollback() {
        let mut db = Connection::open_in_memory().unwrap();
        install_canonical(&db).unwrap();
        let transaction = db.transaction().unwrap();
        reset_identity(&transaction, Some("fixed-identity")).unwrap();
        set_meta(&transaction, "transcript_copies", Some("retired")).unwrap();
        set_meta(&transaction, "transcript_copies", None).unwrap();
        seed_orchestration_settings(
            &transaction,
            r#"{"expired_before_ms":0,"sendAdmissionCutoverMs":12}"#,
        )
        .unwrap();
        seed_federation_settings(&transaction, r#"{"enabled":true}"#).unwrap();
        seed_access_settings(&transaction, 2).unwrap();
        seed_migration_progress(
            &transaction,
            &crate::settings_schema::PayloadMigrations {
                name: "fixture-progress".into(),
                cursor: 3,
                complete: 0,
                status: "pending".into(),
                updated: None,
                error: None,
                cutover_ms: Some(7),
                receipt_enum_guard: None,
                cursor_key: None,
                copy_source_rows: None,
                copy_target_rows: None,
            },
        )
        .unwrap();
        assert_eq!(
            migration_progress(&transaction, "fixture-progress").unwrap(),
            Some((0, "pending".into()))
        );
        transaction.commit().unwrap();
        let target = FailpointPartition::Metadata(
            crate::consolidation_kinds::CanvasMetadataKind::AccessSettings,
        );
        set_failpoint(&db, target, FailpointOperation::Update, true).unwrap();
        let transaction = db.transaction().unwrap();
        assert!(seed_access_settings(&transaction, 0).is_err());
        seed_orchestration_settings(&transaction, "{}").unwrap();
        transaction.rollback().unwrap();
        assert_eq!(
            crate::settings_schema::AccessSettings::get(&db)
                .unwrap()
                .unwrap()
                .auto_pair,
            2
        );
        set_failpoint(&db, target, FailpointOperation::Update, false).unwrap();
        let transaction = db.transaction().unwrap();
        seed_access_settings(&transaction, 0).unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            crate::settings_schema::AccessSettings::get(&db)
                .unwrap()
                .unwrap()
                .auto_pair,
            0
        );
    }

    #[tokio::test]
    async fn frozen_supported_release_runs_real_startup_and_reopens() {
        let root = tempfile::tempdir().unwrap();
        write_supported_fixture(root.path()).unwrap();
        let source = open_supported_canvas(root.path()).unwrap();
        assert!(crate::schema::table_exists(&source, "sync_entities").unwrap());
        install_legacy_source_schema(&source).unwrap();
        drop(source);
        let store = migrate_legacy(root.path()).await.unwrap();
        let current = store.test_reader().unwrap();
        assert!(!crate::schema::table_exists(&current, "sync_entities").unwrap());
        let before = publications(&current).unwrap();
        drop(current);
        drop(store);
        let store = migrate_legacy(root.path()).await.unwrap();
        let current = store.test_reader().unwrap();
        assert_eq!(publications(&current).unwrap(), before);
    }
}

#[cfg(test)]
mod third_drop_tests {
    use super::*;

    #[test]
    fn clearing_change_partitions_preserves_other_rows_and_rollback() {
        let mut db = Connection::open_in_memory().unwrap();
        install_canonical(&db).unwrap();
        let tx = db.transaction().unwrap();
        seed_scheduler_change(
            &tx,
            &crate::clock_schema::SchedulerAgentChanges {
                id: "same".into(),
                generation: 7,
            },
            1,
        )
        .unwrap();
        seed_transcript_revision(
            &tx,
            &crate::context_settings::TranscriptRevisionsStorageRow {
                agent: "same".into(),
                revision: 9,
            },
            1,
        )
        .unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<crate::change_schema::DurableRows>()
            .put(&crate::change_schema::DurableRows {
                identity: studio_db_api::migration::encode_key(&(
                    "other".to_owned(),
                    "same".to_owned(),
                )),
                source_rowid: 1,
                source_label: crate::change_schema::DurableSource::from_stored("other".into()),
                durable_id: "same".into(),
                sequence: 11,
                deleted: crate::metadata_values::StoredBoolean::Valid(false),
                deleted_at: 0,
                scope: None,
            })
            .unwrap();
        tx.commit().unwrap();
        let scheduler = scheduler_changes(&db).unwrap();
        let transcripts = transcript_revisions(&db).unwrap();
        let durable = changes(&db).unwrap();
        let tx = db.transaction().unwrap();
        clear_scheduler_changes(&tx).unwrap();
        assert!(scheduler_changes(&tx).unwrap().is_empty());
        assert_eq!(transcript_revisions(&tx).unwrap(), transcripts);
        assert_eq!(changes(&tx).unwrap(), durable);
        tx.rollback().unwrap();
        assert_eq!(scheduler_changes(&db).unwrap(), scheduler);
        let tx = db.transaction().unwrap();
        clear_transcript_revisions(&tx).unwrap();
        assert!(transcript_revisions(&tx).unwrap().is_empty());
        assert_eq!(scheduler_changes(&tx).unwrap(), scheduler);
        assert_eq!(changes(&tx).unwrap(), durable);
        tx.rollback().unwrap();
        assert_eq!(transcript_revisions(&db).unwrap(), transcripts);
        let tx = db.transaction().unwrap();
        clear_scheduler_changes(&tx).unwrap();
        clear_transcript_revisions(&tx).unwrap();
        tx.commit().unwrap();
        assert!(scheduler_changes(&db).unwrap().is_empty());
        assert!(transcript_revisions(&db).unwrap().is_empty());
        assert_eq!(changes(&db).unwrap(), durable);
    }

    #[test]
    fn stored_batch_totals_preserve_empty_sum_and_name_partition() {
        let mut db = Connection::open_in_memory().unwrap();
        install_canonical(&db).unwrap();
        assert_eq!(stored_text_batch_totals(&db, "wanted").unwrap(), (0, None));
        let tx = db.transaction().unwrap();
        for (name, batch, rows) in [("wanted", 1, 3), ("other", 2, 99), ("wanted", 3, 4)] {
            crate::peer_update_settings::canvas_tx(&tx)
                .partition::<crate::metadata_schema::StoredTextBatches>()
                .put(&crate::metadata_schema::StoredTextBatches {
                    identity: studio_db_api::migration::encode_key(&(name.to_owned(), batch)),
                    source_rowid: batch,
                    text_key: name.into(),
                    batch,
                    first_rowid: 1,
                    last_rowid: 1,
                    batch_rows: rows,
                    plain_bytes: 10,
                    stored_bytes: 5,
                    sha256: "0".repeat(64),
                    batch_created: 0.0,
                })
                .unwrap();
        }
        assert_eq!(
            stored_text_batch_totals(&tx, "wanted").unwrap(),
            (2, Some(7))
        );
        tx.rollback().unwrap();
        assert_eq!(stored_text_batch_totals(&db, "wanted").unwrap(), (0, None));
    }

    #[test]
    fn third_drop_reads_and_seeds_keep_change_families_separate() {
        let mut db = Connection::open_in_memory().unwrap();
        install_canonical(&db).unwrap();
        let tx = db.transaction().unwrap();
        set_meta(&tx, "z", Some(" last ")).unwrap();
        set_meta(&tx, "a", Some("first")).unwrap();
        for name in ["first-progress", "second-progress"] {
            seed_migration_progress(
                &tx,
                &crate::settings_schema::PayloadMigrations {
                    name: name.into(),
                    cursor: 0,
                    complete: 0,
                    status: "pending".into(),
                    updated: None,
                    error: None,
                    cutover_ms: None,
                    receipt_enum_guard: None,
                    cursor_key: None,
                    copy_source_rows: None,
                    copy_target_rows: None,
                },
            )
            .unwrap();
            assert_eq!(
                migration_progress(&tx, name).unwrap(),
                Some((0, "pending".into()))
            );
        }
        seed_scheduler_change(
            &tx,
            &crate::clock_schema::SchedulerAgentChanges {
                id: "same".into(),
                generation: 12,
            },
            -7,
        )
        .unwrap();
        seed_transcript_revision(
            &tx,
            &crate::context_settings::TranscriptRevisionsStorageRow {
                agent: "same".into(),
                revision: 31,
            },
            -7,
        )
        .unwrap();
        assert_eq!(
            meta_entries(&tx).unwrap(),
            vec![("a".into(), "first".into()), ("z".into(), " last ".into())]
        );
        assert_eq!(scheduler_state(&tx).unwrap().unwrap().generation, 0);
        assert_eq!(
            scheduler_changes(&tx).unwrap(),
            vec![SchedulerChangeSnapshot {
                id: Some("same".into()),
                generation: 12,
                source_rowid: -7
            }]
        );
        assert_eq!(
            transcript_revisions(&tx).unwrap(),
            vec![TranscriptRevisionSnapshot {
                agent: Some("same".into()),
                revision: 31,
                source_rowid: -7
            }]
        );
        assert_eq!(
            agent_clock_trigger_sql(),
            crate::clock_schema::target_agent_clock_sql()
        );
        tx.rollback().unwrap();
        assert!(meta_entries(&db).unwrap().is_empty());
        assert!(scheduler_changes(&db).unwrap().is_empty());
        assert!(transcript_revisions(&db).unwrap().is_empty());
    }
}

/// Exact named sync markers, sorted by business key. NULL is not a marker name.
pub fn meta_entries(connection: &Connection) -> StoreResult<Vec<(String, String)>> {
    let mut entries = studio_db_api::db_query!(
        canvas_read(connection),
        crate::metadata_schema::SyncEntityMeta
    )?
    .into_iter()
    .map(|row| {
        Ok((
            row.text_key
                .ok_or(crate::StoreError::RecordContract("sync marker has no name"))?,
            row.meta_value.as_stored().to_owned(),
        ))
    })
    .collect::<StoreResult<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

pub fn scheduler_state(
    connection: &Connection,
) -> StoreResult<Option<crate::clock_schema::SchedulerAgentChangeState>> {
    Ok(canvas_read(connection)
        .get::<crate::metadata_schema::SchedulerAgentChangeState>(&(
            studio_db_api::migration::encode_key(&(1_i64,)),
        ))?
        .map(|row| crate::clock_schema::SchedulerAgentChangeState {
            id: row.integer_key,
            floor: row.scheduler_floor,
            generation: row.scheduler_generation,
            entries: row.scheduler_entries,
            total: row.scheduler_total,
        }))
}

/// Pure output of the canonical agent-clock producer owner for minimal fixtures.
pub fn agent_clock_trigger_sql() -> String {
    crate::clock_schema::target_agent_clock_sql()
}

/// Refresh current producer definitions through the authority owner's installer.
pub fn install_current_producers(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    crate::bootstrap::install_current_producers(transaction)
}

pub fn seed_scheduler_change(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::clock_schema::SchedulerAgentChanges,
    source_rowid: i64,
) -> StoreResult<()> {
    let original_key = Some(row.id.clone());
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<crate::change_schema::SchedulerAgentChanges>()
        .put(&crate::change_schema::SchedulerAgentChanges {
            identity: crate::consolidation_codec::nullable_text_identity(
                &original_key,
                source_rowid,
            ),
            source_rowid,
            original_key,
            generation: row.generation,
        })?;
    Ok(())
}

pub fn seed_transcript_revision(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::context_settings::TranscriptRevisionsStorageRow,
    source_rowid: i64,
) -> StoreResult<()> {
    let original_key = Some(row.agent.clone());
    crate::peer_update_settings::canvas_tx(tx)
        .partition::<crate::change_schema::TranscriptRevisions>()
        .put(&crate::change_schema::TranscriptRevisions {
            identity: crate::consolidation_codec::nullable_text_identity(
                &original_key,
                source_rowid,
            ),
            source_rowid,
            original_key,
            revision: row.revision,
        })?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct SchedulerChangeSnapshot {
    pub id: Option<String>,
    pub generation: i64,
    pub source_rowid: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TranscriptRevisionSnapshot {
    pub agent: Option<String>,
    pub revision: i64,
    pub source_rowid: i64,
}

pub fn scheduler_changes(connection: &Connection) -> StoreResult<Vec<SchedulerChangeSnapshot>> {
    let mut rows = studio_db_api::db_query!(
        canvas_read(connection),
        crate::change_schema::SchedulerAgentChanges
    )?
    .into_iter()
    .map(|row| SchedulerChangeSnapshot {
        id: row.original_key,
        generation: row.generation,
        source_rowid: row.source_rowid,
    })
    .collect::<Vec<_>>();
    rows.sort_by(|left, right| (&left.id, left.source_rowid).cmp(&(&right.id, right.source_rowid)));
    Ok(rows)
}
pub fn transcript_revisions(
    connection: &Connection,
) -> StoreResult<Vec<TranscriptRevisionSnapshot>> {
    let mut rows = studio_db_api::db_query!(
        canvas_read(connection),
        crate::change_schema::TranscriptRevisions
    )?
    .into_iter()
    .map(|row| TranscriptRevisionSnapshot {
        agent: row.original_key,
        revision: row.revision,
        source_rowid: row.source_rowid,
    })
    .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (&left.agent, left.source_rowid).cmp(&(&right.agent, right.source_rowid))
    });
    Ok(rows)
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=(i64,Option<i64>),
    tables(metadata=crate::metadata_schema::Metadata),
    columns(kind=crate::metadata_schema::Metadata::KIND,name=crate::metadata_schema::Metadata::TEXT_KEY,rows=crate::metadata_schema::Metadata::BATCH_ROWS),
    sql="SELECT count(*),sum({rows}) FROM {metadata} WHERE {kind}=?1 AND {name}=?2")]
struct StoredTextBatchTotals<'a> {
    #[db(column=crate::metadata_schema::Metadata::KIND)]
    kind: crate::consolidation_kinds::CanvasMetadataKind,
    #[db(column=crate::metadata_schema::Metadata::TEXT_KEY)]
    name: Option<&'a str>,
}

pub fn stored_text_batch_totals(
    connection: &Connection,
    name: &str,
) -> rusqlite::Result<(i64, Option<i64>)> {
    canvas_read(connection).named(
        &StoredTextBatchTotals::QUERY,
        &StoredTextBatchTotals {
            kind: crate::consolidation_kinds::CanvasMetadataKind::StoredTextBatches,
            name: Some(name),
        },
    )
}

pub fn clear_scheduler_changes(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    use crate::change_schema::SchedulerAgentChanges;
    let mut typed = crate::peer_update_settings::canvas_tx(transaction);
    for row in studio_db_api::db_query!(typed.read(), SchedulerAgentChanges)? {
        typed.delete::<SchedulerAgentChanges>(&(row.identity,))?;
    }
    Ok(())
}

pub fn clear_transcript_revisions(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    use crate::change_schema::TranscriptRevisions;
    let mut typed = crate::peer_update_settings::canvas_tx(transaction);
    for row in studio_db_api::db_query!(typed.read(), TranscriptRevisions)? {
        typed.delete::<TranscriptRevisions>(&(row.identity,))?;
    }
    Ok(())
}
