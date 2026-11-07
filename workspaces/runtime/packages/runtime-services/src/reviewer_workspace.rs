//! Prepare a reviewer's isolated workspace using the ordinary workspace owners.
use crate::workspace_ready::{ReadyToken, WorkspaceDecision};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use studio_runtime_schema::review::{ReviewAssignment, ReviewTarget, WorkspaceMode};
use studio_runtime_store::{AgentId, AgentRepository, RuntimeStore, StoreError};

// These stored names predate Git reviewers.
pub fn read_only_copy(record: &studio_runtime_store::AgentRecord) -> Option<&str> {
    record.layr_read_cwd.as_deref()
}

pub fn scratch_dir(record: &studio_runtime_store::AgentRecord) -> Option<&str> {
    record
        .extra
        .get("layrScratch")
        .and_then(studio_json::Value::as_str)
}

pub(crate) fn ensure_scratch(
    state_dir: &Path,
    id: &str,
) -> Result<PathBuf, crate::layr_native::NativeLayrError> {
    use studio_runtime_store::path_identity::CanonicalPath;
    let suffix = PathBuf::from(crate::layr_native::NATIVE_LAYR_DIRECTORY)
        .join("scratch")
        .join(studio_runtime_store::sha256_hex(id.as_bytes()));
    let scratch = state_dir.join(&suffix);
    std::fs::create_dir_all(&scratch)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o700))?;
    }
    let canonical = CanonicalPath::existing(&scratch).map(CanonicalPath::into_path_buf)?;
    let expected = CanonicalPath::existing(state_dir)
        .map(CanonicalPath::into_path_buf)?
        .join(suffix);
    if canonical != expected {
        return Err(crate::layr_native::NativeLayrError::Refused(
            "Reviewer scratch folder has a symlink ancestor".into(),
        ));
    }
    Ok(canonical)
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewerWorkspaceError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Workspace(#[from] crate::workspace_ready_git::WorkspaceError),
    #[error("The review assignment changed during workspace preparation")]
    Changed,
    #[error("The review workspace is not ready")]
    Deferred,
    #[error("The reviewer is not active")]
    ActorDeferred,
    #[error("The review workspace was refused: {0}")]
    Refused(String),
}

pub struct ReviewerWorkspace {
    pub cwd: PathBuf,
    pub(crate) token: ReadyToken,
}

async fn ready_token(
    store: &Arc<RuntimeStore>,
    agent_id: &str,
) -> Result<ReadyToken, ReviewerWorkspaceError> {
    match crate::workspace_ready_git::prepare(Arc::clone(store), agent_id.to_owned()).await? {
        WorkspaceDecision::Ready { token } => Ok(token),
        WorkspaceDecision::Refuse(reason) => {
            Err(ReviewerWorkspaceError::Refused(reason.to_string()))
        }
        WorkspaceDecision::Defer {
            by: crate::workspace_ready::DeferredBy::Actor,
        } => Err(ReviewerWorkspaceError::ActorDeferred),
        WorkspaceDecision::Provision(_) | WorkspaceDecision::Defer { .. } => {
            Err(ReviewerWorkspaceError::Deferred)
        }
    }
}

pub async fn prepare(
    store: &Arc<RuntimeStore>,
    agent_id: &str,
    assignment: &ReviewAssignment,
) -> Result<ReviewerWorkspace, ReviewerWorkspaceError> {
    let scratch = if matches!(assignment.target(), ReviewTarget::GitRange { .. }) {
        let state_dir = store.root().to_owned();
        let id = agent_id.to_owned();
        Some(
            studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-services/reviewer-scratch",
                move || ensure_scratch(&state_dir, &id),
            )
            .await
            .map_err(|_| ReviewerWorkspaceError::Changed)?
            .map_err(|error| ReviewerWorkspaceError::Refused(error.to_string()))?,
        )
    } else {
        None
    };
    let id = agent_id.to_owned();
    let expected = assignment.clone();
    store
        .transaction_with_error(move |tx| {
            let mut agent = AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                .ok_or(ReviewerWorkspaceError::Changed)?;
            if agent.review_assignment.as_ref() != Some(&expected) {
                return Err(ReviewerWorkspaceError::Changed);
            }
            match expected.target() {
                ReviewTarget::LayrState { state, .. } => {
                    if agent.workspace_mode != Some(WorkspaceMode::LayrNative)
                        || (agent.layr_ready == Some(true)
                            && agent.layr_base_state.as_deref() != Some(state.as_str()))
                    {
                        return Err(ReviewerWorkspaceError::Changed);
                    }
                    agent.layr_base_state = Some(state.as_str().to_owned());
                }
                ReviewTarget::GitRange { head, .. } => {
                    if agent.workspace_mode != Some(WorkspaceMode::Worktree)
                        || (agent.worktree_ready == Some(true)
                            && agent.worker_base_commit.as_deref() != Some(head.as_str()))
                    {
                        return Err(ReviewerWorkspaceError::Changed);
                    }
                    agent.worktree = Some(true);
                    agent.worker_base_commit = Some(head.as_str().to_owned());
                    let scratch = scratch.as_ref().ok_or(ReviewerWorkspaceError::Changed)?;
                    agent
                        .extra
                        .insert("layrScratch".into(), studio_json::json!(scratch));
                }
            }
            AgentRepository::put(tx, &agent)?;
            Ok::<_, ReviewerWorkspaceError>(())
        })
        .await?;
    let mut token = ready_token(store, agent_id).await?;
    if matches!(assignment.target(), ReviewTarget::GitRange { .. }) {
        let validation = token.clone();
        let id = agent_id.to_owned();
        let expected = assignment.clone();
        let changed = store
            .transaction_with_error(move |tx| {
                let cwd = validation
                    .validate(tx)?
                    .map_err(|_| ReviewerWorkspaceError::Changed)?;
                let mut agent = AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                    .ok_or(ReviewerWorkspaceError::Changed)?;
                if agent.review_assignment.as_ref() != Some(&expected) {
                    return Err(ReviewerWorkspaceError::Changed);
                }
                let copy = crate::worktree_root::owned_root(&cwd, &id)
                    .ok_or(ReviewerWorkspaceError::Changed)?
                    .root()
                    .to_str()
                    .ok_or(ReviewerWorkspaceError::Changed)?;
                if read_only_copy(&agent) == Some(copy) {
                    return Ok(false);
                }
                agent.layr_read_cwd = Some(copy.into());
                AgentRepository::put(tx, &agent)?;
                Ok::<_, ReviewerWorkspaceError>(true)
            })
            .await?;
        if changed {
            token = ready_token(store, agent_id).await?;
        }
    }
    let validation = token.clone();
    let id = agent_id.to_owned();
    let cwd = store
        .read_async("runtime-services/reviewer-workspace", move |tx| {
            validation
                .validate(tx)?
                .map_err(|_| ReviewerWorkspaceError::Changed)?;
            let agent = AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                .ok_or(ReviewerWorkspaceError::Changed)?;
            read_only_copy(&agent)
                .map(PathBuf::from)
                .ok_or(ReviewerWorkspaceError::Changed)
        })
        .await?;
    Ok(ReviewerWorkspace { cwd, token })
}
