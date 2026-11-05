//! A Claude chat moves to another Claude account of this server by copying its
//! session files and switching the account in one transaction. Both accounts
//! run on this host, so the target process opens the same session on its next
//! turn. Chats that are busy, or that change provider, use the transfer flow.

use crate::account_transfer::TransferError;
use std::path::{Path, PathBuf};
use studio_json::{Value, json};
use studio_runtime_store::{AgentId, AgentRepository, RuntimeStore, StoreError};

/// The files of one Claude session under an account home and the server state.
struct SessionFiles {
    /// `account-servers/<account>/sessions/<thread>.*` of the bridge.
    bridge: Vec<PathBuf>,
    /// Native Claude paths relative to the account home.
    native: Vec<PathBuf>,
}

fn session_files(
    state_root: &Path,
    source: &str,
    home: &Path,
    thread: &str,
) -> Result<SessionFiles, TransferError> {
    let sessions = state_root
        .join("account-servers")
        .join(source)
        .join("sessions");
    let bridge: Vec<PathBuf> = ["json", "jsonl", "meta.json"]
        .iter()
        .map(|extension| sessions.join(format!("{thread}.{extension}")))
        .filter(|path| path.exists())
        .collect();
    let snapshot = sessions.join(format!("{thread}.json"));
    let native_id = std::fs::read(&snapshot)
        .ok()
        .and_then(|bytes| studio_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value["session"]["nativeId"].as_str().map(str::to_owned))
        .unwrap_or_else(|| thread.to_owned());
    let mut native = Vec::new();
    if let Ok(projects) = std::fs::read_dir(home.join("projects")) {
        for project in projects.flatten() {
            for name in [format!("{native_id}.jsonl"), native_id.clone()] {
                let path = project.path().join(&name);
                if path.exists() {
                    native.push(Path::new("projects").join(project.file_name()).join(name));
                }
            }
        }
    }
    let history = Path::new("file-history").join(&native_id);
    if home.join(&history).exists() {
        native.push(history);
    }
    if bridge.is_empty()
        || !native
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "jsonl"))
    {
        return Err(TransferError::Refused(
            "The Claude session files of this chat were not found",
        ));
    }
    Ok(SessionFiles { bridge, native })
}

fn copy_path(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_path(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Copy the session of `thread` from the source account to the target account.
pub fn copy_claude_session(
    state_root: &Path,
    source: &str,
    source_home: &Path,
    target: &str,
    target_home: &Path,
    thread: &str,
) -> Result<(), TransferError> {
    let files = session_files(state_root, source, source_home, thread)?;
    let failed = |_| TransferError::Refused("The Claude session files could not be copied");
    let target_sessions = state_root
        .join("account-servers")
        .join(target)
        .join("sessions");
    for path in &files.bridge {
        let name = path
            .file_name()
            .ok_or(TransferError::Refused("Invalid session file"))?;
        copy_path(path, &target_sessions.join(name)).map_err(failed)?;
    }
    for relative in &files.native {
        copy_path(&source_home.join(relative), &target_home.join(relative)).map_err(failed)?;
    }
    Ok(())
}

/// Move an idle Claude chat to another Claude account. Returns `None` when the
/// chat is not an idle Claude chat, so the caller uses the transfer flow.
#[allow(clippy::too_many_arguments)]
pub async fn move_claude_chat(
    store: &RuntimeStore,
    lead: &str,
    home: &(dyn Fn(&str) -> Option<String> + Send + Sync),
    target: &str,
    request_id: &str,
    scope: &str,
    now: f64,
) -> Result<Option<Value>, TransferError> {
    let id = lead.to_owned();
    let agent = store
        .read_async("runtime-services/simple_move", move |tx| {
            AgentRepository::get(tx, &AgentId::from(id.as_str()))
                .map(|agent| agent.map(|a| a.value()))
        })
        .await?;
    let Some(agent) = agent else {
        return Ok(None);
    };
    let Some(state) = Movable::read(&agent, target) else {
        return Ok(None);
    };
    let (Some(source_home), Some(target_home)) = (home(&state.source), home(target)) else {
        return Ok(None);
    };
    let root = store.root().to_path_buf();
    let (source_home, target_home) = (PathBuf::from(source_home), PathBuf::from(target_home));
    let (source, thread, target_key) = (
        state.source.clone(),
        state.thread.clone(),
        target.to_owned(),
    );
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/simple_move",
        move || {
            copy_claude_session(
                &root,
                &source,
                &source_home,
                &target_key,
                &target_home,
                &thread,
            )
        },
    )
    .await
    .map_err(|_| TransferError::Refused("The Claude session copy stopped"))??;
    let id = lead.to_owned();
    let target_key = target.to_owned();
    store
        .transaction_with_error(move |tx| {
            let current = AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                .ok_or(TransferError::Refused("Unknown managed agent"))?
                .value();
            // The chat must still be idle on the same source and thread.
            if Movable::read(&current, &target_key) != Some(state) {
                return Err(TransferError::Refused(
                    "The chat changed during the account move",
                ));
            }
            let mut value = current;
            value["accountKey"] = json!(target_key);
            value["executionSettingsAccountKey"] = json!(target_key);
            if let Some(object) = value.as_object_mut() {
                for key in [
                    "accountTransfer",
                    "accountTransferId",
                    "lazyAccountTransfer",
                ] {
                    object.remove(key);
                }
            }
            AgentRepository::put(
                tx,
                &serde::Deserialize::deserialize(value.clone()).map_err(StoreError::from)?,
            )?;
            crate::agents_create::publish_agent(tx, &value)?;
            Ok::<_, TransferError>(())
        })
        .await?;
    Ok(Some(json!({
        "id": request_id,
        "leadId": lead,
        "targetAccountKey": target,
        "targetProvider": "claude",
        "status": "completed",
        "scope": scope,
        "created": now,
        "updated": now,
        "members": {},
        "total": 1,
        "completed": 1,
        "moved": 1,
    })))
}

/// The facts a simple move depends on; equal before the copy and at commit.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Movable {
    source: String,
    thread: String,
    epoch: String,
}

impl Movable {
    fn read(agent: &Value, target: &str) -> Option<Self> {
        let source = agent["accountKey"].as_str()?;
        let busy = agent["inFlight"].as_bool() == Some(true)
            || matches!(agent["status"].as_str(), Some("running" | "starting"));
        (agent["provider"] == "claude"
            && source != target
            && source.starts_with("claude-")
            && target.starts_with("claude-")
            && !busy
            && agent.get("environment").is_none_or(|value| value == "host"))
        .then(|| Self {
            source: source.to_owned(),
            thread: agent["threadId"].as_str().unwrap_or_default().to_owned(),
            epoch: agent["epoch"].to_string(),
        })
        .filter(|state| !state.thread.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_files_move_to_the_target_account() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        let (source_home, target_home) = (root.path().join("source"), root.path().join("target"));
        let sessions = state.join("account-servers/claude-a/sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(
            sessions.join("thread-1.json"),
            r#"{"session":{"nativeId":"native-1"}}"#,
        )
        .unwrap();
        std::fs::write(sessions.join("thread-1.jsonl"), "{}\n").unwrap();
        let project = source_home.join("projects/-work");
        std::fs::create_dir_all(project.join("native-1")).unwrap();
        std::fs::write(project.join("native-1.jsonl"), "history\n").unwrap();
        std::fs::write(project.join("native-1/sub.jsonl"), "sub\n").unwrap();

        copy_claude_session(
            &state,
            "claude-a",
            &source_home,
            "claude-b",
            &target_home,
            "thread-1",
        )
        .unwrap();

        let target_sessions = state.join("account-servers/claude-b/sessions");
        assert!(target_sessions.join("thread-1.json").exists());
        assert!(target_sessions.join("thread-1.jsonl").exists());
        assert_eq!(
            std::fs::read_to_string(target_home.join("projects/-work/native-1.jsonl")).unwrap(),
            "history\n"
        );
        assert!(
            target_home
                .join("projects/-work/native-1/sub.jsonl")
                .exists()
        );
    }

    #[test]
    fn a_missing_native_history_refuses_the_simple_move() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("account-servers/claude-a/sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join("t.json"), "{}").unwrap();
        assert!(
            copy_claude_session(
                root.path(),
                "claude-a",
                &root.path().join("h"),
                "claude-b",
                &root.path().join("g"),
                "t"
            )
            .is_err()
        );
    }

    #[test]
    fn only_an_idle_claude_chat_moves_simply() {
        let idle = json!({"provider":"claude","accountKey":"claude-a","threadId":"t","status":"waiting","epoch":3});
        assert!(Movable::read(&idle, "claude-b").is_some());
        let mut busy = idle.clone();
        busy["inFlight"] = json!(true);
        assert!(Movable::read(&busy, "claude-b").is_none());
        assert!(Movable::read(&idle, "login-codex").is_none());
        assert!(Movable::read(&idle, "claude-a").is_none());
    }
}
