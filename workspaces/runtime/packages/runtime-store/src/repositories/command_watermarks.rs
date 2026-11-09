//! Per-client command watermark: the highest applied command number of each client.
//!
//! A client is a device, an agent, or a peer. It numbers its commands from 1. A command
//! older than the `request_results` window whose number is at or below the watermark was
//! applied once and must never run again as a new command.

use rusqlite::{Connection, Transaction, params};

/// Install the watermark owner without changing existing rows or physical DDL.
pub(crate) fn prepare(connection: &Connection) -> rusqlite::Result<()> {
    use studio_db_api::Table;
    crate::settings_schema::ClientCommandWatermarks::descriptor().install_if_absent(connection)
}

pub struct CommandWatermarks;

/// Durable admission for a send, saved before its asset upload.
pub(crate) fn prepare_send_admissions(connection: &Connection) -> rusqlite::Result<()> {
    use studio_db_api::Table;
    crate::agent_tables::SendAdmissionsStorageRow::descriptor().install_if_absent(connection)
}

impl CommandWatermarks {
    pub fn install_send_admissions(tx: &Transaction<'_>, now_ms: i64) -> rusqlite::Result<()> {
        prepare_send_admissions(tx)?;
        tx.execute("INSERT INTO runtime_orchestration_settings(id,record) VALUES('global',json_object('sendAdmissionCutoverMs',?1)) ON CONFLICT(id) DO UPDATE SET record=json_insert(record,'$.sendAdmissionCutoverMs',?1)", [now_ms])?;
        Ok(())
    }
    /// Migration time in Unix milliseconds; stored once so restarts keep the boundary.
    pub fn send_cutover(conn: &Connection) -> rusqlite::Result<i64> {
        conn.query_row("SELECT json_extract(record,'$.sendAdmissionCutoverMs') FROM runtime_orchestration_settings WHERE id='global'", [], |row| row.get(0))
    }
    /// Reserve the original fingerprint for the lifetime of the replay window.
    pub fn reserve_send(
        tx: &Transaction<'_>,
        id: &str,
        signature: &str,
        created: i64,
        now: i64,
    ) -> rusqlite::Result<bool> {
        let cutoff = now.saturating_sub(studio_runtime_schema::persistence::REQUEST_RESULT_TTL_MS);
        tx.execute("DELETE FROM command_send_admissions WHERE id IN (SELECT id FROM command_send_admissions WHERE created<=?1 ORDER BY created,id LIMIT ?2)", params![cutoff, crate::request_results::REQUEST_RESULT_GC_BATCH_ROWS as i64])?;
        tx.execute(
            "INSERT OR IGNORE INTO command_send_admissions(id,signature,created) VALUES(?1,?2,?3)",
            params![id, signature, created],
        )?;
        tx.query_row(
            "SELECT signature=?2 FROM command_send_admissions WHERE id=?1",
            params![id, signature],
            |row| row.get(0),
        )
    }
    /// The highest applied number of `client`, or 0 when it never applied a command.
    pub fn applied(conn: &Connection, client: &str) -> rusqlite::Result<i64> {
        Ok(crate::peer_update_settings::canvas_read(conn)
            .get::<crate::settings_schema::ClientCommandWatermarks>(&(client.to_owned(),))?
            .map_or(0, |row| row.applied))
    }

    /// Revision of an event: 1 until the first change, then one more for each change of its
    /// status or text (side table `runtime_event_revisions`, kept by triggers).
    pub fn event_revision(conn: &Connection, event: &str) -> rusqlite::Result<i64> {
        conn.query_row(
            "SELECT COALESCE((SELECT rev FROM runtime_event_revisions WHERE id=?1),1)",
            [event],
            |row| row.get(0),
        )
    }

    /// Raise the watermark to `number`. It never moves down.
    pub fn advance(tx: &Transaction<'_>, client: &str, number: i64) -> rusqlite::Result<()> {
        // Raw SQL: raise the existing watermark with an atomic MAX expression.
        tx.execute(
            "INSERT INTO client_command_watermarks(client,applied) VALUES(?1,?2)
             ON CONFLICT(client) DO UPDATE SET applied=max(applied,excluded.applied)",
            params![client, number],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_cutover_is_recorded_once() {
        let mut db = Connection::open_in_memory().expect("database");
        db.execute_batch(
            "CREATE TABLE runtime_orchestration_settings(id TEXT PRIMARY KEY,record TEXT NOT NULL)",
        )
        .expect("settings");
        let tx = db.transaction().expect("transaction");
        CommandWatermarks::install_send_admissions(&tx, 123).expect("first migration");
        CommandWatermarks::install_send_admissions(&tx, 456).expect("repeated migration");
        assert_eq!(CommandWatermarks::send_cutover(&tx).expect("cutover"), 123);
    }

    #[test]
    fn watermark_starts_at_zero_and_never_moves_down() {
        let mut db = Connection::open_in_memory().expect("database");
        prepare(&db).expect("schema");
        assert_eq!(CommandWatermarks::applied(&db, "a").unwrap(), 0);
        let tx = db.transaction().unwrap();
        CommandWatermarks::advance(&tx, "a", 5).unwrap();
        CommandWatermarks::advance(&tx, "a", 3).unwrap();
        CommandWatermarks::advance(&tx, "b", 1).unwrap();
        tx.commit().unwrap();
        assert_eq!(CommandWatermarks::applied(&db, "a").unwrap(), 5);
        assert_eq!(CommandWatermarks::applied(&db, "b").unwrap(), 1);
    }
}
