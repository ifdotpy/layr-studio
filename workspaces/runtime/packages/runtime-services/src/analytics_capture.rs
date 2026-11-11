//! Pure analytics capture decisions and the synchronous notification consumer.
//!
//! Prepared payload measurements happen before the writer is held. A checked
//! snapshot selects only the affected rows; no unbounded history is loaded.
use crate::analytics_measurements::{item_sizes, payload_size};
use crate::notification_dispatch::NotificationStreamMetrics;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::sync::Arc;
use studio_json::{Value, json};
use studio_runtime_schema::json_compat::python_json_compact_serialized;
use studio_runtime_schema::provider_kind::ProviderKind;
use studio_runtime_schema::usage::ledger::{
    UsageCorrection, UsageIdentity, UsageLedgerRow, correction,
};
use studio_runtime_schema::usage::{
    LegacyUsageTotals, UsageObservation, UsageParseError, UsageScope, UsageTokens,
    parse_analytics_usage_row, parse_legacy_totals, parse_token_usage,
};
use studio_runtime_store::CachedSql;
use studio_runtime_store::analytics::Notification;
use studio_runtime_store::analytics_capture::{AnalyticsCaptureRepository as Repo, CaptureTable};
use studio_runtime_store::{RuntimeStore, StoreError, sha256_hex};
use thiserror::Error;

pub const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureSource {
    Live,
    Rollout,
}
impl CaptureSource {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Rollout => "rollout",
        }
    }
}
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("invalid analytics input at {0}")]
    Invalid(&'static str),
    #[error("analytics capture belongs to a stale agent account, epoch or thread")]
    StaleIdentity,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("analytics SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
impl From<UsageParseError> for CaptureError {
    fn from(error: UsageParseError) -> Self {
        Self::Store(StoreError::SchemaValidation(error.to_string()))
    }
}
#[derive(Clone)]
pub struct PreparedCapture {
    agent: Arc<Value>,
    method: String,
    params: Value,
    at: f64,
    recorded_at: f64,
    source: CaptureSource,
    input: Value,
    output: Value,
    stream: Value,
    model: bool,
    usage: Option<UsageObservation>,
    legacy_usage: Option<LegacyUsageTotals>,
    rejected_usage: Option<studio_runtime_schema::usage::ledger::UsageReplayConflict>,
    ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    unknown_usage_key: Option<String>,
    boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    first_delta_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
}
impl PreparedCapture {
    pub fn at_provider_boundary(
        mut self,
        stamp: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    ) -> Result<Self, CaptureError> {
        if stamp.is_some_and(|stamp| !stamp.unix_seconds.is_finite() || stamp.unix_seconds < 0.) {
            return Err(CaptureError::Invalid("/boundaryAt"));
        }
        self.boundary_at = stamp;
        self.first_delta_at = (self.method == "item/modelOutput/firstDelta"
            || self.params["delta"]
                .as_str()
                .is_some_and(|delta| !delta.is_empty()))
        .then_some(stamp)
        .flatten();
        Ok(self)
    }
    pub fn event(
        agent: &Value,
        method: &str,
        params: &Value,
        at: f64,
        recorded_at: f64,
        source: CaptureSource,
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<Self, CaptureError> {
        CaptureTable::Agents.validate(agent)?;
        Self::event_validated(
            Arc::new(agent.clone()),
            method,
            params,
            at,
            recorded_at,
            source,
            ledger_since,
        )
    }

    pub(crate) fn notification_event(
        agent: &Value,
        method: &str,
        params: &Value,
        at: f64,
        recorded_at: f64,
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<Self, CaptureError> {
        Self::event_validated(
            Arc::new(agent.clone()),
            method,
            params,
            at,
            recorded_at,
            CaptureSource::Live,
            ledger_since,
        )
    }

    pub(crate) fn event_shared(
        agent: Arc<Value>,
        method: &str,
        params: &Value,
        at: f64,
        recorded_at: f64,
        source: CaptureSource,
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<Self, CaptureError> {
        CaptureTable::Agents.validate(&agent)?;
        Self::event_validated(agent, method, params, at, recorded_at, source, ledger_since)
    }

    fn event_validated(
        agent: Arc<Value>,
        method: &str,
        params: &Value,
        at: f64,
        recorded_at: f64,
        source: CaptureSource,
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<Self, CaptureError> {
        let provider = ProviderKind::of_record(&agent)
            .map_err(|_| CaptureError::Invalid("/agent/provider"))?;
        if !params.is_object() {
            return Err(CaptureError::Invalid("/params"));
        }
        if !at.is_finite() || at < 0. || !recorded_at.is_finite() || recorded_at < 0. {
            return Err(CaptureError::Invalid("/at"));
        }
        if encoded_len(params) > MAX_CAPTURE_BYTES {
            return Err(CaptureError::Invalid("/params/size"));
        }
        if let Some((pointer, _)) = studio_runtime_schema::first_error("AnalyticsNotice", params)
            .map_err(StoreError::SchemaValidation)?
        {
            return Err(StoreError::SchemaValidation(format!("analytics_notice{pointer}")).into());
        }
        let (usage, legacy_usage) = if method == "thread/tokenUsage/updated" {
            match parse_token_usage(provider, params) {
                Ok(observation) => (Some(observation), None),
                Err(error) => match parse_legacy_totals(provider, params) {
                    Ok(legacy) => (None, Some(legacy)),
                    Err(_) => return Err(error.into()),
                },
            }
        } else {
            (None, None)
        };
        let (input, output) = item_sizes(
            params["item"]["type"].as_str().unwrap_or(""),
            &params["item"],
        );
        Ok(Self {
            agent,
            method: method.to_owned(),
            params: params.clone(),
            at,
            recorded_at,
            source,
            input,
            output,
            stream: payload_size(&params["delta"]),
            model: false,
            usage,
            legacy_usage,
            rejected_usage: None,
            ledger_since,
            unknown_usage_key: None,
            boundary_at: None,
            first_delta_at: None,
        })
    }
    pub(crate) fn model_payload_shared(
        agent: Arc<Value>,
        params: &Value,
        at: f64,
        recorded_at: f64,
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<Self, CaptureError> {
        let mut result = Self::event_shared(
            agent,
            "model/payload",
            params,
            at,
            recorded_at,
            CaptureSource::Rollout,
            ledger_since,
        )?;
        result.model = true;
        let kind = params["type"].as_str().unwrap_or("unknown");
        if is_model_input(kind) {
            result.input = payload_size(
                params
                    .get("arguments")
                    .or_else(|| params.get("input"))
                    .unwrap_or(&Value::Null),
            );
        } else if is_model_output(kind) {
            result.output = payload_size(
                params
                    .get("output")
                    .or_else(|| params.get("content"))
                    .unwrap_or(&Value::Null),
            );
        } else {
            let size = payload_size(
                params
                    .get("content")
                    .or_else(|| params.get("summary"))
                    .unwrap_or(&Value::Null),
            );
            if matches!(
                params["role"].as_str(),
                Some("user" | "system" | "developer")
            ) {
                result.input = size
            } else {
                result.output = size
            }
        }
        Ok(result)
    }
    pub(crate) fn bind_budget(
        &mut self,
        analytics: &Connection,
        epoch: studio_runtime_schema::usage::ledger::LedgerEpoch,
    ) -> Result<(), CaptureError> {
        self.ledger_since = epoch;
        self.unknown_usage_key = unknown_usage_receipt(
            analytics,
            text(&self.agent["id"]),
            self.params["threadId"]
                .as_str()
                .or_else(|| self.agent["threadId"].as_str()),
            self.usage.as_ref(),
            self.legacy_usage.as_ref(),
        )?;
        Ok(())
    }
    fn legacy_counter_era(&self) -> bool {
        (matches!(self.usage, Some(UsageObservation::Cumulative { .. }))
            || self
                .legacy_usage
                .as_ref()
                .is_some_and(|legacy| legacy.response_id.is_none()))
            && studio_runtime_schema::usage::ledger::legacy_era(
                self.ledger_since,
                self.at,
                self.params["_analyticsTimestampSource"] == "fileModifiedEstimate",
            )
    }

    pub fn agent(&self) -> &Value {
        &self.agent
    }
    fn meta(&self) -> Value {
        let a = &self.agent;
        let mut meta = project(
            a,
            &[
                "accountKey",
                "accountHistory",
                "provider",
                "threadId",
                "model",
                "effort",
                "fastMode",
                "daybreakEnabled",
                "cyberAccessProgram",
            ],
        );
        meta["agentId"] = a["id"].clone();
        meta["agentName"] = a["name"].clone();
        meta["rootId"] = first(&[&a["rootId"], &a["id"]]);
        meta["accountKey"] = a.get("accountKey").cloned().unwrap_or(json!("default"));
        if !meta["accountHistory"].is_array() {
            meta["accountHistory"] = json!([])
        }
        if self.source != CaptureSource::Live && !self.model {
            meta["daybreakEnabled"] = Value::Null;
            meta["cyberAccessProgram"] = Value::Null;
        }
        meta["threadId"] = first(&[&self.params["threadId"], &a["threadId"]]);
        meta["turnId"] = if self.model {
            self.params["_analyticsTurnId"].clone()
        } else {
            first(&[
                &self.params["turnId"],
                &self.params["turn"]["id"],
                &a["turnId"],
            ])
        };
        if let Some(
            UsageObservation::Response {
                turn_id: Some(turn),
                ..
            }
            | UsageObservation::Cumulative {
                turn_id: Some(turn),
                ..
            }
            | UsageObservation::Gap {
                turn_id: Some(turn),
                ..
            },
        ) = &self.usage
        {
            meta["turnId"] = json!(turn);
        }
        if let Some(legacy) = &self.legacy_usage
            && let Some(turn) = &legacy.turn_id
        {
            meta["turnId"] = json!(turn);
        }
        meta
    }
    fn item_key(&self, meta: &Value) -> Option<String> {
        if self.method == "analytics/compaction" {
            return Some(key(
                meta,
                &first(&[
                    &self.params["id"],
                    &self.params["_analyticsId"],
                    &json!(self.at.to_string()),
                ]),
            ));
        }
        if self.model {
            let p = &self.params;
            let call = first(&[&p["call_id"], &p["callId"], &p["id"], &p["_analyticsId"]]);
            if !studio_runtime_store::truthy(Some(&call)) {
                return None;
            }
            let category = model_category(p);
            let identity =
                if category == "compactionReplacement" || category == "compactionGuardian" {
                    first(&[&p["_analyticsId"], &call])
                } else {
                    call
                };
            Some(format!(
                "{}:{}:model:{}:{}",
                text(&meta["agentId"]),
                text(&meta["threadId"]),
                text(&identity),
                category
            ))
        } else {
            let id = first(&[&self.params["item"]["id"], &self.params["itemId"]]);
            studio_runtime_store::truthy(Some(&id)).then(|| key(meta, &id))
        }
    }
}
/// Raw database rows are accepted only through `from_records`.
#[derive(Default)]
struct CaptureRows {
    pub turn: Option<Value>,
    pub item: Option<Value>,
    pub usage: Option<Value>,
    pub collision_usage: Option<Value>,
    pub exact_response: Option<Value>,
    pub prior_usage: Option<Value>,
    pub last_limit: Option<Value>,
}
pub struct CaptureSnapshot {
    prepared: std::sync::Arc<PreparedCapture>,
    meta: Value,
    rows: CaptureRows,
}
impl CaptureSnapshot {
    fn from_records(
        prepared: std::sync::Arc<PreparedCapture>,
        rows: CaptureRows,
    ) -> Result<Self, CaptureError> {
        for (table, row) in [
            (CaptureTable::Turns, &rows.turn),
            (CaptureTable::Items, &rows.item),
            (CaptureTable::Usage, &rows.usage),
            (CaptureTable::Usage, &rows.collision_usage),
            (CaptureTable::Usage, &rows.exact_response),
            (CaptureTable::Usage, &rows.prior_usage),
        ] {
            if let Some(row) = row {
                table.validate(row)?;
                if table == CaptureTable::Usage && row.get("studioUsage").is_some() {
                    UsageLedgerRow::from_typed_analytics(row)?;
                }
            }
        }
        let meta = prepared.meta();
        for row in [&rows.turn, &rows.item].into_iter().flatten() {
            if row["agentId"] != meta["agentId"]
                || row["threadId"] != meta["threadId"]
                || row["rootId"] != meta["rootId"]
            {
                return Err(CaptureError::Invalid("/snapshot/row/identity"));
            }
        }
        for row in [
            &rows.usage,
            &rows.collision_usage,
            &rows.exact_response,
            &rows.prior_usage,
        ]
        .into_iter()
        .flatten()
        {
            if row["agentId"] != meta["agentId"] {
                return Err(CaptureError::Invalid("/snapshot/row/identity"));
            }
        }
        // Python keys items by agent, thread and item id (`analytics_event`):
        // a command started in one turn still receives terminal input and
        // output deltas in later turns. Only the turn row is turn-scoped.
        if let Some(row) = &rows.turn
            && row["turnId"] != meta["turnId"]
        {
            return Err(CaptureError::Invalid("/snapshot/row/turnId"));
        }
        Ok(Self {
            prepared,
            meta,
            rows,
        })
    }
    pub fn load(
        db: &Connection,
        prepared: std::sync::Arc<PreparedCapture>,
    ) -> Result<Self, CaptureError> {
        let meta = prepared.meta();
        let turn = studio_runtime_store::truthy(Some(&meta["turnId"]))
            .then(|| Repo::read(db, CaptureTable::Turns, &key(&meta, &meta["turnId"])))
            .transpose()?
            .flatten();
        let item = prepared
            .item_key(&meta)
            .map(|id| Repo::read(db, CaptureTable::Items, &id))
            .transpose()?
            .flatten();
        let mut rows = CaptureRows {
            turn,
            item,
            ..CaptureRows::default()
        };
        if let Some(observation) = &prepared.usage {
            let id = usage_key(&meta, observation);
            rows.usage = Repo::read(db, CaptureTable::Usage, &id)?;
            if rows.usage.is_none() && prepared.legacy_counter_era() {
                let old_id = hash(&(
                    &meta["agentId"],
                    &meta["threadId"],
                    fingerprint(observation),
                ));
                rows.usage = Repo::read(db, CaptureTable::Usage, &old_id)?;
            }
            if let UsageObservation::Response { response_id, .. }
            | UsageObservation::Gap { response_id, .. } = observation
            {
                let response = Repo::read(
                    db,
                    CaptureTable::Usage,
                    &hash(&(&meta["agentId"], response_id)),
                )?;
                rows.exact_response = match response {
                    Some(row) => Some(row),
                    None => Repo::usage_response(
                        db,
                        text(&meta["agentId"]),
                        meta["threadId"].as_str(),
                        response_id,
                    )?,
                };
            }
            rows.prior_usage = Repo::usage_prior(
                db,
                text(&meta["agentId"]),
                meta["threadId"].as_str(),
                prepared.at,
                domain(observation),
            )?;
        }
        if let Some(legacy) = &prepared.legacy_usage {
            rows.usage = Repo::read(
                db,
                CaptureTable::Usage,
                &legacy_usage_key(&meta, legacy, &prepared.params),
            )?;
            if rows.usage.is_none() && prepared.legacy_counter_era() {
                rows.usage = Repo::read(
                    db,
                    CaptureTable::Usage,
                    &old_legacy_usage_key(&meta, legacy, &prepared.params),
                )?;
            }
        }
        if prepared.method == "analytics/rateLimits" {
            rows.last_limit = Repo::last_limit(db, meta["accountKey"].as_str())?;
        }
        Self::from_records(prepared, rows)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseRateAssociation {
    pub agent: String,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub response: String,
    pub output_tokens: f64,
}
#[derive(Default)]
pub struct CaptureDecision {
    unknown_usage: Option<UnknownUsageReceipt>,
    pub usage_conflict: Option<studio_runtime_schema::usage::ledger::UsageReplayConflict>,
    pub rows: Vec<(CaptureTable, Value)>,
    notifications: Vec<Notification>,
    pub limit: Option<Value>,
    pub generation_roots: Vec<String>,
    pub response_rate: Option<ResponseRateAssociation>,
    pub response_replay_conflict: bool,
}
enum UnknownUsageOwner {
    Agent(String),
    Identity(UsageIdentity),
}
struct UnknownUsageReceipt {
    owner: UnknownUsageOwner,
    observation_key: String,
}
impl serde::Serialize for UnknownUsageReceipt {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(3))?;
        match &self.owner {
            UnknownUsageOwner::Agent(agent) => map.serialize_entry("agent", agent)?,
            UnknownUsageOwner::Identity(identity) => map.serialize_entry("identity", identity)?,
        }
        map.serialize_entry("observationKey", &self.observation_key)?;
        map.serialize_entry(
            "fault",
            studio_runtime_schema::usage::ledger::UNKNOWN_USAGE_RECEIPT,
        )?;
        map.end()
    }
}
impl CaptureDecision {
    pub fn apply(&self, tx: &Transaction<'_>) -> Result<(), CaptureError> {
        if let Some(unknown) = &self.unknown_usage {
            tx.execute_cached(
                "INSERT INTO analytics_meta(key,value) VALUES (?1,?2) ON CONFLICT(key) DO NOTHING",
                params![
                    format!("unknownUsageReceipt:{}", unknown.observation_key),
                    python_json_compact_serialized(unknown).map_err(StoreError::from)?
                ],
            )?;
        }
        if self.response_replay_conflict {
            record_capture_errors(
                tx,
                1,
                "analytics_usage",
                studio_runtime_schema::usage::ledger::RESPONSE_REPLAY_CONFLICT,
                &self
                    .usage_conflict
                    .as_ref()
                    .map(|conflict| {
                        python_json_compact_serialized(conflict).expect("usage conflict serializes")
                    })
                    .unwrap_or_else(|| {
                        "Response replay changed final usage; first evidence retained".into()
                    }),
            )?;
        }
        for (table, row) in &self.rows {
            Repo::write(tx, *table, row)?;
        }
        for n in &self.notifications {
            tx.execute_cached("INSERT INTO analytics_notifications VALUES (?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET count=count+excluded.count,bytes=bytes+excluded.bytes",params![n.id,n.agent,n.root,n.method,n.hour,n.count,n.bytes])?;
        }
        if let Some(n) = &self.limit {
            tx.execute_cached(
                "INSERT INTO analytics_limits(account,at,record) VALUES (?,?,?)",
                params![
                    n["account"].as_str(),
                    n["at"].as_f64(),
                    n["record"].to_string()
                ],
            )?;
        }
        if !self.generation_roots.is_empty() {
            Repo::generation(tx, &self.generation_roots)?;
        }
        Ok(())
    }
}
/// Primary-key-only lookup of durable zero-debit replay protection.
fn unknown_usage_receipt(
    analytics: &Connection,
    agent: &str,
    thread: Option<&str>,
    usage: Option<&UsageObservation>,
    legacy: Option<&LegacyUsageTotals>,
) -> Result<Option<String>, StoreError> {
    let key = match usage {
        Some(observation @ UsageObservation::Cumulative { .. }) => {
            Some(UsageLedgerRow::capture(agent, thread, observation.clone()).observation_key())
        }
        _ => legacy
            .filter(|legacy| legacy.response_id.is_none())
            .map(|legacy| {
                studio_runtime_schema::usage::ledger::legacy_counter_observation_key(
                    agent, thread, legacy,
                )
            }),
    };
    key.map(|key| {
        analytics
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM analytics_meta WHERE key=?1)",
                [format!("unknownUsageReceipt:{key}")],
                |row| row.get::<_, bool>(0),
            )
            .map(|found| found.then_some(key))
    })
    .transpose()
    .map(Option::flatten)
    .map_err(StoreError::from)
}

pub async fn capture(
    store: &RuntimeStore,
    mut prepared: PreparedCapture,
) -> Result<CaptureDecision, CaptureError> {
    check_agent_identity(store, vec![prepared.agent().clone()]).await?;
    store
        .analytics_projection_transaction(move |tx| {
            let epoch = studio_runtime_schema::usage::ledger::LedgerEpoch::Current;
            prepared.bind_budget(tx, epoch)?;
            let decision = decide(
                CaptureSnapshot::load(tx, std::sync::Arc::new(prepared))?,
                epoch,
            )?;
            decision.apply(tx)?;
            Ok(decision)
        })
        .await
}

/// Apply one logged `Analytics` or `AnalyticsStream` effect inside the
/// projector's analytics transaction. The journal commit checked its agent
/// identity when it logged the effect, so no Canvas read happens here.
pub(crate) fn capture_logged_effect(
    tx: &Transaction<'_>,
    effect: &crate::notification_dispatch::NotificationPostCommit,
) -> Result<(), CaptureError> {
    use crate::notification_dispatch::NotificationPostCommit as Effect;
    let (observation, metrics) = match effect {
        Effect::Analytics { observation, .. } => (observation, None),
        Effect::AnalyticsStream {
            observation,
            metrics,
        } => (observation, Some(metrics)),
        Effect::AnalyticsCaptured(_)
        | Effect::AnalyticsStreamCaptured(_)
        | Effect::TokenRateStream(_)
        | Effect::TokenRateAssociation(_)
        | Effect::TokenRateUsage(_)
        | Effect::Recovered { .. } => return Err(CaptureError::Invalid("/effect")),
    };
    let agent = observation
        .agent
        .as_ref()
        .ok_or(CaptureError::Invalid("/agent"))?;
    let epoch = match effect {
        Effect::Analytics { ledger_since, .. } => *ledger_since,
        Effect::AnalyticsStream { .. }
        | Effect::AnalyticsCaptured(_)
        | Effect::AnalyticsStreamCaptured(_)
        | Effect::TokenRateStream(_)
        | Effect::TokenRateAssociation(_)
        | Effect::TokenRateUsage(_)
        | Effect::Recovered { .. } => studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
    };
    let mut prepared = PreparedCapture::notification_event(
        agent,
        &observation.method,
        &observation.params,
        observation.observed_at.get(),
        observation.observed_at.get(),
        epoch,
    )?
    .at_provider_boundary(observation.boundary_at)?;
    if let Some(metrics) = metrics {
        prepared.first_delta_at = metrics.first_delta_at;
    }
    if let Effect::Analytics {
        usage_conflict: Some(conflict),
        ..
    } = effect
    {
        prepared.rejected_usage = Some((**conflict).clone());
    }
    prepared.bind_budget(tx, epoch)?;
    let prepared = std::sync::Arc::new(prepared);
    let mut decision = decide(
        CaptureSnapshot::load(tx, std::sync::Arc::clone(&prepared))?,
        prepared.ledger_since,
    )?;
    if let Some(metrics) = metrics {
        apply_stream_metrics(&mut decision, &prepared, metrics);
    }
    decision.apply(tx)?;
    Ok(())
}

/// Add `count` failures to the `captureErrors` coverage metadata that the
/// insights API reports, with the latest failure's operation, code and message.
pub(crate) fn record_capture_errors(
    tx: &Transaction<'_>,
    count: i64,
    operation: &str,
    code: &str,
    error: &str,
) -> Result<(), CaptureError> {
    tx.execute_cached(
        "INSERT INTO analytics_meta(key,value) VALUES ('captureErrors',json_object('count',?1,'last',json_object('at',CAST(strftime('%s','now') AS INTEGER),'operation',?2,'code',?3,'error',?4))) ON CONFLICT(key) DO UPDATE SET value=json_set(analytics_meta.value,'$.count',COALESCE(json_extract(analytics_meta.value,'$.count'),0)+?1,'$.last',json_object('at',CAST(strftime('%s','now') AS INTEGER),'operation',?2,'code',?3,'error',?4))",
        params![count, operation, code, error],
    )?;
    Ok(())
}

/// Preserve the lossy queue's overflow count in analytics coverage metadata.
/// Python's Runtime.schedule_analytics_captures records queueOverflow instead
/// of failing the already committed provider notification.
pub async fn record_publication_overflow(
    store: &RuntimeStore,
    receipt_id: String,
    analytics_overflow: u64,
    publication_overflow: u64,
) -> Result<(), CaptureError> {
    let dropped = analytics_overflow.saturating_add(publication_overflow);
    if dropped == 0 {
        return Ok(());
    }
    if receipt_id.is_empty() || receipt_id.len() > 1024 {
        return Err(CaptureError::Invalid("/receiptId"));
    }
    store.analytics_projection_transaction(move |tx| {
        let prior: Option<(i64, i64)> = tx
            .query_row_cached(
                "SELECT analytics_overflow,publication_overflow FROM analytics_publication_overflow_receipts WHERE receipt_id=?1",
                [receipt_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let analytics_overflow = analytics_overflow.min(i64::MAX as u64) as i64;
        let publication_overflow = publication_overflow.min(i64::MAX as u64) as i64;
        let additional_analytics = analytics_overflow.saturating_sub(prior.map_or(0, |row| row.0));
        let additional_publications =
            publication_overflow.saturating_sub(prior.map_or(0, |row| row.1));
        let additional = additional_analytics.saturating_add(additional_publications);
        if additional == 0 {
            return Ok(());
        }
        tx.execute_cached(
            "INSERT INTO analytics_publication_overflow_receipts(receipt_id,analytics_overflow,publication_overflow) VALUES (?1,?2,?3) ON CONFLICT(receipt_id) DO UPDATE SET analytics_overflow=MAX(analytics_publication_overflow_receipts.analytics_overflow,excluded.analytics_overflow),publication_overflow=MAX(analytics_publication_overflow_receipts.publication_overflow,excluded.publication_overflow)",
            params![receipt_id.as_str(), analytics_overflow, publication_overflow],
        )?;
        record_capture_errors(
            tx,
            additional,
            "analytics_capture_queue",
            "queueOverflow",
            "Publication queue limit reached",
        )?;
        Ok(())
    })
    .await
}

fn apply_stream_metrics(
    decision: &mut CaptureDecision,
    prepared: &PreparedCapture,
    metrics: &NotificationStreamMetrics,
) {
    let meta = prepared.meta();
    decision.notifications = metrics
        .hours
        .iter()
        .map(|hour| Notification {
            id: python_json_compact_serialized(&(&meta["agentId"], &prepared.method, hour.hour))
                .expect("notification identity serializes"),
            agent: meta["agentId"].as_str().map(str::to_owned),
            root: meta["rootId"].as_str().map(str::to_owned),
            method: Some(prepared.method.clone()),
            hour: i64::try_from(hour.hour).ok(),
            count: i64::try_from(hour.count).unwrap_or(1),
            bytes: i64::try_from(hour.encoded_bytes).ok(),
        })
        .collect();

    let synthetic_delta = prepared.params["delta"].as_str().unwrap_or_default();
    let total_deltas = metrics.hours.iter().map(|hour| hour.count).sum::<u64>();
    let extra = [
        total_deltas.saturating_sub(1),
        metrics.bytes.saturating_sub(synthetic_delta.len() as u64),
        metrics
            .characters
            .saturating_sub(synthetic_delta.chars().count() as u64),
        metrics.lines.saturating_sub(
            synthetic_delta
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count() as u64,
        ),
    ];
    for (table, row) in &mut decision.rows {
        if *table != CaptureTable::Items || !row["stream"].is_object() {
            continue;
        }
        for (field, addition) in ["deltas", "bytes", "chars", "lines"].into_iter().zip(extra) {
            row["stream"][field] = json!(
                row["stream"][field]
                    .as_u64()
                    .unwrap_or_default()
                    .saturating_add(addition)
            );
        }
    }
}

/// The serialized JSON length in bytes, counted without building the text.
pub(crate) fn encoded_len(value: &(impl serde::Serialize + ?Sized)) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    studio_json::to_writer(&mut count, value).expect("analytics values serialize");
    count.0
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureIdentity {
    thread_id: Option<String>,
    account_key: Option<String>,
    epoch: Option<i64>,
}

/// Check agent identities on a Canvas read snapshot before the analytics commit. The
/// analytics write then needs no Canvas writer admission. An identity that changes
/// between the check and the commit lets one capture of the old identity land, as if
/// it had been captured just before the change.
pub(crate) async fn check_agent_identity(
    store: &RuntimeStore,
    agents: Vec<Value>,
) -> Result<(), CaptureError> {
    store
        .read_async("runtime-services/analytics_identity", move |canvas| {
            agents
                .iter()
                .try_for_each(|agent| check_identity(canvas, agent))
        })
        .await
}
pub(crate) fn check_identity(canvas: &Connection, agent: &Value) -> Result<(), CaptureError> {
    let raw: Option<String> = canvas
        .query_row_cached(
            "SELECT record FROM runtime_agents WHERE id=?",
            [agent["id"].as_str()],
            |r| r.get(0),
        )
        .optional()?;
    let current: CaptureIdentity = raw
        .map(|r| studio_json::from_str(&r).map_err(StoreError::from))
        .transpose()?
        .ok_or(CaptureError::StaleIdentity)?;
    let expected: CaptureIdentity =
        serde::Deserialize::deserialize(agent).map_err(StoreError::from)?;
    if current.thread_id != expected.thread_id
        || current.account_key != expected.account_key
        || current.epoch != expected.epoch
    {
        return Err(CaptureError::StaleIdentity);
    }
    Ok(())
}
pub fn decide(
    snapshot: CaptureSnapshot,
    epoch: studio_runtime_schema::usage::ledger::LedgerEpoch,
) -> Result<CaptureDecision, CaptureError> {
    if snapshot.prepared.ledger_since != epoch {
        return Err(CaptureError::Invalid("/ledgerEpoch"));
    }
    let CaptureSnapshot {
        prepared: q,
        mut meta,
        rows,
    } = snapshot;
    let p = &q.params;
    let method = q.method.as_str();
    let at = q.at;
    let mut result = CaptureDecision::default();
    let mut agent = project(
        &q.agent,
        &[
            "id",
            "name",
            "rootId",
            "parentId",
            "accountKey",
            "accountHistory",
            "provider",
            "threadId",
            "model",
            "effort",
            "fastMode",
            "daybreakEnabled",
            "cyberAccessProgram",
            "cwd",
            "deletedAt",
        ],
    );
    if !agent["accountHistory"].is_array() {
        agent["accountHistory"] = json!([])
    }
    result.rows.push((CaptureTable::Agents, agent));
    if q.source == CaptureSource::Live && !q.model {
        let hour = (at / 3600.).floor() as i64 * 3600;
        result.notifications.push(Notification {
            id: python_json_compact_serialized(&(&q.agent["id"], method, hour))
                .expect("notification identity serializes"),
            agent: q.agent["id"].as_str().map(str::to_owned),
            root: meta["rootId"].as_str().map(str::to_owned),
            method: Some(method.to_owned()),
            hour: Some(hour),
            count: 1,
            bytes: i64::try_from(p.to_string().len()).ok(),
        });
    }
    if studio_runtime_store::truthy(Some(&meta["turnId"]))
        && !q.model
        && (matches!(
            method,
            "turn/started"
                | "turn/completed"
                | "item/agentMessage/delta"
                | "item/started"
                | "item/completed"
        ) || q.source == CaptureSource::Live
            && (q.boundary_at.is_some() || method == "analytics/adapterBoundary"))
    {
        let mut row=rows.turn.clone().unwrap_or_else(||merge(&meta,json!({"id":key(&meta,&meta["turnId"]),"at":at,"startedAt":null,"finishedAt":null,"firstOutputAt":null,"durationMs":null,"firstOutputDelayMs":null,"status":"unknown","source":q.source.name()})));
        match method {
            "turn/started" => {
                if q.source == CaptureSource::Live {
                    row["daybreakEnabled"] = meta["daybreakEnabled"].clone();
                    row["cyberAccessProgram"] = meta["cyberAccessProgram"].clone();
                }
                row["startedAt"] = json!(number(&row["startedAt"]).map_or(at, |n| n.min(at)));
                if row["finishedAt"].is_null() {
                    row["status"] = json!("running")
                }
            }
            "turn/completed" => {
                let error = &p["turn"]["error"];
                let status = if p["turn"]["status"] == "uncertain" {
                    json!("uncertain")
                } else if studio_runtime_store::truthy(Some(error)) {
                    json!("failed")
                } else {
                    p["turn"].get("status").cloned().unwrap_or(json!("unknown"))
                };
                let previous =
                    studio_runtime_store::truthy(Some(&row["error"])) || row["status"] == "failed";
                let incoming = studio_runtime_store::truthy(Some(error)) || status == "failed";
                if !previous
                    || incoming
                        && (!studio_runtime_store::truthy(Some(&row["error"]))
                            || q.source == CaptureSource::Live
                            || row.get("terminalSource").unwrap_or(&row["source"]) != "live")
                {
                    row["finishedAt"] = json!(at);
                    row["status"] = status;
                    row["error"] = if incoming {
                        first(&[error, &row["error"]])
                    } else {
                        Value::Null
                    };
                    studio_runtime_schema::claude_failure::sanitize_error(&mut row["error"]);
                    row["terminalSource"] = json!(q.source.name());
                }
                for (source, target) in [
                    ("durationMs", "nativeDurationMs"),
                    ("timeToFirstTokenMs", "nativeTimeToFirstTokenMs"),
                ] {
                    if let Some(n) = number(&p[source]) {
                        row[target] = json!(n)
                    }
                }
            }
            _ => {
                if (method == "item/agentMessage/delta"
                    && studio_runtime_store::truthy(Some(&p["delta"]))
                    || p["item"]["type"] == "agentMessage"
                        && studio_runtime_store::truthy(Some(&p["item"]["text"])))
                    && row["firstOutputAt"].is_null()
                {
                    row["firstOutputAt"] = json!(at)
                }
            }
        }
        boundary_timings(&q, &mut row)?;
        timings(&mut row);
        if rows.turn.as_ref() != Some(&row) {
            result.rows.push((CaptureTable::Turns, row));
        }
    }
    if method == "thread/tokenUsage/updated" {
        let provider = ProviderKind::of_record(&q.agent)
            .map_err(|_| CaptureError::Invalid("/agent/provider"))?;
        usage(&q, provider, &mut meta, &rows, &mut result);
        return Ok(result);
    }
    if q.model {
        model_item(&q, &meta, rows.item, &mut result);
        return Ok(result);
    }
    if method == "analytics/rateLimits" {
        if rows.last_limit.as_ref().map(limit_content) != Some(limit_content(p)) {
            result.limit = Some(json!({"account":meta["accountKey"],"at":at,"record":p}));
        }
        return Ok(result);
    }
    if method == "analytics/compaction" {
        let mut params = q.params.clone();
        params["item"] = json!({"id":first(&[&p["id"],&p["_analyticsId"],&json!(at.to_string())]),"type":"compactionSnapshot","compactionMetadata":p});
        let next = PreparedCapture {
            agent: std::sync::Arc::clone(&q.agent),
            method: "item/completed".into(),
            params,
            at: q.at,
            recorded_at: q.recorded_at,
            source: q.source,
            input: q.input.clone(),
            output: q.output.clone(),
            stream: q.stream.clone(),
            model: q.model,
            usage: q.usage.clone(),
            legacy_usage: q.legacy_usage.clone(),
            rejected_usage: q.rejected_usage.clone(),
            ledger_since: q.ledger_since,
            unknown_usage_key: q.unknown_usage_key.clone(),
            boundary_at: q.boundary_at,
            first_delta_at: q.first_delta_at,
        };
        let recursive = decide(
            CaptureSnapshot::from_records(std::sync::Arc::new(next), rows)?,
            q.ledger_since,
        )?;
        result.rows.extend(
            recursive
                .rows
                .into_iter()
                .filter(|(table, _)| *table != CaptureTable::Agents),
        );
        result.notifications.extend(recursive.notifications);
        return Ok(result);
    }
    protocol_item(&q, &meta, rows.item, &mut result);
    Ok(result)
}
fn boundary_timings(q: &PreparedCapture, row: &mut Value) -> Result<(), CaptureError> {
    use studio_runtime_schema::provider_timing::ProviderBoundaryTiming;
    if q.source != CaptureSource::Live {
        return Ok(());
    }
    let mut timing: ProviderBoundaryTiming = row
        .get("boundaryTiming")
        .map(serde::Deserialize::deserialize)
        .transpose()
        .map_err(StoreError::from)?
        .unwrap_or_default();
    let mut incoming = if q.method == "analytics/adapterBoundary" {
        serde::Deserialize::deserialize(&q.params["boundaryTiming"]).map_err(StoreError::from)?
    } else {
        ProviderBoundaryTiming::default()
    };
    // Account-wide notices without a native turn identity must not claim the
    // current agent turn's first event.
    if let Some(stamp) = q.boundary_at
        && (q.params["turnId"].is_string() || q.params["turn"]["id"].is_string())
    {
        incoming.first_event = Some(stamp);
        let output = q.method == "item/modelOutput/firstDelta"
            || matches!(
                q.method.as_str(),
                "item/agentMessage/delta"
                    | "item/reasoning/textDelta"
                    | "item/reasoning/summaryTextDelta"
                    | "item/reasoning/summaryDelta"
            );
        if output
            && let Some(token) = q.first_delta_at
            && timing.terminal.is_none_or(|terminal| {
                token.same_boot(terminal) && token.monotonic_ns <= terminal.monotonic_ns
            })
        {
            incoming.first_token = Some(token);
        }
        if q.method == "turn/completed" && q.params["turn"]["status"] != "uncertain" {
            incoming.terminal = Some(stamp);
        }
    }
    timing.merge_first(incoming);
    if timing != Default::default() {
        row["boundaryTiming"] =
            serde::Serialize::serialize(&timing, studio_json::value::Serializer)
                .map_err(StoreError::from)?;
    }
    Ok(())
}
fn protocol_item(
    q: &PreparedCapture,
    meta: &Value,
    existing: Option<Value>,
    result: &mut CaptureDecision,
) {
    let p = &q.params;
    let item = &p["item"];
    let method = q.method.as_str();
    let Some(id) = q.item_key(meta).filter(|_| method.starts_with("item/")) else {
        return;
    };
    let row = if !matches!(method, "item/started" | "item/completed") {
        let Some(mut row) = existing else { return };
        if !row["finishedAt"].is_null()
            || !(method.ends_with("Delta") || method.ends_with("/delta"))
        {
            return;
        }
        let Some(delta) = p["delta"].as_str() else {
            return;
        };
        let mut stream = row
            .get("stream")
            .filter(|s| s.is_object())
            .cloned()
            .unwrap_or(json!({"bytes":0,"chars":0,"lines":0,"deltas":0}));
        for k in ["bytes", "chars", "lines"] {
            let size = if k == "lines" {
                delta.bytes().filter(|c| *c == b'\n').count() as u64
            } else {
                q.stream[k].as_u64().unwrap_or(0)
            };
            stream[k] = json!(stream[k].as_u64().unwrap_or(0).saturating_add(size));
        }
        stream["deltas"] = json!(stream["deltas"].as_u64().unwrap_or(0).saturating_add(1));
        row["stream"] = stream;
        row
    } else {
        if existing
            .as_ref()
            .is_some_and(|r| !r["finishedAt"].is_null())
            && method == "item/started"
        {
            return;
        }
        let mut row=existing.unwrap_or_else(||merge(meta,json!({"id":id,"itemId":first(&[&item["id"],&p["itemId"]]),"startedAt":if method=="item/started"{json!(q.at)}else{Value::Null},"at":q.at,"source":q.source.name(),"input":null,"output":null,"payloadBoundary":"protocol"})));
        if row["source"] == "live" && q.source != CaptureSource::Live {
            for (src, dst) in [("startedAt", "startedAt"), ("completedAt", "finishedAt")] {
                if let Some(n) = number(&p[src]) {
                    row[dst] = json!(n)
                }
            }
            if let (Some(start), Some(end)) =
                (number(&row["startedAt"]), number(&row["finishedAt"]))
                && row["durationSource"] != "provider"
            {
                row["durationMs"] = json!(((end - start) * 1000.).max(0.));
                row["durationSource"] = json!("native_item_timestamps")
            }
        } else {
            let kind = first(&[&item["type"], &row["type"], &json!("unknown")]);
            let kind_name = kind.as_str().unwrap_or("unknown");
            row["name"] = first(&[&item["tool"], &item["name"], &kind]);
            row["isTool"] = json!(!matches!(
                kind_name,
                "userMessage"
                    | "agentMessage"
                    | "reasoning"
                    | "plan"
                    | "contextCompaction"
                    | "compactionSnapshot"
            ));
            row["type"] = kind;
            row["recordedAt"] = json!(q.recorded_at);
            row["source"] = json!(q.source.name());
            if let Some(n) = number(&p["startedAt"]) {
                row["startedAt"] = json!(n)
            }
            row["timestampSource"] = timestamp_source(q);
            if method == "item/completed" {
                row["finishedAt"] = json!(number(&p["completedAt"]).unwrap_or(q.at));
            }
            row["status"] = json!(if method != "item/completed" {
                "running"
            } else if item["success"] == false
                || studio_runtime_store::truthy(Some(&item["error"]))
                || matches!(item["status"].as_str(), Some("failed" | "declined"))
                || !item["exitCode"].is_null() && item["exitCode"] != 0
            {
                "failed"
            } else {
                "completed"
            });
            for k in ["command", "cwd", "server", "exitCode", "processId"] {
                if let Some(v) = item.get(k) {
                    row[k] = v.clone()
                }
            }
            if studio_runtime_store::truthy(Some(&item["compactionMetadata"])) {
                row["compactionMetadata"] = item["compactionMetadata"].clone();
            }
            if studio_runtime_store::truthy(Some(&item["error"])) {
                let s = item["error"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| item["error"].to_string());
                row["error"] = json!(s.chars().take(2000).collect::<String>());
            }
            let supplied = number(&item["durationMs"]);
            let duration = supplied.or_else(|| {
                if method == "item/completed" {
                    number(&row["startedAt"])
                        .map(|s| ((number(&row["finishedAt"]).unwrap_or(q.at) - s) * 1000.).max(0.))
                } else {
                    None
                }
            });
            row["durationMs"] = json!(duration);
            row["durationSource"] = json!(if supplied.is_some() {
                Some("provider")
            } else if duration.is_some() {
                Some("observed_wall_time")
            } else {
                None
            });
            if !q.input.is_null() {
                row["input"] = q.input.clone()
            }
            if !q.output.is_null() {
                row["output"] = q.output.clone()
            }
            row["coverage"] = json!(if q.source == CaptureSource::Live {
                "protocol_payload"
            } else {
                q.source.name()
            });
            row["payloadTruncated"] = item["outputTruncated"].clone();
        }
        row
    };
    result
        .rows
        .push((CaptureTable::Items, finish_item(row, q.recorded_at)));
}
fn model_item(
    q: &PreparedCapture,
    meta: &Value,
    existing: Option<Value>,
    result: &mut CaptureDecision,
) {
    let p = &q.params;
    let Some(id) = q.item_key(meta) else { return };
    let kind = p["type"].as_str().unwrap_or("unknown");
    let category = model_category(p);
    let tool = category == "tool" && (is_model_input(kind) || is_model_output(kind));
    let call = first(&[&p["call_id"], &p["callId"], &p["id"], &p["_analyticsId"]]);
    let mut row=existing.unwrap_or_else(||merge(meta,json!({"id":id,"itemId":call,"at":q.at,"startedAt":if is_model_input(kind){json!(q.at)}else{Value::Null},"finishedAt":null,"type":if tool{"modelToolCall"}else{"modelMessage"},"name":first(&[&p["name"],&json!(kind)]),"isTool":tool,"input":null,"output":null,"durationMs":null,"payloadBoundary":"model","category":category,"role":p["role"],"callId":call,"snapshotItemId":if matches!(category,"compactionReplacement"|"compactionGuardian"){p["_analyticsId"].clone()}else{Value::Null}})));
    row["source"] = json!(q.source.name());
    row["recordedAt"] = json!(q.recorded_at);
    row["coverage"] = json!("model_protocol_payload");
    row["timestampSource"] = p
        .get("_analyticsTimestampSource")
        .cloned()
        .unwrap_or(json!("record"));
    if studio_runtime_store::truthy(Some(&p["name"])) {
        row["name"] = json!(format!(
            "{}{}",
            p["namespace"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| format!("{s}."))
                .unwrap_or_default(),
            text(&p["name"])
        ));
        row["namespace"] = p["namespace"].clone();
    }
    if is_model_input(kind) {
        row["input"] = q.input.clone();
        row["modelInput"] = q.input.clone();
        row["startedAt"] = json!(q.at);
        row["status"] = json!(if row["finishedAt"].is_null() {
            "running"
        } else {
            "completed"
        })
    } else if is_model_output(kind) {
        row["output"] = q.output.clone();
        row["modelOutput"] = q.output.clone();
        row["finishedAt"] = json!(q.at);
        row["status"] = json!(if p["is_error"] == true {
            "failed"
        } else {
            "completed"
        })
    } else {
        let direction = if matches!(p["role"].as_str(), Some("user" | "system" | "developer")) {
            "input"
        } else {
            "output"
        };
        row[direction] = if direction == "input" {
            q.input.clone()
        } else {
            q.output.clone()
        };
        row["status"] = json!("completed");
        row["finishedAt"] = json!(q.at)
    }
    if let (Some(start), Some(end)) = (number(&row["startedAt"]), number(&row["finishedAt"])) {
        row["durationMs"] = json!(((end - start) * 1000.).max(0.));
        row["durationSource"] = json!("observed_wall_time");
    }
    result
        .rows
        .push((CaptureTable::Items, finish_item(row, q.recorded_at)));
}
fn finish_item(mut row: Value, now: f64) -> Value {
    if row.get("firstRecordedAt").is_none() {
        row["firstRecordedAt"] = row.get("recordedAt").cloned().unwrap_or(json!(now));
    }
    if row.get("firstSourceAt").is_none() {
        row["firstSourceAt"] = row["at"].clone();
    }
    row["at"] = json!(
        number(&row["startedAt"])
            .or_else(|| number(&row["finishedAt"]))
            .or_else(|| number(&row["at"]))
    );
    row
}
fn timings(row: &mut Value) {
    if let Some(start) = number(&row["startedAt"]) {
        for (event, metric) in [
            ("finishedAt", "durationMs"),
            ("firstOutputAt", "firstOutputDelayMs"),
        ] {
            if let Some(end) = number(&row[event]) {
                row[metric] = json!(((end - start) * 1000.).max(0.));
            }
        }
    }
}
fn is_model_input(kind: &str) -> bool {
    matches!(kind, "function_call" | "custom_tool_call" | "tool_call")
}
fn is_model_output(kind: &str) -> bool {
    matches!(
        kind,
        "function_call_output" | "custom_tool_call_output" | "tool_result"
    )
}
fn model_category(p: &Value) -> &str {
    p["_analyticsCategory"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_model_input(text(&p["type"])) || is_model_output(text(&p["type"])) {
                "tool"
            } else {
                "message"
            }
        })
}
fn limit_content(p: &Value) -> Value {
    match p.as_object() {
        Some(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    !matches!(k.as_str(), "at" | "processedAt" | "checkedAt") && !k.starts_with('_')
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
        None => p.clone(),
    }
}
pub(crate) use studio_runtime_store::analytics_capture::{number, text};
pub(crate) fn first(values: &[&Value]) -> Value {
    values
        .iter()
        .find(|v| studio_runtime_store::truthy(Some(v)))
        .map(|v| (*v).clone())
        .unwrap_or_default()
}
pub(crate) fn project(v: &Value, keys: &[&str]) -> Value {
    Value::Object(keys.iter().map(|k| ((*k).into(), v[*k].clone())).collect())
}
pub(crate) fn merge(meta: &Value, extra: Value) -> Value {
    let mut value = meta.clone();
    if let (Some(v), Some(e)) = (value.as_object_mut(), extra.as_object()) {
        v.extend(e.clone());
    }
    value
}
fn key(meta: &Value, id: &Value) -> String {
    format!(
        "{}:{}:{}",
        text(&meta["agentId"]),
        text(&meta["threadId"]),
        text(id)
    )
}
fn timestamp_source(q: &PreparedCapture) -> Value {
    q.params
        .get("_analyticsTimestampSource")
        .cloned()
        .unwrap_or(json!(if q.source == CaptureSource::Live {
            "observed"
        } else {
            "record"
        }))
}
fn hash(v: &impl serde::Serialize) -> String {
    sha256_hex(
        python_json_compact_serialized(v)
            .expect("identity serializes")
            .as_bytes(),
    )
}

fn domain(observation: &UsageObservation) -> &'static str {
    match observation {
        UsageObservation::Response { .. } => "response",
        UsageObservation::Cumulative {
            scope: UsageScope::CodexThread,
            ..
        } => "codexThread",
        UsageObservation::Cumulative {
            scope: UsageScope::ClaudeQuery,
            ..
        } => "claudeQuery",
        UsageObservation::Gap { .. } => "gap",
    }
}
fn fingerprint(observation: &UsageObservation) -> String {
    hash(observation)
}
fn usage_key(meta: &Value, observation: &UsageObservation) -> String {
    match UsageIdentity::new(
        text(&meta["agentId"]),
        meta["threadId"].as_str(),
        observation,
    ) {
        UsageIdentity::Response { agent, response_id } => {
            if matches!(observation, UsageObservation::Gap { .. }) {
                hash(&(agent, "gap", response_id))
            } else {
                hash(&(agent, response_id))
            }
        }
        UsageIdentity::Cumulative { .. } => UsageLedgerRow::capture(
            text(&meta["agentId"]),
            meta["threadId"].as_str(),
            observation.clone(),
        )
        .observation_key(),
    }
}
/// Compatibility presentation for existing analytics charts; accounting uses studioUsage.
pub(crate) fn analytics_tokens(tokens: &UsageTokens) -> Value {
    json!({"inputTokens": tokens.uncached_input().saturating_add(tokens.cache_read_input()).saturating_add(tokens.cache_write_input()),
        "cachedInputTokens":tokens.cache_read_input(),"cacheWriteInputTokens":tokens.cache_write_input(),
        "outputTokens":tokens.output(),"reasoningOutputTokens":tokens.reasoning_output(),"totalTokens":tokens.total()})
}
fn legacy_usage_key(meta: &Value, legacy: &LegacyUsageTotals, params: &Value) -> String {
    if legacy.response_id.is_none() {
        return studio_runtime_schema::usage::ledger::legacy_counter_observation_key(
            text(&meta["agentId"]),
            meta["threadId"].as_str(),
            legacy,
        );
    }
    old_legacy_usage_key(meta, legacy, params)
}
fn old_legacy_usage_key(meta: &Value, legacy: &LegacyUsageTotals, params: &Value) -> String {
    if let Some(response) = &legacy.response_id {
        return hash(&(&meta["agentId"], response));
    }
    let identity = studio_runtime_schema::usage::legacy_notice_identity(
        text(&meta["agentId"]),
        text(&meta["accountKey"]),
        meta["turnId"].as_str(),
        params,
    )
    .unwrap_or_else(|| {
        json!([
            meta["agentId"],
            meta["accountKey"],
            legacy.total_tokens,
            legacy.last_total_tokens
        ])
    });
    hash(&identity)
}
fn usage(
    q: &PreparedCapture,
    provider: ProviderKind,
    meta: &mut Value,
    rows: &CaptureRows,
    out: &mut CaptureDecision,
) {
    if let Some(observation_key) = &q.unknown_usage_key {
        out.unknown_usage = Some(UnknownUsageReceipt {
            owner: UnknownUsageOwner::Agent(text(&meta["agentId"]).to_owned()),
            observation_key: observation_key.clone(),
        });
        return;
    }
    if let Some(conflict) = &q.rejected_usage {
        out.response_replay_conflict = true;
        out.usage_conflict = Some(conflict.clone());
        return;
    }
    let Some(mut observation) = q.usage.clone() else {
        if let Some(legacy) = &q.legacy_usage {
            if rows.usage.is_some() {
                return;
            }
            if q.legacy_counter_era() {
                out.unknown_usage = Some(UnknownUsageReceipt {
                    owner: UnknownUsageOwner::Identity(UsageIdentity::Cumulative {
                        agent: text(&meta["agentId"]).into(),
                        thread_id: meta["threadId"].as_str().map(str::to_owned),
                        scope: legacy.scope,
                    }),
                    observation_key: legacy_usage_key(meta, legacy, &q.params),
                });
                return;
            }
            let mut last = json!({});
            if let Some(total) = legacy.last_total_tokens {
                last["totalTokens"] = json!(total);
            }
            let row = merge(
                meta,
                json!({"id":legacy_usage_key(meta,legacy,&q.params),"at":q.at,"recordedAt":q.recorded_at,
                "source":q.source.name(),"timestampSource":timestamp_source(q),"model":legacy.model.as_deref().or_else(||meta["model"].as_str()),
                "last":last,"total":{"totalTokens":legacy.total_tokens},"responseId":legacy.response_id,
                "modelContextWindow":legacy.model_context_window,"counterDomain":legacy.scope}),
            );
            mark_generation(meta, None, out);
            out.rows.push((CaptureTable::Usage, row));
        }
        return;
    };
    if let Some(turn) = &rows.turn
        && turn["accountKey"] == meta["accountKey"]
        && !turn["cyberAccessProgram"].is_null()
    {
        meta["daybreakEnabled"] = turn["daybreakEnabled"].clone();
        meta["cyberAccessProgram"] = turn["cyberAccessProgram"].clone();
    }
    if let UsageObservation::Response { model, .. } = &mut observation
        && model.is_empty()
    {
        *model = text(&meta["model"]).to_owned();
    }
    if q.source == CaptureSource::Rollout
        && let UsageObservation::Response {
            response_id,
            turn_id,
            tokens,
            ..
        } = &observation
    {
        out.response_rate = Some(ResponseRateAssociation {
            agent: text(&meta["agentId"]).into(),
            thread: meta["threadId"].as_str().map(str::to_owned),
            turn: turn_id
                .clone()
                .or_else(|| meta["turnId"].as_str().map(str::to_owned)),
            response: response_id.clone(),
            output_tokens: tokens.output() as f64,
        });
    }
    let existing = rows.exact_response.as_ref().or(rows.usage.as_ref());
    if existing.is_none() && q.legacy_counter_era() {
        let ledger = UsageLedgerRow::capture(
            text(&meta["agentId"]),
            meta["threadId"].as_str(),
            observation,
        );
        out.unknown_usage = Some(UnknownUsageReceipt {
            owner: UnknownUsageOwner::Identity(ledger.usage_identity().clone()),
            observation_key: ledger.observation_key(),
        });
        return;
    }
    if let Some(previous) = existing
        && let Ok(old) = UsageLedgerRow::from_analytics(provider, previous)
    {
        match correction(old.studio_usage(), &observation) {
            UsageCorrection::Apply | UsageCorrection::UnresolvedGap => {}
            decision @ UsageCorrection::Replay
            | decision @ UsageCorrection::ResponseReplayConflict => {
                out.response_replay_conflict = decision == UsageCorrection::ResponseReplayConflict;
                if let (Some(rate), UsageObservation::Response { tokens, .. }) =
                    (&mut out.response_rate, old.studio_usage())
                {
                    rate.output_tokens = tokens.output() as f64;
                }
                return;
            }
        }
    }

    let id = if matches!(&observation, UsageObservation::Gap { .. }) {
        rows.usage
            .as_ref()
            .map(|row| text(&row["id"]).to_owned())
            .unwrap_or_else(|| usage_key(meta, &observation))
    } else {
        existing
            .map(|row| text(&row["id"]).to_owned())
            .unwrap_or_else(|| usage_key(meta, &observation))
    };
    let mut ledger = UsageLedgerRow::capture(
        text(&meta["agentId"]),
        meta["threadId"].as_str(),
        observation.clone(),
    );
    if let Some(old) = existing.and_then(|row| UsageLedgerRow::from_analytics(provider, row).ok())
        && old.receipt_key().is_some()
    {
        ledger.set_receipt_key(old.receipt_key().map(str::to_owned));
    }
    let mut row = merge(
        meta,
        json!({"id":id,"at":q.at,"recordedAt":q.recorded_at,"source":q.source.name(),
        "timestampSource":timestamp_source(q),"counterDomain":domain(&observation),"fingerprint":fingerprint(&observation),
        "last":{},"total":{},"delta":{},"studioUsage":observation,
        "usageIdentity":ledger.usage_identity(),"receiptKey":ledger.receipt_key()}),
    );
    match &observation {
        UsageObservation::Response {
            response_id,
            turn_id,
            model,
            tokens,
            model_context_window,
            ..
        } => {
            row["responseId"] = json!(response_id);
            row["model"] = json!(model);
            if let Some(turn) = turn_id {
                row["turnId"] = json!(turn);
            }
            row["last"] = analytics_tokens(tokens);
            row["delta"] = analytics_tokens(tokens);
            row["modelContextWindow"] = json!(model_context_window);
        }
        UsageObservation::Cumulative {
            total,
            last,
            turn_id,
            model_context_window,
            ..
        } => {
            row["total"] = analytics_tokens(total);
            if let Some(last) = last {
                row["last"] = analytics_tokens(last);
            }
            if let Some(turn) = turn_id {
                row["turnId"] = json!(turn);
            }
            row["modelContextWindow"] = json!(model_context_window);
            let prior = rows
                .prior_usage
                .as_ref()
                .and_then(|row| parse_analytics_usage_row(provider, row).ok());
            let previous = match prior {
                Some(UsageObservation::Cumulative { total, .. }) => Some(total),
                _ => None,
            };
            let reset = previous
                .as_ref()
                .is_some_and(|old| total.decreased_from(old));
            row["reset"] = json!(reset);
            row["baselineMissing"] = json!(previous.is_none() || reset);
        }
        UsageObservation::Gap {
            response_id,
            turn_id,
            model,
            ..
        } => {
            row["responseId"] = json!(response_id);
            if let Some(turn) = turn_id {
                row["turnId"] = json!(turn);
            }
            row["model"] = json!(model);
        }
    }
    mark_generation(meta, existing, out);
    out.rows.push((CaptureTable::Usage, row));
}
fn mark_generation(meta: &Value, previous: Option<&Value>, out: &mut CaptureDecision) {
    let mut roots = vec![text(&meta["rootId"]).to_owned()];
    if let Some(old) = previous.and_then(|v| v["rootId"].as_str())
        && !roots.iter().any(|r| r == old)
    {
        roots.push(old.into())
    }
    out.generation_roots = roots;
}
/// The notification dispatcher can commit one bounded delta batch. Preparation
/// remains outside the transaction; each item observes prior items in order.
pub async fn capture_batch(
    store: &RuntimeStore,
    prepared: Vec<PreparedCapture>,
) -> Result<Vec<ResponseRateAssociation>, CaptureError> {
    if prepared.len() > 128 {
        return Err(CaptureError::Invalid("/capture/batch"));
    }
    check_agent_identity(
        store,
        prepared.iter().map(|item| item.agent().clone()).collect(),
    )
    .await?;
    store
        .analytics_projection_transaction(move |tx| {
            let mut effects = Vec::new();
            for mut prepared in prepared {
                let epoch = studio_runtime_schema::usage::ledger::LedgerEpoch::Current;
                prepared.bind_budget(tx, epoch)?;
                let decision = decide(
                    CaptureSnapshot::load(tx, std::sync::Arc::new(prepared))?,
                    epoch,
                )?;
                decision.apply(tx)?;
                if let Some(effect) = decision.response_rate {
                    effects.push(effect)
                }
            }
            Ok(effects)
        })
        .await
}
/// Account-level rate-limit capture uses the same change-only series as events.
pub async fn capture_limit(
    store: &RuntimeStore,
    account: String,
    value: Value,
    at: f64,
) -> Result<(), CaptureError> {
    if !at.is_finite() || at < 0. || account.is_empty() || encoded_len(&value) > MAX_CAPTURE_BYTES {
        return Err(CaptureError::Invalid("/rateLimits"));
    }
    store
        .analytics_projection_transaction(move |tx| {
            let previous = Repo::last_limit(tx, Some(&account))?;
            if previous.as_ref().map(limit_content) != Some(limit_content(&value)) {
                tx.execute_cached(
                    "INSERT INTO analytics_limits(account,at,record) VALUES (?,?,?)",
                    params![account, at, value.to_string()],
                )?;
            }
            Ok(())
        })
        .await
}

#[cfg(test)]
mod usage_contract_tests {
    use super::*;

    #[test]
    fn unknown_receipt_reuses_the_exact_lookup_key() {
        let agent: Value = studio_json::from_str(r#"{"id":"agent","rootId":"agent","provider":"claude","accountKey":"default","threadId":"thread"}"#).expect("agent");
        let params: Value = studio_json::from_str(
            r#"{"threadId":"","tokenUsage":{"total":{"totalTokens":23},"last":{"totalTokens":7}}}"#,
        )
        .expect("params");
        let epoch = studio_runtime_schema::usage::ledger::LedgerEpoch::Current;
        let mut prepared = PreparedCapture::event(
            &agent,
            "thread/tokenUsage/updated",
            &params,
            1.,
            2.,
            CaptureSource::Live,
            epoch,
        )
        .expect("capture");
        let legacy = prepared.legacy_usage.as_ref().expect("legacy usage");
        let key = studio_runtime_schema::usage::ledger::legacy_counter_observation_key(
            "agent",
            Some(""),
            legacy,
        );
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch("CREATE TABLE analytics_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
            .expect("table");
        db.execute(
            "INSERT INTO analytics_meta VALUES (?1,'{}')",
            [format!("unknownUsageReceipt:{key}")],
        )
        .expect("receipt");
        prepared.bind_budget(&db, epoch).expect("bind");
        let decision = decide(
            CaptureSnapshot::from_records(Arc::new(prepared), CaptureRows::default())
                .expect("snapshot"),
            epoch,
        )
        .expect("decision");
        assert_eq!(
            decision
                .unknown_usage
                .expect("unknown receipt")
                .observation_key,
            key
        );
    }

    #[test]
    fn unknown_usage_receipts_preserve_both_stored_shapes() {
        let receipts = [
            (
                UnknownUsageReceipt {
                    owner: UnknownUsageOwner::Agent("agent".into()),
                    observation_key: "key".into(),
                },
                r#"{"agent":"agent","fault":"unknownUsageReceipt","observationKey":"key"}"#,
            ),
            (
                UnknownUsageReceipt {
                    owner: UnknownUsageOwner::Identity(UsageIdentity::Cumulative {
                        agent: "agent".into(),
                        thread_id: Some("thread".into()),
                        scope: UsageScope::CodexThread,
                    }),
                    observation_key: "key".into(),
                },
                r#"{"fault":"unknownUsageReceipt","identity":{"agent":"agent","kind":"cumulative","scope":"codexThread","threadId":"thread"},"observationKey":"key"}"#,
            ),
        ];
        for (receipt, expected) in receipts {
            assert_eq!(
                python_json_compact_serialized(&receipt).expect("serialize"),
                expected
            );
        }
    }

    #[test]
    fn typed_usage_identity_keeps_legacy_hash() {
        let raw = r#"{"kind":"response","responseId":"résponse","model":"model","evidence":"live","tokens":{"uncachedInput":10,"cacheReadInput":3,"cacheWriteInput":2,"output":7}}"#;
        let observation: UsageObservation = studio_json::from_str(raw).expect("usage observation");
        assert_eq!(
            fingerprint(&observation),
            "652b1851ea247a0d6b76e1b383f4b9cc7c8478f08e0b49bca10106359c03961d"
        );
        let tuple = ("agent", "résponse");
        assert_eq!(
            hash(&tuple),
            "bc48ea7cafb6dacb4f868dfa49767e025ab93a46e29130ae86152884c4a85524"
        );
    }

    #[test]
    fn provider_identity_invalid_capture_is_refused() {
        for provider in [
            None,
            Some(Value::Null),
            Some(json!("future")),
            Some(json!("")),
            Some(json!(7)),
            Some(json!({})),
        ] {
            let mut agent =
                json!({"id":"agent","name":"Agent","rootId":"root","threadId":"thread"});
            if let Some(provider) = &provider {
                agent["provider"] = provider.clone();
            }
            let params = json!({"threadId":"thread","studioUsage":{"kind":"response","responseId":"r","model":"gpt-test","evidence":"live","tokens":{"uncachedInput":12,"cacheReadInput":3,"cacheWriteInput":2,"output":7}}});
            assert!(
                PreparedCapture::event(
                    &agent,
                    "thread/tokenUsage/updated",
                    &params,
                    1.,
                    2.,
                    CaptureSource::Live,
                    Default::default()
                )
                .is_err(),
                "provider {provider:?}"
            );
        }
    }

    #[test]
    fn provider_identity_capture_preserves_known_typed_usage() {
        for provider in ProviderKind::ALL {
            let mut agent =
                json!({"id":"agent","name":"Agent","rootId":"root","threadId":"thread"});
            agent["provider"] = json!(provider.key());
            let params = json!({"threadId":"thread","studioUsage":{"kind":"response","responseId":"r","model":"model","evidence":"live","tokens":{"uncachedInput":12,"cacheReadInput":3,"cacheWriteInput":2,"output":7}}});
            let prepared = PreparedCapture::event(
                &agent,
                "thread/tokenUsage/updated",
                &params,
                1.,
                2.,
                CaptureSource::Live,
                Default::default(),
            )
            .expect("known identity");
            let decision = decide(
                CaptureSnapshot::from_records(Arc::new(prepared), CaptureRows::default())
                    .expect("snapshot"),
                Default::default(),
            )
            .expect("capture");
            let usage = &decision
                .rows
                .iter()
                .find(|(table, _)| *table == CaptureTable::Usage)
                .expect("usage row")
                .1;
            assert_eq!(usage["studioUsage"], params["studioUsage"]);
            let mut malformed = params.clone();
            malformed["studioUsage"]["tokens"]["uncachedInput"] = json!("invalid");
            assert!(
                PreparedCapture::event(
                    &agent,
                    "thread/tokenUsage/updated",
                    &malformed,
                    1.,
                    2.,
                    CaptureSource::Live,
                    Default::default()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn typed_usage_legacy_claude_totals_are_preserved_without_invented_categories() {
        let agent = json!({"id":"agent","name":"Agent","rootId":"root","provider":"claude","threadId":"thread"});
        let prepared=PreparedCapture::event(&agent,"thread/tokenUsage/updated",&json!({"threadId":"thread","tokenUsage":{"total":{"totalTokens":23},"last":{"totalTokens":7}}}),1.0,2.0,CaptureSource::Live, studio_runtime_schema::usage::ledger::LedgerEpoch::Current).expect("supported pre-contract notification");
        let decision = decide(
            CaptureSnapshot::from_records(Arc::new(prepared), CaptureRows::default())
                .expect("snapshot"),
            studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
        )
        .expect("legacy capture");
        let row = &decision
            .rows
            .iter()
            .find(|(table, _)| *table == CaptureTable::Usage)
            .expect("legacy usage row")
            .1;
        assert_eq!(row["total"]["totalTokens"], 23);
        assert_eq!(row["last"]["totalTokens"], 7);
        assert!(row.get("studioUsage").is_none());
        assert!(row["last"].get("inputTokens").is_none());
        assert!(row["last"].get("outputTokens").is_none());
    }

    #[test]
    fn typed_usage_final_replay_is_immutable_and_legacy_is_recoverable() {
        let agent = json!({"id":"agent","name":"Agent","rootId":"root","provider":"claude","threadId":"thread","model":"claude"});
        let observation = json!({"kind":"response","responseId":"response","model":"claude","evidence":"live","tokens":{"uncachedInput":10,"cacheReadInput":3,"cacheWriteInput":2,"output":7}});
        let prepare = |observation| {
            Arc::new(
                PreparedCapture::event(
                    &agent,
                    "thread/tokenUsage/updated",
                    &json!({"threadId":"thread","studioUsage":observation}),
                    1.0,
                    2.0,
                    CaptureSource::Live,
                    studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
                )
                .expect("input"),
            )
        };
        let first = decide(
            CaptureSnapshot::from_records(prepare(observation.clone()), CaptureRows::default())
                .expect("snapshot"),
            studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
        )
        .expect("first");
        let row = first
            .rows
            .into_iter()
            .find(|(table, _)| *table == CaptureTable::Usage)
            .expect("response")
            .1;
        let replay = |old: Value, new: Value| {
            decide(
                CaptureSnapshot::from_records(
                    prepare(new),
                    CaptureRows {
                        exact_response: Some(old),
                        ..Default::default()
                    },
                )
                .expect("replay snapshot"),
                studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
            )
            .expect("replay")
        };
        let identical = replay(row.clone(), observation.clone());
        assert!(!identical.response_replay_conflict);
        assert!(
            identical
                .rows
                .iter()
                .all(|(table, _)| *table != CaptureTable::Usage)
        );
        let mut changed = observation.clone();
        changed["tokens"]["output"] = json!(99);
        let conflict = replay(row.clone(), changed.clone());
        assert!(conflict.response_replay_conflict);
        assert!(
            conflict
                .rows
                .iter()
                .all(|(table, _)| *table != CaptureTable::Usage)
        );
        let mut provisional = row;
        provisional["studioUsage"]["evidence"] = json!("legacy");
        changed["evidence"] = json!("recovered");
        let recovered = replay(provisional, changed.clone());
        assert_eq!(
            recovered
                .rows
                .iter()
                .find(|(table, _)| *table == CaptureTable::Usage)
                .expect("replacement")
                .1["studioUsage"],
            changed
        );
    }

    #[test]
    fn typed_usage_capture_persists_normalized_response() {
        let agent = json!({"id":"agent","name":"Agent","rootId":"root","provider":"claude","threadId":"thread","model":"claude-sonnet-4-5"});
        let observation = json!({"kind":"response","responseId":"response","model":"claude-sonnet-4-5","evidence":"live","tokens":{"uncachedInput":10,"cacheReadInput":3,"cacheWriteInput":2,"output":7}});
        let prepared = PreparedCapture::event(
            &agent,
            "thread/tokenUsage/updated",
            &json!({"threadId":"thread","turnId":"turn","studioUsage":observation}),
            1.0,
            2.0,
            CaptureSource::Live,
            studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
        )
        .expect("capture input");
        let snapshot = CaptureSnapshot::from_records(Arc::new(prepared), CaptureRows::default())
            .expect("snapshot");
        let decision = decide(
            snapshot,
            studio_runtime_schema::usage::ledger::LedgerEpoch::Current,
        )
        .expect("capture decision");
        let row = &decision
            .rows
            .iter()
            .find(|(table, _)| *table == CaptureTable::Usage)
            .expect("usage row")
            .1;
        assert_eq!(row["studioUsage"], observation);
        assert_eq!(row["responseId"], "response");
    }
}

#[cfg(test)]
mod identity_read_tests {
    use super::*;

    #[test]
    fn identity_check_rejects_stale_account_and_epoch() {
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL)")
            .expect("table");
        let current = r#"{"id":"agent","rootId":"agent","status":"idle","accountKey":"account","epoch":3,"threadId":"thread"}"#;
        db.execute("INSERT INTO runtime_agents VALUES ('agent',?1)", [current])
            .expect("agent");
        let agent: Value = studio_json::from_str(current).expect("agent fixture");
        assert!(check_identity(&db, &agent).is_ok());
        for stale in [
            r#"{"id":"agent","rootId":"agent","status":"idle","accountKey":"other","epoch":3,"threadId":"thread"}"#,
            r#"{"id":"agent","rootId":"agent","status":"idle","accountKey":"account","epoch":2,"threadId":"thread"}"#,
        ] {
            let agent: Value = studio_json::from_str(stale).expect("stale agent fixture");
            assert!(matches!(
                check_identity(&db, &agent),
                Err(CaptureError::StaleIdentity)
            ));
        }
    }

    #[tokio::test]
    async fn identity_check_reads_a_snapshot_and_takes_no_writer_admission() {
        let directory = tempfile::tempdir().expect("temporary state");
        let store = RuntimeStore::open(Some(directory.path()))
            .await
            .expect("store");
        let before = store.writer_admissions();
        let result = check_agent_identity(&store, vec![json!({"id":"missing"})]).await;
        assert!(matches!(result, Err(CaptureError::StaleIdentity)));
        assert_eq!(store.writer_admissions(), before);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod boundary_timing_tests {
    use super::*;

    #[test]
    fn model_boundary_stamps_first_delta_once_without_repeated_turn_writes() {
        for (provider, model_delta) in [
            ("codex", "item/reasoning/textDelta"),
            ("claude", "item/modelOutput/firstDelta"),
        ] {
            let agent =
                json!({"id":"agent","name":"Agent","provider":provider,"threadId":"thread"});
            let mut turn = None;
            for (index, (method, delta, status)) in [
                ("turn/started", "", "running"),
                ("item/agentMessage/delta", "", ""),
                (model_delta, "thought", ""),
                ("item/agentMessage/delta", "answer", ""),
                ("item/agentMessage/delta", "more", ""),
                ("turn/completed", "", "interrupted"),
                ("item/modelOutput/firstDelta", "", ""),
            ]
            .into_iter()
            .enumerate()
            {
                let at = index as f64 + 1.;
                let params = json!({"threadId":"thread","turnId":"turn","delta":delta,"turn":{"id":"turn","status":status}});
                let prepared = PreparedCapture::event(
                    &agent,
                    method,
                    &params,
                    at,
                    at,
                    CaptureSource::Live,
                    Default::default(),
                )
                .unwrap()
                .at_provider_boundary(Some(
                    studio_runtime_schema::provider_timing::BoundaryStamp {
                        boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
                        unix_seconds: at,
                        monotonic_ns: (at * 1000000.) as u64,
                    },
                ))
                .unwrap();
                let decision = decide(
                    CaptureSnapshot::from_records(
                        Arc::new(prepared),
                        CaptureRows {
                            turn: turn.clone(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                    Default::default(),
                )
                .unwrap();
                let changed = decision
                    .rows
                    .into_iter()
                    .find(|(table, _)| *table == CaptureTable::Turns)
                    .map(|(_, row)| row);
                if index == 4 || index == 6 {
                    assert!(changed.is_none(), "repeated stamps do not write a turn");
                }
                if changed.is_some() {
                    turn = changed;
                }
            }
            let row = turn.unwrap();
            assert_eq!(row["boundaryTiming"]["firstEvent"]["monotonicNs"], 1000000);
            assert_eq!(row["boundaryTiming"]["firstToken"]["monotonicNs"], 3000000);
            assert_eq!(row["boundaryTiming"]["terminal"]["monotonicNs"], 6000000);
        }
    }

    #[test]
    fn boundary_stamps_keep_first_occurrences_across_retries_interrupts_and_no_output() {
        for provider in ["codex", "claude"] {
            let agent =
                json!({"id":"agent","name":"Agent","provider":provider,"threadId":"thread"});
            let mut turn = None;
            for (method, time, extra) in [
                (
                    "analytics/adapterBoundary",
                    1.,
                    json!({"boundaryTiming":{"requestId":"request","accepted":{"bootId":"00000000-0000-0000-0000-000000000001","unixSeconds":10.,"monotonicNs":1000000},"written":null}}),
                ),
                ("turn/started", 3., json!({"turn":{"id":"turn"}})),
                (
                    "analytics/adapterBoundary",
                    4.,
                    json!({"boundaryTiming":{"requestId":"request","accepted":{"bootId":"00000000-0000-0000-0000-000000000001","unixSeconds":20.,"monotonicNs":2000000},"written":{"bootId":"00000000-0000-0000-0000-000000000001","unixSeconds":21.,"monotonicNs":2500000}}}),
                ),
                (
                    "turn/completed",
                    5.,
                    json!({"turn":{"id":"turn","status":"interrupted"}}),
                ),
            ] {
                let mut params = json!({"threadId":"thread","turnId":"turn"});
                params
                    .as_object_mut()
                    .unwrap()
                    .extend(extra.as_object().unwrap().clone());
                let stamp = (!method.starts_with("analytics/")).then_some(
                    studio_runtime_schema::provider_timing::BoundaryStamp {
                        boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
                        unix_seconds: 100. - time,
                        monotonic_ns: (time * 1000000.) as u64,
                    },
                );
                let prepared = PreparedCapture::event(
                    &agent,
                    method,
                    &params,
                    time,
                    time,
                    CaptureSource::Live,
                    Default::default(),
                )
                .unwrap()
                .at_provider_boundary(stamp)
                .unwrap();
                let snapshot = CaptureSnapshot::from_records(
                    Arc::new(prepared),
                    CaptureRows {
                        turn: turn.clone(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let decision = decide(snapshot, Default::default()).unwrap();
                if let Some((_, row)) = decision
                    .rows
                    .into_iter()
                    .find(|(table, _)| *table == CaptureTable::Turns)
                {
                    turn = Some(row);
                }
            }
            let row = turn.expect("timed turn");
            assert_eq!(row["boundaryTiming"]["accepted"]["monotonicNs"], 1000000);
            assert_eq!(row["boundaryTiming"]["firstEvent"]["monotonicNs"], 3000000);
            assert_eq!(row["boundaryTiming"]["acceptedToWrittenMs"], 1.5);
            assert_eq!(row["boundaryTiming"]["acceptedToTerminalMs"], 4.);
            assert!(row["boundaryTiming"]["firstToken"].is_null());
            assert_eq!(row["status"], "interrupted");
        }
    }
}
