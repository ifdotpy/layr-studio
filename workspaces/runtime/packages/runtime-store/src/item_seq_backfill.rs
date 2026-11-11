//! Online numbering of the rows of `runtime_items` that predate its `seq` column.
//! The installer adds the column and the triggers (`durable_seq`); this migration gives
//! each old row its number in small batches, so the single writer keeps serving commands.

use rusqlite::{Connection, OptionalExtension};

use crate::StoreResult;
use crate::online_migration::OnlineMigration;

pub struct ItemSeqBackfill;

impl OnlineMigration for ItemSeqBackfill {
    type Key = i64;
    type Row = ();
    type Out = ();
    const NAME: &'static str = "runtime_items_seq";
    const SOURCE_TABLE: &'static str = "runtime_items";
    const SOURCE_KEYS: &'static [&'static str] = &["rowid"];

    fn read_after(
        &self,
        db: &Connection,
        after: Option<&i64>,
        limit: usize,
    ) -> StoreResult<Vec<(i64, ())>> {
        let sql = if after.is_some() {
            "SELECT rowid FROM runtime_items WHERE rowid>?1 ORDER BY rowid LIMIT ?2"
        } else {
            "SELECT rowid FROM runtime_items ORDER BY rowid LIMIT ?2"
        };
        Ok(db
            .prepare_cached(sql)?
            .query_map(rusqlite::params![after, limit as i64], |row| {
                Ok((row.get(0)?, ()))
            })?
            .collect::<Result<_, _>>()?)
    }

    fn read_one(&self, db: &Connection, key: &i64) -> StoreResult<Option<()>> {
        Ok(db
            .query_row("SELECT 1 FROM runtime_items WHERE rowid=?1", [key], |_| {
                Ok(())
            })
            .optional()?)
    }

    fn transform(&self, row: ()) -> StoreResult<()> {
        Ok(row)
    }

    /// A row that already has a number is left alone, so a re-copy of a changed key does
    /// not touch the source table again.
    fn write(
        &self,
        tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
        key: &i64,
        _: (),
    ) -> StoreResult<i64> {
        use crate::clock_schema::queries::{ChangeRowidHigh, High, ItemIdByRowid, NumberItem};
        use studio_db_api::Table;
        let Some(id) = tx
            .read()
            .named(&ItemIdByRowid::QUERY, &ItemIdByRowid { rowid: *key })?
        else {
            return Ok(0);
        };
        let table = crate::item_monitor_terminal::ItemsRow::NAME;
        let identity = studio_db_api::migration::encode_key(&(table.to_owned(), id));
        if tx
            .read()
            .get::<crate::change_schema::DurableRows>(&(identity.clone(),))?
            .is_some()
        {
            return Ok(0);
        }
        let high = tx.read().named(
            &High::QUERY,
            &High {
                generation_kind: crate::consolidation_kinds::CanvasMetadataKind::SyncGeneration,
            },
        )?;
        high.checked_add(1)
            .ok_or(crate::StoreError::RecordContract("item sequence exhausted"))?;
        let kind = crate::consolidation_kinds::CanvasChangesKind::DurableRows;
        let source_rowid = tx
            .read()
            .named(&ChangeRowidHigh::QUERY, &ChangeRowidHigh { kind })?
            .checked_add(1)
            .ok_or(crate::StoreError::RecordContract(
                "durable source rowid exhausted",
            ))?;
        tx.named(
            &NumberItem::QUERY,
            &NumberItem {
                rowid: *key,
                high,
                kind,
                identity: &identity,
                source_rowid,
                table,
            },
        )?;
        Ok(0)
    }

    fn remove(
        &self,
        _: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
        _: &i64,
    ) -> StoreResult<i64> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online_migration::{Step, SystemClock, begin, step};

    #[test]
    fn resumed_backfill_uses_a_rowid_search() {
        thread_local! { static SQL: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_items(id TEXT); INSERT INTO runtime_items(rowid,id) VALUES(-1,'negative'),(0,'zero'),(20,'later')").unwrap();
        db.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Stmt(statement, sql) = event
                    && sql.starts_with("SELECT rowid FROM runtime_items")
                {
                    SQL.with(|saved| *saved.borrow_mut() = statement.expanded_sql().unwrap());
                }
            }),
        );
        assert_eq!(
            ItemSeqBackfill.read_after(&db, Some(&0), 1).unwrap(),
            vec![(20, ())]
        );
        db.trace_v2(rusqlite::trace::TraceEventCodes::empty(), None);
        let sql = SQL.with(|saved| saved.borrow().clone());
        let details = db
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("SEARCH") && detail.contains("rowid>")),
            "{details:?}"
        );
        assert_eq!(
            ItemSeqBackfill.read_after(&db, None, 1).unwrap(),
            vec![(-1, ())]
        );
        // Existing rows can be numbered after the F1 source tables retire.
        use studio_db_api::Table;
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::change_schema::Changes::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let transaction = db.unchecked_transaction().unwrap();
        let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
        tx.partition::<crate::metadata_schema::SyncGeneration>()
            .put(&crate::metadata_schema::SyncGeneration {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                sync_generation: 0,
            })
            .unwrap();
        ItemSeqBackfill.write(&mut tx, &-1, ()).unwrap();
        ItemSeqBackfill.write(&mut tx, &20, ()).unwrap();
        assert_eq!(crate::durable_seq::high(&transaction).unwrap(), 2);
        ItemSeqBackfill.write(&mut tx, &-1, ()).unwrap();
        assert_eq!(
            crate::durable_seq::high(&transaction).unwrap(),
            2,
            "numbered rows keep their original number"
        );
        assert_eq!(
            tx.partition::<crate::change_schema::DurableRows>()
                .list()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn transcript_upsert_replays_while_item_backfill_tracking_is_active() {
        assert_transcript_upsert_replays(false);
    }

    #[test]
    fn resumed_backfill_refreshes_legacy_tracking_without_losing_progress() {
        assert_transcript_upsert_replays(true);
    }

    fn assert_transcript_upsert_replays(legacy: bool) {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        let original = studio_json::json!({"id":"agent:item","role":"assistant","text":"before"});
        crate::repositories::transcript::TranscriptRepository::put_item(
            &tx,
            "agent:item",
            "agent",
            &original,
            1.0,
            "before",
            false,
        )
        .expect("insert transcript item");
        begin(&tx, &ItemSeqBackfill).expect("begin tracking");
        if legacy {
            // Leave an existing dirty key, as an interrupted old binary would.
            crate::repositories::transcript::TranscriptRepository::put_item(
                &tx,
                "agent:item",
                "agent",
                &original,
                1.0,
                "before",
                false,
            )
            .expect("dirty existing item");
            tx.execute_batch(
                "DROP TRIGGER online_runtime_items_seq_ins;
                 DROP TRIGGER online_runtime_items_seq_upd;
                 DROP TRIGGER online_runtime_items_seq_del;
                 CREATE TRIGGER online_runtime_items_seq_ins AFTER INSERT ON runtime_items BEGIN
                   INSERT OR IGNORE INTO runtime_online_migration_dirty(migration,key) VALUES('online:runtime_items_seq',NEW.rowid);
                   UPDATE runtime_payload_migrations SET copy_source_rows=copy_source_rows+1 WHERE name='online:runtime_items_seq' AND copy_source_rows IS NOT NULL;
                 END;
                 CREATE TRIGGER online_runtime_items_seq_upd AFTER UPDATE ON runtime_items BEGIN
                   INSERT OR IGNORE INTO runtime_online_migration_dirty(migration,key) VALUES('online:runtime_items_seq',OLD.rowid);
                   INSERT OR IGNORE INTO runtime_online_migration_dirty(migration,key) VALUES('online:runtime_items_seq',NEW.rowid);
                 END;
                 CREATE TRIGGER online_runtime_items_seq_del AFTER DELETE ON runtime_items BEGIN
                   INSERT OR IGNORE INTO runtime_online_migration_dirty(migration,key) VALUES('online:runtime_items_seq',OLD.rowid);
                   UPDATE runtime_payload_migrations SET copy_source_rows=copy_source_rows-1 WHERE name='online:runtime_items_seq' AND copy_source_rows IS NOT NULL;
                 END;"
            ).expect("legacy binary trigger definitions");
        }
        let before = crate::online_migration::progress(&tx).expect("before resume");
        let dirty_before = studio_db_api::db_query!(
            crate::peer_update_settings::canvas_read(&tx),
            crate::metadata_schema::OnlineMigrationDirty
        )
        .expect("dirty before resume");
        begin(&tx, &ItemSeqBackfill).expect("resume incomplete migration");
        assert_eq!(crate::online_migration::progress(&tx).unwrap(), before);
        let dirty_after = studio_db_api::db_query!(
            crate::peer_update_settings::canvas_read(&tx),
            crate::metadata_schema::OnlineMigrationDirty
        )
        .expect("dirty after resume");
        assert_eq!(dirty_before.len(), dirty_after.len());
        for row in &dirty_before {
            assert!(
                dirty_after
                    .iter()
                    .any(|after| after.dirty_migration == row.dirty_migration
                        && after.dirty_key == row.dirty_key)
            );
        }
        let updated = studio_json::json!({"id":"agent:item","role":"assistant","text":"after"});
        crate::repositories::transcript::TranscriptRepository::put_item(
            &tx,
            "agent:item",
            "agent",
            &updated,
            1.0,
            "after",
            false,
        )
        .expect("upsert existing transcript while tracking");
        assert_eq!(
            crate::online_migration::progress(&tx).expect("progress")[0].dirty,
            1
        );
        while step(&tx, &ItemSeqBackfill, &SystemClock::new()).expect("replay") == Step::Progress {}
        let progress = crate::online_migration::progress(&tx).expect("completed progress");
        assert!(progress[0].complete);
        assert_eq!(progress[0].dirty, 0);
        begin(&tx, &ItemSeqBackfill).expect("completed migration remains complete");
        assert_eq!(crate::online_migration::progress(&tx).unwrap(), progress);
        crate::repositories::transcript::TranscriptRepository::put_item(
            &tx,
            "agent:item",
            "agent",
            &updated,
            1.0,
            "after",
            false,
        )
        .expect("completed migration has no tracking");
        assert_eq!(crate::online_migration::progress(&tx).unwrap()[0].dirty, 0);
        tx.commit().expect("commit replayed transcript");
    }

    #[test]
    fn old_items_get_increasing_entries_and_numbered_ones_keep_theirs() {
        let mut db = crate::fresh_state::test_canvas();
        // Items written before the source existed have no entry.
        db.execute_batch(
            "DROP TRIGGER durable_runtime_items_insert;
             INSERT INTO runtime_items(id,agent,record,created) VALUES('a','x','{}',1),('b','x','{}',2);",
        )
        .expect("old rows");
        let seqs = |db: &Connection| -> Vec<i64> {
            use crate::change_schema::{DurableRows, DurableSource};
            studio_db_api::db_query!(
                crate::peer_update_settings::canvas_read(db),
                DurableRows,
                scalars(DurableRows::SEQUENCE),
                order_by(DurableRows::SEQUENCE.asc),
                filter(
                    DurableRows::SOURCE_LABEL
                        .eq(&DurableSource::from_stored("runtime_items".into()))
                )
            )
            .expect("seqs")
        };
        assert!(seqs(&db).is_empty());
        let tx = db.transaction().expect("transaction");
        begin(&tx, &ItemSeqBackfill).expect("begin");
        while step(&tx, &ItemSeqBackfill, &SystemClock::new()).expect("step") == Step::Progress {}
        tx.commit().expect("commit");
        let numbered = seqs(&db);
        assert!(
            numbered.len() == 2 && numbered[0] > 0 && numbered[1] > numbered[0],
            "{numbered:?}"
        );
        // A repeat changes nothing.
        let tx = db.transaction().unwrap();
        ItemSeqBackfill
            .write(&mut crate::peer_update_settings::canvas_tx(&tx), &1, ())
            .expect("repeat");
        tx.commit().unwrap();
        assert_eq!(seqs(&db), numbered);
    }
}
