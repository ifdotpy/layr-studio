//! Server-wide orchestration settings that the scheduler and the account
//! workers read on every use.

use crate::metadata_schema::OrchestrationSettings;
use crate::schema::{SchemaRequirements, TableRequirement};
use crate::{StoreError, StoreResult};
use serde::Deserialize;
use std::time::Duration;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::shared_values::{
    CHILD_RESULT_COALESCE_DEFAULT_SECONDS, CHILD_RESULT_COALESCE_MAX_SECONDS,
    CHILD_RESULT_COALESCE_MIN_SECONDS, IDLE_SESSION_UNLOAD_DEFAULT_MINUTES,
    IDLE_SESSION_UNLOAD_MAX_MINUTES, IDLE_SESSION_UNLOAD_MIN_MINUTES,
};

use crate::consolidation_kinds::CanvasMetadataKind;
use crate::metadata_schema::Metadata;
use crate::peer_update_settings::{Canvas, canvas_read};
use studio_db_api::DbQuery;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,result=Option<String>,decode=Metadata::TEXT_KEY,
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,record=Metadata::ORCHESTRATION_RECORD,value=Metadata::TEXT_KEY),
 sql="SELECT json_extract({record},?3) AS {value} FROM {metadata} WHERE {kind}=?1 AND {identity}=?2")]
struct PolicyValue {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    path: String,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,key=Metadata::TEXT_KEY,rowid=Metadata::SOURCE_ROWID,record=Metadata::ORCHESTRATION_RECORD),
 sql="INSERT INTO {metadata}({kind},{identity},{key},{rowid},{record}) VALUES(?1,?2,'global',0,json_object(?3,?4)) ON CONFLICT({kind},{identity}) DO UPDATE SET {record}=json_set({record},?5,?4)")]
struct StoreNumber {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    field: String,
    #[db(column=crate::settings_schema::ClientCommandWatermarks::APPLIED)]
    value: i64,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    path: String,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,key=Metadata::TEXT_KEY,rowid=Metadata::SOURCE_ROWID,record=Metadata::ORCHESTRATION_RECORD),
 sql="INSERT INTO {metadata}({kind},{identity},{key},{rowid},{record}) VALUES(?1,?2,'global',0,json_object(?3,json(?4))) ON CONFLICT({kind},{identity}) DO UPDATE SET {record}=json_set({record},?5,json(?4))")]
struct StorePolicy {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    field: String,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    value: String,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    path: String,
}
// Owner: request-result admission. Unit: UTC milliseconds below which new IDs expire.
const REQUEST_EXPIRATION_FIELD: &str = "expired_before_ms";

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=i64,
 tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,record=Metadata::ORCHESTRATION_RECORD),
 sql="SELECT coalesce(json_extract({record},?3),0) FROM {metadata} WHERE {kind}=?1 AND {identity}=?2")]
struct RequestExpiration {
    #[db(column=Metadata::KIND)]
    kind: CanvasMetadataKind,
    #[db(column=Metadata::IDENTITY)]
    identity: Vec<u8>,
    #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
    path: String,
}

pub(crate) fn request_expiration_floor(connection: &rusqlite::Connection) -> rusqlite::Result<i64> {
    Ok(canvas_read(connection)
        .named(
            &RequestExpiration::QUERY,
            &RequestExpiration {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                path: format!("$.{REQUEST_EXPIRATION_FIELD}"),
            },
        )?
        .unwrap_or(0))
}

pub(crate) fn set_request_expiration_floor(
    transaction: &rusqlite::Transaction<'_>,
    floor: i64,
) -> rusqlite::Result<()> {
    // This admission marker does not change scheduler settings or notify their observers.
    crate::peer_update_settings::canvas_tx(transaction)
        .named(
            &StoreNumber::QUERY,
            &StoreNumber {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                field: REQUEST_EXPIRATION_FIELD.into(),
                value: floor,
                path: format!("$.{REQUEST_EXPIRATION_FIELD}"),
            },
        )
        .map(|_| ())
}

fn store_settings<P: studio_db_api::QueryParams>(
    connection: &rusqlite::Connection,
    query: &studio_db_api::NamedQuery<
        Canvas,
        P,
        (),
        studio_db_api::WriteMode,
        studio_db_api::NoRows,
    >,
    params: &P,
) -> StoreResult<()> {
    let before = connection.total_changes();
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        crate::peer_update_settings::canvas_tx(&transaction).named(query, params)?;
        transaction.commit()?;
    } else {
        crate::peer_update_settings::canvas_write(connection)?.named(query, params)?;
    }
    if connection.total_changes() != before {
        crate::record_settings_change(CanvasMetadataKind::OrchestrationSettings);
    }
    Ok(())
}

const TABLE: &str = "runtime_orchestration_settings";
const ROW: &str = "global";
const SECONDS_PER_MINUTE: u64 = 60;

/// One whole-number field of the settings record, with its unset value and
/// accepted range; the shared values own the numbers and their unit.
#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub field: &'static str,
    pub default: u64,
    pub min: u64,
    pub max: u64,
}

impl Setting {
    pub fn valid(&self, value: u64) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// Seconds a lead's child result waits for sibling subagents that are still
/// queued or running; zero delivers every child result at once.
pub const CHILD_RESULT_COALESCE_SETTING: Setting = Setting {
    field: "childResultCoalesceSeconds",
    default: CHILD_RESULT_COALESCE_DEFAULT_SECONDS,
    min: CHILD_RESULT_COALESCE_MIN_SECONDS,
    max: CHILD_RESULT_COALESCE_MAX_SECONDS,
};
/// Minutes a lead's or ordinary chat's idle thread stays loaded and an
/// account's provider process runs after its last thread unloaded; managed
/// workers unload as soon as their turn settles; zero never unloads.
pub const IDLE_SESSION_UNLOAD_SETTING: Setting = Setting {
    field: "idleSessionUnloadMinutes",
    default: IDLE_SESSION_UNLOAD_DEFAULT_MINUTES,
    min: IDLE_SESSION_UNLOAD_MIN_MINUTES,
    max: IDLE_SESSION_UNLOAD_MAX_MINUTES,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredSettings {
    #[serde(default = "default_child_result_coalesce")]
    child_result_coalesce_seconds: u64,
    #[serde(default = "default_idle_session_unload")]
    idle_session_unload_minutes: u64,
}

fn default_child_result_coalesce() -> u64 {
    CHILD_RESULT_COALESCE_SETTING.default
}

fn default_idle_session_unload() -> u64 {
    IDLE_SESSION_UNLOAD_SETTING.default
}

/// How long a lead's child result waits for sibling subagents that are still
/// queued or running, until the setting is changed.
pub const CHILD_RESULT_COALESCE_DEFAULT: Duration =
    Duration::from_secs(CHILD_RESULT_COALESCE_DEFAULT_SECONDS);
/// The shortest accepted hold; zero delivers every child result at once.
pub const CHILD_RESULT_COALESCE_MIN: Duration =
    Duration::from_secs(CHILD_RESULT_COALESCE_MIN_SECONDS);
/// The longest accepted hold.
pub const CHILD_RESULT_COALESCE_MAX: Duration =
    Duration::from_secs(CHILD_RESULT_COALESCE_MAX_SECONDS);

const TABLES: &[TableRequirement] = &[TableRequirement {
    name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
    columns: &[
        crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
        crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
        crate::metadata_schema::Metadata::__DB_COLUMN_ORCHESTRATION_RECORD,
    ],
}];
pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: TABLES,
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Copy, Debug, Default)]
pub struct OrchestrationSettingsRepository;

impl OrchestrationSettingsRepository {
    pub fn shell_policy(
        connection: &rusqlite::Connection,
    ) -> StoreResult<studio_runtime_schema::shell_policy::Settings> {
        let raw = canvas_read(connection)
            .named(
                &PolicyValue::QUERY,
                &PolicyValue {
                    kind: CanvasMetadataKind::OrchestrationSettings,
                    identity: crate::consolidation_codec::nullable_text_identity(
                        &Some(ROW.into()),
                        0,
                    ),
                    path: "$.shellPolicy".into(),
                },
            )?
            .flatten();
        let settings: studio_runtime_schema::shell_policy::Settings = raw
            .map(|raw| studio_json::from_str(&raw))
            .transpose()
            .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?
            .unwrap_or_default();
        if !settings.valid() {
            return Err(StoreError::SchemaMismatch(
                "Invalid shellPolicy.quietThresholdMs".into(),
            ));
        }
        Ok(settings)
    }

    pub fn set_shell_policy(
        connection: &rusqlite::Connection,
        settings: &studio_runtime_schema::shell_policy::Settings,
    ) -> StoreResult<()> {
        if !settings.valid() {
            return Err(StoreError::SchemaMismatch(
                "Invalid shellPolicy.quietThresholdMs".into(),
            ));
        }
        store_settings(
            connection,
            &StorePolicy::QUERY,
            &StorePolicy {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                field: "shellPolicy".into(),
                value: studio_json::to_string(settings)
                    .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?,
                path: "$.shellPolicy".into(),
            },
        )?;
        Ok(())
    }

    /// A missing row or field uses the default; an out-of-range value is refused.
    pub fn value(connection: &rusqlite::Connection, setting: Setting) -> StoreResult<u64> {
        let invalid = || {
            StoreError::SchemaMismatch(format!(
                "invalid stored object {TABLE}/{ROW}: {}",
                setting.field
            ))
        };
        let Some(row) = crate::peer_update_settings::canvas_read(connection)
            .get::<OrchestrationSettings>(&(crate::consolidation_codec::nullable_text_identity(
                &Some(ROW.into()),
                0,
            ),))
            .map_err(|error| {
                if matches!(&error, rusqlite::Error::FromSqlConversionFailure(_, _, _)) {
                    invalid()
                } else {
                    StoreError::from(error)
                }
            })?
        else {
            return Ok(setting.default);
        };
        let JsonObject(record): JsonObject<StoredSettings> =
            studio_json::from_str(row.orchestration_record.as_raw()).map_err(|_| invalid())?;
        let value = match setting.field {
            field if field == CHILD_RESULT_COALESCE_SETTING.field => {
                record.child_result_coalesce_seconds
            }
            field if field == IDLE_SESSION_UNLOAD_SETTING.field => {
                record.idle_session_unload_minutes
            }
            _ => return Err(invalid()),
        };
        setting.valid(value).then_some(value).ok_or_else(invalid)
    }

    /// Store a validated value of `setting`, keeping the record's other fields.
    pub fn set(connection: &rusqlite::Connection, setting: Setting, value: u64) -> StoreResult<()> {
        let value = i64::try_from(value)
            .ok()
            .filter(|_| setting.valid(value))
            .ok_or_else(|| {
                StoreError::SchemaMismatch(format!(
                    "{} must be between {} and {}",
                    setting.field, setting.min, setting.max
                ))
            })?;
        store_settings(
            connection,
            &StoreNumber::QUERY,
            &StoreNumber {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                field: setting.field.into(),
                value,
                path: format!("$.{}", setting.field),
            },
        )?;
        Ok(())
    }

    /// The stored Claude idle compaction setting; `Mode::Default` when none is stored.
    pub fn idle_compaction(
        connection: &rusqlite::Connection,
    ) -> StoreResult<studio_runtime_schema::idle_compaction::Settings> {
        let raw = canvas_read(connection)
            .named(
                &PolicyValue::QUERY,
                &PolicyValue {
                    kind: CanvasMetadataKind::OrchestrationSettings,
                    identity: crate::consolidation_codec::nullable_text_identity(
                        &Some(ROW.into()),
                        0,
                    ),
                    path: "$.idleCompaction".into(),
                },
            )?
            .flatten();
        let settings: studio_runtime_schema::idle_compaction::Settings = raw
            .map(|raw| studio_json::from_str(&raw))
            .transpose()
            .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?
            .unwrap_or_default();
        if !settings.valid() {
            return Err(StoreError::SchemaMismatch(
                "idleCompaction.minTokens is below the minimum".into(),
            ));
        }
        Ok(settings)
    }

    /// The setting with the default mode resolved for a lead or a worker chat.
    pub fn idle_compaction_for(
        connection: &rusqlite::Connection,
        is_lead: bool,
    ) -> StoreResult<studio_runtime_schema::idle_compaction::Settings> {
        Ok(Self::idle_compaction(connection)?.resolved(is_lead))
    }

    pub fn set_idle_compaction(
        connection: &rusqlite::Connection,
        settings: &studio_runtime_schema::idle_compaction::Settings,
    ) -> StoreResult<()> {
        if !settings.valid() {
            return Err(StoreError::SchemaMismatch(
                "idleCompaction.minTokens is below the minimum".into(),
            ));
        }
        store_settings(
            connection,
            &StorePolicy::QUERY,
            &StorePolicy {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                field: "idleCompaction".into(),
                value: studio_json::to_string(settings)
                    .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?,
                path: "$.idleCompaction".into(),
            },
        )?;
        Ok(())
    }

    /// The effective child-result hold.
    pub fn child_result_coalesce(connection: &rusqlite::Connection) -> StoreResult<Duration> {
        Self::value(connection, CHILD_RESULT_COALESCE_SETTING).map(Duration::from_secs)
    }

    /// Store a validated child-result hold in whole seconds.
    pub fn set_child_result_coalesce(
        connection: &rusqlite::Connection,
        seconds: u64,
    ) -> StoreResult<()> {
        Self::set(connection, CHILD_RESULT_COALESCE_SETTING, seconds)
    }

    /// How long an idle lead or chat thread stays loaded and an idle provider
    /// process runs; `None` never unloads.
    pub fn idle_session_unload(connection: &rusqlite::Connection) -> StoreResult<Option<Duration>> {
        let minutes = Self::value(connection, IDLE_SESSION_UNLOAD_SETTING)?;
        Ok((minutes > 0).then(|| Duration::from_secs(minutes * SECONDS_PER_MINUTE)))
    }
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "orchestration_settings",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
      tables(metadata=Metadata),columns(kind=Metadata::KIND,identity=Metadata::IDENTITY,key=Metadata::TEXT_KEY,rowid=Metadata::SOURCE_ROWID,record=Metadata::ORCHESTRATION_RECORD),
      sql="INSERT INTO {metadata}({kind},{identity},{key},{rowid},{record}) VALUES(?1,?2,'global',0,?3) ON CONFLICT({kind},{identity}) DO UPDATE SET {record}=excluded.{record}")]
    struct StoredFixture {
        #[db(column=Metadata::KIND)]
        kind: CanvasMetadataKind,
        #[db(column=Metadata::IDENTITY)]
        identity: Vec<u8>,
        #[db(column=crate::settings_schema::SidebarOrder::RECORD)]
        raw: String,
    }
    fn seed(connection: &rusqlite::Connection, raw: &str) {
        store_settings(
            connection,
            &StoredFixture::QUERY,
            &StoredFixture {
                kind: CanvasMetadataKind::OrchestrationSettings,
                identity: crate::consolidation_codec::nullable_text_identity(&Some(ROW.into()), 0),
                raw: raw.into(),
            },
        )
        .unwrap();
    }
    fn database() -> rusqlite::Connection {
        let connection = rusqlite::Connection::open_in_memory().expect("memory database opens");
        use studio_db_api::Table;
        Metadata::descriptor()
            .install_if_absent(&connection)
            .expect("target metadata fixture");
        connection
    }

    fn unload(connection: &rusqlite::Connection) -> Option<Duration> {
        OrchestrationSettingsRepository::idle_session_unload(connection).expect("reads")
    }

    #[test]
    fn request_expiration_selector_preserves_first_member_null_type_and_other_settings() {
        let mut connection = database();
        assert_eq!(request_expiration_floor(&connection).unwrap(), 0);
        for raw in ["{}", r#"{"expired_before_ms":null}"#] {
            seed(&connection, raw);
            assert_eq!(request_expiration_floor(&connection).unwrap(), 0);
        }
        seed(
            &connection,
            r#"{"expired_before_ms":7,"expired_before_ms":99,"kept":"yes"}"#,
        );
        assert_eq!(request_expiration_floor(&connection).unwrap(), 7);
        let tx = connection.transaction().unwrap();
        set_request_expiration_floor(&tx, 11).unwrap();
        assert_eq!(request_expiration_floor(&tx).unwrap(), 11);
        assert_eq!(
            canvas_read(&tx)
                .named(
                    &PolicyValue::QUERY,
                    &PolicyValue {
                        kind: CanvasMetadataKind::OrchestrationSettings,
                        identity: crate::consolidation_codec::nullable_text_identity(
                            &Some(ROW.into()),
                            0
                        ),
                        path: "$.kept".into(),
                    }
                )
                .unwrap(),
            Some(Some("yes".into()))
        );
        tx.rollback().unwrap();
        assert_eq!(request_expiration_floor(&connection).unwrap(), 7);
        seed(&connection, r#"{"expired_before_ms":"7"}"#);
        assert!(matches!(
            request_expiration_floor(&connection),
            Err(rusqlite::Error::InvalidColumnType(..))
        ));
    }

    #[test]
    fn policy_selectors_preserve_first_member_null_and_independent_fields() {
        let connection = database();
        for raw in ["{}", r#"{"shellPolicy":null,"idleCompaction":null}"#] {
            seed(&connection, raw);
            assert_eq!(
                OrchestrationSettingsRepository::shell_policy(&connection).unwrap(),
                studio_runtime_schema::shell_policy::Settings::default()
            );
            assert_eq!(
                OrchestrationSettingsRepository::idle_compaction(&connection).unwrap(),
                studio_runtime_schema::idle_compaction::Settings::default()
            );
        }
        seed(
            &connection,
            r#"{"shellPolicy":null,"shellPolicy":false,"childResultCoalesceSeconds":"bad"}"#,
        );
        assert!(OrchestrationSettingsRepository::shell_policy(&connection).is_ok());
        assert!(OrchestrationSettingsRepository::child_result_coalesce(&connection).is_err());
        seed(&connection, r#"{"shellPolicy":false,"shellPolicy":null}"#);
        assert!(OrchestrationSettingsRepository::shell_policy(&connection).is_err());
    }

    #[test]
    fn setters_work_inside_existing_transactions_and_rollback() {
        let connection = database();
        let tx = connection.unchecked_transaction().unwrap();
        OrchestrationSettingsRepository::set_child_result_coalesce(&tx, 0).unwrap();
        OrchestrationSettingsRepository::set_shell_policy(
            &tx,
            &studio_runtime_schema::shell_policy::Settings::default(),
        )
        .unwrap();
        OrchestrationSettingsRepository::set_idle_compaction(
            &tx,
            &studio_runtime_schema::idle_compaction::Settings::default(),
        )
        .unwrap();
        assert_eq!(
            OrchestrationSettingsRepository::child_result_coalesce(&tx).unwrap(),
            Duration::ZERO
        );
        tx.rollback().unwrap();
        assert_eq!(
            OrchestrationSettingsRepository::child_result_coalesce(&connection).unwrap(),
            CHILD_RESULT_COALESCE_DEFAULT
        );
    }

    #[test]
    fn missing_row_uses_the_default() {
        assert_eq!(
            OrchestrationSettingsRepository::child_result_coalesce(&database()).expect("reads"),
            CHILD_RESULT_COALESCE_DEFAULT
        );
    }

    #[test]
    fn stored_field_types_and_unrelated_fields_keep_their_behavior() {
        let connection = database();
        for (record, expected) in [
            (r#"{}"#, Some(CHILD_RESULT_COALESCE_SETTING.default)),
            (r#"[]"#, None),
            (r#"null"#, None),
            (r#"{"idleSessionUnloadMinutes":null}"#, None),
            (
                r#"{"unrelated":null,"childResultCoalesceSeconds":0}"#,
                Some(0),
            ),
            (r#"{"childResultCoalesceSeconds":null}"#, None),
            (r#"{"childResultCoalesceSeconds":"1"}"#, None),
            (r#"{"childResultCoalesceSeconds":-1}"#, None),
            (r#"{"#, None),
        ] {
            seed(&connection, record);
            assert_eq!(
                OrchestrationSettingsRepository::value(&connection, CHILD_RESULT_COALESCE_SETTING)
                    .ok(),
                expected,
                "{record}"
            );
        }
    }

    #[test]
    fn stored_value_persists_and_bounds_are_enforced() {
        let connection = database();
        for duration in [CHILD_RESULT_COALESCE_MIN, CHILD_RESULT_COALESCE_MAX] {
            OrchestrationSettingsRepository::set_child_result_coalesce(
                &connection,
                duration.as_secs(),
            )
            .expect("bound is accepted");
            assert_eq!(
                OrchestrationSettingsRepository::child_result_coalesce(&connection).expect("reads"),
                duration
            );
        }
        let above = CHILD_RESULT_COALESCE_MAX.as_secs() + 1;
        assert!(
            OrchestrationSettingsRepository::set_child_result_coalesce(&connection, above).is_err()
        );
        assert_eq!(
            OrchestrationSettingsRepository::child_result_coalesce(&connection).expect("reads"),
            CHILD_RESULT_COALESCE_MAX,
            "a refused write keeps the stored value"
        );
    }

    #[test]
    fn idle_unload_persists_beside_the_hold_and_zero_never_unloads() {
        let connection = database();
        let default = Duration::from_secs(IDLE_SESSION_UNLOAD_DEFAULT_MINUTES * SECONDS_PER_MINUTE);
        assert_eq!(unload(&connection), Some(default));
        OrchestrationSettingsRepository::set_child_result_coalesce(&connection, 0).expect("hold");
        assert_eq!(
            unload(&connection),
            Some(default),
            "a row written before the field existed uses the default"
        );
        let max = IDLE_SESSION_UNLOAD_SETTING.max;
        OrchestrationSettingsRepository::set(&connection, IDLE_SESSION_UNLOAD_SETTING, max)
            .expect("bound is accepted");
        assert_eq!(
            unload(&connection),
            Some(Duration::from_secs(max * SECONDS_PER_MINUTE))
        );
        assert_eq!(
            OrchestrationSettingsRepository::child_result_coalesce(&connection).expect("reads"),
            Duration::ZERO,
            "writing one field keeps the other"
        );
        assert!(
            OrchestrationSettingsRepository::set(&connection, IDLE_SESSION_UNLOAD_SETTING, max + 1)
                .is_err()
        );
        OrchestrationSettingsRepository::set(
            &connection,
            IDLE_SESSION_UNLOAD_SETTING,
            IDLE_SESSION_UNLOAD_SETTING.min,
        )
        .expect("zero is accepted");
        assert_eq!(unload(&connection), None);
    }
}
