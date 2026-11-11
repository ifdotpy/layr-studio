use crate::consolidation_codec::{Required, RequiredJson};
use crate::consolidation_kinds::{
    CanvasChangesKind, CanvasMetadataKind as MetadataKind,
    CanvasPublicationsKind as PublicationKind,
};
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{Codec, DbPartition, DbTable, DbValue, Table};

#[derive(DbTable)]
#[db(table="f1_metadata_witness", database=Canvas, partitioned_by=MetadataKind)]
struct Metadata {
    #[db(key)]
    kind: MetadataKind,
    #[db(key)]
    identity: Vec<u8>,
    #[db(
        check = "(kind=1 AND original_client IS NOT NULL) OR (kind<>1 AND original_client IS NULL)"
    )]
    original_client: Option<String>,
    #[db(
        check = "(kind=1 AND command_applied IS NOT NULL AND command_applied>=0) OR (kind<>1 AND command_applied IS NULL)"
    )]
    command_applied: Option<i64>,
    #[db(
        check = "(kind=19 AND dirty_migration IS NOT NULL AND ((dirty_integer IS NOT NULL AND dirty_blob IS NULL) OR (dirty_integer IS NULL AND dirty_blob IS NOT NULL))) OR (kind<>19 AND dirty_migration IS NULL AND dirty_integer IS NULL AND dirty_blob IS NULL)"
    )]
    dirty_migration: Option<String>,
    dirty_integer: Option<i64>,
    dirty_blob: Option<Vec<u8>>,
}
#[derive(DbPartition)]
#[db(of=Metadata, kind=MetadataKind::ClientCommandWatermarks)]
struct ClientCommandWatermarks {
    #[db(key)]
    identity: Vec<u8>,
    #[db(codec=Required<String>)]
    original_client: String,
    #[db(codec=Required<i64>)]
    command_applied: i64,
}
#[derive(DbPartition)]
#[db(of=Metadata, kind=MetadataKind::OnlineMigrationDirty)]
struct OnlineMigrationDirty {
    #[db(key)]
    identity: Vec<u8>,
    #[db(codec=Required<String>)]
    dirty_migration: String,
    dirty_integer: Option<i64>,
    dirty_blob: Option<Vec<u8>>,
}

#[test]
fn f1_sparse_partition_witness() {
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    Metadata::descriptor().install_if_absent(&db).unwrap();
    let tx = db.transaction().unwrap();
    let mut typed = canvas_tx(&tx);
    typed
        .partition::<ClientCommandWatermarks>()
        .put(&ClientCommandWatermarks {
            identity: vec![1],
            original_client: "client".into(),
            command_applied: 7,
        })
        .unwrap();
    assert_eq!(
        typed
            .get::<ClientCommandWatermarks>(&(vec![1],))
            .unwrap()
            .unwrap()
            .command_applied,
        7
    );
    typed
        .partition::<OnlineMigrationDirty>()
        .put(&OnlineMigrationDirty {
            identity: vec![1],
            dirty_migration: "online:other".into(),
            dirty_integer: Some(-7),
            dirty_blob: None,
        })
        .unwrap();
    assert!(
        typed
            .partition::<OnlineMigrationDirty>()
            .put(&OnlineMigrationDirty {
                identity: vec![2],
                dirty_migration: "online:other".into(),
                dirty_integer: Some(1),
                dirty_blob: Some(vec![])
            })
            .is_err()
    );
    assert_eq!(
        typed
            .get::<ClientCommandWatermarks>(&(vec![1],))
            .unwrap()
            .unwrap()
            .command_applied,
        7
    );
    assert_eq!(
        typed
            .get::<OnlineMigrationDirty>(&(vec![1],))
            .unwrap()
            .unwrap()
            .dirty_integer,
        Some(-7)
    );
    assert_eq!(
        typed.delete::<OnlineMigrationDirty>(&(vec![1],)).unwrap(),
        1
    );
    assert!(
        typed
            .get::<ClientCommandWatermarks>(&(vec![1],))
            .unwrap()
            .is_some()
    );
}

#[test]
fn f1_required_partition_codec_refuses_null_without_changing_derive() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    assert!(Required::<String>::decode(&db, None).is_err());
    assert_eq!(Required::<i64>::decode(&db, Some(7)).unwrap(), 7);
}

// Same physical TEXT storage, distinct named payload views, exact admitted bytes.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
#[derive(serde::Serialize, serde::Deserialize)]
struct PayloadWitness {
    revision: i64,
}
#[derive(DbTable)]
#[db(table="f1_publications_witness", database=Canvas, partitioned_by=PublicationKind)]
struct Publications {
    #[db(key)]
    kind: PublicationKind,
    #[db(key)]
    identity: Vec<u8>,
    #[db(check = "publication_seq IS NOT NULL")]
    publication_seq: Option<i64>,
    #[db(
        check = "(kind=1 AND document_scope IS NOT NULL AND document_id IS NOT NULL AND document_payload IS NOT NULL) OR (kind<>1 AND document_scope IS NULL AND document_id IS NULL AND document_payload IS NULL)"
    )]
    document_scope: Option<String>,
    document_id: Option<String>,
    document_payload: Option<String>,
    #[db(
        check = "(kind=2 AND collection_order IS NOT NULL AND entity_id IS NOT NULL AND entity_hash IS NOT NULL) OR (kind<>2 AND collection_order IS NULL AND entity_id IS NULL AND entity_hash IS NULL AND entity_payload IS NULL)"
    )]
    collection_order: Option<String>,
    entity_id: Option<String>,
    entity_hash: Option<String>,
    entity_payload: Option<String>,
    #[db(check = "deleted IS NOT NULL")]
    deleted: Option<bool>,
}
#[derive(DbPartition)]
#[db(of=Publications, kind=PublicationKind::SyncDocuments,
    index(name="f1_witness_document_identity", columns(document_scope,document_id), unique))]
struct SyncDocuments {
    #[db(key)]
    identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    publication_seq: i64,
    #[db(codec=Required<String>)]
    document_scope: String,
    #[db(codec=Required<String>)]
    document_id: String,
    #[db(codec=RequiredJson<PayloadWitness>)]
    document_payload: ExactJson<PayloadWitness>,
    #[db(codec=Required<bool>)]
    deleted: bool,
}
#[derive(DbPartition)]
#[db(of=Publications, kind=PublicationKind::SyncEntities,
    index(name="f1_witness_entity_identity", columns(collection_order,entity_id), unique))]
struct SyncEntities {
    #[db(key)]
    identity: Vec<u8>,
    #[db(codec=Required<i64>)]
    publication_seq: i64,
    #[db(codec=Required<String>)]
    collection_order: String,
    #[db(codec=Required<String>)]
    entity_id: String,
    #[db(codec=Required<String>)]
    entity_hash: String,
    #[db(codec=ExactJsonCodec<PayloadWitness>)]
    entity_payload: Option<ExactJson<PayloadWitness>>,
    #[db(codec=Required<bool>)]
    deleted: bool,
}

#[test]
fn f1_publication_sparse_layout_retains_exact_payloads_and_scope_uniqueness() {
    use studio_db_api::migration::encode_key;
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    SyncDocuments::descriptor().install_if_absent(&db).unwrap();
    SyncEntities::descriptor().install_if_absent(&db).unwrap();
    let tx = db.transaction().unwrap();
    let mut typed = canvas_tx(&tx);
    let raw = "{ \"revision\" : 3 }";
    let document = |seq| SyncDocuments {
        identity: encode_key(&(seq,)),
        publication_seq: seq,
        document_scope: "drafts".into(),
        document_id: "one".into(),
        document_payload: ExactJson::from_stored(raw.into()).unwrap(),
        deleted: false,
    };
    typed
        .partition::<SyncDocuments>()
        .put(&document(7))
        .unwrap();
    typed
        .partition::<SyncEntities>()
        .put(&SyncEntities {
            identity: encode_key(&(7i64,)),
            publication_seq: 7,
            collection_order: "unknown:scope".into(),
            entity_id: "one".into(),
            entity_hash: "existing exact hash".into(),
            entity_payload: None,
            deleted: true,
        })
        .unwrap();
    let stored = typed
        .get::<SyncDocuments>(&(encode_key(&(7i64,)),))
        .unwrap()
        .unwrap();
    assert_eq!(stored.document_payload.raw(), raw);
    assert_eq!(stored.document_payload.value().revision, 3);
    assert!(
        typed
            .get::<SyncEntities>(&(encode_key(&(7i64,)),))
            .unwrap()
            .unwrap()
            .entity_payload
            .is_none()
    );
    assert_eq!(
        typed
            .read()
            .collect(&SyncDocuments::SELECT_QUERY, &())
            .unwrap()
            .len(),
        1
    );
    // A new sequence must replace the old physical identity in the same transaction.
    assert!(
        typed
            .partition::<SyncDocuments>()
            .put(&document(8))
            .is_err()
    );
    assert_eq!(
        typed
            .delete::<SyncDocuments>(&(encode_key(&(7i64,)),))
            .unwrap(),
        1
    );
    typed
        .partition::<SyncDocuments>()
        .put(&document(8))
        .unwrap();
    assert!(
        typed
            .get::<SyncDocuments>(&(encode_key(&(7i64,)),))
            .unwrap()
            .is_none()
    );
    assert!(
        typed
            .get::<SyncEntities>(&(encode_key(&(7i64,)),))
            .unwrap()
            .is_some()
    );
    assert!(RequiredJson::<PayloadWitness>::decode(&tx, None).is_err());
}

#[test]
fn f1_kind_codes_are_integer_and_closed_in_independent_namespaces() {
    use rusqlite::types::{FromSql, ValueRef};
    assert_eq!(MetadataKind::CODE_NAMES.len(), 19);
    assert_eq!(CanvasChangesKind::CODE_NAMES.len(), 3);
    assert_eq!(PublicationKind::CODE_NAMES.len(), 2);
    assert_eq!(MetadataKind::ClientCommandWatermarks as i64, 1);
    assert_eq!(MetadataKind::OnlineMigrationDirty as i64, 19);
    assert_eq!(CanvasChangesKind::DurableRows as i64, 1);
    assert_eq!(PublicationKind::SyncDocuments as i64, 1);
    assert_eq!(MetadataKind::SQL_TYPE, "INTEGER");
    assert_eq!(CanvasChangesKind::SQL_TYPE, "INTEGER");
    assert_eq!(PublicationKind::SQL_TYPE, "INTEGER");
    assert_eq!(
        MetadataKind::column_result(ValueRef::Integer(19)).unwrap(),
        MetadataKind::OnlineMigrationDirty
    );
    assert!(MetadataKind::column_result(ValueRef::Integer(20)).is_err());
    assert!(CanvasChangesKind::column_result(ValueRef::Integer(4)).is_err());
    assert!(PublicationKind::column_result(ValueRef::Integer(3)).is_err());
    assert!(MetadataKind::column_result(ValueRef::Text(b"1")).is_err());
}
