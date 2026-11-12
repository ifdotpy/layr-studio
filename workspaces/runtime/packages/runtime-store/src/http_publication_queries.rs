//! Fixed canonical publication queries for HTTP owners.

use crate::agent_tables::AgentsStorageRow;
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::publication_schema::Publications;
use studio_db_api::{CodecContext, DbQuery};

pub struct HttpPublicationQueries;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=Publications),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities_only=rows),
    sql="SELECT {id} FROM {rows} WHERE {entities_only} AND {collection}=?1 AND {deleted}=0 ORDER BY {rowid}")]
struct LiveIds<'a> {
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
    tables(rows=Publications),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD,rowid=Publications::SOURCE_ROWID),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities_only=rows),
    sql="SELECT {id},{payload} FROM {rows} WHERE {entities_only} AND {collection}=?1 AND {deleted}=0 ORDER BY {rowid}")]
struct LiveRows<'a> {
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=Option<String>,
    tables(rows=Publications),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY),predicates(entities_only=rows),
    sql="SELECT CASE WHEN json_valid({payload}) THEN json_extract({payload},'$.value.observation.data.accountId') END FROM {rows} WHERE {entities_only} AND {collection}='rateLimit' AND {id}=?1 AND {deleted}=0")]
struct RateLimitIdentity<'a> {
    #[db(column=Publications::ENTITY_ID)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,Option<String>),
    tables(agents=AgentsStorageRow,rows=Publications),
    columns(id=AgentsStorageRow::ID,record=AgentsStorageRow::RECORD,deleted_at=AgentsStorageRow::DELETED_AT,
        collection=Publications::COLLECTION_ORDER,entity_id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY),predicates(entities_only=rows),
    sql="SELECT a.{id},{payload} FROM {agents} a LEFT JOIN {rows} ON {entities_only} AND {collection}='sessionCost' AND {entity_id}=a.{id} AND {deleted}=0 WHERE a.{deleted_at} IS NULL AND json_extract(a.{record},'$.isLead')=1")]
struct SessionCostSeeds {}

impl HttpPublicationQueries {
    pub fn live_ids(connection: &CodecContext, collection: &str) -> rusqlite::Result<Vec<String>> {
        canvas_read(connection).collect(
            &LiveIds::QUERY,
            &LiveIds {
                collection: Some(collection),
            },
        )
    }

    pub fn live_rows(
        connection: &CodecContext,
        collection: &str,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        canvas_read(connection).collect(
            &LiveRows::QUERY,
            &LiveRows {
                collection: Some(collection),
            },
        )
    }

    pub fn rate_limit_identity(
        connection: &CodecContext,
        id: &str,
    ) -> rusqlite::Result<Option<Option<String>>> {
        canvas_read(connection).named(
            &RateLimitIdentity::QUERY,
            &RateLimitIdentity { id: Some(id) },
        )
    }

    pub fn session_cost_seeds(
        connection: &CodecContext,
    ) -> rusqlite::Result<Vec<(String, Option<String>)>> {
        canvas_read(connection).collect(&SessionCostSeeds::QUERY, &SessionCostSeeds {})
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,bool,String),
    tables(rows=Publications),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities_only=rows),
    sql="SELECT {id},{deleted},{payload} FROM {rows} WHERE {entities_only} AND {collection}=?1 ORDER BY {id}")]
struct TestCollectionRows<'a> {
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[cfg(any(test, feature = "test-support"))]
impl HttpPublicationQueries {
    pub fn test_collection_rows(
        connection: &CodecContext,
        collection: &str,
    ) -> rusqlite::Result<Vec<(String, bool, String)>> {
        canvas_read(connection).collect(
            &TestCollectionRows::QUERY,
            &TestCollectionRows {
                collection: Some(collection),
            },
        )
    }

    pub fn test_seed_entity(
        transaction: &rusqlite::Transaction<'_>,
        collection: &str,
        id: &str,
        rowid: i64,
        deleted: i64,
        payload: Option<&str>,
    ) -> crate::StoreResult<()> {
        use crate::metadata_values::StoredBoolean;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities};
        let name = CollectionName::from_stored(collection.to_owned());
        let deleted = StoredBoolean::from_stored(deleted);
        crate::peer_update_settings::canvas_tx(transaction)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: studio_db_api::migration::encode_key(&(
                    collection.to_owned(),
                    id.to_owned(),
                )),
                source_rowid: rowid,
                publication_seq: rowid,
                deleted,
                collection_kind: name.kind(),
                collection_agent: name.agent().cloned(),
                non_transcript: name.non_transcript(),
                collection_order: name,
                entity_id: id.to_owned(),
                entity_hash: "fixture".into(),
                entity_payload: EntityValue::from_stored(
                    collection,
                    id,
                    deleted,
                    payload.map(str::to_owned),
                )?,
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    fn database() -> CodecContext {
        let db = CodecContext::open_in_memory().unwrap();
        Publications::descriptor().install_if_absent(&db).unwrap();
        db
    }

    #[test]
    fn http_publication_canonical_raw_rows_preserve_selectors_null_and_source_order() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        for (id, rowid, deleted, raw) in [
            ("physical-first", 30, 0, Some(" malformed ")),
            ("source-first", 2, 0, Some(" {\"raw\":1e+02} ")),
            ("deleted", 3, 1, Some("ignored")),
            ("other-integer", 4, 2, Some("ignored")),
        ] {
            HttpPublicationQueries::test_seed_entity(&tx, "model", id, rowid, deleted, raw)
                .unwrap();
        }
        assert_eq!(
            HttpPublicationQueries::live_ids(&tx, "model").unwrap(),
            vec!["source-first", "physical-first"]
        );
        assert_eq!(
            HttpPublicationQueries::live_rows(&tx, "model").unwrap(),
            vec![
                ("source-first".into(), " {\"raw\":1e+02} ".into()),
                ("physical-first".into(), " malformed ".into())
            ]
        );
        assert!(
            HttpPublicationQueries::live_rows(&tx, "missing")
                .unwrap()
                .is_empty()
        );
        HttpPublicationQueries::test_seed_entity(&tx, "model", "null", 5, 0, None).unwrap();
        assert!(HttpPublicationQueries::live_rows(&tx, "model").is_err());
    }

    #[test]
    fn http_publication_canonical_identity_preserves_missing_null_and_first_duplicate() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            HttpPublicationQueries::rate_limit_identity(&tx, "absent").unwrap(),
            None
        );
        for (id, rowid, deleted, raw, expected) in [
            ("null", 1, 0, None, Some(None)),
            ("malformed", 2, 0, Some("{invalid"), Some(None)),
            (
                "duplicate",
                3,
                0,
                Some(
                    r#"{"value":{"observation":{"data":{"accountId":"first","accountId":"last"}}}}"#,
                ),
                Some(Some("first".into())),
            ),
            ("deleted", 4, 1, Some("{}"), None),
            ("integer", 5, 2, Some("{}"), None),
        ] {
            HttpPublicationQueries::test_seed_entity(&tx, "rateLimit", id, rowid, deleted, raw)
                .unwrap();
            assert_eq!(
                HttpPublicationQueries::rate_limit_identity(&tx, id).unwrap(),
                expected,
                "{id}"
            );
        }
        HttpPublicationQueries::test_seed_entity(
            &tx,
            "rateLimit",
            "number",
            6,
            0,
            Some(r#"{"value":{"observation":{"data":{"accountId":17}}}}"#),
        )
        .unwrap();
        assert!(HttpPublicationQueries::rate_limit_identity(&tx, "number").is_err());
    }

    #[test]
    fn http_publication_session_seed_left_join_preserves_nullable_saved_payloads() {
        let mut db = database();
        AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::agent_tables::AgentVolatileStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        for (id, lead, deleted) in [
            ("saved", true, false),
            ("missing", true, false),
            ("null", true, false),
            ("worker", false, false),
            ("deleted", true, true),
        ] {
            let mut value = studio_json::json!({"compactions":0,"accountKey":"default","provider":"codex","id":id,"rootId":id,"epoch":0,"status":"idle","isLead":lead});
            if deleted {
                value["deletedAt"] = studio_json::json!(0);
            }
            let record = crate::AgentRecord::from_value(value).unwrap();
            crate::peer_update_settings::canvas_tx(&tx)
                .insert(&AgentsStorageRow {
                    account_key: String::new(),
                    id: id.into(),
                    root_id: None,
                    deleted_at: None,
                    record: crate::json_record_codec::RawJsonRecord::from_record(&tx, &record)
                        .unwrap(),
                })
                .unwrap();
        }
        HttpPublicationQueries::test_seed_entity(
            &tx,
            "sessionCost",
            "saved",
            1,
            0,
            Some(" {\"raw\":1e+02} "),
        )
        .unwrap();
        HttpPublicationQueries::test_seed_entity(
            &tx,
            "sessionCost",
            "missing",
            2,
            2,
            Some("ignored"),
        )
        .unwrap();
        HttpPublicationQueries::test_seed_entity(&tx, "sessionCost", "null", 3, 0, None).unwrap();
        let mut rows = HttpPublicationQueries::session_cost_seeds(&tx).unwrap();
        rows.sort();
        assert_eq!(
            rows,
            vec![
                ("missing".into(), None),
                ("null".into(), None),
                ("saved".into(), Some(" {\"raw\":1e+02} ".into()))
            ]
        );
        let plan = crate::query_plan_test_support::explain(
            &tx,
            SessionCostSeeds::QUERY.sql(),
            &SessionCostSeeds {},
        )
        .unwrap();
        eprintln!("HTTP canonical plan: {plan:?}");
        assert!(
            plan.iter().any(
                |step| step.contains("publications_entity_identity") && step.contains("SEARCH")
            ),
            "{plan:?}"
        );
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let steps = Arc::new(AtomicUsize::new(0));
        let mut measured = Vec::new();
        for size in [0, 2000] {
            for index in 0..size {
                HttpPublicationQueries::test_seed_entity(
                    &tx,
                    "unrelated",
                    &format!("id-{index}"),
                    index + 10,
                    1,
                    Some("{}"),
                )
                .unwrap();
            }
            steps.store(0, Ordering::Relaxed);
            let count = steps.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    count.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                HttpPublicationQueries::session_cost_seeds(&tx)
                    .unwrap()
                    .len(),
                3
            );
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            measured.push(steps.load(Ordering::Relaxed));
        }
        eprintln!("HTTP canonical VM (0 vs 2000 unrelated rows): {measured:?}");
        assert!(measured[1] <= measured[0] + 20, "session VM {measured:?}");
        assert!(measured[1] < 300, "session VM {measured:?}");
    }

    #[test]
    fn http_publication_hot_reads_use_owned_indexes_and_flat_vm() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = database();
        let tx = db.transaction().unwrap();
        HttpPublicationQueries::test_seed_entity(&tx, "rateLimit", "selected", 1, 0, Some("{}"))
            .unwrap();
        let ids = LiveIds {
            collection: Some("rateLimit"),
        };
        let rows = LiveRows {
            collection: Some("rateLimit"),
        };
        let identity = RateLimitIdentity {
            id: Some("selected"),
        };
        for (sql, plan) in [
            (
                "ids",
                crate::query_plan_test_support::explain(&tx, LiveIds::QUERY.sql(), &ids).unwrap(),
            ),
            (
                "rows",
                crate::query_plan_test_support::explain(&tx, LiveRows::QUERY.sql(), &rows).unwrap(),
            ),
            (
                "identity",
                crate::query_plan_test_support::explain(
                    &tx,
                    RateLimitIdentity::QUERY.sql(),
                    &identity,
                )
                .unwrap(),
            ),
        ] {
            assert!(
                plan.iter()
                    .any(|step| step.contains("SEARCH") && step.contains("publications_entity_")),
                "{sql}: {plan:?}"
            );
        }
        let steps = Arc::new(AtomicUsize::new(0));
        let mut measured = Vec::new();
        for size in [0, 2000] {
            for index in 0..size {
                HttpPublicationQueries::test_seed_entity(
                    &tx,
                    "unrelated",
                    &format!("id-{index}"),
                    index + 2,
                    1,
                    Some("{}"),
                )
                .unwrap();
            }
            steps.store(0, Ordering::Relaxed);
            let count = steps.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    count.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                HttpPublicationQueries::live_ids(&tx, "rateLimit").unwrap(),
                vec!["selected"]
            );
            assert_eq!(
                HttpPublicationQueries::live_rows(&tx, "rateLimit").unwrap(),
                vec![("selected".into(), "{}".into())]
            );
            assert_eq!(
                HttpPublicationQueries::rate_limit_identity(&tx, "selected").unwrap(),
                Some(None)
            );
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            measured.push(steps.load(Ordering::Relaxed));
        }
        eprintln!("HTTP canonical VM (0 vs 2000 unrelated rows): {measured:?}");
        assert!(measured[1] <= measured[0] + 20, "VM {measured:?}");
        assert!(measured[1] < 300, "VM {measured:?}");
    }
}
