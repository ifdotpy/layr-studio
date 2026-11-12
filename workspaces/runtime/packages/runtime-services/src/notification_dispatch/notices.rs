//! Pure native notice decisions. The adapter supplies checked stored records and time.
use serde::Deserialize;
use studio_json::{Value, json};
use studio_runtime_schema::provider_errors::{KnownCodexErrorKind, known_error_kind};
use studio_runtime_schema::{agent_entity_nested::NativeStatusPhase, queue_command::Field};
use studio_runtime_store::CachedSql;
use studio_runtime_store::sha256_hex;
use studio_runtime_store::{
    AgentActivity, AgentRecord, NativeSafetyBuffering, NativeStatus, NativeThreadBlock,
    NativeTurnError, StoredTransferObject, StoredTransferValue,
};

use super::{NotificationDispatchError, format_utc};

const ACCOUNT_NOTICE_LIMIT: i64 = 50;

pub(super) use studio_runtime_providers::codex_disposition::NoticeHandler as NoticeKind;
pub(super) fn is_account_notice(kind: NoticeKind) -> bool {
    matches!(
        kind,
        NoticeKind::Warning
            | NoticeKind::Guardian
            | NoticeKind::Config
            | NoticeKind::Deprecation
            | NoticeKind::McpStartup
            | NoticeKind::McpLogin
            | NoticeKind::StrictReview
            | NoticeKind::AuthStarted
            | NoticeKind::AuthCompleted
    )
}

#[derive(Clone, Debug)]
pub(super) struct NoticeItem {
    pub key: String,
    pub text: String,
    pub metadata: Value,
}
#[derive(Default)]
pub(super) struct NoticeDecision {
    pub agent: Option<AgentRecord>,
    pub item: Option<NoticeItem>,
    pub refresh_limits: bool,
}

fn nonempty<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}
fn notice(key: String, text: String, severity: &str, mut metadata: Value) -> NoticeItem {
    metadata["nativeNotice"] = json!(severity);
    NoticeItem {
        key: format!("native-notice:{key}"),
        text,
        metadata,
    }
}

pub(super) fn warning_text(kind: NoticeKind, params: &Value) -> String {
    let text = nonempty(params, "message", nonempty(params, "summary", ""));
    match kind {
        NoticeKind::Rerouted => format!(
            "Model changed from {} to {} (additional safety checks).",
            nonempty(params, "fromModel", "the requested model"),
            nonempty(params, "toModel", "another model"),
        ),
        NoticeKind::StrictReview => {
            "Codex requires additional safety checks. Some tools may take longer.".into()
        }
        NoticeKind::McpLogin if params["success"] == false => format!(
            "{}: {}",
            nonempty(params, "name", "Connected tool"),
            nonempty(params, "error", "Sign-in failed.")
        ),
        NoticeKind::McpLogin => String::new(),
        NoticeKind::McpStartup
            if matches!(params["status"].as_str(), Some("failed" | "cancelled")) =>
        {
            format!(
                "{}: {}",
                nonempty(params, "name", "Connected tool"),
                nonempty(params, "error", nonempty(params, "status", ""))
            )
        }
        NoticeKind::McpStartup => String::new(),
        NoticeKind::Guardian if text.starts_with("Automatic approval review approved (") => {
            String::new()
        }
        NoticeKind::Warning
        | NoticeKind::Guardian
        | NoticeKind::Config
        | NoticeKind::Deprecation
        | NoticeKind::HookStarted
        | NoticeKind::HookCompleted
        | NoticeKind::Safety
        | NoticeKind::Verification
        | NoticeKind::ReviewStarted
        | NoticeKind::ReviewCompleted
        | NoticeKind::Error
        | NoticeKind::AuthStarted
        | NoticeKind::AuthCompleted => text.into(),
    }
}

pub(super) fn decide(
    snapshot: &super::snapshot::NotificationSnapshot,
    method: &str,
    kind: NoticeKind,
    params: &Value,
    account: &str,
    generation: u64,
    at: f64,
) -> Result<NoticeDecision, NotificationDispatchError> {
    let original = snapshot.agent();
    let previous_item = snapshot.item();
    let mut decision = NoticeDecision::default();
    let turn = &params["turnId"];
    if matches!(kind, NoticeKind::HookStarted | NoticeKind::HookCompleted) {
        if params["threadId"].as_str().is_some_and(|s| !s.is_empty()) {
            decision.item = hook(kind, params, previous_item);
        }
        return Ok(decision);
    }
    if !captures_analytics(
        original.turn_id.as_deref(),
        original.in_flight == Some(true),
        method,
        params,
    ) {
        return Ok(decision);
    }
    let mut agent = original.clone();
    match kind {
        NoticeKind::Safety => {
            let previous = original.native_safety_buffering.as_ref();
            let turn_id = Option::<String>::deserialize(turn)?;
            let same =
                previous.and_then(|previous| previous.turn_id.as_deref()) == turn_id.as_deref();
            let mut buffering = NativeSafetyBuffering {
                account_key: None,
                at: None,
                connection_id: None,
                dismissed: None,
                response_started: None,
                faster_model: Option::<String>::deserialize(&params["fasterModel"])?,
                show_buffering_ui: Option::<bool>::deserialize(&params["showBufferingUi"])?,
                thread_id: Option::<String>::deserialize(&params["threadId"])?,
                turn_id: turn_id.clone(),
            };
            buffering.account_key = Some(account.to_owned());
            buffering.connection_id = Some(format!("connection-{generation}"));
            buffering.at = Some(if same {
                previous.and_then(|previous| previous.at).unwrap_or(at)
            } else {
                at
            });
            buffering.dismissed =
                Some(same && previous.is_some_and(|previous| previous.dismissed == Some(true)));
            buffering.response_started = Some(
                (same && previous.is_some_and(|previous| previous.response_started == Some(true)))
                    || original.native_response_turn.as_deref() == turn_id.as_deref(),
            );
            if params["showBufferingUi"] == true && buffering.response_started != Some(true) {
                agent.native_status = Some(NativeStatus {
                    phase: Some(NativeStatusPhase::Safety),
                    turn_id,
                    at: Some(at),
                    message: Some("Codex is checking this request".into()),
                    error: None,
                });
                agent.activity = Some(AgentActivity {
                    phase: Some("safety".into()),
                    at: Some(at),
                    ..Default::default()
                });
            } else if agent.native_status.as_ref().and_then(|status| status.phase)
                == Some(NativeStatusPhase::Safety)
            {
                agent.native_status = None;
                agent.activity = Some(AgentActivity {
                    phase: Some("thinking".into()),
                    at: Some(at),
                    ..Default::default()
                });
            }
            agent.native_safety_buffering = Some(buffering);
        }
        NoticeKind::Verification => {
            if params["verifications"]
                .as_array()
                .is_some_and(|v| v.contains(&json!("trustedAccessForCyber")))
            {
                decision.item = Some(notice(
                    "model-verification".into(),
                    "Codex reports additional cybersecurity checks. Responses may take longer."
                        .into(),
                    "warning",
                    json!({"turnId":turn,"details":"Trusted Access: https://chatgpt.com/cyber"}),
                ));
            }
        }
        NoticeKind::ReviewStarted | NoticeKind::ReviewCompleted => {
            let review = &params["review"];
            let status = nonempty(review, "status", "");
            let label = match status {
                "inProgress" => "in progress",
                "timedOut" => "timed out",
                value => value,
            };
            let completed = kind == NoticeKind::ReviewCompleted;
            let approved = completed && status == "approved";
            let text = if approved {
                String::new()
            } else if completed {
                format!("Tool safety review: {label}")
            } else {
                "Checking tool permissions".into()
            };
            decision.item = Some(notice(
                format!("review:{}", params["reviewId"].as_str().unwrap_or("None")),
                text,
                if completed && !approved {
                    "warning"
                } else {
                    "info"
                },
                json!({"turnId":turn,"details":review.get("rationale").filter(|v| !v.is_null() && **v != "").unwrap_or(&review["reason"]),"nativeReview":params,"nativeHookQuiet":approved}),
            ));
        }
        NoticeKind::Error => {
            if params["threadId"].as_str().is_none_or(str::is_empty)
                || turn.as_str().is_none_or(str::is_empty)
            {
                return Ok(decision);
            }
            let label = provider_label(original.provider.value().copied());
            let fallback = json!({"message": format!("{label} reported an error.")});
            let raw_error = params
                .get("error")
                .filter(|v| !v.is_null() && **v != json!({}))
                .unwrap_or(&fallback);
            let typed: studio_runtime_schema::provider_errors::AgentError =
                serde::Deserialize::deserialize(raw_error.clone())?;
            let error = serde::Serialize::serialize(&typed, studio_json::value::Serializer)?;
            let error_kind = known_error_kind(&error);
            let policy = matches!(
                error_kind,
                Some(
                    KnownCodexErrorKind::CyberPolicy
                        | KnownCodexErrorKind::MisalignmentPolicyViolation
                )
            );
            if params["willRetry"] == true && !policy {
                agent.native_status = Some(NativeStatus {
                    phase: Some(NativeStatusPhase::Retrying),
                    error: Some(error_message(&error, label)),
                    turn_id: Option::<String>::deserialize(turn)?,
                    at: Some(at),
                    message: None,
                });
                agent.activity = Some(AgentActivity {
                    phase: Some("retrying".into()),
                    at: Some(at),
                    ..Default::default()
                });
            } else if error_kind == Some(KnownCodexErrorKind::ActiveTurnNotSteerable) {
                decision.item = Some(notice(
                    format!("steer:{}", turn.as_str().unwrap_or_default()),
                    error_message(&error, label),
                    "warning",
                    json!({"turnId":turn,"nativeError":error}),
                ));
            } else {
                if policy {
                    agent.set_native_failure_hold(true);
                }
                agent.native_status = None;
                agent.native_safety_buffering = None;
                if error_kind == Some(KnownCodexErrorKind::MisalignmentPolicyViolation) {
                    agent.native_thread_block =
                        Field::Value(StoredTransferObject::Known(NativeThreadBlock {
                            thread_id: agent.thread_id.clone().map_or(Field::Null, |id| {
                                Field::Value(StoredTransferValue::Known(id))
                            }),
                            extra: [(
                                "error".into(),
                                studio_json::to_raw_value(&error)
                                    .map_err(studio_runtime_store::StoreError::from)?,
                            )]
                            .into(),
                        }));
                }
                decision.refresh_limits = matches!(
                    error_kind,
                    Some(
                        KnownCodexErrorKind::UsageLimitExceeded
                            | KnownCodexErrorKind::RateLimitExceeded
                    )
                );
                agent.native_turn_error =
                    Field::Value(StoredTransferObject::Known(NativeTurnError {
                        turn_id: Field::Value(StoredTransferValue::Known(
                            turn.as_str().expect("error turn checked above").to_owned(),
                        )),
                        extra: [(
                            "error".into(),
                            studio_json::to_raw_value(&error)
                                .map_err(studio_runtime_store::StoreError::from)?,
                        )]
                        .into(),
                    }));
                agent.error = Field::Value(typed);
                agent.activity = Some(AgentActivity {
                    phase: Some("error".into()),
                    at: Some(at),
                    ..Default::default()
                });
                decision.item = Some(notice(
                    format!("error:{}", turn.as_str().unwrap_or_default()),
                    error_message(&error, label),
                    "error",
                    json!({"turnId":turn,"nativeError":error,"cyberAccessProgram":agent.cyber_access_program.value()}),
                ));
            }
        }
        NoticeKind::AuthStarted | NoticeKind::AuthCompleted => {
            let started = kind == NoticeKind::AuthStarted;
            if started {
                agent.native_status = Some(NativeStatus {
                    phase: Some(NativeStatusPhase::Auth),
                    message: params["message"].as_str().map(str::to_owned),
                    turn_id: Option::<String>::deserialize(turn)?,
                    at: Some(at),
                    error: None,
                });
                agent.activity = Some(AgentActivity {
                    phase: Some("auth".into()),
                    at: Some(at),
                    ..Default::default()
                });
            } else if agent.native_status.as_ref().and_then(|status| status.phase)
                == Some(NativeStatusPhase::Auth)
            {
                agent.native_status = None;
                agent.activity = Some(AgentActivity {
                    phase: Some("thinking".into()),
                    at: Some(at),
                    ..Default::default()
                });
            }
            decision.item = Some(notice(
                format!("auth:{}", turn.as_str().unwrap_or("None")),
                nonempty(
                    params,
                    "message",
                    if started {
                        "Restoring sign-in."
                    } else {
                        "Sign-in restored."
                    },
                )
                .into(),
                "info",
                json!({"turnId":turn}),
            ));
        }
        NoticeKind::Warning
        | NoticeKind::Guardian
        | NoticeKind::Config
        | NoticeKind::Deprecation
        | NoticeKind::Rerouted
        | NoticeKind::McpStartup
        | NoticeKind::McpLogin
        | NoticeKind::StrictReview
        | NoticeKind::HookStarted
        | NoticeKind::HookCompleted => {
            let text = warning_text(kind, params);
            if text.is_empty() {
                return Ok(decision);
            }
            let identity = json!([method, text, params["details"]]);
            let key = sha256_hex(json_text(&identity).as_bytes());
            decision.item = Some(notice(
                key[..24].into(),
                text,
                "warning",
                json!({"details":params.get("details").unwrap_or(&params["reason"]),"accountWide":params["threadId"].as_str().is_none_or(str::is_empty)}),
            ));
        }
    }
    agent.events = Some(original.events.unwrap_or_default().saturating_add(1));
    agent.last_event = Some(format_utc(at)?);
    decision.agent = Some(agent);
    Ok(decision)
}

fn hook(kind: NoticeKind, params: &Value, previous: Option<&Value>) -> Option<NoticeItem> {
    let mut run = params.get("run")?.as_object()?.clone();
    let id = run
        .get("id")?
        .as_str()
        .filter(|s| !s.is_empty())?
        .to_owned();
    if kind == NoticeKind::HookStarted
        && previous
            .and_then(|p| p["nativeHook"]["status"].as_str())
            .is_some_and(|s| !s.is_empty() && s != "running")
    {
        return None;
    }
    let mut warning_seen = false;
    let entries: Vec<Value> = run
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| {
            if entry["kind"] == "context" {
                return false;
            }
            if entry["kind"] == "warning" {
                if warning_seen {
                    return false;
                }
                warning_seen = true;
            }
            true
        })
        .cloned()
        .collect();
    let status = run
        .get("status")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(if kind == NoticeKind::HookStarted {
            "running"
        } else {
            "completed"
        })
        .to_owned();
    run.insert("status".into(), json!(status));
    run.insert("entries".into(), json!(entries));
    if kind == NoticeKind::HookCompleted && status == "completed" && entries.is_empty() {
        return Some(notice(
            format!("hook:{id}"),
            String::new(),
            "info",
            json!({"turnId":params["turnId"],"nativeHook":run,"nativeHookQuiet":true}),
        ));
    }
    let mut text = match status.as_str() {
        "running" => "Hook running".into(),
        "completed" => "Hook completed".into(),
        "failed" => "Hook failed".into(),
        "blocked" => "Blocked by hook".into(),
        "stopped" => "Hook stopped".into(),
        _ => format!("Hook {status}"),
    };
    if let Some(event) = run
        .get("eventName")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        text += &format!(": {event}");
    }
    let details = run
        .get("statusMessage")
        .into_iter()
        .chain(entries.iter().filter_map(|e| e.get("text")))
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    Some(notice(
        format!("hook:{id}"),
        text,
        if matches!(status.as_str(), "failed" | "blocked" | "stopped") {
            "error"
        } else {
            "info"
        },
        json!({"turnId":params["turnId"],"details":if details.is_empty(){Value::Null}else{json!(details)},"nativeHook":run}),
    ))
}

const ERROR_MESSAGES: &[(KnownCodexErrorKind, &str)] = &[
    (
        KnownCodexErrorKind::CyberPolicy,
        "Codex refused this turn under its cybersecurity policy.",
    ),
    (
        KnownCodexErrorKind::MisalignmentPolicyViolation,
        "Codex refused this turn under its safety policy.",
    ),
    (
        KnownCodexErrorKind::TooManyDenials,
        "Codex stopped this turn after too many denied actions. Review the request and approval settings before trying again.",
    ),
    (
        KnownCodexErrorKind::FlexUnavailable,
        "The selected model is temporarily unavailable on the flexible processing tier. Try again later or choose another model.",
    ),
];
/// Notice text for a provider error; `label` names the agent's provider.
pub(crate) fn error_message(error: &Value, label: &str) -> String {
    if crate::disk_space::is_disk_full_error(error) {
        return crate::disk_space::ERROR_MESSAGE.into();
    }
    let kind = known_error_kind(error);
    if let Some((_, message)) = ERROR_MESSAGES.iter().find(|(k, _)| Some(*k) == kind) {
        return (*message).into();
    }
    error["message"]
        .as_str()
        .or_else(|| error.as_str())
        .filter(|s| !s.is_empty())
        .map_or_else(|| format!("{label} reported an error."), str::to_owned)
}

/// Display a known identity, or a generic label for an invalid stored provider.
pub(crate) fn provider_label(
    provider: Option<studio_runtime_schema::ProviderKind>,
) -> &'static str {
    provider.map_or("Provider", studio_runtime_schema::ProviderKind::label)
}

pub(super) use studio_runtime_store::python_json_utf8 as json_text;

/// Preserve generated object's insertion order, including fields appended by a transition.
pub(super) fn ordered_text(value: &Value, keys: impl IntoIterator<Item = String>) -> String {
    ordered_text_with(value, keys, json_text)
}

pub(super) fn ordered_ascii(value: &Value, keys: impl IntoIterator<Item = String>) -> String {
    ordered_text_with(value, keys, studio_runtime_store::python_json)
}

fn ordered_text_with(
    value: &Value,
    keys: impl IntoIterator<Item = String>,
    render: fn(&Value) -> String,
) -> String {
    let Some(fields) = value.as_object() else {
        return render(value);
    };
    let mut ordered = Vec::new();
    for key in keys.into_iter().chain(fields.keys().cloned()) {
        if fields.contains_key(&key) && !ordered.contains(&key) {
            ordered.push(key);
        }
    }
    format!(
        "{{{}}}",
        ordered
            .iter()
            .map(|key| format!("{}: {}", render(&json!(key)), render(&fields[key])))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(super) fn object_keys(text: &str) -> Option<Vec<String>> {
    use serde::de::{Deserializer, MapAccess, Visitor};
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("JSON object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut keys = Vec::new();
            while let Some((key, _)) = map.next_entry::<String, serde::de::IgnoredAny>()? {
                keys.push(key);
            }
            Ok(keys)
        }
    }
    studio_json::Deserializer::from_str(text)
        .deserialize_map(Keys)
        .ok()
}

pub(crate) fn is_notice(method: &str) -> bool {
    crate::notification_dispatch::kinds::notice_kind(method).is_some()
}

pub(crate) fn captures_analytics(
    agent_turn: Option<&str>,
    in_flight: bool,
    method: &str,
    params: &Value,
) -> bool {
    let Some(kind) = crate::notification_dispatch::kinds::notice_kind(method) else {
        return false;
    };
    let thread = params["threadId"].as_str().filter(|s| !s.is_empty());
    let turn = params["turnId"].as_str().filter(|s| !s.is_empty());
    !matches!(kind, NoticeKind::HookStarted | NoticeKind::HookCompleted)
        && !(thread.is_none() && is_account_notice(kind))
        && !turn.is_some_and(|turn| agent_turn != Some(turn) || !in_flight)
        && !(matches!(
            kind,
            NoticeKind::Safety
                | NoticeKind::Verification
                | NoticeKind::ReviewStarted
                | NoticeKind::ReviewCompleted
        ) && (thread.is_none() || turn.is_none()))
}

/// Account notices can arrive before the first thread/agent exists. The caller
/// has already checked the current account connection and owns its receipt.
pub(crate) fn account_notice(
    tx: &studio_runtime_store::Transaction<'_>,
    method: &str,
    params: &Value,
    identity: &super::ProviderDeliveryIdentity,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    #[allow(unused_imports)]
    use rusqlite::{OptionalExtension, params};
    use studio_runtime_store::SyncDocumentsRepository;
    let account = identity
        .provider_handle
        .strip_prefix("account:")
        .filter(|s| !s.is_empty())
        .ok_or(NotificationDispatchError::invalid())?;
    let kind = crate::notification_dispatch::kinds::notice_kind(method)
        .filter(|kind| is_account_notice(*kind))
        .ok_or(NotificationDispatchError::invalid())?;
    let connection = format!("connection-{}", identity.generation);
    let text = warning_text(kind, params);
    let key = sha256_hex(
        studio_runtime_store::python_json(&json!([
            account,
            connection,
            method,
            if method.starts_with("mcpServer/") {
                params["name"].clone()
            } else {
                json!(text)
            }
        ]))
        .as_bytes(),
    );
    if text.is_empty() {
        tx.execute_cached("DELETE FROM runtime_native_notices WHERE id=?1", [&key])
            .map_err(studio_runtime_store::StoreError::from)?;
    } else {
        let record = json!({"id":key,"accountKey":account,"connectionId":connection,"message":text,"details":params["details"],"at":at});
        tx.execute_cached("INSERT INTO runtime_native_notices(id,record) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            params![key,studio_json::to_string(&record)?]).map_err(studio_runtime_store::StoreError::from)?;
        tx.execute_cached("DELETE FROM runtime_native_notices WHERE json_extract(record,'$.accountKey')=?1 AND id NOT IN (SELECT id FROM runtime_native_notices WHERE json_extract(record,'$.accountKey')=?1 ORDER BY json_extract(record,'$.at') DESC LIMIT ?2)", params![account, ACCOUNT_NOTICE_LIMIT]).map_err(studio_runtime_store::StoreError::from)?;
    }
    // Preserve unrelated volatile sources and other accounts' notices. Replace
    // only this connection owner's portion of the existing renderer projection.
    let workspace: Option<String> =
        studio_runtime_store::ProjectionPublicationQueries::raw_payload(
            tx,
            "workspace",
            "current",
            true,
        )?;
    if let Some(raw) = workspace {
        let workspace: Value = studio_json::from_str(&raw)
            .map_err(|_| super::stored("sync_entities", "workspace:current"))?;
        let mut notices = match &workspace["value"]["nativeNotices"] {
            Value::Null => Vec::new(),
            Value::Array(notices) => notices.clone(),
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
                return Err(super::stored("sync_entities", "workspace:current"));
            }
        };
        let version_warnings: Vec<_> = notices
            .iter()
            .filter(|notice| notice.get("connectionId").is_none())
            .cloned()
            .collect();
        notices.retain(|notice| {
            notice.get("connectionId").is_some() && notice["accountKey"] != account
        });
        let mut stmt = tx.prepare_cached("SELECT id,record FROM runtime_native_notices WHERE json_extract(record,'$.accountKey')=?1 AND json_extract(record,'$.connectionId')=?2 ORDER BY rowid").map_err(studio_runtime_store::StoreError::from)?;
        for row in stmt
            .query_map(params![account, connection], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(studio_runtime_store::StoreError::from)?
        {
            let (id, raw) = row.map_err(studio_runtime_store::StoreError::from)?;
            let record: Value = studio_json::from_str(&raw)
                .map_err(|_| super::stored("runtime_native_notices", &id))?;
            if record["id"] != id
                || !record["message"].is_string()
                || record["at"]
                    .as_f64()
                    .is_none_or(|at| !at.is_finite() || at < 0.0)
            {
                return Err(super::stored("runtime_native_notices", &id));
            }
            notices.push(record);
        }
        notices.extend(version_warnings);
        SyncDocumentsRepository::patch_entity(
            tx,
            "workspace",
            "current",
            json!({"nativeNotices":notices}),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod provider_label_tests {
    use super::{error_message, provider_label};
    use studio_json::json;

    #[test]
    fn provider_identity_unknown_notice_uses_generic_label() {
        for agent in [
            json!({}),
            json!({"provider":null}),
            json!({"provider":"future"}),
            json!({"provider":7}),
        ] {
            assert_eq!(
                provider_label(studio_runtime_schema::ProviderKind::of_record(&agent).ok()),
                "Provider",
                "{agent}"
            );
            assert_eq!(
                error_message(
                    &json!({}),
                    provider_label(studio_runtime_schema::ProviderKind::of_record(&agent).ok())
                ),
                "Provider reported an error."
            );
        }
        assert_eq!(
            provider_label(
                studio_runtime_schema::ProviderKind::of_record(&json!({"provider":"codex"})).ok()
            ),
            "Codex"
        );
        assert_eq!(
            provider_label(
                studio_runtime_schema::ProviderKind::of_record(&json!({"provider":"claude"})).ok()
            ),
            "Claude"
        );
    }

    #[test]
    fn error_notices_name_the_agents_provider() {
        let extension = json!({"message":"Provider detail","codexErrorInfo":{"cyberPolicy":{},"futureCode":{"opaque":[1,null]}},"vendorExtension":{"id":42}});
        let original = extension.clone();
        assert_eq!(error_message(&extension, "Codex"), "Provider detail");
        assert_eq!(extension, original);
        assert_eq!(
            provider_label(
                studio_runtime_schema::ProviderKind::of_record(&json!({"provider": "claude"})).ok()
            ),
            "Claude"
        );
        assert_eq!(
            provider_label(
                studio_runtime_schema::ProviderKind::of_record(&json!({"provider":"codex"})).ok()
            ),
            "Codex"
        );
        assert_eq!(
            error_message(&json!({}), "Claude"),
            "Claude reported an error."
        );
        assert_eq!(
            error_message(&json!({"codexErrorInfo": "cyberPolicy"}), "Codex"),
            "Codex refused this turn under its cybersecurity policy."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification_dispatch::snapshot::NotificationSnapshot;

    #[test]
    fn reroute_is_a_readable_notice_for_the_current_turn() {
        let snapshot = NotificationSnapshot::from_records(
            AgentRecord::from_value(json!({"id":"agent","rootId":"agent","epoch":1,"status":"running","threadId":"thread","accountKey":"default","provider":"codex","turnId":"turn","inFlight":true,"compactions":0})).unwrap(), None, None,
        ).unwrap();
        let params = json!({"threadId":"thread","turnId":"turn","fromModel":"gpt-original","toModel":"gpt-routed","reason":"highRiskCyberActivity"});
        let kind = crate::notification_dispatch::kinds::notice_kind("model/rerouted")
            .expect("reroutes have a notice projection");
        let notice = decide(
            &snapshot,
            "model/rerouted",
            kind,
            &params,
            "default",
            1,
            1.0,
        )
        .unwrap()
        .item
        .unwrap();
        assert_eq!(
            notice.text,
            "Model changed from gpt-original to gpt-routed (additional safety checks)."
        );
        assert_eq!(notice.metadata["nativeNotice"], "warning");
        let mut stale = params;
        stale["turnId"] = json!("old-turn");
        assert!(
            decide(&snapshot, "model/rerouted", kind, &stale, "default", 1, 2.0)
                .unwrap()
                .item
                .is_none()
        );
    }

    #[test]
    fn encoded_biological_error_is_normalized_in_published_notice() {
        let snapshot = NotificationSnapshot::from_records(
            AgentRecord::from_value(json!({"id":"agent","rootId":"agent","epoch":1,"status":"running","threadId":"thread","accountKey":"default","provider":"codex","turnId":"turn","inFlight":true,"autoWake":false,"created":0,"compactions":0,"activeTools":[],"events":0})).unwrap(),
            None,
            None,
        )
        .unwrap();
        let raw = json!({"message":r#"{"message":"This content was flagged for possible biological risk."}"#});
        let params = json!({"threadId":"thread","turnId":"turn","error":raw});
        let decision = decide(
            &snapshot,
            "error",
            NoticeKind::Error,
            &params,
            "default",
            1,
            1.0,
        )
        .unwrap();
        let metadata = decision.item.unwrap().metadata;
        let error = metadata.get("nativeError").unwrap();
        assert_eq!(
            error.get("message"),
            Some(&json!(
                "This content was flagged for possible biological risk."
            ))
        );
        assert_eq!(error.get("code"), Some(&json!("bio_policy")));
        assert_eq!(error.get("studioRawEvidence"), Some(&raw));
    }
}

#[cfg(test)]
mod canonical_publication_tests {
    #[tokio::test]
    async fn canonical_account_notice_updates_workspace_projection() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction_with(|tx| {
                let identity = crate::notification_dispatch::ProviderDeliveryIdentity {
                    provider_handle: "account:default".into(),
                    generation: 1,
                    cursor: 1,
                };
                super::account_notice(
                    tx,
                    "warning",
                    &studio_json::json!({"message":"Canonical notice"}),
                    &identity,
                    1.0,
                )?;
                let raw = studio_runtime_store::ProjectionPublicationQueries::raw_payload(
                    tx,
                    "workspace",
                    "current",
                    true,
                )?
                .unwrap();
                assert!(raw.contains("Canonical notice"));
                Ok::<_, crate::notification_dispatch::NotificationDispatchError>(())
            })
            .await
            .unwrap();
    }
}
