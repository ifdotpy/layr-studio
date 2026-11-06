//! Local managed Spawn and native Review child creation.
//!
//! Python parity sources: `codex_runtime.py::Runtime.spawn_agents` (8666–8862)
//! and `codex_agent_review.py::request` (114–260). Preparation and catalog
//! access happen before the writer; child admission, records, first-turn intents
//! and task assignment commit together. The caller releases returned wake
//! signals only after this transaction commits.

use crate::event_write;
use rusqlite::{OptionalExtension, Transaction};
use std::path::{Path, PathBuf};
use studio_json::{Value, json};
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::path_identity::CanonicalPath;
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore, StoreError};
use thiserror::Error;

use crate::managed_host_tools::MAX_SPAWN_BATCH_AGENTS;
const MAX_NAME_CHARS: usize = 100;
const MAX_PROMPT_CHARS: usize = 32_000;
/// A new worker uses its parent's workspace mode; ordinary reviewers share the folder.
pub(crate) fn default_workspace<'a>(role: &str, parent: &'a Value) -> &'a str {
    if role == "reviewer" && !crate::layr_native::is_native(parent) {
        "shared"
    } else {
        parent
            .get("workspaceMode")
            .and_then(Value::as_str)
            .unwrap_or("worktree")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ManagedCreationEnvironmentError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("Not implemented: requires {0}")]
    Unavailable(&'static str),
}

/// Host account facts and model admission. Production implementations resolve
/// model settings through the retained `AccountSessionPort` broker.
pub trait ManagedCreationEnvironment: Send + Sync {
    /// Whether the caller's catalog offers this review model.
    fn account_offers_model(
        &self,
        account_key: &str,
        model: &str,
    ) -> Result<bool, ManagedCreationEnvironmentError>;
    /// Wait for the actor and worker-candidate model catalogs at the callback's async
    /// boundary. Implementations keep this bounded and never run it at worker
    /// startup or inside the synchronous store callback.
    fn wait_for_catalog<'a>(
        &'a self,
        _account_key: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), ManagedCreationEnvironmentError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { Ok(()) })
    }
    /// Resolve the account and model as `codex_worker_accounts.resolve` does.
    /// `requested_account_key` is the optional per-worker `account_key`.
    /// `excluded_account_keys` comes from the root agent. Project keys are the
    /// resolved project settings allowlist, when the root has a project binding.
    /// Implicit selection picks the ready account with the fewest active agents
    /// (`active_agents`, ties by account key), filtering disabled/unusable
    /// accounts, project keys, provider, and models (including aliases);
    /// explicit selection never falls back.
    fn select_worker_account(
        &self,
        request: crate::provider_callbacks::WorkerAccountRequest<'_>,
    ) -> Result<(String, crate::host_worker_create::AccountIdentity), ManagedCreationEnvironmentError>;
    fn catalog(&self) -> &dyn crate::host_worker_create::CreationCatalog;
    fn prepare_workspace(
        &self,
        store: &RuntimeStore,
        actor: &AgentRecord,
        request_id: &str,
    ) -> Result<
        Box<dyn crate::monitor_runtime::WorkspaceAdmission + Send + Sync>,
        ManagedCreationEnvironmentError,
    >;
}

fn expand_review_home(path: &str, home: Option<&str>) -> PathBuf {
    if path == "~" {
        return home
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home
    {
        return Path::new(home).join(rest);
    }
    PathBuf::from(path)
}

/// Python `codex_worker_workspace.select` note for a default worktree outside Git.
const NO_GIT_WORKTREE_NOTE: &str =
    "This folder has no Git repository; the worker uses the shared folder";

/// Python `codex_runtime.no_worktree_warning`.
fn no_worktree_warning(directory: &str) -> String {
    format!(
        "{directory} is not in a git repository. This agent works directly in the folder, \
         without an isolated worktree. Other agents in this folder can change the same files."
    )
}

/// Maximum characters of a requested worker folder, after trimming.
const MAX_SPAWN_DIRECTORY_CHARS: usize = 4096;

/// Python `codex_runtime.spawn_directory` for a host worker: `None` keeps the
/// parent's folder; a relative path starts at the parent's folder; the
/// resolved path must be an existing folder.
fn spawn_directory(parent: &str, requested: Option<&str>) -> Result<Option<String>, Error> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let trimmed = event_write::python_strip(requested);
    if trimmed.is_empty() || trimmed.chars().count() > MAX_SPAWN_DIRECTORY_CHARS {
        return Err(crate::managed_host_tools::Error::Invalid("cwd must be a folder path").into());
    }
    let home = std::env::var("HOME").ok();
    let expanded = expand_review_home(trimmed, home.as_deref());
    let path = if expanded.is_absolute() {
        expanded
    } else {
        Path::new(parent).join(expanded)
    };
    // `Path.resolve()` of an existing folder is its canonical path.
    match CanonicalPath::existing(&path) {
        Ok(path) if path.is_dir() => Ok(Some(path.to_string_lossy().into_owned())),
        _ => {
            Err(crate::managed_host_tools::Error::Invalid("cwd must be an existing folder").into())
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Managed(#[from] crate::managed_host_tools::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("Host child creation was refused")]
    Refused,
    #[error("Not implemented: requires {0}")]
    Unavailable(&'static str),
}

impl From<ManagedCreationEnvironmentError> for Error {
    fn from(error: ManagedCreationEnvironmentError) -> Self {
        match error {
            ManagedCreationEnvironmentError::Invalid(message) => {
                crate::managed_host_tools::Error::Invalid(message).into()
            }
            ManagedCreationEnvironmentError::Unavailable(name) => Error::Unavailable(name),
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkerSettingsError {
    #[error(transparent)]
    Environment(#[from] ManagedCreationEnvironmentError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Resolved child routing and execution settings shared by both creation callers.
pub(crate) struct ResolvedWorkerSettings {
    pub account_key: String,
    pub account: crate::host_worker_create::AccountIdentity,
    pub model: String,
    pub effort: Option<String>,
    pub fast_mode: bool,
    pub daybreak: bool,
}

pub(crate) struct WorkerSettingsRequest<'a> {
    pub parent: &'a AgentRecord,
    pub root: &'a AgentRecord,
    pub spec: &'a studio_runtime_schema::creation_intent::CreationIntent,
    pub profile: Option<&'a studio_runtime_store::ProfileRecord>,
    pub role: &'a str,
    pub requested_account_key: Option<&'a str>,
    pub account_load: &'a std::collections::HashMap<String, i64>,
}

async fn account_scope(
    store: &RuntimeStore,
    root: &AgentRecord,
) -> Result<(Vec<String>, Option<Vec<String>>, Option<Vec<String>>), StoreError> {
    let excluded = root
        .excluded_account_keys
        .value()
        .cloned()
        .unwrap_or_default();
    let project = match root
        .project_id
        .as_deref()
        .zip(root.project_server_id.as_deref())
    {
        Some((id, home)) => {
            let id = id.to_owned();
            store
                .read_async("runtime-services/managed_child_create", move |read| {
                    studio_runtime_store::ProjectsRepository::get(read, &id)
                })
                .await?
                .map(|(_, record)| (record, home))
        }
        None => None,
    }
    .filter(|(record, home)| record["homeServerId"] == *home)
    .map(|(record, _)| record);
    let groups = project.as_ref().map(|record| {
        crate::account_preference::groups_of(
            record.get(crate::account_preference::PROJECT_GROUPS_FIELD),
        )
    });
    let keys = project.as_ref().and_then(|record| {
        record["accountKeys"].as_array().map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
    });
    Ok((excluded, keys, groups))
}

async fn select_review_account(
    store: &RuntimeStore,
    environment: &dyn ManagedCreationEnvironment,
    parent: &AgentRecord,
    root: &AgentRecord,
    explicit: Option<&str>,
    model: &str,
    account_load: &std::collections::HashMap<String, i64>,
) -> Result<(String, crate::host_worker_create::AccountIdentity), WorkerSettingsError> {
    let selected = explicit;
    let (excluded, keys, groups) = account_scope(store, root).await?;
    let caller_eligible = !excluded.contains(&parent.account_key)
        && keys
            .as_ref()
            .is_none_or(|keys| keys.is_empty() || keys.contains(&parent.account_key));
    let caller_offers = selected.is_none()
        && caller_eligible
        && environment.account_offers_model(&parent.account_key, model)?;
    Ok(
        environment.select_worker_account(crate::provider_callbacks::WorkerAccountRequest {
            parent_account_key: &parent.account_key,
            requested_account_key: selected
                .or_else(|| caller_offers.then_some(parent.account_key.as_str())),
            model,
            excluded_account_keys: &excluded,
            project_account_keys: keys.as_deref(),
            project_account_groups: groups.as_deref(),
            active_agents: account_load,
        })?,
    )
}

pub(crate) async fn resolve_worker_settings(
    store: &RuntimeStore,
    environment: &dyn ManagedCreationEnvironment,
    request: WorkerSettingsRequest<'_>,
) -> Result<ResolvedWorkerSettings, WorkerSettingsError> {
    let WorkerSettingsRequest {
        parent,
        root,
        spec,
        profile,
        role,
        requested_account_key,
        account_load,
    } = request;
    let defaults = root.worker_defaults.value();
    let known = match defaults {
        Some(studio_runtime_store::StoredWorkerDefaults::Known(defaults)) => Some(defaults),
        _ => None,
    };
    let unreadable = match defaults {
        Some(studio_runtime_store::StoredWorkerDefaults::Unreadable(value)) => Some(value),
        _ => None,
    };
    let default_model = known
        .and_then(|value| value.model.value().map(String::as_str))
        .or_else(|| {
            unreadable
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
        });
    let default_effort = known
        .and_then(|value| value.effort.value().map(String::as_str))
        .or_else(|| {
            unreadable
                .and_then(|value| value.get("effort"))
                .and_then(Value::as_str)
        });
    let default_fast = known
        .and_then(|value| value.fast_mode.value().copied())
        .or_else(|| {
            unreadable
                .and_then(|value| value.get("fastMode"))
                .and_then(Value::as_bool)
        });
    let default_daybreak = known
        .and_then(|value| value.daybreak_enabled.value().copied())
        .or_else(|| {
            unreadable
                .and_then(|value| value.get("daybreakEnabled"))
                .and_then(Value::as_bool)
        });
    let profile_model = profile.map(|profile| profile.model.as_str());
    let review = (role == "reviewer")
        .then(|| crate::native_review::execution_settings(parent, root, &spec.model, &spec.effort));
    let requested_model = spec
        .model
        .value()
        .map(String::as_str)
        .or(profile_model)
        .or_else(|| review.as_ref().and_then(|(model, _, _)| model.as_deref()))
        .or(default_model)
        .or(root.model.as_deref())
        .unwrap_or(crate::agents_create::DEFAULT_LEAD_MODEL);
    let (account_key, account) = if role == "reviewer" {
        select_review_account(
            store,
            environment,
            parent,
            root,
            requested_account_key,
            requested_model,
            account_load,
        )
        .await?
    } else {
        let (excluded, keys, groups) = account_scope(store, root).await?;
        environment.select_worker_account(crate::provider_callbacks::WorkerAccountRequest {
            parent_account_key: &parent.account_key,
            requested_account_key,

            model: requested_model,
            excluded_account_keys: &excluded,
            project_account_keys: keys.as_deref(),
            project_account_groups: groups.as_deref(),
            active_agents: account_load,
        })?
    };
    let adapter = crate::provider_adapter::adapter(account.provider);
    let model = if requested_model == crate::agents_create::DEFAULT_LEAD_MODEL
        && root.model.is_none()
        && default_model.is_none()
        && profile_model.is_none()
        && spec.model.value().is_none()
        && !adapter.owns_model(requested_model)
    {
        adapter.default_model()
    } else {
        requested_model
    };
    let profile_effort = profile.and_then(|profile| profile.effort);
    let mut effort = if !spec.effort.is_inherit() {
        spec.effort.selected().map(String::as_str)
    } else if let Some(effort) = profile_effort {
        Some(effort.as_str())
    } else if let Some((_, effort, _)) = &review {
        effort.as_deref()
    } else {
        default_effort
    };
    let fast_mode = spec
        .fast_mode
        .value()
        .copied()
        .or_else(|| review.as_ref().and_then(|(_, _, fast)| *fast))
        .or(default_fast)
        .unwrap_or(false);
    let daybreak = spec
        .daybreak_enabled
        .value()
        .copied()
        .or(default_daybreak)
        .unwrap_or(false);
    // An inherited effort unsupported by the selected model uses its native default.
    // A request or profile explicitly choosing an effort remains a validation error.
    if spec.effort.is_inherit()
        && profile_effort.is_none()
        && effort.is_some()
        && matches!(
            environment.catalog().resolve(
                &account_key,
                &account,
                model,
                effort,
                fast_mode,
                daybreak
            ),
            Err(crate::agents_create::CreateError::InvalidCreation("effort"))
        )
    {
        effort = None;
    }
    Ok(ResolvedWorkerSettings {
        account_key,
        account,
        model: model.to_owned(),
        effort: effort.map(str::to_owned),
        fast_mode,
        daybreak,
    })
}

impl From<WorkerSettingsError> for Error {
    fn from(error: WorkerSettingsError) -> Self {
        match error {
            WorkerSettingsError::Environment(error) => error.into(),
            WorkerSettingsError::Store(error) => Self::Store(error),
        }
    }
}

struct Draft {
    typed: AgentRecord,
    record: AgentRecord,
    id: String,
    prompt: String,
    task_id: Option<String>,
    intent: studio_runtime_schema::creation_intent::CreationIntent,
    worktree: bool,
    workspace: String,
    worker_base_main_ref: Option<String>,
    worker_base_behind_main: Option<i64>,
    epoch: i64,
    account: crate::host_worker_create::AccountIdentity,
    workspace_admission: Box<dyn crate::monitor_runtime::WorkspaceAdmission + Send + Sync>,
}

async fn prepare_reviewer(
    store: &RuntimeStore,
    directory: &str,
    assignment: &studio_runtime_schema::review::ReviewAssignment,
) -> Result<
    (
        studio_runtime_schema::review::ReviewAssignment,
        crate::native_review::ResolvedBase,
    ),
    Error,
> {
    use crate::native_review::ResolvedBase;
    use studio_runtime_schema::review::{GitCommit, LayrBase, LayrStateId, ReviewTarget};
    let invalid = || crate::managed_host_tools::Error::Invalid("Could not resolve review base");
    if let ReviewTarget::LayrState { state, .. } = assignment.target() {
        let resolved = crate::layr_native::run(
            store.state_dir(),
            Path::new(directory),
            &["rev-parse", state.as_str()],
        )
        .map_err(|_| {
            crate::managed_host_tools::Error::Invalid("Review state must exist in this project")
        })?;
        if resolved != state.as_str() {
            return Err(crate::managed_host_tools::Error::Invalid(
                "Review state must exist in this project",
            )
            .into());
        }
    }
    let base = match assignment.target() {
        ReviewTarget::LayrState {
            state,
            base: LayrBase::MergeBaseWithMain,
        } => {
            let root = store.root().to_owned();
            let cwd = PathBuf::from(directory);
            let state = state.to_string();
            let output = studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-services/review-layr-base",
                move || {
                    // workerBaseRef is a Git-worktree setting, not a Layr integration ref.
                    let main =
                        if crate::layr_native::run(&root, &cwd, &["rev-parse", "origin/main"])
                            .is_ok()
                        {
                            "origin/main"
                        } else {
                            "main"
                        };
                    crate::layr_native::run(&root, &cwd, &["merge-base", main, &state])
                },
            )
            .await
            .map_err(|_| invalid())?
            .map_err(|_| invalid())?;
            ResolvedBase::Layr(
                LayrStateId::try_from(output.trim().to_owned()).map_err(|_| invalid())?,
            )
        }
        ReviewTarget::LayrState {
            base: LayrBase::State { state },
            ..
        } => ResolvedBase::Layr(state.clone()),
        ReviewTarget::GitRange { base, .. } => {
            let cwd = PathBuf::from(directory);
            let revision = format!("{}^{{commit}}", base.as_str());
            let output = studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-services/review-git-base",
                move || {
                    crate::native_review::git_read(
                        &cwd,
                        &["rev-parse", "--verify", "--end-of-options", &revision],
                        256,
                    )
                },
            )
            .await
            .map_err(|_| invalid())?
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
            ResolvedBase::Git(GitCommit::try_from(output.trim().to_owned()).map_err(|_| invalid())?)
        }
    };
    let assignment = crate::native_review::pin_assignment(assignment, &base)
        .map_err(|_| crate::managed_host_tools::Error::Invalid("Invalid review base"))?;
    Ok((assignment, base))
}

pub(crate) fn reviewer_brief(
    assignment: &studio_runtime_schema::review::ReviewAssignment,
) -> String {
    use studio_runtime_schema::review::{LayrBase, ReviewTarget};
    match assignment.target() {
        ReviewTarget::LayrState { state, base } => {
            let comparison = match base {
                LayrBase::MergeBaseWithMain => format!(
                    "Find the merge base with main using layr_read, then compare it with {state} using layr_read diff."
                ),
                LayrBase::State { state: base } => {
                    format!("Compare using layr_read diff {base} {state}.")
                }
            };
            format!(
                "\n\n[Studio review] Review Layr state {state}. Use the review line and layr_read to inspect files. {comparison}"
            )
        }
        ReviewTarget::GitRange { base, head } => format!(
            "\n\n[Studio review] Review Git commit {head} against {base}. Inspect the working copy and use git diff {base}..{head}."
        ),
    }
}

#[cfg(test)]
mod review_brief_tests {
    use super::*;
    use studio_runtime_schema::review::{ReviewAssignment, ReviewTarget, WorkspaceMode};
    #[test]
    fn briefs_match_each_workspace() {
        let layr: ReviewTarget = studio_json::from_str(r#"{"kind":"layrState","state":"0123456789abcdef0123456789abcdef","base":{"kind":"mergeBaseWithMain"}}"#).unwrap();
        let git: ReviewTarget = studio_json::from_str(r#"{"kind":"gitRange","base":"main","head":"0123456789abcdef0123456789abcdef01234567"}"#).unwrap();
        assert_eq!(
            reviewer_brief(&ReviewAssignment::new(WorkspaceMode::LayrNative, layr).unwrap()),
            "\n\n[Studio review] Review Layr state 0123456789abcdef0123456789abcdef. Use the review line and layr_read to inspect files. Find the merge base with main using layr_read, then compare it with 0123456789abcdef0123456789abcdef using layr_read diff."
        );
        assert_eq!(
            reviewer_brief(&ReviewAssignment::new(WorkspaceMode::Worktree, git).unwrap()),
            "\n\n[Studio review] Review Git commit 0123456789abcdef0123456789abcdef01234567 against main. Inspect the working copy and use git diff main..0123456789abcdef0123456789abcdef01234567."
        );
    }
}

/// Create a local host batch using the P02 typed draft and recheck contract.
/// `request_id` is the callback's durable operation identity, so child ids and
/// initial-event ids remain stable across retries.
pub(crate) async fn spawn_agents(
    store: &RuntimeStore,
    actor: &crate::managed_host_tools::Actor,
    args: &Value,
    request_id: &str,
    now: f64,
    environment: &dyn ManagedCreationEnvironment,
) -> Result<crate::managed_host_tools::DispatchedOperation, Error> {
    let actor_record = Box::new(actor.record().clone());
    if actor_record.role.value().copied()
        == Some(studio_runtime_schema::creation_intent::AgentRole::Reviewer)
    {
        return Err(
            crate::managed_host_tools::Error::Invalid("Reviewers cannot create agents").into(),
        );
    }
    if !now.is_finite() || now < 0.0 || request_id.is_empty() {
        return Err(crate::managed_host_tools::Error::Invalid("Invalid spawn identity").into());
    }
    if args
        .get("server")
        .is_some_and(|value| event_write::truthy(Some(value)))
    {
        return Err(Error::Unavailable("multi-server/worker-call"));
    }
    let specs = args
        .get("agents")
        .and_then(Value::as_array)
        .filter(|items| (1..=MAX_SPAWN_BATCH_AGENTS).contains(&items.len()))
        .ok_or(crate::managed_host_tools::Error::Invalid(
            crate::managed_host_tools::SPAWN_BATCH_SIZE_REFUSAL,
        ))?;
    let parent_value = actor_record.value();
    let root_id = actor_record.root_id.0.as_str();
    let root = Box::new(stored_root(store, AgentId::from(root_id)).await?);
    event_write::assert_delegation(&root)
        .map_err(|_| crate::managed_host_tools::Error::Invalid("Delegation is disabled"))?;
    let parent_cwd =
        actor_record
            .cwd
            .as_deref()
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Select a project directory",
            ))?;
    let parent_account = parent_value
        .get("accountKey")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let mut drafts = Vec::with_capacity(specs.len());
    let mut task_ids = std::collections::BTreeSet::new();

    // Workers without an explicit account go to the least loaded one; the count
    // grows with each pick so a batch spreads across accounts as well.
    let mut account_load = store
        .read_async("runtime-services/managed_child_create", |read| {
            studio_runtime_store::AgentIndexedReads::slots_by_account(read)
        })
        .await?;
    for (index, raw_spec) in specs.iter().enumerate() {
        let spec: studio_runtime_schema::creation_intent::CreationIntent =
            studio_json::from_value(raw_spec.clone()).map_err(|_| {
                crate::managed_host_tools::Error::Invalid("Invalid worker creation settings")
            })?;
        let profile =
            if let Some(profile_id) = spec.profile_id.value().filter(|value| !value.is_empty()) {
                let profile_id = profile_id.to_owned();
                Some(
                    store
                        .read_async("runtime-services/managed_child_create", move |read| {
                            studio_runtime_store::Table::<studio_runtime_store::ProfileRecord>::new(
                                read,
                            )
                            .get(&profile_id)
                        })
                        .await?
                        .ok_or(crate::managed_host_tools::Error::Invalid(
                            "Unknown worker profile",
                        ))?,
                )
            } else {
                None
            };
        let name = spec
            .name
            .as_deref()
            .filter(|text| !text.trim().is_empty() && text.trim().chars().count() <= MAX_NAME_CHARS)
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Every worker needs a name, task and valid role",
            ))?;
        let prompt = spec
            .prompt
            .as_deref()
            .filter(|text| {
                !text.trim().is_empty() && text.trim().chars().count() <= MAX_PROMPT_CHARS
            })
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Every worker needs a name, task and valid role",
            ))?;
        let role = spec.role.map(|role| role.as_str()).unwrap_or_else(|| {
            profile
                .as_ref()
                .map(|profile| match profile.role {
                    studio_runtime_store::ProfileRole::Implementer => "implementer",
                    studio_runtime_store::ProfileRole::Reviewer => "reviewer",
                })
                .unwrap_or("implementer")
        });
        if !matches!(role, "implementer" | "reviewer") {
            return Err(crate::managed_host_tools::Error::Invalid(
                "Every worker needs a name, task and valid role",
            )
            .into());
        }
        if let Some(task) = spec.task_id.as_deref() {
            let task = Some(task).filter(|value| !value.is_empty()).ok_or(
                crate::managed_host_tools::Error::Invalid("task_id must be a task id"),
            )?;
            if actor_record.is_lead != Some(true) {
                return Err(crate::managed_host_tools::Error::Invalid(
                    "Only the lead can delegate an existing task; ask the lead to assign it",
                )
                .into());
            }
            if !task_ids.insert(task.to_owned()) {
                return Err(crate::managed_host_tools::Error::Invalid(
                    "Assign each task to one worker in a batch",
                )
                .into());
            }
        }
        let requested_base_ref = if let Some(base_ref) = spec.base_ref.as_deref() {
            let value = Some(base_ref)
                .filter(|value| !value.trim().is_empty() && value.chars().count() <= 1024)
                .ok_or(crate::managed_host_tools::Error::Invalid(
                    "base_ref must be a branch, tag, or commit",
                ))?;
            Some(value.trim())
        } else {
            None
        };
        if spec.server.as_ref().is_some_and(|value| !value.is_empty()) {
            return Err(Error::Unavailable("multi-server/worker-call"));
        }
        if parent_value.get("executionMode").and_then(Value::as_str) == Some("vm") {
            return Err(crate::managed_host_tools::Error::Unavailable(
                "workspace-state/unavailable",
            )
            .into());
        }
        let alternate_directory = spawn_directory(parent_cwd, spec.cwd.as_deref())?
            .filter(|directory| directory.as_str() != parent_cwd);
        let child_directory = alternate_directory.as_deref().unwrap_or(parent_cwd);
        if role == "reviewer" && spec.workspace.is_some() {
            return Err(crate::managed_host_tools::Error::Invalid(
                "workspace applies to implementers; reviewers use the shared folder with read-only access",
            )
            .into());
        }
        let assignment = if let Some(target) = &spec.review {
            if role != "reviewer" || alternate_directory.is_some() {
                return Err(crate::managed_host_tools::Error::Invalid(
                    "A review target requires a reviewer in the parent's project",
                )
                .into());
            }
            let mode = actor_record
                .workspace_mode
                .unwrap_or(studio_runtime_schema::records::WorkspaceMode::Shared);
            let assignment =
                studio_runtime_schema::review::ReviewAssignment::new(mode, target.clone())
                    .map_err(|_| {
                        crate::managed_host_tools::Error::Invalid(
                            "Review target does not match the project workspace",
                        )
                    })?;
            Some(prepare_reviewer(store, parent_cwd, &assignment).await?.0)
        } else {
            None
        };
        let workspace = assignment
            .as_ref()
            .map(|assignment| match assignment.target() {
                studio_runtime_schema::review::ReviewTarget::LayrState { .. } => "layr-native",
                studio_runtime_schema::review::ReviewTarget::GitRange { .. } => "worktree",
            })
            .unwrap_or_else(|| {
                spec.workspace
                    .map(|mode| mode.as_str())
                    .unwrap_or_else(|| default_workspace(role, &parent_value))
            });
        if workspace == "layr-native" && !crate::layr_native::is_native(&parent_value) {
            return Err(crate::managed_host_tools::Error::Invalid(
                "A native Layr workspace requires a native Layr parent",
            )
            .into());
        }
        if !studio_runtime_schema::workspace_modes::accepts_worker(workspace) {
            return Err(crate::managed_host_tools::Error::Invalid("Unknown workspace mode").into());
        }
        if crate::projects::refuses_worktree(store, child_directory, workspace).await? {
            return Err(crate::managed_host_tools::Error::Invalid(
                crate::projects::WORKTREE_REFUSED_BY_LAYR_POLICY,
            )
            .into());
        }
        let worktree = workspace == "worktree";
        // Python `git_toplevel` decides Git support for an implementer's
        // folder; a reviewer has no repository there (`spawn_agents`).
        let in_git = role == "implementer"
            && crate::host_worker_create::inside_git_work_tree(Path::new(child_directory));
        let explicit_workspace = spec.workspace.map(|mode| mode.as_str());
        if explicit_workspace == Some("worktree") && !in_git {
            return Err(crate::managed_host_tools::Error::Invalid(
                "workspace \"worktree\" requires a Git repository",
            )
            .into());
        }
        if workspace != "layr-native" && requested_base_ref.is_some() && !in_git {
            return Err(crate::managed_host_tools::Error::Invalid(
                "base_ref requires an implementer in a Git repository",
            )
            .into());
        }
        if workspace == "shared" && spec.base_ref.is_some() {
            return Err(crate::managed_host_tools::Error::Invalid(
                "base_ref cannot be used with workspace \"shared\"",
            )
            .into());
        }
        // Python `select_workspace`: the parent's default worktree mode uses
        // the shared folder, with a note, where the folder has no Git.
        let workspace_note = (role == "implementer"
            && worktree
            && !in_git
            && explicit_workspace.is_none()
            && parent_value.get("workspaceMode").and_then(Value::as_str) == Some("worktree"))
        .then_some(NO_GIT_WORKTREE_NOTE);
        let worktree = worktree && in_git;
        let id = child_id(request_id, index);
        let requested_account_key = spec.account_key.as_deref();
        let workspace_admission =
            environment.prepare_workspace(store, &actor_record, request_id)?;
        let resolved = resolve_worker_settings(
            store,
            environment,
            WorkerSettingsRequest {
                parent: &actor_record,
                root: &root,
                spec: &spec,
                profile: profile.as_ref(),
                role,
                requested_account_key,
                account_load: &account_load,
            },
        )
        .await?;
        *account_load
            .entry(resolved.account_key.clone())
            .or_default() += 1;
        let account_key = &resolved.account_key;
        let account = resolved.account;
        let model = &resolved.model;
        let effort = resolved.effort.as_deref();
        let fast_mode = resolved.fast_mode;
        let daybreak = resolved.daybreak;
        let host_cwd = child_directory;
        // The parent's project binding names the parent's folder. Another
        // folder uses its own path settings, as Python `settings_key` does.
        let parent_project_id = parent_value
            .get("projectId")
            .filter(|_| alternate_directory.is_none());
        let project = stored_project(
            store,
            parent_project_id
                .and_then(Value::as_str)
                .unwrap_or(host_cwd),
            host_cwd,
        )
        .await?;
        let project_worker_base = if requested_base_ref.is_none() && in_git {
            worker_base_for_directory(store, parent_project_id, host_cwd).await?
        } else {
            None
        };
        let project_worker_base = project_worker_base.as_deref();
        let base_ref_request = requested_base_ref.or(project_worker_base);
        let (worktree, (base_ref, base_commit, main_ref, behind_main)) =
            resolve_child_base(worktree, Path::new(host_cwd), base_ref_request)?;
        let workspace_in_git = in_git;
        let body = json!({"account_key":account_key,"name":name,"model":model,"effort":effort,
            "prompt":prompt,"workspace_explicit":spec.workspace.is_some(),
            "_workspaceInGit":workspace_in_git});
        let mut draft = crate::host_worker_create::prepare_work_item(
            &body,
            Path::new(host_cwd),
            AgentId::from(id.as_str()),
            actor_record.root_id.clone(),
            actor_record.id.clone(),
            &base_ref,
            &base_commit,
            actor_record.epoch,
            parent_cwd,
            host_cwd,
            parent_account,
            parent_value.get("projectId"),
            parent_value.get("projectServerId"),
            project.as_ref(),
            &account,
            environment.catalog(),
            role,
            worktree,
            fast_mode,
            daybreak,
        )
        .map_err(map_create_error)?;
        draft.extra.insert(
            "_p02ProjectLookupId".into(),
            json!(
                parent_project_id
                    .and_then(Value::as_str)
                    .unwrap_or(host_cwd)
            ),
        );
        let mut record =
            crate::host_worker_create::persisted_record(&draft).map_err(map_create_error)?;
        if assignment.is_some() {
            record.workspace_mode = Some(if workspace == "layr-native" {
                studio_runtime_schema::records::WorkspaceMode::LayrNative
            } else {
                studio_runtime_schema::records::WorkspaceMode::Worktree
            });
        }
        record.review_assignment = assignment;
        if workspace == "layr-native" {
            crate::layr_native::configure_child(&mut record, &actor_record, &root);
            if let Some(base_ref) = requested_base_ref {
                let base_state = crate::layr_native::run(
                    store.state_dir(),
                    Path::new(parent_cwd),
                    &["rev-parse", base_ref],
                )
                .map_err(|_| {
                    crate::managed_host_tools::Error::Invalid(
                        "Native Layr base revision could not be resolved",
                    )
                })?;
                record.layr_base_state = Some(base_state);
            }
        }
        if let Some(note) = workspace_note {
            record.worktree_warning = Some(note.to_owned());
        }
        // An implementer outside Git uses the selected folder.
        if role == "implementer" && !worktree && workspace != "layr-native" && !in_git {
            record.worktree_warning = Some(no_worktree_warning(
                record.cwd.as_deref().unwrap_or(child_directory),
            ));
        }
        record.provider = Field::Value(account.provider);
        record.worker_base_main_ref = main_ref.clone();
        record.worker_base_behind_main = behind_main
            .map(u64::try_from)
            .transpose()
            .map_err(|_| Error::Refused)?;
        // Python's AgentFactory persists these keys on every row (including
        // rows without a selected profile): codex_runtime.py:5014-5015.
        record.profile_id = spec
            .profile_id
            .value()
            .cloned()
            .map_or(Field::Null, Field::Value);
        record.profile_instructions = Field::Value(String::new());
        if let Some(profile) = &profile {
            record.profile_instructions = Field::Value(profile.instructions.clone());
        }
        record.name = Some(name.trim().to_owned());
        record.permission_mode = Some(root.permission_mode.unwrap_or(
            studio_runtime_schema::claude_permissions::PermissionMode::BypassPermissions,
        ));
        record.capacity = studio_runtime_store::AgentCapacity::Automatic {
            max: studio_runtime_schema::AgentCounter::from_i64(root.capacity.max())
                .ok_or(Error::Refused)?,
        };
        drafts.push(Draft {
            typed: draft,
            record,
            id,
            prompt: prompt.trim().to_owned(),
            task_id: spec.task_id.clone(),
            intent: spec.clone(),
            worktree,
            workspace: workspace.to_owned(),
            worker_base_main_ref: main_ref,
            worker_base_behind_main: behind_main,
            epoch: actor_record.epoch,
            account,
            workspace_admission,
        });
    }

    let request_id = request_id.to_owned();
    let result = store.transaction_with_error(move |tx| -> Result<crate::managed_host_tools::OperationResult, crate::managed_host_tools::Error> {
        recheck_actor(tx, &actor_record)?;
        let current_root = AgentRepository::get(tx, &actor_record.root_id)?
            .ok_or(crate::managed_host_tools::Error::Stale)?;
        let current_parent = AgentRepository::get(tx, &actor_record.id)?
            .ok_or(crate::managed_host_tools::Error::Stale)?;
        event_write::assert_delegation(&current_root)
            .map_err(|_| crate::managed_host_tools::Error::Invalid("Delegation is disabled"))?;
        let max = current_root.capacity.max();
        let existing = drafts.iter().map(|draft| AgentRepository::get(tx, &AgentId::from(draft.id.as_str())))
            .collect::<Result<Vec<_>, _>>()?;
        let new_count = existing.iter().filter(|item| item.is_none()).count() as i64;
        if new_count != 0 && new_count != drafts.len() as i64 {
            return Err(crate::managed_host_tools::Error::Invalid("This request id has different content"));
        }
        if active_agents(tx, &actor_record.root_id.0)? + new_count > max {
            return Err(crate::managed_host_tools::Error::Invalid("This batch exceeds the active agent limit; no workers were created"));
        }
        for draft in &drafts {
            draft.workspace_admission.check(tx, &actor_record)
                .map_err(|error| match error {
                    crate::monitor_policy::MonitorError::Store(error) => crate::managed_host_tools::Error::Store(error),
                    error @ (crate::monitor_policy::MonitorError::Expired
                        | crate::monitor_policy::MonitorError::InvalidStored { .. }
                        | crate::monitor_policy::MonitorError::InvalidResultReceipt(_)
                        | crate::monitor_policy::MonitorError::InvalidDeliveryIdentity(_)
                        | crate::monitor_policy::MonitorError::InvalidOutputReceipt(_)
                        | crate::monitor_policy::MonitorError::TaskFailed
                        | crate::monitor_policy::MonitorError::Refused(_)
                        | crate::monitor_policy::MonitorError::InputOutcomeUnknown
                        | crate::monitor_policy::MonitorError::Unavailable(_)
                        | crate::monitor_policy::MonitorError::ProviderRequest { .. }
                        | crate::monitor_policy::MonitorError::ConfigurationRejected { .. }
                        | crate::monitor_policy::MonitorError::ConfigurationReceipt { .. }
                        | crate::monitor_policy::MonitorError::Event(_)
                        | crate::monitor_policy::MonitorError::Rules(_)
                        | crate::monitor_policy::MonitorError::Io(_)) => crate::managed_host_tools::Error::Refusal(error.to_string()),
                })?;
            crate::host_worker_create::recheck_worker(tx, &draft.typed, draft.epoch, &draft.account)
                .map_err(map_recheck_error)?;
        }
        if !can_delegate(tx, &actor_record, &current_root)? {
            return Err(crate::managed_host_tools::Error::Invalid("Delegation depth reached"));
        }
        let mut children = Vec::with_capacity(drafts.len());
        let mut signals = Vec::new();
        for (draft, saved) in drafts.iter_mut().zip(existing) {
            if let Some(saved) = saved {
                children.push(project_child(&saved.value(), draft.task_id.as_deref(), draft.worktree));
                continue;
            }
            crate::child_permissions::inherit_claude_permissions(&mut draft.record, &current_parent, &current_root, draft.intent.permission_mode).map_err(crate::managed_host_tools::Error::Invalid)?;
            let mut text = draft.prompt.clone();
            if let Some(assignment) = &draft.record.review_assignment {
                text.push_str(&reviewer_brief(assignment));
            }
            if draft.worktree {
                text.push_str(&format!("\n\n[Studio worker base] Commit {} from ref {}.", draft.record.worker_base_commit.as_deref().unwrap_or_default(), draft.record.worker_base_ref.as_deref().unwrap_or("HEAD")));
                if let Some(behind) = draft.worker_base_behind_main.filter(|behind| *behind != 0) {
                    text.push_str(&format!(" This base is {behind} commits behind {}. Check for newer project instructions before starting.", draft.worker_base_main_ref.as_deref().unwrap_or("main")));
                }
            }
            if let Some(task) = &draft.task_id {
                let work = studio_runtime_store::WorkItemsRepository::work_record(tx, task)?
                    .ok_or(crate::managed_host_tools::Error::Invalid("Unknown task_id in this team; no workers were created"))?;
                let title = work.title().unwrap_or("Assigned task");
                text.push_str(&format!("\n\n[Studio task {task}] {title}\nYou own this task. Submit the result with the work item tools."));
                text.push_str(&studio_runtime_schema::task_results::assignment_text(&work.value()));
            }
            let record = &draft.record;
            AgentRepository::put(tx, record)?;
            let record_value = record.value();
            record_spawn_effect(tx, &actor_record, record, &request_id);
            crate::agents_create::publish_agent(tx, &record_value)?;
            if let Some(task_id) = &draft.task_id {
                crate::work_assignment_runtime::assign_in_transaction(tx, task_id, &draft.id, &actor_record, now)
                    .map_err(|_| crate::managed_host_tools::Error::Invalid("Work item assignment was rejected"))?;
            }
            let inserted = event_write::enqueue_event(tx, record, format!("{}:initial", draft.id), "user", text, now)
                .map_err(|_| crate::managed_host_tools::Error::Invalid("Initial turn intent was rejected"))?;
            if !inserted {
                return Err(crate::managed_host_tools::Error::Invalid("This request id has different content"));
            }
            children.push(project_child(&record_value, draft.task_id.as_deref(), draft.worktree));
            signals.push(crate::managed_host_tools::WakeSignal::new("worker-driver"));
        }
        Ok(crate::managed_host_tools::OperationResult {
            signals,
            ..crate::managed_host_tools::OperationResult::reply(&json!({"requestId":request_id,"agents":children,"delivery":"Results wake you automatically. Finish your turn while waiting."}))
        })
    }).await.map_err(Error::from)?;
    Ok(crate::managed_host_tools::DispatchedOperation::Applied(
        result,
    ))
}

pub(crate) fn child_id(request_id: &str, index: usize) -> String {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("{request_id}:{index}").as_bytes(),
    )
    .to_string()
}

/// The stored root agent; a missing root makes the request stale.
async fn stored_root(store: &RuntimeStore, root_id: AgentId) -> Result<AgentRecord, Error> {
    store
        .read_async("runtime-services/managed_child_create", move |read| {
            AgentRepository::get(read, &root_id)?
                .ok_or_else(|| crate::managed_host_tools::Error::Stale.into())
        })
        .await
}

/// The project stored under `project_id`, else the one stored under `fallback`.
async fn stored_project(
    store: &RuntimeStore,
    project_id: &str,
    fallback: &str,
) -> Result<Option<Value>, Error> {
    let (project_id, fallback) = (project_id.to_owned(), fallback.to_owned());
    store
        .read_async("runtime-services/managed_child_create", move |read| {
            Ok::<_, Error>(
                studio_runtime_store::ProjectsRepository::get(read, &project_id)?
                    .or(studio_runtime_store::ProjectsRepository::get(
                        read, &fallback,
                    )?)
                    .map(|(_, project)| project),
            )
        })
        .await
}

/// Match Python `Runtime.project_worker_base`: project handles are exact ids;
/// filesystem directories inherit the setting from their deepest ancestor.
async fn worker_base_for_directory(
    store: &RuntimeStore,
    project_id: Option<&Value>,
    cwd: &str,
) -> Result<Option<String>, Error> {
    if let Some(handle) = project_id
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("project:"))
    {
        let handle = handle.to_owned();
        let (_, project) = store
            .read_async("runtime-services/managed_child_create", move |read| {
                studio_runtime_store::ProjectsRepository::get(read, &handle)
            })
            .await?
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Select an existing project",
            ))?;
        return Ok(project
            .get("workerBaseRef")
            .and_then(Value::as_str)
            .map(str::to_owned));
    }
    let projects = store
        .read_async("runtime-services/managed_child_create", |read| {
            studio_runtime_store::ProjectsRepository::list(read)
        })
        .await?;
    let directory = canonical_or_expanded(cwd);
    Ok(projects
        .into_iter()
        .filter_map(|(_, project)| {
            let worker_base = project.get("workerBaseRef")?.as_str()?;
            let path = project.get("path")?.as_str()?;
            let path = canonical_or_expanded(path);
            directory
                .starts_with(&path)
                .then_some((path.components().count(), worker_base.to_owned()))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, worker_base)| worker_base))
}

fn canonical_or_expanded(path: &str) -> PathBuf {
    let expanded = if path == "~" {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path))
    } else if let Some(suffix) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(suffix))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    };
    CanonicalPath::existing(&expanded)
        .map(CanonicalPath::into_path_buf)
        .unwrap_or(expanded)
}

fn recheck_actor(
    tx: &Transaction<'_>,
    actor: &AgentRecord,
) -> Result<(), crate::managed_host_tools::Error> {
    let current =
        AgentRepository::get(tx, &actor.id)?.ok_or(crate::managed_host_tools::Error::Stale)?;
    if current.epoch != actor.epoch
        || current.root_id != actor.root_id
        || current.account_key != actor.account_key
        || current.provider != actor.provider
        || current.is_deleted()
        || current.auto_wake != Some(true)
    {
        return Err(crate::managed_host_tools::Error::Stale);
    }
    Ok(())
}

fn active_agents(
    tx: &Transaction<'_>,
    root_id: &str,
) -> Result<i64, crate::managed_host_tools::Error> {
    Ok(studio_runtime_store::AgentIndexedReads::active_team_members(tx, root_id)?)
}

/// Preserve Python's best-effort execution history for a spawned child. The
/// child row itself is authoritative; a broken optional history table must not
/// turn successful child creation into a refusal.
fn record_spawn_effect(
    tx: &Transaction<'_>,
    actor: &AgentRecord,
    child: &AgentRecord,
    request_id: &str,
) {
    let result = (|| -> Result<(), rusqlite::Error> {
        tx.execute_batch("SAVEPOINT managed_spawn_execution_effect")?;
        let write = (|| -> Result<(), rusqlite::Error> {
            let has_runs: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_execution_runs')",
                [],
                |row| row.get(0),
            )?;
            if !has_runs {
                return Ok(());
            }
            let account = actor.account_key.as_str();
            let thread = actor.thread_id.as_deref();
            let turn = actor.turn_id.as_deref();
            let active_turn = turn.filter(|turn| !turn.is_empty());
            let run = if let Some(turn) = active_turn {
                let mut statement = tx.prepare_cached(
                    "SELECT id,record FROM runtime_execution_runs WHERE agent=?1 AND account=?2 AND thread IS ?3 AND turn=?4 LIMIT 2",
                )?;
                let rows = statement
                    .query_map(
                        rusqlite::params![actor.id.0, account, thread, turn],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                (rows.len() == 1).then(|| rows[0].clone())
            } else {
                tx.query_row(
                    "SELECT id,record FROM runtime_execution_runs WHERE agent=?1 AND account=?2 AND epoch=?3 AND turn IS NULL ORDER BY created DESC LIMIT 1",
                    rusqlite::params![actor.id.0, account, actor.epoch],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
            };
            let Some((run_id, raw_run)) = run else {
                return Ok(());
            };
            let Ok(run) = studio_json::from_str::<Value>(&raw_run) else {
                return Ok(());
            };
            if active_turn.is_none()
                && matches!(
                    run.get("status").and_then(Value::as_str),
                    Some("completed" | "failed" | "interrupted" | "cancelled")
                )
            {
                return Ok(());
            }
            let child_id = child.id.0.as_str();
            let effect_id = format!("spawn:{child_id}");
            let previous: Option<String> = tx
                .query_row(
                    "SELECT record FROM runtime_execution_effects WHERE id=?1",
                    [&effect_id],
                    |row| row.get(0),
                )
                .optional()?;
            let previous: Option<Value> = previous.and_then(|raw| studio_json::from_str(&raw).ok());
            let effect_run = previous
                .as_ref()
                .and_then(|value| value.get("runId"))
                .and_then(Value::as_str)
                .unwrap_or(&run_id);
            let attempt_id = previous
                .as_ref()
                .and_then(|value| value.get("attemptId"))
                .cloned()
                .or_else(|| run.get("latestAttemptId").cloned())
                .unwrap_or(Value::Null);
            let effect = json!({"id":effect_id,"runId":effect_run,"kind":"spawn",
                "referenceId":child_id,"attemptId":attempt_id,"requestId":request_id,
                "status":child.status});
            tx.execute(
                "INSERT INTO runtime_execution_effects(id,run,kind,reference,record) VALUES(?1,?2,'spawn',?3,?4) ON CONFLICT(id) DO UPDATE SET run=excluded.run,kind=excluded.kind,reference=excluded.reference,record=excluded.record",
                rusqlite::params![effect_id, effect_run, child_id, effect.to_string()],
            )?;
            let node_id = format!("worker:{child_id}");
            let node = json!({"id":node_id,"runId":run_id,"kind":"managed_worker",
                "agentId":child_id,"status":child.status});
            tx.execute(
                "INSERT INTO runtime_execution_nodes(id,run,record) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET run=excluded.run,record=excluded.record",
                rusqlite::params![node_id, run_id, node.to_string()],
            )?;
            Ok(())
        })();
        if write.is_err()
            && let Err(error) = tx.execute_batch("ROLLBACK TO managed_spawn_execution_effect")
        {
            eprintln!("managed child execution history rollback failed: {error}");
        }
        tx.execute_batch("RELEASE managed_spawn_execution_effect")?;
        write
    })();
    if result.is_err() {
        eprintln!("managed child execution history could not be recorded");
    }
}

fn can_delegate(
    tx: &Transaction<'_>,
    actor: &AgentRecord,
    root: &AgentRecord,
) -> Result<bool, crate::managed_host_tools::Error> {
    if actor.role.value().copied()
        == Some(studio_runtime_schema::creation_intent::AgentRole::Reviewer)
    {
        return Ok(false);
    }
    let limit = root
        .worker_defaults
        .value()
        .and_then(|value| match value {
            studio_runtime_store::StoredWorkerDefaults::Known(value) => value
                .delegation_depth
                .value()
                .map(|depth| depth.get() as u64),
            studio_runtime_store::StoredWorkerDefaults::Unreadable(value) => {
                value.get("delegationDepth").and_then(Value::as_u64)
            }
        })
        .filter(|value| (1..=3).contains(value))
        .unwrap_or(1);
    let mut cursor = actor.clone();
    let mut depth = 0_u64;
    let mut visited = std::collections::BTreeSet::new();
    while cursor.id != root.id {
        if !visited.insert(cursor.id.0.clone()) {
            return Ok(false);
        }
        let Some(parent_id) = cursor.parent_id.as_deref() else {
            return Ok(false);
        };
        cursor = AgentRepository::get(tx, &AgentId::from(parent_id))?
            .ok_or(crate::managed_host_tools::Error::Stale)?;
        if cursor.root_id != root.id {
            return Ok(false);
        }
        depth += 1;
        if depth >= limit {
            return Ok(false);
        }
    }
    Ok(true)
}

fn project_child(record: &Value, task_id: Option<&str>, worktree: bool) -> Value {
    let mut output = studio_json::Map::new();
    for field in [
        "id",
        "name",
        "status",
        "model",
        "effort",
        "fastMode",
        "accountKey",
        "provider",
        "cwd",
    ] {
        output.insert(
            field.to_owned(),
            record.get(field).cloned().unwrap_or(Value::Null),
        );
    }
    output.insert("worktree".to_owned(), json!(worktree));
    output.insert("environment".to_owned(), json!("host"));
    output.insert(
        "workspace".to_owned(),
        record
            .get("workspaceMode")
            .cloned()
            .unwrap_or_else(|| json!(if worktree { "worktree" } else { "shared" })),
    );
    if let Some(value) = record
        .get("worktreeWarning")
        .filter(|value| event_write::truthy(Some(value)))
    {
        output.insert("warning".to_owned(), value.clone());
    }
    if let Some(task) = task_id {
        output.insert("taskId".to_owned(), json!(task));
    }
    if worktree {
        output.insert(
            "baseRef".to_owned(),
            record.get("workerBaseRef").cloned().unwrap_or(Value::Null),
        );
        output.insert(
            "baseCommit".to_owned(),
            record
                .get("workerBaseCommit")
                .cloned()
                .unwrap_or(Value::Null),
        );
        if event_write::truthy(record.get("workerBaseBehindMain")) {
            let behind = &record["workerBaseBehindMain"];
            let main_ref = record
                .get("workerBaseMainRef")
                .cloned()
                .unwrap_or_else(|| json!("main"));
            output.insert("baseBehindMain".to_owned(), behind.clone());
            output.insert("baseMainRef".to_owned(), main_ref.clone());
            output.insert(
                "baseWarning".to_owned(),
                json!(format!(
                    "Base commit {} is {} commits behind {}",
                    record["workerBaseCommit"].as_str().unwrap_or(""),
                    behind,
                    main_ref.as_str().unwrap_or("main"),
                )),
            );
        }
    }
    Value::Object(output)
}

/// Base commit, ref, main ref and main lag of a child, as resolved by the host.
type WorkerBase = (String, String, Option<String>, Option<i64>);

/// Resolve the child's base commit. A worktree request in a project without a
/// repository or commits (and no explicit base ref) falls back to the host
/// folder, as lead creation does; the returned flag is the workspace used.
fn resolve_child_base(
    worktree: bool,
    host_cwd: &Path,
    base_ref_request: Option<&str>,
) -> Result<(bool, WorkerBase), Error> {
    if !worktree {
        return Ok((worktree, Default::default()));
    }
    match crate::host_worker_create::resolve_worker_base(host_cwd, base_ref_request) {
        Ok(base) => Ok((worktree, base)),
        Err(crate::agents_create::CreateError::BaseUnresolved)
            if worktree && base_ref_request.is_none() =>
        {
            Ok((false, Default::default()))
        }
        Err(error) => Err(map_create_error(error)),
    }
}

fn map_create_error(error: crate::agents_create::CreateError) -> Error {
    match error {
        crate::agents_create::CreateError::CapabilityUnavailable(name) => Error::Unavailable(name),
        crate::agents_create::CreateError::Store(error) => Error::Store(error),
        crate::agents_create::CreateError::BaseUnresolved => Error::Managed(
            crate::managed_host_tools::Error::Invalid("base commit could not be resolved"),
        ),
        crate::agents_create::CreateError::InvalidCreation(message) => {
            Error::Managed(crate::managed_host_tools::Error::Invalid(message))
        }
        crate::agents_create::CreateError::Conflict
        | crate::agents_create::CreateError::MissingCreationIntent
        | crate::agents_create::CreateError::Deleted
        | crate::agents_create::CreateError::UnknownAgent
        | crate::agents_create::CreateError::LeadRequired
        | crate::agents_create::CreateError::RequestIdRequired
        | crate::agents_create::CreateError::ModelUnavailable
        | crate::agents_create::CreateError::IdentityChanged
        | crate::agents_create::CreateError::Internal
        | crate::agents_create::CreateError::Stale
        | crate::agents_create::CreateError::InFlight
        | crate::agents_create::CreateError::Request(_)
        | crate::agents_create::CreateError::InvalidStoredObject { .. } => Error::Refused,
    }
}

fn map_recheck_error(
    error: crate::agents_create::MutationError,
) -> crate::managed_host_tools::Error {
    match error {
        crate::agents_create::MutationError::Store(error) => {
            crate::managed_host_tools::Error::Store(error)
        }
        error @ (crate::agents_create::MutationError::Conflict
        | crate::agents_create::MutationError::MissingCreationIntent
        | crate::agents_create::MutationError::Deleted
        | crate::agents_create::MutationError::UnknownAgent
        | crate::agents_create::MutationError::LeadRequired
        | crate::agents_create::MutationError::RequestIdRequired
        | crate::agents_create::MutationError::InvalidCreation(_)
        | crate::agents_create::MutationError::BaseUnresolved
        | crate::agents_create::MutationError::ModelUnavailable
        | crate::agents_create::MutationError::CapabilityUnavailable(_)
        | crate::agents_create::MutationError::IdentityChanged
        | crate::agents_create::MutationError::Internal
        | crate::agents_create::MutationError::Stale
        | crate::agents_create::MutationError::InFlight
        | crate::agents_create::MutationError::InvalidStoredObject { .. }
        | crate::agents_create::MutationError::Request(_)) => {
            crate::managed_host_tools::Error::Refusal(error.to_string())
        }
    }
}

struct ReviewDraft {
    typed: AgentRecord,
    record: Value,
    epoch: i64,
    account: crate::host_worker_create::AccountIdentity,
    workspace_admission: Box<dyn crate::monitor_runtime::WorkspaceAdmission + Send + Sync>,
}

/// Reserve a review child and its durable first-turn intent before waking it.
pub(crate) async fn request_review(
    store: &RuntimeStore,
    actor: &crate::managed_host_tools::Actor,
    args: &Value,
    request_id: &str,
    now: f64,
    environment: &dyn ManagedCreationEnvironment,
) -> Result<crate::managed_host_tools::DispatchedOperation, Error> {
    let actor_record = Box::new(actor.record().clone());
    if actor_record.execution_mode == Some(studio_runtime_store::AgentExecutionMode::Vm) {
        return Err(
            crate::managed_host_tools::Error::Unavailable("workspace-state/unavailable").into(),
        );
    }
    if actor_record
        .extra
        .get("nativeReview")
        .is_some_and(|value| event_write::truthy(Some(value)))
    {
        return Err(crate::managed_host_tools::Error::Invalid(
            "A native reviewer cannot create another review",
        )
        .into());
    }
    if actor_record.daybreak_enabled == Some(true) {
        return Err(crate::managed_host_tools::Error::Invalid(
            "Native review cannot select Daybreak. Delegate a review task to a subagent instead",
        )
        .into());
    }
    if !now.is_finite() || now < 0.0 || request_id.is_empty() {
        return Err(crate::managed_host_tools::Error::Invalid("Invalid review identity").into());
    }
    let parent_value = actor_record.value();
    let root = Box::new(stored_root(store, actor_record.root_id.clone()).await?);
    let root_value = root.value();
    let parent_cwd =
        actor_record
            .cwd
            .as_deref()
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Select a project directory",
            ))?;
    let directory_path = PathBuf::from(parent_cwd);
    let directory = CanonicalPath::existing_utf8_directory(&directory_path)
        .map_err(|_| {
            crate::managed_host_tools::Error::Invalid("Select an existing project directory")
        })?
        .to_string_lossy()
        .into_owned();
    crate::native_actions::bounded(&parent_value, "review.actor")
        .and_then(|()| crate::native_actions::validate_agent(&parent_value))
        .and_then(|()| crate::native_actions::validate_agent(&root_value))
        .map_err(|_| crate::managed_host_tools::Error::Invalid("Invalid native review request"))?;
    let mut plan = crate::native_review::ReviewPlan::from_checked_records(
        &actor_record,
        &root,
        args.clone(),
        request_id.to_owned(),
        directory.clone(),
    )
    .map_err(|_| crate::managed_host_tools::Error::Invalid("Invalid native review request"))?;
    let child_id =
        plan.spec()
            .id
            .value()
            .cloned()
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Invalid native review request",
            ))?;
    let saved_id = child_id.clone();
    if let Some(saved) = store
        .read_async("runtime-services/managed_child_create", move |read| {
            studio_runtime_store::ContextRepairRepository::agent(read, &saved_id)
        })
        .await?
    {
        let value = plan.existing(&saved).map_err(|_| {
            crate::managed_host_tools::Error::Invalid("This request id has different content")
        })?;
        return Ok(crate::managed_host_tools::DispatchedOperation::Applied(
            crate::managed_host_tools::OperationResult::reply(&value),
        ));
    }
    use studio_runtime_schema::review::ReviewTarget;
    let (assignment, resolved_base) =
        prepare_reviewer(store, &directory, plan.assignment()).await?;
    plan.set_prepared_assignment(assignment, resolved_base);
    let parent_account_key = parent_value
        .get("accountKey")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let spec = plan.spec();
    let model =
        spec.model
            .value()
            .map(String::as_str)
            .ok_or(crate::managed_host_tools::Error::Invalid(
                "Invalid native review model",
            ))?;
    let (account_key, account) = select_review_account(
        store,
        environment,
        &actor_record,
        &root,
        plan.account(),
        model,
        &std::collections::HashMap::new(),
    )
    .await?;
    // Workspace preparation runs after the reservation has committed.
    let workspace_admission = environment.prepare_workspace(store, &actor_record, request_id)?;
    let effort = spec.effort.selected().map(String::as_str);
    let fast_mode = spec.fast_mode.value().copied().unwrap_or(false);
    let (base_ref, base_commit) = match plan.assignment().target() {
        ReviewTarget::GitRange { base, head } => (base.to_string(), head.to_string()),
        ReviewTarget::LayrState { .. } => (String::new(), String::new()),
    };
    let project = stored_project(
        store,
        parent_value
            .get("projectId")
            .and_then(Value::as_str)
            .unwrap_or(parent_cwd),
        parent_cwd,
    )
    .await?;
    let body = json!({"account_key":account_key,"name":"Review","model":model,
            "effort":effort,"prompt":spec.prompt.as_deref().unwrap_or(""),
            "_workspaceInGit":parent_value.get("workspaceInGit").and_then(Value::as_bool).unwrap_or(false),
    });
    let mut prepared = crate::host_worker_create::prepare_work_item(
        &body,
        Path::new(&directory),
        AgentId::from(child_id.as_str()),
        actor_record.root_id.clone(),
        actor_record.id.clone(),
        &base_ref,
        &base_commit,
        actor_record.epoch,
        parent_cwd,
        parent_cwd,
        parent_account_key,
        parent_value.get("projectId"),
        parent_value.get("projectServerId"),
        project.as_ref(),
        &account,
        environment.catalog(),
        "reviewer",
        false,
        fast_mode,
        false,
    )
    .map_err(map_create_error)?;
    prepared.extra.insert(
        "_p02ProjectLookupId".into(),
        json!(
            parent_value
                .get("projectId")
                .and_then(Value::as_str)
                .unwrap_or(parent_cwd)
        ),
    );
    let mut record = crate::host_worker_create::persisted_record(&prepared)
        .map_err(map_create_error)?
        .value();
    record["permissionMode"] =
        json!(studio_runtime_schema::claude_permissions::PermissionMode::Default);
    record["workspaceMode"] = json!(actor_record.workspace_mode);
    record["worktree"] = json!(false);
    crate::agents_create::set_capacity(
        &mut record,
        crate::agents_create::capacity_max(&root_value),
        false,
    )?;
    let review = ReviewDraft {
        typed: prepared,
        record,
        epoch: actor_record.epoch,
        account,
        workspace_admission,
    };
    let child_id = child_id.to_owned();
    let value = store
        .transaction_with_error(move |tx| -> Result<Value, crate::managed_host_tools::Error> {
            recheck_actor(tx, &actor_record)?;
            if AgentRepository::get(tx, &AgentId::from(child_id.as_str()))?.is_none() {
                let maximum = crate::agents_create::capacity_max(&root_value);
                if active_agents(tx, &actor_record.root_id.0)? >= maximum {
                    return Err(crate::managed_host_tools::Error::Invalid(
                        "Team active agent limit reached; free a finished agent before creating a review",
                    ));
                }
            }
            let creator = ReviewCreator {
                actor: &actor_record,
                draft: &review,
            };
            let response =
                crate::native_review::reserve(tx, &plan, &creator).map_err(
                    |error| match error {
                        crate::native_actions::NativeError::Expired => crate::managed_host_tools::Error::Expired,
                        crate::native_actions::NativeError::Store(error) => {
                            crate::managed_host_tools::Error::Store(error)
                        }
                        crate::native_actions::NativeError::Unavailable(name) => {
                            crate::managed_host_tools::Error::Unavailable(name)
                        }
                        crate::native_actions::NativeError::Invalid(_) | crate::native_actions::NativeError::InvalidStored { .. } | crate::native_actions::NativeError::Conflict | crate::native_actions::NativeError::Changed | crate::native_actions::NativeError::Unknown | crate::native_actions::NativeError::Json(_) | crate::native_actions::NativeError::Sql(_) => crate::managed_host_tools::Error::Invalid(
                            "Native review creation was refused",
                        ),
                    },
                )?;
            let child = AgentRepository::get(tx, &AgentId::from(child_id.as_str()))?
                .ok_or(crate::managed_host_tools::Error::Stale)?;
            crate::agents_create::publish_agent(tx, &child.value())?;
            Ok(response)
        }).await
        .map_err(Error::from)?;
    Ok(crate::managed_host_tools::DispatchedOperation::Applied(
        crate::managed_host_tools::OperationResult {
            signals: vec![crate::managed_host_tools::WakeSignal::new("worker-driver")],
            ..crate::managed_host_tools::OperationResult::reply(&value)
        },
    ))
}

struct ReviewCreator<'a> {
    actor: &'a AgentRecord,
    draft: &'a ReviewDraft,
}
impl crate::native_review::ReviewCreator for ReviewCreator<'_> {
    fn child(
        &self,
        tx: &Transaction<'_>,
        actor: &Value,
        spec: &studio_runtime_schema::creation_intent::CreationIntent,
    ) -> crate::native_actions::Result<Value> {
        if actor.get("id") != Some(&Value::String(self.actor.id.0.clone()))
            || spec.id.value() != Some(&self.draft.typed.id.0)
        {
            return Err(crate::native_actions::NativeError::Changed);
        }
        self.draft
            .workspace_admission
            .check(tx, self.actor)
            .map_err(|_| crate::native_actions::NativeError::Changed)?;
        crate::host_worker_create::recheck_worker(
            tx,
            &self.draft.typed,
            self.draft.epoch,
            &self.draft.account,
        )
        .map_err(|_| crate::native_actions::NativeError::Changed)?;
        Ok(self.draft.record.clone())
    }
    fn receipt(
        &self,
        _: &Transaction<'_>,
        _: &str,
        _: &Value,
    ) -> crate::native_actions::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod review_identity_tests {
    use super::{WorkerBase, resolve_child_base};

    #[test]
    fn worker_recheck_refusal_reports_its_cause() {
        use crate::agents_create::MutationError;
        assert_eq!(
            super::map_recheck_error(MutationError::ModelUnavailable).to_string(),
            "The selected model is not available for the selected account"
        );
        assert_eq!(
            super::map_recheck_error(MutationError::InvalidCreation("cwd")).to_string(),
            "Invalid host agent creation field: cwd"
        );
        assert_eq!(
            super::map_recheck_error(MutationError::IdentityChanged).to_string(),
            "The account or project changed; reload before creating the agent"
        );
    }

    #[test]
    fn review_home_expansion_uses_the_supplied_home() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().to_str().unwrap();
        assert_eq!(super::expand_review_home("~", Some(home)), root.path());
        assert_eq!(
            super::expand_review_home("~/review-target", Some(home)),
            root.path().join("review-target")
        );
        assert_eq!(
            super::expand_review_home("~/review-target", None),
            std::path::Path::new("~/review-target")
        );
    }

    #[test]
    fn worktree_without_repository_falls_back_to_host() {
        let directory = tempfile::tempdir().unwrap();
        let (worktree, base) = resolve_child_base(true, directory.path(), None).unwrap();
        assert!(!worktree);
        assert_eq!(base, WorkerBase::default());
        assert!(resolve_child_base(true, directory.path(), Some("HEAD")).is_err());
    }
}
