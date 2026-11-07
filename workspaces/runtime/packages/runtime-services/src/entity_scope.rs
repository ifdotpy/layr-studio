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
use studio_runtime_schema::sidebar::COLLECTION as SIDEBAR_COLLECTION;
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
    let priority_id = if after == 0 { priority_id } else { None };
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
    units.sort_by_key(|unit| unit.seq);
    let room = limit as usize - head.len();
    let truncated = units.len() > room;
    units.truncate(room);
    let mut rows = head;
    rows.extend(units.into_iter().flat_map(|unit| unit.rows));
    let checkpoint = if truncated {
        rows.last().map_or(after, |row| row.sequence)
    } else {
        max_sequence
    };
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
    units.sort_by_key(|unit| unit.seq);
    let truncated = units.len() as i64 > limit;
    units.truncate(limit as usize);
    let checkpoint = if truncated {
        units.last().map_or(after, |unit| unit.seq)
    } else {
        through
    };
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
const DIRECT_SOURCES: [DirectSource; 7] = [
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
        if !source.collections.iter().any(|c| wanted.contains(c))
            || source.table != durable_seq::CHAT_VIEW_MARK
                && !crate::sync_policy::table_exists(connection, source.table)?
        {
            continue;
        }
        let changed: Vec<(String, i64, bool)> = connection
            .prepare_cached(
                "SELECT id,seq,deleted FROM durable_rows
                 WHERE tbl=?1 AND seq>?2 AND seq<=?3 AND (deleted=0 OR seq>?4)
                   AND (?5 IS NULL OR id=?5)
                 ORDER BY seq LIMIT ?6",
            )?
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
            if !rows.is_empty() {
                units.push(Unit { seq, rows });
            }
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
/// agent. These collections belong to the global list and the id-scoped detail, so an
/// agent-scoped reader gets none of them.
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
    let mut units = Vec::new();
    if agent.is_some() || !AGENT_VIEW_COLLECTIONS.iter().any(|c| wanted.contains(c)) {
        return Ok(units);
    }
    // The agent row and its view mark both name the agent: one unit at the later one.
    let changed: Vec<(String, i64)> = connection
        .prepare_cached(
            "SELECT id,MAX(seq) AS last FROM durable_rows
             WHERE tbl IN ('runtime_agents',?1) AND seq>?2 AND seq<=?3
               AND (deleted=0 OR seq>?4) AND (?5 IS NULL OR id=?5)
             GROUP BY id ORDER BY last LIMIT ?6",
        )?
        .query_map(
            params![
                durable_seq::AGENT_VIEW_MARK,
                after,
                through,
                deletes_after,
                id,
                fetch
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<_, _>>()?;
    let snapshot = deletes_after == i64::MAX;
    for (agent_id, seq) in changed {
        // A gone agent reads as no views: deletes in a delta, nothing in a snapshot.
        let rows =
            agent_docs(connection, &agent_id, Some(seq), wanted, snapshot).or_else(|error| {
                // One agent whose view cannot be built must not stop the stream.
                eprintln!("agent {agent_id} views are not streamed: {error}");
                Ok::<_, StoreError>(Vec::new())
            })?;
        if !rows.is_empty() {
            units.push(Unit { seq, rows });
        }
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
    let mut sidebar = Vec::new();
    if wanted.contains(SIDEBAR_COLLECTION) {
        match crate::sidebar::row(connection, id, server.as_deref())? {
            Some(value) => sidebar.push(upsert(SIDEBAR_COLLECTION, id, seq, value)?),
            None if !snapshot => sidebar.extend(tombstones(&[SIDEBAR_COLLECTION], wanted, id, seq)),
            None => {}
        }
        if wanted.len() == 1 {
            return Ok(sidebar);
        }
    }
    let Some(documents) =
        crate::sync_policy::agent_view_documents(connection, id, server.as_deref())?
    else {
        return Ok(if snapshot {
            Vec::new()
        } else {
            tombstones(AGENT_VIEW_COLLECTIONS, wanted, id, seq)
        });
    };
    let mut rows = sidebar;
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
    let wanted = AGENT_VIEW_COLLECTIONS
        .iter()
        .copied()
        .filter(|name| *name != SIDEBAR_COLLECTION)
        .collect();
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
                        continue;
                    }
                    vec![upsert(collection, &member, seq, value)?]
                }
                None if snapshot => continue,
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
                    let value: Value = studio_json::from_str(&raw)?;
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
