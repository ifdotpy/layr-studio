//! Scoped entity pages for every reader. Work documents (`work`, `workDetail`, and the
//! history collections) are built here from the main tables: `runtime_work`, its history
//! child tables, and the rows that change a `workDetail` (owner agent, execution run,
//! dependency). All other collections come from `sync_entities`. One merge by change
//! number keeps the paging rules of `SyncDocumentsRepository`: a page is cut at a whole
//! unit, and `checkpoint` is the last unit taken.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use studio_runtime_schema::shared_values::SYNC_ENTITY_STREAM_MAX_DOCUMENTS;
use studio_runtime_store::entity_publication::WorkHistoryKind;
use studio_runtime_store::{
    EntityPage, EntityRow, StoreError, StoreResult, SyncDocumentsRepository, WORK_COLLECTIONS,
    WorkItemRecord, WorkItemsRepository, WorkStatus, durable_seq, work_history,
};

use crate::work_write::{WorkWriteError, current_submission_refusal};

/// One history row of `work_history`: change id, seq, tombstone flag, record, parent
/// work id, index, and the parent's `rootId`.
type HistoryRow = (
    String,
    i64,
    bool,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
);

/// Rows that share one change number. A unit is never cut inside.
struct Unit {
    seq: i64,
    rows: Vec<EntityRow>,
}

/// Start a scoped read: resolve the reset decision over both sources, then read the
/// first page. `since` is the client cursor; None or an out-of-range cursor resets.
pub fn resume(
    transaction: &Transaction<'_>,
    since: Option<i64>,
    collections_json: Option<&str>,
    id: Option<&str>,
    agent: Option<&str>,
    include_team_members: bool,
    limit: i64,
) -> StoreResult<EntityPage> {
    let entity_floor = SyncDocumentsRepository::entity_tombstone_floor(transaction)?;
    let (high, durable_floor) = durable_seq::high_and_floor(transaction)?;
    let floor = entity_floor.max(durable_floor);
    let max_sequence = high.max(floor);
    let reset = since.is_none_or(|seq| seq < floor || seq > max_sequence || seq < 0);
    let after = if reset { 0 } else { since.unwrap_or(0) };
    let interval = EntityPage {
        rows: Vec::new(),
        max_sequence,
        tombstone_floor: floor,
        initial_high: if reset { max_sequence } else { 0 },
        checkpoint: after,
        reset,
    };
    continue_page(
        transaction,
        &interval,
        collections_json,
        id,
        agent,
        include_team_members,
        limit,
    )
}

/// Non-fresh REST pull of the global list. Same reset rule as
/// `SyncDocumentsRepository::pull_global_entities`: a cursor above zero that is
/// outside the range gets an empty reset page; zero reads from the beginning.
pub fn pull_global(
    transaction: &Transaction<'_>,
    after: i64,
    limit: i64,
    reset_support: bool,
) -> StoreResult<EntityPage> {
    let entity_floor = SyncDocumentsRepository::entity_tombstone_floor(transaction)?;
    let (high, durable_floor) = durable_seq::high_and_floor(transaction)?;
    let floor = entity_floor.max(durable_floor);
    let max_sequence = high.max(floor);
    let after = after.max(0);
    if reset_support && after > 0 && (after < floor || after > max_sequence) {
        return Ok(EntityPage {
            rows: Vec::new(),
            max_sequence,
            tombstone_floor: floor,
            initial_high: 0,
            checkpoint: max_sequence,
            reset: true,
        });
    }
    let interval = EntityPage {
        rows: Vec::new(),
        max_sequence,
        tombstone_floor: floor,
        initial_high: 0,
        checkpoint: after,
        reset: false,
    };
    continue_page(
        transaction,
        &interval,
        Some(studio_runtime_schema::sync_protocol::global_entity_collections_json()),
        None,
        None,
        true,
        limit.clamp(
            1,
            studio_runtime_schema::shared_values::SYNC_ENTITY_PAGE_LIMIT,
        ),
    )
}

/// Continue inside the caller's read transaction with the same scope. The snapshot
/// flag and the high mark come from `interval`, as in `SyncDocumentsRepository`.
pub fn continue_page(
    transaction: &Transaction<'_>,
    interval: &EntityPage,
    collections_json: Option<&str>,
    id: Option<&str>,
    agent: Option<&str>,
    include_team_members: bool,
    limit: i64,
) -> StoreResult<EntityPage> {
    let limit = limit.clamp(1, SYNC_ENTITY_STREAM_MAX_DOCUMENTS);
    let after = interval.checkpoint;
    let through = interval.max_sequence;
    let snapshot = interval.reset;
    let fetch = limit + 1;
    let mut units: Vec<Unit> = SyncDocumentsRepository::scope_rows(
        transaction,
        after,
        through,
        snapshot,
        collections_json,
        id,
        agent,
        include_team_members,
        fetch,
        true,
    )?
    .into_iter()
    .map(|row| Unit {
        seq: row.sequence,
        rows: vec![row],
    })
    .collect();
    let wanted = wanted(collections_json)?;
    // Work rows are not addressed by id; an id-scoped reader gets no work documents.
    if id.is_none() && !wanted.is_empty() {
        units.extend(work_units(
            transaction,
            after,
            through,
            snapshot,
            &wanted,
            agent,
            fetch,
        )?);
    }
    units.sort_by_key(|unit| unit.seq);
    let truncated = units.len() as i64 > limit;
    units.truncate(limit as usize);
    let checkpoint = if truncated {
        units.last().map_or(after, |unit| unit.seq)
    } else {
        through
    };
    let mut rows: Vec<EntityRow> = units.into_iter().flat_map(|unit| unit.rows).collect();
    // One document per key: the highest change number wins (a fan-out may repeat a row).
    let mut seen = HashSet::new();
    rows.reverse();
    rows.retain(|row| seen.insert((row.collection.clone(), row.id.clone())));
    rows.reverse();
    Ok(EntityPage {
        rows,
        max_sequence: through,
        tombstone_floor: interval.tombstone_floor,
        initial_high: if snapshot { through } else { 0 },
        checkpoint,
        reset: snapshot,
    })
}

/// Work collections the reader selected. None selects all of them.
fn wanted(collections_json: Option<&str>) -> StoreResult<BTreeSet<&'static str>> {
    let Some(raw) = collections_json else {
        return Ok(WORK_COLLECTIONS.iter().copied().collect());
    };
    let selected: Vec<String> = serde_json::from_str(raw)?;
    Ok(WORK_COLLECTIONS
        .iter()
        .copied()
        .filter(|collection| selected.iter().any(|name| name == collection))
        .collect())
}

/// Work units in `(after, through]`, at most `fetch` per source. Snapshots need no
/// fan-out: every live work row is already in the snapshot range.
fn work_units(
    transaction: &Connection,
    after: i64,
    through: i64,
    snapshot: bool,
    wanted: &BTreeSet<&'static str>,
    agent: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let mut units = Vec::new();
    // Work rows, live or deleted. A deleted row is the work tombstone.
    let changed: Vec<(String, i64, bool)> = transaction
        .prepare(
            "SELECT d.id,d.seq,d.deleted FROM durable_rows d LEFT JOIN runtime_work w ON w.id=d.id
             WHERE d.tbl='runtime_work' AND d.seq>?1 AND d.seq<=?2 AND (?3=0 OR d.deleted=0)
               AND (?4 IS NULL OR d.deleted=1 OR json_extract(w.record,'$.rootId')=?4)
             ORDER BY d.seq LIMIT ?5",
        )?
        .query_map(
            params![after, through, i64::from(snapshot), agent, fetch],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
        )?
        .collect::<Result<_, _>>()?;
    for (id, seq, deleted) in &changed {
        let rows = if *deleted {
            tombstones(&["work", "workDetail"], wanted, id, *seq)
        } else {
            work_docs(transaction, id, *seq, wanted, false)?
        };
        if !rows.is_empty() {
            units.push(Unit { seq: *seq, rows });
        }
    }
    // History rows: one entry per row, keyed `{work}:{index}`.
    for kind in WorkHistoryKind::ALL {
        let collection = kind.collection();
        if !wanted.contains(collection) {
            continue;
        }
        let table = work_history::table_of(kind);
        let entries: Vec<HistoryRow> =
            transaction
                .prepare(&format!(
                    "SELECT d.id,d.seq,d.deleted,c.record,c.work,c.idx,json_extract(w.record,'$.rootId')
                     FROM durable_rows d LEFT JOIN {table} c ON c.id=d.id
                     LEFT JOIN runtime_work w ON w.id=c.work
                     WHERE d.tbl='{table}' AND d.seq>?1 AND d.seq<=?2 AND (?3=0 OR d.deleted=0)
                       AND (?4 IS NULL OR d.deleted=1 OR json_extract(w.record,'$.rootId')=?4)
                     ORDER BY d.seq LIMIT ?5"
                ))?
                .query_map(
                    params![after, through, i64::from(snapshot), agent, fetch],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get::<_, i64>(2)? != 0,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                        ))
                    },
                )?
                .collect::<Result<_, _>>()?;
        for (id, seq, deleted, record, work, index, root) in entries {
            let rows = match (deleted, record, work, index, root) {
                (true, ..) => tombstones(&[collection], wanted, &id, seq),
                (false, Some(raw), Some(work), Some(index), Some(root)) => {
                    let value: Value = serde_json::from_str(&raw)?;
                    let entry = crate::sync_views::work_history_entry(
                        &work,
                        &json!(root),
                        index as usize,
                        &value,
                    );
                    vec![upsert(collection, &id, seq, &entry)?]
                }
                _ => Vec::new(),
            };
            if !rows.is_empty() {
                units.push(Unit { seq, rows });
            }
        }
    }
    if snapshot || !wanted.contains("workDetail") {
        return Ok(units);
    }
    // Fan-out: a changed owner agent, execution run or dependency refreshes the
    // `workDetail` of the works it affects. Each trigger keeps its own change number.
    let owners: Vec<(String, i64)> = transaction
        .prepare(
            "SELECT d.id,d.seq FROM durable_rows d WHERE d.tbl='runtime_agents'
               AND d.seq>?1 AND d.seq<=?2 ORDER BY d.seq LIMIT ?3",
        )?
        .query_map(params![after, through, fetch], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    let runs: Vec<(String, i64)> = transaction
        .prepare(
            "SELECT r.agent,d.seq FROM durable_rows d JOIN runtime_execution_runs r ON r.id=d.id
               WHERE d.tbl='runtime_execution_runs' AND d.deleted=0
                 AND d.seq>?1 AND d.seq<=?2 ORDER BY d.seq LIMIT ?3",
        )?
        .query_map(params![after, through, fetch], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    for (owner, seq) in owners.iter().chain(runs.iter()) {
        let works = transaction
            .prepare(
                "SELECT id FROM runtime_work WHERE json_extract(record,'$.owner')=?1
                   AND (?2 IS NULL OR json_extract(record,'$.rootId')=?2)",
            )?
            .query_map(params![owner, agent], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = Vec::new();
        for work in works {
            rows.extend(work_docs(transaction, &work, *seq, wanted, true)?);
        }
        if !rows.is_empty() {
            units.push(Unit { seq: *seq, rows });
        }
    }
    let changed_works: Vec<(String, i64)> = transaction
        .prepare(
            "SELECT id,seq FROM durable_rows WHERE tbl='runtime_work'
               AND seq>?1 AND seq<=?2 ORDER BY seq LIMIT ?3",
        )?
        .query_map(params![after, through, fetch], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    for (changed, seq) in changed_works {
        // A deleted dependency is still named by its dependents.
        let dependents = transaction
            .prepare(
                "SELECT id FROM runtime_work w WHERE EXISTS (SELECT 1 FROM json_each(w.record,'$.dependencies') j WHERE j.value=?1)
                   AND (?2 IS NULL OR json_extract(w.record,'$.rootId')=?2)",
            )?
            .query_map(params![changed, agent], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = Vec::new();
        for dependent in dependents {
            rows.extend(work_docs(transaction, &dependent, seq, wanted, true)?);
        }
        if !rows.is_empty() {
            units.push(Unit { seq, rows });
        }
    }
    Ok(units)
}

/// Documents of one live work row at `seq`: `work` and `workDetail`, or only
/// `workDetail` when `detail_only` (fan-out). Empty when the row is gone.
fn work_docs(
    transaction: &Connection,
    id: &str,
    seq: i64,
    wanted: &BTreeSet<&'static str>,
    detail_only: bool,
) -> StoreResult<Vec<EntityRow>> {
    let raw: Option<String> = transaction
        .query_row("SELECT record FROM runtime_work WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    // A row that does not decode has no document: the stream skips it and reports it here.
    let item = match serde_json::from_str::<WorkItemRecord>(&raw) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("work row {id} does not decode, no work document is streamed: {error}");
            return Ok(Vec::new());
        }
    };
    let record: Value = serde_json::from_str(&raw)?;
    let mut rows = Vec::new();
    if !detail_only
        && wanted.contains("work")
        && let Some(summary) = crate::sync_views::project_work_record(&item)
    {
        rows.push(upsert("work", id, seq, summary)?);
    }
    if wanted.contains("workDetail") {
        let refusal = current_submission_refusal(transaction, &record).map_err(work_error)?;
        let mut statuses: BTreeMap<String, WorkStatus> = BTreeMap::new();
        for dependency in item.dependencies() {
            match WorkItemsRepository::work_record(transaction, dependency) {
                Ok(Some(dependency_record)) => {
                    statuses.insert(dependency.clone(), dependency_record.status());
                }
                Ok(None) => {}
                Err(error) => eprintln!(
                    "work row {dependency} does not decode, its status is left out of the dependent document: {error}"
                ),
            }
        }
        let detail = crate::sync_views::work_detail_view(&item, &record, refusal, &statuses)?;
        rows.push(upsert("workDetail", id, seq, detail)?);
    }
    Ok(rows)
}

/// Deleted rows of the selected collections, all at `seq`.
fn tombstones(
    collections: &[&str],
    wanted: &BTreeSet<&'static str>,
    id: &str,
    seq: i64,
) -> Vec<EntityRow> {
    collections
        .iter()
        .filter(|collection| wanted.contains(*collection))
        .map(|collection| EntityRow {
            collection: (*collection).to_owned(),
            id: id.to_owned(),
            sequence: seq,
            payload: Some(json!({"collection":collection,"id":id,"value":{}}).to_string()),
            deleted: true,
        })
        .collect()
}

/// A live row in the envelope shape that `sync_entities` rows use. The typed value is
/// serialized straight into the envelope, as the publisher does.
fn upsert<T: serde::Serialize>(
    collection: &str,
    id: &str,
    seq: i64,
    value: T,
) -> StoreResult<EntityRow> {
    #[derive(serde::Serialize)]
    struct Envelope<'a, T> {
        collection: &'a str,
        id: &'a str,
        value: T,
    }
    Ok(EntityRow {
        collection: collection.to_owned(),
        id: id.to_owned(),
        sequence: seq,
        payload: Some(serde_json::to_string(&Envelope {
            collection,
            id,
            value,
        })?),
        deleted: false,
    })
}

/// Same mapping as the publisher: store errors pass through, refusals become schema errors.
fn work_error(error: WorkWriteError) -> StoreError {
    match error {
        WorkWriteError::Store(error) => error,
        WorkWriteError::Expired => {
            studio_runtime_store::request_results::RequestResultError::Expired.into()
        }
        other @ (WorkWriteError::Event(_)
        | WorkWriteError::NotImplemented(_)
        | WorkWriteError::PreparationWorker) => StoreError::SchemaValidation(other.to_string()),
    }
}
