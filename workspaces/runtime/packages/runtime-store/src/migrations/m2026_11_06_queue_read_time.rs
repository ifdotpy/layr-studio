//! `queue` documents are built at read time (`entity_scope` in runtime-services). The
//! agents with a live copy move to `durable_seq::QUEUE_WINDOW` with the revision the client
//! holds, so a later queue change takes a new number, and the copies go. The counter keeps
//! every number already used (#630 item 127).
use rusqlite::Transaction;

use crate::StoreResult;

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let visible = {
        let mut statement = tx.prepare(
            "SELECT id,json_extract(payload,'$.value.revision') FROM sync_entities
             WHERE collection='queue' AND deleted=0 ORDER BY seq",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='queue')",
        [],
        |row| row.get(0),
    )?;
    if !copies {
        return Ok(());
    }
    crate::durable_seq::keep_counter(tx)?;
    for (id, revision) in visible {
        crate::durable_seq::set_window_view(
            tx,
            crate::durable_seq::QUEUE_WINDOW,
            &id,
            Some(revision.as_deref().unwrap_or_default()),
        )?;
    }
    tx.execute("DELETE FROM sync_entities WHERE collection='queue'", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_revision_takes_a_new_number_and_the_same_one_does_not() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('queue','a',999999,'h','{\"value\":{\"revision\":\"r1\"}}',0),
               ('queue','gone',7,'h',NULL,1)",
            [],
        )
        .expect("old copies");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        let window = crate::durable_seq::QUEUE_WINDOW;
        assert_eq!(
            crate::durable_seq::window_members(&tx, window).unwrap(),
            ["a"]
        );
        let seq = || -> i64 {
            tx.query_row(
                "SELECT seq FROM durable_rows WHERE tbl='queue_window' AND id='a'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        let first = seq();
        assert!(first > 999_999, "a new number is above every used number");
        assert!(!crate::durable_seq::set_window_view(&tx, window, "a", Some("r1")).unwrap());
        assert_eq!(seq(), first);
        assert!(crate::durable_seq::set_window_view(&tx, window, "a", Some("r2")).unwrap());
        assert!(seq() > first);

        let changed = seq();
        assert!(crate::durable_seq::set_window_view(&tx, window, "a", None).unwrap());
        assert!(!crate::durable_seq::in_window(&tx, window, "a").unwrap());
        let removed = seq();
        assert!(removed > changed, "removal produces a newer tombstone");
        assert!(!crate::durable_seq::set_window_view(&tx, window, "a", None).unwrap());
        assert_eq!(seq(), removed, "repeated removal keeps its tombstone");

        assert!(crate::durable_seq::set_window_view(&tx, window, "a", Some("r2")).unwrap());
        assert!(crate::durable_seq::in_window(&tx, window, "a").unwrap());
        assert!(seq() > removed, "a restored queue follows its tombstone");
    }
}
