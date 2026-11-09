//! Atomic work-item writes that need no worker or filesystem driver.

use std::collections::BTreeMap;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use studio_runtime_schema::persistence::{
    OutboxKind, OutboxStatus, OwnerKind, RequestIdentity, WorkEffectStatus,
};
use studio_runtime_schema::queue_command::Field;
use studio_runtime_schema::raw_json::RawJson;
use studio_runtime_schema::shared_values::{
    TASK_SUBMISSION_MAX_FILES, TASK_SUBMISSION_PATH_MAX_CODEPOINTS,
};
use studio_runtime_store::path_identity::CanonicalPath;

use rusqlite::OptionalExtension;
use studio_json::{Value, json};
use studio_runtime_store::{
    AgentId, AgentRecord, AgentRepository, Connection, LegacyWorkReply, ReviewerArchiveRepository,
    RuntimeStore, Transaction, WorkItemRecord, WorkItemsRepository, WorkStatus,
};

use studio_runtime_store::{
    outbox::{Outbox, OutboxRow},
    request_results::AdmittedRequest,
};

use crate::event_write::{self, EventWriteError as Error};
use thiserror::Error;

static GENERATED_WORK_ID: AtomicU64 = AtomicU64::new(0);
/// Maximum accepted archive receipts handled by one scheduler pass.
pub const ACCEPTED_ARCHIVE_PAGE_SIZE: usize = 16;
const ACCEPTED_ARCHIVE_BASE_DELAY_SECONDS: u64 = 2;
const ACCEPTED_ARCHIVE_MAX_DELAY_SECONDS: u64 = 60;
const ACCEPTED_ARCHIVE_MAX_BACKOFF_EXPONENT: u64 = 6;
// Reviewer archive retry policy; delays and persisted deadlines use seconds.
const REVIEW_ARCHIVE_BASE_DELAY_SECONDS: f64 = 1.0;
const REVIEW_ARCHIVE_MAX_DELAY_SECONDS: f64 = 60.0;
// Capped exponent bounds the seconds-based doubling before the 60-second cap.
const REVIEW_ARCHIVE_MAX_BACKOFF_EXPONENT: u32 = 6;

/// Check whether a submitted commit is on local main without contacting a remote.
/// Repositories with an origin are kept for the separately scheduled remote check.
pub fn accepted_revision_is_on_local_main(repository: &str, revision: &str) -> bool {
    use std::ffi::OsStr;
    use std::time::Duration;

    if !(7..=64).contains(&revision.len()) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let run_git = |args: &[&str]| {
        crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
            program: OsStr::new("git"),
            git_directory: Some(OsStr::new(repository)),
            args,
            environment: &[],
            input: None,
            stdout_limit: 256,
            stderr_limit: 256,
            timeout: Duration::from_secs(2),
        })
        .ok()
        .map(|output| output.stdout.bytes)
    };
    let submitted_ref = format!("{revision}^{{commit}}");
    let Some(submitted) = run_git(&["rev-parse", "--verify", "--end-of-options", &submitted_ref])
    else {
        return false;
    };
    let submitted = String::from_utf8_lossy(&submitted).trim().to_owned();
    let Some(remotes) = run_git(&["remote"]) else {
        return false;
    };
    if String::from_utf8_lossy(&remotes)
        .lines()
        .any(|remote| remote == "origin")
    {
        return false;
    }
    let Some(main) = run_git(&["rev-parse", "--verify", "refs/heads/main^{commit}"]) else {
        return false;
    };
    let main = String::from_utf8_lossy(&main).trim().to_owned();
    run_git(&["merge-base", "--is-ancestor", &submitted, &main]).is_some()
}

/// Identify repositories whose accepted-result main check needs remote transport.
pub fn repository_has_origin(repository: &str) -> bool {
    use std::ffi::OsStr;
    use std::time::Duration;

    crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
        program: OsStr::new("git"),
        git_directory: Some(OsStr::new(repository)),
        args: &["remote"],
        environment: &[],
        input: None,
        stdout_limit: 256,
        stderr_limit: 256,
        timeout: Duration::from_secs(2),
    })
    .ok()
    .is_some_and(|output| {
        String::from_utf8_lossy(&output.stdout.bytes)
            .lines()
            .any(|remote| remote == "origin")
    })
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveIntent {
    status: WorkEffectStatus,
    task_id: String,
    intent_id: String,
    actor: String,
    actor_epoch: i64,
    target: String,
    attempts: u64,
    created: f64,
    next_attempt_at: Option<f64>,
    attempt_started_at: Option<f64>,
    updated: Option<f64>,
    last_outcome: Option<RawJson>,
    outcome: Option<RawJson>,
    public_request_id: Option<String>,
    public_admission: Option<AdmittedRequest>,
    public_response: Option<LegacyWorkReply>,
}

#[derive(serde::Deserialize)]
struct ArchiveNotice {
    version: i64,
    decisions: Vec<ArchiveDecision>,
}
#[derive(serde::Deserialize)]
struct ArchiveDecision {
    reason: Option<String>,
}

#[derive(serde::Serialize)]
struct WorkDecisionNotice<'a> {
    task: &'a str,
    decision: &'a str,
    reason: Option<&'a str>,
}

fn decision_notice(task: &str, decision: &str, reason: Option<&str>) -> String {
    studio_runtime_store::projection_value(&WorkDecisionNotice {
        task,
        decision,
        reason,
    })
    .to_string()
}

#[derive(serde::Serialize)]
struct WorkReviewNotice<'a> {
    task: &'a str,
    title: Option<&'a str>,
    result: RawJson,
}

fn work_review_notice(task: &str, title: Option<&str>, result: &Value) -> String {
    studio_runtime_store::projection_value(&WorkReviewNotice {
        task,
        title,
        result: RawJson(studio_json::to_raw_value(result).expect("JSON result serializes")),
    })
    .to_string()
}

#[derive(serde::Serialize)]
struct ResultRefusalNotice<'a> {
    task: &'a str,
    owner: &'a str,
    version: Option<&'a studio_json::Number>,
    refusal: RawJson,
}

fn refusal_notice(
    task: &str,
    owner: &str,
    version: Option<&studio_json::Number>,
    refusal: &Value,
) -> String {
    studio_runtime_store::projection_value(&ResultRefusalNotice {
        task,
        owner,
        version,
        refusal: RawJson(studio_json::to_raw_value(refusal).expect("JSON value serializes")),
    })
    .to_string()
}

#[derive(serde::Deserialize)]
struct ResultIdentity {
    #[serde(default, deserialize_with = "optional_string")]
    id: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    revision: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChildResultIdentity {
    #[serde(default, deserialize_with = "optional_string")]
    agent_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewerArchiveView {
    #[serde(default, deserialize_with = "optional_string")]
    review_archive_scheduled: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    parent_id: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    root_id: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    role: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    status: Option<String>,
    agent_archive: Option<RawJson>,
    review_archive_error: Option<RawJson>,
}

#[derive(serde::Deserialize)]
struct ReviewerArchiveOutcome {
    #[serde(default, deserialize_with = "optional_string")]
    status: Option<String>,
    blockers: Option<RawJson>,
}

fn raw_truthy(raw: Option<&RawJson>) -> bool {
    let value = raw.and_then(|raw| studio_json::from_str::<Value>(raw.0.as_str()).ok());
    event_write::truthy(value.as_ref())
}

fn result_identity(work: &WorkItemRecord) -> Option<ResultIdentity> {
    let result = work.results()?.last()?;
    <ResultIdentity as serde::Deserialize>::deserialize(&Value::Object(result.clone())).ok()
}

fn optional_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrOther {
        String(String),
        Other(serde::de::IgnoredAny),
    }
    Ok(
        match <StringOrOther as serde::Deserialize>::deserialize(deserializer)? {
            StringOrOther::String(value) => Some(value),
            StringOrOther::Other(_) => None,
        },
    )
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveOutcomeView {
    status: ArchiveOutcomeStatus,
    #[serde(default)]
    retryable: bool,
    #[serde(default)]
    cleanup_pending: bool,
}

#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArchiveOutcomeStatus {
    Archived,
    Blocked,
    Kept,
    Skipped,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct KeptOutcome<'a> {
    status: &'static str,
    reason: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    cleanup_pending: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tasks: Option<&'a [String]>,
}

fn kept_outcome(reason: &str) -> KeptOutcome<'_> {
    KeptOutcome {
        status: "kept",
        reason,
        cleanup_pending: None,
        retryable: None,
        tasks: None,
    }
}

fn kept(reason: &str) -> Value {
    studio_runtime_store::projection_value(&kept_outcome(reason))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ArchiveWorktreeState {
    Missing,
    Removed,
    Kept,
    None,
}

#[derive(serde::Serialize)]
struct ArchiveWorktree<'a> {
    state: ArchiveWorktreeState,
    bytes: Option<u64>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    reason: Field<&'a str>,
}

#[derive(serde::Serialize)]
struct ArchivedOutcome<'a> {
    status: &'static str,
    worktree: ArchiveWorktree<'a>,
}

#[cfg(test)]
mod archive_outcome_view_tests {
    use super::{ArchiveOutcomeStatus, ArchiveOutcomeView, RawJson, ResultIdentity};

    #[test]
    fn legacy_unused_fields_are_preserved_without_decoding() {
        let encoded = r#"{"status":"archived","agent":{"epoch":["legacy"]},"worktree":42,"extra":{"nested":true}}"#;
        let raw: RawJson = studio_json::from_str(encoded).expect("raw outcome");
        let view: ArchiveOutcomeView = studio_json::from_str(raw.0.as_str()).expect("outcome view");
        assert_eq!(view.status, ArchiveOutcomeStatus::Archived);
        assert!(!view.retryable);
        assert!(!view.cleanup_pending);
        assert_eq!(studio_json::to_string(&raw).expect("round trip"), encoded);
    }

    #[test]
    fn result_identity_ignores_legacy_fields_and_wrong_revision_type() {
        let encoded =
            r#"{"id":"r1","revision":{"old":true},"files":false,"result":{"nested":[1,2]}}"#;
        let identity: ResultIdentity = studio_json::from_str(encoded).expect("identity");
        assert_eq!(identity.id.as_deref(), Some("r1"));
        assert_eq!(identity.revision, None);
        let raw: RawJson = studio_json::from_str(encoded).expect("raw result");
        assert_eq!(studio_json::to_string(&raw).expect("round trip"), encoded);
    }
}

/// Retry accepted-result archives whose Python-compatible retry deadline has
/// elapsed. `now` is injected so scheduling and exponential backoff are
/// deterministic in tests. Pending/unknown receipts are reconciled against the
/// target before any retry; a known retryable response may be retried with the
/// same intent identity and target.
pub async fn accepted_archive_tick(
    store: Arc<RuntimeStore>,
    now: f64,
) -> Result<usize, WorkWriteError> {
    reviewer_archive_tick(Arc::clone(&store), now).await?;
    let due = store
        .read_async("runtime-services/work_write", move |connection| {
            accepted_archive_due_records(connection, now)
        })
        .await?;
    let mut handled = 0;
    for (task_id, encoded_intent) in due {
        if matches!(
            run_accepted_archive(&store, task_id, encoded_intent, now, ArchiveTiming::Due).await?,
            ArchiveRun::Handled(_)
        ) {
            handled += 1;
        }
    }
    Ok(handled)
}

#[derive(Clone, Copy)]
enum ArchiveTiming {
    Immediate,
    Due,
}
enum ArchiveRun {
    Skipped,
    Handled(LegacyWorkReply),
}

async fn run_accepted_archive(
    store: &Arc<RuntimeStore>,
    task_id: String,
    encoded_intent: String,
    now: f64,
    timing: ArchiveTiming,
) -> Result<ArchiveRun, WorkWriteError> {
    // Archive intents of older builds lack the typed identity; they are left as they are.
    let Ok(intent) = studio_json::from_str::<ArchiveIntent>(&encoded_intent) else {
        return Ok(ArchiveRun::Skipped);
    };
    let task_id_for_read = task_id.clone();
    let Some(work) = store
        .read_async("runtime-services/work_write", move |connection| {
            Ok::<_, WorkWriteError>(WorkItemsRepository::work_record(
                connection,
                &task_id_for_read,
            )?)
        })
        .await?
    else {
        return Ok(ArchiveRun::Skipped);
    };
    if work.status() != studio_runtime_store::WorkStatus::Accepted {
        return Ok(ArchiveRun::Skipped);
    }
    let intent_id = &intent.intent_id;
    let owner_id = work.owner().unwrap_or_default().to_owned();
    let root_id = work.root_id().unwrap_or_default().to_owned();
    if intent_id.is_empty() || owner_id.is_empty() || root_id.is_empty() {
        return Ok(ArchiveRun::Skipped);
    }
    let task_id_for_claim = task_id.clone();
    let owner_for_claim = owner_id.clone();
    let root_for_claim = root_id.clone();
    let Some((intent, prior_status)) = store
        .transaction_with_error(
            move |tx| -> Result<Option<(ArchiveIntent, WorkEffectStatus)>, WorkWriteError> {
                let mut current = WorkItemsRepository::work_record(tx, &task_id_for_claim)?
                    .ok_or(Error::InvalidRequest("Unknown work item"))?;
                if current.status() != studio_runtime_store::WorkStatus::Accepted
                    || current.owner() != Some(owner_for_claim.as_str())
                    || current.root_id() != Some(root_for_claim.as_str())
                {
                    return Ok(None);
                }
                let raw: String = tx.query_row(
                    "SELECT json_extract(record,'$.archiveIntent') FROM runtime_work WHERE id=?1",
                    [&task_id_for_claim], |row| row.get(0),
                ).map_err(studio_runtime_store::StoreError::from)?;
                let mut intent: ArchiveIntent =
                    studio_json::from_str(&raw).map_err(studio_runtime_store::StoreError::from)?;
                let prior_status = intent.status;
                if !matches!(
                    prior_status,
                    WorkEffectStatus::Pending
                        | WorkEffectStatus::Retryable
                        | WorkEffectStatus::Unknown
                ) || (matches!(timing, ArchiveTiming::Due)
                    && intent.next_attempt_at.unwrap_or(0.0) > now)
                {
                    return Ok(None);
                }
                let claimed = intent.clone();
                intent.status = WorkEffectStatus::Unknown;
                intent.attempt_started_at = Some(now);
                intent.next_attempt_at = Some(now + ACCEPTED_ARCHIVE_MAX_DELAY_SECONDS as f64);
                current
                    .set(
                        "archiveIntent",
                        studio_runtime_store::projection_value(&intent),
                    )
                    .map_err(studio_runtime_store::StoreError::from)?;
                WorkItemsRepository::update_checked_work(tx, &task_id_for_claim, &current)?;
                Ok(Some((claimed, prior_status)))
            },
        )
        .await?
    else {
        return Ok(ArchiveRun::Skipped);
    };
    let dispatch_actor_epoch = intent.actor_epoch;

    let work_for_preflight = work.clone();
    let intent_for_preflight = intent.clone();
    let owner_for_preflight = owner_id.clone();
    let root_for_preflight = root_id.clone();
    let (preflight, owner_snapshot, revision) = store
        .read_async("runtime-services/work_write", move |connection| {
            let result = result_identity(&work_for_preflight);
            let outcome = accepted_archive_preflight(
                connection,
                &work_for_preflight,
                result.as_ref(),
                &intent_for_preflight,
                &owner_for_preflight,
                &root_for_preflight,
                dispatch_actor_epoch,
            )?;
            let owner =
                AgentRepository::get(connection, &AgentId::from(owner_for_preflight.as_str()))?
                    .map(Box::new);
            Ok::<_, WorkWriteError>((outcome, owner, result.and_then(|r| r.revision)))
        })
        .await?;
    let mut verify_terminal = false;
    let preflight = if preflight.is_none() {
        let owner = owner_snapshot.as_ref().expect("preflight requires owner");
        if owner.execution_mode != Some(studio_runtime_store::AgentExecutionMode::Vm)
            && owner.workspace_mode
                != Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
        {
            let cwd = owner.cwd.as_deref().unwrap_or_default().to_owned();
            let revision = revision.unwrap_or_default();
            let (on_main, remote) = studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-services/accepted-archive/git",
                move || {
                    let on_main =
                        !cwd.is_empty() && accepted_revision_is_on_local_main(&cwd, &revision);
                    let remote = !on_main && !cwd.is_empty() && repository_has_origin(&cwd);
                    (on_main, remote)
                },
            )
            .await
            .map_err(|_| WorkWriteError::PreparationWorker)?;
            if !on_main {
                verify_terminal = true;
                let reason = if remote {
                    "Remote-origin main verification requires cross-server/remote-transport"
                } else {
                    "The result commit is not reachable from local main"
                };
                Some(kept(reason))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        preflight
    };
    let outcome = match preflight {
        Some(value) => Ok(value),
        None if prior_status == WorkEffectStatus::Unknown => return Ok(ArchiveRun::Skipped),
        None => {
            let guard = ArchiveGuard {
                work: work.clone(),
                owner: owner_snapshot.clone().expect("preflight requires owner"),
                task_id: task_id.clone(),
                owner_id: owner_id.clone(),
                root_id: root_id.clone(),
                intent: intent.clone(),
                actor_epoch: dispatch_actor_epoch,
            };
            crate::agent_management_store::execute(
                Arc::clone(store),
                &crate::agent_management::Action::Archive {
                    actor: root_id.clone(),
                    epoch: dispatch_actor_epoch,
                    target: owner_id.clone(),
                    reason: "Accepted task result is on main".to_owned(),
                    unassign_work: false,
                },
                now,
                Some(Box::new(move |tx| guard.check(tx))),
            )
            .await
        }
    };
    if matches!(
        &outcome,
        Err(crate::agent_management::ManagementError::Invalid(
            "The accepted task owner or result changed"
        ))
    ) {
        return Err(Error::Conflict("The accepted task owner or result changed").into());
    }
    let (receipt_status, archive, retryable) = match outcome {
        Ok(value) => {
            let view = <ArchiveOutcomeView as serde::Deserialize>::deserialize(&value)
                .map_err(studio_runtime_store::StoreError::from)?;
            let retryable = view.retryable
                || view.status == ArchiveOutcomeStatus::Blocked
                || view.cleanup_pending;
            let receipt_status = if retryable {
                WorkEffectStatus::Retryable
            } else if view.status == ArchiveOutcomeStatus::Archived {
                WorkEffectStatus::Complete
            } else {
                WorkEffectStatus::Terminal
            };
            (receipt_status, value, retryable)
        }
        Err(error) => (
            WorkEffectStatus::Unknown,
            kept(&format!("The archive check failed: {error}")),
            false,
        ),
    };
    let task_id_for_settlement = task_id.clone();
    let intent_for_settlement = intent.clone();
    let owner_for_settlement = owner_id.clone();
    let root_for_settlement = root_id.clone();
    let terminal_guard = verify_terminal.then(|| ArchiveGuard {
        work: work.clone(),
        owner: owner_snapshot.expect("preflight requires owner"),
        task_id: task_id.clone(),
        owner_id: owner_id.clone(),
        root_id: root_id.clone(),
        intent: intent.clone(),
        actor_epoch: dispatch_actor_epoch,
    });
    let result = store
        .transaction_with_error(move |tx| -> Result<ArchiveRun, WorkWriteError> {
            if let Some(guard) = &terminal_guard {
                guard
                    .check(tx)
                    .map_err(|_| Error::Conflict("The accepted task owner or result changed"))?;
            }
            let Some(mut current) = WorkItemsRepository::work_record(tx, &task_id_for_settlement)?
            else {
                return Ok(ArchiveRun::Skipped);
            };
            if current.status() != studio_runtime_store::WorkStatus::Accepted
                || current.owner() != Some(owner_for_settlement.as_str())
                || current.root_id() != Some(root_for_settlement.as_str())
            {
                return Ok(ArchiveRun::Skipped);
            }
            let next_attempt = intent_for_settlement.attempts + 1;
            let delay = if retryable {
                ACCEPTED_ARCHIVE_BASE_DELAY_SECONDS
                    .pow(next_attempt.min(ACCEPTED_ARCHIVE_MAX_BACKOFF_EXPONENT) as u32)
                    .min(ACCEPTED_ARCHIVE_MAX_DELAY_SECONDS)
            } else {
                ACCEPTED_ARCHIVE_MAX_DELAY_SECONDS
            };
            if receipt_status != WorkEffectStatus::Unknown {
                current
                    .set("archive", archive.clone())
                    .map_err(studio_runtime_store::StoreError::from)?;
            }
            let mut final_intent = intent_for_settlement.clone();
            final_intent.status = if retryable {
                WorkEffectStatus::Pending
            } else {
                receipt_status
            };
            final_intent.attempts = next_attempt;
            final_intent.next_attempt_at = Some(now + delay as f64);
            let raw_archive = RawJson(
                studio_json::to_raw_value(&archive)
                    .map_err(studio_runtime_store::StoreError::from)?,
            );
            final_intent.last_outcome = Some(raw_archive.clone());
            final_intent.updated = Some(now);
            final_intent.actor_epoch = dispatch_actor_epoch;
            if !retryable && receipt_status != WorkEffectStatus::Unknown {
                final_intent.outcome = Some(raw_archive);
            }
            current
                .set(
                    "archiveIntent",
                    studio_runtime_store::projection_value(&final_intent),
                )
                .map_err(studio_runtime_store::StoreError::from)?;
            WorkItemsRepository::update_checked_work(tx, &task_id_for_settlement, &current)?;
            if matches!(timing, ArchiveTiming::Immediate)
                && !retryable
                && receipt_status == WorkEffectStatus::Terminal
                && let Some(owner) =
                    AgentRepository::get(tx, &AgentId::from(owner_for_settlement.as_str()))?
                && owner.deleted_at.is_none()
            {
                let raw: String = tx
                    .query_row(
                        "SELECT record FROM runtime_work WHERE id=?1",
                        [&task_id_for_settlement],
                        |row| row.get(0),
                    )
                    .map_err(studio_runtime_store::StoreError::from)?;
                let notice: ArchiveNotice =
                    studio_json::from_str(&raw).map_err(studio_runtime_store::StoreError::from)?;
                let reason = notice
                    .decisions
                    .last()
                    .and_then(|decision| decision.reason.as_deref());
                event_write::enqueue_recovery_event(
                    tx,
                    &owner,
                    format!(
                        "work-decision:{}:{}",
                        task_id_for_settlement,
                        notice.version - 1
                    ),
                    "work_decision",
                    decision_notice(&task_id_for_settlement, "accept", reason),
                    now,
                )?;
            }
            let mut response = match final_intent.public_response {
                Some(response) => response,
                None => LegacyWorkReply::from_value(work_item_view(tx, &current)?)
                    .map_err(studio_runtime_store::StoreError::from)?,
            };
            response
                .set("archive", archive)
                .map_err(studio_runtime_store::StoreError::from)?;
            response
                .set(
                    "archivePending",
                    json!(retryable || !archive_outcome_can_settle(receipt_status)),
                )
                .map_err(studio_runtime_store::StoreError::from)?;
            if archive_outcome_can_settle(receipt_status)
                && let Some(root) = final_intent.public_request_id.as_deref()
            {
                response = settle_accepted_archive(tx, &task_id_for_settlement, root, &response)?;
            }
            Ok(ArchiveRun::Handled(response))
        })
        .await?;
    Ok(result)
}

pub fn archive_outcome_can_settle(status: WorkEffectStatus) -> bool {
    status != WorkEffectStatus::Unknown
}

pub async fn finalize_accepted_archive(
    store: Arc<RuntimeStore>,
    response: WorkItemRecord,
) -> Result<LegacyWorkReply, WorkWriteError> {
    let response = LegacyWorkReply::from_value(response.into_value())
        .map_err(studio_runtime_store::StoreError::from)?;
    let task_id = response.id().0.clone();
    let task_for_read = task_id.clone();
    let intent = store
        .read_async("accepted-archive/intent", move |connection| {
            connection
                .query_row(
                    "SELECT json_extract(record,'$.archiveIntent') FROM runtime_work WHERE id=?1",
                    [&task_for_read],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map(|row| row.flatten())
                .map_err(studio_runtime_store::StoreError::from)
        })
        .await?;
    let Some(intent) = intent else {
        if response
            .get("archivePending")
            .and_then(|value| value.as_bool())
            == Some(true)
        {
            return Err(Error::Conflict("The accepted archive outcome is pending").into());
        }
        return Ok(response);
    };
    let now = event_write::unix_time()?;
    match run_accepted_archive(&store, task_id, intent, now, ArchiveTiming::Immediate).await? {
        ArchiveRun::Handled(result) => Ok(result),
        ArchiveRun::Skipped => {
            Err(Error::Conflict("The accepted archive outcome is pending").into())
        }
    }
}

/// Schedule and retry archiving completed reviewers after their child result
/// has been delivered. The event ID stays in the agent record as the durable
/// identity for every retry; scheduling commits before the archive operation.
async fn reviewer_archive_tick(store: Arc<RuntimeStore>, now: f64) -> Result<(), WorkWriteError> {
    retry_refused_reviewer_archives(&store).await?;
    store
        .transaction_with_error(move |tx| -> Result<(), WorkWriteError> {
            let candidates = ReviewerArchiveRepository::candidates(tx)?;
            for (child_id, encoded) in candidates {
                let record: ReviewerArchiveView = studio_json::from_str(&encoded)
                    .map_err(studio_runtime_store::StoreError::from)?;
                if raw_truthy(record.agent_archive.as_ref())
                    || raw_truthy(record.review_archive_error.as_ref())
                    || record
                        .review_archive_scheduled
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
                {
                    continue;
                }
                let Some(parent_id) = record.parent_id.as_deref() else {
                    continue;
                };
                let event_id =
                    ReviewerArchiveRepository::child_result_id(tx, parent_id, &child_id)?;
                let Some(event_id) = event_id else {
                    continue;
                };
                let mut child: AgentRecord = studio_json::from_str(&encoded).map_err(|_| {
                    studio_runtime_store::StoreError::InvalidStoredObject {
                        table: "runtime_agents",
                        id: child_id.to_owned(),
                    }
                })?;
                child
                    .extra
                    .insert("reviewArchiveScheduled".into(), json!(event_id));
                AgentRepository::put(tx, &child)?;
            }
            Ok(())
        })
        .await?;

    let due = store
        .read_async("runtime-services/work_write", move |connection| {
            Ok::<_, WorkWriteError>(ReviewerArchiveRepository::due_retries(connection, now)?)
        })
        .await?;

    for (child_id, encoded) in due {
        let record: ReviewerArchiveView =
            studio_json::from_str(&encoded).map_err(studio_runtime_store::StoreError::from)?;
        let Some(event_id) = record.review_archive_scheduled.as_deref() else {
            continue;
        };
        let Some(parent_id) = record.parent_id.as_deref() else {
            continue;
        };
        let Some(root_id) = record.root_id.as_deref() else {
            continue;
        };
        if record.role.as_deref() != Some("reviewer")
            || record.status.as_deref() != Some("completed")
            || raw_truthy(record.agent_archive.as_ref())
        {
            continue;
        }
        let event_id_for_read = event_id.to_owned();
        let parent_id_for_read = parent_id.to_owned();
        let child_id_for_read = child_id.clone();
        let event_matches = store
            .read_async("runtime-services/work_write", move |connection| {
                let event: Option<(String, String)> = connection
                    .query_row(
                        "SELECT agent,text FROM runtime_events \
                     WHERE id=?1 AND kind='child_result' AND status='delivered'",
                        [event_id_for_read],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .map_err(studio_runtime_store::StoreError::from)?;
                Ok::<_, WorkWriteError>(event.is_some_and(|(event_parent, text)| {
                    event_parent == parent_id_for_read
                        && studio_json::from_str::<ChildResultIdentity>(&text)
                            .ok()
                            .and_then(|payload| payload.agent_id)
                            .as_deref()
                            == Some(child_id_for_read.as_str())
                }))
            })
            .await?;
        if !event_matches {
            continue;
        }

        let epoch_root = AgentId::from(root_id);
        let actor_epoch = store
            .read_async("runtime-services/work_write", move |connection| {
                AgentRepository::get(connection, &epoch_root)
            })
            .await?
            .map(|agent| agent.epoch)
            .unwrap_or_default();
        let outcome = crate::agent_management_store::execute(
            Arc::clone(&store),
            &crate::agent_management::Action::Archive {
                actor: root_id.to_owned(),
                epoch: actor_epoch,
                target: child_id.clone(),
                reason: "Reviewer result delivered to parent".to_owned(),
                unassign_work: false,
            },
            now,
            None,
        )
        .await;
        let (error, retryable) = match outcome {
            Ok(value) => {
                let outcome: ReviewerArchiveOutcome = studio_json::from_str(&value.to_string())
                    .map_err(studio_runtime_store::StoreError::from)?;
                if outcome.status.as_deref() == Some("archived") {
                    continue;
                }
                let blockers = outcome
                    .blockers
                    .as_ref()
                    .and_then(|raw| studio_json::from_str::<Vec<RawJson>>(raw.0.as_str()).ok());
                let retryable = blockers.as_ref().is_some_and(|items| !items.is_empty());
                let error =
                    if let Some(blockers) = blockers.as_ref().filter(|items| !items.is_empty()) {
                        Some(
                            studio_json::to_string(blockers)
                                .map_err(studio_runtime_store::StoreError::from)?,
                        )
                    } else {
                        Some(
                            outcome
                                .status
                                .unwrap_or_else(|| "Archive was not completed".to_owned()),
                        )
                    };
                (error, retryable)
            }
            Err(error) => (Some(error.to_string()), false),
        };
        let Some(error) = error else { continue };
        let child_for_settlement = child_id.clone();
        let event_for_settlement = event_id.to_owned();
        store
            .transaction_with_error(move |tx| -> Result<(), WorkWriteError> {
                let Some(mut child) =
                    AgentRepository::get(tx, &AgentId::from(child_for_settlement.as_str()))?
                else {
                    return Ok(());
                };
                if child
                    .extra
                    .get("reviewArchiveScheduled")
                    .and_then(Value::as_str)
                    != Some(event_for_settlement.as_str())
                {
                    return Ok(());
                }
                child.extra.insert(
                    "reviewArchiveError".into(),
                    json!(error.chars().take(180).collect::<String>()),
                );
                if retryable {
                    let attempts = child
                        .extra
                        .get("reviewArchiveAttempts")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                        .saturating_add(1);
                    let exponent = attempts
                        .saturating_sub(1)
                        .min(u64::from(REVIEW_ARCHIVE_MAX_BACKOFF_EXPONENT));
                    let delay = (REVIEW_ARCHIVE_BASE_DELAY_SECONDS * 2_f64.powi(exponent as i32))
                        .min(REVIEW_ARCHIVE_MAX_DELAY_SECONDS);
                    child
                        .extra
                        .insert("reviewArchiveAttempts".into(), json!(attempts));
                    child
                        .extra
                        .insert("reviewArchiveNextAt".into(), json!(now + delay));
                } else {
                    child.extra.remove("reviewArchiveScheduled");
                }
                AgentRepository::put(tx, &child)?;
                Ok(())
            })
            .await?;
    }
    Ok(())
}

/// Prefix of the error that a Rust lifecycle dependency writes when an archive
/// was refused because Rust did not implement a step yet ("Not implemented:
/// requires <dependency>"). Python never wrote it.
const NOT_IMPLEMENTED_REFUSAL: &str = "Not implemented: requires ";

/// Once per store and process, give each reviewer whose archive was refused by
/// a missing Rust dependency its schedule back. Python keeps such an error final
/// because only a real blocker or failure writes it; these refusals came from
/// Rust gaps that a later build closes. A reviewer refused again keeps the new
/// error until the next process start.
async fn retry_refused_reviewer_archives(store: &Arc<RuntimeStore>) -> Result<(), WorkWriteError> {
    static SWEPT: std::sync::OnceLock<
        std::sync::Mutex<std::collections::BTreeSet<std::path::PathBuf>>,
    > = std::sync::OnceLock::new();
    let root = store.root().to_path_buf();
    {
        let mut swept = SWEPT
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !swept.insert(root) {
            return Ok(());
        }
    }
    store
        .transaction_with_error(move |tx| -> Result<(), WorkWriteError> {
            let mut statement = tx
                .prepare(
                    "SELECT id,record FROM runtime_agents \
                     WHERE json_extract(record,'$.role')='reviewer' \
                       AND json_extract(record,'$.status')='completed' \
                       AND substr(json_extract(record,'$.reviewArchiveError'),1,?1)=?2",
                )
                .map_err(studio_runtime_store::StoreError::from)?;
            let rows = statement
                .query_map(
                    rusqlite::params![
                        NOT_IMPLEMENTED_REFUSAL.chars().count() as i64,
                        NOT_IMPLEMENTED_REFUSAL
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(studio_runtime_store::StoreError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(studio_runtime_store::StoreError::from)?;
            drop(statement);
            for (id, raw) in rows {
                let record: ReviewerArchiveView =
                    studio_json::from_str(&raw).map_err(studio_runtime_store::StoreError::from)?;
                if raw_truthy(record.agent_archive.as_ref()) {
                    continue;
                }
                let mut agent: AgentRecord = studio_json::from_str(&raw).map_err(|_| {
                    studio_runtime_store::StoreError::InvalidStoredObject {
                        table: "runtime_agents",
                        id: id.clone(),
                    }
                })?;
                for key in [
                    "reviewArchiveError",
                    "reviewArchiveScheduled",
                    "reviewArchiveAttempts",
                    "reviewArchiveNextAt",
                ] {
                    agent.extra.remove(key);
                }
                AgentRepository::put(tx, &agent)?;
            }
            Ok(())
        })
        .await
}

/// Find the next durable archive retry deadline. New reviewer candidates are
/// discovered on startup and after commits; only persisted retries need a timer.
pub async fn next_archive_retry_at(
    store: &RuntimeStore,
    now: f64,
    accepted_batch_full: bool,
) -> Result<Option<f64>, WorkWriteError> {
    if accepted_batch_full {
        return Ok(Some(now));
    }
    store
        .read_async("runtime-services/work_write", move |connection| {
            let accepted: Option<f64> = connection
                .query_row(
                    "SELECT archive_next_at FROM runtime_work WHERE archive_status IN ('pending','retryable','unknown') AND json_type(record,'$.archiveIntent.nextAttemptAt') IN ('integer','real') AND archive_next_at>?1 ORDER BY archive_next_at,id LIMIT 1",
                    [now],
                    |row| row.get(0),
                )
                .optional()
                .map_err(studio_runtime_store::StoreError::from)?;
            let reviewer = ReviewerArchiveRepository::next_retry_at(connection, now)?;
            Ok::<_, WorkWriteError>(accepted.into_iter().chain(reviewer).reduce(f64::min))
        })
        .await
}

struct ArchiveGuard {
    work: WorkItemRecord,
    owner: Box<AgentRecord>,
    task_id: String,
    owner_id: String,
    root_id: String,
    intent: ArchiveIntent,
    actor_epoch: i64,
}

impl ArchiveGuard {
    fn check(&self, tx: &Transaction<'_>) -> Result<(), crate::agent_management::ManagementError> {
        let changed = || {
            crate::agent_management::ManagementError::Invalid(
                "The accepted task owner or result changed",
            )
        };
        let current = WorkItemsRepository::work_record(tx, &self.task_id)
            .map_err(|_| changed())?
            .ok_or_else(changed)?;
        let owner = AgentRepository::get(tx, &AgentId::from(self.owner_id.as_str()))
            .map_err(|_| changed())?
            .ok_or_else(changed)?;
        if current.status() != WorkStatus::Accepted
            || current.version() != self.work.version()
            || current.results() != self.work.results()
            || owner != *self.owner
            || accepted_archive_preflight(
                tx,
                &current,
                result_identity(&current).as_ref(),
                &self.intent,
                &self.owner_id,
                &self.root_id,
                self.actor_epoch,
            )
            .map_err(|_| changed())?
            .is_some()
        {
            return Err(changed());
        }
        Ok(())
    }
}

/// Reconcile the target and accepted result before replaying a lifecycle archive.
/// A canonical archive marker proves the prior effect committed; cleanupPending
/// or a conflicting marker is held without repeating cleanup effects.
fn accepted_archive_preflight(
    connection: &Connection,
    work: &WorkItemRecord,
    result: Option<&ResultIdentity>,
    intent: &ArchiveIntent,
    owner_id: &str,
    root_id: &str,
    actor_epoch: i64,
) -> Result<Option<Value>, WorkWriteError> {
    let task_id = work.id().as_ref();
    let Some(actor) = AgentRepository::get(connection, &AgentId::from(root_id))? else {
        return Ok(Some(kept("The archive actor changed")));
    };
    if actor.epoch != actor_epoch || actor.deleted_at.is_some() {
        return Ok(Some(kept("The archive actor changed")));
    }
    let Some(owner) = AgentRepository::get(connection, &AgentId::from(owner_id))? else {
        return Ok(Some(kept("The accepted task owner changed")));
    };
    let owner_value = owner.value();
    if owner_value
        .get("remoteWorker")
        .is_some_and(|value| event_write::truthy(Some(value)))
    {
        return Ok(Some(kept(
            "The remote worker workspace requires review on its own server",
        )));
    }
    let Some(result) = result else {
        return Ok(Some(kept("The accepted task result is missing")));
    };
    let Some(result_id) = result.id.as_deref() else {
        return Ok(Some(kept("The accepted task result is missing")));
    };
    if intent.intent_id != format!("{task_id}:{result_id}")
        || work.owner() != Some(owner_id)
        || work.root_id() != Some(root_id)
    {
        return Ok(Some(kept("The accepted task owner or result changed")));
    }

    if let Some(archive_marker) = &owner.agent_archive {
        let canonical = archive_marker.by.as_deref() == Some(root_id)
            && archive_marker.reason.as_deref() == Some("Accepted task result is on main")
            && i64::try_from(archive_marker.epoch).ok() == Some(owner.epoch)
            && Some(archive_marker.at) == owner.deleted_at
            && archive_marker.at >= intent.created
            && archive_marker.cleanup_pending == Some(false);
        if !canonical {
            return Ok(Some(studio_runtime_store::projection_value(&KeptOutcome {
                cleanup_pending: Some(archive_marker.cleanup_pending.unwrap_or(false)),
                ..kept_outcome("The accepted archive outcome is unconfirmed")
            })));
        }
        let cleaned = owner.cleaned_worktree.as_ref();
        let worktree = if cleaned.is_some_and(|value| value.missing == Some(true)) {
            ArchiveWorktree {
                state: ArchiveWorktreeState::Missing,
                reason: cleaned
                    .and_then(|v| v.note.as_deref())
                    .map_or(Field::Null, Field::Value),
                bytes: Some(0),
            }
        } else if let Some(cleaned) = cleaned {
            ArchiveWorktree {
                state: ArchiveWorktreeState::Removed,
                reason: Field::Missing,
                bytes: cleaned.bytes,
            }
        } else if owner.worktree_ready == Some(true) {
            ArchiveWorktree {
                state: ArchiveWorktreeState::Kept,
                reason: Field::Value("The archived worktree remains"),
                bytes: Some(0),
            }
        } else {
            ArchiveWorktree {
                state: ArchiveWorktreeState::None,
                reason: Field::Missing,
                bytes: Some(0),
            }
        };
        return Ok(Some(studio_runtime_store::projection_value(
            &ArchivedOutcome {
                status: "archived",
                worktree,
            },
        )));
    }

    let mut statement = connection
        .prepare(
            "SELECT id FROM runtime_work WHERE json_extract(record,'$.owner')=?1 \
             AND id<>?2 AND json_extract(record,'$.status') NOT IN ('accepted','cancelled') \
             ORDER BY id LIMIT 20",
        )
        .map_err(studio_runtime_store::StoreError::from)?;
    let open_tasks = statement
        .query_map([owner_id, task_id], |row| row.get::<_, String>(0))
        .map_err(studio_runtime_store::StoreError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(studio_runtime_store::StoreError::from)?;
    if !open_tasks.is_empty() {
        return Ok(Some(studio_runtime_store::projection_value(&KeptOutcome {
            retryable: Some(true),
            tasks: Some(&open_tasks),
            ..kept_outcome("The owner has another open task")
        })));
    }

    if layr_result_needs_merge(root_id, &owner)
        && !layr_merge_confirmed(
            connection,
            task_id,
            result_id,
            result.revision.as_deref().unwrap_or_default(),
        )?
    {
        return Ok(Some(kept("The layr result merge is not confirmed")));
    }
    Ok(None)
}

fn accepted_archive_due_records(
    connection: &Connection,
    now: f64,
) -> Result<Vec<(String, String)>, WorkWriteError> {
    let mut statement = connection
        .prepare(
            "SELECT id,json_extract(record,'$.archiveIntent') FROM runtime_work WHERE archive_status IN ('pending','retryable','unknown') AND archive_next_at<=?1 ORDER BY archive_next_at,id LIMIT ?2",
        )
        .map_err(studio_runtime_store::StoreError::from)?;
    Ok(statement
        .query_map(
            rusqlite::params![now, ACCEPTED_ARCHIVE_PAGE_SIZE as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(studio_runtime_store::StoreError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(studio_runtime_store::StoreError::from)?)
}

pub struct CreateWork {
    pub gate: Field<studio_runtime_schema::task_gate::GateInput>,
    pub actor_id: String,
    pub title: String,
    pub description: String,
    pub owner: Option<String>,
    pub status: Option<studio_runtime_schema::task_command::InitialStatus>,
    pub dependencies: Vec<String>,
    pub result_schema: Option<Value>,
}

pub enum WorkActorWrite {
    Create(CreateWork),
    Mutate(WorkMutation),
}

pub enum WorkMutation {
    Update {
        gate: Field<studio_runtime_schema::task_gate::GateInput>,
        task_id: String,
        version: Option<i64>,
        title: Option<String>,
        description: Option<String>,
        owner: Option<Option<String>>,
        status: Option<studio_runtime_schema::task_command::InitialStatus>,
        dependencies: Option<Vec<String>>,
        result_schema: Option<Value>,
    },
    Reject {
        task_id: String,
        version: Option<i64>,
        reason: String,
    },
    Cancel {
        task_id: String,
        version: Option<i64>,
        reason: String,
    },
}

#[derive(Clone)]
pub struct SubmitWork {
    pub prepared_gate_workspace:
        Option<Arc<dyn crate::monitor_runtime::WorkspaceAdmission + Send + Sync>>,
    pub task_id: String,
    pub version: Option<i64>,
    pub result: Value,
    pub checks: String,
    pub revision: String,
    pub files: Vec<String>,
    pub prepared_files: Option<PreparedSubmissionFiles>,
    pub state_dir: PathBuf,
    pub layr_evidence: Option<studio_runtime_store::work_intent::Evidence>,
    pub layr_evidence_request_id: Option<String>,
}

/// File validation bound to the exact stored owner/cwd and submitted list.
#[derive(Clone)]
pub struct PreparedSubmissionFiles {
    owner: String,
    cwd: Option<String>,
    files: Vec<String>,
    resolved: Option<Vec<CanonicalPath>>,
}

/// Snapshot SQL ownership first, then resolve paths outside every SQL callback.
/// Validation failures stay in the evidence until after the writer's replay check.
pub async fn prepare_submission_files(
    store: &RuntimeStore,
    agent_id: &str,
    input: &SubmitWork,
) -> Result<Option<PreparedSubmissionFiles>, WorkWriteError> {
    if input.files.is_empty() {
        return Ok(None);
    }
    let agent_id = agent_id.to_owned();
    let task_id = input.task_id.clone();
    let owner = store
        .read_async("runtime-services/work-files/sql", move |connection| {
            let Some(agent) = AgentRepository::get(connection, &AgentId::from(agent_id.as_str()))?
            else {
                return Ok::<_, WorkWriteError>(None);
            };
            let work = WorkItemsRepository::work_record(connection, &task_id)?;
            let owner = work
                .as_ref()
                .and_then(|work| work.owner())
                .unwrap_or(&agent.id.0);
            Ok(AgentRepository::get(connection, &AgentId::from(owner))?)
        })
        .await?;
    let files = input.files.clone();
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/work-files/filesystem",
        move || {
            owner.map(|owner| {
                let resolved = resolve_submission_files(&owner, &files).ok();
                PreparedSubmissionFiles {
                    owner: owner.id.0,
                    cwd: owner.cwd.clone(),
                    files,
                    resolved,
                }
            })
        },
    )
    .await
    .map_err(|_| WorkWriteError::PreparationWorker)
}

pub struct LayrSubmissionCall {
    pub owner_id: String,
    pub revision: String,
    pub request_id: String,
}

use studio_runtime_store::work_intent::{
    Acceptance, AcceptanceResult, Intent as LayrWorkIntent, Owner as LayrOwner,
    PublicRequest as LayrPublic, Submission, SubmissionResult as LayrSubmissionResult,
    View as LayrView,
};

fn read_layr_view(connection: &Connection, task_id: &str) -> Result<LayrView, WorkWriteError> {
    WorkItemsRepository::work_record(connection, task_id)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?;
    let raw: String = connection
        .query_row(
            "SELECT record FROM runtime_work WHERE id=?1",
            [task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(studio_runtime_store::StoreError::from)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?;
    Ok(studio_json::from_str(&raw).map_err(studio_runtime_store::StoreError::from)?)
}
fn write_layr_view(
    tx: &Transaction<'_>,
    task_id: &str,
    view: &LayrView,
) -> Result<(), WorkWriteError> {
    let mut work = WorkItemsRepository::work_record(tx, task_id)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?;
    let fields = studio_runtime_store::projection_value(view);
    for (field, value) in fields
        .as_object()
        .ok_or_else(|| invalid("runtime_work", task_id))?
    {
        work.set(field, value.clone())
            .map_err(studio_runtime_store::StoreError::from)?;
    }
    WorkItemsRepository::update_checked_work(tx, task_id, &work).map_err(Into::into)
}

pub enum LayrSubmissionPreparation {
    NotLayr,
    Replay,
    Evidence {
        evidence: studio_runtime_store::work_intent::Evidence,
        request_id: String,
    },
    Call(LayrSubmissionCall),
}

fn reviewer_without_revision(owner: &AgentRecord, revision: &str) -> bool {
    owner.role.value().copied() == Some(studio_runtime_schema::creation_intent::AgentRole::Reviewer)
        && revision.trim().is_empty()
}

/// Persist an exact layr evidence request before contacting the guest. A pending
/// or unknown request is never sent again; completed evidence can be replayed.
pub async fn prepare_layr_submission(
    store: &RuntimeStore,
    agent_id: &str,
    request_id: Option<&str>,
    signature: &str,
    input: &mut SubmitWork,
) -> Result<LayrSubmissionPreparation, WorkWriteError> {
    if read_work_receipt(
        store,
        agent_id.to_owned(),
        request_id.map(str::to_owned),
        signature.to_owned(),
    )
    .await?
    .is_some()
    {
        return Ok(LayrSubmissionPreparation::Replay);
    }
    input.prepared_files = prepare_submission_files(store, agent_id, input).await?;
    let agent_id = agent_id.to_owned();
    let request_id = request_id.map(str::to_owned);
    let signature = signature.to_owned();
    let input = input.clone();
    store.transaction_with_error(move |tx| {
        let agent = event_write::checked_agent(tx, &agent_id)?;
        if let Some(root) = request_id.as_deref()
            && work_receipt_in(tx, &agent_id, root, &signature)?.is_some() {
            return Ok(LayrSubmissionPreparation::Replay);
        }
        let work = load_work(tx, &agent.root_id.0, &input.task_id, input.version)?;
        if !matches!(work.status(), WorkStatus::Running | WorkStatus::Ready | WorkStatus::Blocked | WorkStatus::Review) {
            return Err(Error::InvalidRequest("This result is already accepted").into());
        }
        if !work.blocked_by(&dependency_statuses(tx, &work)?).is_empty() {
            return Err(Error::InvalidRequest("Dependencies have not been accepted").into());
        }
        let owner_id = work.owner().unwrap_or(&agent.id.0);
        let owner = event_write::checked_agent(tx, owner_id)?;
        // Invalid results reach submit's durable refusal before any workspace operation.
        if result_refusal(tx, &work, &owner, &input)?.is_some() {
            return Ok(LayrSubmissionPreparation::NotLayr);
        }
        if work.extension("resultSchema").is_none() {
            text(input.result.as_str().ok_or(Error::InvalidRequest("Supply result as text when the task has no result_schema"))?, false)?;
        }
        text(&input.checks, false)?;
        if reviewer_without_revision(&owner, &input.revision) {
            return Ok(LayrSubmissionPreparation::NotLayr);
        }
        let revision = python_bounded_text(&input.revision, 200)?;
        validate_submission_files(&owner, &input.files, input.prepared_files.as_ref())?;
        if owner.workspace_mode != Some(studio_runtime_schema::records::WorkspaceMode::LayrNative) {
            return Ok(LayrSubmissionPreparation::NotLayr);
        }
        let rpc_id = format!("layr-evidence:{owner_id}:{revision}");
        let mut view = read_layr_view(tx, &input.task_id)?;
        match &view.submit {
            Some(LayrWorkIntent::Pending(..) | LayrWorkIntent::Unknown(..) | LayrWorkIntent::UnknownUnattributed(..)) => return Err(Error::Conflict("A prior layr evidence request has an unknown outcome; inspect it before retrying").into()),
            Some(LayrWorkIntent::Completed { owner, result }) if owner.request_id==rpc_id && owner.owner==owner_id && Some(owner.id.version)==work.version().and_then(studio_json::Number::as_i64) && owner.revision==revision => return Ok(LayrSubmissionPreparation::Evidence{evidence:result.evidence.clone(),request_id:rpc_id}),
            Some(LayrWorkIntent::Completed { .. } | LayrWorkIntent::NotSent(..)) | None => {}
        }
        view.submit = Some(LayrWorkIntent::Pending(LayrOwner {request_id:rpc_id.clone(),owner:owner_id.into(),revision:revision.clone(),id:Submission { version:work.version().and_then(studio_json::Number::as_i64).ok_or_else(|| invalid("runtime_work", &input.task_id))? },public:None}));
        if let Some(root) = request_id.as_deref()
            && admit_layr_work_request(tx, &agent, root, &input.task_id, &signature, event_write::unix_time()?)?.is_none() {
            return Ok(LayrSubmissionPreparation::Replay);
        }
        write_layr_view(tx, &input.task_id, &view)?;
        Ok(LayrSubmissionPreparation::Call(LayrSubmissionCall {
            owner_id: owner_id.to_owned(),
            revision,
            request_id: rpc_id,
        }))
    }).await
}

/// Record the guest's terminal response before the normal work submission write.
/// A lost response is persisted as unknown and can never trigger an automatic resend.
pub async fn finish_layr_submission(
    store: &RuntimeStore,
    task_id: &str,
    request_id: &str,
    outcome: Result<studio_runtime_store::work_intent::Evidence, bool>,
) -> Result<Option<studio_runtime_store::work_intent::Evidence>, WorkWriteError> {
    let outcome = outcome.map(|evidence| LayrSubmissionResult { evidence });
    finish_layr_intent(store, task_id, request_id, outcome, |view| &mut view.submit)
        .await
        .map(|result| result.map(|result| result.evidence))
}

async fn finish_layr_intent<I: Clone + Send + 'static, R: Clone + PartialEq + Send + 'static>(
    store: &RuntimeStore,
    task_id: &str,
    request_id: &str,
    outcome: Result<R, bool>,
    select: for<'a> fn(&'a mut LayrView) -> &'a mut Option<LayrWorkIntent<I, R>>,
) -> Result<Option<R>, WorkWriteError> {
    let task_id = task_id.to_owned();
    let request_id = request_id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let mut view = read_layr_view(tx, &task_id)?;
            let intent = select(&mut view)
                .take()
                .ok_or(Error::Conflict("The layr intent changed"))?;
            let owner = match intent {
                LayrWorkIntent::Completed { owner, result } if owner.request_id == request_id => {
                    if outcome.as_ref().is_ok_and(|value| value != &result) {
                        return Err(Error::Conflict("The layr result changed").into());
                    }
                    return Ok(Some(result));
                }
                LayrWorkIntent::Pending(owner)
                | LayrWorkIntent::Unknown(owner)
                | LayrWorkIntent::NotSent(owner)
                    if owner.request_id == request_id =>
                {
                    owner
                }
                LayrWorkIntent::Pending(..)
                | LayrWorkIntent::Unknown(..)
                | LayrWorkIntent::NotSent(..)
                | LayrWorkIntent::Completed { .. }
                | LayrWorkIntent::UnknownUnattributed(..) => {
                    return Err(Error::Conflict("The layr intent changed").into());
                }
            };
            let (intent, result) = match outcome {
                Ok(result) => (
                    LayrWorkIntent::Completed {
                        owner,
                        result: result.clone(),
                    },
                    Some(result),
                ),
                Err(true) => (LayrWorkIntent::Unknown(owner), None),
                Err(false) => (LayrWorkIntent::NotSent(owner), None),
            };
            *select(&mut view) = Some(intent);
            write_layr_view(tx, &task_id, &view)?;
            Ok(result)
        })
        .await
}

pub struct ClaimWork {
    pub task_id: String,
    pub version: Option<i64>,
    pub claimant_id: String,
}

pub struct AcceptWork {
    pub override_reason: Option<String>,
    pub task_id: String,
    pub version: Option<i64>,
    pub reason: String,
    pub merged_result_id: Option<String>,
}

pub struct LayrMergeCall {
    pub lead_id: String,
    pub owner_id: String,
    pub result_id: String,
    pub revision: String,
    pub request_id: String,
}

pub enum LayrAcceptPreparation {
    NotLayr,
    Merged(String),
    Call(LayrMergeCall),
}

/// Whether accepting a task result merges the owner's layr line into main. A result of the
/// lead is in main already. A reviewer's line is read-only and has no changes by design; the
/// reviewer is not a merge source.
fn layr_result_needs_merge(lead_id: &str, owner: &AgentRecord) -> bool {
    owner.id.0 != lead_id
        && owner.workspace_mode == Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
        && owner.role.value().copied()
            != Some(studio_runtime_schema::creation_intent::AgentRole::Reviewer)
}

pub async fn prepare_layr_accept(
    store: &RuntimeStore,
    agent_id: &str,
    request_id: Option<&str>,
    signature: &str,
    input: &AcceptWork,
) -> Result<LayrAcceptPreparation, WorkWriteError> {
    let agent_id = agent_id.to_owned();
    let task_id = input.task_id.clone();
    let version = input.version;
    let request_id = request_id.map(str::to_owned);
    let signature = signature.to_owned();
    let reason = input.reason.clone();
    let override_reason = input.override_reason.clone();
    store
        .transaction_with_error(move |tx| {
            let lead = event_write::checked_agent(tx, &agent_id)?;
            if lead.workspace_mode
                != Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
            {
                return Ok(LayrAcceptPreparation::NotLayr);
            }
            if lead.is_lead != Some(true) || lead.root_id.0 != agent_id {
                return Err(
                    Error::InvalidRequest("Only the active lead can accept a layr result").into(),
                );
            }
            if let Some(root) = request_id.as_deref()
                && let Some(saved) = pending_work_request(tx, &lead, root, &signature)?
            {
                if saved.task != task_id {
                    return Err(Error::Conflict("This request id has different content").into());
                }
                if saved.archive {
                    return Ok(LayrAcceptPreparation::NotLayr);
                }
            }
            let work = load_work(tx, &lead.root_id.0, &task_id, version)?;
            if work.status() != studio_runtime_store::WorkStatus::Review {
                return Ok(LayrAcceptPreparation::NotLayr);
            }
            let Some(owner_id) = work.owner().map(str::to_owned) else {
                return Ok(LayrAcceptPreparation::NotLayr);
            };
            let owner = event_write::checked_agent(tx, &owner_id)?;
            if !layr_result_needs_merge(&lead.id.0, &owner) {
                return Ok(LayrAcceptPreparation::NotLayr);
            }
            // Validate the acceptance and all dependent recipients before the guest
            // merge. Python's WorkMixin refuses unsupported remote recipients while
            // applying accept; the merge must not happen before that refusal.
            let statuses = dependency_statuses(tx, &work)?;
            let now = event_write::unix_time()?;
            decide_accept(
                WorkSnapshot {
                    record: &work,
                    dependency_statuses: &statuses,
                },
                &reason,
                &agent_id,
                now,
                override_reason.as_deref(),
            )?;
            for dependent in WorkItemsRepository::dependent_work(tx, &lead.root_id.0, &task_id)? {
                let mut dependent_statuses = dependency_statuses(tx, &dependent)?;
                dependent_statuses.insert(task_id.clone(), WorkStatus::Accepted);
                if dependent.blocked_by(&dependent_statuses).is_empty()
                    && let Some(dependent_owner) = dependent.owner()
                {
                    let recipient = AgentRepository::get(tx, &AgentId::from(dependent_owner))?
                        .ok_or_else(|| invalid("runtime_agents", dependent_owner))?;
                    event_write::refuse_remote(&recipient)?;
                }
            }
            if lead.layr_project_id != owner.layr_project_id {
                return Err(
                    Error::InvalidRequest("The result belongs to another layr project").into(),
                );
            }
            let result = work
                .results()
                .and_then(|items| items.last())
                .ok_or_else(|| invalid("runtime_work", &task_id))?;
            let result =
                <ResultIdentity as serde::Deserialize>::deserialize(&Value::Object(result.clone()))
                    .map_err(|_| invalid("runtime_work", &task_id))?;
            let result_id = result.id.ok_or_else(|| invalid("runtime_work", &task_id))?;
            let revision = result
                .revision
                .ok_or_else(|| invalid("runtime_work", &task_id))?;
            let rpc_id = format!("layr-accept:{result_id}");
            let mut view = read_layr_view(tx, &task_id)?;
            match &view.accept {
                Some(
                    LayrWorkIntent::Pending(..)
                    | LayrWorkIntent::Unknown(..)
                    | LayrWorkIntent::UnknownUnattributed(..),
                ) => {
                    return Err(Error::Conflict(
                        "The prior layr merge outcome is unknown; inspect it before retrying",
                    )
                    .into());
                }
                Some(LayrWorkIntent::Completed { owner, .. }) if owner.request_id == rpc_id => {
                    return Ok(LayrAcceptPreparation::Merged(result_id.to_owned()));
                }
                Some(LayrWorkIntent::Completed { .. } | LayrWorkIntent::NotSent(..)) | None => {}
            }
            let mut owner = LayrOwner {
                request_id: rpc_id.clone(),
                owner: owner_id.clone(),
                revision: revision.clone(),
                id: Acceptance {
                    result_id: result_id.clone(),
                },
                public: None,
            };
            if let Some(root) = request_id.as_deref() {
                let token = admit_layr_work_request(tx, &lead, root, &task_id, &signature, now)?
                    .ok_or(Error::Conflict("The acceptance already completed"))?;
                owner.public = Some(LayrPublic {
                    public_admission: token,
                    public_request_id: root.into(),
                });
            }
            view.accept = Some(LayrWorkIntent::Pending(owner));
            write_layr_view(tx, &task_id, &view)?;
            Ok(LayrAcceptPreparation::Call(LayrMergeCall {
                lead_id: lead.id.0,
                owner_id,
                result_id,
                revision,
                request_id: rpc_id,
            }))
        })
        .await
}

pub async fn finish_layr_accept(
    store: &RuntimeStore,
    task_id: &str,
    request_id: &str,
    uncertain: Option<bool>,
    merge: Option<studio_runtime_store::work_intent::MergeReceipt>,
) -> Result<(), WorkWriteError> {
    let outcome = uncertain.map_or(Ok(AcceptanceResult { merge }), Err);
    finish_layr_intent(store, task_id, request_id, outcome, |view| &mut view.accept)
        .await
        .map(|_| ())
}

/// Verify the exact accepted result has a durable successful guest merge.
pub fn layr_merge_confirmed(
    connection: &Connection,
    task_id: &str,
    result_id: &str,
    revision: &str,
) -> Result<bool, WorkWriteError> {
    Ok(matches!(read_layr_view(connection, task_id)?.accept,
        Some(LayrWorkIntent::Completed { owner, .. }) if owner.id.result_id == result_id && owner.revision == revision))
}

pub enum WorkWrite {
    Create(CreateWork),
    Mutate(WorkMutation),
    Claim(ClaimWork),
    Submit(SubmitWork),
    Accept(AcceptWork),
}

/// Convert the shared ingress command once. Filesystem preparation remains
/// owned by the HTTP driver; authenticated actor selection stays with the caller.
pub fn task_command<const DESCRIPTION_MAX: usize, const CANCEL_MAX: usize>(
    command: studio_runtime_schema::task_command::TaskCommand<DESCRIPTION_MAX, CANCEL_MAX>,
    actor_id: String,
    state_dir: PathBuf,
) -> Result<WorkWrite, Error> {
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_schema::task_command::TaskCommand;
    let provided = |field: Field<Value>| match field {
        Field::Missing => None,
        Field::Null => Some(Value::Null),
        Field::Value(value) => Some(value),
    };
    Ok(match command {
        TaskCommand::Create {
            title,
            description,
            owner,
            status,
            dependencies,
            result_schema,
            gate,
        } => WorkWrite::Create(CreateWork {
            actor_id,
            title: title.as_str().to_owned(),
            description: description
                .map(|value| value.as_str().to_owned())
                .unwrap_or_default(),
            owner,
            status,
            dependencies: dependencies.as_slice().to_vec(),
            result_schema: provided(result_schema),
            gate,
        }),
        TaskCommand::Update {
            task_id,
            version,
            title,
            description,
            owner,
            status,
            dependencies,
            result_schema,
            gate,
        } => WorkWrite::Mutate(WorkMutation::Update {
            task_id,
            version: version.map(|value| value.get()),
            title: title.map(|value| value.as_str().to_owned()),
            description: description.map(|value| value.as_str().to_owned()),
            owner: match owner {
                Field::Missing => None,
                Field::Null => Some(None),
                Field::Value(value) => Some(Some(value)),
            },
            status,
            dependencies: dependencies.map(|value| value.as_slice().to_vec()),
            result_schema: provided(result_schema),
            gate,
        }),
        TaskCommand::Cancel {
            task_id,
            version,
            reason,
        } => WorkWrite::Mutate(WorkMutation::Cancel {
            task_id,
            version: version.map(|value| value.get()),
            reason: reason.as_str().to_owned(),
        }),
        TaskCommand::Reject {
            task_id,
            version,
            result,
        } => WorkWrite::Mutate(WorkMutation::Reject {
            task_id,
            version: version.map(|value| value.get()),
            reason: result.as_str().to_owned(),
        }),
        TaskCommand::Claim {
            task_id,
            version,
            owner,
        } => WorkWrite::Claim(ClaimWork {
            task_id,
            version: version.map(|value| value.get()),
            claimant_id: owner.unwrap_or(actor_id),
        }),
        TaskCommand::Submit {
            task_id,
            version,
            result,
            checks,
            revision,
            files,
        } => WorkWrite::Submit(SubmitWork {
            task_id,
            version: version.map(|value| value.get()),
            result,
            checks: checks.as_str().to_owned(),
            revision: revision.as_str().to_owned(),
            files: files.as_slice().to_vec(),
            state_dir,
            prepared_gate_workspace: None,
            prepared_files: None,
            layr_evidence: None,
            layr_evidence_request_id: None,
        }),
        TaskCommand::Accept {
            task_id,
            version,
            result,
            override_reason,
        } => WorkWrite::Accept(AcceptWork {
            task_id,
            version: version.map(|value| value.get()),
            reason: result.as_str().to_owned(),
            override_reason: override_reason.map(|value| value.as_str().to_owned()),
            merged_result_id: None,
        }),
        TaskCommand::List { .. } | TaskCommand::SetDefaultGate { .. } => {
            return Err(Error::InvalidRequest("Expected a task mutation"));
        }
    })
}

pub struct WorkWriteResult {
    pub record: Value,
    pub event_inserted: bool,
    pub artifact: Option<ResultArtifact>,
}

pub struct WorkActorResult {
    pub record: Value,
    pub event_inserted: bool,
}

pub struct ResultArtifact {
    state_dir: PathBuf,
    path: PathBuf,
    body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseFailedWorkResult {
    pub task_ids: Vec<String>,
    pub event_inserted: bool,
}

#[derive(Debug, Error)]
pub enum WorkWriteError {
    #[error("Request id expired")]
    Expired,
    #[error(transparent)]
    Event(#[from] Error),
    #[error(transparent)]
    Store(#[from] studio_runtime_store::StoreError),
    #[error("Not implemented: requires {0}")]
    NotImplemented(&'static str),
    #[error("work-write preparation blocking task failed")]
    PreparationWorker,
}

/// Read and validate an idempotency receipt before an external side effect.
pub async fn read_work_receipt(
    store: &RuntimeStore,
    agent_id: String,
    request_id: Option<String>,
    signature: String,
) -> Result<Option<Value>, WorkWriteError> {
    let Some(request_id) = request_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    store
        .read_async("runtime-services/work_write", move |connection| {
            work_receipt_in(connection, &agent_id, &request_id, &signature)
        })
        .await
}

fn work_receipt_in(
    connection: &Connection,
    agent_id: &str,
    request_id: &str,
    signature: &str,
) -> Result<Option<Value>, WorkWriteError> {
    let agent = AgentRepository::get(connection, &AgentId::from(agent_id))?
        .ok_or(Error::InvalidRequest("Invalid request"))?;
    if agent.id.0 != agent_id || agent.is_deleted() {
        return Err(Error::InvalidRequest("Invalid request").into());
    }
    let now_ms = (event_write::unix_time()? * 1000.0) as i64;
    let saved =
        event_write::replay_request::<LegacyWorkReply>(connection, request_id, signature, now_ms)
            .map_err(Error::from)?;
    if saved.is_none() && pending_work_request(connection, &agent, request_id, signature)?.is_some()
    {
        return Ok(None);
    }
    let saved = if saved.is_some() {
        saved
    } else {
        let identity =
            RequestIdentity::new("work", &[&agent.root_id.0], request_id, &[]).map_err(|_| {
                Error::RequestResult(
                    studio_runtime_store::request_results::RequestResultError::Expired,
                )
            })?;
        event_write::replay_request::<LegacyWorkReply>(connection, identity.id(), signature, now_ms)
            .map_err(Error::from)?
    };
    let Some(saved) = saved else {
        return Ok(None);
    };
    let reply = saved.reply;
    if reply.get("rootId").and_then(Value::as_str) != Some(agent.root_id.0.as_str()) {
        return Err(Error::InvalidRequest("Invalid request").into());
    }
    WorkItemsRepository::validate_work(connection, reply.id().as_ref(), reply.value())
        .map_err(WorkWriteError::Store)?;
    Ok(Some(reply.into_value()))
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct WorkReservation {
    root: String,
    agent: String,
    task: String,
    request: String,
    admitted: AdmittedRequest,
    archive: bool,
}

fn outbox_error(error: studio_runtime_store::outbox::OutboxError) -> Error {
    Error::Store(error.into())
}

fn pending_work_request(
    connection: &Connection,
    agent: &AgentRecord,
    root: &str,
    signature: &str,
) -> Result<Option<WorkReservation>, Error> {
    let rows = Outbox::for_request_kind::<WorkReservation>(connection, root, OutboxKind::WorkLayr)
        .map_err(outbox_error)?;
    for row in rows {
        if row.payload.root != agent.root_id.0 {
            continue;
        }
        if row.signature != signature || row.payload.agent != agent.id.0 {
            return Err(Error::Conflict("This request id has different content"));
        }
        return Ok(Some(row.payload));
    }
    Ok(None)
}

fn admit_layr_work_request(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    root: &str,
    task: &str,
    signature: &str,
    now: f64,
) -> Result<Option<AdmittedRequest>, Error> {
    let token = if let Some(saved) = pending_work_request(tx, agent, root, signature)? {
        if saved.task != task {
            return Err(Error::Conflict("This request id has different content"));
        }
        saved.admitted
    } else {
        match event_write::begin_request::<LegacyWorkReply>(
            tx,
            event_write::RequestKey::new("work", &[&agent.root_id.0], root),
            signature,
            (now * 1000.0) as i64,
        )
        .map_err(Error::from)?
        {
            event_write::CallerAdmission::Fresh(token) => token,
            event_write::CallerAdmission::Replay(_) => return Ok(None),
        }
    };
    reserve_work_request(tx, agent, root, task, &token, false)?;
    Ok(Some(token))
}

fn reserve_work_request(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    root: &str,
    task: &str,
    admitted: &AdmittedRequest,
    archive: bool,
) -> Result<(), Error> {
    let id = Outbox::effect_id(
        admitted.id(),
        OutboxKind::WorkLayr,
        OwnerKind::Task,
        task,
        &["public-request"],
    );
    Outbox::put(
        tx,
        &OutboxRow {
            id,
            kind: OutboxKind::WorkLayr,
            owner_kind: OwnerKind::Task,
            owner_id: task.into(),
            request_id: Some(root.into()),
            signature: admitted.signature().into(),
            status: OutboxStatus::Unknown,
            account: None,
            stream_id: None,
            generation: None,
            ordinal: None,
            next_at: None,
            payload: WorkReservation {
                root: agent.root_id.0.clone(),
                agent: agent.id.0.clone(),
                task: task.into(),
                request: root.into(),
                admitted: admitted.clone(),
                archive,
            },
            created: admitted.created(),
        },
    )
    .map_err(outbox_error)
}

fn clear_work_request(
    tx: &Transaction<'_>,
    root: &str,
    admitted: &AdmittedRequest,
) -> Result<(), Error> {
    // The request result and owner transition must commit before this obligation retires.
    let rows = Outbox::for_request_kind::<WorkReservation>(tx, root, OutboxKind::WorkLayr)
        .map_err(outbox_error)?;
    for mut row in rows {
        if row.payload.admitted.id() != admitted.id() {
            continue;
        }
        row.status = OutboxStatus::Committed;
        Outbox::put(tx, &row).map_err(outbox_error)?;
        Outbox::delete_settled(tx, &row.id, &row.signature).map_err(outbox_error)?;
    }
    Ok(())
}

/// Complete the synchronous archive caller using its already-admitted identity.
/// A late completion may settle after expiry; it never admits another effect.
pub fn settle_accepted_archive(
    tx: &Transaction<'_>,
    task: &str,
    request_root: &str,
    reply: &LegacyWorkReply,
) -> Result<LegacyWorkReply, WorkWriteError> {
    let mut work = WorkItemsRepository::work_record(tx, task)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?
        .into_value();
    if work["archiveIntent"]["publicRequestId"] != request_root {
        return Err(Error::Conflict("The acceptance request changed").into());
    }
    let (token, original): (String, String) = tx.query_row(
        "SELECT json_extract(record,'$.archiveIntent.publicAdmission'),json_extract(record,'$.archiveIntent.publicResponse') FROM runtime_work WHERE id=?1",
        [task], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(studio_runtime_store::StoreError::from)?;
    let token: AdmittedRequest =
        studio_json::from_str(&token).map_err(studio_runtime_store::StoreError::from)?;
    let mut original: LegacyWorkReply =
        studio_json::from_str(&original).map_err(studio_runtime_store::StoreError::from)?;
    original
        .set("archive", reply.value()["archive"].clone())
        .map_err(studio_runtime_store::StoreError::from)?;
    original
        .set("archivePending", reply.value()["archivePending"].clone())
        .map_err(studio_runtime_store::StoreError::from)?;
    event_write::settle_request(tx, &token, None, &original).map_err(Error::from)?;
    if let Some(intent) = work["archiveIntent"].as_object_mut() {
        intent.remove("publicAdmission");
        intent.remove("publicRequestId");
        intent.remove("publicResponse");
    }
    WorkItemsRepository::update_work(tx, task, &work)?;
    clear_work_request(tx, request_root, &token)?;
    Ok(original)
}

pub fn write(
    tx: &Transaction<'_>,
    agent_id: &str,
    request_id: Option<&str>,
    signature: &str,
    command: WorkWrite,
) -> Result<WorkWriteResult, WorkWriteError> {
    write_internal(tx, agent_id, None, request_id, signature, command)
}

fn write_internal(
    tx: &Transaction<'_>,
    agent_id: &str,
    actor_id: Option<&str>,
    request_id: Option<&str>,
    signature: &str,
    command: WorkWrite,
) -> Result<WorkWriteResult, WorkWriteError> {
    #[cfg(not(unix))]
    if matches!(&command, WorkWrite::Submit(_)) {
        return Err(WorkWriteError::NotImplemented(
            "workspace-state/submission-files",
        ));
    }

    let agent = event_write::checked_agent(tx, agent_id)?;
    let actor = if let Some(actor_id) = actor_id {
        let actor = event_write::checked_agent(tx, actor_id)?;
        if actor.root_id != agent.root_id {
            return Err(Error::InvalidRequest("Actor belongs to another team").into());
        }
        if actor.auto_wake != Some(true) {
            return Err(Error::InvalidRequest("The caller is stopped").into());
        }
        Some(actor)
    } else {
        None
    };
    let mut admitted = None;
    let mut replay = None;
    if let Some(root) = request_id.filter(|id| !id.is_empty()) {
        replay = work_receipt_in(tx, agent_id, root, signature)?;
        if replay.is_none() {
            if let Some(reservation) = pending_work_request(tx, &agent, root, signature)? {
                let task = match &command {
                    WorkWrite::Accept(input) => Some(input.task_id.as_str()),
                    WorkWrite::Submit(input) => Some(input.task_id.as_str()),
                    WorkWrite::Create(_) | WorkWrite::Mutate(_) | WorkWrite::Claim(_) => None,
                };
                if task != Some(reservation.task.as_str()) {
                    return Err(Error::Conflict("This request id has different content").into());
                }
                if reservation.archive {
                    return Err(Error::Conflict("The accepted archive outcome is pending").into());
                }
                admitted = Some(reservation.admitted);
            } else {
                match event_write::begin_request::<LegacyWorkReply>(
                    tx,
                    event_write::RequestKey::new("work", &[&agent.root_id.0], root),
                    signature,
                    (event_write::unix_time()? * 1000.0) as i64,
                )
                .map_err(Error::from)?
                {
                    event_write::CallerAdmission::Fresh(token) => admitted = Some(token),
                    event_write::CallerAdmission::Replay(saved) => {
                        replay = Some(saved.into_value())
                    }
                }
            }
        }
    }
    if let Some(record) = replay {
        return Ok(WorkWriteResult {
            artifact: if matches!(&command, WorkWrite::Submit(_))
                && record.get("submissionRefusal").is_none()
            {
                let WorkWrite::Submit(input) = &command else {
                    return Err(WorkWriteError::Event(Error::InvalidRequest(
                        "Invalid request",
                    )));
                };
                Some(result_artifact(&record, &input.state_dir)?)
            } else {
                None
            },
            record,
            event_inserted: false,
        });
    }
    let is_accept = matches!(&command, WorkWrite::Accept(_));
    let result = match command {
        WorkWrite::Create(input) => {
            if let Some(actor) = &actor
                && (actor.id != actor.root_id || agent.id != agent.root_id)
            {
                return Err(Error::InvalidRequest("Only the lead can create work").into());
            }
            create(
                tx,
                &agent,
                input,
                actor_id.unwrap_or(&agent.id.0),
                request_id,
                signature,
            )
            .map_err(Into::into)
        }
        WorkWrite::Mutate(input) => {
            if let Some(actor) = &actor {
                authorize_mutation(tx, &agent, actor, &input)?;
            }
            mutate(tx, &agent, input, actor_id, request_id, signature).map_err(Into::into)
        }
        WorkWrite::Claim(input) => claim(tx, &agent, input, actor_id, request_id, signature),
        WorkWrite::Submit(input) => submit(tx, &agent, input, actor_id, request_id, signature),
        WorkWrite::Accept(input) => accept(tx, &agent, input, actor_id, request_id, signature),
    }?;
    if let Some(token) = admitted {
        let typed = WorkItemRecord::checked(&result.record)?;
        if is_accept && result.record["archivePending"] == true {
            let mut work = WorkItemsRepository::work_record(tx, typed.id().as_ref())?
                .ok_or(Error::InvalidRequest("Unknown work item"))?
                .into_value();
            work["archiveIntent"]["publicAdmission"] =
                studio_runtime_store::projection_value(&token);
            work["archiveIntent"]["publicRequestId"] = json!(request_id);
            work["archiveIntent"]["publicResponse"] =
                studio_runtime_store::projection_value(&typed);
            WorkItemsRepository::update_work(tx, typed.id().as_ref(), &work)?;
            reserve_work_request(
                tx,
                &agent,
                request_id.unwrap_or_default(),
                typed.id().as_ref(),
                &token,
                true,
            )?;
        } else {
            event_write::settle_request(tx, &token, actor_id.or(Some(agent_id)), &typed)
                .map_err(Error::from)?;
            clear_work_request(tx, request_id.unwrap_or_default(), &token)?;
        }
    }
    Ok(result)
}

/// Execute a work action with a persisted agent as the acting principal.
pub fn write_as_agent(
    tx: &Transaction<'_>,
    agent_id: &str,
    actor_id: &str,
    request_id: Option<&str>,
    signature: &str,
    command: WorkActorWrite,
) -> Result<WorkActorResult, Error> {
    let command = match command {
        WorkActorWrite::Create(input) => WorkWrite::Create(input),
        WorkActorWrite::Mutate(input) => WorkWrite::Mutate(input),
    };
    let result = write_internal(tx, agent_id, Some(actor_id), request_id, signature, command)
        .map_err(|error| match error {
            WorkWriteError::Expired => Error::RequestResult(
                studio_runtime_store::request_results::RequestResultError::Expired,
            ),
            WorkWriteError::Event(error) => error,
            WorkWriteError::Store(error) => Error::Store(error),
            WorkWriteError::NotImplemented(_) => Error::InvalidRequest("Invalid request"),
            // This synchronous transaction helper cannot create the blocking-worker variant.
            WorkWriteError::PreparationWorker => Error::InvalidRequest("Invalid request"),
        })?;
    Ok(WorkActorResult {
        record: result.record,
        event_inserted: result.event_inserted,
    })
}

/// Apply caller provenance to all work actions introduced by this slice.
pub fn write_action_as_agent(
    tx: &Transaction<'_>,
    agent_id: &str,
    actor_id: &str,
    request_id: Option<&str>,
    signature: &str,
    command: WorkWrite,
) -> Result<WorkWriteResult, WorkWriteError> {
    write_internal(tx, agent_id, Some(actor_id), request_id, signature, command)
}

fn authorize_mutation(
    tx: &Transaction<'_>,
    selected: &AgentRecord,
    actor: &AgentRecord,
    mutation: &WorkMutation,
) -> Result<(), Error> {
    match mutation {
        WorkMutation::Update { .. } | WorkMutation::Reject { .. } => {
            if actor.id != actor.root_id {
                return Err(Error::InvalidRequest(
                    "Only the lead can change work assignments",
                ));
            }
        }
        WorkMutation::Cancel { task_id, .. } => {
            let work = WorkItemsRepository::work_record(tx, task_id)?
                .ok_or(Error::InvalidRequest("Unknown work item"))?;
            if actor.id != actor.root_id
                && work.extension("createdBy").and_then(Value::as_str) != Some(actor.id.0.as_str())
            {
                return Err(Error::InvalidRequest(
                    "Only the lead or task creator can cancel this task",
                ));
            }
        }
    }
    if selected.root_id != actor.root_id {
        return Err(Error::InvalidRequest("Actor belongs to another team"));
    }
    Ok(())
}

/// Run the filesystem half after the transaction commits. A same-identity retry
/// recreates a missing artifact and rejects a conflicting file.
pub fn write_result_artifact(artifact: &ResultArtifact) -> Result<(), std::io::Error> {
    #[cfg(not(unix))]
    {
        let _ = artifact;
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "safe result artifact writes require directory-relative filesystem operations",
        ));
    }
    #[cfg(unix)]
    write_result_artifact_at(artifact)
}

#[cfg(unix)]
fn write_result_artifact_at(artifact: &ResultArtifact) -> Result<(), std::io::Error> {
    use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, openat};

    let root = CanonicalPath::existing(&artifact.state_dir)?;
    let relative = artifact
        .path
        .strip_prefix(&artifact.state_dir)
        .map_err(|_| std::io::Error::other("invalid result artifact path"))?;
    let parent_path = relative
        .parent()
        .ok_or_else(|| std::io::Error::other("invalid result artifact path"))?;
    let mut parent = fs::File::open("/")?;
    for component in root.components() {
        let std::path::Component::Normal(name) = component else {
            if matches!(component, std::path::Component::RootDir) {
                continue;
            }
            return Err(std::io::Error::other("invalid result artifact root"));
        };
        parent = openat(
            &parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map(fs::File::from)
        .map_err(std::io::Error::from)?;
    }
    for component in parent_path.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(std::io::Error::other("invalid result artifact path"));
        };
        let open_directory = || {
            openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map(fs::File::from)
        };
        match open_directory() {
            Ok(directory) => parent = directory,
            Err(error) if error == rustix::io::Errno::NOENT => {
                match mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                parent = open_directory().map_err(std::io::Error::from)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let file_name = relative
        .file_name()
        .ok_or_else(|| std::io::Error::other("invalid result artifact path"))?;
    let mut temporary_file = None;
    let mut temporary_name = String::new();
    for _ in 0..32 {
        temporary_name = format!(
            ".result-{}-{}.tmp",
            std::process::id(),
            GENERATED_WORK_ID.fetch_add(1, Ordering::Relaxed)
        );
        match openat(
            &parent,
            temporary_name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        ) {
            Ok(file) => {
                temporary_file = Some(fs::File::from(file));
                break;
            }
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let mut temporary_file = temporary_file
        .ok_or_else(|| std::io::Error::other("could not allocate result artifact temporary"))?;
    let mut temporary_guard = TemporaryArtifact {
        parent: &parent,
        name: Some(temporary_name.clone()),
    };
    temporary_file.write_all(artifact.body.as_bytes())?;
    temporary_file.sync_all()?;
    drop(temporary_file);
    let result = match linkat(
        &parent,
        temporary_name.as_str(),
        &parent,
        file_name,
        AtFlags::empty(),
    ) {
        Ok(()) => Ok(()),
        Err(error) if error == rustix::io::Errno::EXIST => {
            let existing = openat(
                &parent,
                file_name,
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map(fs::File::from)
            .map_err(std::io::Error::from)?;
            if !existing.metadata()?.is_file()
                || existing.metadata()?.len() > artifact.body.len() as u64
            {
                Err(std::io::Error::other("saved task result file is invalid"))
            } else {
                let mut saved = Vec::with_capacity(artifact.body.len());
                existing
                    .take(artifact.body.len() as u64 + 1)
                    .read_to_end(&mut saved)?;
                if saved == artifact.body.as_bytes() {
                    Ok(())
                } else {
                    Err(std::io::Error::other(
                        "saved task result file differs from its committed record",
                    ))
                }
            }
        }
        Err(error) => Err(error.into()),
    };
    result?;
    temporary_guard.cleanup()?;
    Ok(())
}

#[cfg(unix)]
struct TemporaryArtifact<'a> {
    parent: &'a fs::File,
    name: Option<String>,
}

#[cfg(unix)]
impl TemporaryArtifact<'_> {
    fn cleanup(&mut self) -> Result<(), std::io::Error> {
        let Some(name) = self.name.as_deref() else {
            return Ok(());
        };
        rustix::fs::unlinkat(self.parent, name, rustix::fs::AtFlags::empty())
            .map_err(std::io::Error::from)?;
        self.name.take();
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for TemporaryArtifact<'_> {
    fn drop(&mut self) {
        if let Some(name) = self.name.take() {
            match rustix::fs::unlinkat(self.parent, name.as_str(), rustix::fs::AtFlags::empty()) {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

/// Release persisted claims for failed or deleted workers. Callers own cadence and
/// invoke this in the same transaction that commits the resulting recovery events.
pub fn release_failed_work(
    tx: &Transaction<'_>,
    agents: &[AgentRecord],
    now: f64,
) -> Result<ReleaseFailedWorkResult, Error> {
    let mut released_ids = Vec::new();
    let mut event_inserted = false;
    for snapshot in agents {
        let Some(worker) = AgentRepository::get(tx, &snapshot.id)? else {
            continue;
        };
        if worker.root_id != snapshot.root_id
            || worker.epoch != snapshot.epoch
            || worker.status != snapshot.status
        {
            continue;
        }
        let deleted = worker.is_deleted();
        if worker.id == worker.root_id
            || (!deleted && worker.status.as_str() != "failed")
            || (!deleted && continuation_work_pending(&worker))
        {
            continue;
        }
        let reason = if deleted {
            "The worker was deleted".to_owned()
        } else {
            worker
                .error_text()
                .unwrap_or("The worker failed")
                .to_owned()
        };
        for released in WorkItemsRepository::release_failed_work(tx, &worker.id.0, &reason, now)? {
            let id = released["id"]
                .as_str()
                .ok_or_else(|| invalid("runtime_work", "<invalid>"))?;
            let root = AgentRepository::get(tx, &worker.root_id)?;
            if let Some(root) = root.filter(|root| !root.is_deleted()) {
                let owner = released["releases"]
                    .as_array()
                    .and_then(|history| history.last())
                    .and_then(|release| release.get("agent"))
                    .and_then(Value::as_str)
                    .unwrap_or(&worker.id.0);
                let reason = released["releases"]
                    .as_array()
                    .and_then(|history| history.last())
                    .and_then(|release| release.get("reason"))
                    .cloned()
                    .unwrap_or_else(|| json!("The worker failed"));
                event_inserted |= event_write::enqueue_recovery_event(
                    tx,
                    &root,
                    format!(
                        "work-released:{id}:{}",
                        released["version"].as_i64().unwrap_or_default()
                    ),
                    "work_released",
                    json!({"task":id,"title":released["title"],"agent":owner,"reason":reason})
                        .to_string(),
                    now,
                )?;
            }
            released_ids.push(id.to_owned());
        }
    }
    Ok(ReleaseFailedWorkResult {
        task_ids: released_ids,
        event_inserted,
    })
}

fn continuation_work_pending(agent: &AgentRecord) -> bool {
    if agent.auto_wake != Some(true) || agent.is_deleted() {
        return false;
    }
    let default_account = json!("default");
    let account = Value::String(agent.account_key.clone());
    let thread = agent.thread_id.as_deref();
    let epoch = agent.epoch;
    let last_turn = agent.last_completed_turn.value().map(String::as_str);

    let matches = |record: &Value, statuses: &[&str], turn_field: &str| {
        let record_account = record.get("accountKey").unwrap_or(&default_account);
        record
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| statuses.contains(&status))
            && record_account == &account
            && record.get("threadId").and_then(Value::as_str) == thread
            && thread.is_some_and(|thread| !thread.is_empty())
            && record.get("epoch").and_then(Value::as_i64) == Some(epoch)
            && record.get(turn_field).and_then(Value::as_str) == last_turn
            && last_turn.is_some_and(|turn| !turn.is_empty())
    };

    let capacity = agent.extra.get("capacityRetry").unwrap_or(&Value::Null);
    let Ok(usage_value) = agent.usage_resume_value() else {
        return true;
    };
    let usage = usage_value.as_ref().unwrap_or(&Value::Null);
    let restart = agent.restart_recovery.as_deref();
    matches(capacity, &["scheduled", "starting", "unknown"], "turnId")
        || matches(usage, &["scheduled", "started", "unknown"], "turnId")
        || (restart.and_then(|v| v.stage.as_deref()) == Some("pending")
            && restart.and_then(|v| v.auto_wake) == Some(true)
            && restart
                .and_then(|v| v.account_key.as_deref())
                .unwrap_or("default")
                == agent.account_key
            && restart.and_then(|v| v.thread_id.as_deref()) == thread
            && thread.is_some_and(|thread| !thread.is_empty())
            && restart
                .and_then(|v| v.epoch.as_ref())
                .and_then(|v| v.number().as_i64())
                == Some(epoch))
}

struct WorkSnapshot<'a> {
    record: &'a WorkItemRecord,
    dependency_statuses: &'a BTreeMap<String, WorkStatus>,
}

fn decide_claim(
    snapshot: WorkSnapshot<'_>,
    claimant: &str,
    now: f64,
) -> Result<WorkItemRecord, Error> {
    let record = snapshot.record;
    let checked = record;
    if !matches!(checked.status(), WorkStatus::Ready | WorkStatus::Running)
        || !checked.blocked_by(snapshot.dependency_statuses).is_empty()
    {
        return Err(Error::InvalidRequest("This work item is not ready"));
    }
    if checked.owner().is_some_and(|owner| owner != claimant) {
        return Err(Error::InvalidRequest("Another agent owns this work item"));
    }
    let mut updated = record.clone();
    updated
        .set("owner", json!(claimant))
        .map_err(studio_runtime_store::StoreError::from)?;
    updated.set_status(WorkStatus::Running);
    advance_checked(&mut updated, now)?;
    Ok(updated)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmittedResult<'a> {
    id: &'a str,
    agent: &'a str,
    text: &'a str,
    checks: &'a str,
    revision: &'a str,
    files: &'a [String],
    created: f64,
    result_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a RawJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layr_project_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layr_line: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layr_state_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    attempt_id: Field<&'a str>,
}

fn decide_submit(
    snapshot: WorkSnapshot<'_>,
    submitter: &AgentRecord,
    input: &SubmitWork,
    identity: Option<&(String, Option<String>)>,
    now: f64,
    result_id: &str,
) -> Result<(WorkItemRecord, ResultArtifact), Error> {
    let record = snapshot.record;
    let id = record.id().as_ref();
    if !matches!(
        record.status(),
        WorkStatus::Running | WorkStatus::Ready | WorkStatus::Blocked | WorkStatus::Review
    ) {
        return Err(Error::InvalidRequest("This result is already accepted"));
    }
    if !record.blocked_by(snapshot.dependency_statuses).is_empty() {
        return Err(Error::InvalidRequest("Dependencies have not been accepted"));
    }
    let title = record.title().ok_or_else(|| invalid("runtime_work", id))?;
    let submitter_id = submitter.id.0.as_str();
    let path = result_path(&input.state_dir, id, result_id)?;
    let structured = record
        .extension("resultSchema")
        .map(|schema| {
            studio_runtime_schema::task_results::validate_result(schema, &input.result)
                .map_err(|_| Error::InvalidRequest("Result does not match result_schema"))
        })
        .transpose()?;
    let result_text = if let Some(value) = &structured {
        value.to_string()
    } else {
        python_bounded_text(
            input.result.as_str().ok_or(Error::InvalidRequest(
                "Supply result as text when the task has no result_schema",
            ))?,
            32000,
        )?
    };
    let checks_text = python_bounded_text(&input.checks, 32000)?;
    let structured = structured
        .as_ref()
        .map(|value| studio_json::to_raw_value(value).map(RawJson))
        .transpose()
        .map_err(studio_runtime_store::StoreError::from)?;
    let revision_text = if reviewer_without_revision(submitter, &input.revision) {
        String::new()
    } else {
        python_bounded_text(&input.revision, 200)?
    };
    if let Some(evidence) = &input.layr_evidence {
        let normalized_revision = python_bounded_text(&input.revision, 200)?;
        if evidence.layr_state_id != normalized_revision {
            return Err(Error::InvalidRequest("Submit the exact layr state ID"));
        }
    }
    let result = studio_runtime_store::projection_value(&SubmittedResult {
        id: result_id,
        agent: submitter_id,
        text: &result_text,
        checks: &checks_text,
        revision: &revision_text,
        files: &input.files,
        created: now,
        result_file: path.to_string_lossy().into_owned(),
        result: structured.as_ref(),
        layr_project_id: input
            .layr_evidence
            .as_ref()
            .map(|e| e.layr_project_id.as_str()),
        layr_line: input.layr_evidence.as_ref().map(|e| e.layr_line.as_str()),
        layr_state_id: input
            .layr_evidence
            .as_ref()
            .map(|e| e.layr_state_id.as_str()),
        run_id: identity.as_ref().map(|i| i.0.as_str()),
        attempt_id: identity.as_ref().map_or(Field::Missing, |i| {
            i.1.as_deref().map_or(Field::Null, Field::Value)
        }),
    });
    let mut updated = record.clone();
    updated
        .results_mut()
        .ok_or_else(|| invalid("runtime_work", id))?
        .push(
            result
                .as_object()
                .expect("constructed result object")
                .clone(),
        );
    updated.set_status(WorkStatus::Review);
    advance_checked(&mut updated, now)?;
    let body = result_artifact_body(title, &result)?;
    Ok((
        updated,
        ResultArtifact {
            state_dir: input.state_dir.clone(),
            path,
            body,
        },
    ))
}

fn decide_accept(
    snapshot: WorkSnapshot<'_>,
    reason: &str,
    by: &str,
    now: f64,
    override_reason: Option<&str>,
) -> Result<WorkItemRecord, Error> {
    let record = snapshot.record;
    if record.status() == studio_runtime_store::WorkStatus::Accepted {
        return Err(Error::InvalidRequest(
            "This work is already accepted. Create a follow-up task with this task ID and the new evidence",
        ));
    }
    let result_id = record
        .results()
        .and_then(|results| results.last())
        .and_then(|result| result.get("id"))
        .and_then(Value::as_str)
        .ok_or(Error::InvalidRequest("Submit a result before review"))?;
    if record.status() != studio_runtime_store::WorkStatus::Review {
        return Err(Error::InvalidRequest("Submit a result before review"));
    }
    crate::task_gate::require_pass(record, override_reason)?;
    let mut updated = record.clone();
    if crate::event_write::python_strip(reason).is_empty() {
        return Err(Error::InvalidRequest("Field must not be empty"));
    }
    let decision = json!({"resultId":result_id,"decision":"accept","reason":text(reason, false)?,"by":by,"created":now});
    updated
        .decisions_mut()
        .ok_or_else(|| invalid("runtime_work", record.id().as_ref()))?
        .push(
            decision
                .as_object()
                .expect("constructed decision object")
                .clone(),
        );
    if let Some(reason) = override_reason {
        updated
            .decisions_mut()
            .expect("decision array exists after push")
            .last_mut()
            .expect("accepted decision exists after push")
            .insert("gateOverrideReason".into(), json!(reason));
    }
    updated.set_status(WorkStatus::Accepted);
    updated
        .set(
            "archive",
            json!({"status":"skipped","reason":"The task owner is the lead or is unassigned"}),
        )
        .map_err(studio_runtime_store::StoreError::from)?;
    advance_checked(&mut updated, now)?;
    let _ = snapshot.dependency_statuses;
    Ok(updated)
}

fn advance_checked(record: &mut WorkItemRecord, now: f64) -> Result<(), Error> {
    let id = record.id().as_ref();
    let version = record
        .version()
        .and_then(studio_json::Number::as_i64)
        .ok_or_else(|| invalid("runtime_work", id))?;
    let next = version
        .checked_add(1)
        .ok_or_else(|| invalid("runtime_work", id))?;
    // Preserve the stored-schema diagnostic for out-of-range numbers or non-finite time.
    if record.set("version", json!(next)).is_err() || record.set("updated", json!(now)).is_err() {
        let mut raw = record.value();
        raw["version"] = json!(next);
        raw["updated"] = json!(now);
        return Err(Error::Store(
            WorkItemRecord::checked(&raw).expect_err("invalid advance"),
        ));
    }
    Ok(())
}

fn result_path(state_dir: &Path, task_id: &str, result_id: &str) -> Result<PathBuf, Error> {
    if !task_id
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
        || !result_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    {
        return Err(invalid("runtime_work", task_id));
    }
    Ok(state_dir
        .join("results")
        .join(task_id)
        .join(format!("{result_id}.md")))
}

fn result_artifact(record: &Value, state_dir: &Path) -> Result<ResultArtifact, Error> {
    let result = record["results"]
        .as_array()
        .and_then(|results| results.last())
        .ok_or_else(|| invalid("runtime_work", record["id"].as_str().unwrap_or("<invalid>")))?;
    let task_id = record["id"]
        .as_str()
        .ok_or_else(|| invalid("runtime_work", "<invalid>"))?;
    let result_id = result["id"]
        .as_str()
        .ok_or_else(|| invalid("runtime_work", task_id))?;
    let path = result["resultFile"]
        .as_str()
        .ok_or_else(|| invalid("runtime_work", task_id))?;
    let expected_path = result_path(state_dir, task_id, result_id)?;
    if Path::new(path) != expected_path {
        return Err(invalid("runtime_work", task_id));
    }
    Ok(ResultArtifact {
        state_dir: state_dir.to_owned(),
        path: PathBuf::from(path),
        body: result_artifact_body(
            record["title"]
                .as_str()
                .ok_or_else(|| invalid("runtime_work", task_id))?,
            result,
        )?,
    })
}

#[derive(serde::Deserialize)]
struct ArtifactResult {
    id: String,
    text: String,
    checks: String,
    revision: String,
    files: Vec<String>,
}

fn result_artifact_body(title: &str, result: &Value) -> Result<String, Error> {
    let id = result["id"].as_str().unwrap_or("<invalid>");
    let result = <ArtifactResult as serde::Deserialize>::deserialize(result)
        .map_err(|_| invalid("runtime_work", id))?;
    let ArtifactResult {
        id,
        text,
        checks,
        revision,
        files,
    } = result;
    let files = files
        .iter()
        .map(|value| format!("- `{value}`"))
        .collect::<Vec<_>>();
    Ok(format!(
        "# {title}\n\nResult ID: `{id}`\n\n## Result\n\n{text}\n\n## Checks\n\n{checks}\n\n## Revision\n\n{revision}\n\n## Files\n\n{}\n",
        if files.is_empty() {
            "None".to_owned()
        } else {
            files.join("\n")
        },
    ))
}

fn create(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    input: CreateWork,
    actor_id: &str,
    _request_id: Option<&str>,
    _signature: &str,
) -> Result<WorkWriteResult, Error> {
    if let Some(schema) = &input.result_schema {
        studio_runtime_schema::task_results::compile(schema).map_err(Error::InvalidResultSchema)?;
    }
    let title = text(&input.title, false)?;
    let description = text(&input.description, true)?;
    let status = input
        .status
        .unwrap_or(studio_runtime_schema::task_command::InitialStatus::Ready);
    let owner = input.owner.as_deref();
    check_owner(tx, agent, owner)?;
    let id = work_id(event_write::unix_time()?);
    let dependencies = unique(input.dependencies);
    validate_dependencies(tx, &agent.root_id.0, &id, &dependencies)?;
    let root = root_agent(tx, agent)?;
    let gate = match input.gate {
        Field::Value(gate) => Some(gate.normalize().map_err(Error::InvalidRequest)?),
        Field::Null => None,
        Field::Missing => root
            .task_default_gate
            .value()
            .cloned()
            .map(studio_runtime_schema::task_gate::GateInput::Config)
            .map(studio_runtime_schema::task_gate::GateInput::normalize)
            .transpose()
            .map_err(Error::InvalidRequest)?,
    };
    let mut work = json!({
        "gate":gate,
        "id": id, "rootId": agent.root_id.0, "title": title, "description": description,
        "owner": owner, "dependencies": dependencies, "status": status,
        "created": event_write::unix_time()?, "updated": event_write::unix_time()?, "version": 1,
        "results": [], "decisions": [], "createdBy": actor_id,
    });
    if let Some(schema) = input.result_schema {
        work["resultSchema"] = schema;
    }
    let view = single_view(tx, &WorkItemRecord::checked(&work)?)?;
    if !WorkItemsRepository::put_work(tx, &id, &work)? {
        return Err(Error::Conflict("Work item already exists"));
    }
    let mut event_inserted = false;
    if let Some(owner) = owner.filter(|owner| *owner != agent.root_id.0)
        && is_ready(&view)
    {
        let recipient = AgentRepository::get(tx, &AgentId::from(owner))?
            .ok_or_else(|| invalid("runtime_agents", owner))?;
        event_inserted = queue_assignment(tx, &recipient, &work)?;
    }

    Ok(WorkWriteResult {
        record: view,
        event_inserted,
        artifact: None,
    })
}

fn mutate(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    mutation: WorkMutation,
    actor_id: Option<&str>,
    _request_id: Option<&str>,
    _signature: &str,
) -> Result<WorkWriteResult, Error> {
    let (task_id, expected) = match &mutation {
        WorkMutation::Update {
            task_id, version, ..
        }
        | WorkMutation::Cancel {
            task_id, version, ..
        }
        | WorkMutation::Reject {
            task_id, version, ..
        } => (task_id.clone(), *version),
    };
    let work = WorkItemsRepository::work_record(tx, &task_id)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?;
    if work.root_id() != Some(agent.root_id.0.as_str()) {
        return Err(invalid("runtime_work", &task_id));
    }
    let version = work
        .version()
        .and_then(studio_json::Number::as_i64)
        .ok_or_else(|| invalid("runtime_work", &task_id))?;
    if expected.is_some_and(|expected| expected != version) {
        return Err(Error::Conflict(
            "This work item changed. Reload before editing",
        ));
    }
    let status = work.status();
    let owner = work.owner().map(str::to_owned);
    let previous_ready = is_ready(&single_view(tx, &work)?);
    let mut work = work.into_value();
    let mut wake = false;
    let mut assignment_owner = None;

    match mutation {
        WorkMutation::Update {
            gate,
            title,
            description,
            owner: requested_owner,
            status: requested_status,
            dependencies,
            result_schema,
            ..
        } => {
            if status == WorkStatus::Accepted {
                return Err(Error::InvalidRequest(
                    "Cannot edit accepted work. Create a follow-up task with this task ID and the new evidence",
                ));
            }
            if status == WorkStatus::Review {
                return Err(Error::InvalidRequest(
                    "Work under review cannot be reassigned; reject its result before editing",
                ));
            }
            if let Some(schema) = result_schema {
                studio_runtime_schema::task_results::compile(&schema)
                    .map_err(Error::InvalidResultSchema)?;
                work["resultSchema"] = schema;
            }
            match gate {
                Field::Missing => {}
                Field::Null => work["gate"] = Value::Null,
                Field::Value(gate) => {
                    let gate = gate.normalize().map_err(Error::InvalidRequest)?;
                    work["gate"] = studio_runtime_store::projection_value(&gate);
                }
            }
            if let Some(title) = title {
                work["title"] = json!(text(&title, false)?);
            }
            if let Some(description) = description {
                work["description"] = json!(text(&description, true)?);
            }
            if let Some(status) = requested_status {
                work["status"] = json!(status);
            }
            if let Some(dependencies) = dependencies {
                let dependencies = unique(dependencies);
                validate_dependencies(tx, &agent.root_id.0, &task_id, &dependencies)?;
                work["dependencies"] = json!(dependencies);
            }
            let next_owner = requested_owner.map(|owner| owner.unwrap_or_default());
            if let Some(owner) = next_owner {
                let owner = (!owner.is_empty()).then_some(owner);
                check_owner(tx, agent, owner.as_deref())?;
                work["owner"] = owner.map_or(Value::Null, Value::String);
            }
            if let Some(owner) = work["owner"].as_str()
                && owner != agent.root_id.0
            {
                event_write::assert_delegation(&root_agent(tx, agent)?)?;
            }
            let view = single_view(tx, &WorkItemRecord::checked(&work)?)?;
            let current_owner = work["owner"].as_str();
            let changed = current_owner != owner.as_deref();
            let ready = is_ready(&view);
            if ready
                && (changed || !previous_ready)
                && current_owner.is_some_and(|owner| owner != agent.root_id.0)
            {
                assignment_owner = current_owner.map(str::to_owned);
            }
            if let Some(previous_owner) = owner.as_deref()
                && (changed || previous_ready != ready)
            {
                WorkItemsRepository::retire_pending_work_ready(
                    tx,
                    previous_owner,
                    &task_id,
                    "The work assignment or readiness changed before delivery",
                )?;
            }
        }
        WorkMutation::Cancel { reason, .. } => {
            let reason = text(&reason, false)?;
            if status == WorkStatus::Cancelled {
                let view = single_view(tx, &WorkItemRecord::checked(&work)?)?;

                return Ok(WorkWriteResult {
                    record: view,
                    event_inserted: false,
                    artifact: None,
                });
            }
            if status == WorkStatus::Accepted {
                return Err(Error::InvalidRequest("Cannot cancel accepted work"));
            }
            push_decision(
                &mut work,
                json!({"decision":"cancel", "reason":reason, "by":actor_id.unwrap_or(&agent.id.0), "owner":owner, "created":event_write::unix_time()?}),
            )?;
            work["status"] = json!("cancelled");
            work["owner"] = Value::Null;
            if let Some(owner_id) = owner.as_deref() {
                WorkItemsRepository::retire_pending_work_ready(
                    tx,
                    owner_id,
                    &task_id,
                    "The task was cancelled before delivery",
                )?;
                if let Some(recipient) = AgentRepository::get(tx, &AgentId::from(owner_id))?
                    && live_recipient(&recipient)
                {
                    wake |= event_write::enqueue_recovery_event(
                        tx,
                        &recipient,
                        format!("work-decision:{task_id}:cancel"),
                        "work_decision",
                        decision_notice(&task_id, "cancel", Some(&reason)),
                        event_write::unix_time()?,
                    )?;
                }
            }
        }
        WorkMutation::Reject { reason, .. } => {
            if status == WorkStatus::Accepted {
                return Err(Error::InvalidRequest(
                    "This work is already accepted. Create a follow-up task with this task ID and the new evidence",
                ));
            }
            if status != WorkStatus::Review {
                return Err(Error::InvalidRequest("Submit a result before review"));
            }
            let reason = text(&reason, false)?;
            let result_id = work["results"]
                .as_array()
                .and_then(|v| v.last())
                .and_then(|v| v["id"].as_str())
                .ok_or_else(|| invalid("runtime_work", &task_id))?
                .to_owned();
            if let Some(owner_id) = owner.as_deref() {
                if owner_id != agent.root_id.0 {
                    event_write::assert_delegation(&root_agent(tx, agent)?)?;
                }
                let recipient = AgentRepository::get(tx, &AgentId::from(owner_id))?
                    .ok_or_else(|| invalid("runtime_agents", owner_id))?;
                if actor_id != Some(owner_id) {
                    wake |= event_write::enqueue_recovery_event(
                        tx,
                        &recipient,
                        format!("work-decision:{task_id}:{version}"),
                        "work_decision",
                        decision_notice(&task_id, "reject", Some(&reason)),
                        event_write::unix_time()?,
                    )?;
                }
            }
            WorkItemsRepository::close_review_event(tx, &agent.root_id.0, &result_id)?;
            push_decision(
                &mut work,
                json!({"resultId":result_id,"decision":"reject","reason":reason,"by":actor_id.unwrap_or("user"),"created":event_write::unix_time()?}),
            )?;
            work["status"] = json!("ready");
        }
    }

    work["version"] = json!(version + 1);
    work["updated"] = json!(event_write::unix_time()?);
    WorkItemsRepository::update_work(tx, &task_id, &work)?;
    if let Some(owner_id) = assignment_owner {
        let recipient = AgentRepository::get(tx, &AgentId::from(owner_id.as_str()))?
            .ok_or_else(|| invalid("runtime_agents", &owner_id))?;
        wake |= queue_assignment(tx, &recipient, &work)?;
    }
    let view = single_view(tx, &WorkItemRecord::checked(&work)?)?;

    Ok(WorkWriteResult {
        record: view,
        event_inserted: wake,
        artifact: None,
    })
}

fn claim(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    input: ClaimWork,
    actor_id: Option<&str>,
    _request_id: Option<&str>,
    _signature: &str,
) -> Result<WorkWriteResult, WorkWriteError> {
    let work = load_work(tx, &agent.root_id.0, &input.task_id, input.version)?;
    let claimant = event_write::checked_agent(tx, actor_id.unwrap_or(&input.claimant_id))?;
    if claimant.root_id != agent.root_id {
        return Err(Error::InvalidRequest("Claimant belongs to another team").into());
    }
    if claimant.id != agent.root_id && work.owner() != Some(claimant.id.0.as_str()) {
        event_write::assert_delegation(&root_agent(tx, agent)?)?;
    }
    let statuses = dependency_statuses(tx, &work)?;
    let updated = decide_claim(
        WorkSnapshot {
            record: &work,
            dependency_statuses: &statuses,
        },
        &claimant.id.0,
        event_write::unix_time()?,
    )?;
    WorkItemsRepository::update_checked_work(tx, &input.task_id, &updated)?;
    let record = single_view(tx, &updated)?;

    Ok(WorkWriteResult {
        record,
        event_inserted: false,
        artifact: None,
    })
}

fn refuse_legacy_work_participants(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    actor_id: Option<&str>,
    owner_id: Option<&str>,
) -> Result<(), WorkWriteError> {
    if crate::workspace_ready::legacy_vm_record(agent) {
        return Err(Error::InvalidRequest("VM agents are no longer supported").into());
    }
    for id in [actor_id, owner_id].into_iter().flatten() {
        let participant = event_write::checked_agent(tx, id)?;
        if crate::workspace_ready::legacy_vm_record(&participant) {
            return Err(Error::InvalidRequest("VM agents are no longer supported").into());
        }
    }
    Ok(())
}

fn submit(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    input: SubmitWork,
    actor_id: Option<&str>,
    request_id: Option<&str>,
    signature: &str,
) -> Result<WorkWriteResult, WorkWriteError> {
    let work = load_work(tx, &agent.root_id.0, &input.task_id, input.version)?;
    refuse_legacy_work_participants(tx, agent, actor_id, work.owner())?;
    if actor_id.is_some_and(|actor| work.owner() != Some(actor)) {
        return Err(Error::InvalidRequest("Only the assigned worker can submit its result").into());
    }
    let submitter_id = work.owner().unwrap_or(&agent.id.0);
    let submitter = event_write::checked_agent(tx, submitter_id)?;
    if submitter.root_id != agent.root_id {
        return Err(invalid("runtime_agents", submitter_id).into());
    }
    if !matches!(
        work.status(),
        WorkStatus::Running | WorkStatus::Ready | WorkStatus::Blocked | WorkStatus::Review
    ) {
        return Err(Error::InvalidRequest("This result is already accepted").into());
    }
    if !work.blocked_by(&dependency_statuses(tx, &work)?).is_empty() {
        return Err(Error::InvalidRequest("Dependencies have not been accepted").into());
    }
    if let Some((counter_id, failed, errors)) = result_refusal(tx, &work, &submitter, &input)? {
        return refuse_result(
            tx,
            &work,
            &submitter,
            &counter_id,
            failed,
            errors,
            request_id,
            signature,
        );
    }
    validate_submission_files(&submitter, &input.files, input.prepared_files.as_ref())?;
    if !reviewer_without_revision(&submitter, &input.revision)
        && submitter.workspace_mode
            == Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
    {
        let Some(request_id) = input.layr_evidence_request_id.as_deref() else {
            return Err(Error::Conflict(
                "Layr submission evidence must be checked before saving the result",
            )
            .into());
        };
        let Some(LayrWorkIntent::Completed { owner, result }) =
            read_layr_view(tx, &input.task_id)?.submit
        else {
            return Err(
                Error::Conflict("Layr submission evidence changed before it was saved").into(),
            );
        };
        let revision = python_bounded_text(&input.revision, 200)?;
        if owner.request_id != request_id
            || owner.owner != submitter_id
            || Some(owner.id.version) != work.version().and_then(studio_json::Number::as_i64)
            || owner.revision != revision
            || Some(&result.evidence) != input.layr_evidence.as_ref()
        {
            return Err(
                Error::Conflict("Layr submission evidence changed before it was saved").into(),
            );
        }
    }
    event_write::refuse_remote(&submitter)?;
    let statuses = dependency_statuses(tx, &work)?;
    let identity = WorkItemsRepository::submission_identity(tx, &submitter)?;
    let result_id = new_id()?;
    let now = event_write::unix_time()?;
    let (mut updated, artifact) = decide_submit(
        WorkSnapshot {
            record: &work,
            dependency_statuses: &statuses,
        },
        &submitter,
        &input,
        identity.as_ref(),
        now,
        &result_id,
    )?;
    let gated = crate::task_gate::admit(
        tx,
        &submitter,
        &mut updated,
        input.prepared_gate_workspace.as_deref(),
        &input.state_dir,
        now,
    )
    .map_err(|error| match error {
        crate::monitor_policy::MonitorError::Expired => WorkWriteError::Expired,
        crate::monitor_policy::MonitorError::Store(error) => WorkWriteError::Store(error),
        crate::monitor_policy::MonitorError::InvalidStored { .. }
        | crate::monitor_policy::MonitorError::InvalidResultReceipt(_)
        | crate::monitor_policy::MonitorError::InvalidDeliveryIdentity(_)
        | crate::monitor_policy::MonitorError::InvalidOutputReceipt(_)
        | crate::monitor_policy::MonitorError::TaskFailed
        | crate::monitor_policy::MonitorError::InputOutcomeUnknown
        | crate::monitor_policy::MonitorError::Refused(_)
        | crate::monitor_policy::MonitorError::Unavailable(_)
        | crate::monitor_policy::MonitorError::ProviderRequest { .. }
        | crate::monitor_policy::MonitorError::ConfigurationRejected { .. }
        | crate::monitor_policy::MonitorError::ConfigurationReceipt { .. }
        | crate::monitor_policy::MonitorError::Event(_)
        | crate::monitor_policy::MonitorError::Rules(_)
        | crate::monitor_policy::MonitorError::Io(_) => WorkWriteError::Event(Error::Conflict(
            "The acceptance gate could not be admitted; check workspace and monitor capacity",
        )),
    })?;
    let result = updated.value()["results"]
        .as_array()
        .and_then(|results| results.last())
        .cloned()
        .ok_or_else(|| invalid("runtime_work", &input.task_id))?;
    let mut event_inserted = false;
    if (!gated && actor_id != Some(agent.root_id.0.as_str()))
        || (gated && result["gate"]["status"] != "running")
    {
        let lead = AgentRepository::get(tx, &agent.root_id)?
            .ok_or_else(|| invalid("runtime_agents", &agent.root_id.0))?;
        let result_id = result["id"]
            .as_str()
            .ok_or_else(|| invalid("runtime_work", &input.task_id))?;
        event_inserted |= event_write::enqueue_recovery_event(
            tx,
            &lead,
            format!("work-result:{result_id}"),
            "work_review",
            work_review_notice(&input.task_id, work.title(), &result),
            now,
        )?;
    }
    WorkItemsRepository::update_checked_work(tx, &input.task_id, &updated)?;
    let record = single_view(tx, &updated)?;

    Ok(WorkWriteResult {
        record,
        event_inserted,
        artifact: Some(artifact),
    })
}

/// Shared identity for refusal writes and the current task read projection.
fn submission_counter_id(
    connection: &Connection,
    work: &WorkItemRecord,
    owner: &AgentRecord,
) -> Result<String, WorkWriteError> {
    let attempt = WorkItemsRepository::submission_identity(connection, owner)?;
    let identity = studio_json::to_string(&(
        work.id().as_ref(),
        work.version(),
        &owner.id.0,
        owner.epoch,
        attempt,
    ))
    .map_err(studio_runtime_store::StoreError::from)?;
    let counter_id = format!(
        "work-result-failures:{}",
        studio_runtime_store::sha256_hex(identity.as_bytes())
    );
    Ok(counter_id)
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResultFailures {
    version: i64,
    owner: String,
    epoch: i64,
    provider_attempt: Option<(String, Option<String>)>,
    count: u64,
    errors: Vec<Value>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultFailureView {
    failed_submits: u64,
    limit: u64,
    retry_allowed: bool,
    errors: Vec<Value>,
}

#[derive(serde::Serialize)]
struct ResultRefusal {
    message: &'static str,
    #[serde(flatten)]
    view: ResultFailureView,
}

fn submission_refusal_message(errors: Vec<Value>, failed: u64, limit: u64) -> Value {
    studio_runtime_store::projection_value(&ResultRefusal {
        message: "Result does not satisfy the task result contract",
        view: ResultFailureView {
            failed_submits: failed,
            limit,
            retry_allowed: failed < limit,
            errors,
        },
    })
}

fn current_result_failures(
    connection: &Connection,
    work: &WorkItemRecord,
    owner: &AgentRecord,
) -> Result<Option<ResultFailures>, WorkWriteError> {
    let Some(value) = work.extension("resultFailures") else {
        return Ok(None);
    };
    let invalid_record = || invalid("runtime_work", work.id().as_ref());
    let saved =
        <ResultFailures as serde::Deserialize>::deserialize(value).map_err(|_| invalid_record())?;
    let current = Some(saved.version) == work.version().and_then(studio_json::Number::as_i64)
        && saved.owner == owner.id.0
        && saved.epoch == owner.epoch
        && saved.provider_attempt == WorkItemsRepository::submission_identity(connection, owner)?;
    Ok(current.then_some(saved))
}

/// Project only the current task version, owner epoch and provider attempt.
pub fn current_submission_refusal(
    connection: &Connection,
    work: &WorkItemRecord,
) -> Result<Value, WorkWriteError> {
    use studio_runtime_schema::task_results::MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT;
    let mut view = ResultFailureView {
        failed_submits: 0,
        limit: MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT,
        retry_allowed: true,
        errors: Vec::new(),
    };
    let Some(owner_id) = work.owner() else {
        return Ok(studio_runtime_store::projection_value(&view));
    };
    let Some(owner) = AgentRepository::get(connection, &AgentId::from(owner_id))? else {
        return Ok(studio_runtime_store::projection_value(&view));
    };
    if let Some(counter) = current_result_failures(connection, work, &owner)? {
        if counter.count > MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT {
            return Err(invalid("runtime_work", work.id().as_ref()).into());
        }
        view.failed_submits = counter.count;
        view.retry_allowed = counter.count < MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT;
        view.errors = counter.errors;
    }
    Ok(studio_runtime_store::projection_value(&view))
}

/// Counter identity includes the task version, worker epoch and provider attempt.
/// A lead update or a new provider attempt permits another bounded set of tries.
fn result_refusal(
    tx: &Transaction<'_>,
    work: &WorkItemRecord,
    owner: &AgentRecord,
    input: &SubmitWork,
) -> Result<Option<(String, u64, Vec<Value>)>, WorkWriteError> {
    use studio_runtime_schema::task_results::{
        MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT, validate_result,
    };
    let Some(schema) = work.extension("resultSchema") else {
        return Ok(None);
    };
    let counter_id = submission_counter_id(tx, work, owner)?;
    let failed = current_result_failures(tx, work, owner)?.map_or(0, |counter| counter.count);
    let errors = if failed >= MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT {
        vec![
            json!({"path":"", "message":"Result submission limit reached for this attempt. Ask the lead to update the task before retrying."}),
        ]
    } else {
        match validate_result(schema, &input.result) {
            Ok(_) => return Ok(None),
            Err(errors) => errors,
        }
    };
    Ok(Some((counter_id, failed, errors)))
}

/// Persist current refusal accounting with its exact response in the same transaction.
/// Returning an outcome lets the caller commit the counter before reporting refusal.
#[allow(clippy::too_many_arguments)]
fn refuse_result(
    tx: &Transaction<'_>,
    work: &WorkItemRecord,
    owner: &AgentRecord,
    counter_id: &str,
    failed: u64,
    errors: Vec<Value>,
    _request_id: Option<&str>,
    _signature: &str,
) -> Result<WorkWriteResult, WorkWriteError> {
    use studio_runtime_schema::task_results::MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT;
    let next = failed
        .saturating_add(1)
        .min(MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT);
    let mut current = work.clone();
    if failed < MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT {
        current
            .set(
                "resultFailures",
                studio_runtime_store::projection_value(&ResultFailures {
                    version: work
                        .version()
                        .and_then(studio_json::Number::as_i64)
                        .ok_or_else(|| invalid("runtime_work", work.id().as_ref()))?,
                    owner: owner.id.0.clone(),
                    epoch: owner.epoch,
                    provider_attempt: WorkItemsRepository::submission_identity(tx, owner)?,
                    count: next,
                    errors: errors.clone(),
                }),
            )
            .map_err(studio_runtime_store::StoreError::from)?;
        WorkItemsRepository::update_checked_work(tx, work.id().as_ref(), &current)?;
    }
    let refusal = submission_refusal_message(errors, next, MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT);
    let mut event_inserted = false;
    if failed < MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT
        && next == MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT
    {
        let lead = root_agent(tx, owner)?;
        event_inserted = event_write::enqueue_recovery_event(
            tx,
            &lead,
            format!("{counter_id}:limit"),
            "work_result_invalid",
            refusal_notice(work.id().as_ref(), &owner.id.0, work.version(), &refusal),
            event_write::unix_time()?,
        )?;
    }
    let mut record = single_view(tx, &current)?;
    record["submissionRefusal"] = refusal;
    studio_runtime_store::entity_publication::mark_stale("work", work.id().as_ref());

    Ok(WorkWriteResult {
        record,
        event_inserted,
        artifact: None,
    })
}

fn accept(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    input: AcceptWork,
    actor_id: Option<&str>,
    _request_id: Option<&str>,
    _signature: &str,
) -> Result<WorkWriteResult, WorkWriteError> {
    if actor_id.is_some_and(|actor| actor != agent.root_id.0) {
        return Err(Error::InvalidRequest("Only the lead can accept or reject a result").into());
    }
    if input.override_reason.is_some() && agent.id != agent.root_id {
        return Err(Error::InvalidRequest("Only the lead can override an acceptance gate").into());
    }
    let work = load_work(tx, &agent.root_id.0, &input.task_id, input.version)?;
    refuse_legacy_work_participants(tx, agent, actor_id, work.owner())?;
    if let Some(owner_id) = work.owner()
        && owner_id != agent.root_id.0
    {
        let owner = event_write::checked_agent(tx, owner_id)?;
        if layr_result_needs_merge(&agent.root_id.0, &owner) {
            let latest_result_id = work
                .results()
                .and_then(|items| items.last())
                .and_then(|result| result.get("id").and_then(Value::as_str))
                .ok_or_else(|| invalid("runtime_work", &input.task_id))?;
            if input.merged_result_id.as_deref() != Some(latest_result_id) {
                return Err(Error::Conflict(
                    "The layr result must be merged before it can be accepted",
                )
                .into());
            }
        }
    }
    let statuses = dependency_statuses(tx, &work)?;
    let now = event_write::unix_time()?;
    if crate::event_write::python_strip(&input.reason).is_empty() {
        return Err(Error::InvalidRequest("Field must not be empty").into());
    }
    let root = AgentRepository::get(tx, &agent.root_id)?
        .ok_or_else(|| invalid("runtime_agents", &agent.root_id.0))?;
    let mut updated = decide_accept(
        WorkSnapshot {
            record: &work,
            dependency_statuses: &statuses,
        },
        &input.reason,
        actor_id.unwrap_or("user"),
        now,
        input.override_reason.as_deref(),
    )?;
    let result_id = updated
        .decisions()
        .and_then(|decisions| decisions.last())
        .and_then(|decision| decision.get("resultId"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("runtime_work", &input.task_id))?
        .to_owned();
    let mut ready_dependents = Vec::new();
    for dependent in WorkItemsRepository::dependent_work(tx, &agent.root_id.0, &input.task_id)? {
        let mut dep_statuses = dependency_statuses(tx, &dependent)?;
        dep_statuses.insert(input.task_id.clone(), WorkStatus::Accepted);
        if !dependent.blocked_by(&dep_statuses).is_empty() {
            continue;
        }
        let Some(owner_id) = dependent.owner() else {
            continue;
        };
        let recipient = AgentRepository::get(tx, &AgentId::from(owner_id))?
            .ok_or_else(|| invalid("runtime_agents", owner_id))?;
        event_write::refuse_remote(&recipient)?;
        ready_dependents.push((recipient, dependent));
    }
    WorkItemsRepository::close_review_event(tx, &agent.root_id.0, &result_id)?;
    let mut event_inserted = false;
    if let Some(owner_id) = work.owner()
        && owner_id != actor_id.unwrap_or("")
        && (actor_id.is_some() || owner_id == agent.root_id.0)
    {
        event_inserted |= event_write::enqueue_recovery_event(
            tx,
            &root,
            format!(
                "work-decision:{}:{}",
                input.task_id,
                work.version()
                    .and_then(studio_json::Number::as_i64)
                    .unwrap_or_default()
            ),
            "work_decision",
            decision_notice(&input.task_id, "accept", Some(&input.reason)),
            now,
        )?;
    }
    for (recipient, dependent) in ready_dependents {
        event_inserted |= event_write::enqueue_recovery_event(
            tx,
            &recipient,
            format!("work-ready:{}:{}", dependent.id().as_ref(), input.task_id),
            "work_ready",
            assignment_payload(&dependent.value()).to_string(),
            now,
        )?;
    }
    let archive_owner = work.owner().filter(|owner| *owner != agent.root_id.0);
    if let Some(owner) = archive_owner {
        let archive_intent_id = format!("{}:{result_id}", input.task_id);
        updated
            .set(
                "archiveIntent",
                json!({"status":WorkEffectStatus::Pending,"taskId":input.task_id,
            "intentId":archive_intent_id,"actor":agent.root_id.0,"actorEpoch":root.epoch,
            "target":owner,"attempts":0,"created":now,
            "nextAttemptAt":now + ACCEPTED_ARCHIVE_MAX_DELAY_SECONDS as f64}),
            )
            .map_err(studio_runtime_store::StoreError::from)?;
    }
    WorkItemsRepository::update_checked_work(tx, &input.task_id, &updated)?;
    let mut record = single_view(tx, &updated)?;
    record["archivePending"] = json!(archive_owner.is_some());
    if let Some(LayrWorkIntent::Completed {
        owner,
        result: AcceptanceResult { merge: Some(merge) },
    }) = read_layr_view(tx, &input.task_id)?.accept
        && owner.id.result_id == result_id
    {
        record["merge"] = json!(merge);
    }

    Ok(WorkWriteResult {
        record,
        event_inserted,
        artifact: None,
    })
}

fn load_work(
    tx: &Transaction<'_>,
    root: &str,
    task_id: &str,
    expected_version: Option<i64>,
) -> Result<WorkItemRecord, Error> {
    let work = WorkItemsRepository::work_record(tx, task_id)?
        .ok_or(Error::InvalidRequest("Unknown work item"))?;
    if work.root_id() != Some(root) {
        return Err(invalid("runtime_work", task_id));
    }
    let version = work
        .version()
        .and_then(studio_json::Number::as_i64)
        .ok_or_else(|| invalid("runtime_work", task_id))?;
    if expected_version.is_some_and(|expected| expected != version) {
        return Err(Error::Conflict(
            "This work item changed. Reload before editing",
        ));
    }
    Ok(work)
}

fn dependency_statuses(
    tx: &Connection,
    work: &WorkItemRecord,
) -> studio_runtime_store::StoreResult<BTreeMap<String, WorkStatus>> {
    let checked = work;
    let mut statuses = BTreeMap::from([(checked.id().0.clone(), checked.status())]);
    for dependency in checked.dependencies() {
        if let Some(value) = WorkItemsRepository::work_record(tx, dependency)?
            && value.root_id() == work.root_id()
        {
            statuses.insert(dependency.to_owned(), value.status());
        }
    }
    Ok(statuses)
}

fn new_id() -> Result<String, Error> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| {
        Error::Store(studio_runtime_store::StoreError::StateDirectory(
            std::io::Error::other("secure random generator failed"),
        ))
    })?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string())
}

fn push_decision(work: &mut Value, decision: Value) -> Result<(), Error> {
    let id = work["id"].as_str().unwrap_or("<work>").to_owned();
    work["decisions"]
        .as_array_mut()
        .ok_or_else(|| invalid("runtime_work", &id))?
        .push(decision);
    Ok(())
}

pub fn work_item_view(
    connection: &Connection,
    work: &WorkItemRecord,
) -> studio_runtime_store::StoreResult<Value> {
    let statuses = dependency_statuses(connection, work)?;
    Ok(crate::work_policy::work_item_view(work, &statuses))
}

fn single_view(tx: &Transaction<'_>, work: &WorkItemRecord) -> Result<Value, Error> {
    Ok(work_item_view(tx, work)?)
}

fn validate_dependencies(
    tx: &Transaction<'_>,
    root: &str,
    task: &str,
    deps: &[String],
) -> Result<(), Error> {
    if deps.iter().any(|id| id == task) {
        return Err(Error::InvalidRequest("Task dependencies contain a cycle"));
    }
    for id in deps {
        let item = WorkItemsRepository::work_record(tx, id)?
            .ok_or(Error::InvalidRequest("Dependency is outside this team"))?;
        if item.root_id() != Some(root) {
            return Err(Error::InvalidRequest("Dependency is outside this team"));
        }
    }
    let mut active = vec![task.to_owned()];
    let mut visited = BTreeMap::new();
    for id in deps {
        dependency_visit(tx, root, id, &mut active, &mut visited)?;
    }
    Ok(())
}

fn dependency_visit(
    tx: &Transaction<'_>,
    root: &str,
    task: &str,
    active: &mut Vec<String>,
    visited: &mut BTreeMap<String, ()>,
) -> Result<(), Error> {
    if active.iter().any(|id| id == task) {
        return Err(Error::InvalidRequest("Task dependencies contain a cycle"));
    }
    if visited.contains_key(task) {
        return Ok(());
    }
    let item = WorkItemsRepository::work_record(tx, task)?
        .ok_or(Error::InvalidRequest("Dependency is outside this team"))?;
    if item.root_id() != Some(root) {
        return Err(Error::InvalidRequest("Dependency is outside this team"));
    }
    active.push(task.to_owned());
    let checked = item;
    for dependency in checked.dependencies() {
        dependency_visit(tx, root, dependency, active, visited)?;
    }
    active.pop();
    visited.insert(task.to_owned(), ());
    Ok(())
}

fn unique(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeMap::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone(), ()).is_none())
        .collect()
}

fn check_owner(
    tx: &Transaction<'_>,
    agent: &AgentRecord,
    owner: Option<&str>,
) -> Result<(), Error> {
    if let Some(owner) = owner {
        let candidate = event_write::checked_agent(tx, owner)?;
        if candidate.root_id != agent.root_id {
            return Err(Error::InvalidRequest("Owner belongs to another team"));
        }
        if owner != agent.root_id.0 {
            event_write::assert_delegation(&root_agent(tx, agent)?)?;
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignmentPayload {
    task: RawJson,
    title: RawJson,
    description: RawJson,
    #[serde(skip_serializing_if = "Field::is_missing")]
    result_schema: Field<RawJson>,
    #[serde(skip_serializing_if = "Field::is_missing")]
    result_instructions: Field<String>,
}

fn assignment_payload(work: &Value) -> Value {
    let raw =
        |value: &Value| RawJson(studio_json::to_raw_value(value).expect("JSON value serializes"));
    let empty = json!("");
    let schema = work.get("resultSchema");
    studio_runtime_store::projection_value(&AssignmentPayload {
        task: raw(&work["id"]),
        title: raw(&work["title"]),
        description: raw(work.get("description").unwrap_or(&empty)),
        result_schema: schema.map_or(Field::Missing, |value| Field::Value(raw(value))),
        result_instructions: schema.map_or(Field::Missing, |_| {
            Field::Value(studio_runtime_schema::task_results::assignment_text(work))
        }),
    })
}

fn queue_assignment(
    tx: &Transaction<'_>,
    recipient: &AgentRecord,
    work: &Value,
) -> Result<bool, Error> {
    let id = work["id"]
        .as_str()
        .ok_or_else(|| invalid("runtime_work", "<invalid>"))?;
    let version = work["version"]
        .as_i64()
        .ok_or_else(|| invalid("runtime_work", id))?;
    work["title"]
        .as_str()
        .ok_or_else(|| invalid("runtime_work", id))?;
    let updated = work["updated"]
        .as_f64()
        .ok_or_else(|| invalid("runtime_work", id))?;
    event_write::enqueue_recovery_event(
        tx,
        recipient,
        format!("work-ready:{id}:assignment:{version}"),
        "work_ready",
        assignment_payload(work).to_string(),
        updated,
    )
}

fn root_agent(tx: &Transaction<'_>, agent: &AgentRecord) -> Result<AgentRecord, Error> {
    if agent.id == agent.root_id {
        return Ok(agent.clone());
    }
    AgentRepository::get(tx, &agent.root_id)?.ok_or(Error::InvalidRequest("Invalid request"))
}

fn live_recipient(agent: &AgentRecord) -> bool {
    !agent.is_deleted()
        && !matches!(
            agent.status,
            studio_runtime_store::AgentStatus::Completed
                | studio_runtime_store::AgentStatus::Failed
        )
}

fn is_ready(view: &Value) -> bool {
    matches!(view["status"].as_str(), Some("ready" | "running"))
        && view["blockedBy"].as_array().is_some_and(Vec::is_empty)
}

fn text(value: &str, empty: bool) -> Result<String, Error> {
    let value = value.trim();
    if !empty && value.is_empty() {
        return Err(Error::InvalidRequest("Invalid request"));
    }
    Ok(value.to_owned())
}

fn validate_submission_manifest(files: &[String]) -> Result<(), Error> {
    if files.len() > TASK_SUBMISSION_MAX_FILES
        || files
            .iter()
            .any(|path| path.chars().count() > TASK_SUBMISSION_PATH_MAX_CODEPOINTS)
    {
        return Err(Error::InvalidRequest(
            "Submission files exceed shared count or path-length limits",
        ));
    }
    Ok(())
}

fn validate_submission_files(
    agent: &AgentRecord,
    files: &[String],
    prepared: Option<&PreparedSubmissionFiles>,
) -> Result<(), Error> {
    validate_submission_manifest(files)?;
    if files.is_empty() {
        return Ok(());
    }
    let cwd = agent
        .cwd
        .as_deref()
        .filter(|cwd| !cwd.is_empty())
        .ok_or_else(|| invalid("runtime_agents", &agent.id.0))?;
    let Some(prepared) = prepared else {
        return Err(Error::Conflict(
            "Submission file evidence must be prepared before saving the result",
        ));
    };
    if prepared.owner != agent.id.0
        || prepared.cwd.as_deref() != Some(cwd)
        || prepared.files != files
    {
        return Err(Error::Conflict(
            "Submission file ownership changed before saving the result",
        ));
    }
    if prepared
        .resolved
        .as_ref()
        .is_none_or(|resolved| resolved.len() != files.len())
    {
        return Err(Error::InvalidRequest("Invalid request"));
    }
    Ok(())
}

fn resolve_submission_files(
    agent: &AgentRecord,
    files: &[String],
) -> Result<Vec<CanonicalPath>, Error> {
    validate_submission_manifest(files)?;
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let cwd = agent
        .cwd
        .as_deref()
        .filter(|cwd| !cwd.is_empty())
        .ok_or_else(|| invalid("runtime_agents", &agent.id.0))?;
    let mut resolved = Vec::with_capacity(files.len());
    for raw_path in files {
        let path = python_bounded_text(raw_path, TASK_SUBMISSION_PATH_MAX_CODEPOINTS)?;
        let expanded = if path == "~" || path.starts_with("~/") {
            let home = std::env::var_os("HOME").ok_or(Error::InvalidRequest("Invalid request"))?;
            PathBuf::from(home).join(
                path.strip_prefix('~')
                    .unwrap_or_default()
                    .trim_start_matches('/'),
            )
        } else {
            PathBuf::from(path)
        };
        let candidate = if expanded.is_absolute() {
            expanded
        } else {
            Path::new(cwd).join(expanded)
        };
        resolved.push(
            CanonicalPath::weak_missing_tail(&candidate)
                .map_err(|_| Error::InvalidRequest("Invalid request"))?,
        );
    }
    Ok(resolved)
}

fn python_bounded_text(value: &str, maximum: usize) -> Result<String, Error> {
    let value = event_write::python_strip(value);
    if value.is_empty() || value.chars().count() > maximum {
        return Err(Error::InvalidRequest("Invalid request"));
    }
    Ok(value.to_owned())
}

fn work_id(created: f64) -> String {
    format!(
        "work-{}-{}",
        (created * 1_000_000_000.0) as u128,
        GENERATED_WORK_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn invalid(object: &'static str, id: &str) -> Error {
    Error::InvalidStoredObject {
        object,
        id: id.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod artifact_tests {
    use super::{ResultArtifact, write_result_artifact};
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn replay_of_fifo_is_bounded_and_does_not_leave_a_temporary_file() {
        use std::sync::mpsc;
        use std::time::Duration;

        let temporary = tempfile::tempdir().expect("temporary root opens");
        let state = temporary.path().join("state");
        let parent = state.join("results/task");
        fs::create_dir_all(&parent).expect("artifact parent creates");
        let status = std::process::Command::new("/usr/bin/mkfifo")
            .args(["-m", "600"])
            .arg(parent.join("result.md"))
            .status()
            .expect("FIFO creation starts");
        assert!(status.success(), "FIFO artifact creates");
        let artifact = ResultArtifact {
            state_dir: state,
            path: parent.join("result.md"),
            body: "committed result".to_owned(),
        };
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            drop(sender.send(write_result_artifact(&artifact)));
        });
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("FIFO replay returns without blocking")
            .expect_err("FIFO is not a valid result artifact");
        let entries: Vec<_> = fs::read_dir(&parent)
            .expect("artifact parent lists")
            .map(|entry| entry.expect("directory entry reads").file_name())
            .collect();
        assert_eq!(entries, [std::ffi::OsString::from("result.md")]);
    }

    #[cfg(unix)]
    #[test]
    fn failed_artifact_replay_removes_its_temporary_file() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root opens");
        let state = temporary.path().join("state");
        let parent = state.join("results/task");
        let outside = temporary.path().join("outside.md");
        fs::create_dir_all(&parent).expect("artifact parent creates");
        fs::write(&outside, "foreign content").expect("outside file writes");
        symlink(&outside, parent.join("result.md")).expect("artifact symlink creates");
        let artifact = ResultArtifact {
            state_dir: state,
            path: parent.join("result.md"),
            body: "committed result".to_owned(),
        };

        assert!(write_result_artifact(&artifact).is_err());
        let entries: Vec<_> = fs::read_dir(&parent)
            .expect("artifact parent lists")
            .map(|entry| entry.expect("directory entry reads").file_name())
            .collect();
        assert_eq!(entries, [std::ffi::OsString::from("result.md")]);
    }

    #[cfg(unix)]
    #[test]
    fn result_artifact_writer_is_anchored_to_open_directory_handles() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary root opens");
        let state = temporary.path().join("state");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&state).expect("state directory creates");
        fs::create_dir_all(&outside).expect("outside directory creates");
        let artifact_path = state.join("results/task/result-1.md");
        let artifact = ResultArtifact {
            state_dir: state.clone(),
            path: artifact_path.clone(),
            body: "committed result".to_owned(),
        };
        write_result_artifact(&artifact).expect("safe artifact writes");
        assert_eq!(
            fs::read(&artifact_path).expect("artifact reads"),
            b"committed result"
        );

        fs::remove_dir_all(state.join("results")).expect("result tree removes");
        symlink(&outside, state.join("results")).expect("outside symlink creates");
        let escaped = ResultArtifact {
            state_dir: state,
            path: artifact_path,
            body: "must stay contained".to_owned(),
        };
        assert!(write_result_artifact(&escaped).is_err());
        assert!(!outside.join("task/result-1.md").exists());
    }
}

#[cfg(test)]
mod actor_tests {
    #[cfg(not(unix))]
    use super::SubmitWork;
    use super::{
        AcceptWork, ClaimWork, CreateWork, WorkActorWrite, WorkMutation, WorkWrite, WorkWriteError,
        accepted_archive_due_records, accepted_archive_tick, next_archive_retry_at,
        write_action_as_agent, write_as_agent,
    };
    use rusqlite::Connection;
    use std::sync::Arc;
    use studio_json::{Value, json};
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::{
        AgentId, AgentRecord, AgentRepository, RuntimeStore, WorkItemsRepository,
    };

    fn database() -> Connection {
        let connection = studio_runtime_store::test_canvas();
        for agent in [
            json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":1,"status":"running","autoWake":true,"inFlight":true,"agentMode":"multi","concurrency":8}),
            json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"worker","rootId":"lead","epoch":2,"status":"running","autoWake":true,"inFlight":true,"agentMode":"multi","concurrency":8}),
        ] {
            connection
                .execute(
                    "INSERT INTO runtime_agents(id,record) VALUES (?1,?2)",
                    rusqlite::params![agent["id"].as_str().expect("id"), agent.to_string()],
                )
                .expect("agent row inserts");
        }
        connection
    }

    #[test]
    fn legacy_result_identity_keeps_unused_fields_in_persisted_row() {
        let connection = database();
        let result = json!({"id":"r1","revision":{"legacy":true},"files":false,
            "result":{"nested":[1,null]},"extra":{"keep":true}});
        let item = work("legacy-identity", Some("worker"), "review", json!([result]));
        insert_work(&connection, &item);
        let saved = WorkItemsRepository::work_record(&connection, "legacy-identity")
            .expect("read row")
            .expect("row exists");
        let identity = super::result_identity(&saved).expect("identity");
        assert_eq!(identity.id.as_deref(), Some("r1"));
        assert_eq!(identity.revision, None);
        assert_eq!(saved.value()["results"], item["results"]);
    }

    #[test]
    fn assignment_payload_bytes_match_legacy_wire() {
        let work = json!({"id":"t1","title":"Task","description":"Notes"});
        assert_eq!(
            super::assignment_payload(&work).to_string(),
            r#"{"description":"Notes","task":"t1","title":"Task"}"#
        );
    }

    #[test]
    fn reviewer_child_result_legacy_payload_bytes_and_identity() {
        let encoded = r#"{"agent_id":"child","extra":{"malformed":[false,3]},"result":"done"}"#;
        let payload: Value = studio_json::from_str(encoded).expect("payload");
        assert_eq!(
            payload.get("agent_id").and_then(Value::as_str),
            Some("child")
        );
        assert_eq!(payload.to_string(), encoded);
        let identity: super::ChildResultIdentity =
            studio_json::from_str(encoded).expect("identity");
        assert_eq!(identity.agent_id.as_deref(), Some("child"));
    }

    #[test]
    fn reviewer_schedule_preserves_record_bytes() {
        let seed = json!({"compactions":0,"accountKey":"default","provider":"codex",
            "id":"child","rootId":"lead","parentId":"lead","epoch":2,"status":"completed",
            "role":"reviewer","extraLegacy":{"malformed":[false,3]}});
        let mut updated = AgentRecord::from_value(seed).expect("agent");
        let mut record = updated.value();
        record["reviewArchiveScheduled"] = json!("child:result-1");
        let expected = record.to_string();
        updated
            .extra
            .insert("reviewArchiveScheduled".into(), json!("child:result-1"));
        assert_eq!(updated.value().to_string(), expected);
    }

    #[test]
    fn reviewer_retry_preserves_record_bytes() {
        let seed = json!({"compactions":0,"accountKey":"default","provider":"codex",
            "id":"child","rootId":"lead","parentId":"lead","epoch":2,"status":"completed",
            "role":"reviewer","reviewArchiveScheduled":"child:result-1","extraLegacy":{"x":false}});
        let mut updated = AgentRecord::from_value(seed).expect("agent");
        let mut record = updated.value();
        record["reviewArchiveError"] = json!("blocked");
        record["reviewArchiveAttempts"] = json!(1);
        record["reviewArchiveNextAt"] = json!(12.0);
        let expected = record.to_string();
        updated
            .extra
            .insert("reviewArchiveError".into(), json!("blocked"));
        updated
            .extra
            .insert("reviewArchiveAttempts".into(), json!(1));
        updated
            .extra
            .insert("reviewArchiveNextAt".into(), json!(12.0));
        assert_eq!(updated.value().to_string(), expected);
    }

    #[test]
    fn reviewer_retry_clear_preserves_record_bytes() {
        let seed = json!({"compactions":0,"accountKey":"default","provider":"codex",
            "id":"child","rootId":"lead","parentId":"lead","epoch":2,"status":"completed",
            "role":"reviewer","reviewArchiveScheduled":"child:result-1",
            "reviewArchiveError":"blocked","reviewArchiveAttempts":1,"reviewArchiveNextAt":12.0,
            "extraLegacy":{"x":false}});
        let mut updated = AgentRecord::from_value(seed).expect("agent");
        let mut record = updated.value();
        for key in [
            "reviewArchiveError",
            "reviewArchiveScheduled",
            "reviewArchiveAttempts",
            "reviewArchiveNextAt",
        ] {
            record.as_object_mut().expect("object").remove(key);
        }
        let expected = record.to_string();
        for key in [
            "reviewArchiveError",
            "reviewArchiveScheduled",
            "reviewArchiveAttempts",
            "reviewArchiveNextAt",
        ] {
            updated.extra.remove(key);
        }
        assert_eq!(updated.value().to_string(), expected);
    }

    #[test]
    fn reviewer_blockers_keep_legacy_bytes() {
        let outcome =
            json!({"status":"blocked","blockers":[{"kind":"turn","count":1,"ids":["worker"]}]});
        let view: super::ReviewerArchiveOutcome =
            studio_json::from_str(&outcome.to_string()).expect("outcome");
        let raw = view.blockers.expect("blockers");
        let blockers: Vec<super::RawJson> = studio_json::from_str(raw.0.as_str()).expect("array");
        assert!(!blockers.is_empty());
        assert_eq!(
            studio_json::to_string(&blockers).expect("blockers JSON"),
            r#"[{"count":1,"ids":["worker"],"kind":"turn"}]"#
        );
    }

    #[test]
    fn work_decision_notification_bytes_match_legacy_wire() {
        let event = super::decision_notice("t1", "accept", Some("approved"));
        assert_eq!(
            event,
            r#"{"decision":"accept","reason":"approved","task":"t1"}"#
        );
        let nullable = super::decision_notice("t1", "accept", None);
        assert_eq!(
            nullable,
            r#"{"decision":"accept","reason":null,"task":"t1"}"#
        );
    }

    #[test]
    fn work_review_and_refusal_notification_bytes_match_legacy_wire() {
        let result = json!({"id":"r1","result":{"unknown":true}});
        let review = super::work_review_notice("t1", Some("Task"), &result);
        assert_eq!(
            review,
            r#"{"result":{"id":"r1","result":{"unknown":true}},"task":"t1","title":"Task"}"#
        );
        let refusal = json!({"errors":[],"failedSubmits":1});
        let version = studio_json::Number::from(2);
        let event = super::refusal_notice("t1", "worker", Some(&version), &refusal);
        assert_eq!(
            event,
            r#"{"owner":"worker","refusal":{"errors":[],"failedSubmits":1},"task":"t1","version":2}"#
        );
    }

    #[test]
    fn submission_refusal_view_bytes_match_legacy_wire() {
        let connection = database();
        let item = work("refusal-bytes", None, "running", json!([]));
        let result = super::current_submission_refusal(
            &connection,
            &super::WorkItemRecord::checked(&item).expect("work"),
        )
        .expect("refusal view");
        assert_eq!(
            result.to_string(),
            format!(
                r#"{{"errors":[],"failedSubmits":0,"limit":{},"retryAllowed":true}}"#,
                studio_runtime_schema::task_results::MAX_FAILED_RESULT_SUBMITS_PER_ATTEMPT
            )
        );
    }

    #[test]
    fn submission_counter_id_matches_legacy_identity_bytes() {
        let connection = database();
        let item = work("counter-bytes", Some("worker"), "running", json!([]));
        let owner = AgentRepository::get(&connection, &AgentId::from("worker"))
            .expect("owner read")
            .expect("owner");
        let attempt =
            WorkItemsRepository::submission_identity(&connection, &owner).expect("identity");
        let identity = json!([
            item["id"],
            item["version"],
            owner.id.0,
            owner.epoch,
            attempt
        ]);
        let expected = format!(
            "work-result-failures:{}",
            studio_runtime_store::sha256_hex(identity.to_string().as_bytes())
        );
        assert_eq!(
            super::submission_counter_id(
                &connection,
                &super::WorkItemRecord::checked(&item).expect("work"),
                &owner
            )
            .expect("counter"),
            expected
        );
    }

    #[test]
    fn submission_refusal_message_bytes_match_legacy_wire() {
        let errors = json!([{"path":"/count","message":"invalid"}]);
        let refusal =
            super::submission_refusal_message(errors.as_array().expect("errors").clone(), 1, 3);
        assert_eq!(
            refusal.to_string(),
            r#"{"errors":[{"message":"invalid","path":"/count"}],"failedSubmits":1,"limit":3,"message":"Result does not satisfy the task result contract","retryAllowed":true}"#
        );
    }

    #[tokio::test]
    async fn native_reviewer_submission_needs_no_revision_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(dir.path())).await.unwrap();
        let record = json!({"compactions":0,"accountKey":"default","provider":"codex","id":"reviewer","rootId":"lead","epoch":1,"status":"running","autoWake":true,"role":"reviewer","workspaceMode":"layr-native"});
        let lead = json!({"compactions":0,"accountKey":"default","provider":"codex","id":"lead","rootId":"lead","epoch":1,"status":"running","autoWake":true,"isLead":true,"agentMode":"multi","concurrency":8});
        let item = work("review", Some("reviewer"), "running", json!([]));
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(record)?)?;
                AgentRepository::put(tx, &AgentRecord::from_value(lead)?)?;
                WorkItemsRepository::put_work(tx, "review", &item)?;
                Ok(())
            })
            .await
            .unwrap();
        let mut input = super::SubmitWork {
            prepared_gate_workspace: None,
            task_id: "review".into(),
            version: Some(2),
            result: "reviewed".into(),
            checks: "read".into(),
            revision: String::new(),
            files: vec![],
            prepared_files: None,
            state_dir: dir.path().into(),
            layr_evidence: None,
            layr_evidence_request_id: None,
        };
        let result = super::prepare_layr_submission(&store, "reviewer", None, "review", &mut input)
            .await
            .unwrap();
        assert!(matches!(result, super::LayrSubmissionPreparation::NotLayr));
        let result = store
            .transaction_with_error(move |tx| {
                super::write(tx, "reviewer", None, "review", WorkWrite::Submit(input))
            })
            .await
            .unwrap();
        assert_eq!(result.record["status"], "review");
    }

    #[test]
    fn receipt_domain_invalid_work_root_reports_fieldless_expiry() {
        let connection = database();
        assert!(matches!(
            super::work_receipt_in(&connection, "lead", "invalid-root", "signature"),
            Err(super::WorkWriteError::Event(
                crate::event_write::EventWriteError::RequestResult(
                    studio_runtime_store::request_results::RequestResultError::Expired
                )
            ))
        ));
    }

    #[test]
    fn receipt_domain_skipped_archive_is_in_first_reply_and_replay() {
        for owner in [Some("lead"), None] {
            let mut connection = database();
            insert_work(
                &connection,
                &work(
                    "skip",
                    owner,
                    "review",
                    json!([
                        {"id":"result-1","agent":"lead","text":"done","checks":"checked","revision":"abc1234","files":[]}
                    ]),
                ),
            );
            let root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
            let tx = connection.transaction().expect("transaction");
            let accept = || {
                WorkWrite::Accept(AcceptWork {
                    task_id: "skip".into(),
                    version: Some(2),
                    reason: "verified".into(),
                    override_reason: None,
                    merged_result_id: None,
                })
            };
            let first = super::write(&tx, "lead", Some(&root), "accept", accept())
                .expect("accept")
                .record;
            let replay = super::write(&tx, "lead", Some(&root), "accept", accept())
                .expect("replay")
                .record;
            assert_eq!(first["archive"]["status"], "skipped");
            assert_eq!(first, replay);
        }
    }

    #[tokio::test]
    async fn receipt_domain_layr_accept_retry_checks_archive_reservation_before_version() {
        let dir = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(dir.path())).await.expect("store");
        let root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let request = root.clone();
        store.transaction_with_error(move |tx| -> Result<(), super::WorkWriteError> {
            for agent in [
                json!({"compactions":0,"accountKey":"default","provider":"codex","id":"lead","rootId":"lead","epoch":1,"status":"running","autoWake":true,"isLead":true,"executionMode":"native","workspaceMode":"layr-native"}),
                json!({"compactions":0,"accountKey":"default","provider":"codex","id":"worker","rootId":"lead","epoch":2,"status":"idle","autoWake":true,"executionMode":"native","workspaceMode":"layr-native"}),
            ] { AgentRepository::put(tx, &AgentRecord::from_value(agent).expect("agent"))?; }
            let mut item = work("retry", Some("worker"), "review", json!([{"id":"result-1","agent":"worker","text":"done","checks":"checked","revision":"state","files":[]}]));
            item["vmAccept"] = json!({"status":"completed","requestId":"merge","taskId":"retry","owner":"worker","revision":"state","resultId":"result-1","created":1.0,"merge":{"from_state":"old","to_state":"new"}});
            WorkItemsRepository::put_work(tx, "retry", &item)?;
            let accepted = super::write(tx, "lead", Some(&request), "accept", WorkWrite::Accept(AcceptWork { task_id:"retry".into(), version:Some(2), reason:"verified".into(), override_reason:None, merged_result_id:Some("result-1".into()) }))?;
            assert_eq!(accepted.record["merge"], json!({"from_state":"old","to_state":"new"}));
            let saved = WorkItemsRepository::work_record(tx, "retry")?.unwrap().value();
            assert_eq!(saved["archiveIntent"]["publicResponse"]["merge"], accepted.record["merge"]);
            Ok(())
        }).await.expect("accepted pending archive");
        let input = AcceptWork {
            task_id: "retry".into(),
            version: Some(2),
            reason: "verified".into(),
            override_reason: None,
            merged_result_id: None,
        };
        assert!(matches!(
            super::prepare_layr_accept(&store, "lead", Some(&root), "accept", &input).await,
            Ok(super::LayrAcceptPreparation::NotLayr)
        ));
    }

    #[tokio::test]
    async fn receipt_domain_unknown_archive_keeps_public_reservation() {
        let dir = tempfile::tempdir().expect("state");
        let store = Arc::new(RuntimeStore::open(Some(dir.path())).await.expect("store"));
        let root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let request = root.clone();
        store.transaction_with_error(move |tx| -> Result<(), super::WorkWriteError> {
            for agent in [
                json!({"compactions":0,"accountKey":"default","provider":"codex","id":"lead","rootId":"lead","epoch":1,"status":"running","autoWake":true}),
                json!({"compactions":0,"accountKey":"default","provider":"codex","id":"worker","rootId":"lead","epoch":2,"status":"idle","autoWake":true,"executionMode":"native","workspaceMode":"layr-native"}),
            ] { AgentRepository::put(tx, &AgentRecord::from_value(agent).expect("fixture agent"))?; }
            let mut item = work("unknown", Some("worker"), "review", json!([{"id":"result-1","agent":"worker","text":"done","checks":"checked","revision":"state","files":[]}]));
            item["vmAccept"] = json!({"status":"completed","requestId":"merge","taskId":"unknown","owner":"worker","revision":"state","resultId":"result-1","created":1.0});
            WorkItemsRepository::put_work(tx, "unknown", &item)?;
            super::write(tx, "lead", Some(&request), "accept", WorkWrite::Accept(AcceptWork { task_id:"unknown".into(), version:Some(2), reason:"verified".into(), override_reason:None, merged_result_id:Some("result-1".into()) }))?;
            Ok(())
        }).await.expect("accepted");
        super::accepted_archive_tick(Arc::clone(&store), 4_000_000_000.0)
            .await
            .expect("archive tick");
        store
            .read_async(
                "archive-proof",
                move |db| -> Result<(), super::WorkWriteError> {
                    let item = WorkItemsRepository::work_record(db, "unknown")?.expect("work");
                    assert_eq!(item.value()["archiveIntent"]["status"], "unknown");
                    assert!(super::work_receipt_in(db, "lead", &root, "accept")?.is_none());
                    assert!(
                        item.value()["archiveIntent"]
                            .get("publicAdmission")
                            .is_some()
                    );
                    Ok(())
                },
            )
            .await
            .expect("pending proof");
    }

    #[tokio::test]
    async fn receipt_domain_archive_finished_by_another_caller_does_not_return_stale_pending() {
        let dir = tempfile::tempdir().expect("state");
        let store = Arc::new(RuntimeStore::open(Some(dir.path())).await.expect("store"));
        let mut response = work("finished", Some("worker"), "accepted", json!([]));
        response["archivePending"] = json!(true);
        let pending =
            studio_runtime_store::WorkItemRecord::checked(&response).expect("public response");
        store
            .transaction_with(move |tx| WorkItemsRepository::put_work(tx, "finished", &response))
            .await
            .expect("finished work");
        assert!(matches!(
            super::finalize_accepted_archive(store, pending).await,
            Err(super::WorkWriteError::Event(super::Error::Conflict(
                "The accepted archive outcome is pending"
            )))
        ));
    }

    #[test]
    fn vm_submission_completed_requires_evidence() {
        let missing = r#"{"status":"completed","requestId":"evidence","taskId":"task","owner":"worker","revision":"state","version":2,"created":1.0}"#;
        assert!(
            studio_json::from_str::<
                super::LayrWorkIntent<super::Submission, super::LayrSubmissionResult>,
            >(missing)
            .is_err()
        );
    }

    #[test]
    fn vm_submission_migrated_unknown_round_trips() {
        let stored =
            r#"{"status":"unknown","requestId":"rpc","owner":"a","revision":"rev","version":1}"#;
        let intent: super::LayrWorkIntent<super::Submission, super::LayrSubmissionResult> =
            studio_json::from_str(stored).expect("migrated intent");
        assert_eq!(
            studio_runtime_store::projection_value(&intent),
            studio_json::from_str::<Value>(stored).expect("fixture")
        );
    }

    #[test]
    fn receipt_domain_refusal_reads_current_owner_without_receipt_history() {
        let connection = database();
        let owner = AgentRecord::from_value(json!({"compactions":0,"accountKey":"default",
            "id":"worker", "rootId":"lead", "provider":"codex", "epoch":3,
            "status":"idle"
        }))
        .expect("owner");
        AgentRepository::put(&connection, &owner).expect("owner saved");
        let mut current = work("task", Some("worker"), "running", json!([]));
        current["resultFailures"] = json!({
            "version":current["version"],"owner":"worker","epoch":3,
            "providerAttempt":null,"count":2,"errors":[{"path":"$.result","message":"required"}]
        });
        let view = super::current_submission_refusal(
            &connection,
            &super::WorkItemRecord::checked(&current).expect("work"),
        )
        .expect("projection");
        assert_eq!(view["failedSubmits"], 2);
        assert_eq!(view["errors"], current["resultFailures"]["errors"]);
        current["version"] = json!(current["version"].as_i64().expect("version") + 1);
        assert_eq!(
            super::current_submission_refusal(
                &connection,
                &super::WorkItemRecord::checked(&current).expect("work")
            )
            .expect("new version")["failedSubmits"],
            0
        );
    }

    #[test]
    fn legacy_work_reply_replay_preserves_unchecked_shell_and_bytes() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        let root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let token =
            match crate::event_write::begin_request::<studio_runtime_store::LegacyWorkReply>(
                &tx,
                crate::event_write::RequestKey::new("work", &["lead"], &root),
                "legacy",
                (crate::event_write::unix_time().unwrap() * 1000.0) as i64,
            )
            .unwrap()
            {
                crate::event_write::CallerAdmission::Fresh(token) => token,
                crate::event_write::CallerAdmission::Replay(_) => panic!("fresh receipt"),
            };
        let value =
            json!({"id":"legacy","status":"ready","results":null,"version":-1,"future":{"x":1}});
        let reply = studio_runtime_store::LegacyWorkReply::from_value(value.clone()).unwrap();
        crate::event_write::settle_request(&tx, &token, None, &reply).unwrap();
        let stored = |tx: &rusqlite::Transaction<'_>| {
            tx.query_row(
                "SELECT result FROM request_results WHERE id=?1",
                [token.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        let before = stored(&tx);
        let replay = crate::event_write::replay_request::<studio_runtime_store::LegacyWorkReply>(
            &tx,
            token.id(),
            "legacy",
            (crate::event_write::unix_time().unwrap() * 1000.0) as i64,
        )
        .unwrap()
        .unwrap();
        assert_eq!(replay.reply.into_value(), value);
        assert_eq!(stored(&tx), before);
        assert_eq!(
            tx.query_row("SELECT count(*) FROM runtime_work", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn receipt_domain_archive_final_reply_uses_frozen_public_snapshot() {
        let mut db = database();
        let root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let tx = db.transaction().expect("transaction");
        let lead = AgentRepository::get(&tx, &AgentId::from("lead"))
            .expect("lead")
            .expect("owner");
        let token =
            match crate::event_write::begin_request::<studio_runtime_store::LegacyWorkReply>(
                &tx,
                crate::event_write::RequestKey::new("work", &["lead"], &root),
                "accept",
                (crate::event_write::unix_time().expect("clock") * 1000.0) as i64,
            )
            .expect("admit")
            {
                crate::event_write::CallerAdmission::Fresh(token) => token,
                crate::event_write::CallerAdmission::Replay(_) => panic!("fresh request"),
            };
        let mut current = work("accepted", Some("worker"), "accepted", json!([]));
        current["title"] = json!("Original title");
        let mut initial = current.clone();
        initial["archivePending"] = json!(true);
        // The historically admitted public snapshot only required five fields.
        initial["results"] = Value::Null;
        initial["version"] = json!(-1);
        current["archiveIntent"] = json!({"status":"unknown","taskId":"accepted","intentId":"accepted:result","actor":"lead","actorEpoch":1,"target":"worker","attempts":1,"created":1.0,"nextAttemptAt":60.0,"publicAdmission":token,"publicRequestId":root,"publicResponse":initial});
        current["title"] = json!("Current title");
        current["archive"] = json!({"status":"archived"});
        insert_work(&tx, &current);
        super::reserve_work_request(&tx, &lead, &root, "accepted", &token, true).expect("reserve");
        let mut reply = current.clone();
        reply["archivePending"] = json!(false);
        let settled = super::settle_accepted_archive(
            &tx,
            "accepted",
            &root,
            &studio_runtime_store::LegacyWorkReply::from_value(reply).expect("reply"),
        )
        .expect("settle");
        assert_eq!(settled.value()["title"], "Original title");
        assert_eq!(settled.value()["archive"]["status"], "archived");
        assert_eq!(settled.value()["archivePending"], false);
        assert_eq!(settled.value()["results"], Value::Null);
        assert_eq!(settled.value()["version"], -1);
        let saved = crate::event_write::replay_request::<studio_runtime_store::LegacyWorkReply>(
            &tx,
            token.id(),
            "accept",
            (crate::event_write::unix_time().expect("clock") * 1000.0) as i64,
        )
        .expect("replay")
        .expect("saved");
        assert_eq!(saved.reply.value(), settled.value());
        assert!(super::settle_accepted_archive(&tx, "accepted", &root, &settled).is_err());
        assert!(
            super::pending_work_request(&tx, &lead, &root, "accept")
                .expect("obligation")
                .is_none()
        );
        let owner = WorkItemsRepository::work_record(&tx, "accepted")
            .expect("owner")
            .expect("task");
        assert_eq!(owner.value()["title"], "Current title");
        assert!(
            owner.value()["archiveIntent"]
                .get("publicAdmission")
                .is_none()
        );
        tx.commit().expect("atomic result+retirement");
    }

    #[test]
    fn submission_manifest_boundaries_use_shared_limits() {
        use studio_runtime_schema::shared_values::{
            TASK_SUBMISSION_MAX_FILES, TASK_SUBMISSION_PATH_MAX_CODEPOINTS,
        };
        for count in [0, 51, 73, TASK_SUBMISSION_MAX_FILES] {
            assert!(
                super::validate_submission_manifest(&vec!["src/file.rs".into(); count]).is_ok()
            );
        }
        assert!(
            super::validate_submission_manifest(&vec![
                "src/file.rs".into();
                TASK_SUBMISSION_MAX_FILES + 1
            ])
            .is_err()
        );
        // Non-ASCII paths prove this limit counts Unicode code points, not bytes.
        let mut path = "🦀".repeat(TASK_SUBMISSION_PATH_MAX_CODEPOINTS);
        assert!(super::validate_submission_manifest(&[path.clone()]).is_ok());
        path.push('x');
        assert!(super::validate_submission_manifest(&[path]).is_err());
    }

    #[tokio::test]
    async fn receipt_domain_vm_positive_proof_survives_a_late_unknown_reply() {
        let root = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store.transaction(|tx| {
            let mut item = work("layr-known", Some("worker"), "running", json!([]));
            item["vmSubmit"] = json!({"status":"completed","requestId":"evidence","taskId":"layr-known","owner":"worker","revision":"state","version":2,"created":1.0,"evidence":{"layrProjectId":"project","layrLine":"worker","layrStateId":"state"}});
            item["vmAccept"] = json!({"status":"completed","requestId":"merge","taskId":"layr-known","owner":"worker","revision":"state","resultId":"result","created":1.0});
            WorkItemsRepository::put_work(tx,"layr-known",&item)?;
            Ok(())
        }).await.expect("positive owner proof");
        let evidence = studio_runtime_store::work_intent::Evidence {
            layr_project_id: "project".into(),
            layr_line: "worker".into(),
            layr_state_id: "state".into(),
        };
        assert_eq!(
            super::finish_layr_submission(&store, "layr-known", "evidence", Err(true))
                .await
                .expect("late reply"),
            Some(evidence.clone())
        );
        for outcome in [Err(false), Ok(evidence.clone())] {
            assert_eq!(
                super::finish_layr_submission(&store, "layr-known", "evidence", outcome)
                    .await
                    .expect("stable evidence"),
                Some(evidence.clone())
            );
        }
        assert!(
            super::finish_layr_submission(
                &store,
                "layr-known",
                "evidence",
                Ok(studio_runtime_store::work_intent::Evidence {
                    layr_state_id: "different".into(),
                    ..evidence
                })
            )
            .await
            .is_err()
        );
        super::finish_layr_accept(&store, "layr-known", "merge", Some(true), None)
            .await
            .expect("late merge reply");
        for outcome in [Some(false), None] {
            super::finish_layr_accept(&store, "layr-known", "merge", outcome, None)
                .await
                .expect("stable merge proof");
        }
        let owner = store
            .read_async("layr-proof", |db| {
                WorkItemsRepository::work_record(db, "layr-known")
            })
            .await
            .expect("read")
            .expect("task");
        assert_eq!(owner.value()["vmSubmit"]["status"], "completed");
        assert_eq!(owner.value()["vmAccept"]["status"], "completed");
    }

    #[cfg(not(unix))]
    #[test]
    fn submit_refuses_before_database_access_without_directory_relative_operations() {
        let mut connection = Connection::open_in_memory().expect("memory SQLite opens");
        let tx = connection.transaction().expect("transaction starts");
        let result = super::write(
            &tx,
            "lead",
            None,
            "submit-unsupported-platform",
            WorkWrite::Submit(SubmitWork {
                prepared_gate_workspace: None,
                task_id: "task".to_owned(),
                version: Some(1),
                result: "result".into(),
                checks: "checks".to_owned(),
                revision: "abc1234".to_owned(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: None,
                layr_evidence_request_id: None,
            }),
        );
        assert!(matches!(
            result,
            Err(WorkWriteError::NotImplemented(
                "workspace-state/submission-files"
            ))
        ));
    }

    fn insert_work(connection: &Connection, work: &Value) {
        connection
            .execute(
                "INSERT INTO runtime_work VALUES (?1,?2)",
                rusqlite::params![work["id"].as_str().expect("work id"), work.to_string()],
            )
            .expect("work row inserts");
    }

    #[test]
    fn accepted_archive_due_selection_uses_injected_clock() {
        let connection = database();
        let pending = work("archive-due", Some("worker"), "accepted", json!([]));
        insert_work(&connection, &pending);
        connection
            .execute(
                "UPDATE runtime_work SET record=json_set(record,'$.archiveIntent',json(?3)) WHERE id=json_extract(?3,'$.taskId')",
                rusqlite::params![
                    "work-archive:archive-due:result-1",
                    "accepted-archive-v1",
                    json!({"taskId":"archive-due","intentId":"archive-due:result-1",
                    "status":"pending","nextAttemptAt":120.0})
                    .to_string()
                ],
            )
            .expect("archive intent receipt inserts");

        assert!(
            accepted_archive_due_records(&connection, 119.999)
                .expect("before deadline query succeeds")
                .is_empty()
        );
        let due =
            accepted_archive_due_records(&connection, 120.0).expect("deadline query succeeds");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, "archive-due");
        assert_eq!(
            studio_json::from_str::<Value>(&due[0].1).expect("receipt JSON")["intentId"],
            "archive-due:result-1"
        );
    }

    #[tokio::test]
    async fn archive_scheduler_uses_earliest_persisted_retry_deadline() {
        let directory = tempfile::tempdir().expect("state directory");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("runtime store"),
        );
        assert_eq!(
            next_archive_retry_at(&store, 100.0, false)
                .await
                .expect("empty deadline query"),
            None
        );
        store
            .transaction(|tx| {
                for (id, due) in [("later", 88.0), ("earlier", 42.0)] {
                    tx.execute(
                        "INSERT INTO runtime_work(id,record) VALUES (?1,json_object('id',?1,'status','accepted','rootId','lead','owner','worker','title',?1,'dependencies',json('[]'),'archiveIntent',json(?2)))",
                    rusqlite::params![id, json!({"status":"retryable","nextAttemptAt":due + 100.0}).to_string()],
                    )?;
                }
                for (id, result) in [
                    ("text", json!({"status":"pending","nextAttemptAt":"101"})),
                    ("missing", json!({"status":"pending"})),
                    ("finished", json!({"status":"complete","nextAttemptAt":101})),
                    ("past", json!({"status":"unknown","nextAttemptAt":100})),
                ] {
                    tx.execute(
                        "INSERT INTO runtime_work(id,record) VALUES (?1,json_object('id',?1,'status','accepted','rootId','lead','owner','worker','title',?1,'dependencies',json('[]'),'archiveIntent',json(?2)))",
                        rusqlite::params![id, result.to_string()],
                    )?;
                }
                Ok(())
            })
            .await
            .expect("archive receipts commit");
        assert_eq!(
            next_archive_retry_at(&store, 100.0, false)
                .await
                .expect("deadline query"),
            Some(142.0)
        );
        assert_eq!(
            next_archive_retry_at(&store, 100.0, true)
                .await
                .expect("full batch deadline"),
            Some(100.0)
        );
        store.transaction(|tx| {
            tx.execute("INSERT INTO runtime_work(id,record) VALUES('large',json_object('id','large','status','accepted','rootId','lead','title','large','dependencies',json('[]'),'archiveIntent',json(?1)))",
                [r#"{"status":"pending","nextAttemptAt":9007199254740993}"#])?;
            Ok(())
        }).await.expect("large integer receipt");
        assert_eq!(
            next_archive_retry_at(&store, 9007199254740992.0, false)
                .await
                .expect("rounded deadline"),
            None
        );
    }

    #[tokio::test]
    async fn empty_archive_maintenance_does_not_commit_or_self_wake() {
        let directory = tempfile::tempdir().expect("state directory");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("runtime store"),
        );
        let commits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let commits = std::sync::Arc::clone(&commits);
            store.add_commit_observer(move || {
                commits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        assert_eq!(
            accepted_archive_tick(Arc::clone(&store), 100.0)
                .await
                .expect("maintenance pass"),
            0
        );
        assert_eq!(
            accepted_archive_tick(Arc::clone(&store), 100.0)
                .await
                .expect("second maintenance pass"),
            0
        );
        assert_eq!(commits.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn accepted_archive_tick_persists_terminal_kept_outcome_for_remote_worker() {
        let root = tempfile::tempdir().expect("state directory");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store"),
        );
        store
            .transaction(|tx| {
                for agent in [
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":1,"status":"running"}),
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"worker","rootId":"lead","epoch":2,"status":"paused",
                        "remoteWorker":{"server":"remote","link":"worker-link"}}),
                ] {
                    AgentRepository::put(tx, &AgentRecord::from_value(agent)?)?;
                }
                let work = json!({
                    "id":"accepted-task","rootId":"lead","owner":"worker",
                    "status":"accepted","version":3,"title":"Accepted result",
                    "description":"","dependencies":[],"results":[{"id":"result-1",
                        "text":"verified","revision":"abc1234"}],"decisions":[],
                    "archiveIntent":{"id":"accepted-task:result-1","owner":"worker",
                        "status":"pending","attempts":0,"created":10.0}
                });
                tx.execute(
                    "INSERT INTO runtime_work(id,record) VALUES (?1,?2)",
                    rusqlite::params!["accepted-task", work.to_string()],
                )?;
                tx.execute(
                    "UPDATE runtime_work SET record=json_set(record,'$.archiveIntent',json(?3)) WHERE id=json_extract(?3,'$.taskId')",
                    rusqlite::params![
                        "work-archive:accepted-task:result-1",
                        "accepted-archive-v1",
                        json!({"taskId":"accepted-task","intentId":"accepted-task:result-1",
                            "actor":"lead","actorEpoch":1,"target":"worker","attempts":0,
                            "created":10.0,"status":"pending","nextAttemptAt":100.0})
                        .to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("archive intent state commits");

        assert_eq!(
            accepted_archive_tick(Arc::clone(&store), 100.0)
                .await
                .expect("tick succeeds"),
            1
        );

        let connection = store.test_reader().expect("read tick result");
        let encoded_receipt: String = connection
            .query_row(
                "SELECT json_extract(record,'$.archiveIntent') FROM runtime_work WHERE id='accepted-task' AND ?1 IS NOT NULL",
                ["work-archive:accepted-task:result-1"],
                |row| row.get(0),
            )
            .expect("archive outcome receipt");
        let receipt: Value = studio_json::from_str(&encoded_receipt).expect("receipt JSON");
        assert_eq!(receipt["status"], "terminal");
        assert_eq!(receipt["outcome"]["status"], "kept");
        assert_eq!(
            receipt["outcome"]["reason"],
            "The remote worker workspace requires review on its own server"
        );
        let encoded_work: String = connection
            .query_row(
                "SELECT record FROM runtime_work WHERE id=?1",
                ["accepted-task"],
                |row| row.get(0),
            )
            .expect("accepted work record");
        let work: Value = studio_json::from_str(&encoded_work).expect("work JSON");
        assert_eq!(work["archive"]["status"], "kept");
        assert_eq!(work["archiveIntent"]["status"], "terminal");
    }

    #[tokio::test]
    async fn accepted_archive_tick_preserves_zero_epoch_on_retry() {
        let root = tempfile::tempdir().expect("state directory");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store"),
        );
        store
            .transaction(|tx| {
                for agent in [
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":1,"status":"running"}),
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"worker","rootId":"lead","epoch":2,"status":"paused",
                        "remoteWorker":{"server":"remote","link":"worker-link"}}),
                ] {
                    AgentRepository::put(tx, &AgentRecord::from_value(agent)?)?;
                }
                let work = json!({
                    "id":"accepted-task","rootId":"lead","owner":"worker",
                    "status":"accepted","version":3,"title":"Accepted result",
                    "description":"","dependencies":[],"results":[{"id":"result-1",
                        "text":"verified","revision":"abc1234"}],"decisions":[],
                    "archiveIntent":{"id":"accepted-task:result-1","owner":"worker",
                        "status":"pending","attempts":0,"created":10.0}
                });
                tx.execute(
                    "INSERT INTO runtime_work(id,record) VALUES (?1,?2)",
                    rusqlite::params!["accepted-task", work.to_string()],
                )?;
                tx.execute(
                    "UPDATE runtime_work SET record=json_set(record,'$.archiveIntent',json(?3)) WHERE id=json_extract(?3,'$.taskId')",
                    rusqlite::params![
                        "work-archive:accepted-task:result-1",
                        "accepted-archive-v1",
                        json!({"taskId":"accepted-task","intentId":"accepted-task:result-1",
                            "actor":"lead","actorEpoch":0,"target":"worker","attempts":0,
                            "created":10.0,"status":"retryable","nextAttemptAt":100.0})
                        .to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("archive intent state commits");

        assert_eq!(
            accepted_archive_tick(Arc::clone(&store), 100.0)
                .await
                .expect("tick succeeds"),
            1
        );

        let connection = store.test_reader().expect("read tick result");
        let encoded_receipt: String = connection
            .query_row(
                "SELECT json_extract(record,'$.archiveIntent') FROM runtime_work WHERE id='accepted-task' AND ?1 IS NOT NULL",
                ["work-archive:accepted-task:result-1"],
                |row| row.get(0),
            )
            .expect("archive outcome receipt");
        let receipt: Value = studio_json::from_str(&encoded_receipt).expect("receipt JSON");
        assert_eq!(receipt["actorEpoch"], 0);
        assert_eq!(receipt["status"], "terminal");
        assert_eq!(receipt["outcome"]["status"], "kept");
        assert_eq!(receipt["outcome"]["reason"], "The archive actor changed");
        let encoded_work: String = connection
            .query_row(
                "SELECT record FROM runtime_work WHERE id=?1",
                ["accepted-task"],
                |row| row.get(0),
            )
            .expect("accepted work record");
        let work: Value = studio_json::from_str(&encoded_work).expect("work JSON");
        assert_eq!(work["archive"]["status"], "kept");
        assert_eq!(work["archiveIntent"]["status"], "terminal");
    }

    async fn reviewer_archive_store(role: &str) -> (Arc<RuntimeStore>, tempfile::TempDir) {
        let root = tempfile::tempdir().expect("state directory");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store"),
        );
        let role = role.to_owned();
        store
            .transaction(move |tx| {
                for agent in [
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","parentId":null,
                        "epoch":1,"status":"running","autoWake":true,"isLead":true}),
                    json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"child","rootId":"lead","parentId":"lead",
                        "epoch":2,"status":"completed","autoWake":false,"inFlight":false,
                        "role":role}),
                ] {
                    AgentRepository::put(tx, &AgentRecord::from_value(agent)?)?;
                }
                tx.execute(
                    "INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch,turn_id,error) \
                     VALUES (?1,?2,'child_result',?3,'delivered',1,1,'lead-turn',NULL)",
                    rusqlite::params![
                        "child:result-1",
                        "lead",
                        json!({"agent_id":"child","result":"review complete"}).to_string()
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("reviewer fixture commits");
        (store, root)
    }

    fn reviewer_record(store: &RuntimeStore) -> Value {
        let connection = store.test_reader().expect("runtime store reads");
        AgentRepository::get(&connection, &AgentId::from("child"))
            .expect("reviewer record loads")
            .expect("reviewer exists")
            .value()
    }

    #[tokio::test]
    async fn reviewer_archive_delivered_child_result_schedules_and_archives() {
        let (store, _root) = reviewer_archive_store("reviewer").await;

        accepted_archive_tick(Arc::clone(&store), 10.0)
            .await
            .expect("maintenance tick succeeds");

        let reviewer = reviewer_record(&store);
        assert_eq!(reviewer["reviewArchiveScheduled"], "child:result-1");
        assert_eq!(
            reviewer["agentArchive"]["reason"],
            "Reviewer result delivered to parent"
        );
        assert_eq!(reviewer["agentArchive"]["by"], "lead");
    }

    #[tokio::test]
    async fn reviewer_archive_uses_earliest_matching_parent_event_id() {
        let (store, _root) = reviewer_archive_store("reviewer").await;
        store
            .transaction(|tx| {
                for (id, parent) in [
                    ("child:z-result", "lead"),
                    ("child:a-result", "lead"),
                    ("child:0-wrong-parent", "other-lead"),
                ] {
                    tx.execute(
                        "INSERT INTO runtime_events(id,agent,kind,text,status,created,epoch,turn_id,error) \
                         VALUES (?1,?2,'child_result',?3,'delivered',2,1,'lead-turn',NULL)",
                        rusqlite::params![
                            id,
                            parent,
                            json!({"agent_id":"child","result":"review complete"}).to_string()
                        ],
                    )?;
                }
                Ok(())
            })
            .await
            .expect("review events insert");

        accepted_archive_tick(Arc::clone(&store), 10.0)
            .await
            .expect("maintenance tick succeeds");

        let reviewer = reviewer_record(&store);
        assert_eq!(reviewer["reviewArchiveScheduled"], "child:a-result");
        assert_eq!(
            reviewer["agentArchive"]["reason"],
            "Reviewer result delivered to parent"
        );
    }

    #[tokio::test]
    async fn reviewer_archive_skips_existing_archive_markers_before_event_lookup() {
        for marker in [
            json!({"reviewArchiveError":"previous terminal error"}),
            json!({"reviewArchiveScheduled":"child:existing-result"}),
            json!({"agentArchive":{"at":1.0,"epoch":2,"reason":"already archived"}}),
        ] {
            let (store, _root) = reviewer_archive_store("reviewer").await;
            let marker_for_write = marker.clone();
            store
                .transaction(move |tx| {
                    let mut child =
                        AgentRepository::get(tx, &AgentId::from("child"))?.expect("reviewer loads");
                    let mut record = child.value();
                    for (key, value) in marker_for_write.as_object().expect("marker object") {
                        record[key] = value.clone();
                    }
                    child = AgentRecord::from_value(record)?;
                    AgentRepository::put(tx, &child)?;
                    Ok(())
                })
                .await
                .expect("marker inserts");

            accepted_archive_tick(Arc::clone(&store), 10.0)
                .await
                .expect("maintenance tick succeeds");

            let reviewer = reviewer_record(&store);
            for (key, value) in marker.as_object().expect("marker object") {
                assert_eq!(reviewer.get(key), Some(value));
            }
            assert_eq!(
                reviewer.get("reviewArchiveScheduled"),
                marker.get("reviewArchiveScheduled")
            );
        }
    }

    #[tokio::test]
    async fn reviewer_archive_refused_by_a_missing_rust_dependency_is_retried_and_archived() {
        // Live cutover 4: 18 completed reviewers kept "Not implemented: requires
        // tool-driver/archive-reconciliation" (or account-worker/agent-interrupt)
        // and were never scheduled again.
        let (store, _root) = reviewer_archive_store("reviewer").await;
        store
            .transaction(|tx| {
                let mut child =
                    AgentRepository::get(tx, &AgentId::from("child"))?.expect("reviewer loads");
                let mut record = child.value();
                record["threadId"] = json!("thread-1");
                record["lastCompletedTurn"] = json!("turn-9");
                record["lastCompletedTurnStatus"] = json!("completed");
                record["reviewArchiveError"] =
                    json!("Not implemented: requires tool-driver/archive-reconciliation");
                child = AgentRecord::from_value(record)?;
                AgentRepository::put(tx, &child)?;
                tx.execute(
                    "INSERT INTO runtime_tool_requests(id,record) VALUES (?1,?2)",
                    rusqlite::params![
                        "tool-done",
                        json!({"id":"tool-done","agent":"child","stage":"completed","outcome":"applied"})
                            .to_string()
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("refused reviewer inserts");

        accepted_archive_tick(Arc::clone(&store), 10.0)
            .await
            .expect("maintenance tick succeeds");

        let reviewer = reviewer_record(&store);
        assert!(reviewer.get("reviewArchiveError").is_none(), "{reviewer}");
        assert_eq!(
            reviewer["agentArchive"]["reason"],
            "Reviewer result delivered to parent"
        );
    }

    #[tokio::test]
    async fn reviewer_archive_failure_records_error_and_retries_with_event_identity() {
        let (store, _root) = reviewer_archive_store("reviewer").await;
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_work(id,record) VALUES (?1,?2)",
                    rusqlite::params![
                        "open-work",
                        json!({"id":"open-work","rootId":"lead","owner":"child",
                            "status":"ready","version":1,"releases":[]})
                        .to_string()
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("archive blocker inserts");

        accepted_archive_tick(Arc::clone(&store), 10.0)
            .await
            .expect("first tick succeeds");
        let first = reviewer_record(&store);
        assert_eq!(first["reviewArchiveScheduled"], "child:result-1");
        assert_eq!(first["reviewArchiveAttempts"], 1);
        assert_eq!(first["reviewArchiveNextAt"], 11.0);
        assert!(
            first["reviewArchiveError"]
                .as_str()
                .is_some_and(|error| !error.is_empty())
        );
        assert!(first.get("agentArchive").is_none());

        accepted_archive_tick(Arc::clone(&store), 11.0)
            .await
            .expect("second tick succeeds");
        let second = reviewer_record(&store);
        assert_eq!(second["reviewArchiveScheduled"], "child:result-1");
        assert_eq!(second["reviewArchiveAttempts"], 2);
        assert_eq!(second["reviewArchiveNextAt"], 13.0);

        store
            .transaction(|tx| {
                tx.execute("DELETE FROM runtime_work WHERE id='open-work'", [])?;
                Ok(())
            })
            .await
            .expect("archive blocker clears");
        accepted_archive_tick(Arc::clone(&store), 12.999)
            .await
            .expect("early retry tick succeeds");
        assert!(reviewer_record(&store).get("agentArchive").is_none());
        accepted_archive_tick(Arc::clone(&store), 13.0)
            .await
            .expect("due retry tick succeeds");
        let archived = reviewer_record(&store);
        assert_eq!(archived["reviewArchiveScheduled"], "child:result-1");
        assert_eq!(
            archived["agentArchive"]["reason"],
            "Reviewer result delivered to parent"
        );
    }

    #[tokio::test]
    async fn reviewer_archive_non_reviewer_child_result_is_not_archived() {
        let (store, _root) = reviewer_archive_store("implementer").await;

        accepted_archive_tick(Arc::clone(&store), 10.0)
            .await
            .expect("maintenance tick succeeds");

        let worker = reviewer_record(&store);
        assert!(worker.get("reviewArchiveScheduled").is_none());
        assert!(worker.get("agentArchive").is_none());
    }

    #[test]
    fn assigning_ready_work_persists_a_stable_worker_notification() {
        let mut connection = database();
        let tx = connection.transaction().expect("transaction starts");
        let created = write_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "create-assigned-work",
            WorkActorWrite::Create(CreateWork {
                result_schema: None,
                gate: Field::Missing,
                actor_id: "lead".to_owned(),
                title: "Assigned task".to_owned(),
                description: "Do the task".to_owned(),
                owner: Some("worker".to_owned()),
                status: None,
                dependencies: vec![],
            }),
        )
        .expect("assigned task creates");
        assert_eq!(created.record["version"], 1);
        assert!(created.event_inserted);
        let task_id = created.record["id"].as_str().expect("task ID");
        assert_eq!(
            events(&tx),
            vec![(
                format!("work-ready:{task_id}:assignment:1"),
                "worker".to_owned(),
                "work_ready".to_owned(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn assigning_existing_ready_work_queues_the_new_owner() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("reassign", Some("lead"), "ready", json!([])),
        );
        let tx = connection.transaction().expect("transaction starts");
        let updated = write_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "reassign-ready-work",
            WorkActorWrite::Mutate(WorkMutation::Update {
                result_schema: None,
                gate: Field::Missing,
                task_id: "reassign".to_owned(),
                version: Some(2),
                title: None,
                description: None,
                owner: Some(Some("worker".to_owned())),
                status: None,
                dependencies: None,
            }),
        )
        .expect("task updates");
        assert_eq!(updated.record["version"], 3);
        assert!(updated.event_inserted);
        assert_eq!(
            events(&tx),
            vec![(
                "work-ready:reassign:assignment:3".to_owned(),
                "worker".to_owned(),
                "work_ready".to_owned(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    fn events(connection: &Connection) -> Vec<(String, String, String)> {
        let mut statement = connection
            .prepare("SELECT id,agent,kind FROM runtime_events ORDER BY id")
            .expect("events query prepares");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("events query runs")
            .map(|row| row.expect("event row reads"))
            .collect()
    }

    fn work(id: &str, owner: Option<&str>, status: &str, results: Value) -> Value {
        json!({"id":id,"rootId":"lead","owner":owner,"status":status,"title":id,
            "description":"","created":1.0,"updated":1.0,"version":2,
            "dependencies":[],"results":results,"decisions":[],"createdBy":"worker"})
    }

    #[tokio::test]
    async fn accepted_archive_refuses_changed_work_after_git_check() {
        let repo = tempfile::tempdir().expect("repository");
        let git = |args: &[&str]| {
            let output =
                crate::bounded_child::run_with_isolated_git_config(crate::bounded_child::Request {
                    program: std::ffi::OsStr::new("git"),
                    git_directory: Some(repo.path().as_os_str()),
                    args,
                    environment: &[],
                    input: None,
                    stdout_limit: 4096,
                    stderr_limit: 4096,
                    timeout: std::time::Duration::from_secs(10),
                })
                .expect("git runs");
            String::from_utf8(output.stdout.bytes).expect("git output")
        };
        git(&["init", "-b", "main"]);
        git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ]);
        let revision = git(&["rev-parse", "HEAD"]).trim().to_owned();
        let dir = tempfile::tempdir().expect("state");
        let store = Arc::new(RuntimeStore::open(Some(dir.path())).await.expect("store"));
        let owner = json!({"compactions":0,"accountKey":"default","provider":"codex","id":"worker","rootId":"lead","epoch":2,"status":"paused","cwd":repo.path().to_str().expect("path")});
        let item = work(
            "race",
            Some("worker"),
            "accepted",
            json!([{"id":"result-1","agent":"worker","text":"done","checks":"checked","revision":revision,"files":[]}]),
        );
        let saved_owner = AgentRecord::from_value(owner.clone()).expect("checked owner");
        let saved_work = super::WorkItemRecord::checked(&item).expect("checked work");
        store.transaction(move |tx| {
            AgentRepository::put(tx, &AgentRecord::from_value(json!({"compactions":0,"accountKey":"default","provider":"codex","id":"lead","rootId":"lead","epoch":1,"status":"running"}))?)?;
            AgentRepository::put(tx, &AgentRecord::from_value(owner)?)?;
            WorkItemsRepository::put_work(tx, "race", &item)?;
            Ok(())
        }).await.expect("seed");
        let intent: super::ArchiveIntent = studio_json::from_str(r#"{"status":"unknown","taskId":"race","intentId":"race:result-1","actor":"lead","actorEpoch":1,"target":"worker","attempts":0,"created":1}"#).expect("intent");
        assert!(super::accepted_revision_is_on_local_main(
            repo.path().to_str().expect("path"),
            &revision
        ));
        store
            .transaction(|tx| {
                tx.execute(
                    "UPDATE runtime_work SET record=json_set(record,'$.version',3) WHERE id='race'",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("concurrent work change");
        let guard = super::ArchiveGuard {
            work: saved_work,
            owner: Box::new(saved_owner),
            task_id: "race".into(),
            owner_id: "worker".into(),
            root_id: "lead".into(),
            intent,
            actor_epoch: 1,
        };
        let result = crate::agent_management_store::execute(
            Arc::clone(&store),
            &crate::agent_management::Action::Archive {
                actor: "lead".into(),
                epoch: 1,
                target: "worker".into(),
                reason: "Accepted task result is on main".into(),
                unassign_work: false,
            },
            10.0,
            Some(Box::new(move |tx| guard.check(tx))),
        )
        .await;
        assert!(matches!(
            result,
            Err(crate::agent_management::ManagementError::Invalid(
                "The accepted task owner or result changed"
            ))
        ));
        let reader = store.test_reader().expect("reader");
        let owner = AgentRepository::get(&reader, &AgentId::from("worker"))
            .expect("owner read")
            .expect("owner exists");
        assert!(owner.value().get("deletedAt").is_none());
    }

    #[test]
    fn layr_evidence_is_rejected_when_assignment_changes_before_commit() {
        let public_root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let mut connection = database();
        for id in ["worker", "replacement-worker"] {
            let record = json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,
                "id":id,"rootId":"lead","epoch":2,"status":"running","autoWake":true,
                "inFlight":true,"agentMode":"multi","concurrency":8,"executionMode":"native","workspaceMode":"layr-native",
                "layrProjectId":"project","layrLine":id
            });
            connection
                .execute(
                    "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                    rusqlite::params![record.to_string(), id],
                )
                .expect("agent becomes a Layr worker");
            if id == "replacement-worker" {
                connection
                    .execute(
                        "INSERT INTO runtime_agents(id,record) VALUES (?1,?2)",
                        rusqlite::params![id, record.to_string()],
                    )
                    .expect("replacement worker inserts");
            }
        }
        let mut current = work(
            "layr-owner-race",
            Some("replacement-worker"),
            "running",
            json!([]),
        );
        current["version"] = json!(3);
        insert_work(&connection, &current);
        let request_id = "layr-evidence:worker:state-1";
        let intent = json!({
            "status":"completed","requestId":request_id,"taskId":"layr-owner-race",
            "owner":"worker","revision":"state-1","version":2,"created":1.0,
            "evidence":{"layrProjectId":"project","layrLine":"worker","layrStateId":"state-1"}
        });
        connection
            .execute(
                "UPDATE runtime_work SET record=json_set(record,'$.vmSubmit',json(?3)) WHERE id=json_extract(?3,'$.taskId')",
                rusqlite::params![
                    "layr-owner-race",
                    "work-write-vm-intent-v1",
                    intent.to_string()
                ],
            )
            .expect("completed guest evidence intent inserts");

        let tx = connection.transaction().expect("transaction starts");
        let result = super::write(
            &tx,
            "lead",
            Some(public_root.as_str()),
            "request-signature",
            WorkWrite::Submit(super::SubmitWork {
                prepared_gate_workspace: None,
                task_id: "layr-owner-race".to_owned(),
                version: None,
                result: "result".into(),
                checks: "checks".to_owned(),
                revision: "state-1".to_owned(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: Some(studio_runtime_store::work_intent::Evidence {
                    layr_project_id: "project".into(),
                    layr_line: "worker".into(),
                    layr_state_id: "state-1".into(),
                }),
                layr_evidence_request_id: Some(request_id.to_owned()),
            }),
        );
        assert!(matches!(result, Err(WorkWriteError::Event(_))));
        let saved = WorkItemsRepository::work_record(&tx, "layr-owner-race")
            .expect("work row reads")
            .expect("work row remains");
        assert_eq!(saved.owner(), Some("replacement-worker"));
        assert!(
            saved.value()["results"]
                .as_array()
                .expect("results array")
                .is_empty()
        );
        tx.rollback().expect("transaction rolls back");
    }

    #[test]
    fn actor_claim_overrides_the_selected_payload_claimant() {
        let mut connection = database();
        insert_work(&connection, &work("claim", None, "ready", json!([])));
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "worker",
            None,
            "claim-as-worker",
            WorkWrite::Claim(ClaimWork {
                task_id: "claim".to_owned(),
                version: Some(2),
                claimant_id: "lead".to_owned(),
            }),
        )
        .expect("actor claim succeeds");
        assert_eq!(result.record["owner"], "worker");
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn actor_create_rejects_a_nonlead_selected_agent_without_writing() {
        let mut connection = database();
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "worker",
            "lead",
            None,
            "create-with-worker-selected",
            WorkWrite::Create(CreateWork {
                result_schema: None,
                gate: Field::Missing,
                actor_id: "lead".to_owned(),
                title: "must be refused".to_owned(),
                description: String::new(),
                owner: None,
                status: None,
                dependencies: Vec::new(),
            }),
        );
        assert!(matches!(result, Err(WorkWriteError::Event(_))));
        let count: i64 = tx
            .query_row("SELECT count(*) FROM runtime_work", [], |row| row.get(0))
            .expect("work count reads");
        assert_eq!(count, 0);
        tx.rollback().expect("transaction rolls back");
    }

    #[test]
    fn nonlead_actor_cannot_change_assignments_through_actor_api() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("unauthorized-update", Some("worker"), "running", json!([])),
        );
        let before = WorkItemsRepository::work_record(&connection, "unauthorized-update")
            .expect("stored work reads")
            .expect("work exists");
        let tx = connection.transaction().expect("transaction starts");
        let result = write_as_agent(
            &tx,
            "lead",
            "worker",
            None,
            "worker-assignment-update",
            WorkActorWrite::Mutate(WorkMutation::Update {
                result_schema: None,
                gate: Field::Missing,
                task_id: "unauthorized-update".to_owned(),
                version: Some(2),
                title: None,
                description: None,
                owner: Some(Some("lead".to_owned())),
                status: None,
                dependencies: None,
            }),
        );
        assert!(result.is_err());
        let after = WorkItemsRepository::work_record(&tx, "unauthorized-update")
            .expect("stored work reads")
            .expect("work exists");
        assert_eq!(after, before);
        tx.rollback().expect("transaction rolls back");
    }

    #[test]
    fn actor_owner_cancel_notifies_the_owner_itself() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("cancel", Some("worker"), "running", json!([])),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "worker",
            None,
            "cancel-as-owner",
            WorkWrite::Mutate(WorkMutation::Cancel {
                task_id: "cancel".to_owned(),
                version: Some(2),
                reason: "stop".to_owned(),
            }),
        )
        .expect("owner cancellation succeeds");
        assert_eq!(result.record["status"], "cancelled");
        assert_eq!(
            events(&tx),
            vec![(
                "work-decision:cancel:cancel".to_owned(),
                "worker".to_owned(),
                "work_decision".to_owned(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn lead_actor_cancelling_worker_owned_work_notifies_that_worker() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("lead-cancel", Some("worker"), "running", json!([])),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "cancel-worker-as-lead",
            WorkWrite::Mutate(WorkMutation::Cancel {
                task_id: "lead-cancel".to_owned(),
                version: Some(2),
                reason: "stop".to_owned(),
            }),
        )
        .expect("lead cancellation succeeds");
        assert_eq!(result.record["decisions"][0]["by"], "lead");
        assert_eq!(
            events(&tx),
            vec![(
                "work-decision:lead-cancel:cancel".into(),
                "worker".into(),
                "work_decision".into(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn acceptance_gate_failed_refuses_accept_before_releasing_dependents() {
        let mut connection = database();
        let mut task = work(
            "gated",
            Some("lead"),
            "review",
            json!([
                {"id":"result-gate","agent":"lead","text":"done","checks":"failed","revision":"abc1234","files":[],
                 "gate":{"status":"failed","exitCode":1}}
            ]),
        );
        task["gate"] = json!({"command":"false"});
        insert_work(&connection, &task);
        let mut dependent = work("dependent", Some("worker"), "blocked", json!([]));
        dependent["dependencies"] = json!(["gated"]);
        insert_work(&connection, &dependent);
        let before_events = events(&connection);
        let tx = connection.transaction().expect("transaction");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "gate-accept",
            WorkWrite::Accept(AcceptWork {
                override_reason: None,
                task_id: "gated".into(),
                version: Some(2),
                reason: "reviewed".into(),
                merged_result_id: None,
            }),
        );
        assert_eq!(
            WorkItemsRepository::work_record(&tx, "dependent")
                .expect("dependent reads")
                .expect("dependent exists")
                .into_value(),
            dependent
        );
        assert_eq!(events(&tx), before_events);
        assert!(
            result.is_err(),
            "a failed required gate must refuse acceptance"
        );
        assert_eq!(
            WorkItemsRepository::work_record(&tx, "gated")
                .expect("gate query succeeds")
                .expect("gate exists")
                .status(),
            studio_runtime_store::WorkStatus::Review
        );
    }

    #[test]
    fn lead_actor_accepting_its_own_result_does_not_self_notify() {
        let mut connection = database();
        insert_work(
            &connection,
            &work(
                "accept",
                Some("lead"),
                "review",
                json!([
                    {"id":"result-1","agent":"lead","text":"done","checks":"checked","revision":"abc1234","files":[]}
                ]),
            ),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "accept-as-lead",
            WorkWrite::Accept(AcceptWork {
                override_reason: None,
                task_id: "accept".to_owned(),
                version: Some(2),
                reason: "verified".to_owned(),
                merged_result_id: None,
            }),
        )
        .expect("lead acceptance succeeds");
        assert_eq!(result.record["status"], "accepted");
        let count: i64 = tx
            .query_row(
                "SELECT count(*) FROM runtime_events WHERE kind='work_decision'",
                [],
                |row| row.get(0),
            )
            .expect("decision event count reads");
        assert_eq!(count, 0);
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn native_layr_reviewer_result_is_accepted_without_a_merge_and_implementer_requires_merge() {
        assert_reviewer_result_is_accepted_without_a_layr_merge("native", "layr-native");
    }

    fn assert_reviewer_result_is_accepted_without_a_layr_merge(
        execution_mode: &str,
        workspace_mode: &str,
    ) {
        let mut connection = database();
        for (id, role) in [("lead", "orchestrator"), ("worker", "reviewer")] {
            let record = json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,
                "id":id,"rootId":"lead","epoch":2,"status":"running","autoWake":true,
                "inFlight":true,"agentMode":"multi","concurrency":8,"executionMode":execution_mode,"workspaceMode":workspace_mode,
                "isLead":(id == "lead"),"role":role,"layrProjectId":"project","layrLine":id
            });
            connection
                .execute(
                    "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                    rusqlite::params![record.to_string(), id],
                )
                .expect("agent becomes a Layr agent");
        }
        insert_work(
            &connection,
            &work(
                "review-task",
                Some("worker"),
                "review",
                json!([{"id":"result-r","agent":"worker","text":"no defects","checks":"read","revision":"state-1","files":[]}]),
            ),
        );
        let tx = connection.transaction().expect("transaction starts");
        let accept = |tx: &rusqlite::Transaction<'_>| {
            write_action_as_agent(
                tx,
                "lead",
                "lead",
                None,
                "accept-review",
                WorkWrite::Accept(AcceptWork {
                    override_reason: None,
                    task_id: "review-task".to_owned(),
                    version: Some(2),
                    reason: "review read".to_owned(),
                    merged_result_id: None,
                }),
            )
        };
        let reviewer = AgentRepository::get(&tx, &AgentId::from("worker"))
            .expect("reviewer reads")
            .expect("reviewer exists");
        assert!(!super::layr_result_needs_merge("lead", &reviewer));
        let result = accept(&tx).expect("a reviewer result needs no merge");
        assert_eq!(result.record["status"], "accepted");
        let intent = super::ArchiveIntent {
            status: super::WorkEffectStatus::Pending,
            task_id: "review-task".into(),
            intent_id: "review-task:result-r".into(),
            actor: "lead".into(),
            actor_epoch: 2,
            target: "worker".into(),
            attempts: 0,
            created: 10.0,
            next_attempt_at: Some(10.0),
            attempt_started_at: None,
            updated: None,
            last_outcome: None,
            outcome: None,
            public_request_id: None,
            public_admission: None,
            public_response: None,
        };
        let checked_result =
            super::WorkItemRecord::checked(&result.record).expect("checked result");
        assert_eq!(
            super::accepted_archive_preflight(
                &tx,
                &checked_result,
                super::result_identity(&checked_result).as_ref(),
                &intent,
                "worker",
                "lead",
                2
            )
            .unwrap(),
            None
        );
        tx.rollback().expect("transaction rolls back");

        // An implementer's line still needs the merge before acceptance.
        connection
            .execute(
                "UPDATE runtime_agents SET record=json_set(record,'$.role','implementer') WHERE id='worker'",
                [],
            )
            .expect("worker becomes an implementer");
        let tx = connection.transaction().expect("transaction starts");
        assert!(accept(&tx).is_err());
    }

    #[test]
    fn lead_actor_rejecting_its_own_result_does_not_self_notify() {
        let mut connection = database();
        insert_work(
            &connection,
            &work(
                "reject-own",
                Some("lead"),
                "review",
                json!([{"id":"result-own","agent":"lead","text":"done","checks":"checked","revision":"abc1234","files":[]}]),
            ),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "reject-own-as-lead",
            WorkWrite::Mutate(WorkMutation::Reject {
                task_id: "reject-own".to_owned(),
                version: Some(2),
                reason: "needs changes".to_owned(),
            }),
        )
        .expect("lead rejection succeeds");
        assert_eq!(result.record["decisions"][0]["by"], "lead");
        let count: i64 = tx
            .query_row(
                "SELECT count(*) FROM runtime_events WHERE kind='work_decision'",
                [],
                |row| row.get(0),
            )
            .expect("decision event count reads");
        assert_eq!(count, 0);
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn lead_actor_rejecting_worker_result_notifies_worker() {
        let mut connection = database();
        insert_work(
            &connection,
            &work(
                "reject-worker",
                Some("worker"),
                "review",
                json!([{"id":"result-worker","agent":"worker","text":"done","checks":"checked","revision":"abc1234","files":[]}]),
            ),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "reject-worker-as-lead",
            WorkWrite::Mutate(WorkMutation::Reject {
                task_id: "reject-worker".to_owned(),
                version: Some(2),
                reason: "needs changes".to_owned(),
            }),
        )
        .expect("lead rejection succeeds");
        assert_eq!(result.record["decisions"][0]["by"], "lead");
        assert_eq!(
            events(&tx),
            vec![(
                "work-decision:reject-worker:2".into(),
                "worker".into(),
                "work_decision".into(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonempty_file_submission_rechecks_cwd_and_replays_without_filesystem_work() {
        let state = tempfile::tempdir().expect("state directory");
        let project = tempfile::tempdir().expect("project directory");
        let changed = tempfile::tempdir().expect("changed project directory");
        let store = RuntimeStore::open(Some(state.path())).await.expect("store");
        let request_root = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let cwd = project.path().to_string_lossy().into_owned();
        let record = json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":1,"status":"running",
            "autoWake":true,"cwd":cwd});
        let item = work("file-submission", Some("lead"), "running", json!([]));
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(record)?)?;
                WorkItemsRepository::put_work(tx, "file-submission", &item)?;
                Ok(())
            })
            .await
            .expect("seed");
        let mut input = super::SubmitWork {
            prepared_gate_workspace: None,
            task_id: "file-submission".into(),
            version: Some(2),
            result: "done".into(),
            checks: "checked".into(),
            revision: "abc1234".into(),
            files: vec!["src/first.rs".into(), "src/second.rs".into()],
            prepared_files: None,
            state_dir: state.path().into(),
            layr_evidence: None,
            layr_evidence_request_id: None,
        };
        input.prepared_files = super::prepare_submission_files(&store, "lead", &input)
            .await
            .expect("path observation");
        let new_cwd = changed.path().to_string_lossy().into_owned();
        store
            .transaction(move |tx| {
                let mut agent = AgentRepository::get(tx, &AgentId::from("lead"))?.expect("lead");
                agent.cwd = Some(new_cwd);
                AgentRepository::put(tx, &agent)?;
                Ok(())
            })
            .await
            .expect("cwd changes after preparation");
        let stale = input.clone();
        let refused = store
            .transaction_with({
                let request_root = request_root.clone();
                move |tx| {
                    super::write(
                        tx,
                        "lead",
                        Some(request_root.as_str()),
                        "signature",
                        WorkWrite::Submit(stale),
                    )
                }
            })
            .await
            .map(|_| ())
            .expect_err("stale cwd must be refused");
        assert!(matches!(
            refused,
            WorkWriteError::Event(crate::event_write::EventWriteError::Conflict(_))
        ));
        input.prepared_files = super::prepare_submission_files(&store, "lead", &input)
            .await
            .expect("new path observation");
        let first = input.clone();
        let result = store
            .transaction_with({
                let request_root = request_root.clone();
                move |tx| {
                    super::write(
                        tx,
                        "lead",
                        Some(request_root.as_str()),
                        "signature",
                        WorkWrite::Submit(first),
                    )
                }
            })
            .await
            .expect("nonempty submission");
        assert_eq!(result.record["results"][0]["files"], json!(input.files));
        input.prepared_files = None;
        let replay = store
            .transaction_with({
                let request_root = request_root.clone();
                move |tx| {
                    super::write(
                        tx,
                        "lead",
                        Some(request_root.as_str()),
                        "signature",
                        WorkWrite::Submit(input),
                    )
                }
            })
            .await
            .expect("exact replay needs no filesystem observation");
        assert_eq!(result.record, replay.record);
    }

    #[test]
    fn owner_becoming_legacy_after_preflight_refuses_submit_and_accept() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("race-submit", Some("worker"), "running", json!([])),
        );
        insert_work(
            &connection,
            &work(
                "race-accept",
                Some("worker"),
                "review",
                json!([{"id":"result-1","agent":"worker","text":"done","checks":"ok","revision":"abc1234","files":[]}]),
            ),
        );
        // Preflight saw the native worker; both tasks are reassigned before the write.
        connection.execute("INSERT INTO runtime_agents(id,record) SELECT 'legacy',json_set(record,'$.id','legacy','$.workspaceMode','layr') FROM runtime_agents WHERE id='worker'", []).unwrap();
        connection.execute("UPDATE runtime_work SET record=json_set(record,'$.owner','legacy') WHERE id IN ('race-submit','race-accept')", []).unwrap();
        let tx = connection.transaction().unwrap();
        let submit = super::write(
            &tx,
            "lead",
            None,
            "race-submit",
            WorkWrite::Submit(super::SubmitWork {
                prepared_gate_workspace: None,
                task_id: "race-submit".into(),
                version: Some(2),
                result: "done".into(),
                checks: "ok".into(),
                revision: "abc1234".into(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: None,
                layr_evidence_request_id: None,
            }),
        );
        assert!(matches!(
            submit,
            Err(WorkWriteError::Event(
                crate::event_write::EventWriteError::InvalidRequest(
                    "VM agents are no longer supported"
                )
            ))
        ));
        let accept = super::write(
            &tx,
            "lead",
            None,
            "race-accept",
            WorkWrite::Accept(AcceptWork {
                task_id: "race-accept".into(),
                version: Some(2),
                reason: "reviewed".into(),
                override_reason: None,
                merged_result_id: None,
            }),
        );
        assert!(matches!(
            accept,
            Err(WorkWriteError::Event(
                crate::event_write::EventWriteError::InvalidRequest(
                    "VM agents are no longer supported"
                )
            ))
        ));
    }

    #[test]
    fn actor_submit_records_worker_and_notifies_the_lead() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("submit", Some("worker"), "running", json!([])),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "worker",
            None,
            "submit-as-worker",
            WorkWrite::Submit(super::SubmitWork {
                prepared_gate_workspace: None,
                task_id: "submit".to_owned(),
                version: Some(2),
                result: "done".into(),
                checks: "checked".to_owned(),
                revision: "abc1234".to_owned(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: None,
                layr_evidence_request_id: None,
            }),
        )
        .expect("worker submission succeeds");
        assert_eq!(result.record["results"][0]["agent"], "worker");
        assert_eq!(
            events(&tx),
            vec![(
                format!(
                    "work-result:{}",
                    result.record["results"][0]["id"]
                        .as_str()
                        .expect("result ID")
                ),
                "lead".to_owned(),
                "work_review".to_owned(),
            )]
        );
        tx.commit().expect("transaction commits");
    }

    #[test]
    fn lead_actor_submitting_its_own_work_does_not_create_a_review_event() {
        let mut connection = database();
        insert_work(
            &connection,
            &work("lead-submit", Some("lead"), "running", json!([])),
        );
        let tx = connection.transaction().expect("transaction starts");
        let result = write_action_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "submit-as-lead",
            WorkWrite::Submit(super::SubmitWork {
                prepared_gate_workspace: None,
                task_id: "lead-submit".to_owned(),
                version: Some(2),
                result: "done".into(),
                checks: "checked".to_owned(),
                revision: "abc1234".to_owned(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: None,
                layr_evidence_request_id: None,
            }),
        )
        .expect("lead submission succeeds");
        assert_eq!(result.record["results"][0]["agent"], "lead");
        let count: i64 = tx
            .query_row(
                "SELECT count(*) FROM runtime_events WHERE kind='work_review'",
                [],
                |row| row.get(0),
            )
            .expect("review event count reads");
        assert_eq!(count, 0);
        tx.commit().expect("transaction commits");
    }

    #[tokio::test]
    async fn accepted_archive_skips_intent_of_an_older_shape() {
        let dir = tempfile::tempdir().expect("state");
        let store = Arc::new(RuntimeStore::open(Some(dir.path())).await.expect("store"));
        let old = json!({"status":"pending","attempts":0,"created":1.0}).to_string();
        let run = super::run_accepted_archive(
            &store,
            "old".into(),
            old,
            1000.0,
            super::ArchiveTiming::Immediate,
        )
        .await;
        assert!(matches!(run, Ok(super::ArchiveRun::Skipped)));
    }
}

#[cfg(test)]
mod release_tests {
    use super::{continuation_work_pending, release_failed_work};
    use rusqlite::Connection;
    use studio_json::{Value, json};
    use studio_runtime_store::AgentRecord;

    #[test]
    fn continuation_identity_only_preserves_an_exact_pending_retry() {
        let agent = AgentRecord::from_value(json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
            "id":"worker","rootId":"lead","epoch":7,"status":"failed",
            "autoWake":true,"accountKey":"acct","threadId":"thread",
            "lastCompletedTurn":"turn-3",
            "capacityRetry":{"status":"starting","accountKey":"acct","threadId":"thread","epoch":7,"turnId":"turn-3"}
        }))
        .expect("worker record parses");
        assert!(continuation_work_pending(&agent));

        let mut unrelated = agent.value();
        unrelated["capacityRetry"]["turnId"] = json!("older-turn");
        let unrelated = AgentRecord::from_value(unrelated).expect("worker record parses");
        assert!(!continuation_work_pending(&unrelated));

        let mut explicit_null = agent.value();
        explicit_null["accountKey"] = Value::Null;
        explicit_null["capacityRetry"]
            .as_object_mut()
            .expect("capacity retry is an object")
            .remove("accountKey");
        assert!(AgentRecord::from_value(explicit_null).is_err());
    }

    #[test]
    fn service_releases_failed_worker_claim_and_emits_recovery_event() {
        let mut connection = Connection::open_in_memory().expect("memory SQLite opens");
        connection.execute_batch(
            &studio_runtime_store::agent_columns::fixture_sql("CREATE TABLE runtime_work(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE runtime_work_results(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL); CREATE TABLE runtime_work_decisions(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL); CREATE TABLE runtime_work_releases(id TEXT PRIMARY KEY, work TEXT NOT NULL, idx INTEGER NOT NULL, record TEXT NOT NULL);
             CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
             CREATE TABLE runtime_events(id TEXT PRIMARY KEY,agent TEXT,kind TEXT,text TEXT,status TEXT,created REAL,epoch INTEGER,turn_id TEXT,error TEXT);
             CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL);"),
        ).expect("service tables create");
        connection
            .execute_batch(studio_runtime_store::agent_volatile::agent_volatile_table_sql())
            .expect("volatile agent table creates");
        for agent in [
            json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":1,"status":"paused","autoWake":false}),
            json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,"id":"worker","rootId":"lead","epoch":2,"status":"failed","parentId":"lead","autoWake":false,"error":"worker exited"}),
        ] {
            connection
                .execute(
                    "INSERT INTO runtime_agents(id,record) VALUES (?1,?2)",
                    rusqlite::params![agent["id"].as_str().unwrap_or_default(), agent.to_string()],
                )
                .expect("agent inserts");
        }
        let work = json!({
            "id":"task","rootId":"lead","owner":"worker","status":"running",
            "title":"Task","created":1.0,"updated":1.0,"version":2,
            "dependencies":[],"results":[],"decisions":[]
        });
        let blocked_work = json!({
            "id":"blocked-task","rootId":"lead","owner":"worker","status":"blocked",
            "title":"Blocked task","created":1.0,"updated":1.0,"version":2,
            "dependencies":[],"results":[],"decisions":[]
        });
        connection
            .execute(
                "INSERT INTO runtime_work(id,record) VALUES ('task',?1),('blocked-task',?2)",
                rusqlite::params![work.to_string(), blocked_work.to_string()],
            )
            .expect("work items insert");
        let failed = AgentRecord::from_value(
            json!({"compactions":0,"accountKey":"default","provider":studio_runtime_schema::ProviderKind::Codex,
                "id":"worker","rootId":"lead","epoch":2,"status":"failed",
                "parentId":"lead","autoWake":false,"error":"worker exited"
            }),
        )
        .expect("worker record parses");

        let tx = connection.transaction().expect("transaction starts");
        let released = release_failed_work(&tx, &[failed], 10.0).expect("claims release");
        let mut released_ids = released.task_ids.clone();
        released_ids.sort();
        assert_eq!(released_ids, ["blocked-task", "task"]);
        assert!(released.event_inserted);
        let stored: Value = studio_json::from_str(
            &tx.query_row(
                "SELECT record FROM runtime_work WHERE id='task'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("released record reads"),
        )
        .expect("released JSON parses");
        assert_eq!(stored["owner"], Value::Null);
        assert_eq!(stored["status"], "ready");
        assert_eq!(stored["releases"][0]["reason"], "worker exited");
        let blocked: Value = studio_json::from_str(
            &tx.query_row(
                "SELECT record FROM runtime_work WHERE id='blocked-task'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("blocked release reads"),
        )
        .expect("blocked work JSON parses");
        assert_eq!(blocked["owner"], Value::Null);
        assert_eq!(blocked["status"], "blocked");
        assert_eq!(blocked["version"], 3);
        let event = tx
            .query_row(
                "SELECT kind,text FROM runtime_events WHERE id='work-released:task:3'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("recovery event is persisted");
        assert_eq!(event.0, "work_released");
        assert!(event.1.contains("worker exited"));
        tx.commit().expect("transaction commits");
    }
}
