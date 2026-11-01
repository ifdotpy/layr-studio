//! Typed ingress for the workspace usage-limits schema. Provider extensions remain opaque.
use crate::StoredJsonInteger;
use crate::json_object::JsonObject;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageLimitsResponse {
    pub account_key: String,
    pub at: Option<f64>,
    pub checked_at: Option<f64>,
    pub data: Option<JsonObject<UsageLimitsData>>,
    pub error: Option<String>,
    pub processed_at: Option<f64>,
    pub read_at: Option<f64>,
    #[serde(default, deserialize_with = "present_eligibility")]
    pub eligibility: Option<JsonObject<UsageEligibility>>,
    #[serde(rename = "_syncEntities")]
    pub sync_entities: Option<Vec<JsonObject<SyncEntity>>>,
    #[serde(rename = "_syncEntitiesAfter")]
    pub sync_entities_after: Option<StoredJsonInteger>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitsData {
    pub account_id: Option<String>,
    pub ordinary_usage_allowed: Option<bool>,
    pub rate_limit_reset_credits: Option<JsonObject<RateLimitResetCredits>>,
    pub rate_limits: Option<JsonObject<RateLimitBucket>>,
    pub rate_limits_by_limit_id: Option<BTreeMap<String, JsonObject<RateLimitBucket>>>,
    pub signed_in: Option<bool>,
    pub source: Option<String>,
    pub status: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitBucket {
    pub individual_limit: Option<JsonObject<RateLimitIndividualLimit>>,
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub primary: Option<JsonObject<RateLimitWindow>>,
    pub rate_limit_reached_type: Option<String>,
    pub secondary: Option<JsonObject<RateLimitWindow>>,
    pub spend_control_reached: Option<bool>,
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub credits: Option<BucketCredits>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketCredits {
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub unlimited: Option<bool>,
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub has_credits: Option<bool>,
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub balance: Option<BucketCreditBalance>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum BucketCreditBalance {
    Text(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RateLimitIndividualLimit {
    pub remaining_percent: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub resets_at: Option<f64>,
    pub used_percent: Option<f64>,
    pub window_duration_mins: Option<f64>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCredits {
    pub available_count: Option<StoredJsonInteger>,
    #[serde(default)]
    pub credits: Vec<JsonObject<RateLimitResetCredit>>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCredit {
    pub id: String,
    pub reset_type: String,
    pub status: String,
    pub expires_at: Option<f64>,
    pub granted_at: Option<f64>,
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "studio_client_domain::json_object::optional_field"
    )]
    pub description: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncEntity {
    pub id: String,
    pub seq: StoredJsonInteger,
    pub payload: String,
    #[serde(rename = "_deleted")]
    pub deleted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageEligibility {
    pub account_key: String,
    pub agent_id: String,
    pub allowed: bool,
    pub buckets: Vec<JsonObject<EligibilityBucket>>,
    pub can_run: bool,
    pub checked_at: f64,
    pub fresh: bool,
    pub model: String,
    pub overage: Overage,
    pub provider: String,
    #[serde(deserialize_with = "required_nullable")]
    pub reset_at: Option<f64>,
    #[serde(deserialize_with = "required_nullable")]
    pub valid_until: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EligibilityBucket {
    pub id: String,
    pub overage_allowed: bool,
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
pub enum Overage {
    Allowed,
    InUse,
    Rejected,
    Unavailable,
}
impl TryFrom<String> for Overage {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "allowed" => Ok(Self::Allowed),
            "in_use" => Ok(Self::InUse),
            "rejected" => Ok(Self::Rejected),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid overage"),
        }
    }
}

/// The retained observation and its server-owned presentation travel together.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitView<'a> {
    pub observation: &'a serde_json::value::RawValue,
    pub projection: AccountLimitsProjection,
}

impl<'a> RateLimitView<'a> {
    pub fn read(
        observation: &'a serde_json::value::RawValue,
        account: &str,
        now: f64,
    ) -> Result<Self, serde_json::Error> {
        let JsonObject(value): JsonObject<UsageLimitsResponse> =
            serde_json::from_str(observation.get())?;
        if value.account_key != account {
            return Err(serde::de::Error::custom(
                "rate-limit account identity mismatch",
            ));
        }
        Ok(Self {
            observation,
            projection: AccountLimitsProjection {
                runway: weekly_runway(&value, now),
                reset: reset_projection(&value, now),
                buckets: bucket_projection(&value, now),
            },
        })
    }
}

/// Server-owned account presentation; each field is derived from the same observation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountLimitsProjection {
    pub runway: WeeklyRunway,
    pub reset: ResetProjection,
    pub buckets: BucketProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetProjection {
    pub present: bool,
    pub account_id: Option<String>,
    pub available_count: Option<f64>,
    pub count_label: String,
    pub empty_message: Option<String>,
    pub credits: Vec<ResetCreditView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetCreditView {
    pub id: String,
    pub title: String,
    pub confirm_label: String,
    pub description: Option<String>,
    pub expires_at: Option<f64>,
    pub enabled: bool,
    pub valid_until: Option<f64>,
    pub enabled_after: bool,
}

/// Shared provider-credit admission predicate; the consuming service rechecks it.
pub fn reset_credit_eligible(credit: &RateLimitResetCredit, now: f64) -> bool {
    !credit.id.is_empty()
        && credit.status == "available"
        && credit.reset_type == "codexRateLimits"
        && credit.expires_at.is_none_or(|expires| expires > now)
}

pub fn reset_projection(observation: &UsageLimitsResponse, now: f64) -> ResetProjection {
    let data = observation.data.as_ref().map(|data| &data.0);
    let account_id = data
        .and_then(|data| data.account_id.as_ref())
        .filter(|id| !id.is_empty())
        .cloned();
    let summary = data
        .and_then(|data| data.rate_limit_reset_credits.as_ref())
        .map(|summary| &summary.0);
    let available_count = summary
        .and_then(|summary| summary.available_count.as_ref())
        .and_then(|count| count.number().as_f64());
    let credits: Vec<ResetCreditView> = summary
        .into_iter()
        .flat_map(|summary| &summary.credits)
        .map(|credit| &credit.0)
        .filter(|credit| {
            credit.status == "available" && credit.expires_at.is_none_or(|expires| expires > now)
        })
        .map(|credit| {
            let title = credit.title.as_deref().filter(|title| !title.is_empty());
            ResetCreditView {
                id: credit.id.clone(),
                title: title.unwrap_or("Reset credit").to_owned(),
                confirm_label: format!("Confirm {}", title.unwrap_or("limit reset")),
                description: credit.description.clone(),
                expires_at: credit.expires_at,
                enabled: account_id.is_some() && reset_credit_eligible(credit, now),
                valid_until: credit.expires_at,
                enabled_after: false,
            }
        })
        .collect();
    let count_label = available_count.map_or_else(
        || "Count unavailable".to_owned(),
        |count| format!("{count} available"),
    );
    let empty_message = credits.is_empty().then(|| {
        if available_count == Some(0.0) {
            "No reset credits available."
        } else {
            "Reset credit details unavailable. Refresh to check."
        }
        .to_owned()
    });
    ResetProjection {
        present: summary.is_some(),
        account_id,
        available_count,
        count_label,
        empty_message,
        credits,
    }
}

/// Account bucket colors and thresholds are selected by the server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LimitColor {
    Paused,
    Unread,
    Answer,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BucketWindowView {
    pub label: String,
    pub remaining: Option<f64>,
    pub reset: Option<f64>,
    pub expired: bool,
    pub color: LimitColor,
    pub warning: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BucketView {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub plan_type: Option<String>,
    pub credits_label: Option<String>,
    pub windows: Vec<BucketWindowView>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BucketState {
    pub buckets: Vec<BucketView>,
    pub remaining: Option<f64>,
    pub color: LimitColor,
    pub low: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BucketProjection {
    pub current: BucketState,
    pub valid_until: Option<f64>,
    pub next: BucketState,
}

fn remaining_color(remaining: Option<f64>) -> LimitColor {
    // Percent thresholds for meter color; independent of the low-warning threshold.
    const COMFORTABLE_PERCENT: f64 = 50.0;
    const ANSWER_PERCENT: f64 = 15.0;
    match remaining {
        None => LimitColor::Paused,
        Some(value) if value > COMFORTABLE_PERCENT => LimitColor::Unread,
        Some(value) if value >= ANSWER_PERCENT => LimitColor::Answer,
        Some(_) => LimitColor::Error,
    }
}
fn percent(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
}
fn duration_label(minutes: Option<f64>, fallback: &str) -> String {
    let Some(minutes) = minutes.filter(|minutes| minutes.is_finite() && *minutes > 0.0) else {
        return fallback.to_owned();
    };
    if minutes % 1440.0 == 0.0 {
        format!("{}d", minutes / 1440.0)
    } else if minutes % 60.0 == 0.0 {
        format!("{}h", minutes / 60.0)
    } else {
        format!("{minutes}m")
    }
}
fn credits_label(credits: &BucketCredits) -> String {
    if credits.unlimited == Some(true) {
        return "Unlimited".to_owned();
    }
    if let Some(balance) = &credits.balance {
        return match balance {
            BucketCreditBalance::Text(value) => value.clone(),
            BucketCreditBalance::Number(value) => value.to_string(),
            BucketCreditBalance::Boolean(value) => value.to_string(),
        };
    }
    match credits.has_credits {
        Some(true) => "Available",
        Some(false) => "None",
        None => "Unavailable",
    }
    .to_owned()
}

pub fn bucket_projection(observation: &UsageLimitsResponse, now: f64) -> BucketProjection {
    const LOW_WARNING_PERCENT: f64 = 15.0;
    let data = observation.data.as_ref().map(|data| &data.0);
    let mut rows = Vec::new();
    if let Some(data) = data {
        let mut add = |id: &str, bucket: &RateLimitBucket| {
            let provider = id.parse::<crate::provider_kind::ProviderKind>().ok();
            let known = provider.is_some()
                || bucket
                    .limit_id
                    .as_deref()
                    .is_some_and(|id| id.parse::<crate::provider_kind::ProviderKind>().is_ok());
            let name = bucket
                .limit_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    provider.map_or_else(|| id.to_owned(), |kind| kind.label().to_owned())
                });
            let spark = name.to_ascii_lowercase().contains("spark");
            let mut windows = Vec::new();
            for (value, fallback) in [
                (bucket.primary.as_ref(), "Primary"),
                (bucket.secondary.as_ref(), "Secondary"),
            ] {
                let Some(value) = value else {
                    continue;
                };
                let value = &value.0;
                let reset = value
                    .resets_at
                    .filter(|value| value.is_finite() && *value > 0.0);
                let expired = reset.is_some_and(|reset| reset <= now);
                let remaining = percent(value.used_percent).map(|used| 100.0 - used);
                windows.push(BucketWindowView {
                    label: duration_label(value.window_duration_mins, fallback),
                    remaining,
                    reset,
                    expired,
                    color: remaining_color(remaining),
                    warning: !expired
                        && remaining.is_some_and(|value| value <= LOW_WARNING_PERCENT),
                });
            }
            if let Some(individual) = &bucket.individual_limit {
                let remaining = percent(individual.0.remaining_percent);
                windows.push(BucketWindowView {
                    label: "Individual".to_owned(),
                    remaining,
                    reset: None,
                    expired: false,
                    color: remaining_color(remaining),
                    warning: remaining.is_some_and(|value| value <= LOW_WARNING_PERCENT),
                });
            }
            let rank = if known {
                0
            } else if spark {
                2
            } else {
                1
            };
            let short_name = if spark {
                "Spark".to_owned()
            } else {
                name.clone()
            };
            rows.push((
                rank,
                BucketView {
                    id: id.to_owned(),
                    name,
                    short_name,
                    plan_type: bucket.plan_type.clone(),
                    credits_label: bucket.credits.as_ref().map(credits_label),
                    windows,
                },
            ));
        };
        if let Some(buckets) = data
            .rate_limits_by_limit_id
            .as_ref()
            .filter(|buckets| !buckets.is_empty())
        {
            for (id, bucket) in buckets {
                add(id, &bucket.0);
            }
        } else if let Some(bucket) = &data.rate_limits {
            add(
                bucket
                    .0
                    .limit_id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .unwrap_or("codex"),
                &bucket.0,
            );
        }
    }
    rows.sort_by(|(rank_a, a), (rank_b, b)| {
        rank_a
            .cmp(rank_b)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    let buckets: Vec<_> = rows.into_iter().map(|(_, bucket)| bucket).collect();
    let windows = buckets.iter().flat_map(|bucket| &bucket.windows);
    let expired = windows.clone().any(|window| window.expired);
    let valid_until = windows
        .clone()
        .filter_map(|window| window.reset.filter(|reset| *reset > now))
        .reduce(f64::min);
    let remaining = if expired {
        None
    } else {
        windows
            .clone()
            .filter_map(|window| window.remaining)
            .reduce(f64::min)
    };
    let low = !expired && windows.clone().any(|window| window.warning);
    let mut next_buckets = buckets.clone();
    for window in next_buckets
        .iter_mut()
        .flat_map(|bucket| &mut bucket.windows)
    {
        window.expired = window
            .reset
            .zip(valid_until)
            .is_some_and(|(reset, deadline)| reset <= deadline);
        window.remaining = None;
        window.color = LimitColor::Paused;
        window.warning = false;
    }
    BucketProjection {
        current: BucketState {
            buckets,
            remaining,
            color: remaining_color(remaining),
            low,
        },
        valid_until,
        next: BucketState {
            buckets: next_buckets,
            remaining: None,
            color: LimitColor::Paused,
            low: false,
        },
    }
}

/// Server-owned weekly allowance estimate, computed at `computed_at` (Unix seconds).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeeklyRunway {
    pub computed_at: f64,
    pub valid_until: Option<f64>,
    pub current: WeeklyRunwayState,
    pub next: WeeklyRunwayState,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeeklyRunwayState {
    pub color: RunwayColor,
    pub days: Option<f64>,
    pub unlimited: bool,
    pub windows: Vec<WeeklyRunwayWindow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RunwayColor {
    Green,
    Yellow,
    Red,
    Gray,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeeklyRunwayWindow {
    pub name: String,
    pub remaining: Option<f64>,
    pub reset: Option<f64>,
    pub days: Option<f64>,
    pub unlimited: bool,
    pub reset_first: bool,
}

/// Does not treat a reset deadline as evidence that a provider restored allowance.
pub fn weekly_runway(observation: &UsageLimitsResponse, now: f64) -> WeeklyRunway {
    const DAY: f64 = 86_400.0;
    const WEEK_MINUTES: f64 = 10_080.0;
    let mut windows = Vec::new();
    if let Some(data) = observation.data.as_ref().map(|data| &data.0) {
        let mut add = |id: &str, bucket: &RateLimitBucket| {
            for window in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
            {
                let window = &window.0;
                if window.window_duration_mins != Some(WEEK_MINUTES) {
                    continue;
                }
                let reset = window
                    .resets_at
                    .filter(|value| value.is_finite() && *value > 0.0);
                let expired = reset.is_some_and(|reset| reset <= now);
                let used = window
                    .used_percent
                    .filter(|value| value.is_finite() && (0.0..=100.0).contains(value));
                let remaining = if expired {
                    None
                } else {
                    used.map(|used| 100.0 - used)
                };
                let elapsed = observation
                    .at
                    .zip(reset)
                    .map(|(at, reset)| (at - (reset - WEEK_MINUTES * 60.0)) / DAY);
                let valid = !expired
                    && observation
                        .at
                        .is_some_and(|at| at.is_finite() && at <= now + 5.0)
                    && elapsed.is_some_and(|elapsed| elapsed > 0.0 && elapsed <= 7.0);
                let unlimited = !expired && used == Some(0.0);
                let days = if !expired && used == Some(100.0) {
                    Some(0.0)
                } else if valid {
                    used.zip(elapsed)
                        .and_then(|(used, elapsed)| {
                            (used > 0.0).then_some((100.0 - used) / (used / elapsed))
                        })
                        .filter(|days| days.is_finite())
                } else {
                    None
                };
                let reset_first = reset.is_some_and(|reset| {
                    unlimited || days.is_some_and(|days| (reset - now) / DAY < days)
                });
                let name = bucket
                    .limit_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(match id {
                        "codex" => "Codex",
                        "claude" => "Claude",
                        other => other,
                    });
                windows.push(WeeklyRunwayWindow {
                    name: name.to_owned(),
                    remaining,
                    reset,
                    days,
                    unlimited,
                    reset_first,
                });
            }
        };
        if let Some(buckets) = data
            .rate_limits_by_limit_id
            .as_ref()
            .filter(|buckets| !buckets.is_empty())
        {
            for (id, bucket) in buckets {
                add(id, &bucket.0);
            }
        } else if let Some(bucket) = &data.rate_limits {
            add(bucket.0.limit_id.as_deref().unwrap_or("codex"), &bucket.0);
        }
    }
    let valid_until = windows
        .iter()
        .filter_map(|window| window.reset.filter(|reset| *reset > now))
        .reduce(f64::min);
    let unavailable =
        observation.data.is_none() || windows.iter().any(|window| window.remaining.is_none());
    let exhausted = windows.iter().any(|window| window.remaining == Some(0.0));
    let unknown_days = windows
        .iter()
        .any(|window| window.days.is_none() && !window.unlimited);
    let days = windows
        .iter()
        .filter_map(|window| {
            window.days.map(|days| {
                if window.reset_first {
                    days.max(7.0)
                } else {
                    days
                }
            })
        })
        .reduce(f64::min);
    let unlimited = !unavailable && !unknown_days && days.is_none();
    let color = if unavailable {
        RunwayColor::Gray
    } else if exhausted || unknown_days {
        RunwayColor::Red
    } else if unlimited || days.is_some_and(|days| days >= 7.0) {
        RunwayColor::Green
    } else if days.is_some_and(|days| days >= 5.0) {
        RunwayColor::Yellow
    } else {
        RunwayColor::Red
    };
    let next = WeeklyRunwayState {
        color: RunwayColor::Gray,
        days: None,
        unlimited: false,
        windows: windows
            .iter()
            .map(|window| WeeklyRunwayWindow {
                name: window.name.clone(),
                remaining: None,
                reset: window.reset,
                days: None,
                unlimited: false,
                reset_first: false,
            })
            .collect(),
    };
    WeeklyRunway {
        computed_at: now,
        valid_until,
        current: WeeklyRunwayState {
            color,
            days,
            unlimited,
            windows,
        },
        next,
    }
}

fn required_nullable<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Option::deserialize(deserializer)
}
fn present_eligibility<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<JsonObject<UsageEligibility>>, D::Error> {
    JsonObject::<UsageEligibility>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bucket_color_boundaries_are_server_owned() {
        assert!(matches!(remaining_color(None), LimitColor::Paused));
        assert!(matches!(remaining_color(Some(0.0)), LimitColor::Error));
        assert!(matches!(remaining_color(Some(14.99)), LimitColor::Error));
        assert!(matches!(remaining_color(Some(15.0)), LimitColor::Answer));
        assert!(matches!(remaining_color(Some(50.0)), LimitColor::Answer));
        assert!(matches!(remaining_color(Some(50.01)), LimitColor::Unread));
    }

    #[test]
    fn bucket_projection_orders_rows_and_expires_the_aggregate() {
        let observation: UsageLimitsResponse = serde_json::from_str(
            r#"{
          "accountKey":"a", "at":100,
          "data":{"rateLimitsByLimitId":{
            "spark":{"limitName":"Spark","primary":{"usedPercent":90,"resetsAt":120}},
            "extra":{"limitName":"Other","primary":{"usedPercent":25,"resetsAt":130}},
            "codex":{"limitName":"Codex","primary":{"usedPercent":50,"resetsAt":110}}
          }}
        }"#,
        )
        .unwrap();
        let projection = bucket_projection(&observation, 100.0);
        assert_eq!(
            projection
                .current
                .buckets
                .iter()
                .map(|bucket| bucket.id.as_str())
                .collect::<Vec<_>>(),
            ["codex", "extra", "spark"]
        );
        assert_eq!(projection.current.remaining, Some(10.0));
        assert!(projection.current.low);
        assert_eq!(projection.valid_until, Some(110.0));
        assert_eq!(projection.next.remaining, None);
    }

    #[test]
    fn reset_projection_preserves_credit_identity_and_disables_unspendable_credits() {
        let observation: UsageLimitsResponse = serde_json::from_str(r#"{
            "accountKey":"a", "data":{"accountId":"native-a", "rateLimitResetCredits":{"availableCount":2,"credits":[
              {"id":"spendable","resetType":"codexRateLimits","status":"available","expiresAt":110},
              {"id":"other","resetType":"other","status":"available"},
              {"id":"expired","resetType":"codexRateLimits","status":"available","expiresAt":90},
              {"id":"used","resetType":"codexRateLimits","status":"used"}
            ]}}
        }"#).unwrap();
        let view = reset_projection(&observation, 100.0);
        assert_eq!(view.account_id.as_deref(), Some("native-a"));
        assert_eq!(view.credits.len(), 2);
        assert_eq!(view.credits[0].id, "spendable");
        assert!(view.credits[0].enabled);
        assert_eq!(view.credits[0].valid_until, Some(110.0));
        assert!(!view.credits[1].enabled);
    }

    #[test]
    fn runway_thresholds_and_unknown_windows_are_server_owned() {
        fn view(used: Option<f64>, elapsed: f64) -> WeeklyRunway {
            let now = 1_800_000_000.0;
            let reset = now + (7.0 - elapsed) * 86_400.0;
            let input = format!(
                r#"{{"accountKey":"a","at":{now},"data":{{"rateLimits":{{"secondary":{{"usedPercent":{},"resetsAt":{reset},"windowDurationMins":10080}}}}}}}}"#,
                used.map_or_else(|| "null".to_owned(), |used| used.to_string())
            );
            let observation: UsageLimitsResponse = serde_json::from_str(&input).unwrap();
            weekly_runway(&observation, now)
        }
        assert_eq!(view(Some(10.0), 1.0).current.color, RunwayColor::Green);
        assert_eq!(view(Some(20.0), 1.25).current.color, RunwayColor::Yellow);
        assert_eq!(view(Some(20.0), 1.249).current.color, RunwayColor::Red);
        assert_eq!(view(None, 1.0).current.color, RunwayColor::Gray);
        assert_eq!(view(Some(100.0), 1.0).current.days, Some(0.0));
        assert!(view(Some(0.0), 1.0).current.unlimited);
    }

    #[test]
    fn runway_orders_buckets_and_preserves_unknowns_without_guessing_resets() {
        let observation: UsageLimitsResponse = serde_json::from_str(r#"{
          "accountKey":"a", "at":100,
          "data":{"rateLimitsByLimitId":{
            "z":{"limitName":"Zed","primary":{"usedPercent":80,"resetsAt":90,"windowDurationMins":10080}},
            "a":{"limitName":"Alpha","secondary":{"usedPercent":50,"resetsAt":200,"windowDurationMins":10080}}
          }}
        }"#).unwrap();
        let view = weekly_runway(&observation, 100.0);
        assert_eq!(
            view.current
                .windows
                .iter()
                .map(|window| window.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Zed"]
        );
        assert_eq!(view.current.windows[0].remaining, Some(50.0));
        assert_eq!(view.current.windows[1].remaining, None);
        assert_eq!(view.current.color, RunwayColor::Gray);
        assert_eq!(view.valid_until, Some(200.0));
        assert!(
            view.next
                .windows
                .iter()
                .all(|window| window.remaining.is_none())
        );
    }

    #[test]
    fn exhausted_window_is_zero_even_without_a_reset_time() {
        let observation: UsageLimitsResponse = serde_json::from_str(r#"{"accountKey":"a","data":{"rateLimits":{"primary":{"usedPercent":100,"windowDurationMins":10080}}}}"#).unwrap();
        let view = weekly_runway(&observation, 100.0);
        assert_eq!(view.current.color, RunwayColor::Red);
        assert_eq!(view.current.days, Some(0.0));
        assert_eq!(view.valid_until, None);
    }

    #[test]
    fn rate_limit_view_rejects_another_account_and_preserves_observation() {
        let raw = serde_json::value::RawValue::from_string(
            r#"{"accountKey":"a","data":{"vendor":{"unknown":[1,true]}}}"#.to_owned(),
        )
        .unwrap();
        assert!(RateLimitView::read(&raw, "other", 100.0).is_err());
        let view = RateLimitView::read(&raw, "a", 100.0).unwrap();
        assert_eq!(view.observation.get(), raw.get());
    }

    #[test]
    fn runway_keeps_passed_resets_unknown_until_a_new_observation() {
        let observation: UsageLimitsResponse = serde_json::from_str(
            r#"{"accountKey":"a","at":100,"data":{"rateLimits":{"primary":{"usedPercent":80,"resetsAt":110,"windowDurationMins":10080}}}}"#,
        ).unwrap();
        let view = weekly_runway(&observation, 100.0);
        assert_eq!(view.current.windows[0].remaining, Some(20.0));
        assert_eq!(view.valid_until, Some(110.0));
        assert_eq!(view.next.color, RunwayColor::Gray);
    }

    #[test]
    fn known_fields_track_schema_owner() {
        let schema: Value =
            serde_json::from_str(include_str!("schemas/workspace-usage-limits.schema.json"))
                .expect("schema");
        for (name, fields) in [
            (
                "UsageLimitsResponse",
                "_syncEntities _syncEntitiesAfter accountKey at checkedAt data eligibility error processedAt readAt",
            ),
            (
                "UsageLimitsData",
                "accountId ordinaryUsageAllowed rateLimitResetCredits rateLimits rateLimitsByLimitId signedIn source status",
            ),
            (
                "RateLimitBucket",
                "individualLimit limitId limitName planType primary rateLimitReachedType secondary spendControlReached",
            ),
            ("RateLimitIndividualLimit", "remainingPercent"),
            ("RateLimitWindow", "resetsAt usedPercent windowDurationMins"),
            ("RateLimitResetCredits", "availableCount credits"),
            (
                "RateLimitResetCredit",
                "expiresAt grantedAt id resetType status",
            ),
            ("SyncEntity", "_deleted id payload seq"),
            (
                "UsageEligibility",
                "accountKey agentId allowed buckets canRun checkedAt fresh model overage provider resetAt validUntil",
            ),
        ] {
            let actual = schema["$defs"][name]["properties"]
                .as_object()
                .expect("properties");
            let expected = fields
                .split_whitespace()
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                actual
                    .keys()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn typed_ingress_matches_usage_limits_schema() {
        let full = json!({"accountKey":"a", "at":1, "checkedAt":2, "processedAt":3, "readAt":4,
            "error":null, "data":{"accountId":null,"ordinaryUsageAllowed":true,"signedIn":true,"source":"provider","status":"ok",
                "rateLimits":{"limitId":"one","limitName":"One","planType":"plus","rateLimitReachedType":null,"spendControlReached":false,
                    "individualLimit":{"remainingPercent":5},"primary":{"usedPercent":2,"resetsAt":3,"windowDurationMins":4,"vendor":[true,null]},"secondary":null},
                "rateLimitsByLimitId":{"one":{"primary":{"usedPercent":2}}},
                "rateLimitResetCredits":{"availableCount":2,"credits":[{"id":"x","status":"available","resetType":"manual","grantedAt":2,"expiresAt":3,"vendor":{}}]},"extension":{"arbitrary":[null,true]}},
            "_syncEntities":[{"id":"x","seq":1,"payload":"{}","_deleted":false}],"_syncEntitiesAfter":1,
            "eligibility":{"accountKey":"a","agentId":"b","allowed":true,"buckets":[{"id":"x","overageAllowed":false}],"canRun":true,"checkedAt":1,"fresh":true,"model":"m","overage":"allowed","provider":"codex","resetAt":null,"validUntil":null}});
        let cases = [
            ("/data/rateLimits/primary/usedPercent", json!("2")),
            ("/data/rateLimits/primary", json!([2, 3, 4])),
            ("/data/rateLimits/individualLimit", json!({"extra":1})),
            ("/data/rateLimitsByLimitId/one", json!([])),
            ("/data/rateLimitResetCredits/availableCount", json!(1.5)),
            ("/data/rateLimitResetCredits/credits/0", json!({"id":"x"})),
            ("/_syncEntities/0/seq", json!(-1)),
            ("/eligibility/overage", json!({"allowed":null})),
            ("/eligibility", json!(null)),
            ("/data", json!([])),
            ("/at", json!("1")),
        ];
        let mut candidates = vec![
            full.clone(),
            json!({"accountKey":"a"}),
            json!({"accountKey":"a","data":null}),
            json!([]),
        ];
        for (pointer, value) in cases {
            let mut candidate = full.clone();
            *candidate.pointer_mut(pointer).expect("fixture path") = value;
            candidates.push(candidate);
        }
        for pointer in [
            "/accountKey",
            "/at",
            "/checkedAt",
            "/processedAt",
            "/readAt",
            "/error",
            "/data",
            "/_syncEntities",
            "/_syncEntitiesAfter",
            "/eligibility",
            "/data/accountId",
            "/data/ordinaryUsageAllowed",
            "/data/signedIn",
            "/data/source",
            "/data/status",
            "/data/rateLimits",
            "/data/rateLimitsByLimitId",
            "/data/rateLimitResetCredits",
            "/data/rateLimits/limitId",
            "/data/rateLimits/limitName",
            "/data/rateLimits/planType",
            "/data/rateLimits/rateLimitReachedType",
            "/data/rateLimits/spendControlReached",
            "/data/rateLimits/individualLimit",
            "/data/rateLimits/individualLimit/remainingPercent",
            "/data/rateLimits/primary",
            "/data/rateLimits/secondary",
            "/data/rateLimits/primary/resetsAt",
            "/data/rateLimits/primary/usedPercent",
            "/data/rateLimits/primary/windowDurationMins",
            "/data/rateLimitResetCredits/availableCount",
            "/data/rateLimitResetCredits/credits",
            "/data/rateLimitResetCredits/credits/0/id",
            "/data/rateLimitResetCredits/credits/0/status",
            "/data/rateLimitResetCredits/credits/0/resetType",
            "/data/rateLimitResetCredits/credits/0/expiresAt",
            "/data/rateLimitResetCredits/credits/0/grantedAt",
            "/_syncEntities/0/id",
            "/_syncEntities/0/seq",
            "/_syncEntities/0/payload",
            "/_syncEntities/0/_deleted",
            "/eligibility/accountKey",
            "/eligibility/agentId",
            "/eligibility/allowed",
            "/eligibility/buckets",
            "/eligibility/buckets/0/id",
            "/eligibility/buckets/0/overageAllowed",
            "/eligibility/canRun",
            "/eligibility/checkedAt",
            "/eligibility/fresh",
            "/eligibility/model",
            "/eligibility/overage",
            "/eligibility/provider",
            "/eligibility/resetAt",
            "/eligibility/validUntil",
        ] {
            for value in [
                json!(null),
                json!(true),
                json!(-1),
                json!(1.0),
                json!(1.5),
                json!("x"),
                json!([]),
                json!({}),
            ] {
                let mut candidate = full.clone();
                *candidate.pointer_mut(pointer).expect("fixture path") = value;
                candidates.push(candidate);
            }
        }
        for candidate in candidates {
            let schema_valid =
                crate::first_error("workspace-usage-limits/UsageLimitsResponse", &candidate)
                    .expect("schema exists")
                    .is_none();
            let typed_valid =
                serde_json::from_value::<JsonObject<UsageLimitsResponse>>(candidate.clone())
                    .is_ok();
            assert_eq!(typed_valid, schema_valid, "{candidate}");
        }
    }
}
