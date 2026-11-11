//! Fixed canonical projection and account publication queries.
use crate::consolidation_kinds::{
    CanvasMetadataKind as MetaKind, CanvasPublicationsKind as PubKind,
};
use crate::metadata_schema::Metadata;
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use crate::planning_work::ProjectRow;
use crate::publication_schema::Publications;
use studio_db_api::{DbQuery, db_query};

pub struct ProjectionPublicationQueries;

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=optional, native, result=bool,
 tables(rows=Metadata), columns(kind=Metadata::KIND,key=Metadata::TEXT_KEY,record=Metadata::FEDERATION_RECORD),
 sql="SELECT COALESCE(json_extract({record},'$.enabled'),0) FROM {rows} WHERE {kind}=?1 AND {key}=?2")]
struct FederationEnabled<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    key: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=optional, native, result=(i64,bool),
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,deleted=Publications::DELETED),
 sql="SELECT {seq},{deleted} FROM {rows} WHERE {entities} AND {kind}=?1 AND {collection}=?2 AND {id}=?3")]
struct EntityVersion<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=optional, native, result=Option<String>,
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
 sql="SELECT json_extract(j.value,'$.accountId') FROM {rows}, json_each(CASE WHEN json_valid({payload}) THEN {payload} ELSE json_object() END,'$.value.accounts') j WHERE {entities} AND {kind}=?1 AND {collection}=?2 AND {id}=?3 AND {deleted}=0 AND json_extract(j.value,'$.id')=?4 ORDER BY {rowid},j.id LIMIT 1")]
struct AccountIdentity<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    id: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    account: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=many, native, result=(String,Option<Vec<u8>>),
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
 sql="SELECT {id},substr(CAST({payload} AS BLOB),1,?1) FROM {rows} WHERE {entities} AND {kind}=?2 AND {collection}=?3 AND {deleted}=0 ORDER BY {rowid}")]
struct LimitsViews<'a> {
    #[db(column=Publications::SOURCE_ROWID)]
    bytes: i64,
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=many, native, result=String,
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
 sql="SELECT {id} FROM {rows} WHERE {entities} AND {kind}=?1 AND {collection}=?2 AND {deleted}=0 AND substr({id},1,length(?3))=?3 ORDER BY {rowid}")]
struct PeerProjects<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    prefix: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=many, native, result=Option<String>,
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT DISTINCT json_extract({payload},'$.value.homeServerId') FROM {rows} WHERE {entities} AND {kind}=?1 AND {collection}=?2 AND {deleted}=0 AND substr({id},1,?3)=?4")]
struct PeerServers<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::SOURCE_ROWID)]
    prefix_length: i64,
    #[db(column=Publications::ENTITY_ID)]
    prefix: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=many, native, result=String,
 tables(rows=Publications), indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED), predicates(entities=rows), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,rowid=Publications::SOURCE_ROWID),
 sql="SELECT {id} FROM {rows} WHERE {entities} AND {kind}=?1 AND {collection}=?2 AND json_extract({payload},'$.value.projectPath')=?3 ORDER BY {rowid}")]
struct ProjectTeams<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    path: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas, mode=read, cardinality=optional, native, result=i64,
 tables(rows=Publications,projects=ProjectRow), columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,deleted=Publications::DELETED,project=ProjectRow::ID),
 sql="SELECT s.{seq} FROM {rows} s JOIN {projects} p ON p.{project}=s.{id} WHERE s.{kind}=?1 AND s.{collection}=?2 AND s.{id}=?3 AND s.{deleted}=0")]
struct LocationSequence<'a> {
    #[db(column=Publications::KIND)]
    kind: PubKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    id: Option<&'a str>,
}

impl ProjectionPublicationQueries {
    pub fn federation_enabled(db: &rusqlite::Connection) -> rusqlite::Result<Option<bool>> {
        canvas_read(db).named(
            &FederationEnabled::QUERY,
            &FederationEnabled {
                kind: MetaKind::FederationSettings,
                key: Some("global"),
            },
        )
    }
    pub fn raw_payload(
        db: &rusqlite::Connection,
        collection: &str,
        id: &str,
        live_only: bool,
    ) -> crate::StoreResult<Option<String>> {
        crate::repositories::sync_documents::publication_payload(db, collection, id, live_only)
    }
    pub fn publication_sequence(
        db: &rusqlite::Connection,
        collection: &str,
        id: &str,
        deleted: bool,
    ) -> crate::StoreResult<Option<i64>> {
        crate::repositories::sync_documents::publication_sequence(db, collection, id, deleted)
    }
    pub fn entity_version(
        db: &rusqlite::Connection,
        collection: &str,
        id: &str,
    ) -> rusqlite::Result<Option<(i64, bool)>> {
        canvas_read(db).named(
            &EntityVersion::QUERY,
            &EntityVersion {
                kind: PubKind::SyncEntities,
                collection: Some(collection),
                id: Some(id),
            },
        )
    }
    pub fn current_account_identity(
        db: &rusqlite::Connection,
        account: &str,
    ) -> rusqlite::Result<Option<String>> {
        Ok(canvas_read(db)
            .named(
                &AccountIdentity::QUERY,
                &AccountIdentity {
                    kind: PubKind::SyncEntities,
                    collection: Some("accounts"),
                    id: Some("current"),
                    account: Some(account),
                },
            )?
            .flatten())
    }
    pub fn visit_limits(
        db: &rusqlite::Connection,
        bytes: i64,
        visitor: impl FnMut((String, Option<Vec<u8>>)) -> rusqlite::Result<()>,
    ) -> rusqlite::Result<()> {
        canvas_read(db).visit(
            &LimitsViews::QUERY,
            &LimitsViews {
                bytes,
                kind: PubKind::SyncEntities,
                collection: Some("rateLimit"),
            },
            visitor,
        )
    }
    pub fn delete_entity(
        db: &rusqlite::Transaction<'_>,
        collection: &str,
        id: &str,
    ) -> rusqlite::Result<()> {
        let identity =
            studio_db_api::migration::encode_key(&(collection.to_owned(), id.to_owned()));
        canvas_tx(db)
            .partition::<crate::publication_schema::SyncEntities>()
            .delete(&(identity,))?;
        Ok(())
    }
    pub fn meta_exists(db: &rusqlite::Connection, key: &str) -> rusqlite::Result<bool> {
        let kind = MetaKind::SyncEntityMeta;
        let key = Some(key.to_owned());
        db_query!(
            canvas_read(db),
            Metadata,
            exists(),
            filter(Metadata::KIND.eq(&kind), Metadata::TEXT_KEY.eq(&key))
        )
    }
    pub fn peer_project_ids(
        db: &rusqlite::Connection,
        prefix: &str,
    ) -> rusqlite::Result<Vec<String>> {
        canvas_read(db).collect(
            &PeerProjects::QUERY,
            &PeerProjects {
                kind: PubKind::SyncEntities,
                collection: Some("project"),
                prefix: Some(prefix),
            },
        )
    }
    pub fn peer_servers(
        db: &rusqlite::Connection,
        prefix: &str,
        prefix_length: i64,
    ) -> rusqlite::Result<Vec<Option<String>>> {
        canvas_read(db).collect(
            &PeerServers::QUERY,
            &PeerServers {
                kind: PubKind::SyncEntities,
                collection: Some("project"),
                prefix: Some(prefix),
                prefix_length,
            },
        )
    }
    pub fn project_team_ids(
        db: &rusqlite::Connection,
        path: &str,
    ) -> rusqlite::Result<Vec<String>> {
        canvas_read(db).collect(
            &ProjectTeams::QUERY,
            &ProjectTeams {
                kind: PubKind::SyncEntities,
                collection: Some("peerTeam"),
                path: Some(path),
            },
        )
    }
    pub fn location_sequence(db: &rusqlite::Connection, id: &str) -> rusqlite::Result<Option<i64>> {
        canvas_read(db).named(
            &LocationSequence::QUERY,
            &LocationSequence {
                kind: PubKind::SyncEntities,
                collection: Some("project"),
                id: Some(id),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_values::StoredBoolean;
    use crate::peer_update_settings::canvas_tx;
    use crate::publication_collection::CollectionName;
    use crate::publication_schema::{EntityValue, SyncEntities};
    use studio_db_api::{CodecContext, Table};

    pub(super) fn database() -> CodecContext {
        let db = CodecContext::open_in_memory().unwrap();
        Publications::descriptor().install_if_absent(&db).unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        ProjectRow::descriptor().install_if_absent(&db).unwrap();
        db
    }
    pub(super) fn entity(
        tx: &rusqlite::Transaction<'_>,
        collection: &str,
        id: &str,
        rowid: i64,
        deleted: i64,
        raw: Option<&str>,
    ) {
        let name = CollectionName::from_stored(collection.into());
        let flag = StoredBoolean::from_stored(deleted);
        canvas_tx(tx)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: studio_db_api::migration::encode_key(&(
                    collection.to_owned(),
                    id.to_owned(),
                )),
                source_rowid: rowid,
                publication_seq: rowid,
                deleted: flag,
                collection_kind: name.kind(),
                collection_agent: name.agent().cloned(),
                non_transcript: name.non_transcript(),
                collection_order: name,
                entity_id: id.into(),
                entity_hash: "0".repeat(64),
                entity_payload: EntityValue::from_stored(
                    collection,
                    id,
                    flag,
                    raw.map(str::to_owned),
                )
                .unwrap(),
            })
            .unwrap();
    }
    #[test]
    fn projection_canonical_payload_sequence_and_version_keep_null_deleted_and_raw_bytes() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            ProjectionPublicationQueries::raw_payload(&tx, "workspace", "missing", true).unwrap(),
            None
        );
        let raw = " \n{\"value\":{\"future\":1.00}}\t";
        for (id, rowid, deleted, payload) in [
            ("raw", 1, 0, Some(raw)),
            ("null", 2, 0, None),
            ("dead", 3, 1, Some(" malformed ")),
            ("two", 4, 2, Some(" wrong ")),
        ] {
            entity(&tx, "workspace", id, rowid, deleted, payload);
            assert_eq!(
                ProjectionPublicationQueries::entity_version(&tx, "workspace", id).unwrap(),
                Some((rowid, deleted != 0))
            );
            assert_eq!(
                ProjectionPublicationQueries::publication_sequence(&tx, "workspace", id, false)
                    .unwrap(),
                (deleted == 0).then_some(rowid)
            );
            assert_eq!(
                ProjectionPublicationQueries::publication_sequence(&tx, "workspace", id, true)
                    .unwrap(),
                (deleted == 1).then_some(rowid)
            );
        }
        assert_eq!(
            ProjectionPublicationQueries::raw_payload(&tx, "workspace", "raw", true).unwrap(),
            Some(raw.into())
        );
        assert!(ProjectionPublicationQueries::raw_payload(&tx, "workspace", "null", true).is_err());
        assert_eq!(
            ProjectionPublicationQueries::raw_payload(&tx, "workspace", "dead", true).unwrap(),
            None
        );
        assert_eq!(
            ProjectionPublicationQueries::raw_payload(&tx, "workspace", "dead", false).unwrap(),
            Some(" malformed ".into())
        );
        ProjectionPublicationQueries::delete_entity(&tx, "workspace", "null").unwrap();
        assert_eq!(
            ProjectionPublicationQueries::entity_version(&tx, "workspace", "null").unwrap(),
            None
        );
        tx.rollback().unwrap();
        assert_eq!(
            ProjectionPublicationQueries::entity_version(&db, "workspace", "raw").unwrap(),
            None
        );
    }
    #[test]
    fn projection_canonical_limits_stream_preserves_null_bytes_and_source_order() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(&tx, "rateLimit", "physical-first", 30, 0, Some("éabc"));
        entity(&tx, "rateLimit", "source-first", -7, 0, None);
        entity(&tx, "rateLimit", "dead", 40, 1, Some("ignored"));
        entity(&tx, "workspace", "other", 50, 0, Some("ignored"));
        let mut rows = Vec::new();
        ProjectionPublicationQueries::visit_limits(&tx, 3, |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rows,
            vec![
                ("source-first".into(), None),
                ("physical-first".into(), Some(vec![0xc3, 0xa9, b'a']))
            ]
        );
    }
    #[test]
    fn projection_canonical_account_identity_keeps_first_duplicate_null_and_invalid_json() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            ProjectionPublicationQueries::current_account_identity(&tx, "a").unwrap(),
            None
        );
        for raw in [
            "not JSON",
            r#"{"value":{"accounts":[{"id":"a","accountId":null},{"id":"a","accountId":"later"}]}}"#,
        ] {
            entity(&tx, "accounts", "current", 1, 0, Some(raw));
            assert_eq!(
                ProjectionPublicationQueries::current_account_identity(&tx, "a").unwrap(),
                None
            );
        }
        entity(
            &tx,
            "accounts",
            "current",
            1,
            0,
            Some(
                r#"{"value":{"accounts":[{"id":"a","accountId":"first","accountId":"last"},{"id":"a","accountId":"later"}]}}"#,
            ),
        );
        assert_eq!(
            ProjectionPublicationQueries::current_account_identity(&tx, "a").unwrap(),
            Some("first".into())
        );
    }
    #[test]
    fn projection_canonical_peer_scans_and_team_membership_keep_selectors() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        for (id, rowid, deleted, raw) in [
            ("peer:s:a", 30, 0, r#"{"value":{"homeServerId":"s"}}"#),
            ("peer:s:b", -7, 0, r#"{"value":{"homeServerId":null}}"#),
            ("peer:t:a", 40, 0, r#"{"value":{"homeServerId":"t"}}"#),
            ("peer:s:dead", 50, 1, "malformed"),
        ] {
            entity(&tx, "project", id, rowid, deleted, Some(raw));
        }
        assert_eq!(
            ProjectionPublicationQueries::peer_project_ids(&tx, "peer:s:").unwrap(),
            vec!["peer:s:b", "peer:s:a"]
        );
        let servers = ProjectionPublicationQueries::peer_servers(&tx, "peer:", 5).unwrap();
        assert_eq!(
            servers
                .into_iter()
                .flatten()
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["s".into(), "t".into()])
        );
        entity(
            &tx,
            "peerTeam",
            "live",
            1,
            0,
            Some(r#"{"value":{"projectPath":"p"}}"#),
        );
        entity(
            &tx,
            "peerTeam",
            "dead",
            2,
            1,
            Some(r#"{"value":{"projectPath":"p"}}"#),
        );
        entity(
            &tx,
            "peerTeam",
            "null",
            3,
            0,
            Some(r#"{"value":{"projectPath":null}}"#),
        );
        assert_eq!(
            ProjectionPublicationQueries::project_team_ids(&tx, "p").unwrap(),
            vec!["live", "dead"]
        );
    }
    #[test]
    fn projection_canonical_hot_limits_plan_and_vm_ignore_retained_history() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(&tx, "rateLimit", "live", 1, 0, Some("payload"));
        let params = LimitsViews {
            bytes: 10,
            kind: PubKind::SyncEntities,
            collection: Some("rateLimit"),
        };
        let plan = crate::query_plan_test_support::explain(&tx, LimitsViews::QUERY.sql(), &params)
            .unwrap();
        assert!(
            plan.iter().any(|step| step.contains(
                studio_db_api::query_index::<Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED
                )
            )),
            "{plan:?}"
        );
        let count = Arc::new(AtomicUsize::new(0));
        let measure = || {
            let counter = count.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            ProjectionPublicationQueries::visit_limits(&tx, 10, |_| Ok(())).unwrap();
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            count.swap(0, Ordering::Relaxed)
        };
        let before = measure();
        for index in 2..2002 {
            entity(
                &tx,
                "rateLimit",
                &format!("dead-{index}"),
                index,
                1,
                Some("ignored"),
            );
        }
        let after = measure();
        eprintln!("projection limits VM: {before}->{after}");
        assert!(after <= before + 10, "VM steps {before}->{after}");
        assert!(after < 200, "VM bound {after}");
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    use crate::metadata_schema::{FederationSettings, StoredSyncMetaValue, SyncEntityMeta};
    use crate::peer_update_settings::canvas_tx;
    use studio_db_api::{CodecContext, Table};
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(rows=Metadata),columns(kind=Metadata::KIND,record=Metadata::FEDERATION_RECORD),sql="UPDATE {rows} SET {record}=?1 WHERE {kind}=?2")]
    struct FederationRaw<'a> {
        #[db(column=Metadata::FEDERATION_RECORD)]
        raw: Option<&'a str>,
        #[db(column=Metadata::KIND)]
        kind: MetaKind,
    }
    #[test]
    fn projection_canonical_federation_and_marker_preserve_missing_and_integer_bool() {
        let mut db = CodecContext::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let tx = db.transaction().unwrap();
        assert_eq!(
            ProjectionPublicationQueries::federation_enabled(&tx).unwrap(),
            None
        );
        assert!(!ProjectionPublicationQueries::meta_exists(&tx, "task_window_migrated").unwrap());
        let key = Some("global".to_owned());
        canvas_tx(&tx)
            .partition::<FederationSettings>()
            .put(&FederationSettings {
                identity: crate::consolidation_codec::nullable_text_identity(&key, 1),
                source_rowid: 1,
                text_key: key.clone(),
                federation_record: crate::json_record_codec::ExactJson::from_stored(
                    "{\"enabled\":true}".into(),
                )
                .unwrap(),
            })
            .unwrap();
        for (raw, expected) in [
            ("{}", false),
            ("{\"enabled\":null}", false),
            ("{\"enabled\":0}", false),
            ("{\"enabled\":2}", true),
            ("{\"enabled\":1,\"enabled\":0}", true),
        ] {
            canvas_tx(&tx)
                .named(
                    &FederationRaw::QUERY,
                    &FederationRaw {
                        raw: Some(raw),
                        kind: MetaKind::FederationSettings,
                    },
                )
                .unwrap();
            assert_eq!(
                ProjectionPublicationQueries::federation_enabled(&tx).unwrap(),
                Some(expected)
            );
        }
        for raw in ["malformed", "{\"enabled\":\"yes\"}"] {
            canvas_tx(&tx)
                .named(
                    &FederationRaw::QUERY,
                    &FederationRaw {
                        raw: Some(raw),
                        kind: MetaKind::FederationSettings,
                    },
                )
                .unwrap();
            assert!(ProjectionPublicationQueries::federation_enabled(&tx).is_err());
        }
        let key = Some("task_window_migrated".to_owned());
        canvas_tx(&tx)
            .partition::<SyncEntityMeta>()
            .put(&SyncEntityMeta {
                identity: crate::consolidation_codec::nullable_text_identity(&key, 2),
                source_rowid: 2,
                text_key: key.clone(),
                meta_value: StoredSyncMetaValue::from_stored(key, "0".into()),
            })
            .unwrap();
        assert!(ProjectionPublicationQueries::meta_exists(&tx, "task_window_migrated").unwrap());
    }
}

#[cfg(test)]
mod hot_query_tests {
    use super::*;
    #[test]
    fn projection_canonical_hot_peer_and_point_queries_ignore_retained_history() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = super::tests::database();
        let tx = db.transaction().unwrap();
        super::tests::entity(
            &tx,
            "project",
            "peer:s:a",
            1,
            0,
            Some(r#"{"value":{"homeServerId":"s"}}"#),
        );
        let plans = [
            crate::query_plan_test_support::explain(
                &tx,
                PeerProjects::QUERY.sql(),
                &PeerProjects {
                    kind: PubKind::SyncEntities,
                    collection: Some("project"),
                    prefix: Some("peer:"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                PeerServers::QUERY.sql(),
                &PeerServers {
                    kind: PubKind::SyncEntities,
                    collection: Some("project"),
                    prefix: Some("peer:"),
                    prefix_length: 5,
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                EntityVersion::QUERY.sql(),
                &EntityVersion {
                    kind: PubKind::SyncEntities,
                    collection: Some("project"),
                    id: Some("peer:s:a"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                LocationSequence::QUERY.sql(),
                &LocationSequence {
                    kind: PubKind::SyncEntities,
                    collection: Some("project"),
                    id: Some("peer:s:a"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                AccountIdentity::QUERY.sql(),
                &AccountIdentity {
                    kind: PubKind::SyncEntities,
                    collection: Some("accounts"),
                    id: Some("current"),
                    account: Some("default"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                ProjectTeams::QUERY.sql(),
                &ProjectTeams {
                    kind: PubKind::SyncEntities,
                    collection: Some("peerTeam"),
                    path: Some("p"),
                },
            )
            .unwrap(),
        ];
        for plan in plans {
            assert!(plan.iter().any(|step| step.contains("SEARCH")), "{plan:?}");
            eprintln!("projection plan: {plan:?}");
        }
        let steps = Arc::new(AtomicUsize::new(0));
        let measure = || {
            let counter = steps.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                ProjectionPublicationQueries::peer_project_ids(&tx, "peer:").unwrap(),
                vec!["peer:s:a"]
            );
            assert_eq!(
                ProjectionPublicationQueries::peer_servers(&tx, "peer:", 5).unwrap(),
                vec![Some("s".into())]
            );
            assert_eq!(
                ProjectionPublicationQueries::entity_version(&tx, "project", "peer:s:a").unwrap(),
                Some((1, false))
            );
            assert_eq!(
                ProjectionPublicationQueries::location_sequence(&tx, "peer:s:a").unwrap(),
                None
            );
            assert_eq!(
                ProjectionPublicationQueries::current_account_identity(&tx, "default").unwrap(),
                None
            );
            assert!(
                ProjectionPublicationQueries::project_team_ids(&tx, "p")
                    .unwrap()
                    .is_empty()
            );
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            steps.swap(0, Ordering::Relaxed)
        };
        let before = measure();
        for index in 2..2002 {
            super::tests::entity(
                &tx,
                "project",
                &format!("dead-{index}"),
                index,
                1,
                Some("ignored"),
            );
        }
        let after = measure();
        eprintln!("projection peer/point VM: {before}->{after}");
        assert!(after <= before + 20, "VM steps {before}->{after}");
        assert!(after < 800, "VM bound {after}");
    }
}
