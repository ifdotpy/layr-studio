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
    ) -> StoreResult<()> {
        use crate::clock_schema::queries::{High, NumberItem};
        let high = tx.read().named(&High::QUERY, &High)?;
        tx.named(&NumberItem::QUERY, &NumberItem { rowid: *key, high })?;
        Ok(())
    }

    fn remove(
        &self,
        _: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
        _: &i64,
    ) -> StoreResult<()> {
        Ok(())
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
            db.prepare("SELECT seq FROM durable_rows WHERE tbl='runtime_items' ORDER BY seq")
                .expect("query")
                .query_map([], |row| row.get(0))
                .expect("rows")
                .collect::<Result<_, _>>()
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
