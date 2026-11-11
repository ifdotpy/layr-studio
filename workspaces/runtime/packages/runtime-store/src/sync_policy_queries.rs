//! Fixed typed storage queries used by the sync publication policy.

use crate::agent_tables::AgentsStorageRow;
use crate::change_schema::Changes;
use crate::consolidation_codec::nullable_text_identity;
use crate::consolidation_kinds::CanvasPublicationsKind as PublicationKind;
use crate::consolidation_kinds::{CanvasChangesKind as ChangeKind, CanvasMetadataKind as MetaKind};
use crate::metadata_schema::{Metadata, StoredSyncMetaValue, SyncEntityMeta};
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use crate::publication_schema::Publications;
use studio_db_api::{DbProjection, DbQuery};

/// Storage boundary for the existing sync policy business queries.
pub struct SyncPolicyQueries;

#[derive(DbProjection)]
#[db(of=Metadata)]
struct RawTeamCounts {
    #[db(raw)]
    publication_team_counts: Option<String>,
}

#[derive(DbProjection)]
#[db(of=AgentsStorageRow)]
struct ChangedAgent {
    rowid: i64,
    id: String,
    #[db(raw)]
    record: String,
}

#[derive(DbProjection)]
#[db(of=Metadata)]
struct PriorTask {
    task_agent: Option<String>,
    task_created: Option<f64>,
    task_running: Option<i64>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,decode=Metadata::PUBLICATION_LIVE,
    tables(rows=Metadata),columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,live=Metadata::PUBLICATION_LIVE),
    sql="SELECT EXISTS(SELECT 1 FROM {rows} WHERE {kind}=?1 AND {id}=?2 AND {live}=1) AS {live}")]
struct ProjectedLive<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,decode=Metadata::PUBLICATION_LIVE,
    tables(rows=Metadata),columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,live=Metadata::PUBLICATION_LIVE,flight=Metadata::PUBLICATION_IN_FLIGHT),
    sql="SELECT EXISTS(SELECT 1 FROM {rows} WHERE {kind}=?1 AND {id}=?2 AND {live}=1 AND {flight}=1) AS {live}")]
struct WasInFlight<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,result=RawTeamCounts,projection=RawTeamCounts,
    tables(rows=Metadata),columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,counts=Metadata::PUBLICATION_TEAM_COUNTS),
    sql="SELECT {counts} FROM {rows} WHERE {kind}=?1 AND {id}=?2")]
struct PublishedTeamCounts<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=ChangedAgent,projection=ChangedAgent,
    tables(agents=AgentsStorageRow,changes=Changes),
    columns(rowid=AgentsStorageRow::ROWID,id=AgentsStorageRow::ID,record=AgentsStorageRow::RECORD,
        kind=Changes::KIND,key=Changes::ORIGINAL_KEY,generation=Changes::GENERATION),
    sql="SELECT a.{rowid},a.{id},a.{record} FROM {agents} a JOIN {changes} c
        ON c.{key}=a.{id} WHERE c.{kind}=?1 AND c.{generation}>?2 ORDER BY a.{id}")]
struct AgentsChangedSince {
    #[db(column=Changes::KIND)]
    kind: ChangeKind,
    #[db(column=Changes::GENERATION)]
    generation: Option<i64>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
    tables(rows=Metadata,agents=AgentsStorageRow),
    columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,rowid=Metadata::SOURCE_ROWID,id=Metadata::TEXT_KEY,
        live=Metadata::PUBLICATION_LIVE,flight=Metadata::PUBLICATION_IN_FLIGHT,counts=Metadata::PUBLICATION_TEAM_COUNTS,
        agent_id=AgentsStorageRow::ID,deleted=AgentsStorageRow::DELETED_AT,record=AgentsStorageRow::RECORD),
    sql="INSERT INTO {rows}({kind},{identity},{rowid},{id},{live},{flight},{counts})
        SELECT ?1,?2,?3,?4,{deleted} IS NULL AND NOT ?5,COALESCE(json_extract({record},'$.inFlight'),0),?6
        FROM {agents} WHERE {agent_id}=?4
        ON CONFLICT({kind},{identity}) DO UPDATE SET {live}=excluded.{live},{flight}=excluded.{flight},{counts}=excluded.{counts}")]
struct MarkAgent<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::IDENTITY)]
    identity: &'a Vec<u8>,
    #[db(column=Metadata::SOURCE_ROWID)]
    rowid: Option<i64>,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
    #[db(column=Metadata::PUBLICATION_LIVE)]
    deleted: Option<i64>,
    #[db(column=Metadata::PUBLICATION_TEAM_COUNTS)]
    counts: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
    tables(rows=Metadata),columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,live=Metadata::PUBLICATION_LIVE),
    sql="UPDATE {rows} SET {live}=0 WHERE {kind}=?1 AND {id}=?2")]
struct DeleteAgent<'a> {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,result=PriorTask,projection=PriorTask,
    tables(state=Metadata,changes=Changes),
    columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,agent=Metadata::TASK_AGENT,created=Metadata::TASK_CREATED,running=Metadata::TASK_RUNNING,
        change_kind=Changes::KIND,label=Changes::SOURCE_LABEL,task_id=Changes::DURABLE_ID,deleted=Changes::DELETED),
    sql="SELECT s.{agent},s.{created},s.{running} FROM {state} s JOIN {changes} w
        ON w.{change_kind}=?1 AND w.{label}=?2 AND w.{task_id}=s.{id} AND w.{deleted}=0
        WHERE s.{kind}=?3 AND s.{id}=?4")]
struct TaskPrior<'a> {
    #[db(column=Changes::KIND)]
    change_kind: ChangeKind,
    #[db(column=Changes::SOURCE_LABEL)]
    label: Option<&'a str>,
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::TEXT_KEY)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,result=Option<String>,decode=Metadata::TEXT_KEY,
    tables(changes=Changes,state=Metadata),
    indexes(changes=Changes::INDEX_CHANGES_DURABLE_TASK_WINDOW),
    predicates(live=changes),
    columns(kind=Metadata::KIND,id=Metadata::TEXT_KEY,created=Metadata::TASK_CREATED,running=Metadata::TASK_RUNNING,rowid=Metadata::SOURCE_ROWID,
        task_id=Changes::DURABLE_ID),
    sql="SELECT s.{id} FROM (SELECT {task_id} FROM {changes} WHERE {live}) w
        CROSS JOIN {state} s ON s.{id}=w.{task_id} WHERE s.{kind}=?1 AND NOT s.{running}
        ORDER BY s.{created} DESC,s.{rowid} LIMIT 1 OFFSET ?2")]
struct TaskEviction {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
    #[db(column=Metadata::SOURCE_ROWID)]
    history: Option<i64>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=i64,
    tables(rows=Publications),columns(kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,seq=Publications::PUBLICATION_SEQ),
    sql="SELECT COALESCE((SELECT {seq} FROM {rows} WHERE {kind}=?1 AND {collection}=?2 ORDER BY {seq} DESC LIMIT 1),0)")]
struct CollectionSequence<'a> {
    #[db(column=Publications::KIND)]
    kind: PublicationKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
    tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),
    columns(collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id} FROM {rows} WHERE {entities} AND {collection}=?1 AND {deleted}=0 ORDER BY {rowid}")]
struct LiveEntityIds<'a> {
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=bool,
    tables(agents=AgentsStorageRow,rows=Publications),
    columns(agent=AgentsStorageRow::ID,agent_deleted=AgentsStorageRow::DELETED_AT,kind=Publications::KIND,collection=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED),
    sql="SELECT EXISTS(SELECT 1 FROM {agents} a WHERE a.{agent_deleted} IS NULL AND NOT EXISTS(SELECT 1 FROM {rows} q WHERE q.{kind}=?1 AND q.{collection}=?2 AND q.{id}=a.{agent} AND q.{deleted}=0))")]
struct MissingLiveQueue<'a> {
    #[db(column=Publications::KIND)]
    kind: PublicationKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    collection: Option<&'a str>,
}

impl SyncPolicyQueries {
    /// Raw payload projection: missing rows and stored SQL NULL remain distinct.
    pub fn live_entity_payload(
        connection: &studio_db_api::CodecContext,
        collection: &str,
        id: &str,
    ) -> crate::StoreResult<Option<Option<String>>> {
        crate::repositories::sync_documents::publication_payload_nullable(
            connection, collection, id, true,
        )
    }
    pub fn account_destinations_payload(
        connection: &studio_db_api::CodecContext,
    ) -> crate::StoreResult<Option<String>> {
        crate::repositories::sync_documents::publication_payload(
            connection, "accounts", "current", true,
        )
    }
    pub fn entity_is_live(
        connection: &studio_db_api::CodecContext,
        collection: &str,
        id: &str,
    ) -> crate::StoreResult<bool> {
        Ok(crate::repositories::sync_documents::publication_sequence(
            connection, collection, id, false,
        )?
        .is_some())
    }
    pub fn collection_sequence(
        connection: &studio_db_api::CodecContext,
        collection: &str,
    ) -> rusqlite::Result<i64> {
        canvas_read(connection).named(
            &CollectionSequence::QUERY,
            &CollectionSequence {
                kind: PublicationKind::SyncEntities,
                collection: Some(collection),
            },
        )
    }
    pub fn live_entity_ids(
        connection: &studio_db_api::CodecContext,
        collection: &str,
    ) -> rusqlite::Result<Vec<String>> {
        canvas_read(connection).collect(
            &LiveEntityIds::QUERY,
            &LiveEntityIds {
                collection: Some(collection),
            },
        )
    }
    pub fn missing_live_queue(connection: &studio_db_api::CodecContext) -> rusqlite::Result<bool> {
        canvas_read(connection).named(
            &MissingLiveQueue::QUERY,
            &MissingLiveQueue {
                kind: PublicationKind::SyncEntities,
                collection: Some("queue"),
            },
        )
    }

    pub fn projected_live(
        connection: &studio_db_api::CodecContext,
        id: &str,
    ) -> rusqlite::Result<bool> {
        Ok(canvas_read(connection).named(
            &ProjectedLive::QUERY,
            &ProjectedLive {
                kind: MetaKind::AgentPublicationState,
                id: Some(id),
            },
        )? == Some(1))
    }
    pub fn was_in_flight(
        connection: &studio_db_api::CodecContext,
        id: &str,
    ) -> rusqlite::Result<bool> {
        Ok(canvas_read(connection).named(
            &WasInFlight::QUERY,
            &WasInFlight {
                kind: MetaKind::AgentPublicationState,
                id: Some(id),
            },
        )? == Some(1))
    }
    pub fn published_team_counts(
        connection: &studio_db_api::CodecContext,
        id: &str,
    ) -> rusqlite::Result<Option<Option<String>>> {
        Ok(canvas_read(connection)
            .named(
                &PublishedTeamCounts::QUERY,
                &PublishedTeamCounts {
                    kind: MetaKind::AgentPublicationState,
                    id: Some(id),
                },
            )?
            .map(|row| row.publication_team_counts))
    }
    pub fn agents_changed_since(
        connection: &studio_db_api::CodecContext,
        generation: i64,
    ) -> rusqlite::Result<Vec<(i64, String, String)>> {
        Ok(canvas_read(connection)
            .collect(
                &AgentsChangedSince::QUERY,
                &AgentsChangedSince {
                    kind: ChangeKind::SchedulerAgentChanges,
                    generation: Some(generation),
                },
            )?
            .into_iter()
            .map(|row| (row.rowid, row.id, row.record))
            .collect())
    }
    pub fn mark_agent_projection(
        transaction: &studio_db_api::rusqlite::Transaction<'_>,
        id: &str,
        deleted: bool,
        team_counts: Option<&str>,
    ) -> rusqlite::Result<()> {
        let rowid = next_meta_rowid(transaction, MetaKind::AgentPublicationState)?;
        let identity = nullable_text_identity(&Some(id.to_owned()), rowid);
        let mut tx = canvas_tx(transaction);
        tx.named(
            &MarkAgent::QUERY,
            &MarkAgent {
                kind: MetaKind::AgentPublicationState,
                identity: &identity,
                rowid: Some(rowid),
                id: Some(id),
                deleted: Some(i64::from(deleted)),
                counts: team_counts,
            },
        )?;
        if deleted {
            tx.named(
                &DeleteAgent::QUERY,
                &DeleteAgent {
                    kind: MetaKind::AgentPublicationState,
                    id: Some(id),
                },
            )?;
        }
        Ok(())
    }
    pub fn replace_meta_marker(
        transaction: &studio_db_api::rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        // SQLite allocates REPLACE's rowid before deleting the previous maximum.
        let source_rowid = next_meta_rowid(transaction, MetaKind::SyncEntityMeta)?;
        let text_key = Some(key.to_owned());
        let identity = nullable_text_identity(&text_key, source_rowid);
        let mut tx = canvas_tx(transaction);
        let deleted = tx.delete::<SyncEntityMeta>(&(identity.clone(),))?;
        tx.partition::<SyncEntityMeta>().put(&SyncEntityMeta {
            identity,
            source_rowid,
            text_key: text_key.clone(),
            meta_value: StoredSyncMetaValue::from_stored(text_key, value.to_owned()),
        })?;
        crate::record_entity_metadata_updates(
            (deleted as u64).saturating_add(transaction.changes()),
        );
        Ok(())
    }
    pub fn task_window_prior(
        connection: &studio_db_api::CodecContext,
        id: &str,
    ) -> rusqlite::Result<Option<(String, Option<f64>, bool)>> {
        canvas_read(connection)
            .named(
                &TaskPrior::QUERY,
                &TaskPrior {
                    change_kind: ChangeKind::DurableRows,
                    label: Some(crate::durable_seq::TASK_WINDOW),
                    kind: MetaKind::TaskWindowState,
                    id: Some(id),
                },
            )?
            .map(|row| {
                Ok((
                    row.task_agent.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.task_created,
                    row.task_running.ok_or(rusqlite::Error::InvalidQuery)? != 0,
                ))
            })
            .transpose()
    }
    pub fn task_window_eviction_candidate(
        connection: &studio_db_api::CodecContext,
        history: i64,
    ) -> rusqlite::Result<Option<String>> {
        Ok(canvas_read(connection)
            .named(
                &TaskEviction::QUERY,
                &TaskEviction {
                    kind: MetaKind::TaskWindowState,
                    history: Some(history),
                },
            )?
            .flatten())
    }
}

fn next_meta_rowid(
    connection: &studio_db_api::CodecContext,
    kind: MetaKind,
) -> rusqlite::Result<i64> {
    use crate::clock_schema::queries::MetaRowidHigh;
    canvas_read(connection)
        .named(&MetaRowidHigh::QUERY, &MetaRowidHigh { kind })?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_schema::{Changes, DurableRows, DurableSource};
    use crate::consolidation_codec::nullable_text_identity;
    use crate::metadata_schema::{AgentPublicationState, Metadata, TaskWindowState};
    use crate::metadata_values::{PublicationTeamCounts, StoredBoolean};
    use crate::peer_update_settings::canvas_tx;
    use crate::repositories::sync_documents::publication_queries::RawPayload;
    use studio_db_api::migration::encode_key;
    use studio_db_api::{CodecContext, Table};

    fn database() -> CodecContext {
        let db = CodecContext::open_in_memory().unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        db
    }

    fn publication(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        rowid: i64,
        live: i64,
        flight: i64,
        counts: Option<&str>,
    ) {
        let key = Some(id.to_owned());
        canvas_tx(tx)
            .partition::<AgentPublicationState>()
            .put(&AgentPublicationState {
                identity: nullable_text_identity(&key, rowid),
                source_rowid: rowid,
                text_key: key,
                publication_live: StoredBoolean::from_stored(live),
                publication_in_flight: StoredBoolean::from_stored(flight),
                publication_team_counts: counts
                    .map(|raw| PublicationTeamCounts::from_stored(raw.into())),
            })
            .unwrap();
    }

    fn task(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        rowid: i64,
        created: Option<f64>,
        running: i64,
        deleted: i64,
    ) {
        let key = Some(id.to_owned());
        let mut db = canvas_tx(tx);
        db.partition::<TaskWindowState>()
            .put(&TaskWindowState {
                identity: nullable_text_identity(&key, rowid),
                source_rowid: rowid,
                text_key: key,
                task_agent: "agent".into(),
                task_created: created,
                task_running: StoredBoolean::from_stored(running),
            })
            .unwrap();
        db.partition::<DurableRows>()
            .put(&DurableRows {
                identity: encode_key(&(crate::durable_seq::TASK_WINDOW.to_owned(), id.to_owned())),
                source_rowid: rowid,
                source_label: DurableSource::TaskWindow,
                durable_id: id.into(),
                sequence: 1,
                deleted: StoredBoolean::from_stored(deleted),
                deleted_at: 0,
                scope: None,
            })
            .unwrap();
    }

    fn entity(
        tx: &rusqlite::Transaction<'_>,
        collection: &str,
        id: &str,
        rowid: i64,
        sequence: i64,
        deleted: i64,
        raw: Option<&str>,
    ) {
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities};
        let label = CollectionName::from_stored(collection.into());
        let deleted = StoredBoolean::from_stored(deleted);
        canvas_tx(tx)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: encode_key(&(collection.to_owned(), id.to_owned())),
                source_rowid: rowid,
                publication_seq: sequence,
                deleted,
                collection_kind: label.kind(),
                collection_agent: label.agent().cloned(),
                non_transcript: label.non_transcript(),
                collection_order: label,
                entity_id: id.into(),
                entity_hash: "same".into(),
                entity_payload: EntityValue::from_stored(
                    collection,
                    id,
                    deleted,
                    raw.map(str::to_owned),
                )
                .unwrap(),
            })
            .unwrap();
    }

    #[test]
    fn f1_n_publication_selectors_preserve_null_raw_integer_and_source_order() {
        let mut db = database();
        Publications::descriptor().install_if_absent(&db).unwrap();
        let tx = db.transaction().unwrap();
        let raw = " {\"future\":1.00,\"future\":2} ";
        entity(&tx, "future", "physical-first", 30, 7, 0, Some(raw));
        entity(&tx, "future", "source-first", -7, 7, 0, None);
        entity(&tx, "future", "tombstone", 40, 9, 1, None);
        entity(&tx, "future", "nonboolean", 50, 11, 2, Some("malformed"));
        assert_eq!(
            SyncPolicyQueries::live_entity_payload(&tx, "future", "physical-first").unwrap(),
            Some(Some(raw.into()))
        );
        assert_eq!(
            SyncPolicyQueries::live_entity_payload(&tx, "future", "source-first").unwrap(),
            Some(None)
        );
        for id in ["missing", "tombstone", "nonboolean"] {
            assert_eq!(
                SyncPolicyQueries::live_entity_payload(&tx, "future", id).unwrap(),
                None
            );
            assert!(!SyncPolicyQueries::entity_is_live(&tx, "future", id).unwrap());
        }
        assert!(SyncPolicyQueries::entity_is_live(&tx, "future", "source-first").unwrap());
        assert_eq!(
            SyncPolicyQueries::live_entity_ids(&tx, "future").unwrap(),
            ["source-first", "physical-first"]
        );
        assert_eq!(
            SyncPolicyQueries::collection_sequence(&tx, "future").unwrap(),
            11
        );
        assert_eq!(
            SyncPolicyQueries::collection_sequence(&tx, "missing").unwrap(),
            0
        );
        entity(&tx, "negative", "one", 60, -3, 0, None);
        assert_eq!(
            SyncPolicyQueries::collection_sequence(&tx, "negative").unwrap(),
            -3
        );
    }

    #[test]
    fn f1_n_publication_queue_completeness_uses_exact_live_flag() {
        let mut db = database();
        Publications::descriptor().install_if_absent(&db).unwrap();
        AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        source_agent(&tx, "live", r#"{"id":"live","provider":"codex"}"#);
        source_agent(
            &tx,
            "deleted",
            r#"{"id":"deleted","provider":"codex","deletedAt":1}"#,
        );
        assert!(SyncPolicyQueries::missing_live_queue(&tx).unwrap());
        entity(&tx, "queue", "live", 1, 1, 2, None);
        assert!(SyncPolicyQueries::missing_live_queue(&tx).unwrap());
        entity(&tx, "queue", "live", 1, 2, 0, None);
        assert!(!SyncPolicyQueries::missing_live_queue(&tx).unwrap());
    }

    #[test]
    fn f1_n_publication_reads_plan_and_vm_ignore_unrelated_history() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = database();
        Publications::descriptor().install_if_absent(&db).unwrap();
        AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        entity(&tx, "future", "one", 1, 7, 0, None);
        let identity = encode_key(&("future".to_owned(), "one".to_owned()));
        let plans = [
            crate::query_plan_test_support::explain(
                &tx,
                RawPayload::QUERY.sql(),
                &RawPayload {
                    kind: PublicationKind::SyncEntities,
                    identity,
                    live_only: 1,
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                CollectionSequence::QUERY.sql(),
                &CollectionSequence {
                    kind: PublicationKind::SyncEntities,
                    collection: Some("future"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                LiveEntityIds::QUERY.sql(),
                &LiveEntityIds {
                    collection: Some("future"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                MissingLiveQueue::QUERY.sql(),
                &MissingLiveQueue {
                    kind: PublicationKind::SyncEntities,
                    collection: Some("queue"),
                },
            )
            .unwrap(),
        ];
        assert!(
            plans[0]
                .iter()
                .any(|step| step.contains("kind=? AND identity=?")),
            "{:?}",
            plans[0]
        );
        for (plan, index) in plans[1..].iter().zip([
            studio_db_api::query_index::<Canvas, Publications>(
                &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE,
            ),
            studio_db_api::query_index::<Canvas, Publications>(
                &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED,
            ),
            studio_db_api::query_index::<Canvas, Publications>(
                &Publications::INDEX_PUBLICATIONS_ENTITY_IDENTITY,
            ),
        ]) {
            assert!(plan.iter().any(|step| step.contains(index)), "{plan:?}");
        }
        let steps = Arc::new(AtomicUsize::new(0));
        let mut measured = Vec::new();
        for pass in 0..2 {
            if pass == 1 {
                for n in 2..2002 {
                    entity(&tx, "unrelated", &format!("old-{n}"), n, n, 1, None);
                }
            }
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
                SyncPolicyQueries::live_entity_payload(&tx, "future", "one").unwrap(),
                Some(None)
            );
            assert!(SyncPolicyQueries::entity_is_live(&tx, "future", "one").unwrap());
            assert_eq!(
                SyncPolicyQueries::live_entity_ids(&tx, "future").unwrap(),
                ["one"]
            );
            assert_eq!(
                SyncPolicyQueries::collection_sequence(&tx, "future").unwrap(),
                7
            );
            assert!(!SyncPolicyQueries::missing_live_queue(&tx).unwrap());
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            measured.push(steps.swap(0, Ordering::Relaxed));
        }
        assert!(
            measured[1] <= measured[0] + 30 && measured[1] < 200,
            "VM steps: {measured:?}; plans: {plans:?}"
        );
        eprintln!("F1 N VM steps {measured:?}; EQP {plans:?}");
    }

    // Intentional corrupt source evidence cannot pass the AgentRecord write codec.
    fn source_agent(tx: &rusqlite::Transaction<'_>, id: &str, raw: &str) {
        tx.execute(
            "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
            rusqlite::params![id, raw],
        )
        .unwrap();
    }

    #[test]
    fn sync_policy_target_scheduler_join_preserves_raw_bytes_and_id_order() {
        use crate::change_schema::SchedulerAgentChanges;
        let mut db = database();
        crate::agent_tables::AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        let raw_z = " { \"id\":\"z\", \"provider\":\"codex\", \"epoch\":1.5 } ";
        let raw_a = "{\"id\":\"a\",\"provider\":\"codex\",\"rootId\":17}";
        source_agent(&tx, "z", raw_z);
        source_agent(&tx, "a", raw_a);
        for (rowid, id, generation) in [(1, "z", 8), (2, "a", 9), (3, "absent", 10)] {
            let key = Some(id.to_owned());
            canvas_tx(&tx)
                .partition::<SchedulerAgentChanges>()
                .put(&SchedulerAgentChanges {
                    identity: nullable_text_identity(&key, rowid),
                    source_rowid: rowid,
                    original_key: key,
                    generation,
                })
                .unwrap();
        }
        assert_eq!(
            SyncPolicyQueries::agents_changed_since(&tx, 7).unwrap(),
            vec![(2, "a".into(), raw_a.into()), (1, "z".into(), raw_z.into())]
        );
        assert_eq!(
            SyncPolicyQueries::agents_changed_since(&tx, 8).unwrap(),
            vec![(2, "a".into(), raw_a.into())]
        );
        assert!(
            SyncPolicyQueries::agents_changed_since(&tx, 9)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sync_policy_target_mark_preserves_selectors_raw_counts_and_upsert_rowid() {
        let mut db = database();
        crate::agent_tables::AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        source_agent(
            &tx,
            "live",
            r#"{"id":"live","provider":"codex","inFlight":2}"#,
        );
        source_agent(
            &tx,
            "deleted",
            r#"{"id":"deleted","provider":"codex","deletedAt":0,"inFlight":true}"#,
        );
        source_agent(
            &tx,
            "legacy",
            r#"{"id":"legacy","provider":"codex","deletedAt":"legacy text","inFlight":1.5}"#,
        );
        SyncPolicyQueries::mark_agent_projection(&tx, "live", false, Some(" malformed ")).unwrap();
        let identity = nullable_text_identity(&Some("live".into()), 0);
        let before = canvas_tx(&tx)
            .get::<AgentPublicationState>(&(identity.clone(),))
            .unwrap()
            .unwrap();
        assert_eq!(before.publication_in_flight.as_stored(), 2);
        assert!(SyncPolicyQueries::projected_live(&tx, "live").unwrap());
        assert!(!SyncPolicyQueries::was_in_flight(&tx, "live").unwrap());
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "live").unwrap(),
            Some(Some(" malformed ".into()))
        );
        SyncPolicyQueries::mark_agent_projection(&tx, "live", true, None).unwrap();
        let after = canvas_tx(&tx)
            .get::<AgentPublicationState>(&(identity,))
            .unwrap()
            .unwrap();
        assert_eq!(before.source_rowid, after.source_rowid);
        assert!(!SyncPolicyQueries::projected_live(&tx, "live").unwrap());
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "live").unwrap(),
            Some(None)
        );
        SyncPolicyQueries::mark_agent_projection(&tx, "deleted", false, None).unwrap();
        assert!(!SyncPolicyQueries::projected_live(&tx, "deleted").unwrap());
        SyncPolicyQueries::mark_agent_projection(&tx, "legacy", false, Some("exact")).unwrap();
        assert!(!SyncPolicyQueries::projected_live(&tx, "legacy").unwrap());
        assert!(!SyncPolicyQueries::was_in_flight(&tx, "legacy").unwrap());
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "legacy").unwrap(),
            Some(Some("exact".into()))
        );
    }

    #[test]
    fn sync_policy_target_publication_selectors_require_integer_one() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        for (id, rowid, live, flight) in [
            ("yes", 1, 1, 1),
            ("live-two", 2, 2, 1),
            ("flight-two", 3, 1, 2),
            ("zero", 4, 0, 1),
        ] {
            publication(&tx, id, rowid, live, flight, None);
            assert_eq!(
                SyncPolicyQueries::projected_live(&tx, id).unwrap(),
                live == 1
            );
            assert_eq!(
                SyncPolicyQueries::was_in_flight(&tx, id).unwrap(),
                live == 1 && flight == 1
            );
        }
        assert!(!SyncPolicyQueries::projected_live(&tx, "missing").unwrap());
        assert!(!SyncPolicyQueries::was_in_flight(&tx, "missing").unwrap());
    }

    #[test]
    fn sync_policy_target_team_counts_preserve_absence_null_and_raw_text() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "missing").unwrap(),
            None
        );
        publication(&tx, "null", 1, 1, 0, None);
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "null").unwrap(),
            Some(None)
        );
        for (rowid, raw) in [(2, "not JSON"), (3, " { \"working\":1, \"future\": [2] } ")] {
            publication(&tx, "raw", rowid, 1, 0, Some(raw));
            assert_eq!(
                SyncPolicyQueries::published_team_counts(&tx, "raw").unwrap(),
                Some(Some(raw.into()))
            );
        }
    }

    #[test]
    fn sync_policy_target_replace_allocates_before_deleting_maximum() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        let marker = crate::AGENT_PROJECTIONS_SEED_MARKER;
        SyncPolicyQueries::replace_meta_marker(&tx, marker, "1").unwrap();
        let key = Some(marker.to_owned());
        let identity = nullable_text_identity(&key, 0);
        let first = canvas_tx(&tx)
            .get::<crate::metadata_schema::SyncEntityMeta>(&(identity.clone(),))
            .unwrap()
            .unwrap();
        SyncPolicyQueries::replace_meta_marker(&tx, marker, "1").unwrap();
        let second = canvas_tx(&tx)
            .get::<crate::metadata_schema::SyncEntityMeta>(&(identity,))
            .unwrap()
            .unwrap();
        assert_eq!(second.source_rowid, first.source_rowid + 1);
        assert_eq!(second.meta_value.as_stored(), "1");
    }

    #[test]
    fn sync_policy_target_task_prior_preserves_nullable_created_and_bool_coercion() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        task(&tx, "null", 1, None, 2, 0);
        task(&tx, "finished", 2, Some(7.5), 0, 0);
        task(&tx, "deleted", 3, None, 1, 1);
        assert_eq!(
            SyncPolicyQueries::task_window_prior(&tx, "null").unwrap(),
            Some(("agent".into(), None, true))
        );
        assert_eq!(
            SyncPolicyQueries::task_window_prior(&tx, "finished").unwrap(),
            Some(("agent".into(), Some(7.5), false))
        );
        assert_eq!(
            SyncPolicyQueries::task_window_prior(&tx, "deleted").unwrap(),
            None
        );
        assert_eq!(
            SyncPolicyQueries::task_window_prior(&tx, "missing").unwrap(),
            None
        );
    }

    #[test]
    fn sync_policy_target_task_eviction_preserves_source_rowid_ties() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        task(&tx, "physical-first", 30, Some(8.0), 0, 0);
        task(&tx, "source-first", -7, Some(8.0), 0, 0);
        task(&tx, "running", 40, Some(9.0), 2, 0);
        task(&tx, "tombstone", 50, Some(10.0), 0, 1);
        assert_eq!(
            SyncPolicyQueries::task_window_eviction_candidate(&tx, 0).unwrap(),
            Some("source-first".into())
        );
        assert_eq!(
            SyncPolicyQueries::task_window_eviction_candidate(&tx, 1).unwrap(),
            Some("physical-first".into())
        );
        assert_eq!(
            SyncPolicyQueries::task_window_eviction_candidate(&tx, 2).unwrap(),
            None
        );
    }

    #[test]
    fn sync_policy_target_missing_agent_does_not_insert_and_deleted_update_retains_evidence() {
        let mut db = database();
        crate::agent_tables::AgentsStorageRow::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let tx = db.transaction().unwrap();
        SyncPolicyQueries::mark_agent_projection(&tx, "absent", false, Some("supplied")).unwrap();
        assert_eq!(
            SyncPolicyQueries::published_team_counts(&tx, "absent").unwrap(),
            None
        );
        publication(&tx, "removed", 17, 1, 2, Some("malformed original"));
        SyncPolicyQueries::mark_agent_projection(&tx, "removed", true, Some("unused")).unwrap();
        assert!(!SyncPolicyQueries::projected_live(&tx, "removed").unwrap());
        let identity = nullable_text_identity(&Some("removed".into()), 17);
        let prior = canvas_tx(&tx)
            .get::<AgentPublicationState>(&(identity,))
            .unwrap()
            .unwrap();
        assert_eq!(prior.source_rowid, 17);
        assert_eq!(prior.publication_in_flight.as_stored(), 2);
        assert_eq!(
            prior.publication_team_counts.unwrap().as_raw(),
            "malformed original"
        );
    }

    #[test]
    fn sync_policy_target_task_eviction_cost_ignores_retained_tombstones() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = database();
        let tx = db.transaction().unwrap();
        for index in 1..=101 {
            task(
                &tx,
                &format!("live-{index:05}"),
                index,
                Some(index as f64),
                0,
                0,
            );
        }
        let steps = Arc::new(AtomicUsize::new(0));
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
            SyncPolicyQueries::task_window_eviction_candidate(&tx, 100).unwrap(),
            Some("live-00001".into())
        );
        tx.progress_handler(0, None::<fn() -> bool>).unwrap();
        let before = steps.swap(0, Ordering::Relaxed);
        for index in 102..=2101 {
            task(
                &tx,
                &format!("old-{index}"),
                index,
                Some(index as f64),
                0,
                1,
            );
        }
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
            SyncPolicyQueries::task_window_eviction_candidate(&tx, 100).unwrap(),
            Some("live-00001".into())
        );
        tx.progress_handler(0, None::<fn() -> bool>).unwrap();
        let after = steps.load(Ordering::Relaxed);
        assert!(
            after <= before + 100,
            "retained tombstones: {before}->{after}"
        );
        assert!(after < 101 * 60 + 1000, "history boundary: {after}");
    }

    #[test]
    fn sync_policy_target_task_eviction_plan_uses_live_changes_index() {
        let db = database();
        let params = TaskEviction {
            kind: MetaKind::TaskWindowState,
            history: Some(100),
        };
        let plan = crate::query_plan_test_support::explain(&db, TaskEviction::QUERY.sql(), &params)
            .unwrap();
        assert!(
            plan.iter().any(
                |step| step.contains(studio_db_api::query_index::<Canvas, Changes>(
                    &Changes::INDEX_CHANGES_DURABLE_TASK_WINDOW
                ))
            ),
            "{plan:?}"
        );
    }
}
