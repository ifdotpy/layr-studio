//! Import preserved corpus rows into disposable databases built by current schema owners.
use std::{path::Path, sync::OnceLock};

use rusqlite::{Connection, config::DbConfig};

const DATABASES: [&str; 3] = ["canvas.sqlite3", "analytics.sqlite3", "supervisor.sqlite3"];

fn baseline() -> &'static [Vec<u8>; 3] {
    static BASELINE: OnceLock<[Vec<u8>; 3]> = OnceLock::new();
    BASELINE.get_or_init(|| {
        // These synchronous helpers also run within Tokio tests. Keep their isolated
        // bootstrap runtime on another thread rather than nesting block_on.
        std::thread::spawn(|| {
            let root = tempfile::tempdir().expect("baseline directory");
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("baseline runtime");
            runtime.block_on(async {
                let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
                    .await
                    .expect("current baseline opens");
                store
                    .checkpoint_wal_passive()
                    .await
                    .expect("baseline checkpoint");
                drop(store);
            });
            drop(runtime);
            DATABASES.map(|name| std::fs::read(root.path().join(name)).expect("closed baseline"))
        })
        .join()
        .expect("baseline bootstrap thread")
    })
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(crate) fn database(root: &Path, name: &str, sql: &str) {
    let source = tempfile::NamedTempFile::new().expect("corpus source");
    Connection::open(source.path())
        .expect("corpus source opens")
        .execute_batch(sql)
        .expect("restore historical corpus in scratch database");
    import(root, name, source.path());
}

pub(crate) fn import(root: &Path, name: &str, source: &Path) {
    std::fs::create_dir_all(root).expect("create corpus state directory");
    let index = DATABASES
        .iter()
        .position(|file| *file == name)
        .expect("owned database");
    let mut old = Connection::open(source).expect("corpus source opens");
    old.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
        .expect("disable source projections");
    if name == "canvas.sqlite3" {
        let tx = old.transaction().expect("corpus normalization");
        tx.execute("UPDATE runtime_agents SET record=json_remove(json_set(record,'$.permissionMode',CASE WHEN json_extract(record,'$.yoloMode') THEN 'bypassPermissions' ELSE 'default' END),'$.yoloMode') WHERE json_type(record,'$.yoloMode') IN ('true','false')", []).expect("normalize legacy permissions");
        tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.reviewDefaults.model',?1) WHERE (json_extract(record,'$.isLead')=1 OR json_extract(record,'$.rootId')=id) AND json_extract(record,'$.reviewDefaults.model') IS NULL", [studio_runtime_schema::provider_kind::initial_review_model()]).expect("normalize legacy review choice");
        studio_runtime_store::test_fixtures::normalize_agent(&tx).expect("normalize legacy agents");
        studio_runtime_store::test_fixtures::radio_room(&tx).expect("normalize legacy radio rooms");
        studio_runtime_store::test_fixtures::room_record(&tx).expect("normalize legacy rooms");
        studio_runtime_store::test_fixtures::project_teams(&tx).expect("normalize legacy teams");
        studio_runtime_store::test_fixtures::project_record_kind(&tx)
            .expect("normalize legacy projects");
        tx.commit().expect("corpus normalization commits");
        // Work search exposes the stored object as formatted text. The retired
        // fixture transition ordered its top-level keys before storing it.
        let rows = old
            .prepare("SELECT id,record FROM runtime_work")
            .expect("legacy work")
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("legacy work rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("legacy work reads");
        for (id, raw) in rows {
            let fields: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
                studio_json::from_str(&raw).expect("legacy work object");
            old.execute(
                "UPDATE runtime_work SET record=?1 WHERE id=?2",
                [
                    studio_json::to_string(&fields).expect("ordered work object"),
                    id,
                ],
            )
            .expect("normalize legacy work text");
        }
    }
    drop(old);

    let destination = root.join(name);
    std::fs::write(&destination, &baseline()[index]).expect("copy current schema and real markers");
    let mut db = Connection::open(destination).expect("current corpus database opens");
    studio_runtime_store::sqlite_durability::configure_writer(&db).expect("fixture durability");
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
        .expect("preserve captured projections");
    db.execute(
        "ATTACH DATABASE ?1 AS corpus",
        [source.to_str().expect("corpus path")],
    )
    .expect("attach corpus rows");
    let tx = db.transaction().expect("import transaction");
    let tables = tx.prepare("SELECT name FROM pragma_table_list WHERE schema='main' AND type IN ('table','shadow') AND name NOT LIKE 'sqlite_%' AND name IN (SELECT name FROM corpus.sqlite_master WHERE type='table')")
        .expect("current corpus tables")
        .query_map([], |row| row.get::<_, String>(0)).expect("table names")
        .collect::<Result<Vec<_>, _>>().expect("table list");
    for table in tables {
        // Completion evidence belongs to the newly built schema, never to the old dump.
        if table == "runtime_payload_migrations" {
            continue;
        }
        let columns = tx.prepare("SELECT name FROM pragma_table_xinfo(?1,'main') WHERE hidden=0 AND name IN (SELECT name FROM pragma_table_xinfo(?1,'corpus')) ORDER BY cid")
            .expect("shared corpus columns")
            .query_map([&table], |row| row.get::<_, String>(0)).expect("column names")
            .collect::<Result<Vec<_>, _>>().expect("column list")
            .iter().map(|name| quote(name)).collect::<Vec<_>>().join(",");
        let metadata = matches!(table.as_str(), "sync_entity_meta" | "analytics_meta");
        let table = quote(&table);
        if !metadata {
            tx.execute_batch(&format!("DELETE FROM {table}"))
                .expect("clear fresh seed rows");
        }
        let predicate = if metadata {
            " WHERE key != 'migration_catalog'"
        } else {
            ""
        };
        tx.execute_batch(&format!("INSERT OR REPLACE INTO main.{table}({columns}) SELECT {columns} FROM corpus.{table}{predicate}"))
            .unwrap_or_else(|error| panic!("import {name}/{table}: {error}"));
    }
    if name == "canvas.sqlite3" {
        let has_requests: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM corpus.sqlite_master WHERE name='runtime_requests')",
                [],
                |row| row.get(0),
            )
            .expect("legacy approval source");
        if has_requests {
            let requests = tx
                .prepare("SELECT id,record FROM corpus.runtime_requests WHERE json_type(record,'$.agent')='text'")
                .expect("legacy requests")
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .expect("legacy request rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("legacy requests read");
            for (id, record) in requests {
                match studio_runtime_store::test_fixtures::approval_from_record(&id, &record) {
                    Ok(approval) => studio_runtime_store::approvals::put(&tx, &approval)
                        .expect("preserved approval"),
                    // These incomplete historical rows never became valid approvals.
                    Err(studio_runtime_store::StoreError::SchemaMismatch(message))
                        if message.ends_with("a provider-turn request has no turn")
                            || message.ends_with("incomplete delivery fields") => {}
                    Err(error) => panic!("import approval {id}: {error}"),
                }
            }
        }
        for marker in [
            studio_runtime_store::AGENT_PROJECTIONS_SEED_MARKER,
            studio_runtime_store::SUMMARY_ENTITIES_SEED_MARKER,
            studio_runtime_store::CONVERSATION_DETAILS_SEED_MARKER,
            studio_runtime_store::WORK_DETAILS_SEED_MARKER,
        ] {
            tx.execute("DELETE FROM sync_entity_meta WHERE key=?1", [marker])
                .expect("rebuild imported row projections");
        }
    }
    tx.commit().expect("corpus import commits");
}

pub(crate) fn state(root: &Path, canvas: &str, analytics: &str) {
    database(root, "canvas.sqlite3", canvas);
    database(root, "analytics.sqlite3", analytics);
}
