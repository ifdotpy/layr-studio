//! Complaints, chat edges and chats get their client documents at read time
//! (`entity_scope` in runtime-services). Number the complaint and edge rows in
//! `durable_rows`, mark every chat, and delete the old copies in `sync_entities`. The
//! counter keeps every number already used, so no cursor repeats a number.
use rusqlite::Transaction;

use crate::StoreResult;

const COLLECTIONS: &str = "'complaint','edge','chat'";

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let copies: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ({COLLECTIONS}))"),
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
    }
    // A new number for each existing row; the triggers number later writes.
    crate::durable_seq::install(tx)?;
    if crate::schema::table_exists(tx, "groups")? {
        let chats = tx
            .prepare("SELECT id FROM groups")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for chat in chats {
            crate::durable_seq::mark(tx, crate::durable_seq::CHAT_VIEW_MARK, &chat)?;
        }
    }
    if copies {
        tx.execute_batch(&format!(
            "DELETE FROM sync_entities WHERE collection IN ({COLLECTIONS})"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_go_and_rows_and_chats_get_numbers_above_them() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES('chat','g',999999,'h','{}',0),('workspace','current',5,'h','{}',0)",
            [],
        )
        .expect("old copies");
        tx.execute_batch(
            "DROP TRIGGER IF EXISTS durable_graph_edges_insert;
             INSERT INTO groups(id,name,members) VALUES('g','Chat','[]');
             INSERT INTO graph_edges(id,source,target,kind) VALUES('e','a','g','chat');",
        )
        .expect("rows written before the source");
        apply(&tx).expect("migration applies");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count(
                "SELECT COUNT(*) FROM sync_entities WHERE collection IN ('chat','edge','complaint')"
            ),
            0
        );
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='workspace'"),
            1
        );
        assert!(count("SELECT seq FROM durable_rows WHERE tbl='graph_edges' AND id='e'") > 999_999);
        assert!(count("SELECT seq FROM durable_rows WHERE tbl='chat_views' AND id='g'") > 999_999);
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
