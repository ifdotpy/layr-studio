//! A resolve command refers to an outbox entry, so `request_results.ref_kind` admits the
//! `outbox` reference kind. The CHECK lists every kind, so a store created before the change
//! rebuilds the table with the current CHECK.
use rusqlite::Transaction;
use studio_runtime_schema::persistence::{RequestReferenceKind, RequestResultStatus, enum_check};

use super::m2026_10_31_current_checks::rebuild;
use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "request_results")? {
        return Ok(());
    }
    let table_sql: String = tx.query_row(
        "SELECT sql FROM sqlite_schema WHERE type='table' AND name='request_results'",
        [],
        |row| row.get(0),
    )?;
    if table_sql.contains(&enum_check::<RequestReferenceKind>("ref_kind")) {
        return Ok(());
    }
    let ddl = format!(
        "CREATE TABLE request_results_current_checks (
           id TEXT PRIMARY KEY,
           signature TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'applied' {status},
           ref_kind TEXT {kind},
           ref_id TEXT,
           ref_rev INTEGER,
           result TEXT,
           created INTEGER NOT NULL,
           CHECK ((result IS NOT NULL AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)
               OR (result IS NULL AND ref_kind IS NOT NULL AND ref_id IS NOT NULL AND ref_rev IS NOT NULL)
               OR (result IS NULL AND status = 'rejected' AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)))",
        status = enum_check::<RequestResultStatus>("status"),
        kind = enum_check::<RequestReferenceKind>("ref_kind"),
    );
    rebuild(
        tx,
        "request_results",
        &ddl,
        "INSERT INTO request_results_current_checks(id,signature,status,ref_kind,ref_id,ref_rev,result,created)
         SELECT id,signature,status,ref_kind,ref_id,ref_rev,result,created FROM request_results ORDER BY rowid",
        &[],
    )
}
