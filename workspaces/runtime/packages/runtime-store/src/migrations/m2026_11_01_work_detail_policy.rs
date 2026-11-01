use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")?
        || !crate::schema::table_exists(tx, "sync_entity_meta")?
    {
        return Ok(());
    }
    let stale: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection='workDetail' AND deleted=0 AND (json_type(payload,'$.value.policy.blockedBy') IS NULL OR json_type(payload,'$.value.policy.displayStatus') IS NULL OR json_type(payload,'$.value.policy.gateOverrideRequired') IS NULL OR json_type(payload,'$.value.policy.acceptAvailable') IS NULL))", [], |row| row.get(0))?;
    if stale {
        tx.execute(
            "DELETE FROM sync_entities WHERE collection='workDetail'",
            [],
        )?;
        tx.execute(
            "DELETE FROM sync_entity_meta WHERE key=?1",
            [crate::WORK_DETAILS_SEED_MARKER],
        )?;
    }
    let stale_background: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sync_entities WHERE collection IN ('task','monitor') AND deleted=0 AND json_type(payload,'$.value.display.kindLabel') IS NULL)", [], |row| row.get(0))?;
    if stale_background {
        tx.execute(
            "DELETE FROM sync_entities WHERE collection IN ('task','monitor')",
            [],
        )?;
        tx.execute(
            "DELETE FROM sync_entity_meta WHERE key IN (?1,'task_window_migrated')",
            [crate::WORK_DETAILS_SEED_MARKER],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn stale_task_and_monitor_views_trigger_one_reseed() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE sync_entities(collection TEXT, id TEXT, payload TEXT, deleted INTEGER);
            CREATE TABLE sync_entity_meta(key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO sync_entity_meta VALUES('work_details_v1', '1'), ('task_window_migrated', '1');
            INSERT INTO sync_entities VALUES('task','a','{"value":{"id":"a"}}',0), ('monitor','b','{"value":{"id":"b"}}',0), ('other','c','{}',0);"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entities WHERE collection IN ('task','monitor')",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entity_meta WHERE key='work_details_v1'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        tx.execute(
            "INSERT INTO sync_entities VALUES('task','a',?1,0)",
            [r#"{"value":{"display":{"kindLabel":"command"}}}"#],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO sync_entity_meta VALUES('work_details_v1','1')",
            [],
        )
        .unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entities WHERE collection='task'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entity_meta WHERE key='work_details_v1'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn stale_work_details_are_reseeded_once() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE sync_entities(collection TEXT, id TEXT, payload TEXT, deleted INTEGER);
            CREATE TABLE sync_entity_meta(key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO sync_entity_meta VALUES('work_details_v1', '1');
            INSERT INTO sync_entities VALUES('workDetail','a','{"value":{"id":"a"}}',0), ('other','b','{}',0);"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entities WHERE collection='workDetail'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entity_meta WHERE key='work_details_v1'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        tx.execute("INSERT INTO sync_entities VALUES('workDetail','a',?1,0)", [r#"{"value":{"policy":{"blockedBy":[],"displayStatus":"ready","gateOverrideRequired":false,"acceptAvailable":true}}}"#]).unwrap();
        tx.execute(
            "INSERT INTO sync_entity_meta VALUES('work_details_v1','1')",
            [],
        )
        .unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entities WHERE collection='workDetail'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM sync_entity_meta WHERE key='work_details_v1'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
}
