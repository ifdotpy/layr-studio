//! Named Analytics JSON owners; exact storage text belongs to ExactJson.
use serde::{Deserialize, Serialize};
use studio_runtime_schema::raw_json::RawJson;

// Analytics historically decodes through an object map: duplicate keys keep the last value.
// Normalize only the typed decoding input; ExactJson retains every original storage byte.
macro_rules! analytics_payload {
    ($name:ident, $fields:ty) => {
        #[derive(Serialize)]
        pub struct $name {
            #[serde(flatten)]
            pub fields: $fields,
            #[serde(flatten)]
            pub extensions: studio_runtime_schema::raw_json::RawExtensions,
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let members =
                    studio_runtime_schema::raw_json::RawExtensions::deserialize(deserializer)?;
                let encoded = studio_json::to_string(&members).map_err(serde::de::Error::custom)?;
                let normalized = crate::json_record_codec::last_json_members(
                    studio_json::OwnedRawJson::from_string(encoded)
                        .map_err(serde::de::Error::custom)?,
                )
                .map_err(serde::de::Error::custom)?;
                let (fields, extensions) =
                    studio_runtime_schema::raw_json::deserialize_with_raw_extensions(
                        &mut studio_json::Deserializer::from_str(normalized.as_str()),
                    )
                    .map_err(serde::de::Error::custom)?;
                let extensions = members
                    .into_iter()
                    .filter(|(key, _)| extensions.contains_key(key))
                    .collect();
                Ok(Self { fields, extensions })
            }
        }
    };
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsAgentRecordFields {
    pub account_history: Option<Vec<RawJson>>,
    pub account_key: Option<String>,
    pub effort: Option<String>,
    pub epoch: Option<studio_runtime_schema::StoredJsonInteger>,
    pub id: String,
    pub model: Option<String>,
    pub root_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}
analytics_payload!(AnalyticsAgentRecord, AnalyticsAgentRecordFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTurnRecordFields {
    pub agent_id: String,
    pub at: f64,
    pub finished_at: Option<f64>,
    pub first_output_at: Option<f64>,
    pub id: String,
    pub started_at: Option<f64>,
    pub status: String,
    pub terminal_source: Option<String>,
}
analytics_payload!(AnalyticsTurnRecord, AnalyticsTurnRecordFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSizeFields {
    pub bytes: studio_runtime_schema::StoredJsonInteger,
    pub chars: studio_runtime_schema::StoredJsonInteger,
    pub deltas: Option<studio_runtime_schema::StoredJsonInteger>,
    pub lines: studio_runtime_schema::StoredJsonInteger,
}
analytics_payload!(AnalyticsSize, AnalyticsSizeFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsItemRecordFields {
    pub agent_id: String,
    pub at: f64,
    pub duration_source: Option<String>,
    pub finished_at: Option<f64>,
    pub id: String,
    pub input: Option<AnalyticsSize>,
    pub is_tool: bool,
    pub name: String,
    pub output: Option<AnalyticsSize>,
    pub source: Option<String>,
    pub started_at: Option<f64>,
    pub stream: Option<AnalyticsSize>,
    #[serde(rename = "type")]
    pub record_type: String,
}
analytics_payload!(AnalyticsItemRecord, AnalyticsItemRecordFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsContextFields {
    pub allowed_source_thread_ids: Option<Vec<String>>,
    pub effort: Option<String>,
    pub model: Option<String>,
    pub notice_association: Option<RawJson>,
    pub pending_usage: Option<RawJson>,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub window: Option<f64>,
}
analytics_payload!(AnalyticsContext, AnalyticsContextFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsHistoryRecordFields {
    pub account_key: String,
    pub agent: String,
    pub anchor: Option<String>,
    pub context: AnalyticsContext,
    pub filesystem_identity: Option<Vec<RawJson>>,
    pub id: String,
    pub identity: Option<Vec<RawJson>>,
    pub imported_records: studio_runtime_schema::StoredJsonInteger,
    pub malformed_lines: studio_runtime_schema::StoredJsonInteger,
    pub offset: studio_runtime_schema::StoredJsonInteger,
    pub thread_id: String,
}
analytics_payload!(AnalyticsHistoryFact, AnalyticsHistoryRecordFields);

/// The reserved importer record is retained alongside ordinary history facts.
#[derive(Serialize)]
#[serde(untagged)]
pub enum AnalyticsHistoryRecord {
    Fact(Box<AnalyticsHistoryFact>),
    Importer(AnalyticsHistoryImporterDiagnostic),
}
impl<'de> Deserialize<'de> for AnalyticsHistoryRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = studio_json::OwnedRawJson::deserialize(deserializer)?;
        let normalized =
            crate::json_record_codec::last_json_members(raw).map_err(serde::de::Error::custom)?;
        match studio_json::from_str(normalized.as_str()) {
            Ok(fact) => Ok(Self::Fact(fact)),
            Err(_) => studio_json::from_str(normalized.as_str())
                .map(Self::Importer)
                .map_err(serde::de::Error::custom),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum AnalyticsHistoryImporterDiagnostic {
    Error {
        error: String,
        updated: f64,
    },
    Current {
        error: (),
        #[serde(rename = "lastError")]
        last_error: Option<String>,
        updated: f64,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTokensFields {
    pub cache_write_input_tokens: Option<f64>,
    pub cached_input_tokens: Option<f64>,
    pub input_tokens: Option<f64>,
    pub output_tokens: Option<f64>,
    pub reasoning_output_tokens: Option<f64>,
    pub total_tokens: Option<f64>,
}
analytics_payload!(AnalyticsTokens, AnalyticsTokensFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsTokenEnvelopeFields {
    pub last: Option<AnalyticsTokens>,
    pub model_context_window: Option<f64>,
    pub total: Option<AnalyticsTokens>,
}
analytics_payload!(AnalyticsTokenEnvelope, AnalyticsTokenEnvelopeFields);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsUsageRecordFields {
    pub agent_id: String,
    pub at: f64,
    pub counter_domain: Option<String>,
    pub id: String,
    pub last: AnalyticsTokens,
    pub model_context_window: Option<f64>,
    pub raw: Option<AnalyticsTokenEnvelope>,
    pub receipt_key: Option<String>,
    pub response_id: Option<String>,
    pub studio_usage: Option<studio_runtime_schema::usage::UsageObservation>,
    pub total: AnalyticsTokens,
    pub usage_identity: Option<studio_runtime_schema::usage::ledger::UsageIdentity>,
}
analytics_payload!(AnalyticsUsageRecord, AnalyticsUsageRecordFields);

analytics_payload!(
    AnalyticsLimitRecord,
    studio_runtime_schema::usage_limits::UsageLimitsData
);

/// Metadata mixes decimal counters, markers and JSON evidence; interpretation stays key-scoped.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyticsMetadataValue(pub String);
impl rusqlite::types::FromSql for AnalyticsMetadataValue {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).map(Self)
    }
}
impl rusqlite::types::ToSql for AnalyticsMetadataValue {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl studio_db_api::DbValue for AnalyticsMetadataValue {
    const SQL_TYPE: &'static str = "TEXT";
}

/// Keep the original TEXT or dictionary-compressed BLOB; parsing is explicit.
pub enum AnalyticsItemEncoding {
    Plain(String),
    Compressed(Vec<u8>),
}
impl AnalyticsItemEncoding {
    pub fn record(
        &self,
    ) -> crate::StoreResult<crate::json_record_codec::ExactJson<AnalyticsItemRecord>> {
        let value = match self {
            Self::Plain(text) => text.clone(),
            Self::Compressed(bytes) => {
                crate::stored_text::decode(rusqlite::types::ValueRef::Blob(bytes))
                    .map_err(|error| crate::StoreError::SchemaMismatch(error.to_string()))?
            }
        };
        Ok(crate::json_record_codec::ExactJson::from_stored(value)?)
    }
}
impl rusqlite::types::FromSql for AnalyticsItemEncoding {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            rusqlite::types::ValueRef::Text(_) => String::column_result(value).map(Self::Plain),
            rusqlite::types::ValueRef::Blob(bytes) => Ok(Self::Compressed(bytes.to_vec())),
            rusqlite::types::ValueRef::Null
            | rusqlite::types::ValueRef::Integer(_)
            | rusqlite::types::ValueRef::Real(_) => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}
impl rusqlite::types::ToSql for AnalyticsItemEncoding {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Self::Plain(text) => text.to_sql(),
            Self::Compressed(bytes) => bytes.to_sql(),
        }
    }
}
impl studio_db_api::DbValue for AnalyticsItemEncoding {
    const SQL_TYPE: &'static str = "TEXT";
}
