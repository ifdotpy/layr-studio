//! `peerTeam` documents are built at read time (`entity_scope` in runtime-services). The
//! visible teams move from their copies in `sync_entities` to the window
//! `durable_seq::PEER_TEAM_WINDOW`, then the copies go. The counter keeps every number
//! already used (#630 item 127).
use rusqlite::Transaction;

use crate::StoreResult;

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let visible = {
        let mut statement = tx.prepare(
            "SELECT id,json_extract(payload,'$.value') FROM sync_entities
             WHERE collection='peerTeam' AND deleted=0 ORDER BY seq",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if visible.is_empty()
        && !tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='peerTeam')",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Ok(());
    }
    crate::durable_seq::keep_counter(tx)?;
    for (id, view) in visible {
        // The hash of the view the client holds: a later change takes a new number.
        let hash = crate::sha256_hex(view.unwrap_or_default().as_bytes());
        crate::durable_seq::set_window_view(
            tx,
            crate::durable_seq::PEER_TEAM_WINDOW,
            &id,
            Some(&hash),
        )?;
    }
    tx.execute("DELETE FROM sync_entities WHERE collection='peerTeam'", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_teams_join_the_window_and_copies_go() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('peerTeam','t',999999,'h','{}',0),('peerTeam','gone',7,'h',NULL,1)",
            [],
        )
        .expect("old copies");
        apply(&tx).expect("migration applies");
        apply(&tx).expect("migration repeats");
        let count = |sql: &str| -> i64 { tx.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM sync_entities WHERE collection='peerTeam'"),
            0
        );
        assert_eq!(
            crate::durable_seq::window_members(&tx, crate::durable_seq::PEER_TEAM_WINDOW).unwrap(),
            ["t"]
        );
        assert!(
            count("SELECT seq FROM durable_rows WHERE tbl='peer_team_window' AND id='t'") > 999_999
        );
    }

    #[test]
    fn first_changed_view_after_migration_gets_a_new_sequence() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        let view = studio_json::json!({"id":"team","members":["a","b"],"revision":1});
        let payload = studio_json::json!({"collection":"peerTeam","id":"team","value":view});
        tx.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             VALUES('peerTeam','team',999999,'old',?1,0)",
            [payload.to_string()],
        )
        .expect("old team");
        apply(&tx).expect("migration");
        let window = crate::durable_seq::PEER_TEAM_WINDOW;
        let sequence = || -> i64 {
            tx.query_row(
                "SELECT seq FROM durable_rows WHERE tbl=?1 AND id='team'",
                [window],
                |row| row.get(0),
            )
            .unwrap()
        };
        let migrated = sequence();
        let previous_hash = crate::sha256_hex(&studio_json::to_vec(&view).unwrap());
        assert!(
            !crate::durable_seq::set_window_view(&tx, window, "team", Some(&previous_hash),)
                .unwrap()
        );
        assert_eq!(sequence(), migrated, "the same view keeps its number");

        let changed = studio_json::json!({"id":"team","members":["a","c"],"revision":2});
        let changed_hash = crate::sha256_hex(&studio_json::to_vec(&changed).unwrap());
        assert!(
            crate::durable_seq::set_window_view(&tx, window, "team", Some(&changed_hash),).unwrap()
        );
        assert!(
            sequence() > migrated,
            "the first change reaches existing cursors"
        );
        let changed_sequence = sequence();
        apply(&tx).expect("migration repeats");
        assert_eq!(sequence(), changed_sequence);
    }
}
