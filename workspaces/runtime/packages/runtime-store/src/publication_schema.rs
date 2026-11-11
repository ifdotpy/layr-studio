//! Physical publication storage. Contextual typed views are owned by this module.
use crate::consolidation_kinds::CanvasPublicationsKind as Kind;
use crate::peer_update_settings::Canvas;
use crate::publication_collection::CollectionKind;
use studio_db_api::DbTable;

const DOCUMENT_SHAPE: &str = studio_db_api::static_sql!(
    "(kind=",
    Kind::DB_CODE_SYNCDOCUMENTS,
    " AND document_scope IS NOT NULL AND document_id IS NOT NULL AND document_payload IS NOT NULL AND source_rowid=publication_seq) OR (kind<>",
    Kind::DB_CODE_SYNCDOCUMENTS,
    " AND document_scope IS NULL AND document_id IS NULL AND document_payload IS NULL)"
);
const ENTITY_SHAPE: &str = studio_db_api::static_sql!(
    "(kind=",
    Kind::DB_CODE_SYNCENTITIES,
    " AND collection_order IS NOT NULL AND collection_kind IS NOT NULL AND entity_id IS NOT NULL AND entity_hash IS NOT NULL AND non_transcript IS NOT NULL) OR (kind<>",
    Kind::DB_CODE_SYNCENTITIES,
    " AND collection_order IS NULL AND collection_kind IS NULL AND collection_agent IS NULL AND entity_id IS NULL AND entity_hash IS NULL AND entity_payload IS NULL AND non_transcript IS NULL)"
);
const COLLECTION_SHAPE: &str = studio_db_api::static_sql!(
    "(collection_kind IS NULL AND collection_agent IS NULL) OR (collection_kind=",
    CollectionKind::DB_CODE_TRANSCRIPT,
    " AND collection_agent IS NOT NULL AND non_transcript=0) OR (collection_kind<>",
    CollectionKind::DB_CODE_TRANSCRIPT,
    " AND collection_agent IS NULL AND non_transcript=1)"
);
const DOCUMENTS: &str = studio_db_api::static_sql!("kind=", Kind::DB_CODE_SYNCDOCUMENTS);
const ENTITIES: &str = studio_db_api::static_sql!("kind=", Kind::DB_CODE_SYNCENTITIES);
const TOMBSTONES: &str = studio_db_api::static_sql!(
    "kind=",
    Kind::DB_CODE_SYNCENTITIES,
    " AND deleted=1 AND non_transcript=1"
);

#[derive(DbTable)]
#[db(table="publications",database=Canvas,partitioned_by=Kind,
    index(name="publications_sequence",columns(publication_seq,source_rowid)),
    index(name="publications_document_identity",columns(document_scope,document_id),unique,predicate=DOCUMENTS),
    index(name="publications_document_scope_sequence",columns(document_scope,publication_seq),predicate=DOCUMENTS),
    index(name="publications_entity_identity",columns(collection_order,entity_id),unique,predicate=ENTITIES),
    index(name="publications_source_rowid",columns(kind,source_rowid),unique),
    index(name="publications_entity_collection_deleted",columns(collection_order,deleted),predicate=ENTITIES),
    index(name="publications_entity_collection_sequence",columns(kind,collection_order,publication_seq,source_rowid)),
    index(name="publications_entity_non_transcript_sequence",columns(kind,non_transcript,publication_seq,source_rowid)),
    index(name="publications_entity_prune_order",columns(publication_seq,collection_order,entity_id),predicate=TOMBSTONES),
    index(name="publications_entity_id",columns(kind,entity_id)))]
pub(crate) struct Publications {
    #[db(key)]
    kind: Kind,
    #[db(key)]
    identity: Vec<u8>,
    source_rowid: i64,
    publication_seq: i64,
    deleted: i64,
    #[db(check=DOCUMENT_SHAPE)]
    document_scope: Option<String>,
    document_id: Option<String>,
    document_payload: Option<String>,
    #[db(check=ENTITY_SHAPE)]
    collection_order: Option<String>,
    #[db(check=COLLECTION_SHAPE)]
    collection_kind: Option<CollectionKind>,
    collection_agent: Option<String>,
    entity_id: Option<String>,
    entity_hash: Option<String>,
    entity_payload: Option<String>,
    non_transcript: Option<bool>,
}

use crate::consolidation_codec::{Required, RequiredCodec};
use crate::metadata_values::{StoredBoolean, StoredBooleanCodec};
use crate::publication_collection::{CollectionName, CollectionNameCodec};
use crate::publication_payload::{DocumentPayload, EntityPayload};
use studio_db_api::migration::encode_key;
use studio_db_api::{Codec, CodecContext, CodecError, CodecRow, DbPartition};

fn payload_error(message: String) -> CodecError {
    CodecError::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message,
    )))
}

/// Immutable context accompanies the admitted value, so a full-row write can
/// check identity without parsing the payload a second time.
pub(crate) struct DocumentValue {
    scope: String,
    id: String,
    value: DocumentPayload,
}
impl DocumentValue {
    pub fn from_stored(scope: &str, id: &str, raw: String) -> Result<Self, CodecError> {
        let value = DocumentPayload::from_stored(scope, id, raw).map_err(payload_error)?;
        Ok(Self {
            scope: scope.into(),
            id: id.into(),
            value,
        })
    }
    pub fn value(&self) -> &DocumentPayload {
        &self.value
    }
}
pub(crate) struct DocumentValueCodec;
impl Codec<DocumentValue> for DocumentValueCodec {
    type Stored = Option<String>;
    type Encoded<'a> = &'a str;
    fn encode<'a>(_: &CodecContext, value: &'a DocumentValue) -> Result<&'a str, CodecError> {
        Ok(value.value().raw())
    }
    fn decode(_: &CodecContext, _: Option<String>) -> Result<DocumentValue, CodecError> {
        Err(CodecError::InvalidQuery)
    }
    fn decode_row(
        _: &CodecContext,
        row: &CodecRow<'_>,
        index: usize,
    ) -> Result<DocumentValue, CodecError> {
        let scope: String = row.get(Publications::__DB_COLUMN_DOCUMENT_SCOPE)?;
        let id: String = row.get(Publications::__DB_COLUMN_DOCUMENT_ID)?;
        let raw: String = row.get(index)?;
        DocumentValue::from_stored(&scope, &id, raw)
    }
}

pub(crate) struct EntityValue {
    collection: String,
    id: String,
    deleted: StoredBoolean,
    value: EntityPayload,
}
impl EntityValue {
    pub fn from_stored(
        collection: &str,
        id: &str,
        deleted: StoredBoolean,
        raw: Option<String>,
    ) -> Result<Self, CodecError> {
        let value = EntityPayload::from_stored(collection, id, deleted.as_stored() != 0, raw)
            .map_err(payload_error)?;
        Ok(Self {
            collection: collection.into(),
            id: id.into(),
            deleted,
            value,
        })
    }
    pub fn value(&self) -> &EntityPayload {
        &self.value
    }
}
pub(crate) struct EntityValueCodec;
impl Codec<EntityValue> for EntityValueCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(_: &CodecContext, value: &'a EntityValue) -> Result<Option<&'a str>, CodecError> {
        Ok(value.value().raw())
    }
    fn decode(_: &CodecContext, _: Option<String>) -> Result<EntityValue, CodecError> {
        Err(CodecError::InvalidQuery)
    }
    fn decode_row(
        _: &CodecContext,
        row: &CodecRow<'_>,
        index: usize,
    ) -> Result<EntityValue, CodecError> {
        let collection: String = row.get(Publications::__DB_COLUMN_COLLECTION_ORDER)?;
        let id: String = row.get(Publications::__DB_COLUMN_ENTITY_ID)?;
        let deleted = StoredBoolean::from_stored(row.get(Publications::__DB_COLUMN_DELETED)?);
        EntityValue::from_stored(&collection, &id, deleted, row.get(index)?)
    }
}
pub(crate) struct AgentArgumentCodec;
impl Codec<Option<crate::AgentId>> for AgentArgumentCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(
        _: &CodecContext,
        value: &'a Option<crate::AgentId>,
    ) -> Result<Option<&'a str>, CodecError> {
        Ok(value.as_ref().map(AsRef::as_ref))
    }
    fn decode(
        _: &CodecContext,
        value: Option<String>,
    ) -> Result<Option<crate::AgentId>, CodecError> {
        Ok(value.map(crate::AgentId::from))
    }
}

#[derive(DbPartition)]
#[db(of=Publications,kind=Kind::SyncDocuments,check=check_document)]
pub(crate) struct SyncDocuments {
    #[db(key)]
    pub identity: Vec<u8>,
    pub source_rowid: i64,
    pub publication_seq: i64,
    #[db(codec=StoredBooleanCodec)]
    pub deleted: StoredBoolean,
    #[db(codec=Required<String>)]
    pub document_scope: String,
    #[db(codec=Required<String>)]
    pub document_id: String,
    #[db(codec=DocumentValueCodec)]
    pub document_payload: DocumentValue,
}
fn check_document(row: &SyncDocuments) -> crate::StoreResult<()> {
    if row.identity != encode_key(&(row.publication_seq,))
        || row.source_rowid != row.publication_seq
        || row.document_scope != row.document_payload.scope
        || row.document_id != row.document_payload.id
    {
        return Err(crate::StoreError::RecordContract(
            "document publication context mismatch",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Publications,kind=Kind::SyncEntities,check=check_entity)]
pub(crate) struct SyncEntities {
    #[db(key)]
    pub identity: Vec<u8>,
    pub source_rowid: i64,
    pub publication_seq: i64,
    #[db(codec=StoredBooleanCodec)]
    pub deleted: StoredBoolean,
    #[db(codec=RequiredCodec<CollectionNameCodec,CollectionName>)]
    pub collection_order: CollectionName,
    #[db(codec=Required<CollectionKind>)]
    pub collection_kind: CollectionKind,
    #[db(codec=AgentArgumentCodec)]
    pub collection_agent: Option<crate::AgentId>,
    #[db(codec=Required<String>)]
    pub entity_id: String,
    #[db(codec=Required<String>)]
    pub entity_hash: String,
    #[db(codec=EntityValueCodec)]
    pub entity_payload: EntityValue,
    #[db(codec=Required<bool>)]
    pub non_transcript: bool,
}
fn check_entity(row: &SyncEntities) -> crate::StoreResult<()> {
    let label = row.collection_order.as_str();
    if row.identity != encode_key(&(label.to_owned(), row.entity_id.clone()))
        || row.collection_kind != row.collection_order.kind()
        || row.collection_agent.as_ref() != row.collection_order.agent()
        || row.non_transcript != row.collection_order.non_transcript()
        || row.entity_payload.collection != label
        || row.entity_payload.id != row.entity_id
        || row.entity_payload.deleted != row.deleted
    {
        return Err(crate::StoreError::RecordContract(
            "entity publication context mismatch",
        ));
    }
    if row.collection_kind == CollectionKind::Transcript
        && let Some((pointer, keyword)) =
            crate::repositories::transcript::transcript_index_storage_error(
                label,
                &row.entity_id,
                row.publication_seq,
                &row.entity_hash,
                row.entity_payload.value().raw().is_none(),
                row.deleted.as_stored(),
            )
    {
        return Err(crate::StoreError::SchemaValidation(format!(
            "transcript publication {pointer} failed {keyword}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::{canvas_read, canvas_tx};
    use studio_db_api::{DbQuery, Table};
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=Option<bool>,decode=Publications::NON_TRANSCRIPT,
        columns(flag=Publications::NON_TRANSCRIPT),sql="SELECT (?1 NOT LIKE 'transcript:%') AS {flag}")]
    struct ShippedTranscriptPredicate {
        #[db(column=Publications::COLLECTION_ORDER)]
        label: Option<String>,
    }
    fn entity(
        label: &str,
        id: &str,
        source_rowid: i64,
        publication_seq: i64,
        deleted: StoredBoolean,
        raw: Option<&str>,
    ) -> SyncEntities {
        let collection_order = CollectionName::from_stored(label.into());
        SyncEntities {
            identity: encode_key(&(label.to_owned(), id.to_owned())),
            source_rowid,
            publication_seq,
            deleted,
            collection_kind: collection_order.kind(),
            collection_agent: collection_order.agent().cloned(),
            non_transcript: collection_order.non_transcript(),
            collection_order,
            entity_id: id.into(),
            entity_hash: "0".repeat(64),
            entity_payload: EntityValue::from_stored(label, id, deleted, raw.map(str::to_owned))
                .unwrap(),
        }
    }
    fn assert_transcript_partition_refuses(row: &SyncEntities) {
        let mut db = CodecContext::open_in_memory().unwrap();
        Publications::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        assert!(tx.partition::<SyncEntities>().put(row).is_err());
    }

    fn valid_transcript_index() -> SyncEntities {
        let mut row = entity(
            "transcript:a",
            "@order",
            1,
            1,
            StoredBoolean::Valid(false),
            None,
        );
        row.entity_hash = "0".repeat(64);
        row
    }

    #[test]
    fn f1_transcript_partition_refuses_invalid_id() {
        let mut row = valid_transcript_index();
        row.entity_id = "garbage".into();
        row.identity = encode_key(&("transcript:a".to_owned(), row.entity_id.clone()));
        row.entity_payload =
            EntityValue::from_stored("transcript:a", &row.entity_id, row.deleted, None).unwrap();
        assert_transcript_partition_refuses(&row);
    }

    #[test]
    fn f1_transcript_partition_refuses_negative_sequence() {
        let mut row = valid_transcript_index();
        row.publication_seq = -1;
        assert_transcript_partition_refuses(&row);
    }

    #[test]
    fn f1_transcript_partition_refuses_invalid_hash() {
        let mut row = valid_transcript_index();
        row.entity_hash = "x".into();
        assert_transcript_partition_refuses(&row);
    }

    #[test]
    fn f1_transcript_partition_refuses_invalid_deleted_flag() {
        let mut row = valid_transcript_index();
        row.deleted = StoredBoolean::LegacyInteger(-9);
        row.entity_payload =
            EntityValue::from_stored("transcript:a", &row.entity_id, row.deleted, None).unwrap();
        assert_transcript_partition_refuses(&row);
    }

    #[test]
    fn f1_publication_context_roundtrips_document_live_cache_null_and_legacy() {
        let mut db = CodecContext::open_in_memory().unwrap();
        assert_eq!(Publications::INDEXES.len(), 10);
        Publications::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        let draft = " {\"session\":\"s\",\"device\":\"d\",\"text\":\"\\u0061\",\"updated\":1.75} ";
        let document = SyncDocuments {
            identity: encode_key(&(7_i64,)),
            source_rowid: 7,
            publication_seq: 7,
            deleted: StoredBoolean::Valid(false),
            document_scope: "drafts".into(),
            document_id: "d:s".into(),
            document_payload: DocumentValue::from_stored("drafts", "d:s", draft.into()).unwrap(),
        };
        tx.partition::<SyncDocuments>().put(&document).unwrap();
        let stored = tx
            .get::<SyncDocuments>(&(document.identity,))
            .unwrap()
            .unwrap();
        assert_eq!(stored.document_payload.value().raw(), draft);
        assert!(matches!(
            stored.document_payload.value(),
            DocumentPayload::Draft { .. }
        ));
        let live =
            "{\"collection\":\"edge\",\"id\":\"e\",\"value\":{\"id\":\"e\",\"kind\":\"chat\"}}";
        for row in [
            entity("edge", "e", -7, 17, StoredBoolean::Valid(false), Some(live)),
            entity(
                "transcript:a",
                "@order",
                3,
                18,
                StoredBoolean::Valid(false),
                None,
            ),
            entity(
                "transcript:a",
                "@revision",
                5,
                19,
                StoredBoolean::Valid(false),
                Some(" {} "),
            ),
            entity(
                "future\0collection",
                "old",
                7,
                20,
                StoredBoolean::LegacyInteger(2),
                Some("not JSON"),
            ),
            entity("edge", "null", 9, 21, StoredBoolean::Valid(false), None),
        ] {
            tx.partition::<SyncEntities>().put(&row).unwrap();
            let stored = tx.get::<SyncEntities>(&(row.identity,)).unwrap().unwrap();
            assert_eq!(
                stored.entity_payload.value().raw(),
                row.entity_payload.value().raw()
            );
            assert_eq!(stored.deleted, row.deleted);
            assert_eq!(stored.collection_kind, row.collection_kind);
            assert_eq!(
                stored.collection_order.as_str(),
                row.collection_order.as_str()
            );
            assert_eq!(stored.collection_agent, row.collection_agent);
        }
        let live = tx
            .get::<SyncEntities>(&(encode_key(&("edge".to_owned(), "e".to_owned())),))
            .unwrap()
            .unwrap();
        assert!(matches!(
            live.entity_payload.value(),
            EntityPayload::Live(_)
        ));
        let cached = tx
            .get::<SyncEntities>(&(encode_key(&(
                "transcript:a".to_owned(),
                "@revision".to_owned(),
            )),))
            .unwrap()
            .unwrap();
        assert!(matches!(
            cached.entity_payload.value(),
            EntityPayload::TranscriptCache(_)
        ));
    }
    #[test]
    fn f1_publication_context_refuses_key_flag_and_scope_mismatches() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Publications::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        let mut row = entity(
            "future",
            "id",
            1,
            1,
            StoredBoolean::Valid(false),
            Some("raw"),
        );
        row.entity_id = "other".into();
        row.identity = encode_key(&("future".to_owned(), "other".to_owned()));
        assert!(tx.partition::<SyncEntities>().put(&row).is_err());
        row.entity_id = "id".into();
        row.identity = encode_key(&("future".to_owned(), "id".to_owned()));
        row.deleted = StoredBoolean::Valid(true);
        assert!(tx.partition::<SyncEntities>().put(&row).is_err());
        let document = SyncDocuments {
            identity: encode_key(&(3_i64,)),
            source_rowid: 3,
            publication_seq: 3,
            deleted: StoredBoolean::Valid(false),
            document_scope: "wrong".into(),
            document_id: "id".into(),
            document_payload: DocumentValue::from_stored("future", "id", "raw".into()).unwrap(),
        };
        assert!(tx.partition::<SyncDocuments>().put(&document).is_err());
        assert!(
            EntityValue::from_stored(
                "transcript:a",
                "item:a",
                StoredBoolean::Valid(false),
                Some("{}".into())
            )
            .is_err()
        );
    }
    #[test]
    fn f1_collection_classification_matches_shipped_sqlite_like_semantics() {
        let db = CodecContext::open_in_memory().unwrap();
        for label in [
            "edge",
            "transcript:a",
            "TRANSCRIPT:A",
            "Transcript:",
            "transcript:a:b",
            "transcript",
            "transcript\0:a",
            "transcript:a\0tail",
            "TRANSCRİPT:a",
            "future\0collection",
            "",
        ] {
            let sql = canvas_read(&db)
                .named(
                    &ShippedTranscriptPredicate::QUERY,
                    &ShippedTranscriptPredicate {
                        label: Some(label.into()),
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                CollectionName::from_stored(label.into()).non_transcript(),
                sql,
                "{label:?}"
            );
        }
    }
}
