//! Registry owners of the generated cold-state catalog. Do not read its output here.
use std::collections::BTreeMap;

use rusqlite::Connection;
use studio_db_api::Table;

#[derive(serde::Serialize)]
struct DatabaseCatalog {
    file_migration_v1: Vec<String>,
    payload_migrations: Vec<String>,
    pragma_user_version: u32,
    sqlite_master: Vec<studio_db_api::catalog::Entry>,
}

#[derive(serde::Serialize)]
struct Catalog {
    #[serde(flatten)]
    databases: BTreeMap<&'static str, DatabaseCatalog>,
    pragmas: BTreeMap<&'static str, crate::sqlite_durability::WriterPragmas>,
}

fn canvas(connection: &Connection) -> crate::StoreResult<()> {
    for registration in crate::canvas_schema::registry() {
        registration.descriptor.install_if_absent(connection)?;
    }
    // Evolved legacy tables precede compatibility installers, which keep them.
    connection.execute_batch(include_str!("catalog-canvas.sql"))?;
    connection.execute_batch(&crate::feature_schema::feature_schema_sql())?;
    for sql in [
        crate::feature_schema::SEARCH_SCHEMA,
        crate::feature_schema::RECEIPT_TRANSCRIPT_TRIGGERS,
        crate::schema::RECEIPT_OWNER_INDEXES,
    ] {
        connection.execute_batch(sql)?;
    }
    crate::receipt_schema::create(connection)?;
    crate::approvals::create_schema(connection)?;
    connection.execute_batch(&crate::project_grants::check_triggers_sql())?;
    connection.execute_batch(crate::work_history::CASCADE)?;
    connection.execute_batch(&crate::domain::check_triggers_sql())?;
    connection.execute_batch(&crate::domain::owner_delete_trigger_sql())?;
    connection.execute_batch(include_str!("catalog-canvas-objects.sql"))?;
    crate::feature_schema::install_summary_delete_triggers(connection)?;
    connection.execute_batch(crate::bootstrap::SYNC_SCHEMA)?;
    for table in crate::durable_seq::DURABLE_TABLES {
        connection.execute_batch(&crate::durable_seq::trigger_sql(
            table.name,
            table.record_only,
            table.team_fence,
        ))?;
    }
    crate::entity_scopes::install(connection)?;
    Ok(())
}

fn analytics(connection: &Connection) -> crate::StoreResult<()> {
    crate::analytics_schema::install(connection)?;
    connection.execute_batch(crate::feature_schema::PAYLOAD_MIGRATIONS_SCHEMA)?;
    connection.execute_batch(crate::settings_schema::StoredTextBatches::DDL)?;
    Ok(())
}

fn supervisor(connection: &Connection) -> crate::StoreResult<()> {
    crate::supervisor_schema::install(connection)?;
    Ok(())
}

pub(crate) fn generate() -> crate::StoreResult<String> {
    let mut catalog = BTreeMap::new();
    // One registry per database, referencing existing installation owners.
    for (name, install) in [
        (
            "canvas.sqlite3",
            canvas as fn(&Connection) -> crate::StoreResult<()>,
        ),
        ("analytics.sqlite3", analytics),
        ("supervisor.sqlite3", supervisor),
    ] {
        let connection = Connection::open_in_memory()?;
        crate::schema::register_python_trigger_functions(&connection)?;
        install(&connection)?;
        catalog.insert(
            name,
            DatabaseCatalog {
                file_migration_v1: vec![],
                payload_migrations: vec![],
                pragma_user_version: 0,
                sqlite_master: studio_db_api::catalog::export(&connection)?,
            },
        );
    }
    let catalog = Catalog {
        databases: catalog,
        pragmas: BTreeMap::from([
            ("analytics", crate::sqlite_durability::WRITER_PRAGMAS),
            ("main", crate::sqlite_durability::WRITER_PRAGMAS),
        ]),
    };
    Ok(format!("{}\n", studio_json::to_string_pretty(&catalog)?))
}
