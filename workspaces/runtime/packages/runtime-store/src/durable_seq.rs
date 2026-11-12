//! Durable change producer and cursor reader over canonical Changes, Metadata and Publications.
//! Tombstones retain deletion scope and expire after the owner retention interval.

use rusqlite::{Connection, Transaction};

use crate::change_schema::{Changes, DurableRows, DurableSource};
use crate::consolidation_kinds::{CanvasChangesKind as ChangeKind, CanvasMetadataKind as MetaKind};
use crate::metadata_schema::{Metadata, SyncGeneration};
use crate::metadata_values::StoredBoolean;
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx, canvas_write};
use crate::publication_schema::Publications;
use crate::{StoreError, StoreResult};
use studio_db_api::{Table, migration::encode_key};

/// How long a tombstone stays, in seconds (90 days). A reconnect inside this window gets a
/// delta; a longer absence gets a snapshot.
pub const TOMBSTONE_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;

/// A main table that feeds the durable stream.
pub struct DurableTable {
    /// Table name; the table has `id TEXT PRIMARY KEY` and `rowid`.
    pub name: &'static str,
    /// Column that holds the row payload sent to readers.
    pub payload: &'static str,
    /// True for a table too large to number in one transaction: the installer adds the
    /// triggers, and an online migration gives the old rows their entries later.
    pub online_backfill: bool,
    /// True for a table whose only data is `id` and the payload column: an update that
    /// leaves both unchanged takes no new number.
    pub record_only: bool,
    /// True for `runtime_agents`: a change of an agent's team (liveness) moves
    /// the scope visibility fence of the team, with this change's number.
    pub team_fence: bool,
}

impl DurableTable {
    pub const fn new(name: &'static str, payload: &'static str) -> Self {
        Self {
            name,
            payload,
            online_backfill: false,
            record_only: false,
            team_fence: false,
        }
    }

    /// The agent table; see [`DurableTable::team_fence`].
    pub const fn agents() -> Self {
        Self {
            name: "runtime_agents",
            payload: "record",
            online_backfill: false,
            record_only: false,
            team_fence: true,
        }
    }

    /// A table of `id` and `record` only; see [`DurableTable::record_only`].
    pub const fn record(name: &'static str) -> Self {
        Self {
            name,
            payload: "record",
            online_backfill: false,
            record_only: true,
            team_fence: false,
        }
    }

    pub const fn online(name: &'static str, payload: &'static str) -> Self {
        Self {
            name,
            payload,
            online_backfill: true,
            record_only: false,
            team_fence: false,
        }
    }
}

/// Opt-in list. Add one line to publish another table.
pub const DURABLE_TABLES: &[DurableTable] = &[
    DurableTable::agents(),
    DurableTable::new("runtime_requests", "record"),
    DurableTable::new("runtime_work", "record"),
    // The owner of conversation items; transcript pages are built from it at read time.
    DurableTable::online("runtime_items", "record"),
    // Work history entries, one row per entry (see work_history.rs).
    DurableTable::new(
        <crate::work_history::WorkResultsStorageRow as studio_db_api::Table>::NAME,
        "record",
    ),
    DurableTable::new(
        <crate::work_history::WorkDecisionsStorageRow as studio_db_api::Table>::NAME,
        "record",
    ),
    DurableTable::new(
        <crate::work_history::WorkReleasesStorageRow as studio_db_api::Table>::NAME,
        "record",
    ),
    // Execution runs: a failure refusal of a work document depends on the owner's run.
    DurableTable::new("runtime_execution_runs", "record"),
    // Sources of documents built at read time, one document per row.
    DurableTable::record("runtime_plans"),
    DurableTable::record("runtime_annotations"),
    DurableTable::record("runtime_rules"),
    DurableTable::new("approvals", "request"),
    DurableTable::record("runtime_complaints"),
    // Chat membership edges: an `edge` document per chat edge, built at read time.
    DurableTable::new("graph_edges", "target"),
    // Window sources: a row in its window (`TASK_WINDOW`, `MONITOR_WINDOW`) is sent
    // again when it changes.
    DurableTable::record("runtime_tasks"),
    DurableTable::record("runtime_monitors"),
];

/// One high-plus-one rule, using the canonical named clock read in trigger SQL.
/// SQLite promotes overflowing INTEGER arithmetic to REAL, which must be refused.
/// The existing closed-registry producer, writing only canonical partitions.
pub(crate) fn trigger_sql(table: &str, record_only: bool, team_fence: bool) -> String {
    let registered = registered(table).expect("registered durable trigger source");
    assert_eq!(
        (record_only, team_fence),
        (registered.record_only, registered.team_fence)
    );
    let high = format!(
        "({})",
        crate::clock_schema::queries::High::QUERY
            .sql()
            .replace("?1", MetaKind::DB_CODE_SYNCGENERATION)
    );
    let next = crate::clock_schema::next_sequence_sql();
    let changes = Changes::NAME;
    let metadata = Metadata::NAME;
    let kind = Changes::__DB_COLUMN_KIND;
    let identity = Changes::__DB_COLUMN_IDENTITY;
    let rowid = Changes::__DB_COLUMN_SOURCE_ROWID;
    let label = Changes::__DB_COLUMN_SOURCE_LABEL;
    let id = Changes::__DB_COLUMN_DURABLE_ID;
    let seq = Changes::__DB_COLUMN_SEQUENCE;
    let deleted = Changes::__DB_COLUMN_DELETED;
    let at = Changes::__DB_COLUMN_DELETED_AT;
    let scope_column = Changes::__DB_COLUMN_SCOPE;
    let durable_kind = ChangeKind::DB_CODE_DURABLEROWS;
    let generation_kind = MetaKind::DB_CODE_SYNCGENERATION;
    let floor_kind = MetaKind::DB_CODE_DURABLEFLOOR;
    let meta_kind = MetaKind::DB_CODE_SYNCENTITYMETA;
    let integer_key = Metadata::__DB_COLUMN_INTEGER_KEY;
    let generation = Metadata::__DB_COLUMN_SYNC_GENERATION;
    let floor = Metadata::__DB_COLUMN_DURABLE_FLOOR;
    let meta_rowid = Metadata::__DB_COLUMN_SOURCE_ROWID;
    let meta_key = Metadata::__DB_COLUMN_TEXT_KEY;
    let meta_value = Metadata::__DB_COLUMN_META_VALUE;
    let now = "CAST(strftime('%s','now') AS INTEGER)";
    let cutoff = format!("{now}-{TOMBSTONE_RETENTION_SECONDS}");
    let scope =
        crate::entity_scopes::deleted_scope_sql(table, "OLD").unwrap_or_else(|| "NULL".into());
    let rowid_high =
        format!("(SELECT COALESCE(MAX({rowid}),0) FROM {changes} WHERE {kind}={durable_kind})");
    let put = |source_id: &str, is_deleted: u8, timestamp: &str, condition: &str| {
        let snapshot = if is_deleted == 1 {
            scope.as_str()
        } else {
            "NULL"
        };
        let key_sql =
            studio_db_api::migration::key_expression_sql(&[&format!("'{table}'"), source_id]);
        let increment = format!("{rowid_high}+1");
        format!(
            "INSERT INTO {changes}({kind},{identity},{rowid},{label},{id},{seq},{deleted},{at},{scope_column})
             SELECT {durable_kind},{key_sql},
               COALESCE((SELECT {rowid} FROM {changes} WHERE {kind}={durable_kind} AND {identity}={key_sql}),
                 CASE WHEN typeof({increment})='integer' THEN {increment} ELSE RAISE(ABORT,'durable source rowid exhausted') END),
               '{table}',{source_id},{next},{is_deleted},{timestamp},{snapshot}
             WHERE {condition}
             ON CONFLICT({kind},{identity}) DO UPDATE SET {seq}=excluded.{seq},{deleted}=excluded.{deleted},{at}=excluded.{at},{scope_column}=excluded.{scope_column};"
        )
    };
    let live = put("NEW.id", 0, "0", "1");
    let tombstone = put("OLD.id", 1, now, "1");
    let renamed = put("OLD.id", 1, now, "OLD.id IS NOT NEW.id");
    let require_singletons = format!(
        "SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM {metadata} WHERE {kind}={generation_kind} AND {integer_key}=1)
          OR NOT EXISTS(SELECT 1 FROM {metadata} WHERE {kind}={floor_kind} AND {integer_key}=1)
          THEN RAISE(ABORT,'missing durable clock singleton') END;"
    );
    let fence = |roots: &str, source_id: &str| {
        if !team_fence {
            return String::new();
        }
        let prefix = crate::repositories::sync_documents::TEAM_MEMBERSHIP_KEY_PREFIX;
        let key_sql = studio_db_api::migration::key_sql(&[meta_key, "null_tie"], "source_key");
        let increment = format!(
            "(SELECT COALESCE(MAX({meta_rowid}),0) FROM {metadata} WHERE {kind}={meta_kind})+ROW_NUMBER() OVER (ORDER BY {meta_key})"
        );
        format!(
            "INSERT INTO {metadata}({kind},{identity},{meta_rowid},{meta_key},{meta_value})
             SELECT {meta_kind},{key_sql},
               COALESCE((SELECT {meta_rowid} FROM {metadata} WHERE {kind}={meta_kind} AND {identity}={key_sql}),
                 CASE WHEN typeof({increment})='integer' THEN {increment} ELSE RAISE(ABORT,'metadata source rowid exhausted') END),
               {meta_key},CAST((SELECT {seq} FROM {changes} WHERE {kind}={durable_kind} AND {label}='{table}' AND {id}={source_id}) AS TEXT)
             FROM (SELECT '{prefix}' || json_quote(root) AS {meta_key},NULL AS null_tie FROM ({roots}) WHERE root IS NOT NULL) AS source_key WHERE 1
             ON CONFLICT({kind},{identity}) DO UPDATE SET {meta_value}=CAST(MAX(CAST({metadata}.{meta_value} AS INTEGER),CAST(excluded.{meta_value} AS INTEGER)) AS TEXT);"
        )
    };
    let live_root = |row: &str| {
        format!(
            "SELECT json_extract({row}.record,'$.rootId') AS root WHERE json_extract({row}.record,'$.deletedAt') IS NULL"
        )
    };
    let insert_fence = fence(&live_root("NEW"), "NEW.id");
    let update_fence = fence(
        &format!(
            "SELECT root FROM ({} UNION {}) WHERE (json_extract(OLD.record,'$.deletedAt') IS NULL) IS NOT (json_extract(NEW.record,'$.deletedAt') IS NULL) OR json_extract(OLD.record,'$.rootId') IS NOT json_extract(NEW.record,'$.rootId')",
            live_root("OLD"),
            live_root("NEW")
        ),
        "NEW.id",
    );
    let delete_fence = fence(&live_root("OLD"), "OLD.id");
    let changed = if record_only {
        " WHEN OLD.id IS NOT NEW.id OR OLD.record IS NOT NEW.record"
    } else {
        ""
    };
    format!(
        "DROP TRIGGER IF EXISTS durable_{table}_insert;
         DROP TRIGGER IF EXISTS durable_{table}_update;
         DROP TRIGGER IF EXISTS durable_{table}_delete;
         CREATE TRIGGER durable_{table}_insert AFTER INSERT ON {table} BEGIN
           {require_singletons} {live} {insert_fence}
         END;
         CREATE TRIGGER durable_{table}_update AFTER UPDATE ON {table}{changed} BEGIN
           {require_singletons} {live} {renamed} {update_fence}
         END;
         CREATE TRIGGER durable_{table}_delete AFTER DELETE ON {table} BEGIN
           {require_singletons}
           UPDATE {metadata} SET {generation}=MAX({generation},{high}) WHERE {kind}={generation_kind} AND {integer_key}=1;
           UPDATE {metadata} SET {floor}=MAX({floor},(SELECT MAX({seq}) FROM {changes} WHERE {kind}={durable_kind} AND {deleted}=1 AND {at}<{cutoff}))
             WHERE {kind}={floor_kind} AND {integer_key}=1 AND EXISTS(SELECT 1 FROM {changes} WHERE {kind}={durable_kind} AND {deleted}=1 AND {at}<{cutoff});
           DELETE FROM {changes} WHERE {kind}={durable_kind} AND {deleted}=1 AND {at}<{cutoff};
           {tombstone} {delete_fence}
         END;"
    )
}

/// Prepare target storage and number old rows in original source-rowid order.
/// F1 replaces old trigger objects atomically at authority transition.
#[cfg(test)]
pub(crate) fn install(tx: &Transaction<'_>) -> StoreResult<()> {
    for descriptor in [
        Changes::descriptor(),
        Metadata::descriptor(),
        Publications::descriptor(),
    ] {
        descriptor.install_if_absent(tx)?;
    }
    for table in DURABLE_TABLES {
        let name = table.name;
        if !crate::schema::table_exists(tx, name)? {
            continue;
        }
        if !table.online_backfill {
            for id in source_ids(tx, table)? {
                if entry(tx, table.name, &id)?.is_none() {
                    mark(tx, table.name, &id)?;
                }
            }
        }
        tx.execute_batch(&trigger_sql(name, table.record_only, table.team_fence))?;
    }
    Ok(())
}

/// Keep the persisted allocation floor before removing any numbered rows.
pub fn keep_counter(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut row = canvas_read(tx)
        .get::<SyncGeneration>(&(encode_key(&(1_i64,)),))?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    row.sync_generation = row.sync_generation.max(high(tx)?);
    canvas_tx(tx).partition::<SyncGeneration>().put(&row)?;
    crate::record_entity_metadata_updates(tx.changes());
    Ok(())
}

/// Derived agent-view labels; sequence numbers use the shared durable cursor space.
pub const AGENT_VIEW_MARK: &str = "agent_views";
/// Derived chat-view labels in the same cursor space.
pub const CHAT_VIEW_MARK: &str = "chat_views";
/// Task membership labels in the shared cursor space.
pub const TASK_WINDOW: &str = "task_window";
/// Monitor membership labels in the shared cursor space.
pub const MONITOR_WINDOW: &str = "monitor_window";

fn entry(connection: &Connection, label: &str, id: &str) -> StoreResult<Option<DurableRows>> {
    Ok(canvas_read(connection)
        .get::<DurableRows>(&(encode_key(&(label.to_owned(), id.to_owned())),))?)
}

fn next(connection: &Connection) -> StoreResult<i64> {
    canvas_read(connection)
        .get::<SyncGeneration>(&(encode_key(&(1_i64,)),))?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    high(connection)?
        .checked_add(1)
        .ok_or(StoreError::RecordContract("durable sequence exhausted"))
}

fn put_live(connection: &Connection, label: &str, id: &str, joining: bool) -> StoreResult<bool> {
    if connection.is_autocommit() {
        let transaction =
            Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)?;
        let changed = put_live(&transaction, label, id, joining)?;
        transaction.commit()?;
        return Ok(changed);
    }
    let mut previous = entry(connection, label, id)?;
    if joining
        && previous
            .as_ref()
            .is_some_and(|row| row.deleted.as_stored() != 1)
    {
        return Ok(false);
    }
    let source_rowid = match previous.as_ref() {
        Some(row) => row.source_rowid,
        None => canvas_read(connection)
            .named(
                &crate::clock_schema::queries::ChangeRowidHigh::QUERY,
                &crate::clock_schema::queries::ChangeRowidHigh {
                    kind: ChangeKind::DurableRows,
                },
            )?
            .checked_add(1)
            .ok_or(StoreError::RecordContract("durable source rowid exhausted"))?,
    };
    // The writer lock is held before the first read, including standalone callers.
    let sequence = next(connection)?;
    canvas_write(connection)?.named(
        &queries::PutLive::QUERY,
        &queries::PutLive {
            kind: ChangeKind::DurableRows,
            identity: encode_key(&(label.to_owned(), id.to_owned())),
            source_rowid,
            label: Some(label.to_owned()),
            id: Some(id.to_owned()),
            sequence: Some(sequence),
            scope: if joining {
                previous.as_mut().and_then(|row| row.scope.take())
            } else {
                None
            },
        },
    )?;
    Ok(true)
}

/// Put an absent or deleted member in its window; live and historical rows stay unchanged.
pub fn join_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    put_live(connection, tbl, id, true)
}

/// Tombstone only an exactly live window entry.
pub fn leave_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    if connection.is_autocommit() {
        let transaction =
            Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)?;
        let changed = leave_window(&transaction, tbl, id)?;
        transaction.commit()?;
        return Ok(changed);
    }
    if !entry(connection, tbl, id)?.is_some_and(|row| row.deleted.as_stored() == 0) {
        return Ok(false);
    }
    canvas_write(connection)?.named(
        &queries::LeaveWindow::QUERY,
        &queries::LeaveWindow {
            kind: ChangeKind::DurableRows,
            label: Some(tbl.to_owned()),
            id: Some(id.to_owned()),
            sequence: Some(next(connection)?),
        },
    )?;
    Ok(true)
}

/// The exactly live ids of a window, in source order.
pub fn window_members(connection: &Connection, tbl: &str) -> StoreResult<Vec<String>> {
    let source = DurableSource::from_stored(tbl.to_owned());
    Ok(studio_db_api::db_query!(
        canvas_read(connection),
        DurableRows,
        scalars(DurableRows::DURABLE_ID),
        filter(
            DurableRows::SOURCE_LABEL.eq(&source),
            DurableRows::DELETED.eq(&StoredBoolean::Valid(false))
        ),
        order_by(DurableRows::DURABLE_ID.asc)
    )?)
}

pub fn in_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    Ok(entry(connection, tbl, id)?.is_some_and(|row| row.deleted.as_stored() == 0))
}

/// Always take a fresh number and clear tombstone fields for a derived document.
pub fn mark(connection: &Connection, tbl: &str, id: &str) -> StoreResult<()> {
    put_live(connection, tbl, id, false).map(|_| ())
}

fn registered(name: &str) -> StoreResult<&'static DurableTable> {
    DURABLE_TABLES
        .iter()
        .find(|candidate| candidate.name == name)
        .ok_or_else(|| StoreError::SchemaMismatch(format!("{name} is not a durable table")))
}

// These dispatches select existing descriptor columns; they do not decode whole source records.
macro_rules! source_dispatch {
    ($table:expr, $body:ident) => {{
        match $table.name {
            "runtime_agents" => {
                type Source = crate::agent_tables::AgentsStorageRow;
                $body!(Source, RECORD)
            }
            "runtime_requests" => {
                type Source = crate::interaction_tables::RequestRow;
                $body!(Source, RECORD)
            }
            "runtime_work" => {
                type Source = crate::planning_work::WorkRow;
                $body!(Source, RECORD)
            }
            "runtime_items" => {
                type Source = crate::item_monitor_terminal::ItemsRow;
                $body!(Source, RECORD)
            }
            name if name == crate::work_history::WorkResultsStorageRow::NAME => {
                type Source = crate::work_history::WorkResultsStorageRow;
                $body!(Source, RECORD)
            }
            name if name == crate::work_history::WorkDecisionsStorageRow::NAME => {
                type Source = crate::work_history::WorkDecisionsStorageRow;
                $body!(Source, RECORD)
            }
            name if name == crate::work_history::WorkReleasesStorageRow::NAME => {
                type Source = crate::work_history::WorkReleasesStorageRow;
                $body!(Source, RECORD)
            }
            "runtime_execution_runs" => {
                type Source = crate::queue_schema::ExecutionRunsRow;
                $body!(Source, RECORD)
            }
            "runtime_plans" => {
                type Source = crate::planning_work::PlanRow;
                $body!(Source, RECORD)
            }
            "runtime_annotations" => {
                type Source = crate::planning_work::AnnotationRow;
                $body!(Source, RECORD)
            }
            "runtime_rules" => {
                type Source = crate::planning_work::RuleRow;
                $body!(Source, RECORD)
            }
            "approvals" => {
                type Source = crate::interaction_tables::ApprovalsRow;
                $body!(Source, REQUEST)
            }
            "runtime_complaints" => {
                type Source = crate::interaction_tables::ComplaintRow;
                $body!(Source, RECORD)
            }
            "graph_edges" => {
                type Source = crate::interaction_tables::GraphEdgesRow;
                $body!(Source, TARGET)
            }
            "runtime_tasks" => {
                type Source = crate::planning_work::TaskRow;
                $body!(Source, RECORD)
            }
            "runtime_monitors" => {
                type Source = crate::item_monitor_terminal::MonitorsRow;
                $body!(Source, RECORD)
            }
            _ => unreachable!("registered durable source"),
        }
    }};
}

#[cfg(test)]
fn source_ids(connection: &Connection, table: &DurableTable) -> StoreResult<Vec<String>> {
    macro_rules! ids {
        ($source:ident, $payload:ident) => {{
            type SourceRowid = studio_db_api::Rowid<$source>;
            studio_db_api::db_query!(
                canvas_read(connection),
                $source,
                scalars($source::ID),
                order_by(SourceRowid::COLUMN.asc)
            )?
        }};
    }
    Ok(source_dispatch!(table, ids))
}

fn payload(connection: &Connection, table: &DurableTable, id: &str) -> StoreResult<Option<String>> {
    let id = id.to_owned();
    macro_rules! value {
        ($source:ident, $payload:ident) => {
            studio_db_api::db_query!(
                canvas_read(connection),
                $source,
                scalar_raw($source::$payload),
                filter($source::ID.eq(&id))
            )?
        };
    }
    Ok(source_dispatch!(table, value))
}

/// A live row at or below the returned cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRow {
    pub id: String,
    pub seq: i64,
    pub payload: String,
}

/// A deleted id.
#[derive(Clone, Debug, Eq, PartialEq, studio_db_api::DbProjection)]
#[db(of = crate::clock_schema::DurableRows)]
pub struct DurableTombstone {
    pub id: String,
    pub seq: i64,
}

/// What a subscription receives. `seq` is the new cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableChanges {
    /// Every live row; the reader replaces its copy.
    Snapshot { rows: Vec<DurableRow>, seq: i64 },
    /// Rows and deletes newer than the cursor.
    Delta {
        rows: Vec<DurableRow>,
        tombstones: Vec<DurableTombstone>,
        seq: i64,
    },
}

/// The highest change number of any sync table.
pub fn high(connection: &Connection) -> StoreResult<i64> {
    Ok(crate::peer_update_settings::canvas_read(connection).named(
        &crate::clock_schema::queries::High::QUERY,
        &crate::clock_schema::queries::High {
            generation_kind: crate::consolidation_kinds::CanvasMetadataKind::SyncGeneration,
        },
    )?)
}

/// The highest change number of any sync table, and the durable tombstone floor.
pub fn high_and_floor(connection: &Connection) -> StoreResult<(i64, i64)> {
    let high = high(connection)?;
    let floor = crate::peer_update_settings::canvas_read(connection).named(
        &crate::clock_schema::queries::Floor::QUERY,
        &crate::clock_schema::queries::Floor {
            kind: MetaKind::DurableFloor,
        },
    )?;
    Ok((high, floor))
}

/// Rows of `table` with `seq` above `cursor`, or a snapshot when the cursor is absent,
/// in the future, or below the tombstone floor.
///
/// The shared high is captured before reading rows. One read snapshot covers source
/// projections and change entries, so a later payload cannot acquire an older number.
pub fn changes_since(
    connection: &Connection,
    table: &str,
    cursor: Option<i64>,
) -> StoreResult<DurableChanges> {
    if connection.is_autocommit() {
        let transaction =
            Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Deferred)?;
        let changes = changes_since(&transaction, table, cursor)?;
        transaction.commit()?;
        return Ok(changes);
    }
    let table = DURABLE_TABLES
        .iter()
        .find(|candidate| candidate.name == table)
        .ok_or_else(|| StoreError::SchemaMismatch(format!("{table} is not a durable table")))?;
    let (high, floor) = high_and_floor(connection)?;
    let after = cursor.filter(|cursor| (floor..=high).contains(cursor));
    let rows = |after: i64| -> StoreResult<Vec<DurableRow>> {
        let keys = canvas_read(connection).collect(
            &queries::LiveKeys::QUERY,
            &queries::LiveKeys {
                label: Some(table.name),
                after: Some(after),
                high: Some(high),
            },
        )?;
        let mut rows = Vec::with_capacity(keys.len());
        for identity in keys {
            let entry = canvas_read(connection)
                .get::<DurableRows>(&(identity,))?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if let Some(payload) = payload(connection, table, &entry.durable_id)? {
                rows.push(DurableRow {
                    id: entry.durable_id,
                    seq: entry.sequence,
                    payload,
                });
            }
        }
        Ok(rows)
    };
    let Some(after) = after else {
        return Ok(DurableChanges::Snapshot {
            rows: rows(0)?,
            seq: high,
        });
    };
    let tombstones = crate::peer_update_settings::canvas_read(connection).collect(
        &crate::clock_schema::queries::Tombstones::QUERY,
        &crate::clock_schema::queries::Tombstones {
            table: table.name,
            after,
            high,
        },
    )?;
    Ok(DurableChanges::Delta {
        rows: rows(after)?,
        tombstones,
        seq: high,
    })
}

#[derive(studio_db_api::DbProjection)]
#[db(of=Publications)]
struct MissingCopy {
    collection_agent: Option<String>,
    entity_id: Option<String>,
}

/// Live transcript index entries whose item is absent from its canonical owner.
pub fn transcript_copy_missing_from_owner(
    connection: &Connection,
) -> StoreResult<Vec<(String, String)>> {
    canvas_read(connection)
        .collect(
            &queries::MissingCopies::QUERY,
            &queries::MissingCopies {
                kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
                collection_kind: Some(crate::publication_collection::CollectionKind::Transcript),
            },
        )?
        .into_iter()
        .map(|row| {
            let agent = row.collection_agent.ok_or(rusqlite::Error::InvalidQuery)?;
            let item = row.entity_id.ok_or(rusqlite::Error::InvalidQuery)?;
            Ok((agent, item[5..].to_owned()))
        })
        .collect()
}

const COPIES_KEY: &str = "transcript_copies";
const COPIES_RETIRED: &str = "retired";

pub fn transcript_copies_retired(connection: &Connection) -> StoreResult<bool> {
    let key = Some(COPIES_KEY.to_owned());
    Ok(canvas_read(connection)
        .get::<crate::metadata_schema::SyncEntityMeta>(&(
            crate::consolidation_codec::nullable_text_identity(&key, 0),
        ))?
        .is_some_and(|row| row.meta_value.as_stored() == COPIES_RETIRED))
}

/// Refuse retirement until every live copy has an owner; preserve owner and copy rows.
pub fn retire_transcript_copies(tx: &Transaction<'_>) -> StoreResult<Vec<(String, String)>> {
    use crate::metadata_schema::{StoredSyncMetaValue, SyncEntityMeta};
    let missing = transcript_copy_missing_from_owner(tx)?;
    if missing.is_empty() {
        let key = Some(COPIES_KEY.to_owned());
        let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
        let source_rowid = match canvas_read(tx).get::<SyncEntityMeta>(&(identity.clone(),))? {
            Some(row) => row.source_rowid,
            None => canvas_read(tx)
                .named(
                    &crate::clock_schema::queries::MetaRowidHigh::QUERY,
                    &crate::clock_schema::queries::MetaRowidHigh {
                        kind: MetaKind::SyncEntityMeta,
                    },
                )?
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(StoreError::RecordContract(
                    "metadata source rowid exhausted",
                ))?,
        };
        canvas_tx(tx)
            .partition::<SyncEntityMeta>()
            .put(&SyncEntityMeta {
                identity,
                source_rowid,
                meta_value: StoredSyncMetaValue::from_stored(key.clone(), COPIES_RETIRED.into()),
                text_key: key,
            })?;
        crate::record_entity_metadata_updates(tx.changes());
    }
    Ok(missing)
}

/// The cursor of exactly one agent, its live items, and its scoped item tombstones.
pub fn transcript_change_cursor(connection: &Connection, agent: &str) -> StoreResult<i64> {
    Ok(canvas_read(connection)
        .named(
            &queries::TranscriptCursor::QUERY,
            &queries::TranscriptCursor {
                kind: ChangeKind::DurableRows,
                agent,
            },
        )?
        .unwrap_or(0))
}

#[studio_db_api::db_queries]
pub(crate) mod queries {
    use super::*;
    use studio_db_api::DbQuery;

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,result=Vec<u8>,decode=Changes::IDENTITY,
        tables(rows=Changes),indexes(rows=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=rows),
        columns(identity=Changes::IDENTITY,label=Changes::SOURCE_LABEL,seq=Changes::SEQUENCE,deleted=Changes::DELETED),
        sql="SELECT {identity} FROM {rows} WHERE {durable} AND {label}=?1 AND {deleted}=0 AND {seq}>?2 AND {seq}<=?3 ORDER BY {seq}")]
    pub struct LiveKeys<'a> {
        #[db(column=Changes::SOURCE_LABEL)]
        pub label: Option<&'a str>,
        #[db(column=Changes::SEQUENCE)]
        pub after: Option<i64>,
        #[db(column=Changes::SEQUENCE)]
        pub high: Option<i64>,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=Changes),
        columns(kind=Changes::KIND,identity=Changes::IDENTITY,rowid=Changes::SOURCE_ROWID,
            label=Changes::SOURCE_LABEL,id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,
            deleted=Changes::DELETED,at=Changes::DELETED_AT,scope=Changes::SCOPE),
        sql="INSERT INTO {rows}({kind},{identity},{rowid},{label},{id},{seq},{deleted},{at},{scope})
            VALUES(?1,?2,?3,?4,?5,?6,0,0,?7)
            ON CONFLICT({kind},{identity}) DO UPDATE SET {seq}=excluded.{seq},{deleted}=0,{at}=0,{scope}=excluded.{scope}")]
    pub struct PutLive {
        #[db(column=Changes::KIND)]
        pub kind: ChangeKind,
        #[db(column=Changes::IDENTITY)]
        pub identity: Vec<u8>,
        #[db(column=Changes::SOURCE_ROWID)]
        pub source_rowid: i64,
        #[db(column=Changes::SOURCE_LABEL)]
        pub label: Option<String>,
        #[db(column=Changes::DURABLE_ID)]
        pub id: Option<String>,
        #[db(column=Changes::SEQUENCE)]
        pub sequence: Option<i64>,
        #[db(column=Changes::SCOPE)]
        pub scope: Option<crate::json_record_codec::ExactJson<crate::entity_scopes::DeletedScope>>,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=Changes),
        columns(kind=Changes::KIND,label=Changes::SOURCE_LABEL,id=Changes::DURABLE_ID,
            seq=Changes::SEQUENCE,deleted=Changes::DELETED,at=Changes::DELETED_AT),
        sql="UPDATE {rows} SET {seq}=?4,{deleted}=1,{at}=CAST(strftime('%s','now') AS INTEGER)
            WHERE {kind}=?1 AND {label}=?2 AND {id}=?3 AND {deleted}=0")]
    pub struct LeaveWindow {
        #[db(column=Changes::KIND)]
        pub kind: ChangeKind,
        #[db(column=Changes::SOURCE_LABEL)]
        pub label: Option<String>,
        #[db(column=Changes::DURABLE_ID)]
        pub id: Option<String>,
        #[db(column=Changes::SEQUENCE)]
        pub sequence: Option<i64>,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,result=super::MissingCopy,projection=super::MissingCopy,
        tables(rows=Publications,items=crate::item_monitor_terminal::ItemsRow),
        columns(kind=Publications::KIND,collection_kind=Publications::COLLECTION_KIND,
            collection=Publications::COLLECTION_ORDER,agent=Publications::COLLECTION_AGENT,
            id=Publications::ENTITY_ID,deleted=Publications::DELETED,
            item_id=crate::item_monitor_terminal::ItemsRow::ID,item_agent=crate::item_monitor_terminal::ItemsRow::AGENT),
        sql="SELECT s.{agent} AS {agent},s.{id} AS {id} FROM {rows} s
            WHERE s.{kind}=?1 AND s.{collection_kind}=?2 AND s.{id} LIKE 'item:%' AND s.{deleted}=0
            AND NOT EXISTS(SELECT 1 FROM {items} i WHERE i.{item_agent}=s.{agent} AND i.{item_id}=substr(s.{id},6))
            ORDER BY s.{collection},s.{id}")]
    pub struct MissingCopies {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
        #[db(column=Publications::COLLECTION_KIND)]
        pub collection_kind: Option<crate::publication_collection::CollectionKind>,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,decode=Changes::SEQUENCE,
        tables(rows=Changes,items=crate::item_monitor_terminal::ItemsRow),
        columns(kind=Changes::KIND,label=Changes::SOURCE_LABEL,id=Changes::DURABLE_ID,
            seq=Changes::SEQUENCE,deleted=Changes::DELETED,scope=Changes::SCOPE,
            item_id=crate::item_monitor_terminal::ItemsRow::ID,item_agent=crate::item_monitor_terminal::ItemsRow::AGENT),
        sql="SELECT MAX(
            COALESCE((SELECT {seq} FROM {rows} WHERE {kind}=?1 AND {label}='runtime_agents' AND {id}=?2),0),
            COALESCE((SELECT MAX(d.{seq}) FROM {items} i JOIN {rows} d ON d.{kind}=?1 AND d.{label}='runtime_items' AND d.{id}=i.{item_id} WHERE i.{item_agent}=?2),0),
            COALESCE((SELECT MAX({seq}) FROM {rows} WHERE {kind}=?1 AND {label}='runtime_items' AND {deleted}=1 AND json_extract({scope},'$.agent')=?2),0)) AS {seq}")]
    pub struct TranscriptCursor<'a> {
        #[db(column=Changes::KIND)]
        pub kind: ChangeKind,
        #[db(column=crate::item_monitor_terminal::ItemsRow::AGENT)]
        pub agent: &'a str,
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use studio_db_api::{DbQuery, db_delete};

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=crate::planning_work::WorkRow),columns(id=crate::planning_work::WorkRow::ID),
        sql="UPDATE {rows} SET {id}=?2 WHERE {id}=?1")]
    struct RenameWork<'a> {
        #[db(column=crate::planning_work::WorkRow::ID)]
        old: &'a str,
        #[db(column=crate::planning_work::WorkRow::ID)]
        new: &'a str,
    }

    fn age_tombstones(tx: &Transaction<'_>) {
        for mut row in studio_db_api::db_query!(
            canvas_read(tx),
            DurableRows,
            filter(DurableRows::DELETED.eq(&StoredBoolean::Valid(true)))
        )
        .unwrap()
        {
            row.deleted_at = 0;
            canvas_tx(tx).partition::<DurableRows>().put(&row).unwrap();
        }
    }

    fn seed_durable(tx: &Transaction<'_>, label: &str, id: &str, sequence: i64, deleted: i64) {
        let row = DurableRows {
            identity: encode_key(&(label.to_owned(), id.to_owned())),
            source_rowid: sequence,
            source_label: DurableSource::from_stored(label.into()),
            durable_id: id.into(),
            sequence,
            deleted: StoredBoolean::from_stored(deleted),
            deleted_at: 0,
            scope: None,
        };
        canvas_tx(tx).partition::<DurableRows>().put(&row).unwrap();
    }

    fn seed_entity(db: &Connection, label: &str, id: &str, sequence: i64, deleted: bool) {
        use crate::publication_collection::{CollectionKind, CollectionName};
        use crate::publication_schema::{EntityValue, SyncEntities};
        let collection = CollectionName::from_stored(label.into());
        let deleted = StoredBoolean::Valid(deleted);
        let row = SyncEntities {
            identity: encode_key(&(label.to_owned(), id.to_owned())),
            source_rowid: sequence,
            publication_seq: sequence,
            deleted,
            collection_kind: collection.kind(),
            collection_agent: collection.agent().cloned(),
            non_transcript: collection.non_transcript(),
            collection_order: collection,
            entity_id: id.into(),
            entity_hash: if CollectionKind::of(label) == CollectionKind::Transcript {
                "0".repeat(64)
            } else {
                "h".into()
            },
            entity_payload: EntityValue::from_stored(label, id, deleted, None).unwrap(),
        };
        let transaction = db.unchecked_transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<SyncEntities>()
            .put(&row)
            .unwrap();
        transaction.commit().unwrap();
    }

    fn put_agent(tx: &Transaction<'_>, id: &str, root: &str, deleted: Option<i64>) {
        let deleted = deleted.map_or_else(|| "null".to_owned(), |value| value.to_string());
        let raw = format!(
            r#"{{"id":"{id}","rootId":"{root}","provider":"codex","accountKey":"default","epoch":1,"status":"idle","deletedAt":{deleted}}}"#
        );
        canvas_tx(tx)
            .upsert(&crate::agent_tables::AgentsStorageRow {
                account_key: String::new(),
                id: id.into(),
                root_id: None,
                deleted_at: None,
                record: crate::json_record_codec::RawJsonRecord::from_stored(tx, id, raw).unwrap(),
            })
            .unwrap();
    }

    fn canonical_canvas() -> Connection {
        use studio_db_api::Table;
        let mut db = Connection::open_in_memory().unwrap();
        for descriptor in [
            crate::change_schema::Changes::descriptor(),
            crate::metadata_schema::Metadata::descriptor(),
            crate::publication_schema::Publications::descriptor(),
            crate::planning_work::WorkRow::descriptor(),
            crate::planning_work::RuleRow::descriptor(),
            crate::item_monitor_terminal::ItemsRow::descriptor(),
            crate::agent_tables::AgentsStorageRow::descriptor(),
            crate::agent_tables::AgentVolatileStorageRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        let transaction = db.transaction().unwrap();
        let mut tx = crate::peer_update_settings::canvas_tx(&transaction);
        tx.partition::<crate::metadata_schema::SyncGeneration>()
            .put(&crate::metadata_schema::SyncGeneration {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                integer_key: 1,
                source_rowid: 1,
                sync_generation: 0,
            })
            .unwrap();
        tx.partition::<crate::metadata_schema::DurableFloor>()
            .put(&crate::metadata_schema::DurableFloor {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                integer_key: 1,
                source_rowid: 1,
                durable_floor: 0,
            })
            .unwrap();
        transaction.commit().unwrap();
        db
    }

    #[test]
    fn canonical_windows_restore_delete_marks_and_rollback() {
        let mut db = canonical_canvas();
        assert!(join_window(&db, TASK_WINDOW, "id").unwrap());
        let first = high(&db).unwrap();
        assert!(!join_window(&db, TASK_WINDOW, "id").unwrap());
        assert_eq!(high(&db).unwrap(), first);
        assert!(!in_window(&db, MONITOR_WINDOW, "id").unwrap());
        assert!(leave_window(&db, TASK_WINDOW, "id").unwrap());
        let removed = high(&db).unwrap();
        assert!(removed > first);
        assert!(!leave_window(&db, TASK_WINDOW, "id").unwrap());
        assert!(!leave_window(&db, TASK_WINDOW, "missing").unwrap());
        let tx = db.transaction().unwrap();
        let mut tombstone = entry(&tx, TASK_WINDOW, "id").unwrap().unwrap();
        tombstone.scope = Some(
            crate::json_record_codec::ExactJson::from_stored(
                "{ \"agent\": \"owner\", \"future\": 17 }".into(),
            )
            .unwrap(),
        );
        canvas_tx(&tx)
            .partition::<DurableRows>()
            .put(&tombstone)
            .unwrap();
        tx.commit().unwrap();
        assert!(join_window(&db, TASK_WINDOW, "id").unwrap());
        assert!(high(&db).unwrap() > removed);
        let restored = entry(&db, TASK_WINDOW, "id").unwrap().unwrap();
        assert_eq!(restored.deleted_at, 0);
        assert_eq!(
            restored.scope.unwrap().raw(),
            "{ \"agent\": \"owner\", \"future\": 17 }"
        );
        let before = high(&db).unwrap();
        let tx = db.transaction().unwrap();
        mark(&tx, TASK_WINDOW, "id").unwrap();
        mark(&tx, CHAT_VIEW_MARK, "chat").unwrap();
        assert!(high(&tx).unwrap() > before);
        tx.rollback().unwrap();
        assert_eq!(high(&db).unwrap(), before);
        assert_eq!(window_members(&db, TASK_WINDOW).unwrap(), ["id"]);
        assert!(window_members(&db, CHAT_VIEW_MARK).unwrap().is_empty());
    }

    #[test]
    fn canonical_producers_number_and_resume_without_legacy_tables() {
        let mut db = canonical_canvas();
        let tx = db.transaction().unwrap();
        install(&tx).unwrap();
        tx.commit().unwrap();
        put(&db, "a", "{ \"x\": 1 }");
        let first = high(&db).unwrap();
        put(&db, "a", "{ \"x\": 2 }");
        let (rows, deleted, next) = delta(&db, first);
        assert_eq!(rows, ["a"]);
        assert!(deleted.is_empty());
        assert!(next > first);
    }

    #[test]
    fn canonical_rename_orders_live_before_scoped_tombstone() {
        let mut db = canvas();
        put(&db, "old", r#"{"rootId":"team","payload":"exact"}"#);
        let old_rowid = entry(&db, "runtime_work", "old")
            .unwrap()
            .unwrap()
            .source_rowid;
        let tx = db.transaction().unwrap();
        canvas_tx(&tx)
            .named(
                &RenameWork::QUERY,
                &RenameWork {
                    old: "old",
                    new: "new",
                },
            )
            .unwrap();
        tx.commit().unwrap();
        let live = entry(&db, "runtime_work", "new").unwrap().unwrap();
        let old = entry(&db, "runtime_work", "old").unwrap().unwrap();
        assert_eq!(old.source_rowid, old_rowid);
        assert!(live.source_rowid > old.source_rowid);
        assert!(live.sequence < old.sequence);
        assert_eq!(live.deleted.as_stored(), 0);
        assert!(live.scope.is_none());
        assert_eq!(old.deleted.as_stored(), 1);
        assert_eq!(
            old.scope.as_ref().unwrap().value().root_id.as_deref(),
            Some("team")
        );
        let marked_rowid = old.source_rowid;
        mark(&db, "runtime_work", "old").unwrap();
        let marked = entry(&db, "runtime_work", "old").unwrap().unwrap();
        assert_eq!(marked.source_rowid, marked_rowid);
        assert_eq!((marked.deleted.as_stored(), marked.deleted_at), (0, 0));
        assert!(marked.scope.is_none());
    }

    #[test]
    fn canonical_pruning_keeps_floor_and_generation_above_deleted_high() {
        use crate::planning_work::WorkRow;
        let mut db = canvas();
        put(&db, "a", "{}");
        put(&db, "b", "{}");
        let tx = db.transaction().unwrap();
        db_delete!(
            canvas_tx(&tx),
            WorkRow,
            filter(WorkRow::ID.eq(&"a".to_owned()))
        )
        .unwrap();
        let removed = high(&tx).unwrap();
        age_tombstones(&tx);
        db_delete!(
            canvas_tx(&tx),
            WorkRow,
            filter(WorkRow::ID.eq(&"b".to_owned()))
        )
        .unwrap();
        assert!(entry(&tx, "runtime_work", "a").unwrap().is_none());
        assert_eq!(high_and_floor(&tx).unwrap().1, removed);
        assert!(high(&tx).unwrap() > removed);
        keep_counter(&tx).unwrap();
        db_delete!(canvas_tx(&tx), DurableRows).unwrap();
        assert!(high(&tx).unwrap() > removed);
        tx.rollback().unwrap();
    }

    #[test]
    fn canonical_exact_boolean_labels_overflow_and_missing_singleton_refuse() {
        let mut db = canonical_canvas();
        let tx = db.transaction().unwrap();
        seed_durable(&tx, TASK_WINDOW, "historical", 3, -7);
        assert!(!in_window(&tx, TASK_WINDOW, "historical").unwrap());
        assert!(!join_window(&tx, TASK_WINDOW, "historical").unwrap());
        assert!(!leave_window(&tx, TASK_WINDOW, "historical").unwrap());
        assert!(window_members(&tx, TASK_WINDOW).unwrap().is_empty());
        assert!(changes_since(&tx, "runtime_events", None).is_err());
        let mut generation = canvas_read(&tx)
            .get::<SyncGeneration>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        generation.sync_generation = i64::MAX;
        canvas_tx(&tx)
            .partition::<SyncGeneration>()
            .put(&generation)
            .unwrap();
        assert!(mark(&tx, AGENT_VIEW_MARK, "a").is_err());
        assert!(entry(&tx, AGENT_VIEW_MARK, "a").unwrap().is_none());
        canvas_tx(&tx)
            .delete::<SyncGeneration>(&(encode_key(&(1_i64,)),))
            .unwrap();
        assert!(keep_counter(&tx).is_err());
        assert!(mark(&tx, AGENT_VIEW_MARK, "a").is_err());
        tx.rollback().unwrap();
    }

    #[test]
    fn canonical_team_fences_keep_both_roots_and_sql_null_eligibility() {
        let mut db = canvas();
        let tx = db.transaction().unwrap();
        let fence = |root: &str| {
            let key = Some(format!(
                "{}\"{root}\"",
                crate::repositories::sync_documents::TEAM_MEMBERSHIP_KEY_PREFIX
            ));
            canvas_read(&tx)
                .get::<crate::metadata_schema::SyncEntityMeta>(&(
                    crate::consolidation_codec::nullable_text_identity(&key, 0),
                ))
                .unwrap()
                .map_or(0, |row| row.meta_value.as_stored().parse::<i64>().unwrap())
        };
        put_agent(&tx, "member", "old", None);
        let first = fence("old");
        assert!(first > 0);
        put_agent(&tx, "member", "new", None);
        let moved = fence("new");
        assert!(moved > first);
        assert_eq!(fence("old"), moved);
        put_agent(&tx, "member", "new", Some(0));
        let removed = fence("new");
        assert!(removed > moved, "zero is SQL non-NULL and leaves the team");
        put_agent(&tx, "member", "new", Some(0));
        assert_eq!(fence("new"), removed);
        put_agent(&tx, "excluded", "excluded", Some(0));
        assert_eq!(fence("excluded"), 0);
        put_agent(&tx, "member", "new", None);
        assert!(fence("new") > removed);
        let latest = fence("new");
        db_delete!(
            canvas_tx(&tx),
            crate::agent_tables::AgentsStorageRow,
            filter(crate::agent_tables::AgentsStorageRow::ID.eq(&"member".to_owned()))
        )
        .unwrap();
        assert!(fence("new") > latest);
        tx.rollback().unwrap();
    }

    #[test]
    fn canonical_online_numbering_is_idempotent_and_unknown_sources_refuse() {
        use crate::item_monitor_terminal::ItemsRow;
        use crate::online_migration::OnlineMigration;
        let backfill = crate::item_seq_backfill::ItemSeqBackfill;
        type ItemRowid = studio_db_api::Rowid<ItemsRow>;
        let mut db = canonical_canvas();
        put_item(&db, "old", "owner");
        let ids = studio_db_api::db_query!(canvas_read(&db), ItemsRow, scalars(ItemRowid::COLUMN))
            .unwrap();
        let tx = db.transaction().unwrap();
        install(&tx).unwrap();
        assert!(entry(&tx, "runtime_items", "old").unwrap().is_none());
        backfill.write(&mut canvas_tx(&tx), &ids[0], ()).unwrap();
        let first = entry(&tx, "runtime_items", "old").unwrap().unwrap();
        backfill.write(&mut canvas_tx(&tx), &ids[0], ()).unwrap();
        let repeated = entry(&tx, "runtime_items", "old").unwrap().unwrap();
        assert_eq!(
            (first.sequence, first.source_rowid),
            (repeated.sequence, repeated.source_rowid)
        );
        backfill.write(&mut canvas_tx(&tx), &i64::MAX, ()).unwrap();
        assert!(changes_since(&tx, "runtime_unknown", None).is_err());
        tx.rollback().unwrap();
    }

    #[test]
    fn canonical_delta_and_window_reads_are_bounded_by_the_selected_label() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = canvas();
        let tx = db.transaction().unwrap();
        for n in 1..=2000 {
            seed_durable(&tx, "unrelated", &format!("other-{n}"), n, 0);
            seed_durable(&tx, TASK_WINDOW, &format!("retained-{n}"), 2000 + n, 1);
            // Keep retained history below the delta cursor while its source rowid stays unique.
            let identity = encode_key(&(TASK_WINDOW.to_owned(), format!("retained-{n}")));
            let mut retained = canvas_read(&tx)
                .get::<DurableRows>(&(identity,))
                .unwrap()
                .unwrap();
            retained.sequence = n;
            canvas_tx(&tx)
                .partition::<DurableRows>()
                .put(&retained)
                .unwrap();
        }
        tx.commit().unwrap();
        put(&db, "selected", "{}");
        join_window(&db, TASK_WINDOW, "selected").unwrap();
        let steps = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&steps);
        db.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .unwrap();
        high(&db).unwrap();
        let high_steps = steps.swap(0, Ordering::Relaxed);
        assert_eq!(delta(&db, 2000).0, ["selected"]);
        let delta_steps = steps.swap(0, Ordering::Relaxed);
        assert_eq!(window_members(&db, TASK_WINDOW).unwrap(), ["selected"]);
        let count = steps.load(Ordering::Relaxed);
        db.progress_handler(0, None::<fn() -> bool>).unwrap();
        let plan = crate::query_plan_test_support::explain(
            &db,
            queries::LiveKeys::QUERY.sql(),
            &queries::LiveKeys {
                label: Some("runtime_work"),
                after: Some(2000),
                high: Some(high(&db).unwrap()),
            },
        )
        .unwrap();
        let index = studio_db_api::query_index::<Canvas, Changes>(
            &Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE,
        );
        assert!(
            plan.iter().any(|step| step.contains(index)
                && step.contains("source_label=?")
                && step.contains("sequence>")),
            "{plan:?}"
        );
        assert!(
            high_steps + delta_steps + count < 1000,
            "selected-label reads used high={high_steps}, delta={delta_steps}, window={count} VM steps"
        );
    }

    #[test]
    fn canonical_hash_replays_and_documents_share_the_durable_allocator() {
        use crate::publication_schema::{DocumentValue, SyncDocuments};
        use crate::repositories::sync_documents::SyncDocumentsRepository;
        let mut db = canvas();
        let tx = db.transaction().unwrap();
        canvas_tx(&tx)
            .partition::<SyncDocuments>()
            .put(&SyncDocuments {
                identity: encode_key(&(19_i64,)),
                source_rowid: 19,
                publication_seq: 19,
                deleted: StoredBoolean::Valid(false),
                document_scope: "drafts".into(),
                document_id: "d".into(),
                document_payload: DocumentValue::from_stored("drafts", "d", "{}".into()).unwrap(),
            })
            .unwrap();
        assert_eq!(high(&tx).unwrap(), 19);
        let hash = "0".repeat(64);
        assert!(
            SyncDocumentsRepository::put_entity_hash(
                &tx,
                "transcript:owner",
                "item:one",
                &hash,
                false
            )
            .unwrap()
        );
        let published = high(&tx).unwrap();
        assert_eq!(published, 20);
        assert!(
            !SyncDocumentsRepository::put_entity_hash(
                &tx,
                "transcript:owner",
                "item:one",
                &hash,
                false
            )
            .unwrap()
        );
        assert_eq!(high(&tx).unwrap(), published);
        mark(&tx, AGENT_VIEW_MARK, "owner").unwrap();
        assert_eq!(high(&tx).unwrap(), 21);
        tx.rollback().unwrap();
    }

    #[test]
    fn canonical_producer_and_source_rowid_overflows_leave_no_partial_write() {
        let mut db = canvas();
        put(&db, "a", "{}");
        let tx = db.transaction().unwrap();
        let key = (encode_key(&(1_i64,)),);
        let mut generation = canvas_read(&tx)
            .get::<SyncGeneration>(&key)
            .unwrap()
            .unwrap();
        generation.sync_generation = i64::MAX;
        canvas_tx(&tx)
            .partition::<SyncGeneration>()
            .put(&generation)
            .unwrap();
        assert!(
            canvas_tx(&tx)
                .named(
                    &RenameWork::QUERY,
                    &RenameWork {
                        old: "a",
                        new: "overflow"
                    }
                )
                .is_err()
        );
        assert!(entry(&tx, "runtime_work", "overflow").unwrap().is_none());
        generation.sync_generation = 0;
        canvas_tx(&tx)
            .partition::<SyncGeneration>()
            .put(&generation)
            .unwrap();
        let mut row = entry(&tx, "runtime_work", "a").unwrap().unwrap();
        row.source_rowid = i64::MAX;
        canvas_tx(&tx).partition::<DurableRows>().put(&row).unwrap();
        assert!(mark(&tx, AGENT_VIEW_MARK, "new").is_err());
        assert!(
            canvas_tx(&tx)
                .named(
                    &RenameWork::QUERY,
                    &RenameWork {
                        old: "a",
                        new: "overflow"
                    }
                )
                .is_err()
        );
        mark(&tx, "runtime_work", "a").unwrap();
        assert_eq!(
            entry(&tx, "runtime_work", "a")
                .unwrap()
                .unwrap()
                .source_rowid,
            i64::MAX
        );
        tx.rollback().unwrap();
    }

    #[test]
    fn named_clock_queries_validate_fresh_schema_and_missing_floor() {
        let mut db = canvas();
        crate::clock_schema::queries::validate_queries(&db).unwrap();
        queries::validate_queries(&db).unwrap();
        let tx = db.transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .delete::<crate::metadata_schema::DurableFloor>(&(encode_key(&(1_i64,)),))
            .unwrap();
        assert!(matches!(
            high_and_floor(&tx),
            Err(StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
        ));
    }

    #[test]
    fn named_tombstones_keep_cutoff_order_and_filtering() {
        use crate::clock_schema::queries::Tombstones;
        let mut db = canvas();
        let tx = db.transaction().unwrap();
        let adapter = crate::peer_update_settings::canvas_tx(&tx);
        for (table, id, seq, deleted) in [
            ("runtime_work", "lower", 2, 1),
            ("runtime_work", "live", 3, 0),
            ("runtime_work", "middle", 4, 1),
            ("other", "other", 5, 1),
            ("runtime_work", "upper", 6, 1),
            ("runtime_work", "future", 7, 1),
        ] {
            seed_durable(&tx, table, id, seq, deleted);
        }
        let params = Tombstones {
            table: "runtime_work",
            after: 2,
            high: 6,
        };
        let result = adapter.read().collect(&Tombstones::QUERY, &params).unwrap();
        assert_eq!(
            result,
            vec![
                DurableTombstone {
                    id: "middle".to_owned(),
                    seq: 4
                },
                DurableTombstone {
                    id: "upper".to_owned(),
                    seq: 6
                }
            ]
        );
        assert!(
            adapter
                .read()
                .collect(&Tombstones::QUERY, &Tombstones { after: 6, ..params })
                .unwrap()
                .is_empty()
        );
    }

    fn canvas() -> Connection {
        let mut db = canonical_canvas();
        let tx = db.transaction().unwrap();
        install(&tx).unwrap();
        tx.commit().unwrap();
        db
    }

    fn put_item(db: &Connection, id: &str, agent: &str) {
        let raw = format!(
            r#"{{"id":"{id}","agent":"{agent}","role":"assistant","text":"fixture","created":1}}"#
        );
        let row = crate::item_monitor_terminal::ItemsRow {
            id: id.into(),
            agent: agent.into(),
            created: 1.0,
            record: crate::json_record_codec::RawJsonRecord::from_stored(db, id, raw).unwrap(),
        };
        let transaction = db.unchecked_transaction().unwrap();
        canvas_tx(&transaction).insert(&row).unwrap();
        transaction.commit().unwrap();
    }

    fn put(db: &Connection, id: &str, record: &str) {
        db.execute(
            "INSERT INTO runtime_work(id,record) VALUES(?1,?2)
             ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            [id, record],
        )
        .expect("write");
    }

    fn seq_of(db: &Connection, id: &str) -> i64 {
        entry(db, "runtime_work", id).unwrap().unwrap().sequence
    }

    fn delta(db: &Connection, cursor: i64) -> (Vec<String>, Vec<String>, i64) {
        match changes_since(db, "runtime_work", Some(cursor)).expect("changes") {
            DurableChanges::Delta {
                rows,
                tombstones,
                seq,
            } => (
                rows.into_iter().map(|row| row.id).collect(),
                tombstones.into_iter().map(|t| t.id).collect(),
                seq,
            ),
            DurableChanges::Snapshot { .. } => panic!("expected a delta"),
        }
    }

    #[test]
    fn insert_update_and_delete_take_increasing_numbers() {
        let db = canvas();
        put(&db, "a", "{}");
        let first = seq_of(&db, "a");
        put(&db, "b", "{}");
        let second = seq_of(&db, "b");
        put(&db, "a", r#"{"x":1}"#);
        let updated = seq_of(&db, "a");
        assert!(first > 0 && second > first && updated > second);
        db.execute("DELETE FROM runtime_work WHERE id='b'", [])
            .expect("delete");
        let row = entry(&db, "runtime_work", "b").unwrap().unwrap();
        assert_eq!(row.deleted.as_stored(), 1);
        let tombstone = row.sequence;
        assert!(tombstone > updated);
    }

    #[test]
    fn a_cursor_resume_returns_exactly_the_changes() {
        let db = canvas();
        put(&db, "a", "{}");
        put(&db, "b", "{}");
        let (_, _, cursor) = delta(&db, 0);
        assert_eq!(delta(&db, cursor), (vec![], vec![], cursor), "nothing new");
        put(&db, "c", "{}");
        put(&db, "a", r#"{"x":1}"#);
        db.execute("DELETE FROM runtime_work WHERE id='b'", [])
            .expect("delete");
        let (mut rows, tombstones, next) = delta(&db, cursor);
        rows.sort();
        assert_eq!(
            (rows, tombstones),
            (vec!["a".into(), "c".into()], vec!["b".into()])
        );
        assert!(next > cursor);
        put(&db, "b", "{}");
        let (rows, tombstones, _) = delta(&db, next);
        assert_eq!(
            (rows, tombstones),
            (vec!["b".into()], vec![]),
            "reinsert clears the tombstone"
        );
    }

    #[test]
    fn a_cursor_older_than_the_kept_tombstones_gets_a_snapshot() {
        let db = canvas();
        put(&db, "a", "{}");
        put(&db, "b", "{}");
        let (_, _, cursor) = delta(&db, 0);
        db.execute("DELETE FROM runtime_work WHERE id='a'", [])
            .expect("delete");
        // Age the tombstone past the retention, then the next delete prunes it.
        let tx = db.unchecked_transaction().unwrap();
        age_tombstones(&tx);
        tx.commit().unwrap();
        db.execute("DELETE FROM runtime_work WHERE id='b'", [])
            .expect("delete");
        match changes_since(&db, "runtime_work", Some(cursor)).expect("changes") {
            DurableChanges::Snapshot { rows, .. } => assert!(rows.is_empty()),
            other @ DurableChanges::Delta { .. } => panic!("expected a snapshot, got {other:?}"),
        }
        assert!(matches!(
            changes_since(&db, "runtime_work", None).expect("changes"),
            DurableChanges::Snapshot { .. }
        ));
        let high = high(&db).expect("counter");
        assert!(
            matches!(
                changes_since(&db, "runtime_work", Some(high + 1)).expect("changes"),
                DurableChanges::Snapshot { .. }
            ),
            "a future cursor is foreign"
        );
        assert!(matches!(
            changes_since(&db, "runtime_work", Some(high)).expect("changes"),
            DurableChanges::Delta { .. }
        ));
    }

    #[test]
    fn durable_and_sync_writes_share_one_increasing_counter() {
        let db = canvas();
        let next = || crate::repositories::sync_documents::next_sequence(&db).expect("next");
        put(&db, "a", "{}");
        let first = seq_of(&db, "a");
        assert_eq!(
            next(),
            first + 1,
            "a sync allocation sees the durable number"
        );
        let entity = next();
        seed_entity(&db, "x", "1", entity, false);
        put(&db, "b", "{}");
        assert_eq!(
            seq_of(&db, "b"),
            entity + 1,
            "a durable number follows the entity"
        );
        // A cursor taken from the entity stream is valid here and returns only what followed.
        let (rows, _, _) = delta(&db, entity);
        assert_eq!(rows, vec!["b".to_owned()]);
    }

    #[test]
    fn keep_counter_stops_a_deleted_max_seq_from_being_reused() {
        let mut db = canvas();
        seed_entity(&db, "agentSummary", "a", 5, false);
        seed_entity(&db, "event", "e", 9, false);
        let tx = db.transaction().expect("transaction");
        keep_counter(&tx).expect("keep the counter");
        canvas_tx(&tx)
            .partition::<crate::publication_schema::SyncEntities>()
            .delete(&(encode_key(&("event".to_owned(), "e".to_owned())),))
            .expect("delete the row with the max seq");
        tx.commit().expect("commit");
        let fresh = next(&db).expect("next");
        assert!(
            fresh > 9,
            "a new number must be above the old max 9, got {fresh}"
        );
    }

    #[test]
    fn an_unchanged_record_only_row_keeps_its_number() {
        let db = canvas();
        db.execute(
            "INSERT INTO runtime_rules(id,record) VALUES('r','{\"id\":\"r\"}')",
            [],
        )
        .expect("rule");
        let seq = || -> i64 { entry(&db, "runtime_rules", "r").unwrap().unwrap().sequence };
        let first = seq();
        db.execute("UPDATE runtime_rules SET record=record WHERE id='r'", [])
            .expect("no-op update");
        assert_eq!(seq(), first, "a no-op update takes no number");
        db.execute(
            "UPDATE runtime_rules SET record='{\"id\":\"r\",\"name\":\"n\"}' WHERE id='r'",
            [],
        )
        .expect("real update");
        assert!(seq() > first, "a real update takes a new number");
    }

    #[test]
    fn an_unknown_table_is_refused() {
        assert!(changes_since(&canvas(), "runtime_events", None).is_err());
    }

    #[test]
    fn install_is_idempotent_and_numbers_rows_that_predate_the_source() {
        let mut db = canvas();
        put(&db, "a", "{}");
        let before = seq_of(&db, "a");
        let tx = db.transaction().expect("transaction");
        install(&tx).expect("second install");
        tx.commit().expect("commit");
        assert_eq!(seq_of(&db, "a"), before);
        // Old format: rows without entries and without triggers.
        let tx = db.transaction().unwrap();
        db_delete!(
            canvas_tx(&tx),
            DurableRows,
            filter(
                DurableRows::SOURCE_LABEL.eq(&DurableSource::from_stored("runtime_work".into()))
            )
        )
        .unwrap();
        tx.commit().unwrap();
        // Simulate unnumbered legacy rows by removing the marks after insertion.
        put(&db, "old", "{}");
        put(&db, "older", "{}");
        let tx = db.transaction().unwrap();
        db_delete!(
            canvas_tx(&tx),
            DurableRows,
            filter(
                DurableRows::SOURCE_LABEL.eq(&DurableSource::from_stored("runtime_work".into()))
            )
        )
        .unwrap();
        tx.commit().unwrap();
        let tx = db.transaction().expect("transaction");
        install(&tx).expect("install over old format");
        tx.commit().expect("commit");
        let high = high(&db).expect("counter");
        let (a, old, older) = (seq_of(&db, "a"), seq_of(&db, "old"), seq_of(&db, "older"));
        assert!(a > 0 && old > a && older > old && older <= high);
    }

    #[test]
    fn the_transcript_copy_check_names_items_the_owner_lacks() {
        let db = canvas();
        put_item(&db, "i1", "a1");
        seed_entity(&db, "transcript:a1", "item:i1", 1, false);
        seed_entity(&db, "transcript:a1", "item:i2", 2, false);
        seed_entity(&db, "transcript:a1", "item:i3", 3, true);
        seed_entity(&db, "agent", "a1", 4, false);
        assert_eq!(
            transcript_copy_missing_from_owner(&db).expect("check"),
            vec![("a1".to_owned(), "i2".to_owned())]
        );
    }

    #[test]
    fn retiring_the_copies_needs_every_item_in_the_owner_and_never_deletes_rows() {
        let mut db = canvas();
        assert!(!transcript_copies_retired(&db).expect("flag"));
        seed_entity(&db, "transcript:a1", "item:i2", 2, false);
        let tx = db.transaction().expect("transaction");
        let missing = retire_transcript_copies(&tx).expect("check");
        tx.commit().expect("commit");
        assert_eq!(missing, vec![("a1".to_owned(), "i2".to_owned())]);
        assert!(
            !transcript_copies_retired(&db).expect("flag"),
            "the writer keeps the copies"
        );
        put_item(&db, "i2", "a1");
        let tx = db.transaction().expect("transaction");
        assert!(retire_transcript_copies(&tx).expect("check").is_empty());
        tx.commit().expect("commit");
        assert!(transcript_copies_retired(&db).expect("flag"));
        let copies = studio_db_api::db_query!(
            canvas_read(&db),
            crate::publication_schema::SyncEntities,
            filter(
                crate::publication_schema::SyncEntities::COLLECTION_ORDER.eq(
                    &crate::publication_collection::CollectionName::from_stored(
                        "transcript:a1".into()
                    )
                )
            )
        )
        .unwrap()
        .len();
        assert_eq!(copies, 1, "old rows stay");
    }

    #[test]
    fn the_change_cursor_follows_items_agent_and_deletes_of_one_conversation() {
        let db = canvas();
        db.execute_batch(
            "INSERT INTO runtime_agents(id,record) VALUES('a1','{\"provider\":\"codex\",\"id\":\"a1\",\"rootId\":\"a1\",\"epoch\":0,\"status\":\"idle\",\"accountKey\":\"default\"}');
             INSERT INTO runtime_items(id,agent,record,created) VALUES('i1','a1','{}',1);",
        )
        .expect("rows");
        let first = transcript_change_cursor(&db, "a1").expect("cursor");
        assert!(first > 0);
        db.execute(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES('o1','other','{}',1)",
            [],
        )
        .expect("other chat");
        assert_eq!(transcript_change_cursor(&db, "a1").expect("cursor"), first);
        db.execute(
            "UPDATE runtime_items SET record='{\"x\":1}' WHERE id='i1'",
            [],
        )
        .expect("update");
        let second = transcript_change_cursor(&db, "a1").expect("cursor");
        assert!(second > first);
        db.execute("DELETE FROM runtime_items WHERE id='i1'", [])
            .expect("delete");
        assert!(transcript_change_cursor(&db, "a1").expect("cursor") > second);
        let removed = transcript_change_cursor(&db, "a1").unwrap();
        let tx = db.unchecked_transaction().unwrap();
        db_delete!(
            canvas_tx(&tx),
            crate::item_monitor_terminal::ItemsRow,
            filter(crate::item_monitor_terminal::ItemsRow::ID.eq(&"o1".to_owned()))
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(
            transcript_change_cursor(&db, "a1").unwrap(),
            removed,
            "another conversation's delete does not advance this cursor"
        );
    }
}
