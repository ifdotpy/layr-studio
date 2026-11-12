//! Canonical runtime-event publication producers.
use crate::consolidation_kinds::{CanvasMetadataKind, CanvasPublicationsKind};
use crate::metadata_schema::Metadata;
use crate::peer_update_settings::Canvas;
use crate::publication_collection::CollectionKind;
use crate::publication_schema::Publications;
use crate::queue_schema::EventsRow;
use studio_db_api::{DbQuery, Table, migration::key_sql, query_column as column};

/// Number of event rows retained in the published event window.
pub(crate) const EVENT_WINDOW_ROWS: usize = 200;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=String,decode=EventsRow::ID,
    tables(events=EventsRow),columns(id=EventsRow::ID,agent=EventsRow::AGENT,kind=EventsRow::KIND,
        status=EventsRow::STATUS,created=EventsRow::CREATED,error=EventsRow::ERROR),
    sql="SELECT {id},{agent},{kind},{status},{created},{error} FROM {events} ORDER BY {created} DESC,{id} LIMIT ?1")]
struct Window {
    #[db(column=EventsRow::EPOCH)]
    limit: i64,
}

/// Fixed replacement DDL; the authority owner executes it in its transaction.
pub(crate) fn trigger_sql() -> String {
    let publications = Publications::NAME;
    let events = EventsRow::NAME;
    let metadata = Metadata::NAME;
    let kind = column::<Canvas, _>(Publications::KIND);
    let identity = column::<Canvas, _>(Publications::IDENTITY);
    let rowid = column::<Canvas, _>(Publications::SOURCE_ROWID);
    let seq = column::<Canvas, _>(Publications::PUBLICATION_SEQ);
    let deleted = column::<Canvas, _>(Publications::DELETED);
    let label = column::<Canvas, _>(Publications::COLLECTION_ORDER);
    let collection_kind = column::<Canvas, _>(Publications::COLLECTION_KIND);
    let non_transcript = column::<Canvas, _>(Publications::NON_TRANSCRIPT);
    let id = column::<Canvas, _>(Publications::ENTITY_ID);
    let payload = column::<Canvas, _>(Publications::ENTITY_PAYLOAD);
    let hash = column::<Canvas, _>(Publications::ENTITY_HASH);
    let event_id = column::<Canvas, _>(EventsRow::ID);
    let created = column::<Canvas, _>(EventsRow::CREATED);
    let agent = column::<Canvas, _>(EventsRow::AGENT);
    let event_kind = column::<Canvas, _>(EventsRow::KIND);
    let status = column::<Canvas, _>(EventsRow::STATUS);
    let error = column::<Canvas, _>(EventsRow::ERROR);
    let entity_kind = CanvasPublicationsKind::DB_CODE_SYNCENTITIES;
    let event_code = CollectionKind::DB_CODE_EVENT;
    let event = studio_runtime_schema::generated::SyncEventEntityPayloadCollection::default();
    let event = event.as_str();
    let high = crate::clock_schema::queries::High::QUERY
        .sql()
        .replace("?1", CanvasMetadataKind::DB_CODE_SYNCGENERATION);
    let rowid_high =
        crate::repositories::sync_documents::publication_queries::EntityRowidHigh::QUERY
            .sql()
            .replace("?1", entity_kind);
    let window = Window::QUERY
        .sql()
        .replace("?1", &EVENT_WINDOW_ROWS.to_string());
    let membership_changed = format!(
        "CASE WHEN OLD.{event_id} IS NEW.{event_id} AND OLD.{created} IS NEW.{created} THEN 0
         ELSE (WITH old_member AS (SELECT EXISTS (SELECT 1 FROM (SELECT {event_id} FROM (
           SELECT {event_id},{created} FROM (SELECT {event_id},{created} FROM {events}
             WHERE {event_id}!=NEW.{event_id} ORDER BY {created} DESC,{event_id} LIMIT {EVENT_WINDOW_ROWS})
           UNION ALL SELECT OLD.{event_id},OLD.{created}
         ) ORDER BY {created} DESC,{event_id} LIMIT {EVENT_WINDOW_ROWS}) e WHERE e.{event_id}=OLD.{event_id}) AS present),
         new_member AS (SELECT EXISTS (SELECT 1 FROM ({window}) e WHERE e.{event_id}=NEW.{event_id}) AS present)
         SELECT CASE WHEN OLD.{event_id} IS NEW.{event_id} THEN old_member.present != new_member.present
           ELSE old_member.present OR new_member.present END FROM old_member,new_member) END"
    );
    // These closures are local fragments of this fixed operation, never SQL entry points.
    let live_payload = |row: &str| {
        format!(
            "sync_entity_payload('{event}',{row}.{event_id},json_object('id',{row}.{event_id},'agent',{row}.{agent},'kind',{row}.{event_kind},'status',{row}.{status},'created',{row}.{created},'error',{row}.{error}),0)"
        )
    };
    let prior = |row: &str, field: &str| {
        format!(
            "(SELECT {field} FROM {publications} WHERE {kind}={entity_kind} AND {label}='{event}' AND {id}={row}.{event_id})"
        )
    };
    let tombstone = |row: &str| {
        format!(
            "{}('{event}',{row}.{event_id},{},{})",
            crate::entity_scopes::SCOPE_TOMBSTONE_FUNCTION,
            prior(row, payload),
            prior(row, deleted)
        )
    };
    let entity_key = key_sql(
        &[
            Publications::__DB_COLUMN_COLLECTION_ORDER,
            Publications::__DB_COLUMN_ENTITY_ID,
        ],
        "source_key",
    );
    let publish = |source: String| {
        let increment = format!("({rowid_high})+ordinal");
        let sequence = format!("({high})+ordinal");
        format!(
            "INSERT INTO {publications}({kind},{identity},{rowid},{seq},{deleted},{label},{collection_kind},{id},{hash},{payload},{non_transcript})
             SELECT {entity_kind},{entity_key},
               COALESCE((SELECT {rowid} FROM {publications} WHERE {kind}={entity_kind} AND {identity}={entity_key}),
                 CASE WHEN typeof({increment})='integer' THEN {increment} ELSE RAISE(ABORT,'event source rowid exhausted') END),
               CASE WHEN typeof({sequence})='integer' THEN {sequence} ELSE RAISE(ABORT,'event publication sequence exhausted') END,
               {deleted},{label},{event_code},{id},sync_entity_hash({payload}),{payload},1
             FROM ({source}) AS source_key WHERE 1
             ON CONFLICT({kind},{identity}) DO UPDATE SET {seq}=excluded.{seq},{hash}=excluded.{hash},{payload}=excluded.{payload},{deleted}=excluded.{deleted}
             WHERE {publications}.{hash}!=excluded.{hash} OR {publications}.{deleted}!=excluded.{deleted};"
        )
    };
    let live = publish(format!(
        "SELECT '{event}' AS {label},NEW.{event_id} AS {id},{} AS {payload},0 AS {deleted},1 AS ordinal",
        live_payload("NEW")
    ));
    let restore = publish(format!(
        "SELECT '{event}' AS {label},e.{event_id} AS {id},{} AS {payload},0 AS {deleted},ROW_NUMBER() OVER (ORDER BY e.{created} DESC,e.{event_id}) AS ordinal
         FROM ({window}) e WHERE NOT EXISTS (SELECT 1 FROM {publications} s WHERE s.{kind}={entity_kind} AND s.{label}='{event}' AND s.{id}=e.{event_id} AND s.{deleted}=0)",live_payload("e")
    ));
    let retire = publish(format!(
        "SELECT '{event}' AS {label},s.{id} AS {id},{}('{event}',s.{id},s.{payload},s.{deleted}) AS {payload},1 AS {deleted},ROW_NUMBER() OVER (ORDER BY s.{id}) AS ordinal
         FROM {publications} s WHERE s.{kind}={entity_kind} AND s.{label}='{event}' AND s.{deleted}=0
           AND s.{id} NOT IN (SELECT {event_id} FROM ({window}))",crate::entity_scopes::SCOPE_TOMBSTONE_FUNCTION
    ));
    let outside = publish(format!(
        "SELECT '{event}' AS {label},NEW.{event_id} AS {id},{} AS {payload},1 AS {deleted},1 AS ordinal
         WHERE NOT EXISTS (SELECT 1 FROM ({window}) e WHERE e.{event_id}=NEW.{event_id})",tombstone("NEW")
    ));
    let remove = publish(format!(
        "SELECT '{event}' AS {label},OLD.{event_id} AS {id},{} AS {payload},1 AS {deleted},1 AS ordinal",
        tombstone("OLD")
    ));
    let meta_kind = CanvasMetadataKind::DB_CODE_SYNCENTITYMETA;
    let meta_rowid = column::<Canvas, _>(Metadata::SOURCE_ROWID);
    let meta_key = column::<Canvas, _>(Metadata::TEXT_KEY);
    let meta_value = column::<Canvas, _>(Metadata::META_VALUE);
    let meta_kind_column = column::<Canvas, _>(Metadata::KIND);
    let meta_identity = column::<Canvas, _>(Metadata::IDENTITY);
    let marker_key = crate::metadata_values::EVENT_WINDOW_SEQUENCE_KEY;
    let key = key_sql(&[Metadata::__DB_COLUMN_TEXT_KEY, "null_tie"], "source_key");
    let meta_high = crate::clock_schema::queries::MetaRowidHigh::QUERY
        .sql()
        .replace("?1", meta_kind);
    let increment = format!("({meta_high})+1");
    let marker = format!(
        "INSERT INTO {metadata}({meta_kind_column},{meta_identity},{meta_rowid},{meta_key},{meta_value})
         SELECT {meta_kind},{key},COALESCE((SELECT {meta_rowid} FROM {metadata} WHERE {meta_kind_column}={meta_kind} AND {meta_identity}={key}),
           CASE WHEN typeof({increment})='integer' THEN {increment} ELSE RAISE(ABORT,'event marker source rowid exhausted') END),
           {meta_key},CAST((SELECT COALESCE(MAX({seq}),0) FROM {publications} WHERE {kind}={entity_kind} AND {label}='{event}') AS TEXT)
         FROM (SELECT '{marker_key}' AS {meta_key},NULL AS null_tie) AS source_key WHERE 1
         ON CONFLICT({meta_kind_column},{meta_identity}) DO UPDATE SET {meta_value}=excluded.{meta_value}
         WHERE {metadata}.{meta_value}!=excluded.{meta_value};"
    );
    let columns = format!("{event_id},{agent},{event_kind},{status},{created},{error}");
    let changed = [event_id, agent, event_kind, status, created, error]
        .map(|c| format!("OLD.{c} IS NOT NEW.{c}"))
        .join(" OR ");
    format!(
        "DROP TRIGGER IF EXISTS sync_entity_event_INSERT;
         DROP TRIGGER IF EXISTS sync_entity_event_UPDATE;
         DROP TRIGGER IF EXISTS sync_entity_event_UPDATE_WINDOW;
         DROP TRIGGER IF EXISTS sync_entity_event_DELETE;
         CREATE TRIGGER sync_entity_event_INSERT AFTER INSERT ON {events} BEGIN {live}{restore}{retire}{marker} END;
         CREATE TRIGGER sync_entity_event_UPDATE AFTER UPDATE OF {columns} ON {events}
           WHEN ({changed}) AND NOT ({membership_changed}) BEGIN {live}{outside}{marker} END;
         CREATE TRIGGER sync_entity_event_UPDATE_WINDOW AFTER UPDATE OF {columns} ON {events}
           WHEN {membership_changed} BEGIN {live}{restore}{retire}{marker} END;
         CREATE TRIGGER sync_entity_event_DELETE AFTER DELETE ON {events} BEGIN {remove}{restore}{retire}{marker} END;"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_schema::Metadata;
    use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
    use crate::publication_schema::{Publications, SyncEntities};
    use crate::queue_schema::EventsRow;
    use studio_db_api::{DbQuery, Table, migration::encode_key};

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(events=EventsRow),
        columns(id=EventsRow::ID,agent=EventsRow::AGENT,kind=EventsRow::KIND,text=EventsRow::TEXT,status=EventsRow::STATUS,created=EventsRow::CREATED,epoch=EventsRow::EPOCH),
        sql="INSERT INTO {events}({id},{agent},{kind},{text},{status},{created},{epoch}) VALUES(?1,'worker','monitor_exit','','pending',?2,1) ON CONFLICT({id}) DO NOTHING")]
    struct InsertReceipt<'a> {
        #[db(column=EventsRow::ID)]
        id: &'a str,
        #[db(column=EventsRow::CREATED)]
        created: f64,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,native,result=String,tables(rows=Publications),
        indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_MEMBERS),predicates(entities=rows),
        columns(id=Publications::ENTITY_ID,collection=Publications::COLLECTION_ORDER,deleted=Publications::DELETED),
        sql="SELECT {id} FROM {rows} WHERE {entities} AND {collection}=?1 AND {deleted}=0 ORDER BY {id}")]
    struct ActiveEventIds<'a> {
        #[db(column=Publications::COLLECTION_ORDER)]
        collection: Option<&'a str>,
    }

    #[test]
    fn runtime_event_insert_has_bounded_vm_work_and_indexed_trigger_lookups() {
        use crate::metadata_values::StoredBoolean;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::EntityValue;
        let mut db = crate::fresh_state::canvas_before_indexes(
            std::path::Path::new(":memory:"),
            &["sync_entity_event_INSERT"],
        )
        .unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        // Seed source history before installing its producer. Both the target
        // agent and unrelated agents have history well beyond the public window.
        for n in 1..=6000 {
            let mut row = event(&format!("history-{n:06}"), n as f64);
            if n % 2 != 0 {
                row.agent = "other".into();
            }
            canvas_tx(&tx).insert(&row).unwrap();
        }
        install(&tx);
        canvas_tx(&tx)
            .named(
                &InsertReceipt::QUERY,
                &InsertReceipt {
                    id: "warm",
                    created: 6001.0,
                },
            )
            .unwrap();
        for n in 1..5000 {
            let id = format!("history-{n:06}");
            let payload =
                crate::entity_scopes::tombstone_payload("event", &id, None, None).unwrap();
            canvas_tx(&tx)
                .insert(&SyncEntities {
                    identity: encode_key(&("event".to_owned(), id.clone())),
                    source_rowid: 10000 + n,
                    publication_seq: 10000 + n,
                    deleted: StoredBoolean::Valid(true),
                    collection_order: CollectionName::from_stored("event".into()),
                    collection_kind: CollectionKind::Event,
                    collection_agent: None,
                    entity_hash: crate::schema::sha256_hex(payload.as_bytes()),
                    entity_payload: EntityValue::from_stored(
                        "event",
                        &id,
                        StoredBoolean::Valid(true),
                        Some(payload),
                    )
                    .unwrap(),
                    entity_id: id,
                    non_transcript: true,
                })
                .unwrap();
        }
        let (_, steps) = crate::query_plan_test_support::vm_steps(&tx, || {
            canvas_tx(&tx).named(
                &InsertReceipt::QUERY,
                &InsertReceipt {
                    id: "measured",
                    created: 6002.0,
                },
            )
        })
        .unwrap();
        eprintln!("runtime event single insert vm_steps={steps}");
        assert!(
            steps < 20000,
            "single runtime event insert used {steps} VM steps"
        );
        let plan = crate::query_plan_test_support::explain(
            &tx,
            ActiveEventIds::QUERY.sql(),
            &ActiveEventIds {
                collection: Some("event"),
            },
        )
        .unwrap();
        assert!(
            plan.iter()
                .all(|line| !line.contains("SCAN ") && !line.contains("TEMP B-TREE")),
            "{plan:?}"
        );
        let index = studio_db_api::query_index::<Canvas, Publications>(
            &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_MEMBERS,
        );
        assert!(plan.iter().any(|line| line.contains(index)), "{plan:?}");
        tx.set_db_config(
            rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP,
            true,
        )
        .unwrap();
        let trigger_plan = crate::query_plan_test_support::explain(
            &tx,
            InsertReceipt::QUERY.sql(),
            &InsertReceipt {
                id: "plan-only",
                created: 6003.0,
            },
        )
        .unwrap();
        assert!(
            trigger_plan
                .iter()
                .any(|line| line.contains(index)
                    && line.contains("collection_order=? AND deleted=?")),
            "{trigger_plan:?}"
        );
        for line in &trigger_plan {
            if line.starts_with("SCAN ") {
                // Window scans stop after EVENT_WINDOW_ROWS, independently of history.
                let window = line.starts_with("SCAN runtime_events USING ")
                    && line.contains("runtime_event_created_id");
                // Derived rows/ordinals come from the same fixed window or singleton sources.
                let derived = line.starts_with("SCAN e")
                    || line.starts_with("SCAN (subquery-")
                    || line.starts_with("SCAN source_key")
                    || line == "SCAN CONSTANT ROW";
                // Scope JSON scans inspect only selectors in the keyed prior row.
                let scope = line.starts_with("SCAN json_each VIRTUAL TABLE");
                assert!(
                    window || derived || scope,
                    "unbounded trigger scan: {line}; {trigger_plan:?}"
                );
            }
            assert!(
                !line.contains("USING INDEX publications_entity_id (kind=?)"),
                "unbounded publication traversal: {trigger_plan:?}"
            );
        }
        // TEMP B-TREE is allowed only for window ordinals or json_each's last-key selection;
        // the ordered publication lookup above must have neither a scan nor a temporary sort.
    }

    #[test]
    fn runtime_event_retirement_index_preserves_publication_semantics() {
        let run = |old_index: bool| {
            let excluded = if old_index {
                vec![
                    "sync_entity_event_INSERT",
                    "publications_entity_collection_members",
                ]
            } else {
                vec!["sync_entity_event_INSERT"]
            };
            let mut db = crate::fresh_state::canvas_before_indexes(
                std::path::Path::new(":memory:"),
                &excluded,
            )
            .unwrap();
            crate::schema::register_python_trigger_functions(&db).unwrap();
            let tx = db.transaction().unwrap();
            for n in 0..205 {
                canvas_tx(&tx)
                    .upsert(&event(&format!("e{n:04}"), (n / 2) as f64))
                    .unwrap();
            }
            install(&tx);
            for (id, created) in [
                ("warm", 300.0),
                ("new", 301.0),
                ("old", -1.0),
                ("tie", 300.0),
            ] {
                canvas_tx(&tx).upsert(&event(id, created)).unwrap();
            }
            // A duplicate receipt remains a no-op; deleting a member restores its predecessor.
            assert!(!canvas_tx(&tx).insert_new(&event("new", 301.0)).unwrap());
            canvas_tx(&tx)
                .delete::<EventsRow>(&("new".to_owned(),))
                .unwrap();
            assert_eq!(entity(&tx, "old").deleted.as_stored(), 1);
            assert_eq!(entity(&tx, "tie").deleted.as_stored(), 0);
            let mut rows = canvas_read(&tx)
                .collect(&SyncEntities::SELECT_QUERY, &())
                .unwrap()
                .into_iter()
                .map(|row| {
                    (
                        row.entity_id,
                        row.source_rowid,
                        row.publication_seq,
                        row.deleted.as_stored(),
                        row.entity_hash,
                        row.entity_payload.value().raw().map(str::to_owned),
                    )
                })
                .collect::<Vec<_>>();
            rows.sort_by(|left, right| left.0.cmp(&right.0));
            assert_eq!(
                rows.iter().filter(|row| row.3 == 0).count(),
                EVENT_WINDOW_ROWS
            );
            (rows, marker(&tx))
        };
        assert_eq!(run(true), run(false));
    }

    fn fixture() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            Publications::descriptor(),
            Metadata::descriptor(),
            crate::change_schema::Changes::descriptor(),
            EventsRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        crate::schema::register_python_trigger_functions(&db).unwrap();
        db
    }

    fn install(tx: &rusqlite::Transaction<'_>) {
        tx.execute_batch(&trigger_sql()).unwrap();
    }

    fn event(id: &str, created: f64) -> EventsRow {
        EventsRow {
            id: id.into(),
            agent: "worker".into(),
            kind: studio_runtime_schema::persistence::EventKind::User,
            text: "hidden text".into(),
            status: studio_runtime_schema::persistence::EventStatus::Pending,
            created,
            epoch: 1,
            turn_id: None,
            error: None,
            receipt_enum_guard: None,
        }
    }

    fn entity(db: &rusqlite::Connection, id: &str) -> SyncEntities {
        canvas_read(db)
            .get::<SyncEntities>(&(encode_key(&("event".to_owned(), id.to_owned())),))
            .unwrap()
            .unwrap()
    }

    fn marker(db: &rusqlite::Connection) -> i64 {
        crate::clock_schema::SyncEntityMeta::value(
            db,
            crate::metadata_values::EVENT_WINDOW_SEQUENCE_KEY,
        )
        .unwrap()
        .unwrap()
        .parse()
        .unwrap()
    }

    #[test]
    fn event_publication_insert_noop_payload_source_rowid_and_refresh() {
        let mut db = fixture();
        let tx = db.transaction().unwrap();
        install(&tx);
        let row = event("a", 1.5);
        canvas_tx(&tx).upsert(&row).unwrap();
        let first = entity(&tx, "a");
        let raw = r#"{"collection":"event","id":"a","value":{"agent":"worker","created":1.5,"error":null,"id":"a","kind":"user","status":"pending"}}"#;
        assert_eq!(first.entity_payload.value().raw(), Some(raw));
        assert_eq!(first.entity_hash, crate::schema::sha256_hex(raw.as_bytes()));
        assert_eq!(first.publication_seq, 1);
        assert_eq!(marker(&tx), 1);
        install(&tx);
        canvas_tx(&tx).upsert(&row).unwrap();
        let unchanged = entity(&tx, "a");
        assert_eq!(
            (unchanged.publication_seq, unchanged.source_rowid),
            (1, first.source_rowid)
        );
        let mut changed = row;
        changed.text = "not a publication field".into();
        canvas_tx(&tx).update_existing(&changed).unwrap();
        assert_eq!(entity(&tx, "a").publication_seq, 1);
        changed.error = Some("exact error".into());
        canvas_tx(&tx).update_existing(&changed).unwrap();
        let after = entity(&tx, "a");
        assert_eq!(after.source_rowid, first.source_rowid);
        assert_eq!(after.publication_seq, 2);
        assert_eq!(marker(&tx), 2);
        tx.rollback().unwrap();
        assert!(
            canvas_read(&db)
                .get::<SyncEntities>(&(encode_key(&("event".to_owned(), "a".to_owned())),))
                .unwrap()
                .is_none()
        );
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas, mode=write, cardinality=none, result=(), tables(events=EventsRow), columns(id=EventsRow::ID, created=EventsRow::CREATED), sql="UPDATE {events} SET {id}=?1,{created}=?2 WHERE {id}=?3")]
    struct Move<'a> {
        #[db(column=EventsRow::ID)]
        next: &'a str,
        #[db(column=EventsRow::CREATED)]
        created: f64,
        #[db(column=EventsRow::ID)]
        prior: &'a str,
    }

    #[test]
    fn event_publication_membership_eviction_ties_and_deletes() {
        let mut db = fixture();
        let tx = db.transaction().unwrap();
        install(&tx);
        for n in 0..=EVENT_WINDOW_ROWS {
            canvas_tx(&tx)
                .upsert(&event(&format!("e{n:04}"), 1.5))
                .unwrap();
        }
        let boundary = format!("e{EVENT_WINDOW_ROWS:04}");
        assert_eq!(entity(&tx, &boundary).deleted.as_stored(), 1);
        let before = entity(&tx, "e0000");
        canvas_tx(&tx)
            .named(
                &Move::QUERY,
                &Move {
                    next: "moved",
                    created: 0.5,
                    prior: "e0000",
                },
            )
            .unwrap();
        assert_eq!(entity(&tx, "e0000").deleted.as_stored(), 1);
        assert_eq!(entity(&tx, "moved").deleted.as_stored(), 1);
        assert_eq!(entity(&tx, &boundary).deleted.as_stored(), 0);
        let retired = entity(&tx, "e0000");
        let expected = crate::entity_scopes::tombstone_payload(
            "event",
            "e0000",
            before.entity_payload.value().raw(),
            Some(false),
        )
        .unwrap();
        assert_eq!(
            retired.entity_payload.value().raw(),
            Some(expected.as_str())
        );
        assert_eq!(
            retired.entity_hash,
            crate::schema::sha256_hex(expected.as_bytes())
        );
        assert!(retired.publication_seq < entity(&tx, "moved").publication_seq);
        let mut nonmember = event("moved", 0.5);
        nonmember.error = Some("nonmember update".into());
        canvas_tx(&tx).update_existing(&nonmember).unwrap();
        assert_eq!(entity(&tx, "moved").deleted.as_stored(), 1);
        canvas_tx(&tx)
            .named(
                &Move::QUERY,
                &Move {
                    next: "moved",
                    created: 2.5,
                    prior: "moved",
                },
            )
            .unwrap();
        assert_eq!(entity(&tx, "moved").deleted.as_stored(), 0);
        assert_eq!(entity(&tx, &boundary).deleted.as_stored(), 1);
        canvas_tx(&tx)
            .delete::<EventsRow>(&("moved".to_owned(),))
            .unwrap();
        assert_eq!(entity(&tx, &boundary).deleted.as_stored(), 0);
        let high = marker(&tx);
        canvas_tx(&tx)
            .delete::<EventsRow>(&("e0000".to_owned(),))
            .unwrap();
        assert_eq!(marker(&tx), high);
    }

    #[test]
    fn event_publication_shared_high_ordering_and_nonmember_delete() {
        use crate::metadata_values::StoredBoolean;
        let mut db = fixture();
        let tx = db.transaction().unwrap();
        let mut typed = canvas_tx(&tx);
        typed
            .partition::<crate::metadata_schema::SyncGeneration>()
            .put(&crate::metadata_schema::SyncGeneration {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                sync_generation: 30,
            })
            .unwrap();
        typed
            .partition::<crate::publication_schema::SyncDocuments>()
            .put(&crate::publication_schema::SyncDocuments {
                identity: encode_key(&(40_i64,)),
                source_rowid: 40,
                publication_seq: 40,
                deleted: StoredBoolean::Valid(false),
                document_scope: "drafts".into(),
                document_id: "d:s".into(),
                document_payload: crate::publication_schema::DocumentValue::from_stored(
                    "drafts",
                    "d:s",
                    r#"{"session":"s","device":"d","text":"a","updated":1.75}"#.into(),
                )
                .unwrap(),
            })
            .unwrap();
        typed
            .partition::<crate::change_schema::DurableRows>()
            .put(&crate::change_schema::DurableRows {
                identity: encode_key(&("runtime_work".to_owned(), "x".to_owned())),
                source_rowid: 1,
                source_label: crate::change_schema::DurableSource::from_stored(
                    "runtime_work".into(),
                ),
                durable_id: "x".into(),
                sequence: 50,
                deleted: StoredBoolean::Valid(false),
                deleted_at: 0,
                scope: None,
            })
            .unwrap();
        // Pre-existing history exercises multiple restorations in one trigger.
        for n in 0..EVENT_WINDOW_ROWS {
            typed.upsert(&event(&format!("e{n:04}"), n as f64)).unwrap();
        }
        install(&tx);
        typed
            .upsert(&event("latest", EVENT_WINDOW_ROWS as f64))
            .unwrap();
        assert_eq!(entity(&tx, "latest").publication_seq, 51);
        assert!(
            entity(&tx, &format!("e{:04}", EVENT_WINDOW_ROWS - 1)).publication_seq
                < entity(&tx, "e0001").publication_seq
        );
        assert_eq!(
            entity(&tx, "e0001").publication_seq,
            50 + EVENT_WINDOW_ROWS as i64
        );
        let high = marker(&tx);
        typed.delete::<EventsRow>(&("e0000".to_owned(),)).unwrap();
        let tombstone = entity(&tx, "e0000");
        assert_eq!(tombstone.publication_seq, high + 1);
        assert_eq!(tombstone.deleted.as_stored(), 1);
        assert_eq!(marker(&tx), high + 1);
    }

    #[test]
    fn event_publication_window_plan_and_bounded_history_cost() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut measurements = Vec::new();
        for history in [500, 5000] {
            let mut db = fixture();
            let tx = db.transaction().unwrap();
            for n in 0..history {
                canvas_tx(&tx)
                    .upsert(&event(&format!("e{n:05}"), n as f64))
                    .unwrap();
            }
            install(&tx);
            canvas_tx(&tx)
                .upsert(&event("first", history as f64))
                .unwrap();
            assert_eq!(entity(&tx, "first").deleted.as_stored(), 0);
            let params = Window {
                limit: EVENT_WINDOW_ROWS as i64,
            };
            let plan = crate::query_plan_test_support::explain(&tx, Window::QUERY.sql(), &params)
                .unwrap()
                .join("\n");
            assert!(
                plan.contains(studio_db_api::query_index::<Canvas, EventsRow>(
                    &EventsRow::INDEX_RUNTIME_EVENT_CREATED_ID
                )),
                "{plan}"
            );
            assert!(!plan.contains("TEMP B-TREE"), "{plan}");
            tx.set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_TRIGGER_EQP,
                true,
            )
            .unwrap();
            let trigger_plan = crate::query_plan_test_support::explain(
                &tx,
                Move::QUERY.sql(),
                &Move {
                    next: "first",
                    created: history as f64 + 0.5,
                    prior: "first",
                },
            )
            .unwrap();
            let event_scans = trigger_plan
                .iter()
                .filter(|line| line.contains(&format!("SCAN {}", EventsRow::NAME)))
                .collect::<Vec<_>>();
            let index = studio_db_api::query_index::<Canvas, EventsRow>(
                &EventsRow::INDEX_RUNTIME_EVENT_CREATED_ID,
            );
            assert!(!event_scans.is_empty(), "{trigger_plan:?}");
            assert!(
                event_scans.iter().all(|line| line.contains(index)),
                "{trigger_plan:?}"
            );
            eprintln!("actual UPDATE trigger event plans: {event_scans:?}");
            let steps = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&steps);
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            let mut next = event("next", history as f64 + 1.0);
            canvas_tx(&tx).upsert(&next).unwrap();
            let insert_steps = steps.swap(0, Ordering::Relaxed);
            next.created += 0.5;
            canvas_tx(&tx).update_existing(&next).unwrap();
            let update_steps = steps.load(Ordering::Relaxed);
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            eprintln!(
                "event history={history} insert_vm_steps={insert_steps} update_vm_steps={update_steps}; plan={plan}"
            );
            measurements.push((insert_steps, update_steps));
        }
        assert!(
            measurements[1].0 <= measurements[0].0 + 1000
                && measurements[1].1 <= measurements[0].1 + 1000,
            "history VM steps: {measurements:?}"
        );
    }
}
