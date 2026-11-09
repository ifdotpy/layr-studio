//! Receipt/account descriptors retain the current physical format and exact evidence.
use crate::json_record_codec::{ExactJson, ExactJsonCodec, JsonRecordCodec, RawJsonRecord};
use crate::peer_update_settings::Canvas;
use rusqlite::types::{FromSql, ToSql, ValueRef};
use std::marker::PhantomData;
use studio_db_api::{Codec, DbTable, Table};
use studio_runtime_schema::persistence::{
    IncidentKind, OutboxKind, OutboxStatus, OwnerKind, RequestReferenceKind, RequestResultStatus,
};

/// Existing TEXT enum spelling is independent of a future INTEGER cutover.
pub struct LegacyEnumCodec<T>(PhantomData<T>);
impl<T: FromSql + ToSql> Codec<T> for LegacyEnumCodec<T> {
    type Stored = String;
    type Encoded<'a>
        = &'a T
    where
        T: 'a;
    fn encode<'a>(_: &rusqlite::Connection, value: &'a T) -> rusqlite::Result<&'a T> {
        Ok(value)
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<T> {
        T::column_result(ValueRef::Text(value.as_bytes())).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }
}
pub struct NullableLegacyEnumCodec<T>(PhantomData<T>);
impl<T: FromSql + ToSql> Codec<Option<T>> for NullableLegacyEnumCodec<T> {
    type Stored = Option<String>;
    type Encoded<'a>
        = Option<&'a T>
    where
        T: 'a;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a Option<T>,
    ) -> rusqlite::Result<Option<&'a T>> {
        Ok(value.as_ref())
    }
    fn decode(
        connection: &rusqlite::Connection,
        value: Option<String>,
    ) -> rusqlite::Result<Option<T>> {
        value
            .map(|value| LegacyEnumCodec::<T>::decode(connection, value))
            .transpose()
    }
}

// Polymorphic provider/reply evidence is interpreted by its existing family caller.
// These named envelopes retain original JSON tokens, including unknown members.
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// `request_results::ResultEnvelope<R>` and caller-specific exact replay bodies.
pub struct RequestResultPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// `repositories::outbox::OutboxRow<P>`; each service effect owns its payload.
pub struct OutboxPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Services `account_worker::store::AccountStreams`.
pub struct AccountStreamsPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Services `liveness::SessionFailure`.
pub struct AccountStartFailurePayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Services `notification_dispatch::effects::NotificationPostCommit`.
pub struct AnalyticsProjectionPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Services `safety_buffering::executor::SafetyOperation`.
pub struct SafetyRetryPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Services `limit_reset::Attempt`.
pub struct LimitResetAttemptPayload(pub studio_json::OwnedRawJson);
#[derive(serde::Deserialize)]
#[serde(transparent)]
/// Schema `studio_errors::StudioError`, or null for dismissal-only rows.
pub struct IncidentPayload(pub studio_json::OwnedRawJson);

#[derive(DbTable)]
#[db(table = "request_results", database = Canvas, legacy_ddl = "CREATE TABLE request_results (\n           id TEXT PRIMARY KEY,\n           signature TEXT NOT NULL,\n           status TEXT NOT NULL DEFAULT 'applied',\n           ref_kind TEXT,\n           ref_id TEXT,\n           ref_rev INTEGER,\n           result TEXT,\n           created INTEGER NOT NULL,\n           CHECK ((result IS NOT NULL AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL)\n               OR (result IS NULL AND ref_kind IS NOT NULL AND ref_id IS NOT NULL AND ref_rev IS NOT NULL)\n               OR (result IS NULL AND status = 'rejected' AND ref_kind IS NULL AND ref_id IS NULL AND ref_rev IS NULL))\n         )", receipt)]
#[db(legacy_index = "CREATE INDEX request_results_created ON request_results(created,id)")]
#[db(
    legacy_index = "CREATE INDEX request_results_tool_actor_created\n            ON request_results(json_extract(result,'$.actor'),created DESC,id)\n            WHERE json_extract(result,'$.actor') IS NOT NULL"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<RequestResultsRow>)]
pub struct RequestResultsRow {
    #[db(key)]
    pub id: String,
    pub signature: String,
    #[db(codec = LegacyEnumCodec<RequestResultStatus>)]
    pub status: RequestResultStatus,
    #[db(codec = NullableLegacyEnumCodec<RequestReferenceKind>)]
    pub ref_kind: Option<RequestReferenceKind>,
    pub ref_id: Option<String>,
    pub ref_rev: Option<i64>,
    #[db(codec = ExactJsonCodec<RequestResultPayload>)]
    pub result: Option<ExactJson<RequestResultPayload>>,
    pub created: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_outbox", database = Canvas, legacy_ddl = "CREATE TABLE runtime_outbox (\n            id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(\"kind\" IN ('provider_call','provider_callback','journal_entry','publication','stream_flush','followup_effect','steer','voice_stop','voice_delete','work_archive','work_vm','title_generation','native_action','terminal_action','monitor_input','mcp_reconnect','project_registration','remote_delivery','managed_tool')), owner_kind TEXT NOT NULL CHECK(\"owner_kind\" IN ('agent','task','account','vm','project','monitor','terminal','message','request','server')),\n            owner_id TEXT NOT NULL, request_id TEXT, signature TEXT NOT NULL,\n            status TEXT NOT NULL CHECK(\"status\" IN ('pending','submitted','unknown','committed','cancelled','unrecognized')), account TEXT, stream_id TEXT, generation INTEGER,\n            ordinal INTEGER, next_at INTEGER, payload TEXT NOT NULL, created INTEGER NOT NULL, target TEXT,\n            CHECK((status!='unknown' AND status!='unrecognized' AND status!='cancelled') OR next_at IS NULL))")]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_pending ON runtime_outbox(kind,status,next_at,id) WHERE status='pending'"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_owner ON runtime_outbox(owner_kind,owner_id,kind,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_account ON runtime_outbox(account,kind,status,ordinal,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_stream ON runtime_outbox(stream_id,generation,ordinal,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_stream_ordered ON runtime_outbox(stream_id,kind,ordinal,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_outbox_request ON runtime_outbox(request_id,id) WHERE request_id IS NOT NULL"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<OutboxStorageRow>)]
#[db(legacy_index = "CREATE INDEX runtime_outbox_kind_next_at ON runtime_outbox(kind,next_at)")]
pub struct OutboxStorageRow {
    #[db(key)]
    pub id: String,
    #[db(codec = LegacyEnumCodec<OutboxKind>)]
    pub kind: OutboxKind,
    #[db(codec = LegacyEnumCodec<OwnerKind>)]
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub request_id: Option<String>,
    pub signature: String,
    #[db(codec = LegacyEnumCodec<OutboxStatus>)]
    pub status: OutboxStatus,
    pub account: Option<String>,
    pub stream_id: Option<String>,
    pub generation: Option<i64>,
    pub ordinal: Option<i64>,
    pub next_at: Option<i64>,
    #[db(codec = ExactJsonCodec<OutboxPayload>)]
    pub payload: ExactJson<OutboxPayload>,
    pub created: i64,
    pub target: Option<String>,
}

#[derive(DbTable)]
#[db(table = "runtime_account_state", database = Canvas, legacy_ddl = "CREATE TABLE runtime_account_state (\n            account TEXT PRIMARY KEY, next_generation INTEGER NOT NULL, connection_generation INTEGER,\n            connection_id TEXT, journal_ack INTEGER NOT NULL, recovery_required INTEGER NOT NULL,\n            streams TEXT NOT NULL, limits_observation TEXT, start_failure TEXT)")]
#[db(state_check = crate::state_check::count_legacy_rows::<AccountStateRow>)]
pub struct AccountStateRow {
    #[db(key)]
    pub account: String,
    pub next_generation: i64,
    pub connection_generation: Option<i64>,
    pub connection_id: Option<String>,
    pub journal_ack: i64,
    pub recovery_required: bool,
    #[db(codec = ExactJsonCodec<AccountStreamsPayload>)]
    pub streams: ExactJson<AccountStreamsPayload>,
    #[db(codec = ExactJsonCodec<crate::account_state::LimitsStamp>)]
    pub limits_observation: Option<ExactJson<crate::account_state::LimitsStamp>>,
    #[db(codec = ExactJsonCodec<AccountStartFailurePayload>)]
    pub start_failure: Option<ExactJson<AccountStartFailurePayload>>,
}

#[derive(DbTable)]
#[db(table = "runtime_account_transfers", database = Canvas, legacy_ddl = "CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY, record TEXT NOT NULL,\"target_account_key\" GENERATED ALWAYS AS (CASE WHEN json_valid(record) THEN record->>'$.targetAccountKey' END) VIRTUAL)")]
#[db(
    legacy_index = "CREATE INDEX runtime_account_transfer_status ON runtime_account_transfers(json_extract(record,'$.status'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_transfer_target_account_key ON runtime_account_transfers(json_extract(record,'$.status'),target_account_key)"
)]
#[db(state_check = crate::state_check::check_legacy_json::<crate::AccountTransferRecord>, check = check_transfer_key)]
pub struct AccountTransferRow {
    #[db(key)]
    pub id: String,
    #[db(codec = JsonRecordCodec<crate::AccountTransferRecord>)]
    pub record: RawJsonRecord<crate::AccountTransferRecord>,
    #[db(generated)]
    pub target_account_key: Option<String>,
}

#[derive(DbTable)]
#[db(table = "runtime_analytics_projection", database = Canvas, legacy_ddl = "CREATE TABLE runtime_analytics_projection (seq INTEGER PRIMARY KEY AUTOINCREMENT, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<AnalyticsProjectionRow>)]
pub struct AnalyticsProjectionRow {
    #[db(key)]
    #[db(generated)]
    pub seq: i64,
    #[db(codec = ExactJsonCodec<AnalyticsProjectionPayload>)]
    pub record: ExactJson<AnalyticsProjectionPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_capacity_retries", database = Canvas, legacy_ddl = "CREATE TABLE runtime_capacity_retries(id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::check_legacy_json::<crate::repositories::continuation::CapacityContinuationRecord>, check = check_capacity_key)]
pub struct CapacityRetryRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec = JsonRecordCodec<crate::repositories::continuation::CapacityContinuationRecord>)]
    pub record: RawJsonRecord<crate::repositories::continuation::CapacityContinuationRecord>,
}

#[derive(DbTable)]
#[db(table = "runtime_safety_retries", database = Canvas, legacy_ddl = "CREATE TABLE runtime_safety_retries (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<SafetyRetryRow>)]
pub struct SafetyRetryRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<SafetyRetryPayload>)]
    pub record: ExactJson<SafetyRetryPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_usage_resumes", database = Canvas, legacy_ddl = "CREATE TABLE runtime_usage_resumes(id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL)")]
#[db(legacy_index = "CREATE INDEX runtime_usage_resume_agent ON runtime_usage_resumes(agent)")]
#[db(
    legacy_index = "CREATE INDEX runtime_usage_resume_due ON runtime_usage_resumes(json_extract(record,'$.status'),json_extract(record,'$.dueAt'))"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_usage_resume_account ON runtime_usage_resumes(json_extract(record,'$.accountKey'),json_extract(record,'$.status'))"
)]
#[db(state_check = crate::state_check::check_legacy_json::<crate::UsageResumeRecord>)]
pub struct UsageResumeRow {
    #[db(key)]
    pub id: String,
    pub agent: String,
    #[db(codec = ExactJsonCodec<crate::UsageResumeRecord>)]
    pub record: ExactJson<crate::UsageResumeRecord>,
}

#[derive(DbTable)]
#[db(table = "runtime_limit_reset_attempts", database = Canvas, legacy_ddl = "CREATE TABLE runtime_limit_reset_attempts (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<LimitResetAttemptRow>)]
pub struct LimitResetAttemptRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<LimitResetAttemptPayload>)]
    pub record: ExactJson<LimitResetAttemptPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_limit_reset_requests", database = Canvas, legacy_ddl = "CREATE TABLE runtime_limit_reset_requests (id TEXT PRIMARY KEY, attempt_id TEXT NOT NULL)")]
#[db(state_check = crate::state_check::count_legacy_rows::<LimitResetRequestRow>)]
pub struct LimitResetRequestRow {
    #[db(key)]
    pub id: String,
    pub attempt_id: String,
}

#[derive(DbTable)]
#[db(table = "runtime_error_incidents", database = Canvas, legacy_ddl = "CREATE TABLE runtime_error_incidents (\n            id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(\"kind\" IN ('monitor_outcome_lost','monitor_not_recorded','input_unconfirmed','input_uncertain','input_stuck','turn_disconnected','workspace_failed','http_failure','recovery_failed','stored_record_invalid','unclassified_failure','turn_stalled','input_not_started','session_not_started','schedule_stalled')), owner_kind TEXT NOT NULL CHECK(\"owner_kind\" IN ('agent','task','account','vm','project','monitor','terminal','message','request','server')),\n            owner_id TEXT NOT NULL, created INTEGER NOT NULL, last_seen INTEGER NOT NULL,\n            dismissed_at INTEGER, payload TEXT NOT NULL)")]
#[db(
    legacy_index = "CREATE INDEX runtime_error_incidents_created ON runtime_error_incidents(created DESC,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_error_incidents_owner ON runtime_error_incidents(owner_kind,owner_id,created DESC,id)"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_error_incidents_open ON runtime_error_incidents(last_seen DESC,id) WHERE dismissed_at IS NULL"
)]
#[db(
    legacy_index = "CREATE INDEX runtime_error_incidents_dismissed ON runtime_error_incidents(id,dismissed_at) WHERE dismissed_at IS NOT NULL"
)]
#[db(state_check = crate::state_check::count_legacy_rows::<ErrorIncidentRow>)]
pub struct ErrorIncidentRow {
    #[db(key)]
    pub id: String,
    #[db(codec = LegacyEnumCodec<IncidentKind>)]
    pub kind: IncidentKind,
    #[db(codec = LegacyEnumCodec<OwnerKind>)]
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub created: i64,
    pub last_seen: i64,
    pub dismissed_at: Option<i64>,
    #[db(codec = ExactJsonCodec<IncidentPayload>)]
    pub payload: ExactJson<IncidentPayload>,
}

fn check_transfer_key(row: &AccountTransferRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}
fn check_capacity_key(row: &CapacityRetryRow) -> crate::StoreResult<()> {
    row.record.check_key(&row.id)
}

pub(crate) fn prepare_features(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::canvas_schema::install(
        connection,
        &[
            // Oct 30 prepares features before Nov 6 adds target_account_key.
            studio_db_api::Descriptor {
                indexes: &AccountTransferRow::INDEXES[..1],
                ..AccountTransferRow::descriptor()
            },
            AnalyticsProjectionRow::descriptor(),
            CapacityRetryRow::descriptor(),
            SafetyRetryRow::descriptor(),
            UsageResumeRow::descriptor(),
            LimitResetAttemptRow::descriptor(),
            LimitResetRequestRow::descriptor(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Catalog {
        #[serde(rename = "canvas.sqlite3")]
        canvas: Database,
    }
    #[derive(serde::Deserialize)]
    struct Database {
        sqlite_master: Vec<Object>,
    }
    #[derive(serde::Deserialize)]
    struct Object {
        name: String,
        sql: String,
        table: String,
        create_order: i64,
        r#type: String,
    }

    fn descriptors() -> [studio_db_api::Descriptor; 11] {
        [
            RequestResultsRow::descriptor(),
            OutboxStorageRow::descriptor(),
            AccountStateRow::descriptor(),
            AccountTransferRow::descriptor(),
            AnalyticsProjectionRow::descriptor(),
            CapacityRetryRow::descriptor(),
            SafetyRetryRow::descriptor(),
            UsageResumeRow::descriptor(),
            LimitResetAttemptRow::descriptor(),
            LimitResetRequestRow::descriptor(),
            ErrorIncidentRow::descriptor(),
        ]
    }

    #[test]
    fn receipt_descriptors_preserve_catalog_sql_and_feature_installation() {
        let catalog: Catalog =
            studio_json::from_str(include_str!("runtime-state-schema.json")).unwrap();
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        crate::feature_schema::prepare(&tx).unwrap();
        crate::receipt_schema::create(&tx).unwrap();
        for descriptor in descriptors() {
            for (name, sql) in std::iter::once((descriptor.name, descriptor.ddl)).chain(
                descriptor
                    .indexes
                    .iter()
                    .map(|index| (index.name, index.sql)),
            ) {
                let wanted = catalog
                    .canvas
                    .sqlite_master
                    .iter()
                    .find(|entry| entry.name == name)
                    .unwrap();
                assert_eq!(sql, wanted.sql, "{name} catalog parity");
                let actual: String = tx
                    .query_row(
                        "SELECT sql FROM sqlite_schema WHERE name=?1",
                        [name],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(actual, wanted.sql, "{name} fresh/feature parity");
            }
        }
    }

    #[test]
    fn receipt_descriptors_keep_original_index_and_trigger_order() {
        let catalog: Catalog =
            studio_json::from_str(&crate::storage_catalog::generate().unwrap()).unwrap();
        for (table, wanted) in [
            (
                RequestResultsRow::NAME,
                &[
                    "request_results_created",
                    "request_results_tool_actor_created",
                    "enum_check_request_results_status_ins",
                    "enum_check_request_results_status_upd",
                    "enum_check_request_results_ref_kind_ins",
                    "enum_check_request_results_ref_kind_upd",
                ][..],
            ),
            (
                OutboxStorageRow::NAME,
                &[
                    "runtime_outbox_pending",
                    "runtime_outbox_owner",
                    "runtime_outbox_account",
                    "runtime_outbox_stream",
                    "runtime_outbox_stream_ordered",
                    "runtime_outbox_request",
                    "runtime_outbox_kind_next_at",
                ][..],
            ),
            (
                AccountTransferRow::NAME,
                &[
                    "runtime_account_transfer_status",
                    "runtime_transfer_target_account_key",
                ][..],
            ),
            (
                UsageResumeRow::NAME,
                &[
                    "runtime_usage_resume_agent",
                    "runtime_usage_resume_due",
                    "runtime_usage_resume_account",
                ][..],
            ),
            (
                ErrorIncidentRow::NAME,
                &[
                    "runtime_error_incidents_created",
                    "runtime_error_incidents_owner",
                    "runtime_error_incidents_open",
                    "runtime_error_incidents_dismissed",
                ][..],
            ),
        ] {
            let mut objects = catalog
                .canvas
                .sqlite_master
                .iter()
                .filter(|object| {
                    object.table == table && matches!(object.r#type.as_str(), "index" | "trigger")
                })
                .collect::<Vec<_>>();
            objects.sort_by_key(|object| object.create_order);
            assert_eq!(
                objects
                    .iter()
                    .map(|object| object.name.as_str())
                    .collect::<Vec<_>>(),
                wanted,
                "{table}"
            );
        }
    }

    #[test]
    fn receipt_registry_keeps_count_only_and_legacy_json_reports() {
        for descriptor in descriptors() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch(descriptor.ddl).unwrap();
            // Intentionally bypass indexes: historical unread payloads remain count-only.
            let checked = matches!(
                descriptor.name,
                "runtime_account_transfers" | "runtime_capacity_retries" | "runtime_usage_resumes"
            );
            let fixture = match descriptor.name {
                "request_results" => {
                    "INSERT INTO request_results(id,signature,result,created) VALUES(NULL,'s','not json',1)"
                }
                "runtime_outbox" => {
                    "INSERT INTO runtime_outbox(id,kind,owner_kind,owner_id,signature,status,payload,created) VALUES(NULL,'provider_call','agent','a','s','unknown','not json',1)"
                }
                "runtime_account_state" => {
                    "INSERT INTO runtime_account_state(account,next_generation,journal_ack,recovery_required,streams,limits_observation,start_failure) VALUES(NULL,0,0,7,'not json','not json','not json')"
                }
                "runtime_analytics_projection" => {
                    "INSERT INTO runtime_analytics_projection(record) VALUES('not json')"
                }
                "runtime_capacity_retries" | "runtime_usage_resumes" => "",
                "runtime_limit_reset_requests" => {
                    "INSERT INTO runtime_limit_reset_requests VALUES(NULL,'attempt')"
                }
                "runtime_error_incidents" => {
                    "INSERT INTO runtime_error_incidents(id,kind,owner_kind,owner_id,created,last_seen,payload) VALUES(NULL,'http_failure','agent','a',1,1,'not json')"
                }
                _ => "",
            };
            if fixture.is_empty() {
                let columns = if matches!(
                    descriptor.name,
                    "runtime_capacity_retries" | "runtime_usage_resumes"
                ) {
                    "id,agent,record"
                } else {
                    "id,record"
                };
                let values = if columns == "id,agent,record" {
                    "'row','agent','not json'"
                } else {
                    "'row','not json'"
                };
                db.execute_batch(&format!(
                    "INSERT INTO {}({columns}) VALUES({values})",
                    descriptor.name
                ))
                .unwrap();
            } else {
                db.execute_batch(fixture).unwrap();
            }
            let registration = crate::canvas_schema::registry()
                .into_iter()
                .find(|entry| entry.descriptor.name == descriptor.name)
                .unwrap();
            let report = (registration.check)(&db).unwrap();
            assert_eq!(
                report.counts(),
                (1, u64::from(checked)),
                "{}",
                descriptor.name
            );
            if checked {
                assert_eq!(report.failures[0].keyword, "json");
            } else {
                assert!(report.failures.is_empty());
            }
        }
    }

    #[test]
    fn receipt_raw_envelopes_keep_duplicate_keys_integral_decimals_and_exact_bytes() {
        let raw = " \n{\"future\":false,\"future\":{\"counter\":1.0,\"evidence\":[1,2.00]}}\t";
        let db = rusqlite::Connection::open_in_memory().unwrap();
        macro_rules! witness {
            ($($owner:ty),+) => { $(
                let record = ExactJson::<$owner>::from_stored(raw.into()).unwrap();
                assert_eq!(record.raw(), raw);
                assert_eq!(<ExactJsonCodec<$owner> as Codec<ExactJson<$owner>>>::encode(&db, &record).unwrap(), raw);
                let fields: std::collections::BTreeMap<String, studio_json::OwnedRawJson> = studio_json::from_str(record.value().0.as_str()).unwrap();
                assert_eq!(fields["future"].as_str(), r#"{"counter":1.0,"evidence":[1,2.00]}"#);
            )+ };
        }
        witness!(
            RequestResultPayload,
            OutboxPayload,
            AccountStreamsPayload,
            AccountStartFailurePayload,
            AnalyticsProjectionPayload,
            SafetyRetryPayload,
            LimitResetAttemptPayload,
            IncidentPayload
        );
    }

    #[test]
    fn receipt_scalar_replay_does_not_decode_unread_evidence() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        // The signature conflict must win over malformed historical result/ref-kind.
        tx.execute("DROP TRIGGER enum_check_request_results_ref_kind_ins", [])
            .unwrap();
        tx.execute("DROP INDEX request_results_tool_actor_created", [])
            .unwrap();
        tx.execute("INSERT INTO request_results(id,signature,result,created) VALUES('saved','original','not json',1)", []).unwrap();
        let admitted =
            crate::receipt_migration_contract::admitted_owner_token("saved", "changed", 1);
        let reference = crate::request_results::RequestReference {
            kind: RequestReferenceKind::Agent,
            id: "agent".into(),
            revision: 1,
        };
        assert!(matches!(
            crate::request_results::RequestResults::settle_reference(&tx, &admitted, &reference),
            Err(crate::request_results::RequestResultError::Conflict { .. })
        ));
    }
}
