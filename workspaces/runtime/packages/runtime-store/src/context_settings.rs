//! Wave 2 descriptors preserve the physical context and settings history format.
use crate::json_record_codec::{ExactJson, ExactJsonCodec, JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use crate::{ProfileRecord, StoreResult, TranscriptAssetRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_db_api::DbTable;
use studio_json::OwnedRawJson;

#[derive(DbTable)]
#[db(table = "runtime_assets", database = Canvas, legacy_ddl = "CREATE TABLE runtime_assets (id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_asset, state_check = crate::state_check::check_legacy_json::<TranscriptAssetRecord>)]
pub struct AssetsStorageRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<TranscriptAssetRecord>)]
    pub record: RawJsonRecord<TranscriptAssetRecord>,
}

#[derive(DbTable)]
#[db(table = "runtime_compactions", database = Canvas, legacy_ddl = "CREATE TABLE runtime_compactions (id TEXT PRIMARY KEY, agent TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<CompactionsStorageRow>)]
pub struct CompactionsStorageRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
}

#[derive(DbTable)]
#[db(table = "runtime_context_manifests", database = Canvas, legacy_ddl = "CREATE TABLE runtime_context_manifests (\n                  agent TEXT NOT NULL, thread_id TEXT NOT NULL, compactions INTEGER NOT NULL,\n                  sequence INTEGER NOT NULL, event_id TEXT NOT NULL, record TEXT NOT NULL,\n                  PRIMARY KEY(agent,thread_id,compactions))")]
#[db(
    legacy_index = "CREATE INDEX runtime_context_manifest_latest\n                  ON runtime_context_manifests(agent,sequence DESC)"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<ContextManifestsStorageRow>)]
pub struct ContextManifestsStorageRow {
    #[db(key)]
    pub agent: String,
    #[db(key)]
    pub thread_id: String,
    #[db(key)]
    pub compactions: i64,
    pub sequence: i64,
    pub event_id: String,
    #[db(codec = ExactJsonCodec<ContextManifest>)]
    pub record: ExactJson<ContextManifest>,
}

#[derive(DbTable)]
#[db(table = "runtime_context_reminders", database = Canvas, legacy_ddl = "CREATE TABLE runtime_context_reminders (\n                  agent TEXT NOT NULL, version TEXT NOT NULL, event_id TEXT NOT NULL,\n                  PRIMARY KEY(agent,version))")]
#[db(state_check = crate::state_check::count_legacy_rows::<ContextRemindersStorageRow>)]
pub struct ContextRemindersStorageRow {
    #[db(key)]
    pub agent: String,
    #[db(key)]
    pub version: String,
    pub event_id: String,
}

#[derive(DbTable)]
#[db(table = "runtime_model_status", database = Canvas, legacy_ddl = "CREATE TABLE runtime_model_status (agent TEXT, revision TEXT, at REAL, record TEXT, PRIMARY KEY(agent,revision))")]
#[db(state_check = crate::state_check::count_legacy_rows::<ModelStatusStorageRow>)]
pub struct ModelStatusStorageRow {
    #[db(key)]
    pub agent: String,
    #[db(key)]
    pub revision: String,
    pub at: Option<f64>,
    #[db(codec = ExactJsonCodec<ModelStatusRecordPayload>)]
    pub record: Option<ExactJson<ModelStatusRecordPayload>>,
}

#[derive(DbTable)]
#[db(table = "runtime_profiles", database = Canvas, legacy_ddl = "CREATE TABLE runtime_profiles (id TEXT PRIMARY KEY, record TEXT NOT NULL)", check = check_profile, state_check = crate::state_check::check_legacy_json::<ProfileRecord>)]
pub struct ProfilesStorageRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<ProfileRecord>)]
    pub record: RawJsonRecord<ProfileRecord>,
}

#[derive(DbTable)]
#[db(table = "runtime_transcript_revisions", database = Canvas, legacy_ddl = "CREATE TABLE runtime_transcript_revisions(\n                        agent TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 0)")]
#[db(state_check = crate::state_check::count_legacy_rows::<TranscriptRevisionsStorageRow>)]
pub struct TranscriptRevisionsStorageRow {
    #[db(key)]
    pub agent: String,
    pub revision: i64,
}

fn check_asset(row: &AssetsStorageRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}
fn check_profile(row: &ProfilesStorageRow) -> StoreResult<()> {
    row.record.check_key(&row.id)
}

/// The provider thread and compaction generation used by the submission compiler.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextEpoch(pub Option<String>, pub u64);

/// Accepted developer context versions, owned once for storage and compilation.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub epoch: ContextEpoch,
    pub sequence: u64,
    pub versions: BTreeMap<String, String>,
}

/// Status history is an array of the existing agent and monitor projections.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelStatusRecordPayload(pub Vec<ModelStatusRecord>);

/// Typed selectors plus original evidence, including extensions and member presence.
pub struct ModelStatusRecord {
    projection: StatusProjection,
    original: OwnedRawJson,
}
enum StatusProjection {
    Agent(AgentStatusProjection),
    Monitor(MonitorStatusProjection),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusProjection {
    pub id: String,
    #[serde(deserialize_with = "required_nullable")]
    pub name: Option<String>,
    pub status: String,
    #[serde(deserialize_with = "required_nullable")]
    pub in_flight: Option<bool>,
    #[serde(deserialize_with = "required_nullable")]
    pub parent_id: Option<String>,
    pub error: String,
    #[serde(default)]
    pub waits_for_event: studio_runtime_schema::queue_command::Field<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatusProjection {
    pub id: String,
    #[serde(deserialize_with = "required_nullable")]
    pub agent: Option<String>,
    pub status: String,
    #[serde(deserialize_with = "required_nullable")]
    pub exit_code: Option<StatusExitCode>,
    pub error: String,
}
impl<'de> Deserialize<'de> for ModelStatusRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        struct Kind {
            kind: String,
        }
        let original = OwnedRawJson::deserialize(deserializer)?;
        // The old Value projection chose the last duplicate member; keep its evidence untouched.
        let fields: BTreeMap<String, OwnedRawJson> =
            studio_json::from_str(original.as_str()).map_err(D::Error::custom)?;
        let selectors = studio_json::to_string(&fields).map_err(D::Error::custom)?;
        let kind: Kind = studio_json::from_str(&selectors).map_err(D::Error::custom)?;
        let projection = match kind.kind.as_str() {
            "agent" => StatusProjection::Agent(
                studio_json::from_str(&selectors).map_err(D::Error::custom)?,
            ),
            "monitor" => StatusProjection::Monitor(
                studio_json::from_str(&selectors).map_err(D::Error::custom)?,
            ),
            _ => return Err(D::Error::custom("unknown status kind")),
        };
        let record = Self {
            projection,
            original,
        };
        if record.key().is_none() {
            return Err(D::Error::custom("invalid status identity"));
        }
        Ok(record)
    }
}
impl Serialize for ModelStatusRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.original.serialize(serializer)
    }
}
impl ModelStatusRecord {
    pub fn raw(&self) -> &str {
        self.original.as_str()
    }
    pub fn key(&self) -> Option<String> {
        let (kind, id) = match &self.projection {
            StatusProjection::Agent(record) => {
                if matches!(
                    record.waits_for_event,
                    studio_runtime_schema::queue_command::Field::Null
                ) {
                    return None;
                }
                ("agent", &record.id)
            }
            StatusProjection::Monitor(record) => ("monitor", &record.id),
        };
        (!id.is_empty()).then(|| format!("{kind}:{id}"))
    }
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum StatusExitCode {
    Signed(i64),
    Unsigned(u64),
}
fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

pub fn ensure_context_tables(connection: &rusqlite::Connection) -> StoreResult<()> {
    use studio_db_api::Table;
    ContextManifestsStorageRow::descriptor().install_if_absent(connection)?;
    ContextRemindersStorageRow::descriptor().install_if_absent(connection)?;
    Ok(())
}

pub fn manifest(
    connection: &rusqlite::Connection,
    agent: &str,
    thread: &str,
    compactions: i64,
) -> StoreResult<Option<ContextManifest>> {
    Ok(crate::peer_update_settings::canvas_read(connection)
        .get::<ContextManifestsStorageRow>(&(agent.into(), thread.into(), compactions))?
        // clone: consuming accessor pending
        .map(|row| row.record.value().clone()))
}

pub fn manifests(
    connection: &rusqlite::Connection,
    agent: &str,
) -> StoreResult<Vec<ContextManifest>> {
    use studio_db_api::db_query;
    let agent = agent.to_owned();
    Ok(db_query!(
        crate::peer_update_settings::canvas_read(connection),
        ContextManifestsStorageRow,
        filter(ContextManifestsStorageRow::AGENT.eq(&agent)),
        order_by(ContextManifestsStorageRow::SEQUENCE.desc)
    )?
    .into_iter()
    .map(|row| row.record.value().clone())
    .collect())
}

pub fn reminders(
    connection: &rusqlite::Connection,
    agent: &str,
) -> StoreResult<std::collections::BTreeSet<String>> {
    use studio_db_api::db_query;
    let agent = agent.to_owned();
    Ok(db_query!(
        crate::peer_update_settings::canvas_read(connection),
        ContextRemindersStorageRow,
        filter(ContextRemindersStorageRow::AGENT.eq(&agent))
    )?
    .into_iter()
    .map(|row| row.version)
    .collect())
}

pub fn write_manifest(
    transaction: &rusqlite::Transaction<'_>,
    row: &ContextManifestsStorageRow,
) -> StoreResult<()> {
    // Conditional ON CONFLICT uses excluded.sequence, outside generated CRUD's grammar.
    transaction.execute("INSERT INTO runtime_context_manifests(agent,thread_id,compactions,sequence,event_id,record) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(agent,thread_id,compactions) DO UPDATE SET sequence=excluded.sequence,event_id=excluded.event_id,record=excluded.record WHERE excluded.sequence>=runtime_context_manifests.sequence", rusqlite::params![row.agent,row.thread_id,row.compactions,row.sequence,row.event_id,row.record.raw()])?;
    if let Some(version) = row.record.value().versions.get("worktreeReminder") {
        crate::peer_update_settings::canvas_tx(transaction).insert_new(
            &ContextRemindersStorageRow {
                agent: row.agent.clone(),
                version: version.clone(),
                event_id: row.event_id.clone(),
            },
        )?;
    }
    Ok(())
}

pub fn was_compacted(connection: &rusqlite::Connection, id: &str) -> StoreResult<bool> {
    Ok(crate::peer_update_settings::canvas_read(connection)
        .get::<CompactionsStorageRow>(&(id.into(),))?
        .is_some())
}
pub fn remember_compaction(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
    agent: &str,
) -> StoreResult<()> {
    crate::peer_update_settings::canvas_tx(transaction).insert_new(&CompactionsStorageRow {
        id: id.into(),
        agent: agent.into(),
    })?;
    Ok(())
}
pub fn put_model_status(
    transaction: &rusqlite::Transaction<'_>,
    agent: &str,
    revision: &str,
    at: f64,
    raw: String,
) -> StoreResult<()> {
    crate::peer_update_settings::canvas_tx(transaction).upsert(&ModelStatusStorageRow {
        agent: agent.into(),
        revision: revision.into(),
        at: Some(at),
        record: Some(ExactJson::from_stored(raw)?),
    })?;
    Ok(())
}
pub fn model_status(
    connection: &rusqlite::Connection,
    agent: &str,
    revision: &str,
) -> StoreResult<Option<String>> {
    let row = crate::peer_update_settings::canvas_read(connection)
        .get::<ModelStatusStorageRow>(&(agent.into(), revision.into()))
        .map_err(|error| {
            if matches!(error, rusqlite::Error::FromSqlConversionFailure(..)) {
                crate::StoreError::InvalidStoredRecord {
                    object: "runtime_model_status",
                    id: agent.into(),
                }
            } else {
                error.into()
            }
        })?;
    row.map(|row| {
        row.record.map(|record| record.into_raw()).ok_or_else(|| {
            crate::StoreError::InvalidStoredRecord {
                object: "runtime_model_status",
                id: agent.into(),
            }
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests;
