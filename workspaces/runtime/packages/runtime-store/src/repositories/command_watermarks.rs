//! Per-client command watermark: the highest applied command number of each client.
//!
//! A client is a device, an agent, or a peer. It numbers its commands from 1. A command
//! older than the `request_results` window whose number is at or below the watermark was
//! applied once and must never run again as a new command.

use crate::consolidation_kinds::CanvasMetadataKind;
use crate::metadata_schema::Metadata;
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use rusqlite::{Connection, Transaction, params};
use studio_db_api::DbQuery;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,client=Metadata::TEXT_KEY,applied=Metadata::APPLIED),
 sql="INSERT INTO {metadata}({kind},{identity},{client},{applied}) VALUES(?1,?2,?3,?4) ON CONFLICT({kind},{identity}) DO UPDATE SET {applied}=max({applied},excluded.{applied})")]
struct AdvanceWatermark {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=Metadata::TEXT_KEY)]
    client: Option<String>,
    #[db(column=Metadata::APPLIED)]
    applied: Option<i64>,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,key=Metadata::TEXT_KEY,rowid=Metadata::SOURCE_ROWID,record=Metadata::ORCHESTRATION_RECORD),
 sql="INSERT INTO {metadata}({kind},{identity},{key},{rowid},{record}) VALUES(?1,?2,'global',0,json_object('sendAdmissionCutoverMs',?3)) ON CONFLICT({kind},{identity}) DO UPDATE SET {record}=json_insert({record},'$.sendAdmissionCutoverMs',?3)")]
struct InstallSendCutover {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=crate::settings_schema::ClientCommandWatermarks::APPLIED)]
    now_ms: i64,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=crate::settings_schema::ClientCommandWatermarks::APPLIED,
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,record=Metadata::ORCHESTRATION_RECORD,value=crate::settings_schema::ClientCommandWatermarks::APPLIED),
 sql="SELECT json_extract({record},'$.sendAdmissionCutoverMs') AS {value} FROM {metadata} WHERE {kind}=?1 AND {identity}=?2")]
struct SendCutover {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
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
        canvas_tx(tx).named(
            &InstallSendCutover::QUERY,
            &InstallSendCutover {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(
                    &Some("global".into()),
                    0,
                ),
                now_ms,
            },
        )?;
        Ok(())
    }
    /// Migration time in Unix milliseconds; stored once so restarts keep the boundary.
    pub fn send_cutover(conn: &Connection) -> rusqlite::Result<i64> {
        canvas_read(conn).named(
            &SendCutover::QUERY,
            &SendCutover {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(
                    &Some("global".into()),
                    0,
                ),
            },
        )
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
            .get::<crate::metadata_schema::ClientCommandWatermarks>(&(
                studio_db_api::migration::encode_key(&(client.to_owned(),)),
            ))?
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
        canvas_tx(tx).named(
            &AdvanceWatermark::QUERY,
            &AdvanceWatermark {
                kind: CanvasMetadataKind::ClientCommandWatermarks,
                identity: studio_db_api::migration::encode_key(&(client.to_owned(),)),
                client: Some(client.into()),
                applied: Some(number),
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_cutover_preserves_sqlite_presence_types_and_first_duplicate() {
        use studio_db_api::Table;
        let mut db = Connection::open_in_memory().unwrap();
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        assert!(matches!(
            CommandWatermarks::send_cutover(&db),
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
        for (raw, expected) in [
            ("{}", Some(99)),
            (r#"{"sendAdmissionCutoverMs":null}"#, None),
            (r#"{"sendAdmissionCutoverMs":"7"}"#, None),
            (r#"{"sendAdmissionCutoverMs":1.5}"#, None),
            (r#"{"sendAdmissionCutoverMs":true}"#, Some(1)),
            (
                r#"{"sendAdmissionCutoverMs":7,"sendAdmissionCutoverMs":9}"#,
                Some(7),
            ),
        ] {
            let tx = db.transaction().unwrap();
            crate::peer_update_settings::canvas_tx(&tx)
                .partition::<crate::metadata_schema::OrchestrationSettings>()
                .put(&crate::metadata_schema::OrchestrationSettings {
                    identity: crate::consolidation_codec::nullable_text_identity(
                        &Some("global".into()),
                        0,
                    ),
                    source_rowid: 0,
                    text_key: Some("global".into()),
                    orchestration_record:
                        crate::metadata_values::OrchestrationSettingsEvidence::from_stored(
                            raw.into(),
                        )
                        .unwrap(),
                })
                .unwrap();
            CommandWatermarks::install_send_admissions(&tx, 99).unwrap();
            assert_eq!(CommandWatermarks::send_cutover(&tx).ok(), expected, "{raw}");
            tx.rollback().unwrap();
        }
    }

    #[test]
    fn send_cutover_is_recorded_once() {
        let mut db = Connection::open_in_memory().expect("database");
        use studio_db_api::Table;
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .expect("settings");
        let tx = db.transaction().expect("transaction");
        CommandWatermarks::install_send_admissions(&tx, 123).expect("first migration");
        CommandWatermarks::install_send_admissions(&tx, 456).expect("repeated migration");
        assert_eq!(CommandWatermarks::send_cutover(&tx).expect("cutover"), 123);
    }

    #[test]
    fn watermark_starts_at_zero_and_never_moves_down() {
        let mut db = Connection::open_in_memory().expect("database");
        use studio_db_api::Table;
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .expect("schema");
        assert_eq!(CommandWatermarks::applied(&db, "a").unwrap(), 0);
        let tx = db.transaction().unwrap();
        CommandWatermarks::advance(&tx, "a", 5).unwrap();
        CommandWatermarks::advance(&tx, "a", 3).unwrap();
        CommandWatermarks::advance(&tx, "b", 1).unwrap();
        assert!(CommandWatermarks::advance(&tx, "a", -1).is_err());
        assert!(CommandWatermarks::advance(&tx, "new", -1).is_err());
        assert_eq!(CommandWatermarks::applied(&tx, "new").unwrap(), 0);
        tx.commit().unwrap();
        assert_eq!(CommandWatermarks::applied(&db, "a").unwrap(), 5);
        assert_eq!(CommandWatermarks::applied(&db, "b").unwrap(), 1);
    }
}
