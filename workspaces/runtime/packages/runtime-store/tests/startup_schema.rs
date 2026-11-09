use rusqlite::Connection;
use studio_runtime_store::RuntimeStore;

#[tokio::test]
async fn current_item_index_and_retired_receipt_tables_converge() {
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    let canvas = Connection::open(state.path().join("canvas.sqlite3")).expect("canvas");
    let analytics = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    let plan: String = canvas.query_row("EXPLAIN QUERY PLAN SELECT id,created FROM runtime_items WHERE agent='a' AND json_extract(record,'$.turnId')='t' AND json_extract(record,'$.role')='assistant' ORDER BY created,id", [], |row| row.get(3)).expect("turn query plan");
    assert!(plan.contains("runtime_item_turn_scope"), "{plan}");
    let stale: bool = canvas.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='runtime_request_native_approval')", [], |row| row.get(0)).unwrap();
    assert!(!stale);
    canvas.execute_batch("DROP INDEX runtime_item_turn_scope; CREATE INDEX runtime_request_native_approval ON runtime_requests(id)").expect("older canvas");
    analytics.execute_batch("CREATE TABLE analytics_capture_effect_receipts(id INTEGER); INSERT INTO analytics_capture_effect_receipts VALUES(1); CREATE TABLE analytics_capture_legacy_watermarks(id INTEGER); CREATE TABLE analytics_capture_receipt_migrations(id INTEGER);").expect("retired receipts");
    drop(canvas);
    drop(analytics);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("upgrade"),
    );
    let canvas = Connection::open(state.path().join("canvas.sqlite3")).expect("canvas");
    let analytics = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    let indexes: (bool, bool) = canvas.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='runtime_item_turn_scope'), EXISTS(SELECT 1 FROM sqlite_schema WHERE name='runtime_request_native_approval')", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(indexes, (true, false));
    for table in [
        "analytics_capture_effect_receipts",
        "analytics_capture_legacy_watermarks",
        "analytics_capture_receipt_migrations",
    ] {
        let exists: bool = analytics
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name=?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!exists, "{table}");
    }
}

#[tokio::test]
async fn current_settings_and_overflow_receipts_exist_on_fresh_and_completed_stores() {
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    let canvas = Connection::open(state.path().join("canvas.sqlite3")).expect("canvas");
    let analytics = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    canvas
        .execute("INSERT INTO runtime_peer_update_settings VALUES(1,0)", [])
        .expect("peer setting");
    analytics
        .execute(
            "INSERT INTO analytics_publication_overflow_receipts VALUES('receipt',2,3)",
            [],
        )
        .expect("overflow receipt");
    drop(canvas);
    drop(analytics);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("reopen"),
    );
    let canvas = Connection::open(state.path().join("canvas.sqlite3")).expect("canvas");
    let analytics = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    assert_eq!(
        canvas
            .query_row(
                "SELECT auto_update FROM runtime_peer_update_settings WHERE id=1",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(analytics.query_row("SELECT analytics_overflow FROM analytics_publication_overflow_receipts WHERE receipt_id='receipt'", [], |row| row.get::<_, i64>(0)).unwrap(), 2);
    canvas
        .execute_batch("DROP TABLE runtime_peer_update_settings")
        .expect("older canvas");
    analytics
        .execute_batch("DROP TABLE analytics_publication_overflow_receipts")
        .expect("older analytics");
    drop(canvas);
    drop(analytics);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("upgrade missing tables"),
    );
    for (file, table) in [
        ("canvas.sqlite3", "runtime_peer_update_settings"),
        (
            "analytics.sqlite3",
            "analytics_publication_overflow_receipts",
        ),
    ] {
        let db = Connection::open(state.path().join(file)).expect("database");
        let present: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(present, "{table}");
    }
}

#[tokio::test]
async fn missing_analytics_revision_schema_is_restored_on_open() {
    let state = tempfile::tempdir().expect("state");
    drop(RuntimeStore::open(Some(state.path())).await.expect("fresh"));
    let db = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    db.execute_batch("DROP TABLE analytics_usage_revision_clock; DROP TABLE analytics_usage_revisions; DROP TABLE analytics_cost_group_revisions;").expect("older schema");
    drop(db);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("upgrade"),
    );
    let db = Connection::open(state.path().join("analytics.sqlite3")).expect("analytics");
    let clock: i64 = db
        .query_row(
            "SELECT revision FROM analytics_usage_revision_clock",
            [],
            |row| row.get(0),
        )
        .expect("revision clock");
    assert_eq!(clock, 0);
    let objects: i64 = db.query_row("SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'analytics_usage_revision%' OR name LIKE 'analytics_cost_group_revision%'", [], |row| row.get(0)).unwrap();
    assert_eq!(objects, 11);
    drop(db);
    drop(
        RuntimeStore::open(Some(state.path()))
            .await
            .expect("reopen"),
    );
}
