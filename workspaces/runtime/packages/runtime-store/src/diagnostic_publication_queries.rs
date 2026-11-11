//! Fixed canonical reads for diagnostics and publication maintenance.
use crate::agent_tables::AgentsStorageRow;
use crate::consolidation_kinds::{CanvasMetadataKind as MetaKind, CanvasPublicationsKind as Kind};
use crate::item_monitor_terminal::MonitorsRow;
use crate::metadata_schema::Metadata;
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::publication_schema::Publications;
use crate::queue_schema::EventsRow;
use crate::receipt_tables::{AccountTransferRow, ErrorIncidentRow};
use studio_db_api::{CodecContext, DbProjection, DbQuery};

pub struct DiagnosticPublicationQueries;

/// Persisted migration status fields in their historical selector order.
pub type TaskMigrationStatusRow = (i64, bool, String, Option<f64>, Option<String>);

/// Command identity, label, sequence, bounded payload bytes and full byte length.
pub type CommandPublicationRow = (String, String, i64, Vec<u8>, i64);

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),columns(source_rowid=Publications::SOURCE_ROWID,kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,deleted=Publications::DELETED),
 sql="SELECT {id} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0 ORDER BY {source_rowid}")]
struct LiveIds<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),columns(source_rowid=Publications::SOURCE_ROWID,kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT {id} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0 AND CASE WHEN json_valid({payload}) THEN json_type({payload},'$.value.reviewTargets') IS NOT NULL ELSE 0 END ORDER BY {source_rowid}")]
struct StaleRooms<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=(Option<String>,i64),
 tables(rows=Publications),columns(payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,kind=Publications::KIND,identity=Publications::IDENTITY),
 sql="SELECT {payload},{deleted} FROM {rows} WHERE {kind}=?1 AND {identity}=?2")]
struct Projection<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::IDENTITY)]
    identity: &'a Vec<u8>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),columns(source_rowid=Publications::SOURCE_ROWID,kind=Publications::KIND,label=Publications::COLLECTION_ORDER,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT {payload} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0 ORDER BY {source_rowid}")]
struct LivePayloads<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT {payload} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0 AND {id}>=?3 AND {id}<?4")]
struct ErrorRange<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    start: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    end: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=Option<String>,
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),columns(source_rowid=Publications::SOURCE_ROWID,kind=Publications::KIND,label=Publications::COLLECTION_ORDER,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT json_extract({payload},'$.value.agentName') FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0 AND json_extract({payload},'$.value.agentId')=?3 ORDER BY {source_rowid}")]
struct OwnerNames<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
    #[db(column=AgentsStorageRow::ID)]
    owner: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=bool,
 tables(rows=Publications),columns(deleted=Publications::DELETED,kind=Publications::KIND,identity=Publications::IDENTITY),
 sql="SELECT EXISTS(SELECT 1 FROM {rows} WHERE {kind}=?1 AND {identity}=?2 AND {deleted}=0)")]
struct IsPublished<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::IDENTITY)]
    identity: &'a Vec<u8>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=bool,
 tables(rows=Publications,monitors=MonitorsRow),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),
 columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,monitor_id=MonitorsRow::ID,record=MonitorsRow::RECORD),
 sql="SELECT EXISTS(SELECT 1 FROM (SELECT {payload} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0) e WHERE (json_extract(e.{payload},'$.value.monitorId')=?3 OR json_extract(e.{payload},'$.value.ruleId')=(SELECT json_extract(m.{record},'$.ruleId') FROM {monitors} m WHERE m.{monitor_id}=?3 AND json_extract(m.{record},'$.status')='completed')))")]
struct MonitorPublished<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
    #[db(column=MonitorsRow::ID)]
    id: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=bool,
 tables(rows=Publications,events=EventsRow,agents=AgentsStorageRow,transfers=AccountTransferRow),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),
 columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,
 event_agent=EventsRow::AGENT,event_status=EventsRow::STATUS,agent_id=AgentsStorageRow::ID,agent_record=AgentsStorageRow::RECORD,transfer_record=AccountTransferRow::RECORD),
 sql="SELECT EXISTS(SELECT 1 FROM {events} WHERE {event_agent}=?3 AND {event_status}='pending') OR EXISTS(SELECT 1 FROM {agents} WHERE {agent_id}=?3 AND (json_extract({agent_record},'$.usageResume.id') IS NOT NULL OR json_extract({agent_record},'$.capacityRetry.id') IS NOT NULL)) OR EXISTS(SELECT 1 FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {id}>='liveness:' AND {id}<'liveness;' AND {deleted}=0 AND json_extract({payload},'$.value.agentId')=?3) OR EXISTS(SELECT 1 FROM {transfers} WHERE json_extract({transfer_record},'$.status')='pending')")]
struct LivenessDependencies<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
    #[db(column=AgentsStorageRow::ID)]
    id: &'a str,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=bool,
 tables(rows=Publications,monitors=MonitorsRow,events=EventsRow,agents=AgentsStorageRow,incidents=ErrorIncidentRow),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),
 columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,
 monitor_id=MonitorsRow::ID,event_id=EventsRow::ID,agent_id=AgentsStorageRow::ID,incident_id=ErrorIncidentRow::ID,incident_payload=ErrorIncidentRow::PAYLOAD),
 sql="SELECT EXISTS(SELECT 1 FROM (SELECT {id},{payload} FROM {rows} WHERE {entities} AND {kind}=?1 AND {label}=?2 AND {deleted}=0) e WHERE ((e.{id} LIKE 'monitor:%' AND NOT EXISTS(SELECT 1 FROM {monitors} m WHERE m.{monitor_id}=substr(e.{id},9))) OR (e.{id} LIKE 'event:%' AND NOT EXISTS(SELECT 1 FROM {events} v WHERE v.{event_id}=substr(e.{id},7))) OR (e.{id} LIKE 'stored:runtime_agents:%' AND NOT EXISTS(SELECT 1 FROM {agents} a WHERE a.{agent_id}=json_extract(e.{payload},'$.value.diagnostics.context.id'))) OR ((e.{id} LIKE 'http:%' OR e.{id} LIKE 'recovery:%') AND NOT EXISTS(SELECT 1 FROM {incidents} r WHERE r.{incident_id}=e.{id} AND r.{incident_payload}!='null')) OR (json_extract(e.{payload},'$.value.action')!='details' AND NOT EXISTS(SELECT 1 FROM {agents} a WHERE a.{agent_id}=json_extract(e.{payload},'$.value.agentId')))))")]
struct SourceMissing<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::COLLECTION_ORDER)]
    label: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String,i64,Vec<u8>,i64),
 tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_ID),columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED),
 sql="SELECT {label},{id},{seq},COALESCE(substr(CAST({payload} AS BLOB),1,?4),X''),COALESCE(length(CAST({payload} AS BLOB)),0) FROM {rows} WHERE {kind}=?1 AND {id} IN (?2,?3) AND {deleted}=0 ORDER BY {label},{id}")]
struct CommandRows<'a> {
    #[db(column=Publications::KIND)]
    kind: Kind,
    #[db(column=Publications::ENTITY_ID)]
    reference: Option<&'a str>,
    #[db(column=Publications::ENTITY_ID)]
    chat: Option<&'a str>,
    #[db(column=Publications::SOURCE_ROWID)]
    max_bytes: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=(String,i64,f64),
 tables(rows=Metadata),indexes(rows=Metadata::INDEX_METADATA_SEARCH_ROLLOUT_KEY),predicates(partition=rows),columns(kind=Metadata::KIND,id=Metadata::INTEGER_KEY,phase=Metadata::SEARCH_PHASE,cursor=Metadata::SEARCH_CURSOR,updated=Metadata::SEARCH_UPDATED),
 sql="SELECT {phase},{cursor},{updated} FROM {rows} WHERE {partition} AND {kind}=?1 AND {id}=1")]
struct SearchStatus {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,String),
 tables(rows=Metadata),indexes(rows=Metadata::INDEX_METADATA_SYNC_ENTITY_META_KEY),predicates(partition=rows),columns(kind=Metadata::KIND,key=Metadata::TEXT_KEY,value=Metadata::META_VALUE),
 sql="SELECT {key},{value} FROM {rows} WHERE {partition} AND {kind}=?1 AND {key} IN ('entity_tombstone_count','entity_tombstone_floor')")]
struct Tombstones {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
}

#[derive(DbProjection)]
#[db(of=Metadata)]
struct TaskMigrationFields {
    migration_cursor: Option<i64>,
    migration_complete: Option<i64>,
    migration_status: Option<studio_runtime_schema::persistence::MigrationStatus>,
    migration_updated: Option<f64>,
    migration_error: Option<String>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,result=TaskMigrationFields,projection=TaskMigrationFields,
 tables(rows=Metadata),indexes(rows=Metadata::INDEX_METADATA_PAYLOAD_MIGRATIONS_KEY),predicates(partition=rows),columns(kind=Metadata::KIND,key=Metadata::TEXT_KEY,cursor=Metadata::MIGRATION_CURSOR,complete=Metadata::MIGRATION_COMPLETE,status=Metadata::MIGRATION_STATUS,updated=Metadata::MIGRATION_UPDATED,error=Metadata::MIGRATION_ERROR),
 sql="SELECT {cursor},{complete},{status},{updated},{error} FROM {rows} WHERE {partition} AND {kind}=?1 AND {key}='payload-v1:tasks'")]
struct TaskMigrationStatus {
    #[db(column=Metadata::KIND)]
    kind: MetaKind,
}

impl DiagnosticPublicationQueries {
    pub fn metadata_available(db: &CodecContext) -> rusqlite::Result<bool> {
        canvas_read(db).table_exists::<Metadata>()
    }
    pub fn publications_available(db: &CodecContext) -> rusqlite::Result<bool> {
        canvas_read(db).table_exists::<Publications>()
    }
    pub fn search_status(db: &CodecContext) -> rusqlite::Result<Option<(String, i64, f64)>> {
        if !Self::metadata_available(db)? {
            return Ok(None);
        }
        canvas_read(db).named(
            &SearchStatus::QUERY,
            &SearchStatus {
                kind: MetaKind::SearchRollout,
            },
        )
    }
    pub fn task_migration_status(
        db: &CodecContext,
    ) -> rusqlite::Result<Option<TaskMigrationStatusRow>> {
        if !Self::metadata_available(db)? {
            return Ok(None);
        }
        canvas_read(db)
            .named(
                &TaskMigrationStatus::QUERY,
                &TaskMigrationStatus {
                    kind: MetaKind::PayloadMigrations,
                },
            )?
            .map(|row| {
                Ok((
                    row.migration_cursor.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.migration_complete
                        .ok_or(rusqlite::Error::InvalidQuery)?
                        != 0,
                    row.migration_status
                        .ok_or(rusqlite::Error::InvalidQuery)?
                        .as_str()
                        .to_owned(),
                    row.migration_updated,
                    row.migration_error,
                ))
            })
            .transpose()
    }

    pub fn online_progress(
        db: &CodecContext,
    ) -> rusqlite::Result<Vec<crate::online_migration::Progress>> {
        if !Self::metadata_available(db)?
            && !canvas_read(db).table_exists::<crate::settings_schema::PayloadMigrations>()?
        {
            return Ok(Vec::new());
        }
        crate::online_migration::progress(db)
    }
    pub fn tombstones(db: &CodecContext) -> rusqlite::Result<Vec<(String, String)>> {
        canvas_read(db).collect(
            &Tombstones::QUERY,
            &Tombstones {
                kind: MetaKind::SyncEntityMeta,
            },
        )
    }
    pub fn projection(
        db: &CodecContext,
        label: &str,
        id: &str,
    ) -> rusqlite::Result<Option<(Option<String>, i64)>> {
        let identity = studio_db_api::migration::encode_key(&(label.to_owned(), id.to_owned()));
        canvas_read(db).named(
            &Projection::QUERY,
            &Projection {
                kind: Kind::SyncEntities,
                identity: &identity,
            },
        )
    }
    pub fn stale_rooms(db: &CodecContext) -> rusqlite::Result<Vec<String>> {
        canvas_read(db).collect(
            &StaleRooms::QUERY,
            &StaleRooms {
                kind: Kind::SyncEntities,
                label: Some("room"),
            },
        )
    }
    pub fn visit_live_tasks(
        db: &CodecContext,
        mut visit: impl FnMut(String) -> crate::StoreResult<()>,
    ) -> crate::StoreResult<()> {
        let mut visitor_error = None;
        let result = canvas_read(db).visit(
            &LivePayloads::QUERY,
            &LivePayloads {
                kind: Kind::SyncEntities,
                label: Some("task"),
            },
            |row| {
                visit(row).map_err(|error| {
                    visitor_error = Some(error);
                    rusqlite::Error::InvalidQuery
                })
            },
        );
        if let Some(error) = visitor_error {
            return Err(error);
        }
        Ok(result?)
    }
    pub fn error_ids(db: &CodecContext) -> rusqlite::Result<Vec<String>> {
        canvas_read(db).collect(
            &LiveIds::QUERY,
            &LiveIds {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
            },
        )
    }
    pub fn error_range(db: &CodecContext, start: &str, end: &str) -> rusqlite::Result<Vec<String>> {
        canvas_read(db).collect(
            &ErrorRange::QUERY,
            &ErrorRange {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
                start: Some(start),
                end: Some(end),
            },
        )
    }
    pub fn error_owner_names(
        db: &CodecContext,
        owner: &str,
    ) -> rusqlite::Result<Vec<Option<String>>> {
        canvas_read(db).collect(
            &OwnerNames::QUERY,
            &OwnerNames {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
                owner,
            },
        )
    }
    pub fn is_published(db: &CodecContext, id: &str) -> rusqlite::Result<bool> {
        let identity =
            studio_db_api::migration::encode_key(&("studioError".to_owned(), id.to_owned()));
        canvas_read(db).named(
            &IsPublished::QUERY,
            &IsPublished {
                kind: Kind::SyncEntities,
                identity: &identity,
            },
        )
    }
    pub fn monitor_published(db: &CodecContext, id: &str) -> rusqlite::Result<bool> {
        canvas_read(db).named(
            &MonitorPublished::QUERY,
            &MonitorPublished {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
                id,
            },
        )
    }
    pub fn liveness_dependencies(db: &CodecContext, id: &str) -> rusqlite::Result<bool> {
        canvas_read(db).named(
            &LivenessDependencies::QUERY,
            &LivenessDependencies {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
                id,
            },
        )
    }
    pub fn source_missing(db: &CodecContext) -> rusqlite::Result<bool> {
        canvas_read(db).named(
            &SourceMissing::QUERY,
            &SourceMissing {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
            },
        )
    }
    pub fn command_rows(
        db: &CodecContext,
        reference: &str,
        chat: &str,
        max_bytes: i64,
    ) -> rusqlite::Result<Vec<CommandPublicationRow>> {
        canvas_read(db).collect(
            &CommandRows::QUERY,
            &CommandRows {
                kind: Kind::SyncEntities,
                reference: Some(reference),
                chat: Some(chat),
                max_bytes,
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_schema::{
        PayloadMigrations, SearchRollout, StoredSyncMetaValue, SyncEntityMeta,
    };
    use crate::metadata_values::{SearchPhase, StoredBoolean};
    use crate::peer_update_settings::canvas_tx;
    use crate::publication_collection::CollectionName;
    use crate::publication_schema::{EntityValue, SyncEntities};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use studio_db_api::{Table, migration::encode_key};

    fn database() -> CodecContext {
        let db = CodecContext::open_in_memory().unwrap();
        for descriptor in [
            Publications::descriptor(),
            Metadata::descriptor(),
            AgentsStorageRow::descriptor(),
            MonitorsRow::descriptor(),
            EventsRow::descriptor(),
            AccountTransferRow::descriptor(),
            ErrorIncidentRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        db
    }

    fn entity(
        tx: &rusqlite::Transaction<'_>,
        label: &str,
        id: &str,
        rowid: i64,
        seq: i64,
        deleted: i64,
        raw: Option<&str>,
    ) {
        let collection = CollectionName::from_stored(label.into());
        let deleted = StoredBoolean::from_stored(deleted);
        canvas_tx(tx)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: encode_key(&(label.to_owned(), id.to_owned())),
                source_rowid: rowid,
                publication_seq: seq,
                deleted,
                collection_kind: collection.kind(),
                collection_agent: collection.agent().cloned(),
                non_transcript: collection.non_transcript(),
                collection_order: collection,
                entity_id: id.into(),
                entity_hash: "0".repeat(64),
                entity_payload: EntityValue::from_stored(
                    label,
                    id,
                    deleted,
                    raw.map(str::to_owned),
                )
                .unwrap(),
            })
            .unwrap();
    }

    #[test]
    fn f1_diagnostic_projection_preserves_missing_null_deleted_and_raw_text() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::projection(&tx, "room", "absent").unwrap(),
            None
        );
        entity(&tx, "room", "null", 20, 1, 0, None);
        assert_eq!(
            DiagnosticPublicationQueries::projection(&tx, "room", "null").unwrap(),
            Some((None, 0))
        );
        let raw = " \n{\"value\":{\"reviewTargets\":null,\"text\":\"\\u0061\",\"n\":1.00}}\t";
        entity(&tx, "room", "raw", -7, 2, 0, Some(raw));
        entity(&tx, "room", "malformed", 21, 3, 0, Some("not JSON"));
        entity(&tx, "room", "deleted-two", 22, 4, 2, Some(raw));
        assert_eq!(
            DiagnosticPublicationQueries::projection(&tx, "room", "raw").unwrap(),
            Some((Some(raw.into()), 0))
        );
        assert_eq!(
            DiagnosticPublicationQueries::projection(&tx, "room", "malformed").unwrap(),
            Some((Some("not JSON".into()), 0))
        );
        assert_eq!(
            DiagnosticPublicationQueries::projection(&tx, "room", "deleted-two").unwrap(),
            Some((Some(raw.into()), 2))
        );
        assert_eq!(
            DiagnosticPublicationQueries::stale_rooms(&tx).unwrap(),
            vec!["raw"]
        );
        tx.rollback().unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::projection(&db, "room", "raw").unwrap(),
            None
        );
    }

    #[test]
    fn f1_diagnostic_error_scans_keep_integer_selectors_source_order_and_json_scalar_policy() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(
            &tx,
            "studioError",
            "agent:a:late",
            40,
            1,
            0,
            Some(r#"{"value":{"agentId":"a","agentName":"late","action":"details"}}"#),
        );
        entity(
            &tx,
            "studioError",
            "agent:a:first",
            -5,
            2,
            0,
            Some(
                r#"{"value":{"agentId":"a","agentId":"other","agentName":null,"agentName":"ignored","action":"details"}}"#,
            ),
        );
        entity(
            &tx,
            "studioError",
            "agent:a:deleted",
            50,
            3,
            2,
            Some("not JSON"),
        );
        entity(&tx, "other", "agent:a:other", 60, 4, 0, Some("not JSON"));
        assert_eq!(
            DiagnosticPublicationQueries::error_ids(&tx).unwrap(),
            vec!["agent:a:first", "agent:a:late"]
        );
        assert_eq!(
            DiagnosticPublicationQueries::error_owner_names(&tx, "a").unwrap(),
            vec![None, Some("late".into())]
        );
        assert_eq!(
            DiagnosticPublicationQueries::error_range(&tx, "agent:a:", "agent:a;")
                .unwrap()
                .len(),
            2
        );
        assert!(DiagnosticPublicationQueries::is_published(&tx, "agent:a:first").unwrap());
        assert!(!DiagnosticPublicationQueries::is_published(&tx, "agent:a:deleted").unwrap());
        assert!(!DiagnosticPublicationQueries::is_published(&tx, "absent").unwrap());
        assert!(!DiagnosticPublicationQueries::source_missing(&tx).unwrap());
        entity(
            &tx,
            "studioError",
            "agent:a:number",
            70,
            5,
            0,
            Some(r#"{"value":{"agentId":"a","agentName":17,"action":"details"}}"#),
        );
        assert!(DiagnosticPublicationQueries::error_owner_names(&tx, "a").is_err());
    }

    #[test]
    fn f1_diagnostic_monitor_liveness_and_source_visibility_use_canonical_rows() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert!(!DiagnosticPublicationQueries::monitor_published(&tx, "m").unwrap());
        assert!(!DiagnosticPublicationQueries::liveness_dependencies(&tx, "a").unwrap());
        entity(
            &tx,
            "studioError",
            "liveness:one",
            1,
            1,
            0,
            Some(r#"{"value":{"monitorId":"m","agentId":"a","action":"details"}}"#),
        );
        assert!(DiagnosticPublicationQueries::monitor_published(&tx, "m").unwrap());
        assert!(DiagnosticPublicationQueries::liveness_dependencies(&tx, "a").unwrap());
        assert!(!DiagnosticPublicationQueries::liveness_dependencies(&tx, "other").unwrap());
        assert!(!DiagnosticPublicationQueries::source_missing(&tx).unwrap());
        entity(
            &tx,
            "studioError",
            "monitor:gone",
            2,
            2,
            0,
            Some(r#"{"value":{"action":"details"}}"#),
        );
        assert!(DiagnosticPublicationQueries::source_missing(&tx).unwrap());
    }

    #[test]
    fn f1_diagnostic_task_visit_streams_source_order_and_restores_visitor_error() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(&tx, "task", "later", 20, 1, 0, Some(" malformed "));
        entity(&tx, "task", "earlier", -4, 2, 0, Some(" {\"value\":null} "));
        entity(&tx, "task", "deleted", 30, 3, 2, None);
        let mut raw = Vec::new();
        DiagnosticPublicationQueries::visit_live_tasks(&tx, |row| {
            raw.push(row);
            Ok(())
        })
        .unwrap();
        assert_eq!(raw, vec![" {\"value\":null} ", " malformed "]);
        let mut visited = 0;
        let error = DiagnosticPublicationQueries::visit_live_tasks(&tx, |_| {
            visited += 1;
            Err(crate::StoreError::RecordContract("visitor witness"))
        })
        .unwrap_err();
        assert!(matches!(
            error,
            crate::StoreError::RecordContract("visitor witness")
        ));
        assert_eq!(visited, 1);
        entity(&tx, "task", "null", 40, 4, 0, None);
        assert!(DiagnosticPublicationQueries::visit_live_tasks(&tx, |_| Ok(())).is_err());
    }

    #[test]
    fn f1_diagnostic_command_rows_bound_bytes_preserve_full_length_sequence_and_order() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(&tx, "z", "ref", 1, 90, 0, Some("éabc"));
        entity(&tx, "a", "chat", 2, 10, 0, None);
        entity(&tx, "b", "ref", 3, 80, 2, Some("ignored"));
        let rows = DiagnosticPublicationQueries::command_rows(&tx, "ref", "chat", 1).unwrap();
        assert_eq!(
            rows,
            vec![
                ("a".into(), "chat".into(), 10, vec![], 0),
                ("z".into(), "ref".into(), 90, vec![0xc3], 5)
            ]
        );
    }

    #[test]
    fn f1_diagnostic_metadata_keeps_historical_phase_integer_boolean_and_nullable_progress() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::search_status(&tx).unwrap(),
            None
        );
        assert_eq!(
            DiagnosticPublicationQueries::task_migration_status(&tx).unwrap(),
            None
        );
        let mut write = canvas_tx(&tx);
        write
            .partition::<SearchRollout>()
            .put(&SearchRollout {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                search_phase: SearchPhase::Other("historical".into()),
                search_cursor: 17,
                search_updated: 1.25,
            })
            .unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::search_status(&tx).unwrap(),
            Some(("historical".into(), 17, 1.25))
        );
        let key = Some("payload-v1:tasks".to_owned());
        write
            .partition::<PayloadMigrations>()
            .put(&PayloadMigrations {
                identity: crate::consolidation_codec::nullable_text_identity(&key, 7),
                source_rowid: 7,
                text_key: key,
                migration_cursor: 19,
                migration_complete: StoredBoolean::from_stored(2),
                migration_status: studio_runtime_schema::persistence::MigrationStatus::Running,
                migration_updated: None,
                migration_error: Some("retained error".into()),
                migration_cutover_ms: None,
                migration_cursor_key: None,
                migration_source_rows: None,
                migration_target_rows: None,
            })
            .unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::task_migration_status(&tx).unwrap(),
            Some((
                19,
                true,
                "running".into(),
                None,
                Some("retained error".into())
            ))
        );
        let key = Some("entity_tombstone_count".to_owned());
        write
            .partition::<SyncEntityMeta>()
            .put(&SyncEntityMeta {
                identity: crate::consolidation_codec::nullable_text_identity(&key, 8),
                source_rowid: 8,
                text_key: key.clone(),
                meta_value: StoredSyncMetaValue::from_stored(key, "0007".into()),
            })
            .unwrap();
        assert_eq!(
            DiagnosticPublicationQueries::tombstones(&tx).unwrap(),
            vec![("entity_tombstone_count".into(), "0007".into())]
        );
    }

    #[test]
    fn f1_diagnostic_hot_read_plans_and_vm_cost_ignore_retained_tombstones() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        entity(
            &tx,
            "studioError",
            "live",
            1,
            1,
            0,
            Some(r#"{"value":{"agentId":"a","agentName":"A","action":"details"}}"#),
        );
        let ids = LiveIds {
            kind: Kind::SyncEntities,
            label: Some("studioError"),
        };
        let plan =
            crate::query_plan_test_support::explain(&tx, LiveIds::QUERY.sql(), &ids).unwrap();
        assert!(
            plan.iter().any(|step| step.contains(
                studio_db_api::query_index::<Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED
                )
            )),
            "{plan:?}"
        );
        let command = CommandRows {
            kind: Kind::SyncEntities,
            reference: Some("live"),
            chat: Some("absent"),
            max_bytes: 64,
        };
        let plan = crate::query_plan_test_support::explain(&tx, CommandRows::QUERY.sql(), &command)
            .unwrap();
        assert!(
            plan.iter().any(|step| step.contains(
                studio_db_api::query_index::<Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_ID
                )
            )),
            "{plan:?}"
        );
        let identity = encode_key(&("studioError".to_owned(), "live".to_owned()));
        let point_plan = crate::query_plan_test_support::explain(
            &tx,
            IsPublished::QUERY.sql(),
            &IsPublished {
                kind: Kind::SyncEntities,
                identity: &identity,
            },
        )
        .unwrap();
        assert!(
            point_plan
                .iter()
                .any(|step| step.contains("SEARCH") && step.contains("identity")),
            "{point_plan:?}"
        );
        for plan in [
            crate::query_plan_test_support::explain(
                &tx,
                ErrorRange::QUERY.sql(),
                &ErrorRange {
                    kind: Kind::SyncEntities,
                    label: Some("studioError"),
                    start: Some("agent:a:"),
                    end: Some("agent:a;"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                OwnerNames::QUERY.sql(),
                &OwnerNames {
                    kind: Kind::SyncEntities,
                    label: Some("studioError"),
                    owner: "a",
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                MonitorPublished::QUERY.sql(),
                &MonitorPublished {
                    kind: Kind::SyncEntities,
                    label: Some("studioError"),
                    id: "missing",
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                LivenessDependencies::QUERY.sql(),
                &LivenessDependencies {
                    kind: Kind::SyncEntities,
                    label: Some("studioError"),
                    id: "a",
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                SourceMissing::QUERY.sql(),
                &SourceMissing {
                    kind: Kind::SyncEntities,
                    label: Some("studioError"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                StaleRooms::QUERY.sql(),
                &StaleRooms {
                    kind: Kind::SyncEntities,
                    label: Some("room"),
                },
            )
            .unwrap(),
            crate::query_plan_test_support::explain(
                &tx,
                LivePayloads::QUERY.sql(),
                &LivePayloads {
                    kind: Kind::SyncEntities,
                    label: Some("task"),
                },
            )
            .unwrap(),
        ] {
            assert!(
                plan.iter()
                    .any(
                        |step| step.contains(studio_db_api::query_index::<Canvas, Publications>(
                            &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED
                        ))
                    ),
                "{plan:?}"
            );
        }
        let liveness_plan = crate::query_plan_test_support::explain(
            &tx,
            LivenessDependencies::QUERY.sql(),
            &LivenessDependencies {
                kind: Kind::SyncEntities,
                label: Some("studioError"),
                id: "a",
            },
        )
        .unwrap();
        assert_eq!(
            liveness_plan
                .iter()
                .filter(|step| step.starts_with("SEARCH "))
                .count(),
            4,
            "{liveness_plan:?}"
        );
        assert!(
            liveness_plan
                .iter()
                .all(|step| !step.starts_with("SCAN ") || step == "SCAN CONSTANT ROW"),
            "{liveness_plan:?}"
        );
        let steps = Arc::new(AtomicUsize::new(0));
        let measure = || {
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
            DiagnosticPublicationQueries::error_ids(&tx).unwrap();
            DiagnosticPublicationQueries::error_range(&tx, "agent:a:", "agent:a;").unwrap();
            DiagnosticPublicationQueries::error_owner_names(&tx, "a").unwrap();
            DiagnosticPublicationQueries::is_published(&tx, "live").unwrap();
            DiagnosticPublicationQueries::monitor_published(&tx, "missing").unwrap();
            DiagnosticPublicationQueries::liveness_dependencies(&tx, "a").unwrap();
            DiagnosticPublicationQueries::source_missing(&tx).unwrap();
            DiagnosticPublicationQueries::command_rows(&tx, "live", "absent", 64).unwrap();
            DiagnosticPublicationQueries::stale_rooms(&tx).unwrap();
            DiagnosticPublicationQueries::projection(&tx, "room", "absent").unwrap();
            DiagnosticPublicationQueries::visit_live_tasks(&tx, |_| Ok(())).unwrap();
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            steps.load(Ordering::Relaxed)
        };
        let before = measure();
        for index in 2..=2001 {
            entity(
                &tx,
                "studioError",
                &format!("old-{index}"),
                index,
                index,
                1,
                Some("not JSON"),
            );
        }
        let after = measure();
        assert!(
            after <= before + 100,
            "VM retained-history cost {before}->{after}"
        );
        assert!(after < 2000, "VM bound {after}");
        eprintln!("F1 O hot reads VM: {before}->{after}; EQP collection_deleted and entity_id");
    }
}
