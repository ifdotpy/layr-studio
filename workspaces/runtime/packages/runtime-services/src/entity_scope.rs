//! Entity pages for every reader: the stream, scoped subscriptions, the REST pull and
//! mutation replies. Two groups of collections are built here from the main tables at
//! read time and have no copy in `sync_entities`:
//!
//! - [`WORK_COLLECTIONS`] (`work`, `workDetail`, and the history collections), from
//!   `runtime_work`, its history child tables, and the rows that change a `workDetail`
//!   (owner agent, execution run, dependency).
//! - [`DIRECT_COLLECTIONS`], one document per row of a main table: `plan`
//!   (`runtime_plans`), `annotation` (`runtime_annotations`), `rule` (`runtime_rules`),
//!   and `request` and `questionHistory` (`approvals`).
//!
//! - [`AGENT_VIEW_COLLECTIONS`] (`agent`, `agentSummary`, `agentDetail`,
//!   `retainedWorktree`), from `runtime_agents`. A change of the agent row, or a mark
//!   (`durable_seq::AGENT_VIEW_MARK`) that the publisher writes when a row the view
//!   reads changes, gives the views that change's number.
//!
//! The other collections still come from `sync_entities`. One merge by change number keeps
//! the paging rules of `SyncDocumentsRepository`: a page is cut at a whole unit, and
//! `checkpoint` is the last unit taken.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{Connection, OptionalExtension, params};
use studio_json::{Value, json};
use studio_runtime_schema::shared_values::SYNC_ENTITY_STREAM_MAX_DOCUMENTS;
use studio_runtime_store::entity_publication::WorkHistoryKind;
use studio_runtime_store::{
    AGENT_VIEW_COLLECTIONS, DIRECT_COLLECTIONS, EntityPage, EntityRow, StoreError, StoreResult,
    SyncDocumentsRepository, WINDOW_COLLECTIONS, WORK_COLLECTIONS, WorkItemRecord,
    WorkItemsRepository, WorkStatus, durable_seq, work_history,
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

/// Advance by examined candidates, including those with no visible documents.
fn page_units(units: &mut Vec<Unit>, limit: usize, after: i64, through: i64) -> i64 {
    units.sort_by_key(|unit| unit.seq);
    let truncated = units.len() > limit;
    let mut count = limit.min(units.len());
    while count > 0 && count < units.len() && units[count].seq == units[count - 1].seq {
        count += 1;
    }
    units.truncate(count);
    if truncated {
        units.last().map_or(after, |unit| unit.seq)
    } else {
        through
    }
}

/// Start a scoped read: resolve the reset decision over both sources, then read the
/// first page. `since` is the client cursor; None or an out-of-range cursor resets.
pub fn resume(
    transaction: &Connection,
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

/// Every document of the selected collections that a snapshot reader sees now, from
/// both sources.
pub fn live_documents(
    connection: &Connection,
    collections_json: &str,
) -> StoreResult<Vec<EntityRow>> {
    let mut page = resume(
        connection,
        None,
        Some(collections_json),
        None,
        None,
        true,
        SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
    )?;
    let mut rows = Vec::new();
    loop {
        rows.append(&mut page.rows);
        if page.checkpoint >= page.max_sequence {
            return Ok(rows);
        }
        page = continue_page(
            connection,
            &page,
            Some(collections_json),
            None,
            None,
            true,
            SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
        )?;
    }
}

/// Non-fresh REST pull of the global list. Same reset rule as
/// `SyncDocumentsRepository::pull_entities`: a cursor above zero that is
/// outside the range gets an empty reset page; zero reads from the beginning.
pub fn pull_global(
    transaction: &Connection,
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

/// Fresh REST pull of the global list: every live row, and the deletes above
/// `initial_high` (zero takes the current high). On the first page the summary of
/// `priority_id` comes first. Same rules as `SyncDocumentsRepository::pull_entities`.
pub fn pull_global_fresh(
    transaction: &Connection,
    after: i64,
    limit: i64,
    initial_high: i64,
    priority_id: Option<&str>,
) -> StoreResult<EntityPage> {
    let entity_floor = SyncDocumentsRepository::entity_tombstone_floor(transaction)?;
    let (high, durable_floor) = durable_seq::high_and_floor(transaction)?;
    let floor = entity_floor.max(durable_floor);
    let max_sequence = high.max(floor);
    let initial_high = if initial_high > 0 {
        max_sequence.min(initial_high)
    } else {
        max_sequence
    };
    let after = after.max(0);
    let limit = limit.clamp(
        1,
        studio_runtime_schema::shared_values::SYNC_ENTITY_PAGE_LIMIT,
    );
    let collections = studio_runtime_schema::sync_protocol::global_entity_collections_json();
    let priority_id = if after == 0 && limit > 1 {
        priority_id
    } else {
        None
    };
    let rows = SyncDocumentsRepository::global_rows(
        transaction,
        after,
        floor,
        initial_high,
        collections,
        limit + 1,
    )?;
    // The summary of the open chat comes first, so the list can show it at once.
    let head = match priority_id {
        Some(priority) => agent_docs(
            transaction,
            priority,
            None,
            &BTreeSet::from(["agentSummary"]),
            true,
        )?,
        None => Vec::new(),
    };
    let mut units: Vec<Unit> = rows
        .into_iter()
        .map(|row| Unit {
            seq: row.sequence,
            rows: vec![row],
        })
        .collect();
    let wanted = wanted(Some(collections))?;
    if !wanted.is_empty() {
        units.extend(work_units(
            transaction,
            after,
            max_sequence,
            initial_high,
            &wanted,
            None,
            limit + 1,
        )?);
        units.extend(direct_units(
            transaction,
            after,
            max_sequence,
            initial_high,
            &wanted,
            None,
            None,
            limit + 1,
        )?);
        units.extend(agent_units(
            transaction,
            after,
            max_sequence,
            initial_high,
            &wanted,
            None,
            None,
            limit + 1,
        )?);
    }
    let room = limit as usize - head.len();
    let checkpoint = page_units(&mut units, room, after, max_sequence);
    let mut rows = head;
    rows.extend(units.into_iter().flat_map(|unit| unit.rows));
    Ok(EntityPage {
        rows: dedupe(rows),
        max_sequence,
        tombstone_floor: floor,
        initial_high,
        checkpoint,
        reset: false,
    })
}

/// Every entity change after `after`, for a mutation reply. `complete` is false when the
/// page was cut or a delete may be missing, so the reply must not claim a watermark.
pub fn delta_after(
    transaction: &Connection,
    after: i64,
    limit: i64,
) -> StoreResult<(Vec<EntityRow>, bool)> {
    let entity_floor = SyncDocumentsRepository::entity_tombstone_floor(transaction)?;
    let (high, durable_floor) = durable_seq::high_and_floor(transaction)?;
    let floor = entity_floor.max(durable_floor);
    let interval = EntityPage {
        rows: Vec::new(),
        max_sequence: high.max(floor),
        tombstone_floor: floor,
        initial_high: 0,
        checkpoint: after,
        reset: false,
    };
    let page = continue_page(transaction, &interval, None, None, None, true, limit)?;
    let complete = page.checkpoint == page.max_sequence && floor <= after;
    Ok((page.rows, complete))
}

/// One document per key: the highest change number wins (a fan-out may repeat a row).
fn dedupe(mut rows: Vec<EntityRow>) -> Vec<EntityRow> {
    let mut seen = HashSet::new();
    rows.reverse();
    rows.retain(|row| seen.insert((row.collection.clone(), row.id.clone())));
    rows.reverse();
    rows
}

/// Continue inside the caller's read transaction with the same scope. The snapshot
/// flag and the high mark come from `interval`, as in `SyncDocumentsRepository`.
pub fn continue_page(
    transaction: &Connection,
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
    )?
    .into_iter()
    .map(|row| Unit {
        seq: row.sequence,
        rows: vec![row],
    })
    .collect();
    let wanted = wanted(collections_json)?;
    if !wanted.is_empty() {
        let deletes_after = if snapshot { i64::MAX } else { after };
        // Work rows are not addressed by id; an id-scoped reader gets no work documents.
        if id.is_none() {
            units.extend(work_units(
                transaction,
                after,
                through,
                deletes_after,
                &wanted,
                agent,
                fetch,
            )?);
        }
        units.extend(direct_units(
            transaction,
            after,
            through,
            deletes_after,
            &wanted,
            id,
            agent,
            fetch,
        )?);
        units.extend(agent_units(
            transaction,
            after,
            through,
            deletes_after,
            &wanted,
            id,
            agent,
            fetch,
        )?);
        units.extend(window_units(
            transaction,
            after,
            through,
            deletes_after,
            &wanted,
            id,
            agent.map(|agent| (agent, include_team_members)),
            fetch,
        )?);
    }
    let checkpoint = page_units(&mut units, limit as usize, after, through);
    let rows = dedupe(units.into_iter().flat_map(|unit| unit.rows).collect());
    Ok(EntityPage {
        rows,
        max_sequence: through,
        tombstone_floor: interval.tombstone_floor,
        initial_high: if snapshot { through } else { 0 },
        checkpoint,
        reset: snapshot,
    })
}

/// Read-time collections the reader selected. None selects all of them.
fn wanted(collections_json: Option<&str>) -> StoreResult<BTreeSet<&'static str>> {
    let all = WORK_COLLECTIONS
        .iter()
        .chain(DIRECT_COLLECTIONS)
        .chain(AGENT_VIEW_COLLECTIONS)
        .chain(WINDOW_COLLECTIONS)
        .copied();
    let Some(raw) = collections_json else {
        return Ok(all.collect());
    };
    let selected: Vec<String> = studio_json::from_str(raw)?;
    Ok(all
        .filter(|collection| selected.iter().any(|name| name == collection))
        .collect())
}

/// A main table with one document per row in each of `collections`.
struct DirectSource {
    /// The `durable_rows` table: a main table, or a mark name for a view of several.
    table: &'static str,
    collections: &'static [&'static str],
}

/// Owner of the table of [`DIRECT_COLLECTIONS`]; each table is in `DURABLE_TABLES`.
const DIRECT_SOURCES: [DirectSource; 8] = [
    DirectSource {
        table: "runtime_tasks",
        collections: &["taskDetail"],
    },
    DirectSource {
        table: "runtime_plans",
        collections: &["plan"],
    },
    DirectSource {
        table: "runtime_annotations",
        collections: &["annotation"],
    },
    DirectSource {
        table: "runtime_rules",
        collections: &["rule"],
    },
    DirectSource {
        table: "approvals",
        collections: &["request", "questionHistory"],
    },
    DirectSource {
        table: "runtime_complaints",
        collections: &["complaint"],
    },
    DirectSource {
        table: "graph_edges",
        collections: &["edge"],
    },
    DirectSource {
        table: durable_seq::CHAT_VIEW_MARK,
        collections: &["chat"],
    },
];

/// Units of the direct sources in `(after, through]`, at most `fetch` per source, with
/// the delete rule of [`work_units`]. `id` selects one row. These collections belong
/// only to the global list, so an agent-scoped reader gets none of them.
#[allow(clippy::too_many_arguments)]
fn direct_units(
    connection: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    wanted: &BTreeSet<&'static str>,
    id: Option<&str>,
    agent: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let mut units = Vec::new();
    if agent.is_some() {
        return Ok(units);
    }
    for source in &DIRECT_SOURCES {
        // Full task inputs are available only to an explicit id-scoped observer.
        if source.table == "runtime_tasks" && id.is_none() {
            continue;
        }
        if !source.collections.iter().any(|c| wanted.contains(c)) {
            continue;
        }
        let query = if source.table == "runtime_tasks" {
            // One explicit task also depends on whether its owner is visible.
            // Read the owner's durable sequence in the same interval; no copied view or mark.
            "SELECT id,MAX(seq),MAX(deleted) FROM (
               SELECT id,seq,deleted FROM durable_rows
               WHERE tbl=?1 AND seq>?2 AND seq<=?3 AND (deleted=0 OR seq>?4) AND id=?5
               UNION ALL
               SELECT task.id,owner.seq,0 FROM runtime_tasks task
               JOIN durable_rows owner ON owner.tbl='runtime_agents'
                 AND owner.id=json_extract(task.record,'$.agent')
               WHERE task.id=?5 AND owner.seq>?2 AND owner.seq<=?3
             ) GROUP BY id ORDER BY MAX(seq) LIMIT ?6"
        } else {
            "SELECT id,seq,deleted FROM durable_rows
             WHERE tbl=?1 AND seq>?2 AND seq<=?3 AND (deleted=0 OR seq>?4)
               AND (?5 IS NULL OR id=?5)
             ORDER BY seq LIMIT ?6"
        };
        let changed: Vec<(String, i64, bool)> = connection
            .prepare_cached(query)?
            .query_map(
                params![source.table, after, through, deletes_after, id, fetch],
                |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
            )?
            .collect::<Result<_, _>>()?;
        let snapshot = deletes_after == i64::MAX;
        for (row_id, seq, deleted) in changed {
            let rows = if deleted {
                tombstones(source.collections, wanted, &row_id, seq)
            } else {
                direct_docs(connection, source.table, &row_id, seq, wanted, snapshot)?
            };
            units.push(Unit { seq, rows });
        }
    }
    Ok(units)
}

/// The documents of one live row of a direct source at `seq`. A snapshot leaves out a
/// document that the row's state hides; a delta sends it as a delete.
fn direct_docs(
    connection: &Connection,
    table: &str,
    id: &str,
    seq: i64,
    wanted: &BTreeSet<&'static str>,
    snapshot: bool,
) -> StoreResult<Vec<EntityRow>> {
    let mut rows = Vec::new();
    if table == "runtime_tasks" {
        if !wanted.contains("taskDetail") {
            return Ok(rows);
        }
        let task = studio_runtime_store::WorkspaceViewsRepository::task(connection, id)?;
        let visible = if let Some(value) = &task {
            let owner = value["agent"]
                .as_str()
                .ok_or(StoreError::RecordContract("task owner"))?;
            studio_runtime_store::WorkspaceViewsRepository::agent(connection, owner)?
                .is_some_and(|agent| agent.deleted_at.is_none())
        } else {
            false
        };
        if visible {
            rows.push(upsert("taskDetail", id, seq, task.expect("visible task"))?);
        } else if !snapshot {
            rows.extend(tombstones(&["taskDetail"], wanted, id, seq));
        }
        return Ok(rows);
    }
    if table == "approvals" {
        let Some(row) = studio_runtime_store::approvals::get(
            connection,
            &studio_runtime_schema::approvals::ApprovalId(id.to_owned()),
        )?
        else {
            return Ok(rows);
        };
        let view = crate::approval_sync::view(&row)?;
        for (collection, shown) in [("request", view.open), ("questionHistory", view.asked)] {
            if !wanted.contains(collection) {
                continue;
            }
            if shown {
                rows.push(upsert(collection, id, seq, &view.value)?);
            } else if !snapshot {
                rows.extend(tombstones(&[collection], wanted, id, seq));
            }
        }
        return Ok(rows);
    }
    if table == durable_seq::CHAT_VIEW_MARK || table == "graph_edges" {
        let (collection, view) = if table == "graph_edges" {
            ("edge", edge_view(connection, id)?)
        } else {
            (
                "chat",
                studio_runtime_store::CoordinationRepository::chat_view(connection, id)?,
            )
        };
        match view {
            Some(value) if wanted.contains(collection) => {
                rows.push(upsert(collection, id, seq, value)?)
            }
            Some(_) => {}
            None if !snapshot => rows.extend(tombstones(&[collection], wanted, id, seq)),
            None => {}
        }
        return Ok(rows);
    }
    let raw: Option<String> = connection
        .prepare_cached(&format!("SELECT record FROM {table} WHERE id=?1"))?
        .query_row([id], |row| row.get(0))
        .optional()?;
    let Some(raw) = raw else {
        return Ok(rows);
    };
    let (collection, view) = match table {
        "runtime_plans" => ("plan", crate::sync_policy::plan_view(id, &raw)),
        "runtime_annotations" => ("annotation", crate::sync_policy::annotation_view(id, &raw)),
        "runtime_rules" => ("rule", crate::sync_policy::rule_view(id, &raw).map(Some)),
        "runtime_complaints" => (
            "complaint",
            crate::sync_policy::complaint_view(connection, id, &raw),
        ),
        _ => return Err(StoreError::RecordContract("unknown direct source")),
    };
    // A row that does not decode has no document: the stream skips it and reports it here.
    match view {
        Ok(Some(value)) if wanted.contains(collection) => {
            rows.push(upsert(collection, id, seq, value)?);
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!(
                "{table} row {id} does not decode, no {collection} document is streamed: {error}"
            );
        }
    }
    Ok(rows)
}

/// Units of the agent views in `(after, through]`: one per agent row change or view
/// mark, at most `fetch`, with the delete rule of [`work_units`]. `id` selects one
/// agent. Only retained cleanup records also permit a root-scoped team read;
/// deleted members retain their root identity until cleanup finishes.
#[allow(clippy::too_many_arguments)]
fn agent_units(
    connection: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    wanted: &BTreeSet<&'static str>,
    id: Option<&str>,
    agent: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let mut units = if agent.is_none() && wanted.contains("agentTail") {
        tail_units(connection, after, through, deletes_after, id, fetch)?
    } else {
        Vec::new()
    };
    if wanted.iter().all(|kind| *kind == "agentTail") {
        return Ok(units);
    }
    if (agent.is_some() && wanted.iter().any(|kind| *kind != "retainedWorktree"))
        || !AGENT_VIEW_COLLECTIONS.iter().any(|c| wanted.contains(c))
    {
        return Ok(units);
    }
    // Cleanup uses soft deletion, retaining the root until its tombstone is emitted.
    // Arbitrary hard deletes have no scoped provenance; only snapshots reconcile them.
    // The agent row and its view mark both name the agent: one unit at the later one.
    let changed: Vec<(String, i64)> = prepare_agent_changes(connection, after, through)?
        .query_map(
            params![
                durable_seq::AGENT_VIEW_MARK,
                after,
                through,
                deletes_after,
                id,
                fetch,
                agent
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<_, _>>()?;
    let contexts = if wanted.contains("agentSummary") {
        crate::agent_operational::load(
            connection,
            &changed.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        )?
    } else {
        BTreeMap::new()
    };
    let snapshot = deletes_after == i64::MAX;
    for (agent_id, seq) in changed {
        // A gone agent reads as no views: deletes in a delta, nothing in a snapshot.
        let rows = agent_docs_with_context(
            connection,
            &agent_id,
            Some(seq),
            wanted,
            snapshot,
            contexts.get(&agent_id),
        )
        .or_else(|error| {
            // One agent whose view cannot be built must not stop the stream.
            eprintln!("agent {agent_id} views are not streamed: {error}");
            Ok::<_, StoreError>(Vec::new())
        })?;
        // Filtered candidates occupy the SQL page and must advance its cursor.
        units.push(Unit { seq, rows });
    }
    Ok(units)
}

// Owner: entity_scope. Maximum narrow range in durable_rows sequence numbers;
// larger ranges retain the planner's table-key scan for broad snapshots.
const SEQ_INDEX_RANGE_MAX: i64 = 1024;

const AGENT_CHANGES_SQL: &str = "SELECT id,MAX(seq) AS last FROM durable_rows
             WHERE tbl IN ('runtime_agents',?1) AND seq>?2 AND seq<=?3
               AND (deleted=0 OR seq>?4) AND (?5 IS NULL OR id=?5)
               AND (?7 IS NULL OR id=?7 OR id IN
                 (SELECT id FROM runtime_agents WHERE root_id=?7))
             GROUP BY id ORDER BY last LIMIT ?6";

/// Prepare the grouped agent change read used by every entity page.
fn prepare_agent_changes(
    connection: &Connection,
    after: i64,
    through: i64,
) -> StoreResult<rusqlite::CachedStatement<'_>> {
    if through.saturating_sub(after) <= SEQ_INDEX_RANGE_MAX {
        return Ok(connection.prepare_cached(&AGENT_CHANGES_SQL.replacen(
            "FROM durable_rows",
            "FROM durable_rows INDEXED BY durable_rows_seq",
            1,
        ))?);
    }
    Ok(connection.prepare_cached(AGENT_CHANGES_SQL)?)
}

/// Only lead tails. Four scalar columns from owner tables; no summary/domain aggregation.
fn tail_units(
    connection: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    id: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let changed=connection.prepare_cached("SELECT id,MAX(seq) FROM durable_rows WHERE tbl IN ('runtime_agents','runtime_agent_volatile') AND seq>?1 AND seq<=?2 AND (deleted=0 OR seq>?3) AND (?4 IS NULL OR id=?4) GROUP BY id ORDER BY MAX(seq) LIMIT ?5")?.query_map(params![after,through,deletes_after,id,fetch],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
    let snapshot = deletes_after == i64::MAX;
    let mut units = Vec::new();
    for (id, seq) in changed {
        let source:Option<(bool,bool,Option<String>)>=connection.query_row("SELECT COALESCE(json_extract(a.record,'$.isLead'),0),a.deleted_at IS NOT NULL,json_extract(v.tail,'$') FROM runtime_agents a LEFT JOIN runtime_agent_volatile v ON v.id=a.id WHERE a.id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let rows = match source {
            Some((true, false, tail)) => {
                let tail = tail
                    .unwrap_or_default()
                    .chars()
                    .take(studio_runtime_schema::shared_values::AGENT_TAIL_MAX_CHARS)
                    .collect::<String>();
                vec![upsert(
                    "agentTail",
                    &id,
                    seq,
                    studio_runtime_schema::agent_tail::AgentTail {
                        id: id.clone(),
                        tail: tail
                            .try_into()
                            .map_err(|_| StoreError::RecordContract("bounded tail"))?,
                    },
                )?]
            }
            Some((false, _, _)) => Vec::new(),
            _ if !snapshot => tombstones(&["agentTail"], &BTreeSet::from(["agentTail"]), &id, seq),
            _ => Vec::new(),
        };
        // Filtered candidates occupy the SQL page and must advance its cursor.
        units.push(Unit { seq, rows });
    }
    Ok(units)
}

/// The view documents of one agent at `seq` (its current number when None). A
/// snapshot leaves out the views that are deleted; a delta sends them as deletes.
fn agent_docs(
    connection: &Connection,
    id: &str,
    seq: Option<i64>,
    wanted: &BTreeSet<&'static str>,
    snapshot: bool,
) -> StoreResult<Vec<EntityRow>> {
    if wanted.iter().all(|kind| *kind == "agentTail") {
        let high = durable_seq::high(connection)?;
        return Ok(tail_units(connection, 0, high, i64::MAX, Some(id), 1)?
            .into_iter()
            .flat_map(|unit| unit.rows)
            .collect());
    }
    let contexts = if wanted.contains("agentSummary") {
        crate::agent_operational::load(connection, &[id.to_owned()])?
    } else {
        BTreeMap::new()
    };
    agent_docs_with_context(connection, id, seq, wanted, snapshot, contexts.get(id))
}
fn agent_docs_with_context(
    connection: &Connection,
    id: &str,
    seq: Option<i64>,
    wanted: &BTreeSet<&'static str>,
    snapshot: bool,
    context: Option<&crate::agent_operational::Context>,
) -> StoreResult<Vec<EntityRow>> {
    let seq = match seq {
        Some(seq) => seq,
        None => {
            let current: Option<i64> = connection
                .prepare_cached(
                    "SELECT MAX(seq) FROM durable_rows WHERE tbl IN ('runtime_agents',?1) AND id=?2",
                )?
                .query_row(params![durable_seq::AGENT_VIEW_MARK, id], |row| row.get(0))?;
            let Some(current) = current else {
                return Ok(Vec::new());
            };
            current
        }
    };
    let server = server_id(connection)?;
    let Some(documents) = crate::sync_policy::agent_view_documents(
        connection,
        id,
        server.as_deref(),
        wanted.contains("agentSummary"),
        context,
    )?
    else {
        return Ok(if snapshot {
            Vec::new()
        } else {
            tombstones(AGENT_VIEW_COLLECTIONS, wanted, id, seq)
        });
    };
    let mut rows = Vec::new();
    for (collection, value) in [
        ("agentSummary", documents.summary),
        ("agentDetail", documents.detail),
        ("agent", documents.agent),
        ("retainedWorktree", documents.retained_worktree),
    ] {
        if !wanted.contains(collection) {
            continue;
        }
        match value {
            Some(value) => rows.push(upsert(collection, id, seq, value)?),
            None if !snapshot => rows.extend(tombstones(&[collection], wanted, id, seq)),
            None => {}
        }
    }
    Ok(rows)
}

/// The view documents of every live agent, as a fresh pull sends them.
pub fn agent_snapshot(connection: &Connection) -> StoreResult<Vec<EntityRow>> {
    let wanted = AGENT_VIEW_COLLECTIONS.iter().copied().collect();
    let high = durable_seq::high(connection)?;
    Ok(
        agent_units(connection, 0, high, i64::MAX, &wanted, None, None, i64::MAX)?
            .into_iter()
            .flat_map(|unit| unit.rows)
            .collect(),
    )
}

/// Units of the window collections in `(after, through]`, at most `fetch` per window:
/// one per member change or change of a member's row, at the later number. A member
/// that left the window is a delete in a delta. `agent` keeps the documents of one
/// owner (and of its team members when the flag is set); a delete always passes.
#[allow(clippy::too_many_arguments)]
fn window_units(
    connection: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    wanted: &BTreeSet<&'static str>,
    id: Option<&str>,
    agent: Option<(&str, bool)>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let mut units = Vec::new();
    let snapshot = deletes_after == i64::MAX;
    for (collection, window, source) in [
        ("task", durable_seq::TASK_WINDOW, "runtime_tasks"),
        ("monitor", durable_seq::MONITOR_WINDOW, "runtime_monitors"),
    ] {
        if !wanted.contains(collection) {
            continue;
        }
        let changed: Vec<(String, i64, bool)> = connection
            .prepare_cached(
                "SELECT x.id,MAX(x.seq) AS last,w.deleted FROM (
                   SELECT id,seq FROM durable_rows WHERE tbl=?1 AND seq>?3 AND seq<=?4
                     AND (deleted=0 OR seq>?5)
                   UNION ALL
                   SELECT d.id,d.seq FROM durable_rows d WHERE d.tbl=?2 AND d.deleted=0
                     AND d.seq>?3 AND d.seq<=?4
                     AND EXISTS(SELECT 1 FROM durable_rows w WHERE w.tbl=?1 AND w.id=d.id AND w.deleted=0)
                 ) x JOIN durable_rows w ON w.tbl=?1 AND w.id=x.id
                 WHERE ?6 IS NULL OR x.id=?6
                 GROUP BY x.id ORDER BY last LIMIT ?7",
            )?
            .query_map(
                params![window, source, after, through, deletes_after, id, fetch],
                |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
            )?
            .collect::<Result<_, _>>()?;
        for (member, seq, left) in changed {
            let view = if left {
                None
            } else {
                crate::sync_policy::window_view(connection, collection, &member).or_else(
                    |error| {
                        // One member that does not decode must not stop the stream.
                        eprintln!("{collection} {member} is not streamed: {error}");
                        Ok::<_, StoreError>(None)
                    },
                )?
            };
            let rows = match view {
                Some((owner, value)) => {
                    if let Some((agent, team)) = agent
                        && owner != agent
                        && !(team && team_member(connection, &owner, agent)?)
                    {
                        Vec::new()
                    } else {
                        vec![upsert(collection, &member, seq, value)?]
                    }
                }
                None if snapshot => Vec::new(),
                None => tombstones(&[collection], wanted, &member, seq),
            };
            units.push(Unit { seq, rows });
        }
    }
    Ok(units)
}

/// Whether `agent` is a live member of the team of `root`.
fn team_member(connection: &Connection, agent: &str, root: &str) -> StoreResult<bool> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM runtime_agents WHERE id=?1 AND root_id=?2 AND deleted_at IS NULL)",
        )?
        .query_row([agent, root], |row| row.get(0))?)
}

/// The `edge` document of a chat membership edge; None for another kind.
fn edge_view(connection: &Connection, id: &str) -> StoreResult<Option<Value>> {
    Ok(connection
        .prepare_cached("SELECT source,target FROM graph_edges WHERE id=?1 AND kind='chat'")?
        .query_row([id], |row| {
            Ok(json!({"id":id,"source":row.get::<_, String>(0)?,"target":row.get::<_, String>(1)?,"kind":"chat"}))
        })
        .optional()?)
}

/// This server's id, for the views that name a project's home server.
fn server_id(connection: &Connection) -> StoreResult<Option<String>> {
    studio_runtime_store::RuntimeStore::transaction_local_server_id(connection)
}

/// The current document of a direct or agent view collection, as a reader at the
/// current high mark sees it; None when the row is gone or hidden.
pub fn direct_document(
    connection: &Connection,
    collection: &str,
    id: &str,
) -> StoreResult<Option<EntityRow>> {
    if let Some(name) = AGENT_VIEW_COLLECTIONS
        .iter()
        .find(|name| **name == collection)
    {
        let wanted = BTreeSet::from([*name]);
        return Ok(agent_docs(connection, id, None, &wanted, true)?
            .into_iter()
            .next());
    }
    let source = DIRECT_SOURCES
        .iter()
        .find(|source| source.collections.contains(&collection))
        .ok_or(StoreError::RecordContract("not a direct collection"))?;
    let seq: Option<i64> = connection
        .prepare_cached("SELECT seq FROM durable_rows WHERE tbl=?1 AND id=?2 AND deleted=0")?
        .query_row(params![source.table, id], |row| row.get(0))
        .optional()?;
    let Some(seq) = seq else {
        return Ok(None);
    };
    let wanted = BTreeSet::from([*source
        .collections
        .iter()
        .find(|name| **name == collection)
        .ok_or(StoreError::RecordContract("not a direct collection"))?]);
    Ok(
        direct_docs(connection, source.table, id, seq, &wanted, true)?
            .into_iter()
            .next(),
    )
}

/// Work units in `(after, through]`, at most `fetch` per source. A delete is taken only
/// above `deletes_after`; `i64::MAX` reads a snapshot. Snapshots need no fan-out: every
/// live work row is already in the snapshot range.
fn work_units(
    transaction: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    wanted: &BTreeSet<&'static str>,
    agent: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let mut units = Vec::new();
    // Work rows, live or deleted. A deleted row is the work tombstone.
    let changed: Vec<(String, i64, bool)> = transaction
        .prepare(
            "SELECT d.id,d.seq,d.deleted FROM durable_rows d LEFT JOIN runtime_work w ON w.id=d.id
             WHERE d.tbl='runtime_work' AND d.seq>?1 AND d.seq<=?2 AND (d.deleted=0 OR d.seq>?3)
               AND (?4 IS NULL OR d.deleted=1 OR json_extract(w.record,'$.rootId')=?4)
             ORDER BY d.seq LIMIT ?5",
        )?
        .query_map(
            params![after, through, deletes_after, agent, fetch],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
        )?
        .collect::<Result<_, _>>()?;
    for (id, seq, deleted) in &changed {
        let rows = if *deleted {
            tombstones(&["work", "workDetail"], wanted, id, *seq)
        } else {
            work_docs(transaction, id, *seq, wanted, false)?
        };
        units.push(Unit { seq: *seq, rows });
    }
    // History rows: one entry per row, keyed `{work}:{index}`.
    for kind in WorkHistoryKind::ALL {
        let collection = kind.collection();
        if !wanted.contains(collection) {
            continue;
        }
        let table = work_history::table_of(kind);
        // Raw SQL: history publication joins durable tombstones and the parent's JSON team selector.
        let entries: Vec<HistoryRow> =
            transaction
                .prepare(&format!(
                    "SELECT d.id,d.seq,d.deleted,c.record,c.work,c.idx,json_extract(w.record,'$.rootId')
                     FROM durable_rows d LEFT JOIN {table} c ON c.id=d.id
                     LEFT JOIN runtime_work w ON w.id=c.work
                     WHERE d.tbl='{table}' AND d.seq>?1 AND d.seq<=?2 AND (d.deleted=0 OR d.seq>?3)
                       AND (?4 IS NULL OR d.deleted=1 OR json_extract(w.record,'$.rootId')=?4)
                     ORDER BY d.seq LIMIT ?5"
                ))?
                .query_map(
                    params![after, through, deletes_after, agent, fetch],
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
                    let entry = crate::sync_views::work_history_entry(
                        kind,
                        &work,
                        studio_json::to_raw_value(&root)?,
                        index as usize,
                        studio_json::OwnedRawJson::from_string(raw)?,
                    );
                    vec![upsert(collection, &id, seq, &entry)?]
                }
                _ => Vec::new(),
            };
            units.push(Unit { seq, rows });
        }
    }
    if deletes_after == i64::MAX || !wanted.contains("workDetail") {
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
        units.push(Unit { seq: *seq, rows });
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
        units.push(Unit { seq, rows });
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
    let item = match studio_json::from_str::<WorkItemRecord>(&raw) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("work row {id} does not decode, no work document is streamed: {error}");
            return Ok(Vec::new());
        }
    };
    let record: Value = studio_json::from_str(&raw)?;
    let mut rows = Vec::new();
    if !detail_only
        && wanted.contains("work")
        && let Some(summary) = crate::sync_views::project_work_record(&item)
    {
        rows.push(upsert("work", id, seq, summary)?);
    }
    if wanted.contains("workDetail") {
        let refusal = current_submission_refusal(transaction, &item).map_err(work_error)?;
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
        payload: Some(studio_json::to_string(&Envelope {
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

#[cfg(test)]
mod catch_up_query_tests {
    use super::*;
    use rusqlite::StatementStatus;

    fn fixture() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE durable_rows(tbl TEXT NOT NULL,id TEXT NOT NULL,seq INTEGER NOT NULL,
                 deleted INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(tbl,id));
             CREATE INDEX durable_rows_seq ON durable_rows(seq);
             CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,root_id TEXT);
             WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<300000)
             INSERT INTO durable_rows SELECT 'unrelated',CAST(x AS TEXT),x,0 FROM n;
             WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
             INSERT INTO durable_rows SELECT 'runtime_agents',CAST(x AS TEXT),x,0 FROM n;
             INSERT INTO durable_rows VALUES
                 ('runtime_agents','a',300001,0),('runtime_agents','b',300003,1),
                 ('runtime_agents','c',300004,0),('runtime_agents','outside',300011,0);
             INSERT INTO runtime_agents VALUES('a','root'),('b','root'),('c','other');",
        )
        .unwrap();
        db.execute(
            "INSERT INTO durable_rows VALUES(?1,'a',300002,0)",
            [durable_seq::AGENT_VIEW_MARK],
        )
        .unwrap();
        db
    }

    #[test]
    fn narrow_agent_changes_seek_sequence_range() {
        let db = fixture();
        for (deletes_after, id, root, limit, expected) in [
            (
                0,
                None,
                None,
                100,
                vec![("a", 300002), ("b", 300003), ("c", 300004)],
            ),
            (
                i64::MAX,
                None,
                None,
                100,
                vec![("a", 300002), ("c", 300004)],
            ),
            (0, Some("a"), None, 100, vec![("a", 300002)]),
            (
                0,
                None,
                Some("root"),
                100,
                vec![("a", 300002), ("b", 300003)],
            ),
            (0, None, None, 1, vec![("a", 300002)]),
        ] {
            let mut query = prepare_agent_changes(&db, 300000, 300010).unwrap();
            let rows = query
                .query_map(
                    params![
                        durable_seq::AGENT_VIEW_MARK,
                        300000,
                        300010,
                        deletes_after,
                        id,
                        limit,
                        root
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(
                rows,
                expected
                    .into_iter()
                    .map(|(id, seq)| (id.to_owned(), seq))
                    .collect::<Vec<_>>()
            );
            let steps = query.reset_status(StatementStatus::VmStep);
            eprintln!("narrow agent changes id={id:?} root={root:?} VM steps={steps}");
            assert!(
                steps < 2000,
                "a ten-number delta must avoid historical agents: {steps}"
            );
        }
    }

    #[test]
    fn agent_changes_range_plan_boundaries() {
        let db = fixture();
        for (after, through, indexed, empty) in [
            (298986, 300010, true, false),  // Exactly 1024 sequence numbers.
            (298985, 300010, false, false), // Exactly 1025 sequence numbers.
            (300010, 300010, true, true),
            (300010, 300009, true, true),
        ] {
            let mut query = prepare_agent_changes(&db, after, through).unwrap();
            assert_eq!(
                query.expanded_sql().unwrap().contains("INDEXED BY"),
                indexed
            );
            let parameters = params![
                durable_seq::AGENT_VIEW_MARK,
                after,
                through,
                0,
                None::<&str>,
                100,
                None::<&str>
            ];
            let read =
                |row: &rusqlite::Row<'_>| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?));
            let actual = query
                .query_map(parameters, read)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let mut original = db.prepare(AGENT_CHANGES_SQL).unwrap();
            let expected = original
                .query_map(parameters, read)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.is_empty(), empty);
            let steps = query.reset_status(StatementStatus::VmStep);
            let before = original.get_status(StatementStatus::VmStep);
            eprintln!("range=({after},{through}] indexed={indexed} VM steps={before}->{steps}");
            if indexed {
                assert!(steps < 8000);
            } else {
                assert_eq!(steps, before);
            }
        }
    }

    #[test]
    fn broad_agent_changes_keep_table_plan() {
        let db = fixture();
        let mut query = prepare_agent_changes(&db, 0, 300010).unwrap();
        let rows = query
            .query_map(
                params![
                    durable_seq::AGENT_VIEW_MARK,
                    0,
                    300010,
                    i64::MAX,
                    None::<&str>,
                    i64::MAX,
                    None::<&str>
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 10002);
        assert_eq!(
            &rows[rows.len() - 2..],
            &[("a".to_owned(), 300002), ("c".to_owned(), 300004)]
        );
        let steps = query.reset_status(StatementStatus::VmStep);
        eprintln!("broad agent changes 300000 unrelated rows VM steps={steps}");
        assert!(
            steps < 500000,
            "broad snapshots must avoid unrelated sequence history: {steps}"
        );
    }
}
