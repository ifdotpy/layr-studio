//! Durable change source: a change number for each row of the main tables, tombstones for
//! deletes.
//!
//! `durable_rows` holds one entry per row `(table, id, seq, deleted)`. A trigger on each
//! opted-in table sets the entry's `seq` to the next number on every insert and update;
//! a delete turns the entry into a tombstone. The main tables keep their own columns, so
//! no writer, fixture or migration changes.
//!
//! The numbers are the counter of `sync_entities`, `sync_documents` and `sync_generation`:
//! a new number is [`NEXT`], one above the highest of those tables and `durable_rows`.
//! Every allocator uses that rule, so there is one cursor space. A reader keeps the
//! highest `seq` it applied and asks [`changes_since`] for what is newer. This replaces
//! the copy of the main rows in `sync_entities`; see docs/sync-protocol.md.
//!
//! Opt in a new table with one line in [`DURABLE_TABLES`]. The table needs an `id`
//! primary key and a text payload column. Installation is idempotent.
//!
//! Tombstones are service data and are kept long. A delete prunes tombstones older than
//! [`TOMBSTONE_RETENTION_SECONDS`] and raises `durable_floor.floor` to the highest pruned
//! `seq`. A cursor below the floor cannot learn every delete, so it gets a snapshot.
//!
//! The DDL of `durable_rows` and `durable_floor` is in `sync_schema.sql` (section
//! `common`), where the allocators that read `durable_rows` are.

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::{StoreError, StoreResult};

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
    /// True for `runtime_agents`: a change of an agent's team (root or liveness) moves
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
    DurableTable::new("runtime_work_results", "record"),
    DurableTable::new("runtime_work_decisions", "record"),
    DurableTable::new("runtime_work_releases", "record"),
    // Execution runs: a failure refusal of a work document depends on the owner's run.
    DurableTable::new("runtime_execution_runs", "record"),
    // Sources of documents built at read time, one document per row.
    DurableTable::record("runtime_plans"),
    DurableTable::record("runtime_annotations"),
    DurableTable::record("runtime_rules"),
    DurableTable::new("approvals", "request"),
    DurableTable::record("runtime_complaints"),
    // Projects: a `project` document per live local project (`runtime_projects`, whose
    // entry the project grant writers also mark) and per mirrored peer project.
    DurableTable::record("runtime_projects"),
    DurableTable::record("peer_projects"),
    // Account observations: the rate-limit view of each account, the account list
    // summary and the cost summary of each account, one document per row.
    DurableTable::record("runtime_rate_limits"),
    DurableTable::record("runtime_account_summaries"),
    DurableTable::record("runtime_account_costs"),
    DurableTable::record("runtime_session_costs"),
    DurableTable::record("runtime_desktop_states"),
    DurableTable::record("runtime_models"),
    // Chat membership edges: an `edge` document per chat edge, built at read time.
    DurableTable::new("graph_edges", "target"),
    // Window sources: a row in its window (`TASK_WINDOW`, `MONITOR_WINDOW`) is sent
    // again when it changes.
    DurableTable::record("runtime_tasks"),
    DurableTable::record("runtime_monitors"),
    // The newest events form the event window, built at read time.
    DurableTable::new("runtime_events", "status"),
    // Domain rows of a chat: the chat, its turns and its provider sessions.
    DurableTable::new("chats", "status"),
    DurableTable::new("turns", "state"),
    DurableTable::new("sessions", "state"),
    // Chat messages between agents and people.
    DurableTable::new("messages", "deliveries"),
    // Rows that never change after they are written: checkpoints and attachments.
    DurableTable::record("runtime_checkpoints"),
    DurableTable::record("runtime_assets"),
];

/// The next number of the shared counter. Owner of the allocation rule. The same text
/// is in `sync_schema.sql`, `feature_schema.rs` and `sync_documents::next_sequence`.
const NEXT: &str = "(SELECT MAX(entity_seq,document_seq,generation,durable_seq)+1 FROM (
    SELECT COALESCE(MAX(seq),0) AS entity_seq FROM sync_entities),
    (SELECT COALESCE(MAX(seq),0) AS document_seq FROM sync_documents),
    (SELECT COALESCE(MAX(value),0) AS generation FROM sync_generation WHERE id=1),
    (SELECT COALESCE(MAX(seq),0) AS durable_seq FROM durable_rows))";

/// The generator: SQL for the three triggers of one table.
fn trigger_sql(table: &str, record_only: bool, team_fence: bool) -> String {
    let now = "CAST(strftime('%s','now') AS INTEGER)";
    let cutoff = format!("{now}-{TOMBSTONE_RETENTION_SECONDS}");
    let put = |id: &str, deleted: u8, at: &str| {
        format!(
            "INSERT INTO durable_rows(tbl,id,seq,deleted,at) VALUES('{table}',{id},{NEXT},{deleted},{at})
             ON CONFLICT(tbl,id) DO UPDATE SET seq=excluded.seq,deleted=excluded.deleted,at=excluded.at;"
        )
    };
    let live = put("NEW.id", 0, "0");
    let tombstone = put("OLD.id", 1, now);
    // The scope visibility fence of a team (owner: sync_documents
    // ENTITY_SCOPE_VISIBILITY_KEY_PREFIX) at the number this change just took.
    let fence = |roots: &str, id: &str| {
        if !team_fence {
            return String::new();
        }
        format!(
            "INSERT INTO sync_entity_meta(key,value)
               SELECT 'entity_scope_visibility:' || json_array('*',root),
                      CAST((SELECT seq FROM durable_rows WHERE tbl='{table}' AND id={id}) AS TEXT)
               FROM ({roots}) WHERE root IS NOT NULL
             ON CONFLICT(key) DO UPDATE SET value=CAST(MAX(CAST(sync_entity_meta.value AS INTEGER),CAST(excluded.value AS INTEGER)) AS TEXT);"
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
            "SELECT root FROM ({} UNION {}) WHERE json_extract(OLD.record,'$.rootId') IS NOT json_extract(NEW.record,'$.rootId')
               OR (json_extract(OLD.record,'$.deletedAt') IS NULL) IS NOT (json_extract(NEW.record,'$.deletedAt') IS NULL)",
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
        "
CREATE TRIGGER IF NOT EXISTS durable_{table}_insert AFTER INSERT ON {table} BEGIN
  {live}
  {insert_fence}
END;
CREATE TRIGGER IF NOT EXISTS durable_{table}_update AFTER UPDATE ON {table}{changed} BEGIN
  {live}
  INSERT INTO durable_rows(tbl,id,seq,deleted,at)
    SELECT '{table}',OLD.id,{NEXT},1,{now} WHERE OLD.id IS NOT NEW.id
    ON CONFLICT(tbl,id) DO UPDATE SET seq=excluded.seq,deleted=1,at=excluded.at;
  {update_fence}
END;
CREATE TRIGGER IF NOT EXISTS durable_{table}_delete AFTER DELETE ON {table} BEGIN
  UPDATE durable_floor SET floor=MAX(floor,
    (SELECT MAX(seq) FROM durable_rows WHERE deleted=1 AND at<{cutoff}))
  WHERE id=1 AND EXISTS(SELECT 1 FROM durable_rows WHERE deleted=1 AND at<{cutoff});
  DELETE FROM durable_rows WHERE deleted=1 AND at<{cutoff};
  {tombstone}
  {delete_fence}
END;
"
    )
}

/// Triggers that give a live local project a new number when one of its grants changes:
/// its `project` document lists the active peer leads. A gone project stays gone.
pub(crate) fn grant_mark_triggers_sql() -> String {
    let mark = |row: &str| {
        format!(
            "UPDATE durable_rows SET seq={NEXT} WHERE tbl='runtime_projects' AND id={row}.local_project AND deleted=0;"
        )
    };
    format!(
        "CREATE TRIGGER IF NOT EXISTS durable_project_grants_insert AFTER INSERT ON project_grants BEGIN {new} END;
         CREATE TRIGGER IF NOT EXISTS durable_project_grants_update AFTER UPDATE ON project_grants BEGIN {old} {new} END;
         CREATE TRIGGER IF NOT EXISTS durable_project_grants_delete AFTER DELETE ON project_grants BEGIN {old} END;",
        new = mark("NEW"),
        old = mark("OLD"),
    )
}

/// Number the rows of every opted-in table that have no entry yet (rows that predate
/// the source) and install the triggers. Runs as a migration and on a fresh state.
pub(crate) fn install(tx: &Transaction<'_>) -> StoreResult<()> {
    if crate::schema::table_exists(tx, "project_grants")? {
        tx.execute_batch(&grant_mark_triggers_sql())?;
    }
    for table in DURABLE_TABLES {
        let name = table.name;
        // A table created later by its own migration is numbered when it exists.
        if !crate::schema::table_exists(tx, name)? {
            continue;
        }
        if !table.online_backfill {
            let base: i64 = tx.query_row(&format!("SELECT {NEXT}-1"), [], |r| r.get(0))?;
            tx.execute_batch(&format!(
                "INSERT INTO durable_rows(tbl,id,seq,deleted,at)
                   SELECT '{name}',id,{base}+ROW_NUMBER() OVER (ORDER BY rowid),0,0 FROM {name} t
                   WHERE NOT EXISTS (SELECT 1 FROM durable_rows d WHERE d.tbl='{name}' AND d.id=t.id);"
            ))?;
        }
        tx.execute_batch(&trigger_sql(name, table.record_only, table.team_fence))?;
    }
    Ok(())
}

/// Keep the counter above every number used so far. Owner of the floor rule: it raises the
/// persisted part of the counter, `sync_generation.value`, to at least `NEXT - 1`.
/// Callers must run it in the same transaction before deleting any `sync_entities` rows,
/// because `NEXT` reads the highest `sync_entities.seq`. Without it, deleting the row that
/// holds the highest number lets `NEXT` repeat that number.
pub fn keep_counter(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute(
        &format!("UPDATE sync_generation SET value=MAX(value,{NEXT}-1) WHERE id=1"),
        [],
    )?;
    Ok(())
}

/// Name of the change marks of the agent views in `durable_rows`. The views are built
/// at read time; a mark asks readers to build them again (owner: `entity_scope` in
/// runtime-services).
pub const AGENT_VIEW_MARK: &str = "agent_views";

/// The `durable_rows` table name of the chat views: a chat document reads its group,
/// its member edges and its messages, so their writers mark the chat.
pub const CHAT_VIEW_MARK: &str = "chat_views";

/// The `durable_rows` table name of the message receipts: a receipt reads its event,
/// the event metadata and the transcript items, so their writers mark the event.
pub const RECEIPT_VIEW_MARK: &str = "receipt_views";

/// The `durable_rows` tables of the window members: a live entry puts the task or
/// monitor of that id in its client window, a tombstone takes it out. The writer that
/// owns the window rule (`sync_policy` in runtime-services) keeps them.
pub const TASK_WINDOW: &str = "task_window";
pub const MONITOR_WINDOW: &str = "monitor_window";

/// The `durable_rows` window of the visible peer teams: the peer team writers (owner:
/// `sync_policy` in runtime-services) mark a team when its view may change and take it
/// out when it is no longer visible. Its documents are built at read time.
pub const PEER_TEAM_WINDOW: &str = "peer_team_window";

/// Put `id` in window `tbl`. An id already in it keeps its number.
pub fn join_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    let changed = connection.execute(
        &format!(
            "INSERT INTO durable_rows(tbl,id,seq,deleted,at) VALUES(?1,?2,{NEXT},0,0)
             ON CONFLICT(tbl,id) DO UPDATE SET seq=excluded.seq,deleted=0,at=0
             WHERE durable_rows.deleted=1"
        ),
        rusqlite::params![tbl, id],
    )?;
    Ok(changed > 0)
}

/// Take `id` out of window `tbl`: a tombstone at a new number. An id not in it is
/// left alone.
pub fn leave_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    let changed = connection.execute(
        &format!(
            "UPDATE durable_rows SET seq={NEXT},deleted=1,at=CAST(strftime('%s','now') AS INTEGER)
             WHERE tbl=?1 AND id=?2 AND deleted=0"
        ),
        rusqlite::params![tbl, id],
    )?;
    Ok(changed > 0)
}

/// The ids in window `tbl`.
pub fn window_members(connection: &Connection, tbl: &str) -> StoreResult<Vec<String>> {
    Ok(connection
        .prepare_cached("SELECT id FROM durable_rows WHERE tbl=?1 AND deleted=0")?
        .query_map([tbl], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Whether `id` is in window `tbl`.
pub fn in_window(connection: &Connection, tbl: &str, id: &str) -> StoreResult<bool> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM durable_rows WHERE tbl=?1 AND id=?2 AND deleted=0)",
        )?
        .query_row(rusqlite::params![tbl, id], |row| row.get(0))?)
}

/// Give `(tbl, id)` a new number without a row write: a derived document of `id`
/// changed. A mark is never a tombstone.
pub fn mark(connection: &Connection, tbl: &str, id: &str) -> StoreResult<()> {
    connection.execute(
        &format!(
            "INSERT INTO durable_rows(tbl,id,seq,deleted,at) VALUES(?1,?2,{NEXT},0,0)
             ON CONFLICT(tbl,id) DO UPDATE SET seq=excluded.seq,deleted=0,at=0"
        ),
        rusqlite::params![tbl, id],
    )?;
    Ok(())
}

/// Give one row of an `online_backfill` table its entry, unless it has one. Rows written
/// after the triggers exist have an entry from them, so this only reaches old rows. Safe
/// to repeat: a row that has an entry keeps its number.
pub fn number_row(connection: &Connection, table: &str, rowid: i64) -> StoreResult<()> {
    connection.execute(
        &format!(
            "INSERT INTO durable_rows(tbl,id,seq,deleted,at)
               SELECT '{table}',id,{NEXT},0,0 FROM {table} WHERE rowid=?1
             ON CONFLICT(tbl,id) DO NOTHING"
        ),
        [rowid],
    )?;
    Ok(())
}

/// A live row at or below the returned cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRow {
    pub id: String,
    pub seq: i64,
    pub payload: String,
}

/// A deleted id.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Ok(connection.query_row(&format!("SELECT {NEXT}-1"), [], |row| row.get(0))?)
}

/// The highest change number of any sync table, and the durable tombstone floor.
pub fn high_and_floor(connection: &Connection) -> StoreResult<(i64, i64)> {
    let high = high(connection)?;
    let floor: i64 =
        connection.query_row("SELECT floor FROM durable_floor WHERE id=1", [], |row| {
            row.get(0)
        })?;
    Ok((high, floor))
}

/// Rows of `table` with `seq` above `cursor`, or a snapshot when the cursor is absent,
/// in the future, or below the tombstone floor.
///
/// The cursor space is shared with `sync_entities`, so `seq` is the highest number of any
/// sync table, and a cursor taken from one of them is valid here. The high mark is read
/// first and later writes are cut off at it, so one call needs no read transaction: a
/// change committed meanwhile has a higher `seq` and arrives with the next call.
pub fn changes_since(
    connection: &Connection,
    table: &str,
    cursor: Option<i64>,
) -> StoreResult<DurableChanges> {
    let table = DURABLE_TABLES
        .iter()
        .find(|candidate| candidate.name == table)
        .ok_or_else(|| StoreError::SchemaMismatch(format!("{table} is not a durable table")))?;
    let high: i64 = connection.query_row(&format!("SELECT {NEXT}-1"), [], |row| row.get(0))?;
    let floor: i64 =
        connection.query_row("SELECT floor FROM durable_floor WHERE id=1", [], |row| {
            row.get(0)
        })?;
    let after = cursor.filter(|cursor| (floor..=high).contains(cursor));
    let rows = |after: i64| -> StoreResult<Vec<DurableRow>> {
        let sql = format!(
            "SELECT t.id,d.seq,t.{} FROM durable_rows d JOIN {} t ON t.id=d.id
             WHERE d.tbl=?1 AND d.deleted=0 AND d.seq>?2 AND d.seq<=?3 ORDER BY d.seq",
            table.payload, table.name
        );
        let rows = connection
            .prepare(&sql)?
            .query_map(rusqlite::params![table.name, after, high], |row| {
                Ok(DurableRow {
                    id: row.get(0)?,
                    seq: row.get(1)?,
                    payload: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    };
    let Some(after) = after else {
        return Ok(DurableChanges::Snapshot {
            rows: rows(0)?,
            seq: high,
        });
    };
    let tombstones = connection
        .prepare(
            "SELECT id,seq FROM durable_rows WHERE tbl=?1 AND deleted=1 AND seq>?2 AND seq<=?3
             ORDER BY seq",
        )?
        .query_map(rusqlite::params![table.name, after, high], |row| {
            Ok(DurableTombstone {
                id: row.get(0)?,
                seq: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DurableChanges::Delta {
        rows: rows(after)?,
        tombstones,
        seq: high,
    })
}

/// Data-loss check for removing the transcript copy in `sync_entities`: ids of live
/// `transcript:<agent>` index rows whose item is missing from the owner table
/// `runtime_items`. Empty means the owner holds every item the copy lists.
pub fn transcript_copy_missing_from_owner(
    connection: &Connection,
) -> StoreResult<Vec<(String, String)>> {
    let missing = connection
        .prepare(
            "SELECT substr(s.collection,12),substr(s.id,6) FROM sync_entities s
             WHERE s.collection LIKE 'transcript:%' AND s.id LIKE 'item:%' AND s.deleted=0
               AND NOT EXISTS (SELECT 1 FROM runtime_items i
                               WHERE i.agent=substr(s.collection,12) AND i.id=substr(s.id,6))
             ORDER BY s.collection,s.id",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(missing)
}

/// `sync_entity_meta` key that records the end of the transcript copies in `sync_entities`.
const COPIES_KEY: &str = "transcript_copies";
const COPIES_RETIRED: &str = "retired";

/// True once transcript pages are built at read time and the writer keeps no transcript
/// copy in `sync_entities`.
pub fn transcript_copies_retired(connection: &Connection) -> StoreResult<bool> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key=?1",
            [COPIES_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.as_deref() == Some(COPIES_RETIRED))
}

/// Stop writing transcript copies, but only when the owner table holds every item the
/// copies list. Returns the missing `(agent, item)` pairs; on a non-empty result nothing
/// changes and the writer keeps the copies. Old rows are never deleted.
pub fn retire_transcript_copies(tx: &Transaction<'_>) -> StoreResult<Vec<(String, String)>> {
    let missing = transcript_copy_missing_from_owner(tx)?;
    if missing.is_empty() {
        tx.execute(
            "INSERT INTO sync_entity_meta(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [COPIES_KEY, COPIES_RETIRED],
        )?;
    }
    Ok(missing)
}

/// The change number of a conversation: the highest `durable_rows.seq` of its agent, its
/// items, and any deleted item. It only grows, and a cursor from any sync table is valid
/// against it because they share one counter.
pub fn transcript_change_cursor(connection: &Connection, agent: &str) -> StoreResult<i64> {
    Ok(connection.query_row(
        "SELECT MAX(
            COALESCE((SELECT seq FROM durable_rows WHERE tbl='runtime_agents' AND id=?1),0),
            COALESCE((SELECT MAX(d.seq) FROM runtime_items i
                      JOIN durable_rows d ON d.tbl='runtime_items' AND d.id=i.id
                      WHERE i.agent=?1),0),
            COALESCE((SELECT MAX(seq) FROM durable_rows
                      WHERE deleted=1 AND tbl='runtime_items'),0))",
        [agent],
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    fn canvas() -> Connection {
        crate::fresh_state::test_canvas()
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
        db.query_row(
            "SELECT seq FROM durable_rows WHERE tbl='runtime_work' AND id=?1",
            [id],
            |r| r.get(0),
        )
        .expect("seq")
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

    #[tokio::test]
    async fn domain_rows_messages_checkpoints_and_attachments_take_numbers() {
        let root = tempfile::tempdir().expect("state");
        let store = crate::RuntimeStore::open(Some(root.path()))
            .await
            .expect("store");
        store
            .transaction(|tx| {
                tx.execute_batch(
                    "INSERT INTO chats(id,state,status) VALUES('c','active','idle');
                     INSERT INTO turns(id,chat,state) VALUES('t','c','completed');
                     INSERT INTO sessions(id,chat,thread_id,account_key,state) VALUES('s','c','thread','default','ready');
                     INSERT INTO messages(id,room,author,text,at,deliveries) VALUES('m','room','c','hi',1,'{}');
                     INSERT INTO runtime_checkpoints(id,record) VALUES('k','{\"id\":\"k\"}');
                     INSERT INTO runtime_assets(id,record) VALUES('a','{\"id\":\"a\"}');",
                )?;
                Ok(())
            })
            .await
            .expect("rows");
        let db = store.test_reader().expect("reader");
        for (table, id) in [
            ("chats", "c"),
            ("turns", "t"),
            ("sessions", "s"),
            ("messages", "m"),
            ("runtime_checkpoints", "k"),
            ("runtime_assets", "a"),
        ] {
            let seq: i64 = db
                .query_row(
                    "SELECT seq FROM durable_rows WHERE tbl=?1 AND id=?2 AND deleted=0",
                    [table, id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|error| panic!("{table}: {error}"));
            assert!(seq > 0, "{table}");
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
        let tombstone: i64 = db
            .query_row(
                "SELECT seq FROM durable_rows WHERE tbl='runtime_work' AND id='b' AND deleted=1",
                [],
                |r| r.get(0),
            )
            .expect("tombstone");
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
        db.execute("UPDATE durable_rows SET at=0 WHERE deleted=1", [])
            .expect("age");
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
        let high: i64 = db
            .query_row(&format!("SELECT {NEXT}-1"), [], |r| r.get(0))
            .expect("counter");
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
        db.execute(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             VALUES('x','1',?1,'h','{}',0)",
            [entity],
        )
        .expect("entity");
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
        db.execute_batch(
            "INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('agentSummary','a',5,'h','{}',0),
               ('event','e',9,'h','{}',0);",
        )
        .expect("entities");
        let tx = db.transaction().expect("transaction");
        keep_counter(&tx).expect("keep the counter");
        tx.execute(
            "DELETE FROM sync_entities WHERE collection='event' AND id='e'",
            [],
        )
        .expect("delete the row with the max seq");
        tx.commit().expect("commit");
        let fresh: i64 = db
            .query_row(&format!("SELECT {NEXT}"), [], |r| r.get(0))
            .expect("next");
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
        let seq = || -> i64 {
            db.query_row(
                "SELECT seq FROM durable_rows WHERE tbl='runtime_rules' AND id='r'",
                [],
                |row| row.get(0),
            )
            .expect("entry")
        };
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
        assert!(changes_since(&canvas(), "runtime_rooms", None).is_err());
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
        db.execute_batch(
            "DROP TRIGGER durable_runtime_work_insert;
             DELETE FROM durable_rows WHERE tbl='runtime_work';
             INSERT INTO runtime_work(id,record) VALUES('old','{}'),('older','{}');",
        )
        .expect("old format");
        let tx = db.transaction().expect("transaction");
        install(&tx).expect("install over old format");
        tx.commit().expect("commit");
        let high: i64 = db
            .query_row(&format!("SELECT {NEXT}-1"), [], |r| r.get(0))
            .expect("counter");
        let (a, old, older) = (seq_of(&db, "a"), seq_of(&db, "old"), seq_of(&db, "older"));
        assert!(a > 0 && old > a && older > old && older <= high);
    }

    #[test]
    fn the_transcript_copy_check_names_items_the_owner_lacks() {
        let db = canvas();
        db.execute_batch(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES('i1','a1','{}',1);
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('transcript:a1','item:i1',1,'h',NULL,0),
               ('transcript:a1','item:i2',2,'h',NULL,0),
               ('transcript:a1','item:i3',3,'h',NULL,1),
               ('agent','a1',4,'h','{}',0);",
        )
        .expect("rows");
        assert_eq!(
            transcript_copy_missing_from_owner(&db).expect("check"),
            vec![("a1".to_owned(), "i2".to_owned())]
        );
    }

    #[test]
    fn retiring_the_copies_needs_every_item_in_the_owner_and_never_deletes_rows() {
        let mut db = canvas();
        assert!(!transcript_copies_retired(&db).expect("flag"));
        db.execute_batch(
            "DELETE FROM sync_entity_meta WHERE key='transcript_copies';
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted) VALUES
               ('transcript:a1','item:i2',2,'h',NULL,0);",
        )
        .expect("a copy row without its item");
        let tx = db.transaction().expect("transaction");
        let missing = retire_transcript_copies(&tx).expect("check");
        tx.commit().expect("commit");
        assert_eq!(missing, vec![("a1".to_owned(), "i2".to_owned())]);
        assert!(
            !transcript_copies_retired(&db).expect("flag"),
            "the writer keeps the copies"
        );
        db.execute_batch(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES('i2','a1','{}',1)",
        )
        .expect("owner row");
        let tx = db.transaction().expect("transaction");
        assert!(retire_transcript_copies(&tx).expect("check").is_empty());
        tx.commit().expect("commit");
        assert!(transcript_copies_retired(&db).expect("flag"));
        let copies: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sync_entities WHERE collection='transcript:a1'",
                [],
                |row| row.get(0),
            )
            .expect("count");
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
    }
}
