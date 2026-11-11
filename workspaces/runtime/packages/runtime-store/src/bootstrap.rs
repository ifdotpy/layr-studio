use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::{StoreError, StoreResult};

pub(crate) const SYNC_SCHEMA: &str = include_str!("sync_schema.sql");

struct ParentTableRegistration(&'static str);

inventory::collect!(ParentTableRegistration);

inventory::submit! {
    ParentTableRegistration("runtime_agents")
}

inventory::submit! {
    ParentTableRegistration("runtime_events")
}

inventory::submit! {
    ParentTableRegistration("runtime_items")
}

fn registered_parent_tables() -> Vec<&'static str> {
    let mut tables = inventory::iter::<ParentTableRegistration>
        .into_iter()
        .map(|registration| registration.0)
        .collect::<Vec<_>>();
    tables.sort_unstable();
    tables
}

#[cfg(any(test, feature = "test-support"))]
#[path = "bootstrap_legacy_fixture.rs"]
pub(crate) mod legacy_fixture;

/// One fixed execution point for descriptor-owned current producer factories.
/// Called only within fresh installation or the final authority transaction.
pub(crate) fn install_current_producers(transaction: &Connection) -> StoreResult<()> {
    let _write = crate::peer_update_settings::canvas_write(transaction)?;
    use crate::{
        change_schema::Changes, metadata_schema::Metadata, publication_schema::Publications,
    };
    use studio_db_api::Table;
    for descriptor in [
        Metadata::descriptor(),
        Changes::descriptor(),
        Publications::descriptor(),
    ] {
        descriptor.install_if_absent(transaction)?;
    }
    let read = crate::peer_update_settings::canvas_read(transaction);
    let mut sql = String::new();
    if read.table_exists::<crate::agent_tables::AgentsStorageRow>()? {
        sql.push_str(&crate::clock_schema::target_agent_clock_sql());
    }
    if read.table_exists::<crate::agent_tables::AgentVolatileStorageRow>()? {
        sql.push_str(&crate::agent_volatile::clock_trigger_sql());
    }
    // Both transcript source tables exist in a supported Canvas. Small schema
    // fixtures may expose just one; install its owned names without a new branch policy.
    if read.table_exists::<crate::agent_tables::AgentsStorageRow>()?
        && read.table_exists::<crate::item_monitor_terminal::ItemsRow>()?
    {
        sql.push_str(&crate::clock_schema::target_transcript_clock_sql());
    }
    sql.push_str(&crate::repositories::sync_documents::tombstone_counter_sql());
    sql.push_str(&crate::entity_scopes::publication_trigger_sql());
    if read.table_exists::<crate::queue_schema::EventsRow>()?
        && read.table_exists::<crate::planning_work::TaskRow>()?
    {
        sql.push_str(&crate::feature_schema::summary_delete_trigger_sql());
    }
    if read.table_exists::<crate::queue_schema::EventsRow>()? {
        sql.push_str(&crate::event_publication_triggers::trigger_sql());
    }
    for table in crate::durable_seq::DURABLE_TABLES {
        if crate::schema::table_exists(transaction, table.name)? {
            sql.push_str(&crate::durable_seq::trigger_sql(
                table.name,
                table.record_only,
                table.team_fence,
            ));
        }
    }
    transaction.execute_batch(&sql)?;
    Ok(())
}

pub(crate) fn prepare_runtime_event_indexes(transaction: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<crate::queue_schema::EventsRow>(
        transaction,
        "runtime_events_agent_epoch_status_created",
    )?;
    crate::SchedulerRepository::prepare_account_wake_index(transaction)?;
    crate::SchedulerRepository::prepare_child_result_hold_indexes(transaction)?;
    crate::repositories::reviewer_archive::ReviewerArchiveRepository::prepare_indexes(transaction)?;
    if crate::schema::table_exists(transaction, "runtime_tasks")? {
        crate::TaskRepository::prepare_indexes(transaction)?;
    }
    Ok(())
}

fn parse_sections(source: &str) -> StoreResult<[&str; 4]> {
    let mut sections = source.split("-- section: ").skip(1);
    let mut bodies = [""; 4];
    for (body, expected) in bodies.iter_mut().zip([
        "common",
        "runtime_events",
        "runtime_agents",
        "runtime_items",
    ]) {
        let section = sections.next().ok_or_else(|| {
            StoreError::SchemaMismatch(format!("sync schema is missing {expected}"))
        })?;
        let (name, sql) = section.split_once('\n').ok_or_else(|| {
            StoreError::SchemaMismatch(format!("sync schema section {expected} has no body"))
        })?;
        if name.trim() != expected {
            return Err(StoreError::SchemaMismatch(format!(
                "expected sync schema section {expected}, found {name}"
            )));
        }
        *body = sql;
    }
    if sections.next().is_some() {
        return Err(StoreError::SchemaMismatch(
            "unexpected sync schema section".into(),
        ));
    }
    Ok(bodies)
}

type ColumnLayout = (String, Option<String>, i64, Option<String>, i64, i64);
type ConstraintIndex = (String, Vec<(i64, i64, Option<String>, i64, Option<String>)>);
type TableLayout = (Vec<ColumnLayout>, bool, bool, BTreeSet<ConstraintIndex>);
type CatalogObject = (String, String, String);

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SchemaFinding {
    pub(crate) object: String,
    pub(crate) reason: String,
}

/// Inspect sync table signatures using a read-only connection.
///
/// This builds the expected tables from the canonical DDL in an in-memory
/// database and returns every table difference without writing to `connection`.
#[allow(dead_code)] // Used by the pre-cutover state check when that checker is added.
pub(crate) fn inspect_sync_tables(connection: &Connection) -> StoreResult<Vec<SchemaFinding>> {
    use studio_db_api::Table;
    let read = crate::peer_update_settings::canvas_read(connection);
    if !read.table_exists::<crate::settings_schema::PayloadMigrations>()?
        && read.table_exists::<crate::metadata_schema::Metadata>()?
    {
        let expected = Connection::open_in_memory()?;
        let mut findings = Vec::new();
        for descriptor in [
            crate::metadata_schema::Metadata::descriptor(),
            crate::change_schema::Changes::descriptor(),
            crate::publication_schema::Publications::descriptor(),
        ] {
            descriptor.install_if_absent(&expected)?;
            if !same_table_layout(&expected, connection, descriptor.name)? {
                findings.push(SchemaFinding {
                    object: descriptor.name.into(),
                    reason: "canonical partition table layout differs".into(),
                });
            }
        }
        return Ok(findings);
    }
    let sections = parse_sections(SYNC_SCHEMA)?;
    let has_runtime_items = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_items' COLLATE NOCASE)",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let (_, findings) = expected_catalog(connection, sections, has_runtime_items)?;
    Ok(findings)
}

fn expected_catalog(
    connection: &Connection,
    sections: [&str; 4],
    has_runtime_items: bool,
) -> StoreResult<(Vec<CatalogObject>, Vec<SchemaFinding>)> {
    let expected = Connection::open_in_memory()?;
    let parent_tables = registered_parent_tables();
    for parent in &parent_tables {
        if *parent == "runtime_items" && !has_runtime_items {
            continue;
        }
        let sql = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1 COLLATE NOCASE",
                [*parent],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| schema_object_mismatch(parent, "required parent table is missing"))?;
        expected
            .execute_batch(&sql)
            .map_err(|error| schema_object_mismatch(parent, &error.to_string()))?;
    }
    crate::clock_schema::prepare_sync_tables(&expected)?;
    crate::queue_schema::install_index::<crate::queue_schema::EventsRow>(
        &expected,
        "runtime_event_created_id",
    )?;
    expected.execute_batch(sections[0])?;
    expected.execute_batch(sections[1])?;
    expected.execute_batch(sections[2])?;
    if has_runtime_items {
        expected.execute_batch(sections[3])?;
    }

    let mut statement = expected.prepare(
        "SELECT type,name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        ))
    })?;
    let mut derived = Vec::new();
    let mut findings = Vec::new();
    for object in rows {
        let (kind, name, sql) = object?;
        if kind == "table" {
            if parent_tables
                .iter()
                .any(|parent| parent.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let wanted = table_layout(&expected, &name)?
                .ok_or_else(|| schema_object_mismatch(&name, "scratch table is missing"))?;
            let actual_kind = connection
                .query_row(
                    "SELECT type FROM sqlite_master WHERE name=?1 COLLATE NOCASE AND type IN ('table','view','index')",
                    [&name],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(actual_kind) = actual_kind {
                if actual_kind != "table" {
                    findings.push(SchemaFinding {
                        object: name,
                        reason: format!("expected a table, found {actual_kind}"),
                    });
                    continue;
                }
                let actual = table_layout(connection, &name)?;
                // The immutable-scope migration adds this nullable tombstone identity.
                // Accept precisely the supported prior layout during read-only preflight.
                if name == "durable_rows" {
                    let mut prior = wanted.clone();
                    prior.0.retain(|column| column.0 != "scope");
                    if actual.as_ref() == Some(&prior) {
                        continue;
                    }
                }
                if actual.as_ref() == Some(&wanted) {
                    continue;
                }
                let actual = actual.as_ref().expect("existing table has a layout");
                let column = wanted
                    .0
                    .iter()
                    .zip(&actual.0)
                    .find(|(expected, actual)| expected != actual)
                    .map(|(expected, _)| expected.0.as_str())
                    .or_else(|| wanted.0.get(actual.0.len()).map(|column| column.0.as_str()))
                    .or_else(|| actual.0.get(wanted.0.len()).map(|column| column.0.as_str()));
                let reason = if let Some(column) = column {
                    format!("column {column} differs")
                } else if wanted.3 != actual.3 {
                    format!(
                        "automatic UNIQUE/PRIMARY KEY indexes differ: expected {:?}, found {:?}",
                        wanted.3, actual.3
                    )
                } else {
                    "table options differ".to_owned()
                };
                findings.push(SchemaFinding {
                    object: name,
                    reason,
                });
            }
        } else if kind == "index" || kind == "trigger" {
            derived.push((kind, name, sql));
        }
    }
    drop(statement);
    Ok((derived, findings))
}

fn table_layout(connection: &Connection, name: &str) -> StoreResult<Option<TableLayout>> {
    let options = connection
        .query_row(
            "SELECT wr,strict FROM pragma_table_list WHERE schema='main' AND name=?1 COLLATE NOCASE",
            [name],
            |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, i64>(1)? != 0)),
        )
        .optional()?;
    let Some((without_rowid, strict)) = options else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT name,type,\"notnull\",dflt_value,pk,hidden FROM pragma_table_xinfo(?1) ORDER BY cid",
    )?;
    let rows = statement.query_map([name], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    })?;
    let columns = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut statement = connection
        .prepare("SELECT name,origin FROM pragma_index_list(?1) WHERE origin IN ('u','pk')")?;
    let indexes = statement
        .query_map([name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut constraints = BTreeSet::new();
    for (index, origin) in indexes {
        let mut statement = connection.prepare(
            "SELECT seqno,cid,name,\"desc\",coll FROM pragma_index_xinfo(?1) WHERE key=1 ORDER BY seqno",
        )?;
        let columns = statement
            .query_map([index], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        constraints.insert((origin, columns));
    }
    Ok(Some((columns, without_rowid, strict, constraints)))
}

/// Compare table structure against the caller's canonical DDL without copying
/// a column/constraint list into another schema owner.
pub(crate) fn same_table_layout(
    expected: &Connection,
    actual: &Connection,
    name: &str,
) -> StoreResult<bool> {
    Ok(table_layout(expected, name)? == table_layout(actual, name)?)
}

fn schema_object_mismatch(object: &str, reason: &str) -> StoreError {
    StoreError::SchemaObjectMismatch {
        database: "canvas.sqlite3".to_owned(),
        object: object.to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_inspection_returns_findings_without_writing() {
        let mut connection = Connection::open_in_memory().expect("in-memory database opens");
        connection
            .execute_batch(
                "CREATE TABLE runtime_events(id TEXT,agent TEXT,kind TEXT,status TEXT,created REAL,error TEXT);
                 CREATE TABLE runtime_agents(id TEXT);
                 CREATE TABLE runtime_items(agent TEXT);",
            )
            .expect("required parent tables are created");
        connection
            .execute_batch(crate::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        let transaction = connection
            .transaction()
            .expect("bootstrap transaction starts");
        legacy_fixture::prepare_sync_schema(&transaction).expect("legacy source schema prepares");
        transaction.commit().expect("bootstrap commits");
        let sql = SYNC_SCHEMA.to_ascii_uppercase();
        assert!(
            !sql.contains("CHECK") && !sql.contains("REFERENCES"),
            "extend structural comparison before adding CHECK or FOREIGN KEY constraints"
        );
        let tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND (name GLOB 'sync_*' OR name='runtime_transcript_revisions')")
            .expect("owned tables")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("owned table names")
            .collect::<Result<Vec<_>, _>>()
            .expect("owned tables read");
        for table in tables {
            let unsupported: i64 = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM pragma_foreign_key_list(?1)) + \
                 (SELECT COUNT(*) FROM pragma_table_xinfo(?1) WHERE hidden!=0)",
                    [&table],
                    |row| row.get(0),
                )
                .expect("owned table constraints");
            assert_eq!(unsupported, 0, "extend structural comparison for {table}");
        }
        assert!(
            inspect_sync_tables(&connection)
                .expect("canonical tables inspect")
                .is_empty()
        );

        connection
            .execute_batch(
                "DROP TABLE sync_identity;
                 CREATE TABLE sync_identity(id VARCHAR PRIMARY KEY);",
            )
            .expect("test table is modified");
        let before = connection
            .pragma_query_value(None, "schema_version", |row| row.get::<_, i64>(0))
            .expect("schema version reads");
        let findings = inspect_sync_tables(&connection).expect("table differences inspect");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].object, "sync_identity");
        assert!(findings[0].reason.contains("column id"));
        assert_eq!(
            connection
                .pragma_query_value(None, "schema_version", |row| row.get::<_, i64>(0))
                .expect("schema version remains readable"),
            before,
            "inspection is read-only"
        );
    }
}
