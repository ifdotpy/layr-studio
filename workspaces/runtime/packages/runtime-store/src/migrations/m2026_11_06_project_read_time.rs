//! `project` documents are built at read time (`entity_scope` in runtime-services): a
//! local project from `runtime_projects` and the project grants, a peer project from
//! `peer_projects`, the mirror of a paired server's list. Move the mirrored peer projects
//! out of `sync_entities`, number the rows in `durable_rows`, and delete the old copies.
//! The counter keeps every number already used (#630 item 127).
use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS peer_projects (id TEXT PRIMARY KEY, record TEXT NOT NULL);",
    )?;
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    tx.execute(
        "INSERT OR IGNORE INTO peer_projects(id,record)
           SELECT id,json_extract(payload,'$.value') FROM sync_entities
           WHERE collection='project' AND deleted=0 AND substr(id,1,5)='peer:'
             AND json_valid(payload) AND json_type(payload,'$.value')='object'",
        [],
    )?;
    crate::durable_seq::install(tx)?;
    let copies: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='project')",
        [],
        |row| row.get(0),
    )?;
    if copies {
        crate::durable_seq::keep_counter(tx)?;
        tx.execute("DELETE FROM sync_entities WHERE collection='project'", [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_projects_move_and_copies_go() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('project','peer:s:/p',999999,'h','{\"value\":{\"id\":\"peer:s:/p\",\"homeServerId\":\"s\"}}',0),
               ('project','/local',5,'h','{\"value\":{\"id\":\"/local\"}}',0),
               ('project','peer:s:/gone',6,'h','{}',1)",
            [],
        )
        .expect("old copies");
        tx.execute(
            "INSERT INTO runtime_projects(id,record) VALUES('/local','{\"id\":\"/local\"}')",
            [],
        )
        .expect("local project");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='project'"),
            0
        );
        assert_eq!(count("SELECT COUNT(*) FROM peer_projects"), 1);
        assert_eq!(
            count("SELECT json_extract(record,'$.homeServerId')='s' FROM peer_projects"),
            1
        );
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='runtime_projects' AND id='/local'")
                > 999_999,
            "a new number is above every used number"
        );
    }
}
