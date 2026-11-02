//! The outbox status `cancelled`: a discarded entry keeps its row and id. The status
//! CHECK of `runtime_outbox` lists every status, so a store created before the change
//! rebuilds the table with the current CHECK.
use rusqlite::Transaction;
use studio_runtime_schema::persistence::{OutboxKind, OutboxStatus, OwnerKind, enum_check};

use super::m2026_10_31_current_checks::rebuild;
use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_outbox")? {
        return Ok(());
    }
    let table_sql: String = tx.query_row(
        "SELECT sql FROM sqlite_schema WHERE type='table' AND name='runtime_outbox'",
        [],
        |row| row.get(0),
    )?;
    if table_sql.contains(&enum_check::<OutboxStatus>("status")) {
        return Ok(());
    }
    let ddl = format!(
        "CREATE TABLE runtime_outbox_current_checks (
            id TEXT PRIMARY KEY, kind TEXT NOT NULL {kind}, owner_kind TEXT NOT NULL {owner},
            owner_id TEXT NOT NULL, request_id TEXT, signature TEXT NOT NULL,
            status TEXT NOT NULL {status}, account TEXT, stream_id TEXT, generation INTEGER,
            ordinal INTEGER, next_at INTEGER, payload TEXT NOT NULL, created INTEGER NOT NULL, target TEXT,
            CHECK((status!='unknown' AND status!='unrecognized' AND status!='cancelled') OR next_at IS NULL))",
        kind = enum_check::<OutboxKind>("kind"),
        owner = enum_check::<OwnerKind>("owner_kind"),
        status = enum_check::<OutboxStatus>("status"),
    );
    rebuild(
        tx,
        "runtime_outbox",
        &ddl,
        "INSERT INTO runtime_outbox_current_checks(id,kind,owner_kind,owner_id,request_id,signature,status,account,stream_id,generation,ordinal,next_at,payload,created,target)
         SELECT id,kind,owner_kind,owner_id,request_id,signature,status,account,stream_id,generation,ordinal,next_at,payload,created,target FROM runtime_outbox ORDER BY rowid",
        &[],
    )
}
