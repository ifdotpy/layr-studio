//! Switch an idle WT chat to native Layr in place. The chat keeps its record,
//! transcript and provider thread; only its workspace fields change.
//! The provider adapter carries the native session to the new cwd on the next resume.
use super::{
    runtime::{self, NativeLayrError},
    *,
};
use std::sync::Arc;
use studio_runtime_store::workspace_ready::workspace_generation;
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore};

type Result<T> = std::result::Result<T, NativeLayrError>;
type NativeRelatives = Option<(Box<AgentRecord>, Box<AgentRecord>)>;
/// Filesystem effects of a switch: old record and native target record in, prepared fields out.
pub(super) type Effects = fn(&Path, &Value, &Value) -> Result<Value>;

/// Switch one chat. A repeat after success returns the same association.
pub async fn switch_chat(store: Arc<RuntimeStore>, agent_id: &str) -> Result<Value> {
    switch_chat_with(store, agent_id, prepare_switch).await
}

pub(super) async fn switch_chat_with(
    store: Arc<RuntimeStore>,
    agent_id: &str,
    effects: Effects,
) -> Result<Value> {
    let id = agent_id.to_owned();
    let (record, relatives) = store
        .read_async("runtime-services/layr-native/switch", move |tx| {
            admit(tx, &id)
        })
        .await?;
    if is_native(&record.value()) {
        return summary(&record.value());
    }
    let mut target = record.clone();
    configure(&mut target, relatives.as_ref());
    let state_dir = store.root().to_owned();
    let source = record.value();
    let target = target.value();
    let prepared = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/layr-native/switch",
        move || effects(&state_dir, &source, &target),
    )
    .await
    .map_err(|_| NativeLayrError::Worker)??;
    let id = agent_id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let (mut current, relatives) = admit(tx, &id)?;
            if is_native(&current.value()) {
                return summary(&current.value());
            }
            if workspace_generation(&current.value()) != workspace_generation(&record.value()) {
                return Err(NativeLayrError::Refused(
                    "The chat changed during the switch. Try again.".into(),
                ));
            }
            configure(&mut current, relatives.as_ref());
            for (key, value) in prepared
                .as_object()
                .ok_or_else(|| NativeLayrError::Refused("Invalid native workspace".into()))?
            {
                match key.as_str() {
                    "cwd" => current.cwd = value.as_str().map(str::to_owned),
                    "layrProjectId" => current.layr_project_id = value.as_str().map(Into::into),
                    "layrLine" => current.layr_line = <studio_runtime_schema::queue_command::Field<
                        String,
                    > as serde::Deserialize>::deserialize(
                        value.clone()
                    )
                    .map_err(|error| NativeLayrError::Refused(error.to_string()))?,
                    "layrReadCwd" => current.layr_read_cwd = value.as_str().map(Into::into),
                    "layrStoreRoot" => current.layr_store_root = value.as_str().map(str::to_owned),
                    "layrReady" => current.layr_ready = value.as_bool(),
                    "layrStateId" => {
                        current.layr_state_id = serde::Deserialize::deserialize(value.clone())
                            .map_err(|error| NativeLayrError::Refused(error.to_string()))?
                    }
                    _ => {
                        current.extra.insert(key.clone(), value.clone());
                    }
                }
            }
            AgentRepository::put(tx, &current)?;
            crate::agents_create::publish_agent(tx, &current.value())?;
            summary(&current.value())
        })
        .await
}

/// Read the chat and its native parent and root. A switched chat needs no other check.
fn admit(
    tx: &studio_runtime_store::Transaction<'_>,
    id: &str,
) -> Result<(Box<AgentRecord>, NativeRelatives)> {
    let get = |id: &str| -> Result<AgentRecord> {
        AgentRepository::get(tx, &AgentId::from(id))?
            .ok_or_else(|| NativeLayrError::Refused(format!("Unknown chat {id}")))
    };
    let agent = get(id)?;
    let record = agent.value();
    if is_native(&record) {
        return Ok((Box::new(agent), None));
    }
    if crate::event_write::truthy(record.get("deletedAt")) {
        return Err(NativeLayrError::Refused("The chat was deleted".into()));
    }
    let lead = record["isLead"] == true;
    if record["workspaceMode"] != "worktree"
        || agent.execution_mode == Some(studio_runtime_store::AgentExecutionMode::Vm)
        || !(lead || record["role"] == "implementer")
    {
        return Err(NativeLayrError::Refused(
            "Only a lead or implementer chat in WT mode can switch to Layr".into(),
        ));
    }
    crate::idle_gate::idle(tx, std::slice::from_ref(&record), false)
        .map_err(|error| NativeLayrError::Refused(error.to_string()))?;
    if lead {
        return Ok((Box::new(agent), None));
    }
    let parent = get(record["parentId"].as_str().unwrap_or_default())?;
    if !is_native(&parent.value()) || parent.layr_ready != Some(true) {
        return Err(NativeLayrError::Refused(
            "Switch the parent chat to Layr first".into(),
        ));
    }
    let root = get(&agent.root_id.0)?;
    Ok((Box::new(agent), Some((Box::new(parent), Box::new(root)))))
}

/// Native intent: a lead keeps its project folder as the source; a worker inherits its parent.
fn configure(record: &mut AgentRecord, relatives: Option<&(Box<AgentRecord>, Box<AgentRecord>)>) {
    match relatives {
        Some((parent, root)) => configure_child(record, parent, root),
        None => {
            let source = record.cwd.as_deref().unwrap_or_default().to_owned();
            configure_new_chat(
                record,
                &crate::project_path::ProjectPath::from_stored(&source),
            );
        }
    }
}

fn summary(record: &Value) -> Result<Value> {
    if record["layrReady"] != true {
        return Err(NativeLayrError::Refused(
            "The native Layr workspace is not ready".into(),
        ));
    }
    Ok(json!({
        "id": record["id"],
        "workspaceMode": record["workspaceMode"],
        "cwd": record["cwd"],
        "layrProjectId": record["layrProjectId"],
        "layrLine": record["layrLine"],
        "layrStateId": record["layrStateId"],
    }))
}

/// A worker line receives the committed and pushed worktree content under the same
/// receipt that preparation uses, so preparation then only verifies the line.
fn prepare_switch(state_dir: &Path, source: &Value, target: &Value) -> Result<Value> {
    prepare_switch_reporting(state_dir, source, target, None)
}

fn prepare_switch_reporting(
    state_dir: &Path,
    source: &Value,
    target: &Value,
    progress: Option<&runtime::ProgressSink>,
) -> Result<Value> {
    if target["isLead"] != true {
        import_worker(state_dir, source, target, progress)?;
    }
    runtime::prepare_files_reporting(state_dir, target, false, progress)
}

fn import_worker(
    state_dir: &Path,
    source: &Value,
    target: &Value,
    progress: Option<&runtime::ProgressSink>,
) -> Result<()> {
    let id = source["id"].as_str().unwrap_or_default();
    let cwd = Path::new(source["cwd"].as_str().unwrap_or_default());
    let tree = PathBuf::from(migration::git(cwd, &["rev-parse", "--show-toplevel"])?.trim());
    saved_and_pushed(&tree)?;
    let common = PathBuf::from(
        migration::git(
            &tree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?
        .trim(),
    );
    let repository = CanonicalPath::existing(common.parent().unwrap_or(&common))
        .map(CanonicalPath::into_path_buf)?;
    let project = CanonicalPath::existing(target["hostProjectPath"].as_str().unwrap_or_default())
        .map(CanonicalPath::into_path_buf)?;
    let relative = repository
        .strip_prefix(&project)
        .map_err(|_| {
            NativeLayrError::Refused("The worktree repository is outside the project".into())
        })?
        .to_owned();
    let root = settings::record_store(state_dir, target)?;
    let project_id = runtime::project_id(target)?;
    let main = runtime::main_path_in_store(&root, &project_id);
    let metadata = settings::metadata(state_dir, &root);
    let plan = import::Plan::worktree(&tree, &project, &relative, &metadata)?;
    let line = format!("agent-{}", studio_runtime_store::sha256_hex(id.as_bytes()));
    runtime::operation_with_preflight_in_store(
        state_dir,
        &root,
        &format!("line:{id}"),
        &json!({"project":project_id,"line":line}),
        || {
            plan.admit()?;
            let saved = import::Plan::saved_main(state_dir, &main, &metadata)?;
            plan.migration_size(&saved, &relative).preflight(&main)
        },
        || {
            let _lock = runtime::lock(state_dir, &format!("migration:{project_id}"))?;
            migration::import_worktree_line(state_dir, &main, &plan, &relative, &line, progress)?;
            let path = main
                .parent()
                .ok_or_else(|| NativeLayrError::Refused("Invalid main path".into()))?
                .join(&line);
            Ok(json!({"path":path,"line":line}))
        },
    )?;
    Ok(())
}

/// Uncommitted files and a HEAD missing from every remote-tracking ref refuse the switch.
/// The check reads the existing refs and does not fetch.
fn saved_and_pushed(tree: &Path) -> Result<()> {
    let status = migration::git(
        tree,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err(NativeLayrError::Refused(
            "The worktree has uncommitted files. Commit and push them, then switch again".into(),
        ));
    }
    let remote = migration::git(
        tree,
        &[
            "for-each-ref",
            "--count=1",
            "--contains=HEAD",
            "--format=%(refname)",
            "refs/remotes",
        ],
    )?;
    if remote.trim().is_empty() {
        return Err(NativeLayrError::Refused(
            "The worktree HEAD is not on a remote branch. Push it, then switch again".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
