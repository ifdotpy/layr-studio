//! Persisted native voice records, sessions, deliveries and approval inputs.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use studio_json::Value;

use crate::json_record_codec::ExactJson;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use crate::schema::{SchemaRequirements, TableRequirement};
use crate::voice_storage::{VoiceDeliveryRow, VoiceRecordRow, VoiceSpeechRequestRow};
use crate::{StoreError, StoreResult};
use studio_db_api::db_delete;

/// One durable provider stop effect for one captured voice session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceEffect {
    pub session_id: String,
    pub agent: String,
    pub epoch: i64,
    pub account: String,
    pub thread: String,
    pub disposition: VoiceEffectDisposition,
    pub authority: VoiceEffectAuthority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum VoiceEffectDisposition {
    Active,
    Deleted { epoch: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum VoiceEffectAuthority {
    Admitted {
        scope: studio_runtime_schema::persistence::ProviderScope,
        root_request_id: String,
        owner: crate::request_results::AdmittedRequest,
        admission: crate::request_results::AdmittedRequest,
        outcome: studio_runtime_schema::persistence::VoiceStopOutcome,
    },
    Held {
        scope: studio_runtime_schema::persistence::CapturedScope,
    },
}

const BOOTSTRAP_TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: "voice_approvals",
        columns: &["id", "agent", "speech", "transcript", "status", "result"],
    },
    TableRequirement {
        name: "voice_deliveries",
        columns: &["id", "agent", "records", "text", "edited_text"],
    },
    TableRequirement {
        name: "voice_records",
        columns: &[
            "seq",
            "id",
            "agent",
            "session",
            "kind",
            "text",
            "item_id",
            "previous_item_id",
            "payload",
            "created",
        ],
    },
    TableRequirement {
        name: "voice_sessions",
        columns: &[
            "id", "agent", "created", "ended", "answer", "state", "error",
        ],
    },
    TableRequirement {
        name: "voice_speech_requests",
        columns: &["id", "session", "record_id", "state", "error"],
    },
];
pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: BOOTSTRAP_TABLES,
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VoiceRecord {
    #[serde(rename = "seq")]
    pub sequence: i64,
    pub id: String,
    pub agent: String,
    pub session: String,
    pub kind: String,
    pub text: String,
    pub item_id: String,
    pub previous_item_id: String,
    pub payload: String,
    pub created: f64,
}

impl VoiceRecord {
    pub fn value(&self) -> StoreResult<Value> {
        Ok(studio_json::to_value(self)?)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceDelivery {
    pub id: String,
    pub agent: String,
    pub records: String,
    pub text: String,
    pub edited_text: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceRepository;

impl VoiceRepository {
    /// Delete saved voice state in the same transaction as its owning agent.
    pub fn delete_agent(transaction: &rusqlite::Transaction<'_>, agent: &str) -> StoreResult<()> {
        let agent = agent.to_owned();
        let mut tx = canvas_tx(transaction);
        db_delete!(tx, VoiceRecordRow, filter(VoiceRecordRow::AGENT.eq(&agent)))?;
        db_delete!(
            tx,
            VoiceDeliveryRow,
            filter(VoiceDeliveryRow::AGENT.eq(&agent))
        )?;
        // Session subquery preserves receipt deletion ordering.
        transaction.execute("DELETE FROM voice_speech_requests WHERE session IN (SELECT id FROM voice_sessions WHERE agent=?1)", [&agent])?;
        db_delete!(
            tx,
            crate::voice_storage::VoiceSessionRow,
            filter(crate::voice_storage::VoiceSessionRow::AGENT.eq(&agent))
        )?;
        db_delete!(
            tx,
            crate::voice_storage::VoiceApprovalRow,
            filter(crate::voice_storage::VoiceApprovalRow::AGENT.eq(&agent))
        )?;
        Ok(())
    }

    pub fn record(connection: &Connection, id: &str) -> StoreResult<Option<VoiceRecord>> {
        Ok(connection
            .query_row(
                // Projection API follow-up: typed decoding would newly reject historical payloads.
                "SELECT seq,id,agent,session,kind,text,item_id,previous_item_id,payload,created \
                 FROM voice_records WHERE id=?1",
                [id],
                record_from_row,
            )
            .optional()?)
    }

    pub fn records(
        connection: &Connection,
        agent: &str,
        after: i64,
    ) -> StoreResult<Vec<VoiceRecord>> {
        Self::records_page(connection, agent, after, i64::MAX)
    }

    /// Bound transient stream reads; HTTP recovery retains the full history path.
    pub fn records_page(
        connection: &Connection,
        agent: &str,
        after: i64,
        limit: i64,
    ) -> StoreResult<Vec<VoiceRecord>> {
        let mut statement = connection.prepare_cached(
            // Projection API follow-up: typed decoding would newly reject historical payloads.
            "SELECT seq,id,agent,session,kind,text,item_id,previous_item_id,payload,created \
             FROM voice_records WHERE agent=?1 AND seq>?2 ORDER BY seq LIMIT ?3",
        )?;
        let rows = statement.query_map(params![agent, after, limit], record_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn record_sequence(connection: &Connection, agent: &str) -> StoreResult<i64> {
        Ok(connection.query_row(
            // Aggregate lookup has no generated scalar query.
            "SELECT COALESCE(MAX(seq),0) FROM voice_records WHERE agent=?1",
            [agent],
            |row| row.get(0),
        )?)
    }

    /// Stored voice cursor and session columns for subscription change detection.
    pub fn fingerprint(connection: &Connection, agent: &str) -> StoreResult<Vec<u8>> {
        let sequence = Self::record_sequence(connection, agent)?;
        // Scalar projection avoids decoding unrelated stored columns.
        let session = connection.query_row("SELECT id,state,answer,error,ended FROM voice_sessions WHERE agent=?1 AND state IS NOT NULL ORDER BY created DESC LIMIT 1", [agent], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<f64>>(4)?))).optional()?;
        studio_json::to_vec(&(sequence, session)).map_err(StoreError::from)
    }

    pub fn latest_session(connection: &Connection, agent: &str) -> StoreResult<Option<Value>> {
        connection
            .query_row(
                // Scalar projection avoids decoding unrelated stored columns.
                "SELECT id,state,answer,error,ended FROM voice_sessions \
                 WHERE agent=?1 AND state IS NOT NULL ORDER BY created DESC LIMIT 1",
                [agent],
                |row| {
                    Ok(studio_json::json!({
                        "session_id": row.get::<_, String>(0)?,
                        "state": row.get::<_, Option<String>>(1)?,
                        "sdp": row.get::<_, Option<String>>(2)?,
                        "error": row.get::<_, Option<String>>(3)?,
                        "ended": row.get::<_, Option<f64>>(4)?,
                    }))
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn session(connection: &Connection, id: &str, agent: &str) -> StoreResult<Option<Value>> {
        connection
            .query_row(
                // Scalar projection avoids decoding unrelated stored columns.
                "SELECT id,state,answer,error,ended FROM voice_sessions WHERE id=?1 AND agent=?2",
                params![id, agent],
                |row| {
                    Ok(studio_json::json!({
                        "session_id": row.get::<_, String>(0)?,
                        "state": row.get::<_, Option<String>>(1)?,
                        "sdp": row.get::<_, Option<String>>(2)?,
                        "error": row.get::<_, Option<String>>(3)?,
                        "ended": row.get::<_, Option<f64>>(4)?,
                    }))
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn speech_receipt(connection: &Connection, id: &str) -> StoreResult<Option<Value>> {
        Ok(canvas_read(connection)
            .get::<VoiceSpeechRequestRow>(&(id.to_owned(),))?
            .map(|row| studio_json::json!({"id":row.id,"session":row.session,"record_id":row.record_id,"state":row.state,"error":row.error})))
    }

    pub fn delivered(connection: &Connection, agent: &str) -> StoreResult<Vec<(String, String)>> {
        let mut statement = connection.prepare_cached(
            // Scalar projection avoids decoding unrelated stored columns.
            "SELECT id,records FROM voice_deliveries WHERE agent=?1 \
             AND id IN (SELECT id FROM runtime_events)",
        )?;
        let rows = statement.query_map([agent], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn insert_record(
        transaction: &rusqlite::Transaction<'_>,
        record: &VoiceRecord,
    ) -> StoreResult<()> {
        canvas_tx(transaction).insert(&VoiceRecordRow {
            seq: record.sequence,
            id: record.id.clone(),
            agent: record.agent.clone(),
            session: record.session.clone(),
            kind: record.kind.clone(),
            text: record.text.clone(),
            item_id: record.item_id.clone(),
            previous_item_id: record.previous_item_id.clone(),
            payload: ExactJson::from_stored(record.payload.clone())?,
            created: record.created,
        })?;
        Ok(())
    }

    pub fn session_exists(
        connection: &Connection,
        agent: &str,
        session: &str,
    ) -> StoreResult<bool> {
        connection
            .query_row(
                // Scalar projection avoids decoding unrelated stored columns.
                "SELECT 1 FROM voice_sessions WHERE id=?1 AND agent=?2",
                params![session, agent],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(StoreError::from)
    }

    pub fn delivery(connection: &Connection, id: &str) -> StoreResult<Option<VoiceDelivery>> {
        connection
            .query_row(
                // Projection API follow-up: typed decoding would newly reject historical record arrays.
                "SELECT id,agent,records,text,edited_text FROM voice_deliveries WHERE id=?1",
                [id],
                |row| {
                    Ok(VoiceDelivery {
                        id: row.get(0)?,
                        agent: row.get(1)?,
                        records: row.get(2)?,
                        text: row.get(3)?,
                        edited_text: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn deliveries(connection: &Connection, agent: &str) -> StoreResult<Vec<(String, String)>> {
        let mut statement =
            // Scalar projection avoids decoding unrelated stored columns.
            connection.prepare_cached("SELECT id,records FROM voice_deliveries WHERE agent=?1")?;
        let rows = statement.query_map([agent], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn insert_delivery(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
        agent: &str,
        records: &str,
        text: &str,
        edited_text: Option<&str>,
    ) -> StoreResult<()> {
        canvas_tx(transaction).insert(&VoiceDeliveryRow {
            id: id.to_owned(),
            agent: agent.to_owned(),
            records: ExactJson::from_stored(records.to_owned())?,
            text: text.to_owned(),
            edited_text: edited_text.map(str::to_owned),
        })?;
        Ok(())
    }
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VoiceRecord> {
    Ok(VoiceRecord {
        sequence: row.get(0)?,
        id: row.get(1)?,
        agent: row.get(2)?,
        session: row.get(3)?,
        kind: row.get(4)?,
        text: row.get(5)?,
        item_id: row.get(6)?,
        previous_item_id: row.get(7)?,
        payload: row.get(8)?,
        created: row.get(9)?,
    })
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "voice",
        requirements: &REQUIREMENTS,
    }
}
