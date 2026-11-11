//! Typed metadata values and explicit historical storage evidence for F1.

/// Integer boolean storage admits historical integers without treating them as new booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredBoolean {
    Valid(bool),
    LegacyInteger(i64),
}

impl StoredBoolean {
    pub fn from_stored(value: i64) -> Self {
        match value {
            0 => Self::Valid(false),
            1 => Self::Valid(true),
            _ => Self::LegacyInteger(value),
        }
    }
    pub fn as_stored(&self) -> i64 {
        match self {
            Self::Valid(value) => i64::from(*value),
            Self::LegacyInteger(value) => *value,
        }
    }
}

use crate::json_record_codec::{ExactJson, last_json_members};
use crate::repositories::orchestration_settings::StoredSettings;
use serde::Deserialize;
use std::collections::BTreeMap;
use studio_json::OwnedRawJson;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_schema::raw_json::RawExtensions;

/// Current sidebar selectors. Missing and JSON null remain distinct.
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct SidebarOrderFields {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub revision: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub groups: Field<BTreeMap<String, Vec<String>>>,
}
studio_runtime_schema::raw_json_payload! {
    #[derive(Debug)]
    pub struct SidebarOrderObject {
        pub fields: SidebarOrderFields,
        pub extensions,
    }
}

/// Historical sidebar reads accepted every valid JSON value, including scalars.
#[derive(Debug)]
pub enum SidebarOrderPayload {
    Current(SidebarOrderObject),
    LegacyJson(OwnedRawJson),
}
impl<'de> Deserialize<'de> for SidebarOrderPayload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = OwnedRawJson::deserialize(deserializer)?;
        let selectors = last_json_members(raw.clone()).map_err(serde::de::Error::custom)?;
        match studio_json::from_str(selectors.as_str()) {
            Ok(current) => Ok(Self::Current(current)),
            Err(_) => Ok(Self::LegacyJson(raw)),
        }
    }
}
pub type SidebarOrderRecord = ExactJson<SidebarOrderPayload>;

/// Opaque reuse of the existing two-field owner, including its defaults and duplicate policy.
/// This wrapper deliberately offers no replacement defaults or independently copied fields.
pub struct StoredOrchestrationSettings(ExactJson<JsonObject<StoredSettings>>);
impl StoredOrchestrationSettings {
    pub fn from_stored(raw: String) -> studio_json::Result<Self> {
        ExactJson::from_stored(raw).map(Self)
    }
    pub fn as_raw(&self) -> &str {
        self.0.raw()
    }
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationAdditionalFields {
    #[serde(default)]
    pub idle_compaction: Field<studio_runtime_schema::idle_compaction::Settings>,
    #[serde(default)]
    pub shell_policy: Field<studio_runtime_schema::shell_policy::Settings>,
    /// Send-admission cutover, integer Unix milliseconds.
    #[serde(default)]
    pub send_admission_cutover_ms: Field<i64>,
    /// Request receipt expiry floor, integer Unix milliseconds.
    #[serde(default, rename = "expired_before_ms")]
    pub expired_before_ms: Field<i64>,
}
studio_runtime_schema::raw_json_payload! {
    pub struct OrchestrationAdditionalObject {
        pub fields: OrchestrationAdditionalFields,
        pub extensions,
    }
}
pub struct CompleteOrchestrationSettings {
    settings: StoredOrchestrationSettings,
    additional: OrchestrationAdditionalFields,
    extensions: RawExtensions,
}
impl CompleteOrchestrationSettings {
    pub fn settings(&self) -> &StoredOrchestrationSettings {
        &self.settings
    }
    pub fn additional(&self) -> &OrchestrationAdditionalFields {
        &self.additional
    }
    pub fn extensions(&self) -> &RawExtensions {
        &self.extensions
    }
}

/// Independent old readers could admit a malformed member ignored by another reader.
/// Such an object is preserved explicitly, never repaired with fabricated defaults.
pub enum OrchestrationSettingsEvidence {
    Current(CompleteOrchestrationSettings),
    LegacyObject(ExactJson<BTreeMap<String, OwnedRawJson>>),
}
impl OrchestrationSettingsEvidence {
    pub fn from_stored(raw: String) -> studio_json::Result<Self> {
        let object = ExactJson::<BTreeMap<String, OwnedRawJson>>::from_stored(raw.clone())?;
        let current = (|| {
            let settings = StoredOrchestrationSettings::from_stored(raw)?;
            let mut additional: OrchestrationAdditionalObject =
                studio_json::from_str(settings.as_raw())?;
            if additional
                .fields
                .idle_compaction
                .value()
                .is_some_and(|settings| !settings.valid())
                || additional
                    .fields
                    .shell_policy
                    .value()
                    .is_some_and(|settings| !settings.valid())
            {
                return Err(serde::de::Error::custom(
                    "invalid nested orchestration policy",
                ));
            }
            additional.extensions.remove("childResultCoalesceSeconds");
            additional.extensions.remove("idleSessionUnloadMinutes");
            Ok::<_, studio_json::Error>(CompleteOrchestrationSettings {
                settings,
                additional: additional.fields,
                extensions: additional.extensions,
            })
        })();
        Ok(match current {
            Ok(current) => Self::Current(current),
            Err(_) => Self::LegacyObject(object),
        })
    }
    pub fn as_raw(&self) -> &str {
        match self {
            Self::Current(current) => current.settings.as_raw(),
            Self::LegacyObject(object) => object.raw(),
        }
    }
}

/// Raw comparisons historically admitted arbitrary team-count text, even invalid JSON.
pub enum PublicationTeamCounts {
    Current(ExactJson<studio_runtime_schema::agent_progress::TeamCounts>),
    LegacyText(String),
}
impl PublicationTeamCounts {
    pub fn from_stored(raw: String) -> Self {
        match ExactJson::from_stored(raw.clone()) {
            Ok(value) => Self::Current(value),
            Err(_) => Self::LegacyText(raw),
        }
    }
    pub fn as_raw(&self) -> &str {
        match self {
            Self::Current(value) => value.raw(),
            Self::LegacyText(raw) => raw,
        }
    }
}

/// Open search phase vocabulary; unknown spellings remain exact text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchPhase {
    Building,
    Active,
    Dropping,
    Complete,
    Other(String),
}
impl SearchPhase {
    pub fn from_stored(raw: String) -> Self {
        match raw.as_str() {
            "building" => Self::Building,
            "active" => Self::Active,
            "dropping" => Self::Dropping,
            "complete" => Self::Complete,
            _ => Self::Other(raw),
        }
    }
    pub fn as_stored(&self) -> &str {
        match self {
            Self::Building => "building",
            Self::Active => "active",
            Self::Dropping => "dropping",
            Self::Complete => "complete",
            Self::Other(raw) => raw,
        }
    }
}

/// Wire key of the last published event sequence, measured in shared sync numbers.
pub(crate) const EVENT_WINDOW_SEQUENCE_KEY: &str = "event_window_seq";

/// The key owns the meaning and unit of each text value. Unknown keys stay open.
#[derive(Debug, PartialEq, Eq)]
pub enum SyncMetaKey {
    MigrationCatalog,
    Seeded,
    AgentOrganizationFields,
    AgentProjections,
    SummaryEntities,
    ConversationDetails,
    WorkDetails,
    TaskWindowMigrated,
    EventWindowSequence,
    EntityTombstoneCount,
    EntityTombstoneFloor,
    TeamMembership { root: String },
    TranscriptCopies,
    LegacyEntityScopeVisibility(String),
    LegacyEntityScopeTombstone(String),
    Other(String),
}
impl SyncMetaKey {
    pub fn from_stored(key: &str) -> Self {
        match key {
            crate::migrations::MIGRATION_CATALOG_KEY => Self::MigrationCatalog,
            "seeded" => Self::Seeded,
            "agent_organization_fields" => Self::AgentOrganizationFields,
            crate::AGENT_PROJECTIONS_SEED_MARKER => Self::AgentProjections,
            crate::SUMMARY_ENTITIES_SEED_MARKER => Self::SummaryEntities,
            crate::CONVERSATION_DETAILS_SEED_MARKER => Self::ConversationDetails,
            crate::WORK_DETAILS_SEED_MARKER => Self::WorkDetails,
            "task_window_migrated" => Self::TaskWindowMigrated,
            EVENT_WINDOW_SEQUENCE_KEY => Self::EventWindowSequence,
            crate::repositories::sync_documents::ENTITY_TOMBSTONE_COUNT_KEY => {
                Self::EntityTombstoneCount
            }
            "entity_tombstone_floor" => Self::EntityTombstoneFloor,
            "transcript_copies" => Self::TranscriptCopies,
            _ => {
                if let Some(root) = key
                    .strip_prefix(crate::repositories::sync_documents::TEAM_MEMBERSHIP_KEY_PREFIX)
                    && let Ok(root) = studio_json::from_str::<String>(root)
                {
                    return Self::TeamMembership { root };
                }
                if key.starts_with("entity_scope_visibility:") {
                    Self::LegacyEntityScopeVisibility(key.to_owned())
                } else if key.starts_with("entity_scope_tombstone:") {
                    Self::LegacyEntityScopeTombstone(key.to_owned())
                } else {
                    Self::Other(key.to_owned())
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SyncMetaData {
    MigrationCatalog {
        fingerprint: String,
        schema_version: u64,
    },
    CompleteMarker,
    AgentOrganizationVersion(i64),
    EventWindowSequence(i64),
    EntityTombstoneCount(i64),
    EntityTombstoneFloor(i64),
    TeamMembershipRevision(i64),
    TranscriptCopiesRetired,
    LegacyText,
    OtherText,
}
/// Read-only interpretation paired with exact original key/value text.
/// Parsing a decimal must never normalize its spelling during copy or replay.
#[derive(Debug)]
pub struct SyncMetaValue {
    key: String,
    raw: String,
    kind: SyncMetaKey,
    data: SyncMetaData,
}
impl SyncMetaValue {
    pub fn from_stored(key: String, raw: String) -> Self {
        let kind = SyncMetaKey::from_stored(&key);
        let integer = raw.parse::<i64>().ok();
        let data = match &kind {
            SyncMetaKey::MigrationCatalog => {
                raw.split_once(':').and_then(|(fingerprint, version)| {
                    let version = version.parse::<u64>().ok()?;
                    (fingerprint.len() == 64
                        && fingerprint
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                        && version > 0)
                        .then(|| SyncMetaData::MigrationCatalog {
                            fingerprint: fingerprint.to_owned(),
                            schema_version: version,
                        })
                })
            }
            SyncMetaKey::Seeded
            | SyncMetaKey::AgentProjections
            | SyncMetaKey::SummaryEntities
            | SyncMetaKey::ConversationDetails
            | SyncMetaKey::WorkDetails
            | SyncMetaKey::TaskWindowMigrated => {
                (raw == "1").then_some(SyncMetaData::CompleteMarker)
            }
            SyncMetaKey::AgentOrganizationFields => {
                integer.map(SyncMetaData::AgentOrganizationVersion)
            }
            SyncMetaKey::EventWindowSequence => integer.map(SyncMetaData::EventWindowSequence),
            SyncMetaKey::EntityTombstoneCount => integer.map(SyncMetaData::EntityTombstoneCount),
            SyncMetaKey::EntityTombstoneFloor => integer.map(SyncMetaData::EntityTombstoneFloor),
            SyncMetaKey::TeamMembership { .. } => integer.map(SyncMetaData::TeamMembershipRevision),
            SyncMetaKey::TranscriptCopies => {
                (raw == "retired").then_some(SyncMetaData::TranscriptCopiesRetired)
            }
            SyncMetaKey::LegacyEntityScopeVisibility(_)
            | SyncMetaKey::LegacyEntityScopeTombstone(_) => None,
            SyncMetaKey::Other(_) => Some(SyncMetaData::OtherText),
        }
        .unwrap_or(SyncMetaData::LegacyText);
        Self {
            key,
            raw,
            kind,
            data,
        }
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn kind(&self) -> &SyncMetaKey {
        &self.kind
    }
    pub fn data(&self) -> &SyncMetaData {
        &self.data
    }
    pub fn as_stored(&self) -> &str {
        &self.raw
    }
}

/// One-cell codecs never infer a metadata key from another column or query the database.
pub struct StoredBooleanCodec;
impl studio_db_api::Codec<StoredBoolean> for StoredBooleanCodec {
    type Stored = i64;
    type Encoded<'a> = i64;
    fn encode(_: &rusqlite::Connection, value: &StoredBoolean) -> rusqlite::Result<i64> {
        Ok(value.as_stored())
    }
    fn decode(_: &rusqlite::Connection, value: i64) -> rusqlite::Result<StoredBoolean> {
        Ok(StoredBoolean::from_stored(value))
    }
}

macro_rules! text_codec {
    ($codec:ident, $value:ty, $decode:expr, $raw:ident) => {
        pub struct $codec;
        impl studio_db_api::Codec<$value> for $codec {
            type Stored = String;
            type Encoded<'a> = &'a str;
            fn encode<'a>(
                _: &rusqlite::Connection,
                value: &'a $value,
            ) -> rusqlite::Result<&'a str> {
                Ok(value.$raw())
            }
            fn decode(_: &rusqlite::Connection, raw: String) -> rusqlite::Result<$value> {
                ($decode)(raw)
            }
        }
    };
}
text_codec!(
    SearchPhaseCodec,
    SearchPhase,
    |raw| Ok(SearchPhase::from_stored(raw)),
    as_stored
);
text_codec!(
    PublicationTeamCountsCodec,
    PublicationTeamCounts,
    |raw| Ok(PublicationTeamCounts::from_stored(raw)),
    as_raw
);
text_codec!(
    OrchestrationSettingsCodec,
    OrchestrationSettingsEvidence,
    |raw| {
        OrchestrationSettingsEvidence::from_stored(raw).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    },
    as_raw
);

impl studio_db_api::Codec<Option<PublicationTeamCounts>> for PublicationTeamCountsCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a Option<PublicationTeamCounts>,
    ) -> rusqlite::Result<Option<&'a str>> {
        Ok(value.as_ref().map(PublicationTeamCounts::as_raw))
    }
    fn decode(
        _: &rusqlite::Connection,
        raw: Option<String>,
    ) -> rusqlite::Result<Option<PublicationTeamCounts>> {
        Ok(raw.map(PublicationTeamCounts::from_stored))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_storage_distinguishes_valid_values_and_preserves_invalid_integers() {
        assert_eq!(StoredBoolean::from_stored(0), StoredBoolean::Valid(false));
        assert_eq!(StoredBoolean::from_stored(1), StoredBoolean::Valid(true));
        for value in [i64::MIN, -1, 2, i64::MAX] {
            let evidence = StoredBoolean::from_stored(value);
            assert_eq!(evidence, StoredBoolean::LegacyInteger(value));
            assert_eq!(evidence.as_stored(), value);
        }
    }
}

#[cfg(test)]
mod value_tests {
    use super::*;

    #[test]
    fn sidebar_preserves_presence_extensions_and_last_member_selectors() {
        let raw =
            " \n{\"revision\":false,\"revision\":7,\"groups\":null,\"future\": {\"x\":1.00}}\t";
        let value = SidebarOrderRecord::from_stored(raw.into()).unwrap();
        assert_eq!(value.raw(), raw);
        let SidebarOrderPayload::Current(current) = value.value() else {
            panic!("typed current")
        };
        assert_eq!(current.fields.revision, Field::Value(7));
        assert_eq!(current.fields.groups, Field::Null);
        assert_eq!(current.extensions["future"].as_str(), r#"{"x":1.00}"#);
        let value = SidebarOrderRecord::from_stored("{}".into()).unwrap();
        let SidebarOrderPayload::Current(current) = value.value() else {
            panic!("typed missing")
        };
        assert_eq!(current.fields.revision, Field::Missing);
        assert_eq!(current.fields.groups, Field::Missing);
        let raw = r#"{"revision":1,"groups":{"a":["x","y"]}}"#;
        let value = SidebarOrderRecord::from_stored(raw.into()).unwrap();
        let SidebarOrderPayload::Current(current) = value.value() else {
            panic!("typed groups")
        };
        assert_eq!(current.fields.groups.value().unwrap()["a"], ["x", "y"]);
    }

    #[test]
    fn sidebar_preserves_permissive_histories_and_refuses_invalid_json() {
        for raw in [
            "null",
            "42",
            "[]",
            r#""text""#,
            r#"{"revision":-1}"#,
            r#"{"groups":{"x":42,"y":[null,{},"agent"]},"revision":1,"extra":1.00}"#,
        ] {
            let value = SidebarOrderRecord::from_stored(raw.into()).unwrap();
            assert!(
                matches!(value.value(), SidebarOrderPayload::LegacyJson(_)),
                "{raw}"
            );
            assert_eq!(value.raw(), raw);
        }
        for raw in ["", "not JSON", "{", "{} trailing", r#"{"groups":NaN}"#] {
            assert!(
                SidebarOrderRecord::from_stored(raw.into()).is_err(),
                "{raw}"
            );
        }
    }

    #[test]
    fn orchestration_reuses_all_six_owners_without_losing_unknown_members() {
        let raw = " \n{\"childResultCoalesceSeconds\":0,\"idleSessionUnloadMinutes\":1440,\"idleCompaction\":{\"mode\":\"log\",\"minTokens\":60000},\"shellPolicy\":{\"quietThresholdMs\":10000,\"pressureCleanup\":false},\"sendAdmissionCutoverMs\":-3,\"expired_before_ms\":9,\"future\": [1.00]}\t";
        let value = OrchestrationSettingsEvidence::from_stored(raw.into()).unwrap();
        assert_eq!(value.as_raw(), raw);
        let OrchestrationSettingsEvidence::Current(current) = value else {
            panic!("complete typed settings")
        };
        assert_eq!(current.settings.as_raw(), raw);
        let fields = current.additional;
        assert!(fields.idle_compaction.value().unwrap().valid());
        assert!(fields.shell_policy.value().unwrap().valid());
        assert_eq!(fields.send_admission_cutover_ms, Field::Value(-3));
        assert_eq!(fields.expired_before_ms, Field::Value(9));
        assert_eq!(current.extensions.len(), 1);
        assert_eq!(current.extensions["future"].as_str(), "[1.00]");
        let value = OrchestrationSettingsEvidence::from_stored("{}".into()).unwrap();
        let OrchestrationSettingsEvidence::Current(current) = value else {
            panic!("existing defaults")
        };
        assert_eq!(current.additional.idle_compaction, Field::Missing);
        let value = OrchestrationSettingsEvidence::from_stored(
            r#"{"idleCompaction":null,"shellPolicy":null,"expired_before_ms":null}"#.into(),
        )
        .unwrap();
        let OrchestrationSettingsEvidence::Current(current) = value else {
            panic!("nullable selectors")
        };
        assert_eq!(current.additional.idle_compaction, Field::Null);
        assert_eq!(current.additional.expired_before_ms, Field::Null);
    }

    #[test]
    fn orchestration_known_field_refusal_never_fabricates_defaults() {
        for raw in [
            r#"{"childResultCoalesceSeconds":null}"#,
            r#"{"idleSessionUnloadMinutes":"5"}"#,
            r#"{"childResultCoalesceSeconds":1,"childResultCoalesceSeconds":2}"#,
        ] {
            assert!(StoredOrchestrationSettings::from_stored(raw.into()).is_err());
            let evidence = OrchestrationSettingsEvidence::from_stored(raw.into()).unwrap();
            assert!(matches!(
                evidence,
                OrchestrationSettingsEvidence::LegacyObject(_)
            ));
            assert_eq!(evidence.as_raw(), raw);
        }
        for raw in [
            r#"{"idleCompaction":{"mode":"log"}}"#,
            r#"{"shellPolicy":false}"#,
            r#"{"expired_before_ms":"9"}"#,
            r#"{"sendAdmissionCutoverMs":1,"sendAdmissionCutoverMs":2}"#,
        ] {
            let evidence = OrchestrationSettingsEvidence::from_stored(raw.into()).unwrap();
            assert!(
                matches!(evidence, OrchestrationSettingsEvidence::LegacyObject(_)),
                "{raw}"
            );
            assert_eq!(evidence.as_raw(), raw);
        }
        for raw in ["null", "[]", "false", "4", "not JSON"] {
            assert!(OrchestrationSettingsEvidence::from_stored(raw.into()).is_err());
        }
    }

    #[test]
    fn orchestration_refuses_invalid_nested_policy_values_as_current_settings() {
        for raw in [
            r#"{"idleCompaction":{"mode":"log","minTokens":1}}"#,
            r#"{"shellPolicy":{"quietThresholdMs":1,"pressureCleanup":true}}"#,
        ] {
            let evidence = OrchestrationSettingsEvidence::from_stored(raw.into()).unwrap();
            assert!(
                matches!(evidence, OrchestrationSettingsEvidence::LegacyObject(_)),
                "{raw}"
            );
            assert_eq!(evidence.as_raw(), raw);
        }
    }

    #[test]
    fn sync_meta_values_keep_purpose_and_original_numeric_spelling() {
        for (key, expected) in [
            (
                "agent_organization_fields",
                SyncMetaData::AgentOrganizationVersion(-7),
            ),
            ("event_window_seq", SyncMetaData::EventWindowSequence(-7)),
            (
                "entity_tombstone_count",
                SyncMetaData::EntityTombstoneCount(-7),
            ),
            (
                "entity_tombstone_floor",
                SyncMetaData::EntityTombstoneFloor(-7),
            ),
            (
                r#"team_membership:"root\n\"id""#,
                SyncMetaData::TeamMembershipRevision(-7),
            ),
        ] {
            let value = SyncMetaValue::from_stored(key.into(), "-007".into());
            assert_eq!(value.data, expected);
            assert_eq!(value.as_stored(), "-007");
            assert_eq!(value.key(), key);
        }
        let value =
            SyncMetaValue::from_stored(r#"team_membership:"root\n\"id""#.into(), "1".into());
        assert_eq!(
            value.kind,
            SyncMetaKey::TeamMembership {
                root: "root\n\"id".into()
            }
        );
        for key in [
            "seeded",
            crate::AGENT_PROJECTIONS_SEED_MARKER,
            crate::SUMMARY_ENTITIES_SEED_MARKER,
            crate::CONVERSATION_DETAILS_SEED_MARKER,
            crate::WORK_DETAILS_SEED_MARKER,
            "task_window_migrated",
        ] {
            assert_eq!(
                SyncMetaValue::from_stored(key.into(), "1".into()).data,
                SyncMetaData::CompleteMarker
            );
            let value = SyncMetaValue::from_stored(key.into(), "01".into());
            assert_eq!(value.data, SyncMetaData::LegacyText);
            assert_eq!(value.as_stored(), "01");
        }
        assert_eq!(
            SyncMetaValue::from_stored("transcript_copies".into(), "retired".into()).data,
            SyncMetaData::TranscriptCopiesRetired
        );
    }

    #[test]
    fn sync_meta_refuses_typed_interpretation_but_preserves_legacy_and_unknown_text() {
        for key in [
            "event_window_seq",
            "entity_tombstone_count",
            "entity_tombstone_floor",
            "agent_organization_fields",
            r#"team_membership:"a""#,
        ] {
            for raw in ["", "no", "1.5", "9223372036854775808", "2 trailing"] {
                let value = SyncMetaValue::from_stored(key.into(), raw.into());
                assert_eq!(value.data, SyncMetaData::LegacyText);
                assert_eq!(value.as_stored(), raw);
            }
        }
        for key in [
            "future\0key",
            "team_membership:unquoted",
            "team_membership:null",
        ] {
            let value = SyncMetaValue::from_stored(key.into(), "not JSON\0".into());
            assert_eq!(value.kind, SyncMetaKey::Other(key.into()));
            assert_eq!(value.data, SyncMetaData::OtherText);
            assert_eq!(value.key(), key);
            assert_eq!(value.as_stored(), "not JSON\0");
        }
        for key in ["entity_scope_visibility:[]", "entity_scope_tombstone:[]"] {
            let value = SyncMetaValue::from_stored(key.into(), "[]".into());
            assert_eq!(value.data, SyncMetaData::LegacyText);
            assert_eq!(value.key(), key);
        }
        let raw = format!("{}:0003", "a".repeat(64));
        let value = SyncMetaValue::from_stored("migration_catalog".into(), raw.clone());
        assert_eq!(
            value.data,
            SyncMetaData::MigrationCatalog {
                fingerprint: "a".repeat(64),
                schema_version: 3
            }
        );
        assert_eq!(value.as_stored(), raw);
        for raw in [
            format!("{}:0", "a".repeat(64)),
            format!("{}:1", "A".repeat(64)),
            "not-a-catalog".into(),
        ] {
            let value = SyncMetaValue::from_stored("migration_catalog".into(), raw.clone());
            assert_eq!(value.data, SyncMetaData::LegacyText);
            assert_eq!(value.as_stored(), raw);
        }
    }

    #[test]
    fn team_counts_and_search_phases_preserve_their_historical_evidence() {
        let raw = " \n{\"working\":1,\"total\":2,\"future\":1.00}\t";
        let value = PublicationTeamCounts::from_stored(raw.into());
        assert!(
            matches!(&value, PublicationTeamCounts::Current(value) if value.value().working == 1)
        );
        assert_eq!(value.as_raw(), raw);
        for raw in [
            "not JSON",
            "null",
            r#"{"working":1}"#,
            r#"{"working":false,"total":2}"#,
        ] {
            let value = PublicationTeamCounts::from_stored(raw.into());
            assert!(matches!(value, PublicationTeamCounts::LegacyText(_)));
            assert_eq!(value.as_raw(), raw);
        }
        for phase in [
            "building",
            "active",
            "dropping",
            "complete",
            "",
            "future\0phase",
            "ACTIVE",
        ] {
            let value = SearchPhase::from_stored(phase.into());
            assert_eq!(value.as_stored(), phase);
        }
        assert!(matches!(
            SearchPhase::from_stored("ACTIVE".into()),
            SearchPhase::Other(_)
        ));
    }
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use studio_db_api::Codec;

    #[test]
    fn codecs_bind_exact_source_cells_and_refuse_invalid_orchestration_objects() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for raw in [0, 1, -1, i64::MIN, i64::MAX] {
            let value = StoredBooleanCodec::decode(&db, raw).unwrap();
            assert_eq!(StoredBooleanCodec::encode(&db, &value).unwrap(), raw);
        }
        for raw in ["", "active", "future\0phase"] {
            let value = SearchPhaseCodec::decode(&db, raw.into()).unwrap();
            assert_eq!(SearchPhaseCodec::encode(&db, &value).unwrap(), raw);
        }
        for raw in ["not JSON", "null", " {\"working\":1,\"total\":2} "] {
            let value: PublicationTeamCounts =
                PublicationTeamCountsCodec::decode(&db, raw.into()).unwrap();
            assert_eq!(
                PublicationTeamCountsCodec::encode(&db, &value).unwrap(),
                raw
            );
        }
        for raw in [" {} ", r#"{"idleCompaction":false,"future":1.00}"#] {
            let value = OrchestrationSettingsCodec::decode(&db, raw.into()).unwrap();
            assert_eq!(
                OrchestrationSettingsCodec::encode(&db, &value).unwrap(),
                raw
            );
        }
        for raw in ["null", "[]", "4", "invalid"] {
            assert!(OrchestrationSettingsCodec::decode(&db, raw.into()).is_err());
        }
    }
}

#[cfg(test)]
mod nullable_tests {
    use super::*;
    use studio_db_api::Codec;

    #[test]
    fn sql_null_team_counts_are_distinct_from_json_null_and_malformed_text() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        type CountsCodec = PublicationTeamCountsCodec;
        assert!(
            <CountsCodec as Codec<Option<PublicationTeamCounts>>>::decode(&db, None)
                .unwrap()
                .is_none()
        );
        for raw in ["null", "", " {\"working\":1,\"total\":2} "] {
            let value = <CountsCodec as Codec<Option<PublicationTeamCounts>>>::decode(
                &db,
                Some(raw.into()),
            )
            .unwrap();
            assert_eq!(
                <CountsCodec as Codec<Option<PublicationTeamCounts>>>::encode(&db, &value).unwrap(),
                Some(raw)
            );
        }
    }
}
