//! Narrow storage effects for federation; the caller owns the transaction.
use crate::schema::{SchemaRequirements, TableRequirement};
use crate::{StoreError, StoreResult};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use studio_json::Value;
use studio_runtime_schema::persistence::OutboxStatus;

use super::remote_outbox::{RemoteEffect, RemoteOutbox};
use crate::consolidation_kinds::CanvasMetadataKind;
use crate::metadata_schema::{FederationSettings, Metadata};
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};

const MAX_RECORD_BYTES: usize = 512 * 1024;

#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=Option<String>,
    tables(rows=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,record=Metadata::FEDERATION_RECORD),
    sql="SELECT CASE WHEN length(CAST({record} AS BLOB))<=?3 THEN {record} ELSE NULL END FROM {rows} WHERE {kind}=?1 AND {identity}=?2")]
struct SettingsRecord {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=Metadata::SOURCE_ROWID)]
    max_bytes: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
pub enum FederationTable {
    Settings,
    Peer,
    Room,
    Outbox,
    Inbox,
}
impl FederationTable {
    pub fn name(self) -> &'static str {
        match self {
            Self::Settings => "runtime_federation_settings",
            Self::Peer => "runtime_federation_peers",
            Self::Room => "runtime_federation_rooms",
            Self::Outbox => "runtime_federation_outbox",
            Self::Inbox => "runtime_federation_inbox",
        }
    }
    pub fn schema(self) -> &'static str {
        match self {
            Self::Settings => "PersistedFederationSettings",
            Self::Peer => "PersistedFederationPeer",
            Self::Room => "PersistedFederationRoom",
            Self::Outbox => "PersistedFederationOutbox",
            Self::Inbox => "PersistedFederationInbox",
        }
    }
    pub fn validate(self, id: &str, value: &Value) -> StoreResult<()> {
        let valid = studio_runtime_schema::validate(self.schema(), value)
            .map_err(StoreError::SchemaValidation)?;
        let identity = match self {
            Self::Peer => Some("stateId"),
            Self::Room | Self::Outbox => Some("id"),
            Self::Settings | Self::Inbox => None,
        };
        if !valid
            || identity.is_some_and(|field| value.get(field).and_then(Value::as_str) != Some(id))
        {
            return Err(StoreError::InvalidStoredObject {
                table: self.name(),
                id: id.to_owned(),
            });
        }
        Ok(())
    }
}

pub struct FederationMessagingRepository;
impl FederationMessagingRepository {
    pub fn record(
        db: &Connection,
        table: FederationTable,
        id: &str,
        peer: &str,
    ) -> StoreResult<Option<Value>> {
        let sql = match table {
            FederationTable::Inbox => {
                // Raw SQL: legacy record admission needs a typed-read equivalence proof.
                "SELECT record FROM runtime_federation_inbox WHERE message_id=?1 AND peer_id=?2"
                    .to_owned()
            }
            FederationTable::Outbox => return Self::outbox_record(db, id, peer),
            FederationTable::Settings => {
                return canvas_read(db)
                    .named(
                        &SettingsRecord::QUERY,
                        &SettingsRecord {
                            kind: CanvasMetadataKind::FederationSettings,
                            identity: crate::consolidation_codec::nullable_text_identity(
                                &Some(id.to_owned()),
                                0,
                            ),
                            max_bytes: Some(MAX_RECORD_BYTES as i64),
                        },
                    )?
                    .map(|raw| {
                        Self::parse(
                            table,
                            id,
                            &raw.ok_or_else(|| StoreError::InvalidStoredObject {
                                table: table.name(),
                                id: id.into(),
                            })?,
                        )
                    })
                    .transpose();
            }
            FederationTable::Peer | FederationTable::Room => {
                format!("SELECT record FROM {} WHERE id=?1 AND ?2=?2", table.name())
            }
        };
        let sql=sql.replacen("SELECT record", &format!("SELECT CASE WHEN length(CAST(record AS BLOB))<={MAX_RECORD_BYTES} THEN record ELSE NULL END"),1);
        let raw: Option<Option<String>> = db
            .query_row(&sql, params![id, peer], |r| r.get(0))
            .optional()?;
        raw.map(|raw| {
            Self::parse(
                table,
                id,
                &raw.ok_or_else(|| StoreError::InvalidStoredObject {
                    table: table.name(),
                    id: id.into(),
                })?,
            )
        })
        .transpose()
    }
    /// The record of a federation message in the shared outbox.
    fn outbox_record(db: &Connection, id: &str, peer: &str) -> StoreResult<Option<Value>> {
        let key = RemoteOutbox::id(RemoteEffect::FederationMessage, peer, id);
        let raw: Option<Option<String>> = db
            .query_row(
                "SELECT CASE WHEN length(CAST(json_extract(payload,'$.record') AS BLOB))<=?2 THEN json_extract(payload,'$.record') END FROM runtime_outbox WHERE id=?1",
                params![key, MAX_RECORD_BYTES as i64],
                |r| r.get(0),
            )
            .optional()?;
        raw.map(|raw| {
            Self::parse(
                FederationTable::Outbox,
                id,
                &raw.ok_or_else(|| StoreError::InvalidStoredObject {
                    table: FederationTable::Outbox.name(),
                    id: id.into(),
                })?,
            )
        })
        .transpose()
    }
    pub fn parse(table: FederationTable, id: &str, raw: &str) -> StoreResult<Value> {
        if raw.len() > MAX_RECORD_BYTES {
            return Err(StoreError::InvalidStoredObject {
                table: table.name(),
                id: id.into(),
            });
        }
        let value = studio_json::from_str(raw).map_err(|_| StoreError::InvalidStoredObject {
            table: table.name(),
            id: id.into(),
        })?;
        table.validate(id, &value)?;
        Ok(value)
    }
    pub fn put(
        tx: &Transaction<'_>,
        table: FederationTable,
        id: &str,
        peer: &str,
        value: &Value,
    ) -> StoreResult<()> {
        table.validate(id, value)?;
        let raw = studio_json::to_string(value)?;
        if raw.len() > MAX_RECORD_BYTES {
            return Err(StoreError::InvalidStoredObject {
                table: table.name(),
                id: id.into(),
            });
        }
        match table {
            FederationTable::Inbox => {
                use crate::access_tables::FederationInboxRow;
                let record = crate::json_record_codec::ExactJson::<
                    crate::access_payloads::FederationInboxPayload,
                >::from_stored(raw)?;
                let receipt_ack = record.value().fields.ack.as_str().to_owned();
                let receipt_kind = record
                    .value()
                    .fields
                    .kind
                    .value()
                    .map(|kind| kind.as_str().to_owned());
                crate::peer_update_settings::canvas_tx(tx).upsert(&FederationInboxRow {
                    peer_id: peer.into(),
                    message_id: id.into(),
                    record,
                    receipt_ack,
                    receipt_kind,
                })?;
            }
            FederationTable::Outbox => put_outbox(tx, id, peer, value)?,
            FederationTable::Peer => {
                crate::peer_update_settings::canvas_tx(tx).upsert(
                    &crate::access_tables::FederationPeersRow {
                        id: id.into(),
                        record: crate::json_record_codec::ExactJson::from_stored(raw)?,
                    },
                )?;
            }
            FederationTable::Room => {
                crate::peer_update_settings::canvas_tx(tx).upsert(
                    &crate::access_tables::FederationRoomsRow {
                        id: id.into(),
                        record: crate::json_record_codec::ExactJson::from_stored(raw)?,
                    },
                )?;
            }
            FederationTable::Settings => {
                let kind = CanvasMetadataKind::FederationSettings;
                let text_key = Some(id.to_owned());
                let identity = crate::consolidation_codec::nullable_text_identity(&text_key, 0);
                let source_rowid = match studio_db_api::db_query!(
                    canvas_read(tx),
                    Metadata,
                    scalar(Metadata::SOURCE_ROWID),
                    filter(Metadata::KIND.eq(&kind), Metadata::IDENTITY.eq(&identity))
                )? {
                    Some(rowid) => rowid.ok_or(rusqlite::Error::InvalidColumnType(
                        0,
                        "source_rowid".into(),
                        rusqlite::types::Type::Null,
                    ))?,
                    None => crate::clock_schema::next_metadata_rowid(tx, kind)?,
                };
                let before = tx.total_changes();
                canvas_tx(tx)
                    .partition::<FederationSettings>()
                    .put(&FederationSettings {
                        identity,
                        source_rowid,
                        text_key,
                        federation_record: crate::json_record_codec::ExactJson::from_stored(raw)?,
                    })?;
                if tx.total_changes() != before {
                    crate::record_settings_change(kind);
                }
            }
        }
        Ok(())
    }
    pub fn batch(
        db: &Connection,
        table: FederationTable,
        peer: &str,
        now: f64,
    ) -> StoreResult<Vec<(String, Value)>> {
        let sql = match table {
            FederationTable::Outbox => {
                "SELECT request_id,CASE WHEN length(CAST(json_extract(payload,'$.record') AS BLOB))<=?3 THEN json_extract(payload,'$.record') END FROM runtime_outbox WHERE kind='remote_delivery' AND json_extract(payload,'$.effect')='federation_message' AND target=?1 AND json_extract(payload,'$.record.status')='queued' AND json_extract(payload,'$.record.nextAt')<=?2 ORDER BY json_extract(payload,'$.record.created'),request_id LIMIT 50"
            }
            FederationTable::Inbox => {
                // Raw SQL: JSON selectors/projections are outside db_query.
                "SELECT message_id,CASE WHEN length(CAST(record AS BLOB))<=?3 THEN record END FROM runtime_federation_inbox WHERE peer_id=?1 AND json_extract(record,'$.ack') IN ('pending','sent') AND ?2=?2 ORDER BY json_extract(record,'$.created'),message_id LIMIT 50"
            }
            FederationTable::Settings | FederationTable::Peer | FederationTable::Room => {
                return Err(StoreError::RecordContract("federation batch table"));
            }
        };
        let mut stmt = db.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params![peer, now, MAX_RECORD_BYTES as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, raw)| {
                let raw = raw.ok_or_else(|| StoreError::InvalidStoredObject {
                    table: table.name(),
                    id: id.clone(),
                })?;
                Ok((id.clone(), Self::parse(table, &id, &raw)?))
            })
            .collect()
    }
    pub fn room_rate(db: &Connection, peer: &str, room: &str, now: f64) -> StoreResult<u64> {
        // Raw SQL: JSON selectors/projections are outside db_query.
        Ok(db.query_row("SELECT count(*) FROM runtime_federation_inbox WHERE peer_id=?1 AND json_extract(record,'$.room')=?2 AND json_extract(record,'$.kind')='chat-message' AND json_extract(record,'$.created')>=?3",params![peer,room,now-60.0],|r|r.get::<_,u32>(0))? as u64)
    }
    pub fn queue_size(db: &Connection, peer: &str) -> StoreResult<(u64, u64)> {
        queued_outbox_size(db, peer)
    }
}

/// Store a federation message in the shared outbox. The signature is that of the saved
/// admission, so an update of the delivery state keeps one identity.
pub(crate) fn put_outbox(
    tx: &Transaction<'_>,
    id: &str,
    peer: &str,
    record: &Value,
) -> StoreResult<()> {
    let signature = record
        .pointer("/admission/signature")
        .and_then(Value::as_str)
        .ok_or(StoreError::RecordContract("federation outbox admission"))?;
    let created = record.get("created").and_then(Value::as_f64).unwrap_or(0.0);
    let (status, next_at) = if record.get("status").and_then(Value::as_str) == Some("queued") {
        let due = record.get("nextAt").and_then(Value::as_f64).unwrap_or(0.0);
        (OutboxStatus::Pending, Some((due * 1000.0).round() as i64))
    } else {
        (OutboxStatus::Committed, None)
    };
    RemoteOutbox::upsert(
        tx,
        RemoteEffect::FederationMessage,
        peer,
        id,
        signature,
        status,
        next_at,
        record,
        (created * 1000.0).round() as i64,
    )?;
    Ok(())
}

pub(crate) fn queued_outbox_size(db: &Connection, peer: &str) -> StoreResult<(u64, u64)> {
    let (count, bytes): (i64, i64) = db.query_row(
        "SELECT count(*),COALESCE(sum(length(CAST(json_extract(payload,'$.record') AS BLOB))),0) FROM runtime_outbox WHERE kind='remote_delivery' AND json_extract(payload,'$.effect')='federation_message' AND target=?1 AND json_extract(payload,'$.record.status')='queued'",
        [peer],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((
        u64::try_from(count).map_err(|_| StoreError::RecordContract("federation queue size"))?,
        u64::try_from(bytes).map_err(|_| StoreError::RecordContract("federation queue size"))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::FederationMessagingRepository;
    use rusqlite::Connection;

    #[test]
    fn federation_record_write_uses_native_json() {
        let mut db = crate::test_canvas();
        let record = studio_json::json!({"enabled":true,"number":1e-6});
        let tx = db.transaction().unwrap();
        FederationMessagingRepository::put(
            &tx,
            super::FederationTable::Settings,
            "settings",
            "",
            &record,
        )
        .unwrap();
        let raw = crate::peer_update_settings::canvas_read(&tx)
            .get::<crate::metadata_schema::FederationSettings>(&(
                crate::consolidation_codec::nullable_text_identity(&Some("settings".to_owned()), 0),
            ))
            .unwrap()
            .unwrap()
            .federation_record
            .raw()
            .to_owned();
        assert_eq!(raw, record.to_string());
        assert_eq!(
            FederationMessagingRepository::record(
                &tx,
                super::FederationTable::Settings,
                "settings",
                ""
            )
            .unwrap(),
            Some(record)
        );
    }

    #[test]
    fn queued_outbox_size_counts_utf8_bytes() {
        let mut db = Connection::open_in_memory().expect("open in-memory database");
        crate::receipt_schema::create(&db).expect("shared outbox");
        let queued = studio_json::json!({"status":"queued","text":"é😀","nextAt":0,"created":1,"admission":{"id":"a","signature":"s1","created":1}});
        let delivered = studio_json::json!({"status":"delivered","admission":{"id":"b","signature":"s2","created":1}});
        let tx = db.transaction().expect("transaction");
        super::put_outbox(&tx, "m1", "peer", &queued).expect("insert queued record");
        super::put_outbox(&tx, "m2", "peer", &delivered).expect("insert delivered record");
        tx.commit().expect("commit");
        let queued = queued_text(&db);
        assert_eq!(
            FederationMessagingRepository::queue_size(&db, "peer")
                .expect("queued outbox size is readable"),
            (1, queued.len() as u64),
        );
        assert!(queued.len() > queued.chars().count());
    }

    /// The stored record text, as the size query measures it.
    fn queued_text(db: &Connection) -> String {
        db.query_row(
            "SELECT json_extract(payload,'$.record') FROM runtime_outbox WHERE request_id='m1'",
            [],
            |row| row.get(0),
        )
        .expect("stored record")
    }
}

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[TableRequirement {
        name: "runtime_federation_inbox",
        columns: &["peer_id", "message_id", "record"],
    }],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};
inventory::submit! { crate::repositories::SchemaRegistration { repository:"federation_messaging",requirements:&REQUIREMENTS } }

/// Read-only coverage for every federation record consumed by this subsystem.
pub fn check_state(db: &Connection) -> StoreResult<Vec<crate::StateCheckTable>> {
    [FederationTable::Outbox]
        .into_iter()
        .map(|table| check_table_state(db, table))
        .collect()
}

/// Preserve the legacy parser and diagnostics for a single table's registry report.
pub(crate) fn check_table_state(
    db: &Connection,
    table: FederationTable,
) -> StoreResult<crate::StateCheckTable> {
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table.name()],
        |r| r.get(0),
    )?;
    let mut row_count = 0;
    let mut rejected_count = 0;
    if exists {
        let key = if matches!(table, FederationTable::Inbox) {
            "message_id"
        } else {
            "id"
        };
        let mut stmt = db.prepare(&format!("SELECT {key},record FROM {}", table.name()))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            row_count += 1;
            let id: String = row.get(0)?;
            let raw: String = row.get(1)?;
            if FederationMessagingRepository::parse(table, &id, &raw).is_err() {
                rejected_count += 1;
            }
        }
    }
    Ok(crate::StateCheckTable {
        table: table.name(),
        row_count,
        rejected_count,
        failures: if rejected_count == 0 {
            vec![]
        } else {
            vec![crate::StateCheckFailure {
                pointer: "/".into(),
                keyword: "schema".into(),
                count: rejected_count,
            }]
        },
    })
}
