//! `terminal` documents are built at read time from `user_terminals` (`entity_scope` in
//! runtime-services). Number the rows in `durable_rows` and delete the old copies. The
//! counter keeps every number already used (#630 item 127).
use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    crate::durable_seq::install(tx)?;
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='terminal')",
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute("DELETE FROM sync_entities WHERE collection='terminal'", [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_go_and_rows_keep_their_numbers_above_the_old_ones() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             VALUES('terminal','shell',999999,'h','{}',0)",
            [],
        )
        .expect("old copy");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        tx.execute(
            "INSERT INTO user_terminals(id,record,output,offset) VALUES('shell','{\"id\":\"shell\"}','',0)",
            [],
        )
        .expect("terminal row");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='terminal'"),
            0
        );
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='user_terminals' AND id='shell'")
                > 999_999,
            "a new number is above every used number"
        );
    }
}
