use rusqlite::Connection;
use studio_runtime_store::RuntimeStore;

#[tokio::test]
async fn migration_baseline_missing_catalog_refuses_before_any_database_write() {
    let root = tempfile::tempdir().unwrap();
    drop(RuntimeStore::open(Some(root.path())).await.unwrap());
    let path = root.path().join("canvas.sqlite3");
    let db = Connection::open(&path).unwrap();
    db.execute(
        "DELETE FROM sync_entity_meta WHERE key='migration_catalog'",
        [],
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    let analytics = std::fs::read(root.path().join("analytics.sqlite3")).unwrap();
    let error = RuntimeStore::open(Some(root.path()))
        .await
        .err()
        .expect("unsupported state refuses");
    assert!(error.to_string().contains("state too old:"), "{error}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        std::fs::read(root.path().join("analytics.sqlite3")).unwrap(),
        analytics
    );
}

fn write_fixture(root: &std::path::Path, frames: [&[u8]; 3]) {
    for (name, frame) in ["canvas.sqlite3", "analytics.sqlite3", "supervisor.sqlite3"]
        .into_iter()
        .zip(frames)
    {
        let size = zstd_safe::get_frame_content_size(frame)
            .expect("valid compressed fixture")
            .expect("fixture content size") as usize;
        let mut bytes = Vec::with_capacity(size);
        zstd_safe::decompress(&mut bytes, frame).expect("decompress fixture");
        std::fs::write(root.join(name), bytes).expect("write fixture database");
    }
}

pub(super) fn write_supported_fixture(root: &std::path::Path) {
    write_fixture(
        root,
        [
            include_bytes!("fixtures/f832d09/canvas.sqlite3.zst"),
            include_bytes!("fixtures/f832d09/analytics.sqlite3.zst"),
            include_bytes!("fixtures/f832d09/supervisor.sqlite3.zst"),
        ],
    );
}

#[tokio::test]
async fn migration_baseline_supported_release_upgrades_and_reopens_with_current_features() {
    let root = tempfile::tempdir().unwrap();
    write_supported_fixture(root.path());
    for _ in 0..2 {
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        store.transaction(|tx| {
            let budget: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='runtime_budget')", [], |r| r.get(0))?;
            assert!(!budget);
            let cursors: i64 = tx.query_row("SELECT count(*) FROM pragma_table_info('runtime_payload_migrations') WHERE name IN ('cursor_key','copy_source_rows','copy_target_rows')", [], |r| r.get(0))?;
            assert_eq!(cursors, 3);
            let agent = studio_runtime_store::AgentRecord::from_value(studio_json::json!({"id":"baseline-current","rootId":"baseline-current","provider":"codex","accountKey":"default","epoch":0,"compactions":0,"status":"idle"}))?;
            studio_runtime_store::AgentRepository::put(tx, &agent)?;
            assert!(studio_runtime_store::AgentRepository::get(tx, &studio_runtime_store::AgentId::from("baseline-current"))?.is_some());
            assert!(tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.rootId','other') WHERE id='baseline-current'", []).is_err(), "upgraded state preserves immutable team ownership");
            use studio_runtime_store::request_results::{Admission, RequestResults, ResultEnvelope};
            let identity = studio_runtime_schema::persistence::RequestIdentity::opaque("baseline", &["owner"], "current-request", &[], 1000);
            match RequestResults::admit(tx, &identity, "exact-input", 1000)? {
                Admission::Fresh(token) => { RequestResults::settle(tx, &token, &ResultEnvelope { actor: None, reply: "original reply", references: vec![] })?; }
                Admission::Replay(saved) => assert!(saved.result.contains("original reply")),
            }
            assert!(RequestResults::replay(tx, identity.id(), "different-input", 1000).is_err(), "a changed request cannot reuse the durable receipt");
            Ok(())
        }).await.unwrap();
        drop(store);
    }
}

#[tokio::test]
async fn migration_baseline_populated_preledger_release_upgrade_preserves_missing_checkpoint() {
    let root = tempfile::tempdir().expect("state directory");
    write_fixture(
        root.path(),
        [
            include_bytes!("fixtures/ff02c9d-f832d09/canvas.sqlite3.zst"),
            include_bytes!("fixtures/ff02c9d-f832d09/analytics.sqlite3.zst"),
            include_bytes!("fixtures/ff02c9d-f832d09/supervisor.sqlite3.zst"),
        ],
    );
    let checkpoint = "2026-11-07-explicit-review-model";
    for _ in 0..2 {
        let db = Connection::open(root.path().join("canvas.sqlite3")).expect("fixture database");
        let present: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runtime_payload_migrations WHERE name=?1)",
                [checkpoint],
                |row| row.get(0),
            )
            .expect("historical checkpoint");
        assert!(
            !present,
            "the historical upgrade never wrote this checkpoint"
        );
        drop(db);
        let store = RuntimeStore::open(Some(root.path()))
            .await
            .expect("valid populated release upgrade opens");
        store
            .read_async(
                "baseline/preledger",
                |db| -> studio_runtime_store::StoreResult<_> {
                    let agent =
                        studio_runtime_store::AgentRepository::get(db, &"pre-ledger-root".into())?
                            .expect("preserved agent");
                    assert_eq!(
                        agent
                            .review_defaults
                            .as_ref()
                            .and_then(|defaults| defaults.model.as_deref()),
                        Some("gpt-5.3-codex")
                    );
                    let work = studio_runtime_store::WorkItemsRepository::work_record(
                        db,
                        "pre-ledger-work",
                    )?
                    .expect("preserved work");
                    assert_eq!(work.value()["title"], "Preserved upgrade task");
                    Ok(())
                },
            )
            .await
            .expect("populated evidence stays readable");
        drop(store);
    }
}

fn database_bytes(root: &std::path::Path) -> Vec<Vec<u8>> {
    ["canvas.sqlite3", "analytics.sqlite3", "supervisor.sqlite3"]
        .iter()
        .map(|file| std::fs::read(root.join(file)).expect("snapshot fixture database"))
        .collect()
}

#[tokio::test]
async fn migration_baseline_every_missing_or_incomplete_checkpoint_refuses_unchanged() {
    let sample = tempfile::tempdir().unwrap();
    write_supported_fixture(sample.path());
    let db = Connection::open(sample.path().join("canvas.sqlite3")).unwrap();
    let names: Vec<String> = db.prepare("SELECT name FROM runtime_payload_migrations WHERE name LIKE 'receipts:%' OR name LIKE '2026-%'").unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(names.len(), 22, "actual supported-release checkpoints");
    for name in names {
        for sql in [
            "DELETE FROM runtime_payload_migrations WHERE name=?1",
            "UPDATE runtime_payload_migrations SET complete=0 WHERE name=?1",
            "UPDATE runtime_payload_migrations SET status='running' WHERE name=?1",
        ] {
            let root = tempfile::tempdir().unwrap();
            write_supported_fixture(root.path());
            let db = Connection::open(root.path().join("canvas.sqlite3")).unwrap();
            db.execute(sql, [&name]).unwrap();
            drop(db);
            if name == "2026-11-07-explicit-review-model" && sql.starts_with("DELETE") {
                drop(
                    RuntimeStore::open(Some(root.path()))
                        .await
                        .expect("catalog covers the pre-ledger step"),
                );
                continue;
            }
            let before = database_bytes(root.path());
            let error = RuntimeStore::open(Some(root.path()))
                .await
                .err()
                .expect("incomplete baseline refuses");
            assert!(
                error.to_string().contains("state too old:"),
                "{name}: {error}"
            );
            assert!(error.to_string().contains(&name), "{name}: {error}");
            assert_eq!(
                database_bytes(root.path()),
                before,
                "{name}: no database write"
            );
        }
    }
}

#[tokio::test]
async fn migration_baseline_bad_or_future_catalog_and_other_file_formats_refuse_unchanged() {
    for (file, sql) in [
        (
            "canvas.sqlite3",
            "DELETE FROM sync_entity_meta WHERE key='migration_catalog'",
        ),
        (
            "analytics.sqlite3",
            "DELETE FROM analytics_meta WHERE key='migration_catalog'",
        ),
        (
            "canvas.sqlite3",
            "UPDATE sync_entity_meta SET value=substr(value,1,instr(value,':')) || '999999999' WHERE key='migration_catalog'",
        ),
        (
            "analytics.sqlite3",
            "UPDATE analytics_meta SET value='wrong:47' WHERE key='migration_catalog'",
        ),
        (
            "analytics.sqlite3",
            "DELETE FROM analytics_meta WHERE key='providerIdentityFormat'",
        ),
        ("supervisor.sqlite3", "DROP TABLE operation_writes"),
    ] {
        let root = tempfile::tempdir().unwrap();
        write_supported_fixture(root.path());
        let db = Connection::open(root.path().join(file)).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = database_bytes(root.path());
        let error = RuntimeStore::open(Some(root.path()))
            .await
            .err()
            .expect("unsupported baseline refuses");
        assert!(
            error.to_string().contains("state too old:"),
            "{file}: {error}"
        );
        assert_eq!(
            database_bytes(root.path()),
            before,
            "{file}: all files unchanged"
        );
    }
}

#[tokio::test]
async fn migration_baseline_cleanup_failure_rolls_back_canvas_and_restart_finishes() {
    let root = tempfile::tempdir().unwrap();
    write_supported_fixture(root.path());
    let db = Connection::open(root.path().join("canvas.sqlite3")).unwrap();
    db.execute_batch(
        "DROP TABLE runtime_budget; CREATE VIEW runtime_budget AS SELECT 'blocked' AS id;",
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(root.path().join("canvas.sqlite3")).unwrap();
    let error = RuntimeStore::open(Some(root.path()))
        .await
        .err()
        .expect("cleanup refuses wrong object kind");
    assert!(
        error.to_string().contains("2026-11-09-retired-objects"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(root.path().join("canvas.sqlite3")).unwrap(),
        before
    );
    let db = Connection::open(root.path().join("canvas.sqlite3")).unwrap();
    let cursor: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('runtime_payload_migrations') WHERE name='cursor_key')", [], |r| r.get(0)).unwrap();
    assert!(!cursor, "earlier Nov-9 steps roll back with cleanup");
    db.execute_batch("DROP VIEW runtime_budget; CREATE TABLE runtime_budget(id TEXT PRIMARY KEY,record TEXT NOT NULL)").unwrap();
    drop(db);
    drop(RuntimeStore::open(Some(root.path())).await.unwrap());
    drop(RuntimeStore::open(Some(root.path())).await.unwrap());
}

#[tokio::test]
async fn migration_baseline_fresh_and_supported_release_have_matching_current_objects() {
    use std::collections::BTreeSet;
    let upgraded = tempfile::tempdir().unwrap();
    write_supported_fixture(upgraded.path());
    drop(RuntimeStore::open(Some(upgraded.path())).await.unwrap());
    let fresh = tempfile::tempdir().unwrap();
    drop(RuntimeStore::open(Some(fresh.path())).await.unwrap());
    for file in ["canvas.sqlite3", "analytics.sqlite3", "supervisor.sqlite3"] {
        let catalog = |root: &std::path::Path| -> BTreeSet<(String, String)> {
            let db = Connection::open(root.join(file)).unwrap();
            db.prepare("SELECT type,name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").unwrap().query_map([], |r| Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap()
        };
        if file == "canvas.sqlite3" {
            for table in [
                "runtime_agents",
                "runtime_agent_volatile",
                "chats",
                "sessions",
                "turns",
            ] {
                let columns = |root: &std::path::Path| {
                    let db = Connection::open(root.join(file)).expect("schema database");
                    db.prepare("SELECT name,type,\"notnull\",dflt_value,pk,hidden FROM pragma_table_xinfo(?1) ORDER BY cid").expect("column query")
                        .query_map([table], |row| Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,i64>(2)?, row.get::<_,Option<String>>(3)?, row.get::<_,i64>(4)?, row.get::<_,i64>(5)?)))
                        .expect("columns").collect::<Result<Vec<_>,_>>().expect("column metadata")
                };
                assert_eq!(
                    columns(upgraded.path()),
                    columns(fresh.path()),
                    "{table} physical columns"
                );
            }
        }
        let upgraded = catalog(upgraded.path());
        let fresh = catalog(fresh.path());
        assert!(
            upgraded == fresh,
            "{file}: upgraded only: {:?}; fresh only: {:?}",
            upgraded.difference(&fresh).collect::<Vec<_>>(),
            fresh.difference(&upgraded).collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn migration_baseline_named_clock_queries_validate_after_supported_upgrade() {
    let root = tempfile::tempdir().unwrap();
    write_supported_fixture(root.path());
    let store = RuntimeStore::open(Some(root.path())).await.unwrap();
    store
        .read_async(
            "named-queries/upgraded-schema",
            |db| -> studio_runtime_store::StoreResult<()> {
                studio_runtime_store::clock_schema::validate_named_queries(db).map_err(
                    |error| studio_runtime_store::StoreError::SchemaMismatch(error.to_string()),
                )?;
                studio_runtime_store::ExecutionHistoryRepository::validate_provider_request_queries(db)
                    .map_err(|error| studio_runtime_store::StoreError::SchemaMismatch(error.to_string()))?;
                let (high, floor) = studio_runtime_store::durable_seq::high_and_floor(db)?;
                assert!(high >= floor);
                Ok(())
            },
        )
        .await
        .unwrap();
}
