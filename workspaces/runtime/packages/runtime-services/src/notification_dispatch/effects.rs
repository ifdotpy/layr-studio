//! Injection seam for effects owned by other notification subsystems.
use super::{DispatchOutcome, NotificationDispatchError, ProviderDeliveryIdentity};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use studio_json::Value;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::{
    AgentId, AgentRecord, StoredTransferObject, StoredTransferValue, Transaction,
};

/// Only the agent fields consumed by notification observations. Borrow strings and
/// typed historical evidence; never retain or decode unrelated agent extensions.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationAgent<'a> {
    id: &'a str,
    root_id: &'a str,
    account_key: &'a str,
    epoch: i64,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    name: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    parent_id: Field<&'a str>,
    #[serde(default, borrow, skip_serializing_if = "Field::is_missing")]
    account_history: Field<
        Cow<
            'a,
            StoredTransferValue<
                Vec<
                    StoredTransferObject<
                        studio_runtime_store::transfer_record::TransferAccountHistoryEntry,
                    >,
                >,
            >,
        >,
    >,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    provider: Field<studio_runtime_schema::ProviderKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    thread_id: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub(crate) turn_id: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub(crate) in_flight: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    events: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    model: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    effort: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    daybreak_enabled: Field<bool>,
    #[serde(default, borrow, skip_serializing_if = "Field::is_missing")]
    cyber_access_program: Field<Cow<'a, StoredTransferValue<String>>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    cwd: Field<&'a str>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    deleted_at: Field<f64>,
}

impl<'a> From<&'a AgentRecord> for NotificationAgent<'a> {
    fn from(agent: &'a AgentRecord) -> Self {
        fn field_ref<'a, T, U>(field: &'a Field<T>, map: impl FnOnce(&'a T) -> U) -> Field<U> {
            match field {
                Field::Missing => Field::Missing,
                Field::Null => Field::Null,
                Field::Value(value) => Field::Value(map(value)),
            }
        }
        Self {
            id: agent.id.as_ref(),
            root_id: agent.root_id.as_ref(),
            account_key: &agent.account_key,
            epoch: agent.epoch,
            name: agent.name.as_deref().map_or(Field::Missing, Field::Value),
            parent_id: agent
                .parent_id
                .as_deref()
                .map_or(Field::Missing, Field::Value),
            account_history: field_ref(&agent.account_history, Cow::Borrowed),
            provider: agent.provider.clone(),
            thread_id: agent
                .thread_id
                .as_deref()
                .map_or(Field::Missing, Field::Value),
            turn_id: agent
                .turn_id
                .as_deref()
                .map_or(Field::Missing, Field::Value),
            in_flight: agent.in_flight.map_or(Field::Missing, Field::Value),
            events: agent.events.map_or(Field::Missing, Field::Value),
            model: agent.model.as_deref().map_or(Field::Missing, Field::Value),
            effort: field_ref(&agent.effort, String::as_str),
            fast_mode: agent.fast_mode.map_or(Field::Missing, Field::Value),
            daybreak_enabled: agent.daybreak_enabled.map_or(Field::Missing, Field::Value),
            cyber_access_program: field_ref(&agent.cyber_access_program, Cow::Borrowed),
            cwd: agent.cwd.as_deref().map_or(Field::Missing, Field::Value),
            deleted_at: agent.deleted_at.map_or(Field::Missing, Field::Value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_agent_view_borrows_strings_and_skips_unconsumed_fields() {
        let mut raw = studio_json::json!({
            "id":"agent", "rootId":"root", "epoch":1, "accountKey":"default",
            "events":0, "effort":null, "status":[], "startAttempt":false,
            "unrelatedTranscript":"untouched".repeat(8192)
        });
        let view = NotificationAgent::deserialize(&raw).unwrap();
        assert_eq!(view.id.as_ptr(), raw["id"].as_str().unwrap().as_ptr());
        assert_eq!(
            studio_json::to_string(&SharedJson::agent_projection(&view)).unwrap(),
            r#"{"accountKey":"default","effort":null,"epoch":1,"events":0,"id":"agent","rootId":"root"}"#
        );
        raw["events"] = studio_json::json!("invalid");
        assert!(NotificationAgent::deserialize(&raw).is_err());
    }

    #[test]
    fn notification_agent_view_borrows_typed_history_without_cloning() {
        let mut agent = AgentRecord::test_defaults();
        agent.account_history = Field::Value(StoredTransferValue::Known(Vec::new()));
        let view = NotificationAgent::from(&agent);
        let Field::Value(Cow::Borrowed(history)) = view.account_history else {
            panic!("typed history must be borrowed");
        };
        assert!(std::ptr::eq(
            history,
            agent.account_history.value().unwrap()
        ));
    }
}

/// JSON payload shared by immutable publication observations. Serialization is
/// transparent so durable journal records retain the same shape.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedJson(Arc<Value>);

impl SharedJson {
    pub fn new(value: Value) -> Self {
        Self(Arc::new(value))
    }

    pub fn agent_projection(agent: &NotificationAgent<'_>) -> Self {
        Self::new(studio_runtime_store::projection_value(agent))
    }

    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Deref for SharedJson {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SharedJson {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl Serialize for SharedJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SharedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::new)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NotificationDependency {
    /// Older builds recorded this dependency of the removed team token budget;
    /// stored receipts still decode. No current step needs it.
    #[serde(rename = "agent-move/cache-capture")]
    MoveCache,
    #[serde(rename = "accounts/login-completed")]
    AccountLogin,
    #[serde(rename = "accounts/refresh")]
    AccountRefresh,
    #[serde(rename = "accounts/rate-limits")]
    RateLimits,
    #[serde(rename = "analytics/token-rate")]
    TokenRate,
    #[serde(rename = "voice/realtime")]
    Realtime,
    #[serde(rename = "monitors/output")]
    MonitorOutput,
    #[serde(rename = "turn-recovery/thread-unloaded")]
    ThreadUnloaded,
    #[serde(rename = "native-notices/account")]
    AccountNotice,
    #[serde(rename = "native-limits/refresh")]
    RefreshLimits,
    #[serde(rename = "scheduler/turn-lifecycle")]
    TurnLifecycle,
    #[serde(rename = "scheduler/user-message-receipt")]
    UserMessageReceipt,
}
impl NotificationDependency {
    pub fn name(self) -> &'static str {
        match self {
            Self::MoveCache => "agent-move/cache-capture",
            Self::AccountLogin => "accounts/login-completed",
            Self::AccountRefresh => "accounts/refresh",
            Self::RateLimits => "accounts/rate-limits",
            Self::TokenRate => "analytics/token-rate",
            Self::Realtime => "voice/realtime",
            Self::MonitorOutput => "monitors/output",
            Self::ThreadUnloaded => "turn-recovery/thread-unloaded",
            Self::AccountNotice => "native-notices/account",
            Self::RefreshLimits => "native-limits/refresh",
            Self::TurnLifecycle => "scheduler/turn-lifecycle",
            Self::UserMessageReceipt => "scheduler/user-message-receipt",
        }
    }
}

/// A finite timestamp in seconds, shared by live and recovered observations.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ObservationTime(f64);

impl ObservationTime {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ObservationTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| serde::de::Error::custom("non-finite observation time"))
    }
}

/// Immutable observation handed to analytics/token-rate owners after commit.
/// The effect kind plus `delivery` is the idempotency key; retain account and generation.
/// The caller stages these serializable values in its existing effect journal
/// before committing, and drains the journal after commit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NotificationObservation {
    pub agent: Option<SharedJson>,
    pub method: String,
    pub params: SharedJson,
    pub delivery: ProviderDeliveryIdentity,
    pub observed_at: ObservationTime,
    pub provider_at: ObservationTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum NotificationPostCommit {
    Analytics {
        observation: NotificationObservation,
        /// The team token budget total of older builds. Stored batches still
        /// decode; current builds write `None` and read nothing from it.
        budget_capture_value: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage_conflict: Option<Box<studio_runtime_schema::usage::ledger::UsageReplayConflict>>,
        /// Budget migration epoch bound when this effect is durably logged.
        #[serde(
            default,
            skip_serializing_if = "studio_runtime_schema::usage::ledger::LedgerEpoch::is_legacy"
        )]
        ledger_since: studio_runtime_schema::usage::ledger::LedgerEpoch,
        #[serde(default)]
        usage_unknown: bool,
    },
    AnalyticsStream {
        observation: NotificationObservation,
        metrics: NotificationStreamMetrics,
    },
    /// `Analytics` after its rows were captured inside the journal commit. The
    /// sink only projects the token rate; the store observer owns state changes.
    AnalyticsCaptured(NotificationObservation),
    /// `AnalyticsStream` after capture inside the journal commit; the store
    /// observer already published its state changes.
    AnalyticsStreamCaptured(NotificationObservation),
    TokenRateStream(NotificationObservation),
    TokenRateAssociation(NotificationObservation),
    TokenRateUsage(NotificationObservation),
    /// Reconstructed from retained native history after a restart. This has a
    /// recovery identity rather than an invented provider delivery cursor.
    Recovered {
        identity: String,
        observation: RecoveredNotificationObservation,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecoveredNotificationObservation {
    pub agent: Value,
    pub method: String,
    pub params: Value,
    pub observed_at: ObservationTime,
}

/// One committed publication unit. `sequence` is monotone per retained account
/// buffer; it is independent of original provider cursors. Persist this entire
/// vector with the updated buffer before commit. Recover batches in sequence
/// order, and effects in vector order. A sink deduplicates (account,sequence,index).
/// Never recover these values in a hashed effect-ID order.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct NotificationPublicationBatch {
    pub sequence: u64,
    pub effects: Vec<NotificationPostCommit>,
    pub analytics_overflow: u64,
}
impl NotificationPublicationBatch {
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty() && self.analytics_overflow == 0
    }
}

/// Aggregate matching Python StreamBuffer's analytics closure. The consumer
/// captures `observation` once, then corrects notifications per hour and stream
/// counters using these totals (without retaining each raw delta).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct NotificationStreamMetrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_delta_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    pub hours: Vec<NotificationHourMetrics>,
    pub bytes: u64,
    pub characters: u64,
    pub lines: u64,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NotificationHourMetrics {
    pub hour: u64,
    pub count: u64,
    pub encoded_bytes: u64,
}

/// Transactional effects only: no provider call, blocking IO, or await here.
/// The worker supplies agent_move::capture_cache; it must commit with the
/// notification receipt.
#[derive(Clone, Copy)]
pub struct NotificationClock {
    pub observed_at: ObservationTime,
    pub boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
}

pub trait NotificationEffects {
    /// Pure staging only. A receipt replay never calls this method.
    fn post_commit(
        &self,
        _before: Option<&NotificationAgent<'_>>,
        _after: Option<&NotificationAgent<'_>>,
        _frame: &Value,
        _identity: &ProviderDeliveryIdentity,
        _clock: NotificationClock,
        _outcome: &DispatchOutcome,
    ) -> Vec<NotificationPostCommit> {
        Vec::new()
    }

    fn account_notice(
        &self,
        _tx: &Transaction<'_>,
        _method: &str,
        _params: &Value,
        _identity: &ProviderDeliveryIdentity,
        _at: f64,
    ) -> Result<(), NotificationDispatchError> {
        Err(NotificationDispatchError::Dependency(
            NotificationDependency::AccountNotice.name(),
        ))
    }

    fn generation_started(&self) -> Result<(), NotificationDispatchError> {
        Err(NotificationDispatchError::Dependency(
            NotificationDependency::TokenRate.name(),
        ))
    }

    fn capture_move_cache(
        &self,
        _tx: &Transaction<'_>,
        _agent: &AgentId,
        _params: &Value,
        _usage: &studio_runtime_schema::usage::UsageObservation,
        _at: f64,
    ) -> Result<(), NotificationDispatchError> {
        Err(NotificationDispatchError::Dependency(
            NotificationDependency::MoveCache.name(),
        ))
    }
}
#[derive(Default)]
pub struct UnavailableNotificationEffects;
impl NotificationEffects for UnavailableNotificationEffects {}

#[cfg(test)]
mod finite_observation_tests {
    use super::*;

    #[test]
    fn observation_cannot_hold_nonfinite_times() {
        for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ObservationTime::new(time).is_none());
        }
        assert!(studio_json::from_str::<ObservationTime>("null").is_err());
        assert_eq!(
            studio_json::from_str::<ObservationTime>("1.5")
                .unwrap()
                .get(),
            1.5
        );
    }
}
