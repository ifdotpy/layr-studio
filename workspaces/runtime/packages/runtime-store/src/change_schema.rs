//! The three logical change streams share one physical table; no active routing yet.
use crate::consolidation_codec::{Required, RequiredCodec, nullable_text_identity};
use crate::consolidation_kinds::CanvasChangesKind as Kind;
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::metadata_values::{StoredBoolean, StoredBooleanCodec};
use crate::peer_update_settings::Canvas;
use studio_db_api::{Codec, CodecContext, CodecError, DbPartition, DbTable};

/// Open durable labels retain exact comparison text. Registered names reference
/// the existing trigger registry; window labels reference their existing owner.
pub(crate) enum DurableSource {
    Registered(&'static crate::durable_seq::DurableTable),
    TaskWindow,
    MonitorWindow,
    Other(String),
}
impl DurableSource {
    pub fn from_stored(name: String) -> Self {
        if let Some(source) = crate::durable_seq::DURABLE_TABLES
            .iter()
            .find(|source| source.name == name)
        {
            return Self::Registered(source);
        }
        match name.as_str() {
            crate::durable_seq::TASK_WINDOW => Self::TaskWindow,
            crate::durable_seq::MONITOR_WINDOW => Self::MonitorWindow,
            _ => Self::Other(name),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Self::Registered(source) => source.name,
            Self::TaskWindow => crate::durable_seq::TASK_WINDOW,
            Self::MonitorWindow => crate::durable_seq::MONITOR_WINDOW,
            Self::Other(name) => name,
        }
    }
}
pub(crate) struct DurableSourceCodec;
impl Codec<DurableSource> for DurableSourceCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(_: &CodecContext, value: &'a DurableSource) -> Result<&'a str, CodecError> {
        Ok(value.as_str())
    }
    fn decode(_: &CodecContext, value: String) -> Result<DurableSource, CodecError> {
        Ok(DurableSource::from_stored(value))
    }
}

const DURABLE_TOMBSTONES: &str =
    studio_db_api::static_sql!("kind=", Kind::DB_CODE_DURABLEROWS, " AND deleted=1");
const DURABLE_TASK_WINDOW: &str = studio_db_api::static_sql!(
    "kind=",
    Kind::DB_CODE_DURABLEROWS,
    " AND source_label='",
    crate::durable_seq::TASK_WINDOW,
    "' AND deleted=0"
);

const DURABLE_SHAPE: &str = studio_db_api::static_sql!(
    "(kind=",
    Kind::DB_CODE_DURABLEROWS,
    " AND original_key IS NULL AND source_label IS NOT NULL AND durable_id IS NOT NULL AND sequence IS NOT NULL AND deleted IS NOT NULL AND deleted_at IS NOT NULL) OR (kind<>",
    Kind::DB_CODE_DURABLEROWS,
    " AND source_label IS NULL AND durable_id IS NULL AND sequence IS NULL AND deleted IS NULL AND deleted_at IS NULL AND scope IS NULL)"
);
const SCHEDULER_SHAPE: &str = studio_db_api::static_sql!(
    "(kind=",
    Kind::DB_CODE_SCHEDULERAGENTCHANGES,
    " AND generation IS NOT NULL AND generation>=0) OR (kind<>",
    Kind::DB_CODE_SCHEDULERAGENTCHANGES,
    " AND generation IS NULL)"
);
const TRANSCRIPT_SHAPE: &str = studio_db_api::static_sql!(
    "(kind=",
    Kind::DB_CODE_TRANSCRIPTREVISIONS,
    " AND revision IS NOT NULL) OR (kind<>",
    Kind::DB_CODE_TRANSCRIPTREVISIONS,
    " AND revision IS NULL)"
);

#[derive(DbTable)]
#[db(table="changes",database=Canvas,partitioned_by=Kind,
index(name="changes_source_rowid",columns(kind,source_rowid),unique),
index(name="changes_durable_identity",columns(source_label,durable_id),unique,predicate=kind.eq(Kind::DurableRows)),
index(name="changes_durable_window_members",columns(source_label,deleted,durable_id),predicate=kind.eq(Kind::DurableRows)),
index(name="changes_durable_label_sequence",columns(source_label,sequence,source_rowid),predicate=kind.eq(Kind::DurableRows)),
index(name="changes_durable_sequence",columns(sequence,source_rowid),predicate=kind.eq(Kind::DurableRows)),
index(name="changes_durable_tombstone_at",columns(deleted_at),predicate=DURABLE_TOMBSTONES),
index(name="changes_durable_task_window",columns(durable_id),predicate=DURABLE_TASK_WINDOW),
index(name="changes_scheduler_generation",columns(generation,source_rowid),predicate=kind.eq(Kind::SchedulerAgentChanges)),
index(name="changes_scheduler_key",columns(original_key),unique,predicate=kind.eq(Kind::SchedulerAgentChanges)),
index(name="changes_transcript_key",columns(original_key),unique,predicate=kind.eq(Kind::TranscriptRevisions)) )]
pub(crate) struct Changes {
    #[db(key)]
    kind: Kind,
    #[db(key)]
    identity: Vec<u8>,
    source_rowid: i64,
    original_key: Option<String>,
    #[db(check=DURABLE_SHAPE)]
    source_label: Option<String>,
    durable_id: Option<String>,
    sequence: Option<i64>,
    deleted: Option<i64>,
    deleted_at: Option<i64>,
    #[db(codec=ExactJsonCodec<crate::entity_scopes::DeletedScope>)]
    scope: Option<ExactJson<crate::entity_scopes::DeletedScope>>,
    #[db(check=SCHEDULER_SHAPE)]
    generation: Option<i64>,
    #[db(check=TRANSCRIPT_SHAPE)]
    revision: Option<i64>,
}

#[derive(DbPartition)]
#[db(of=Changes,kind=Kind::DurableRows,check=check_durable_key)]
pub(crate) struct DurableRows {
    #[db(key)]
    pub identity: Vec<u8>,
    pub source_rowid: i64,
    #[db(codec=RequiredCodec<DurableSourceCodec, DurableSource>)]
    pub source_label: DurableSource,
    #[db(codec=Required<String>)]
    pub durable_id: String,
    #[db(codec=Required<i64>)]
    pub sequence: i64,
    #[db(codec=RequiredCodec<StoredBooleanCodec, StoredBoolean>)]
    pub deleted: StoredBoolean,
    #[db(codec=Required<i64>)]
    pub deleted_at: i64,
    #[db(codec=ExactJsonCodec<crate::entity_scopes::DeletedScope>)]
    pub scope: Option<ExactJson<crate::entity_scopes::DeletedScope>>,
}

fn check_durable_key(row: &DurableRows) -> crate::StoreResult<()> {
    let expected = studio_db_api::migration::encode_key(&(
        row.source_label.as_str().to_owned(),
        row.durable_id.clone(),
    ));
    if row.identity != expected {
        return Err(crate::StoreError::RecordContract(
            "durable change identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Changes,kind=Kind::SchedulerAgentChanges,check=check_scheduler_key)]
pub(crate) struct SchedulerAgentChanges {
    #[db(key)]
    pub identity: Vec<u8>,
    pub source_rowid: i64,
    pub original_key: Option<String>,
    #[db(codec=Required<i64>)]
    pub generation: i64,
}

fn check_scheduler_key(row: &SchedulerAgentChanges) -> crate::StoreResult<()> {
    check_key(&row.identity, &row.original_key, row.source_rowid)
}

fn check_key(identity: &[u8], original: &Option<String>, rowid: i64) -> crate::StoreResult<()> {
    if identity != nullable_text_identity(original, rowid) {
        return Err(crate::StoreError::RecordContract(
            "change identity disagrees with original key",
        ));
    }
    Ok(())
}

#[derive(DbPartition)]
#[db(of=Changes,kind=Kind::TranscriptRevisions,check=check_transcript_key)]
pub(crate) struct TranscriptRevisions {
    #[db(key)]
    pub identity: Vec<u8>,
    pub source_rowid: i64,
    pub original_key: Option<String>,
    #[db(codec=Required<i64>)]
    pub revision: i64,
}

fn check_transcript_key(row: &TranscriptRevisions) -> crate::StoreResult<()> {
    check_key(&row.identity, &row.original_key, row.source_rowid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_update_settings::canvas_tx;
    use studio_db_api::migration::encode_key;
    use studio_db_api::{CodecContext, Table};

    #[test]
    fn f1_change_partitions_preserve_null_multiplicity_and_kind_isolation() {
        assert_eq!(
            Changes::INDEXES.len(),
            10,
            "the physical owner installs every changes index"
        );
        let mut db = CodecContext::open_in_memory().unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        for source_rowid in [-7, 3] {
            let identity = encode_key(&(None::<String>, Some(source_rowid)));
            tx.partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity: identity.clone(),
                    source_rowid,
                    original_key: None,
                    generation: 11,
                })
                .unwrap();
            tx.partition::<TranscriptRevisions>()
                .put(&TranscriptRevisions {
                    identity: identity.clone(),
                    source_rowid,
                    original_key: None,
                    revision: 29,
                })
                .unwrap();
            assert_eq!(
                tx.get::<SchedulerAgentChanges>(&(identity.clone(),))
                    .unwrap()
                    .unwrap()
                    .generation,
                11
            );
            assert_eq!(
                tx.get::<TranscriptRevisions>(&(identity,))
                    .unwrap()
                    .unwrap()
                    .revision,
                29
            );
        }
        let key = encode_key(&(None::<String>, Some(-7_i64)));
        tx.delete::<SchedulerAgentChanges>(&(key.clone(),)).unwrap();
        assert!(tx.get::<TranscriptRevisions>(&(key,)).unwrap().is_some());
        let other = encode_key(&(None::<String>, Some(3_i64)));
        assert_eq!(
            tx.get::<SchedulerAgentChanges>(&(other,))
                .unwrap()
                .unwrap()
                .source_rowid,
            3
        );
        assert!(
            tx.partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity: nullable_text_identity(&Some("agent".into()), 5),
                    source_rowid: 5,
                    original_key: Some("agent".into()),
                    generation: -1,
                })
                .is_err()
        );
    }
    #[test]
    fn f1_nullable_change_identity_preserves_key_and_signed_tie() {
        let key = Some("\0é".to_owned());
        assert_eq!(
            nullable_text_identity(&key, -7),
            nullable_text_identity(&key, 3)
        );
        assert_ne!(
            nullable_text_identity(&None, -7),
            nullable_text_identity(&None, 3)
        );
        assert_ne!(
            nullable_text_identity(&None, 3),
            nullable_text_identity(&Some(String::new()), 3)
        );
        let mut db = CodecContext::open_in_memory().unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        let identity = nullable_text_identity(&key, -7);
        for source_rowid in [-7, 3] {
            tx.partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity: identity.clone(),
                    source_rowid,
                    original_key: key.clone(),
                    generation: 11,
                })
                .unwrap();
        }
        assert_eq!(
            tx.get::<SchedulerAgentChanges>(&(identity.clone(),))
                .unwrap()
                .unwrap()
                .source_rowid,
            3
        );
        assert!(
            tx.partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity,
                    source_rowid: 3,
                    original_key: None,
                    generation: 11,
                })
                .is_err()
        );
    }
    #[test]
    fn f1_durable_partition_preserves_scope_and_open_source_labels() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        let raw = "{ \"agent\":\"a\", \"future\": [1,2] }";
        for (source_rowid, source) in [-7_i64, 3, 5].into_iter().zip([
            crate::durable_seq::DURABLE_TABLES[0].name,
            crate::durable_seq::TASK_WINDOW,
            "historical\0source",
        ]) {
            let identity = encode_key(&(source.to_owned(), "id".to_owned()));
            tx.partition::<DurableRows>()
                .put(&DurableRows {
                    identity: identity.clone(),
                    source_rowid,
                    source_label: DurableSource::from_stored(source.into()),
                    durable_id: "id".into(),
                    sequence: 17,
                    deleted: StoredBoolean::LegacyInteger(-9),
                    deleted_at: 23,
                    scope: Some(ExactJson::from_stored(raw.into()).unwrap()),
                })
                .unwrap();
            let stored = tx.get::<DurableRows>(&(identity,)).unwrap().unwrap();
            assert_eq!(stored.source_label.as_str(), source);
            assert_eq!(stored.source_rowid, source_rowid);
            assert_eq!(stored.deleted.as_stored(), -9);
            assert_eq!(stored.scope.unwrap().raw(), raw);
        }
    }
    #[test]
    fn f1_change_order_retains_nonnull_secondary_ties_and_dirty_normalization() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        for (id, source_rowid) in [("a", 17), ("z", -3)] {
            let original_key = Some(id.to_owned());
            let identity = nullable_text_identity(&original_key, source_rowid);
            tx.partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity: identity.clone(),
                    source_rowid,
                    original_key: original_key.clone(),
                    generation: 19,
                })
                .unwrap();
            let dirty = studio_db_api::migration::DirtyKey::Composite(encode_key(&(
                original_key.clone(),
                source_rowid,
            )));
            let (key, tie) = dirty.decode::<(Option<String>, i64)>().unwrap();
            assert_eq!(nullable_text_identity(&key, tie), identity);
            assert_eq!(nullable_text_identity(&key, tie + 1), identity);
        }
        let rows = studio_db_api::db_query!(
            tx,
            SchedulerAgentChanges,
            filter(SchedulerAgentChanges::GENERATION.eq(&19_i64)),
            order_by(
                SchedulerAgentChanges::GENERATION.asc,
                SchedulerAgentChanges::SOURCE_ROWID.asc
            )
        )
        .unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row.original_key.as_deref())
                .collect::<Vec<_>>(),
            [Some("z"), Some("a")]
        );
        for source_rowid in [-7_i64, 0, 3] {
            let dirty = studio_db_api::migration::DirtyKey::Composite(encode_key(&(
                None::<String>,
                source_rowid,
            )));
            let (key, tie) = dirty.decode::<(Option<String>, i64)>().unwrap();
            assert_eq!(
                nullable_text_identity(&key, tie),
                nullable_text_identity(&None, source_rowid)
            );
            assert_ne!(
                nullable_text_identity(&key, tie),
                nullable_text_identity(&None, source_rowid + 1)
            );
        }
        assert!(
            studio_db_api::migration::DirtyKey::Composite(Vec::new())
                .decode::<(Option<String>, i64)>()
                .is_err()
        );
    }
}
