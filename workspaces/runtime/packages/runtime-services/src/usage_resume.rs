//! Durable usage/authentication continuation decisions; observations never invoke a provider.
use crate::continuation::{
    ContinuationError, ContinuationUpdate as U, Decision, Effect, POLL_SECONDS, ProposedRecord,
    Result, Snapshot, UsageDraft, WaitStatus, flag, now_checked, text,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use studio_runtime_providers::ProviderKind;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::provider_errors::{
    KnownCodexErrorKind, known_error_kind, raw_error_kind,
};
use studio_runtime_schema::usage_limits::{
    AgentLimitProjection, ClaudeRateInfo, ClaudeRateLimit, ClaudeRateStatus, ClaudeRateType,
    EligibilityBucket, EligibilityState, LimitRecovery, Overage, RateLimitBucket, RecoveryAction,
    RecoveryTiming, UsageEligibility, UsageLimitsData,
};
use studio_runtime_store::AgentRecord;

const RESET_GRACE: f64 = 5.0;
const RESUME_MIN: f64 = 60.0;
const AUTH_BACKOFF: [f64; 3] = [180.0, 600.0, 1800.0];
const FRESH_SECONDS: f64 = 5.0;
pub const CONTINUATION: &str = "The previous turn stopped because this account reached a usage or rate limit. Check the current task and conversation state, then continue the same task. Do not repeat completed work.";
pub const AUTH_CONTINUATION: &str = "The previous turn stopped because this account sign-in failed. Check the current task and conversation state, then continue the same task. Do not repeat completed work.";
const STOPPED: &str = "The chat stopped or changed accounts before automatic resume.";
const CHANGED: &str = "The failed turn or chat state changed before automatic resume.";
pub const TASK_CHANGED: &str = "The assigned task changed before automatic continuation.";
const OFF: &str = "Automatic resume is off for this chat.";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    UsageLimit,
    RateLimit,
    Auth,
}
#[derive(Clone)]
pub struct LimitsObservation {
    response: studio_runtime_schema::usage_limits::UsageLimitsResponse,
    stale: bool,
    marker: Option<String>,
}
impl LimitsObservation {
    pub(crate) fn from_response(
        response: studio_runtime_schema::usage_limits::UsageLimitsResponse,
    ) -> Self {
        Self {
            response,
            stale: false,
            marker: None,
        }
    }
    pub fn from_record(mut raw: Value, marker: Option<String>) -> Result<Self> {
        if let Some((path, _)) =
            studio_runtime_schema::first_error("continuation/ContinuationLimitsObservation", &raw)
                .map_err(|_| ContinuationError::Invalid("/limits/schema".into()))?
        {
            return Err(ContinuationError::Invalid(format!("/limits{path}")));
        }
        let stale = flag(&raw, "stale");
        if let Some(object) = raw.as_object_mut() {
            object.remove("stale");
        }
        let JsonObject(response) = serde_json::from_value(raw)
            .map_err(|_| ContinuationError::Invalid("/limits".into()))?;
        Ok(Self {
            response,
            stale,
            marker,
        })
    }
    pub fn account_key(&self) -> &str {
        &self.response.account_key
    }
    fn data(&self) -> Option<&UsageLimitsData> {
        self.response.data.as_ref().map(|data| &data.0)
    }
    fn fresh_for(&self, key: &str, after: f64, strict: bool) -> bool {
        self.response.error.as_deref().is_none_or(str::is_empty)
            && !self.stale
            && self.data().is_none_or(supported)
            && self.account_key() == key
            && self
                .response
                .at
                .is_some_and(|at| if strict { at > after } else { at >= after })
    }
    pub fn fresh_after(&self, after: f64, now: f64) -> bool {
        self.fresh_for(self.account_key(), after, true)
            && self.fresh_for(self.account_key(), now - FRESH_SECONDS, false)
    }
    pub fn reset_at(&self, agent: &AgentRecord) -> Option<f64> {
        self.data().and_then(|data| reset_at(data, agent))
    }
    pub fn allowed(&self, agent: &AgentRecord, now: f64) -> bool {
        self.data().is_some_and(|data| allowed(data, agent, now))
    }
}
/// Read projection of the same model gate used by automatic continuation.
pub fn eligibility(
    observation: &LimitsObservation,
    agent: &AgentRecord,
    now: f64,
) -> UsageEligibility {
    let key = if agent.account_key.is_empty() {
        "default"
    } else {
        &agent.account_key
    };
    let fresh = observation.fresh_for(key, now - FRESH_SECONDS, false);
    let data = observation.data();
    let applicable: Vec<JsonObject<EligibilityBucket>> = data
        .into_iter()
        .flat_map(|data| {
            buckets(data, agent).map(|(id, _)| {
                JsonObject(EligibilityBucket {
                    id: id.to_owned(),
                    overage_allowed: overage_allowed(data, agent, id),
                })
            })
        })
        .collect();
    let info = data
        .filter(|data| supported(data))
        .and_then(|data| rate_info(data.claude_rate_limit.as_ref()));
    let overage =
        if info.is_some_and(|info| info.overage_status == Some(ClaudeRateStatus::Rejected)) {
            Overage::Rejected
        } else if info.is_some_and(|info| {
            info.is_using_overage == Some(true) || info.overage_in_use == Some(true)
        }) {
            Overage::InUse
        } else if applicable.iter().any(|bucket| bucket.0.overage_allowed) {
            Overage::Allowed
        } else {
            Overage::Unavailable
        };
    let valid_until = observation.response.at.map(|at| {
        data.into_iter()
            .flat_map(|data| {
                buckets(data, agent)
                    .filter(|(id, _)| !overage_allowed(data, agent, id))
                    .flat_map(|(_, bucket)| {
                        [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                            .into_iter()
                            .flatten()
                            .filter_map(|window| window.0.resets_at)
                    })
            })
            .fold(at + FRESH_SECONDS, f64::min)
    });
    let allowed = observation.allowed(agent, now);
    UsageEligibility {
        agent_id: agent.id.0.clone(),
        account_key: key.to_owned(),
        model: agent.model.clone().unwrap_or_default(),
        provider: agent.provider.as_ref().map_or("", |p| p.key()).to_owned(),
        state: if !fresh {
            EligibilityState::Stale
        } else if allowed {
            EligibilityState::Allowed
        } else {
            EligibilityState::Blocked
        },
        valid_until,
        buckets: applicable,
        overage,
    }
}

/// Agent-specific presentation, derived from the same typed gate as continuation.
pub fn agent_limit_projection(
    limits: &LimitsObservation,
    agent: &AgentRecord,
    now: f64,
) -> AgentLimitProjection {
    let eligibility = eligibility(limits, agent, now);
    if eligibility.account_key != limits.account_key() {
        return AgentLimitProjection {
            eligibility,
            recovery: None,
        };
    }
    let kind = agent.error.as_ref().and_then(|error| error.known_kind());
    let recovery = kind
        .filter(|kind| matches!(kind, KnownCodexErrorKind::UsageLimitExceeded | KnownCodexErrorKind::RateLimitExceeded))
        .map(|kind| {
            let previous_reset = agent.usage_continuation()
                .filter(|resume| {
                    use studio_runtime_schema::continuation_snapshot::UsageContinuationCause;
                    matches!(resume.cause, Some(UsageContinuationCause::UsageLimit | UsageContinuationCause::RateLimit))
                        && resume.account_key.as_str() == limits.account_key()
                        && resume.epoch.as_u64() == u64::try_from(agent.epoch).ok()
                        && agent.thread_id.as_deref() == Some(resume.thread_id.as_str())
                        && agent.last_completed_turn.as_ref().map(String::as_str) == Some(resume.turn_id.as_str())
                })
                .and_then(|resume| resume.reset_at.value().map(|time| time.get()).filter(|reset| *reset > resume.failed_at.get()));
            let reset_at = limits.reset_at(agent).or(previous_reset);
            let confirmed = previous_reset.filter(|previous| limits.response.read_at.is_some_and(|read| read > *previous)
                && limits.response.error.as_deref().is_none_or(str::is_empty)
                && limits.allowed(agent, now));
            let timing = match (confirmed, reset_at) {
                (Some(reset_at), _) => RecoveryTiming::Confirmed { reset_at },
                (None, Some(reset_at)) if reset_at > now => RecoveryTiming::Waiting { reset_at },
                (None, Some(reset_at)) => RecoveryTiming::Unconfirmed { reset_at },
                (None, None) => RecoveryTiming::Unknown,
            };
            let bucket = limits.data().and_then(|data| data.rate_limits.as_ref()).map(|bucket| &bucket.0);
            let reached = bucket.and_then(|bucket| bucket.rate_limit_reached_type.as_deref());
            let reached = match (kind, reached) {
                (KnownCodexErrorKind::UsageLimitExceeded, Some("workspace_owner_credits_depleted")) => Some("workspace_owner_usage_limit_reached"),
                (KnownCodexErrorKind::UsageLimitExceeded, Some("workspace_member_credits_depleted")) => Some("workspace_member_usage_limit_reached"),
                (_, reached) => reached,
            };
            let fallback = ("Usage limit reached", "View account limits for reset times and available credits.", RecoveryAction::None);
            let (title, message, action) = if agent.provider.as_ref() != Some(&ProviderKind::Codex)
                || eligibility.state == EligibilityState::Stale || bucket.is_none() {
                fallback
            } else {
                match reached {
                    Some("workspace_owner_credits_depleted") => ("Workspace credits depleted", "Your workspace is out of credits. Add credits to continue using Codex.", RecoveryAction::Link { label: "Add workspace credits".into(), href: "https://chatgpt.com/admin/billing?codex_credit_action=add_credits".into() }),
                    Some("workspace_member_credits_depleted") => ("Workspace credits depleted", "Your workspace is out of credits. Ask your workspace owner to add credits.", RecoveryAction::AskOwner { request: "Codex says our workspace is out of credits. Can you add credits?".into() }),
                    Some("workspace_owner_usage_limit_reached") => ("Workspace usage limit reached", "Increase your workspace usage limit to continue using Codex.", RecoveryAction::Link { label: "Open workspace usage limits".into(), href: "https://chatgpt.com/admin/usage-limits/workspace".into() }),
                    Some("workspace_member_usage_limit_reached") => ("Workspace usage limit reached", "Ask your workspace owner to increase your usage limit.", RecoveryAction::AskOwner { request: "Codex says I reached my workspace usage limit. Can you increase my limit?".into() }),
                    Some(_) => fallback,
                    None => match bucket.and_then(|bucket| bucket.plan_type.as_deref()) {
                        Some("pro" | "plus" | "pro_lite") => ("Usage limit reached", "Wait for the limit to reset, or add credits in ChatGPT.", RecoveryAction::Link { label: "View credits in ChatGPT".into(), href: "https://chatgpt.com/codex/settings/usage?credits_modal=true".into() }),
                        Some("free" | "go") => ("Usage limit reached", "Wait for the limit to reset, or review your plan in ChatGPT.", RecoveryAction::Link { label: "View ChatGPT usage".into(), href: "https://chatgpt.com/codex/settings/usage".into() }),
                        _ => fallback,
                    },
                }
            };
            LimitRecovery { title: title.into(), message: message.into(), action, timing }
        });
    AgentLimitProjection {
        eligibility,
        recovery,
    }
}

pub fn next_check(reset: Option<f64>, now: f64) -> f64 {
    reset
        .filter(|r| *r > now)
        .map_or(now + POLL_SECONDS, |r| r + RESET_GRACE)
}
fn auth_backoff(attempt: u64) -> f64 {
    AUTH_BACKOFF[attempt.min(2) as usize]
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeUsagePolicy {
    model_buckets: std::collections::BTreeMap<String, String>,
    allowed_overage_statuses: Vec<ClaudeRateStatus>,
}
fn claude_policy() -> &'static ClaudeUsagePolicy {
    static POLICY: std::sync::OnceLock<ClaudeUsagePolicy> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        // The JavaScript module exports a JSON string so both runtimes consume
        // one contract, and the existing bridge source hash includes it.
        let source = include_str!("../../../../providers/apps/claude-bridge/usage-policy.ts");
        let (_, json) = source
            .split_once("export default `")
            .expect("usage policy export");
        let json = json.trim().strip_suffix("`;").expect("usage policy string");
        serde_json::from_str(json).expect("shared Claude usage policy")
    })
}
fn model_family(agent: &AgentRecord) -> Option<&'static str> {
    if agent.provider.as_ref() != Some(&ProviderKind::Claude) {
        return None;
    }
    let model = agent.model.as_deref().unwrap_or_default();
    claude_policy()
        .model_buckets
        .keys()
        .find(|family| {
            model == family.as_str()
                || model == format!("claude-{family}")
                || model.starts_with(&format!("claude-{family}-"))
        })
        .map(String::as_str)
}
fn relevant_bucket(id: &str, agent: &AgentRecord) -> bool {
    model_family(agent).is_none_or(|family| {
        claude_policy()
            .model_buckets
            .iter()
            .all(|(name, bucket)| bucket != id || name == family)
    })
}
fn buckets<'a>(
    data: &'a UsageLimitsData,
    agent: &'a AgentRecord,
) -> impl Iterator<Item = (&'a str, &'a RateLimitBucket)> {
    data.rate_limits
        .iter()
        .map(|bucket| (bucket.0.limit_id.as_deref().unwrap_or_default(), &bucket.0))
        .chain(
            data.rate_limits_by_limit_id
                .iter()
                .flat_map(|buckets| buckets.iter().map(|(id, bucket)| (id.as_str(), &bucket.0))),
        )
        .filter(|(id, _)| relevant_bucket(id, agent))
}
fn rate_info(rate: Option<&ClaudeRateLimit>) -> Option<&ClaudeRateInfo> {
    match rate {
        Some(ClaudeRateLimit::Known(info)) => Some(info),
        _ => None,
    }
}
fn supported(data: &UsageLimitsData) -> bool {
    let valid =
        |rate: Option<&ClaudeRateLimit>| !matches!(rate, Some(ClaudeRateLimit::Unsupported(_)));
    valid(data.claude_rate_limit.as_ref())
        && data
            .rate_limits
            .as_ref()
            .is_none_or(|b| valid(b.0.claude_rate_limit.as_ref()))
        && data.rate_limits_by_limit_id.as_ref().is_none_or(|buckets| {
            buckets
                .values()
                .all(|b| valid(b.0.claude_rate_limit.as_ref()))
        })
}
fn overage_allowed(data: &UsageLimitsData, agent: &AgentRecord, id: &str) -> bool {
    if agent.provider.as_ref() != Some(&ProviderKind::Claude) || !supported(data) {
        return false;
    }
    let Some(info) = rate_info(data.claude_rate_limit.as_ref()) else {
        return false;
    };
    if info.overage_status == Some(ClaudeRateStatus::Rejected) {
        return false;
    }
    let family = model_family(agent);
    let known_bucket = id == "claude"
        || id == "claude-overage"
        || family.is_some_and(|family| claude_policy().model_buckets[family] == id);
    let known_scope = matches!(
        info.rate_limit_type,
        Some(ClaudeRateType::FiveHour | ClaudeRateType::SevenDay | ClaudeRateType::Overage)
    ) || matches!(
        (info.rate_limit_type, family),
        (Some(ClaudeRateType::SevenDayOpus), Some("opus"))
            | (Some(ClaudeRateType::SevenDaySonnet), Some("sonnet"))
    );
    known_bucket
        && known_scope
        && (info
            .overage_status
            .is_some_and(|status| claude_policy().allowed_overage_statuses.contains(&status))
            || info.is_using_overage == Some(true)
            || info.overage_in_use == Some(true))
}
fn reset_at(data: &UsageLimitsData, agent: &AgentRecord) -> Option<f64> {
    if !supported(data) {
        return None;
    }
    buckets(data, agent)
        .filter(|(id, _)| !overage_allowed(data, agent, id))
        .flat_map(|(_, bucket)| {
            let windows: Vec<_> = [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
                .map(|window| &window.0)
                .collect();
            let full = windows
                .iter()
                .any(|window| window.used_percent.is_some_and(|used| used >= 100.0));
            let event = rate_info(bucket.claude_rate_limit.as_ref());
            let blocked = event.is_some_and(|event| event.status == ClaudeRateStatus::Rejected);
            windows
                .into_iter()
                .filter(move |window| {
                    if full {
                        window.used_percent.is_some_and(|used| used >= 100.0)
                    } else {
                        blocked || bucket.rate_limit_reached_type.is_some()
                    }
                })
                .filter_map(|window| window.resets_at)
                .chain(event.filter(|_| blocked).and_then(|event| event.resets_at))
                .chain(
                    event
                        .filter(|event| event.overage_status == Some(ClaudeRateStatus::Rejected))
                        .and_then(|event| event.overage_resets_at),
                )
        })
        .reduce(f64::max)
}
fn allowed(data: &UsageLimitsData, agent: &AgentRecord, now: f64) -> bool {
    use studio_runtime_schema::queue_command::Field;
    if !supported(data)
        || data.rate_limits.is_none()
        || matches!(
            data.ordinary_usage_allowed,
            Field::Null | Field::Value(false)
        )
    {
        return false;
    }
    let mut measured = false;
    for (id, bucket) in buckets(data, agent) {
        if bucket.spend_control_reached == Some(true)
            || bucket
                .individual_limit
                .as_ref()
                .and_then(|limit| limit.0.remaining_percent)
                .is_some_and(|remaining| remaining <= 0.0)
        {
            return false;
        }
        if overage_allowed(data, agent, id) {
            measured = true;
            continue;
        }
        if bucket.rate_limit_reached_type.is_some()
            || rate_info(bucket.claude_rate_limit.as_ref())
                .is_some_and(|info| info.status == ClaudeRateStatus::Rejected)
        {
            return false;
        }
        for window in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            let Some(used) = window.0.used_percent else {
                return false;
            };
            measured = true;
            if used >= 100.0 || window.0.resets_at.is_some_and(|at| at <= now) {
                return false;
            }
        }
    }
    measured
}
/// A limits read that fails or is not current cannot show that the provider
/// reset the limit. Once the reset time that the wait holds has passed, with
/// the grace and the anti-loop floor, the wait resumes without a current read.
/// Only a reset later than the failure counts: a reset that had passed when
/// the turn failed is no evidence that the limit ends. The follow-up id is
/// fixed and the resumed wait is `started`, so it resumes once; a new limit
/// failure of the resumed turn starts a new wait.
fn reset_passed(s: &Snapshot, now: f64) -> bool {
    let failed = s.record_failed_at().unwrap_or(0.0);
    now >= failed + RESUME_MIN
        && s.record_reset_at()
            .is_some_and(|reset| reset > failed && now >= reset + RESET_GRACE)
}
/// Start the wait's continuation: one follow-up with the wait's fixed id.
fn resume(s: &Snapshot, mut r: ProposedRecord, auth: bool, now: f64) -> Decision {
    let id = format!("usage-resume:{}", s.record_id().unwrap_or(""));
    r.update(U::Status("started"));
    r.update(U::DueAt(None));
    r.update(U::StartedAt(now));
    r.update(U::Reason(None));
    let mut d = Decision::save(s, r, "usageResume", now);
    d.effects.push(Effect::Followup {
        id,
        text: if auth {
            AUTH_CONTINUATION
        } else {
            CONTINUATION
        }
        .into(),
    });
    if let Some(a) = &mut d.agent {
        a.clear_hold();
        a.queued();
    }
    d
}
fn cancel(s: &Snapshot, _r: &Value, reason: &str, now: f64) -> Decision {
    let mut r = ProposedRecord::existing(s).expect("validated record");
    r.update(U::Status("cancelled"));
    r.update(U::DueAt(None));
    r.update(U::Reason(Some(reason.to_owned())));
    Decision::save(s, r, "usageResume", now)
}
fn excluded(s: &Snapshot, key: &str) -> bool {
    let agent = s.agent_view();
    let root = if agent.id.as_str() == agent.root_id.as_str() {
        Some(agent)
    } else {
        s.ancestors_view()
            .iter()
            .find(|a| a.id.as_str() == agent.root_id.as_str())
    };
    root.and_then(|root| root.excluded_account_keys.as_ref())
        .is_some_and(|keys| keys.as_slice().iter().any(|id| id.as_str() == key))
}
/// Called after an account observation; current identity and task ownership are checked anew.
pub fn tick(s: &Snapshot, limits: &LimitsObservation, now: f64) -> Result<Decision> {
    if let Some(decision) = tick_before_observation(s, now)? {
        return Ok(decision);
    }
    let _r = s
        .record
        .as_ref()
        .ok_or(ContinuationError::Invalid("/record".into()))?;
    let key = s.record_account().unwrap_or("default");
    if limits.account_key() != key {
        return Err(ContinuationError::Unavailable(
            "usage-policy/limits-account-mismatch",
        ));
    }
    let auth = s.record_is_auth();
    let failed = s.record_failed_at().unwrap_or(0.0);
    let fresh = limits.fresh_for(key, now - FRESH_SECONDS, false);
    let recovered = if auth {
        s.usage_record()
            .and_then(|r| r.proof_at.as_ref())
            .is_some_and(|t| t.get() > failed)
            || limits.fresh_for(key, failed, true)
            || (limits.account_key() == key
                && limits.marker.as_ref().is_some_and(|marker| {
                    s.usage_record()
                        .and_then(|r| r.auth_refresh_marker.as_ref())
                        .is_some_and(|old| old.as_str() != marker)
                }))
    } else if fresh {
        limits.allowed(s.agent_record(), now)
    } else {
        reset_passed(s, now)
    };
    let mut r = ProposedRecord::existing(s).expect("validated record");
    if !recovered {
        // Only a current read replaces the reset that the wait holds.
        let reset = fresh
            .then(|| limits.reset_at(s.agent_record()))
            .flatten()
            .or_else(|| s.record_reset_at());
        let due = if auth {
            now + auth_backoff(
                s.usage_record()
                    .and_then(|r| r.auth_attempt.as_ref())
                    .and_then(|n| n.as_u64())
                    .unwrap_or(0),
            )
        } else {
            next_check(reset, now)
        };
        r.update(U::DueAt(Some(due)));
        r.update(U::PlannedAt(reset.filter(|t| *t > now)));
        r.update(U::ResetAt(reset));
        r.update(U::LastCheckedAt(
            limits.response.at.and_then(serde_json::Number::from_f64),
        ));
        r.update(U::WaitingForAuth(auth));
        r.update(U::Reason(
            if auth {
                Some("Waiting for the account sign-in.")
            } else if fresh {
                None
            } else {
                Some("Waiting for a current account limits result.")
            }
            .map(str::to_owned),
        ));
        return Ok(Decision::save(s, r, "usageResume", now));
    }
    Ok(resume(s, r, auth, now))
}

/// Preserve identity checks and reschedule when no current observation is available.
pub fn tick_without_observation(s: &Snapshot, now: f64) -> Result<Decision> {
    if let Some(decision) = tick_before_observation(s, now)? {
        return Ok(decision);
    }
    let mut record =
        ProposedRecord::existing(s).ok_or_else(|| ContinuationError::Invalid("/record".into()))?;
    let auth = s.record_is_auth();
    if !auth && reset_passed(s, now) {
        return Ok(resume(s, record, auth, now));
    }
    let due = if auth {
        now + auth_backoff(
            s.usage_record()
                .and_then(|r| r.auth_attempt.as_ref())
                .and_then(|n| n.as_u64())
                .unwrap_or(0),
        )
    } else {
        next_check(s.record_reset_at(), now)
    };
    record.update(U::DueAt(Some(due)));
    record.update(U::WaitingForAuth(auth));
    record.update(U::Reason(Some(
        "Waiting for a current account limits result.".into(),
    )));
    Ok(Decision::save(s, record, "usageResume", now))
}

fn tick_before_observation(s: &Snapshot, now: f64) -> Result<Option<Decision>> {
    now_checked(now)?;
    let Some(r) = &s.record else {
        return Ok(Some(Decision::default()));
    };
    if !s.current("usageResume")
        || !s.agent_view().usage_resume.as_ref().is_some_and(|r| {
            matches!(
                r.status,
                studio_runtime_schema::continuation_snapshot::UsageContinuationStatus::Scheduled
            )
        })
        || s.status() != Some(crate::continuation::WaitStatus::Scheduled)
        || s.record_due_at().is_none_or(|t| t > now)
    {
        return Ok(Some(Decision::default()));
    }
    let key = s.record_account().unwrap_or("default");
    if excluded(s, key) {
        let reason =
            format!("Automatic resume stopped because account {key} is disabled for this chat.");
        let mut d = cancel(s, r, &reason, now);
        if let Some(a) = &mut d.agent {
            a.failed(&reason);
        }
        d.hold(s.record_id().unwrap_or(""), "account-excluded", &reason);
        return Ok(Some(d));
    }
    if s.agent_record().is_deleted()
        || !s.agent_record().auto_wake.unwrap_or(false)
        || s.agent_record().usage_resume_enabled == Some(false)
        || s.agent_account() != key
        || !s.parents_running(false)
    {
        return Ok(Some(cancel(s, r, STOPPED, now)));
    }
    if !s.record_matches_agent_identity() || !s.agent_view().native_failure_hold.unwrap_or(false) {
        return Ok(Some(cancel(s, r, CHANGED, now)));
    }
    if !s.claims_valid() {
        let mut d = cancel(s, r, TASK_CHANGED, now);
        d.hold(s.record_id().unwrap_or(""), "task-changed", TASK_CHANGED);
        return Ok(Some(d));
    }
    Ok(None)
}
/// Limit telemetry may advance a wait, but never earlier than the anti-loop floor.
pub fn limits_changed(s: &Snapshot, limits: &LimitsObservation, now: f64) -> Result<Decision> {
    now_checked(now)?;
    let Some(_r) = &s.record else {
        return Ok(Decision::default());
    };
    if s.status() != Some(WaitStatus::Scheduled)
        || s.record_is_auth()
        || limits
            .response
            .error
            .as_deref()
            .is_some_and(|error| !error.is_empty())
        || limits.response.data.is_none()
    {
        return Ok(Decision::default());
    }
    if limits.account_key() != s.agent_account() || Some(limits.account_key()) != s.record_account()
    {
        return Err(ContinuationError::Unavailable(
            "usage-policy/limits-account-mismatch",
        ));
    }
    // Telemetry without a blocked window keeps the reset that the wait holds.
    let reset = limits
        .reset_at(s.agent_record())
        .or_else(|| s.record_reset_at());
    let mut r = ProposedRecord::existing(s).expect("validated record");
    r.update(U::ResetAt(reset));
    r.update(U::PlannedAt(reset.filter(|t| *t > now)));
    if limits.allowed(s.agent_record(), now) {
        r.update(U::DueAt(Some(
            now.max(s.record_failed_at().unwrap_or(now) + RESUME_MIN),
        )));
    } else if reset.is_some_and(|t| t > now) {
        r.update(U::DueAt(Some(next_check(reset, now))));
    }
    let mut d = Decision::save(s, r, "usageResume", now);
    if !s.current("usageResume") {
        d.agent = None;
    }
    Ok(d)
}
/// Existing exact cancelled rows can be re-enabled; a changed failed turn needs turn capture.
pub fn enable(s: &Snapshot, now: f64) -> Result<Decision> {
    now_checked(now)?;
    let Some(_r) = &s.record else {
        return Err(ContinuationError::Invalid("/record".into()));
    };
    if !s.current("usageResume") {
        return Ok(Decision::default());
    }
    let mut r = ProposedRecord::existing(s).expect("validated record");
    if s.status() == Some(WaitStatus::Cancelled)
        && s.record_reason() == Some(OFF)
        && s.agent_view().native_failure_hold.unwrap_or(false)
        && s.agent_view().last_completed_turn_status.as_deref() == Some("failed")
        && recoverable_error(&s.agent["error"], s.agent_view().provider.key())?
    {
        if s.agent.get("lastCompletedTurn")
            != s.record.as_ref().and_then(|record| record.get("turnId"))
        {
            return Err(ContinuationError::Unavailable(
                "turn-driver/failed-turn-capture",
            ));
        }
        let due = if s.record_is_auth() {
            now + POLL_SECONDS
        } else {
            next_check(s.record_reset_at(), now)
        };
        r.update(U::Status("scheduled"));
        r.update(U::Reason(None));
        r.update(U::DueAt(Some(due)));
    }
    let mut d = Decision::save(s, r, "usageResume", now);
    if let Some(a) = &mut d.agent {
        a.enable_usage();
    }
    Ok(d)
}

/// Turn completion invokes this with its already-classified, known terminal failure.
/// The caller must query this exact identity before supplying the snapshot.
pub fn failure_identity(s: &Snapshot, turn_id: &str) -> String {
    crate::event_write::request_signature(&json!([
        s.agent_id(),
        s.agent_account(),
        s.agent_record().thread_id,
        s.agent_record().epoch,
        turn_id
    ]))
}
pub struct FailureObservation<'a> {
    pub turn_id: &'a str,
    pub cause: Cause,
    pub error_reset: Option<f64>,
    pub auth_attempt: u64,
    pub limits: &'a LimitsObservation,
}
pub fn failure_eligible(s: &Snapshot) -> bool {
    !crate::continuation::native_thread_blocked(&s.agent)
        && s.agent_record().auto_wake.unwrap_or(false)
        && !s.agent_record().is_deleted()
        && s.agent_record().usage_resume_enabled != Some(false)
        && s.agent_record().thread_id.is_some()
        && !s
            .agent
            .get("turnEpoch")
            .is_some_and(|e| Some(e) != s.agent.get("epoch"))
}
pub fn record_failure(s: &Snapshot, failure: FailureObservation<'_>, now: f64) -> Result<Decision> {
    now_checked(now)?;
    if !failure_eligible(s) {
        return Ok(Decision::default());
    }
    if failure.limits.account_key() != s.agent_account() {
        return Err(ContinuationError::Unavailable(
            "usage-policy/limits-account-mismatch",
        ));
    }
    let id = failure_identity(s, failure.turn_id);
    if s.record_id() == Some(id.as_str()) {
        return Ok(Decision::default());
    }
    let is_auth = failure.cause == Cause::Auth;
    let reset = if is_auth {
        failure.error_reset
    } else {
        failure
            .limits
            .reset_at(s.agent_record())
            .into_iter()
            .chain(failure.error_reset)
            .reduce(f64::max)
    };
    let excluded = excluded(s, s.agent_account());
    let reason = excluded.then(|| {
        format!(
            "Automatic resume stopped because account {} is disabled for this chat.",
            s.agent_account()
        )
    });
    let due_at = if excluded {
        None
    } else {
        Some(if is_auth {
            now + auth_backoff(failure.auth_attempt)
        } else {
            next_check(reset, now)
        })
    };
    let r = ProposedRecord::usage(UsageDraft {
        id: id.clone(),
        status: if excluded { "cancelled" } else { "scheduled" },
        account_key: s.agent_account().into(),
        thread_id: s.agent_record().thread_id.clone(),
        epoch: s.agent_record().epoch,
        turn_id: failure.turn_id.into(),
        cause: failure.cause,
        failed_at: now,
        auth_attempt: if is_auth { failure.auth_attempt } else { 0 },
        auth_refresh_marker: if is_auth && !excluded {
            failure.limits.marker.clone()
        } else {
            None
        },
        due_at,
        planned_at: if excluded {
            None
        } else {
            reset.filter(|t| *t > now)
        },
        reset_at: if excluded { None } else { reset },
        reason: reason.clone(),
        task_claims: s.claims.clone(),
        updated_at: now,
    });
    let mut d = Decision::save(s, r, "usageResume", now);
    if let Some(a) = &mut d.agent {
        if is_auth {
            a.auth_attempt(failure.auth_attempt.saturating_add(1));
        }
        if let Some(reason) = &reason {
            a.failed(reason);
        }
    }
    if let Some(reason) = reason {
        d.hold(&id, "account-excluded", &reason);
    }
    Ok(d)
}
/// One successful turn is proof only for older auth waits on the same account.
pub fn completed_proof(s: &Snapshot, account_key: &str, now: f64) -> Result<Decision> {
    now_checked(now)?;
    let proof = s.record.as_ref().filter(|_| {
        s.record_account() == Some(account_key)
            && s.record_is_auth()
            && s.status() == Some(WaitStatus::Scheduled)
            && s.usage_record()
                .and_then(|r| r.proof_at.as_ref())
                .is_none_or(|t| t.get() == 0.0)
            && s.record_failed_at().is_some_and(|t| t < now)
    });
    let reset_attempt = s.current("usageResume")
        && s.agent_view().usage_resume.as_ref().is_some_and(|r| {
            matches!(
                r.cause,
                Some(studio_runtime_schema::continuation_snapshot::UsageContinuationCause::Auth)
            ) && matches!(
                r.status,
                studio_runtime_schema::continuation_snapshot::UsageContinuationStatus::Started
            )
        });
    if proof.is_none() && !reset_attempt {
        return Ok(Decision::default());
    }
    let mut decision = Decision::default();
    if proof.is_some() {
        let mut record = ProposedRecord::existing(s).expect("validated record");
        record.update(U::ProofAt(now));
        record.update(U::DueAt(Some(now)));
        if s.current("usageResume") {
            let mut agent = crate::continuation::ProposedAgent::new(s);
            agent.continuation("usageResume", record.value());
            decision.agent = Some(agent);
        }
        decision.record = Some(record);
    }
    if reset_attempt {
        let agent = decision
            .agent
            .get_or_insert_with(|| crate::continuation::ProposedAgent::new(s));
        agent.auth_attempt(0);
    }
    Ok(decision)
}

fn recoverable_error(error: &Value, provider: &str) -> Result<bool> {
    if matches!(
        known_error_kind(error),
        Some(
            KnownCodexErrorKind::UsageLimitExceeded
                | KnownCodexErrorKind::RateLimitExceeded
                | KnownCodexErrorKind::Unauthorized
        )
    ) || raw_error_kind(error).is_some_and(|kind| kind.to_ascii_lowercase().contains("auth"))
    {
        return Ok(true);
    }
    static VALIDATOR: std::sync::OnceLock<std::result::Result<jsonschema::Validator, String>> =
        std::sync::OnceLock::new();
    let validator=VALIDATOR.get_or_init(||jsonschema::validator_for(&json!({"type":"object","anyOf":[
      {"properties":{"message":{"pattern":r"(?i)\b401\b"}},"required":["message"]},
      {"properties":{"message":{"allOf":[{"pattern":r"(?i)\b403\b"},{"pattern":r"(?i)\b(?:invalid|expired|revoked).{0,32}\b(?:token|key)\b|\b(?:token|key)\b.{0,32}\b(?:invalid|expired|revoked)\b|\bsign[ -]?in\b|\bauthentication required\b|\bunauthorized\b"}]}},"required":["message"]},
      {"properties":{"provider":{"const":ProviderKind::Claude.key()},"message":{"pattern":"(?i)oauth session expired|failed to authenticate"}},"required":["provider","message"]}
    ]})).map_err(|e|e.to_string())).as_ref().map_err(|_|ContinuationError::Invalid("/error/schema".into()))?;
    Ok(validator.is_valid(&json!({"message":text(error,"message"),"provider":provider})))
}
