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
//! The other collections come from canonical publications. One merge by change number keeps
//! the paging rules of `SyncDocumentsRepository`: a page is cut at a whole unit, and
//! `checkpoint` is the last unit taken.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rusqlite::{Connection, OptionalExtension, params};
use studio_json::{Value, json};
use studio_runtime_schema::shared_values::SYNC_ENTITY_STREAM_MAX_DOCUMENTS;
use studio_runtime_store::entity_publication::WorkHistoryKind;
use studio_runtime_store::{
    AGENT_VIEW_COLLECTIONS, DIRECT_COLLECTIONS, EntityPage, EntityRow, EntityScopeQueries,
    StoreError, StoreResult, SyncDocumentsRepository, WINDOW_COLLECTIONS, WORK_COLLECTIONS,
    WorkItemRecord, WorkItemsRepository, WorkStatus, durable_seq,
};

use crate::work_write::{WorkWriteError, current_submission_refusal};

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

/// Later sources need no projection beyond the extra candidate already known to
/// fall outside this page. Keep that candidate to prove truncation, including ties.
fn projection_through(units: &[Unit], limit: usize, through: i64) -> i64 {
    if units.len() <= limit {
        return through;
    }
    let mut sequences = units.iter().map(|unit| unit.seq).collect::<Vec<_>>();
    (*sequences.select_nth_unstable(limit).1).min(through)
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
            max_sequence,
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
        let project_through = projection_through(&units, limit as usize, through);
        units.extend(agent_units(
            transaction,
            after,
            through,
            deletes_after,
            &wanted,
            id,
            agent,
            fetch,
            project_through,
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
    /// The durable source label: a main table, or a mark name for a view of several.
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
        let changed = if source.table == "runtime_tasks" {
            EntityScopeQueries::task_changes(connection, after, through, deletes_after, id, fetch)?
        } else {
            EntityScopeQueries::direct_changes(
                connection,
                source.table,
                after,
                through,
                deletes_after,
                id,
                fetch,
            )?
        };
        let snapshot = deletes_after == i64::MAX;
        for (row_id, seq, deleted) in changed {
            let deleted = deleted != 0;
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
    project_through: i64,
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
    let changed = EntityScopeQueries::agent_changes(
        connection,
        after,
        through,
        deletes_after,
        id,
        fetch,
        agent,
    )?;
    let project_through =
        project_through.min(projection_through(&units, (fetch - 1) as usize, through));
    let contexts = if wanted.contains("agentSummary") {
        crate::agent_operational::load(
            connection,
            &changed
                .iter()
                .filter(|(_, seq)| *seq <= project_through)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>(),
        )?
    } else {
        BTreeMap::new()
    };
    let snapshot = deletes_after == i64::MAX;
    for (agent_id, seq) in changed {
        // Read candidate keys over the original interval before deferring views:
        // narrowing SQL before MAX(seq) could expose an older mark for an agent.
        if seq > project_through {
            units.push(Unit {
                seq,
                rows: Vec::new(),
            });
            continue;
        }
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

/// Only lead tails. Four scalar columns from owner tables; no summary/domain aggregation.
fn tail_units(
    connection: &Connection,
    after: i64,
    through: i64,
    deletes_after: i64,
    id: Option<&str>,
    fetch: i64,
) -> StoreResult<Vec<Unit>> {
    let changed =
        EntityScopeQueries::tail_changes(connection, after, through, deletes_after, id, fetch)?;
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
            let current = EntityScopeQueries::agent_sequence(connection, id)?;
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
    Ok(agent_units(
        connection,
        0,
        high,
        i64::MAX,
        &wanted,
        None,
        None,
        i64::MAX,
        high,
    )?
    .into_iter()
    .flat_map(|unit| unit.rows)
    .collect())
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
        let changed = EntityScopeQueries::window_changes(
            connection,
            window,
            source,
            (after, through),
            deletes_after,
            id,
            fetch,
        )?;
        for (member, seq, left) in changed {
            let view = if left != 0 {
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
    let seq = EntityScopeQueries::direct_sequence(connection, source.table, id)?;
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
    let changed =
        EntityScopeQueries::work_changes(transaction, after, through, deletes_after, agent, fetch)?;
    for (id, seq, deleted) in &changed {
        let rows = if *deleted != 0 {
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
        let entries = EntityScopeQueries::history_changes(
            transaction,
            kind,
            after,
            through,
            deletes_after,
            agent,
            fetch,
        )?;
        for (id, seq, deleted, record, work, index, root) in entries {
            let rows = match (deleted != 0, record, work, index, root) {
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
    let owners =
        EntityScopeQueries::changed_ids(transaction, "runtime_agents", after, through, fetch)?;
    let runs = EntityScopeQueries::changed_runs(transaction, after, through, fetch)?;
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
    let changed_works =
        EntityScopeQueries::changed_ids(transaction, "runtime_work", after, through, fetch)?;
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
mod canonical_scope_tests {
    use super::*;

    #[test]
    fn canonical_scope_direct_candidates_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(
            direct_units(&db, 0, 10, 0, &BTreeSet::from(["plan"]), None, None, 10)
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn scoped_page_does_not_project_agents_beyond_its_candidate_boundary() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let directory = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(directory.path()))
            .await
            .unwrap();
        store
            .transaction_with(|tx| {
                for id in ["first", "second"] {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        id,
                        json!({"stateDir":id}),
                        false,
                    )?;
                }
                studio_runtime_store::AgentRepository::put(
                    tx,
                    &studio_runtime_store::AgentRecord::from_value(json!({
                        "id":"later", "rootId":"later", "provider":"codex", "accountKey":"default",
                        "epoch":1, "compactions":0, "status":"idle"
                    }))?,
                )?;
                Ok::<_, StoreError>(())
            })
            .await
            .unwrap();
        store
            .test_read_transaction(|tx| {
                let projections = Arc::new(AtomicUsize::new(0));
                let observed = projections.clone();
                tx.authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                    if matches!(
                        context.action,
                        rusqlite::hooks::AuthAction::Read {
                            table_name: "runtime_agents",
                            column_name: "record",
                            ..
                        }
                    ) {
                        observed.fetch_add(1, Ordering::Relaxed);
                    }
                    rusqlite::hooks::Authorization::Allow
                }))?;
                let collections = r#"["workspace","agentSummary"]"#;
                let mut page = resume(tx, None, Some(collections), None, None, false, 1)?;
                assert_eq!(page.rows[0].id, "current");
                assert!(page.checkpoint < page.max_sequence);
                assert_eq!(
                    projections.load(Ordering::Relaxed),
                    0,
                    "later agent views must not be built and discarded on this page"
                );
                tx.authorizer(
                    None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
                )?;
                let mut ids = page
                    .rows
                    .iter()
                    .map(|row| row.id.clone())
                    .collect::<Vec<_>>();
                while page.checkpoint < page.max_sequence {
                    let next = continue_page(tx, &page, Some(collections), None, None, false, 1)?;
                    assert!(next.checkpoint > page.checkpoint);
                    ids.extend(next.rows.iter().map(|row| row.id.clone()));
                    page = next;
                }
                assert_eq!(ids, ["current", "first", "second", "later"]);
                Ok::<_, StoreError>(())
            })
            .unwrap();
    }

    #[test]
    fn canonical_scope_task_candidates_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(
            direct_units(
                &db,
                0,
                10,
                0,
                &BTreeSet::from(["taskDetail"]),
                Some("missing"),
                None,
                10
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn canonical_scope_agent_candidates_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(
            agent_units(
                &db,
                0,
                10,
                0,
                &BTreeSet::from(["agentDetail"]),
                None,
                None,
                10,
                10
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn canonical_scope_tail_candidates_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(tail_units(&db, 0, 10, 0, None, 10).unwrap().is_empty());
    }
    #[test]
    fn canonical_scope_agent_sequence_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(
            agent_docs_with_context(
                &db,
                "missing",
                None,
                &BTreeSet::from(["agentDetail"]),
                true,
                None
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn canonical_scope_window_candidates_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(
            window_units(
                &db,
                0,
                10,
                0,
                &BTreeSet::from(["task", "monitor"]),
                None,
                None,
                10
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn canonical_scope_direct_sequence_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        assert!(direct_document(&db, "plan", "missing").unwrap().is_none());
    }
    #[test]
    fn canonical_scope_work_and_history_fanout_need_no_legacy_table() {
        let db = studio_runtime_store::test_canvas();
        let wanted = WORK_COLLECTIONS.iter().copied().collect();
        assert!(
            work_units(&db, 0, 10, 0, &wanted, None, 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn canonical_scope_work_history_and_fanout_keep_service_envelopes() {
        let mut db = studio_runtime_store::test_canvas();
        let tx = db.transaction().unwrap();
        let raw = r#"{"id":"work","status":"ready","rootId":"root","title":"example","results":[{"text":"result"}],"decisions":[{"text":"decision"}],"releases":["release"]}"#;
        let record = WorkItemRecord::decode("work", raw).unwrap();
        studio_runtime_store::Table::new(&tx).put(&record).unwrap();
        studio_runtime_store::work_history::reconcile(&tx, "work", &record.value()).unwrap();
        let high = durable_seq::high(&tx).unwrap();
        let wanted = WORK_COLLECTIONS
            .iter()
            .copied()
            .filter(|name| *name != "workDetail")
            .collect();
        let units = work_units(&tx, 0, high, 0, &wanted, Some("root"), 100).unwrap();
        let rows: Vec<_> = units.into_iter().flat_map(|unit| unit.rows).collect();
        assert!(
            rows.iter()
                .any(|row| row.collection == "work" && row.id == "work" && !row.deleted)
        );
        for kind in WorkHistoryKind::ALL {
            assert!(rows.iter().any(|row| row.collection == kind.collection()
                && row.id == "work:0"
                && !row.deleted));
        }
        assert!(
            work_units(&tx, 0, high, 0, &wanted, Some("other"), 100)
                .unwrap()
                .iter()
                .all(|unit| unit.rows.is_empty())
        );
    }
}
