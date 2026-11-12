//! A Claude chat moves to another Claude account of this server by copying its
//! session files and switching the account in one transaction. Both accounts
//! run on this host, so the target process opens the same session on its next
//! turn. A chat that changes provider starts a new session that reads a
//! transcript of its Studio items. Busy chats use the transfer flow.

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
            let mut value = release_source_holds(tx, &id, current, now)?;
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

/// Holds that belong to the source account end with the move: its automatic
/// usage resume and its failure hold. Otherwise the chat waits for the old
/// account's limit after it moved to an account that has allowance.
fn release_source_holds(
    tx: &studio_runtime_store::Transaction<'_>,
    id: &str,
    mut value: Value,
    now: f64,
) -> Result<Value, TransferError> {
    if let Some(resume) = value.pointer("/usageResume/id").and_then(Value::as_str)
        && matches!(
            value.pointer("/usageResume/status").and_then(Value::as_str),
            Some("scheduled" | "started")
        )
    {
        let resume = resume.to_owned();
        value["usageResume"] = studio_runtime_store::cancel_usage_resume(
            tx,
            id,
            &resume,
            "The chat moved to another account.",
            now,
        )
        .map_err(|_| TransferError::Refused("Could not cancel automatic resume"))?;
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("nativeFailureHold");
    }
    value["error"] = Value::Null;
    Ok(value)
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

fn legacy_vm_workspace(record: &Value) -> bool {
    crate::workspace_ready::vm_workspace_unsupported(
        record["executionMode"].as_str(),
        record["workspaceMode"].as_str(),
        record["layrReady"] == true,
        record["layrLine"].as_str().is_some(),
    )
}

/// The readable transcript of a chat from its Studio items, oldest first.
fn transcript(items: &[Value]) -> String {
    let mut text = String::new();
    for item in items {
        let role = match item["role"].as_str() {
            Some("user") => "User",
            Some("assistant") => "Assistant",
            _ => continue,
        };
        let body = item["text"].as_str().unwrap_or_default();
        if !body.is_empty() {
            text.push_str(&format!("{role}: {body}\n\n"));
        }
    }
    text
}

/// The note that tells the next session where the earlier conversation is.
fn history_note(path: &Path, provider: &str) -> String {
    format!(
        "\n\nThis chat moved from another provider ({provider}). The earlier conversation is in {}. Read it before you continue; do not repeat finished work.",
        path.display()
    )
}

/// Move an idle chat to an account of another provider. The new provider has
/// no copy of the native history, so the chat starts a new session that reads
/// a transcript of its Studio items. Returns `None` when the chat is busy or
/// has no role guidance to carry the note, so the caller uses the transfer flow.
#[allow(clippy::too_many_arguments)]
pub async fn move_across_providers(
    store: &RuntimeStore,
    lead: &str,
    target: &str,
    target_provider: studio_runtime_providers::ProviderKind,
    request_id: &str,
    scope: &str,
    now: f64,
) -> Result<Option<Value>, TransferError> {
    let id = lead.to_owned();
    let read = store
        .read_async("runtime-services/simple_move_cross", move |tx| {
            let Some(agent) = AgentRepository::get(tx, &AgentId::from(id.as_str()))? else {
                return Ok::<_, StoreError>(None);
            };
            let mut rows = tx.prepare(
                "SELECT record FROM runtime_items WHERE agent=?1 ORDER BY created, rowid",
            )?;
            let items = rows
                .query_map([id.as_str()], |row| row.get::<_, String>(0))?
                .filter_map(|row| row.ok())
                .filter_map(|raw| studio_json::from_str::<Value>(&raw).ok())
                .collect::<Vec<_>>();
            let Some(provider) = agent.provider.value().copied() else {
                return Ok(None);
            };
            Ok(Some((provider, agent.value(), items)))
        })
        .await?;
    let Some((source_provider, agent, items)) = read else {
        return Ok(None);
    };
    if legacy_vm_workspace(&agent) {
        return Err(TransferError::Refused("VmWorkspaceUnsupported"));
    }
    let busy = agent["inFlight"].as_bool() == Some(true)
        || matches!(agent["status"].as_str(), Some("running" | "starting"));
    let guidance = agent["nativeRoleGuidance"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let cwd = agent["cwd"].as_str().unwrap_or_default().to_owned();
    if busy
        || source_provider == target_provider
        || guidance.is_empty()
        || cwd.is_empty()
        || agent
            .get("environment")
            .is_some_and(|value| value != "host")
    {
        return Ok(None);
    }
    let text = transcript(&items);
    let directory = PathBuf::from(&cwd).join(".studio-history");
    let path = directory.join(format!("{lead}-{}.md", now as u64));
    let (file_path, body) = (path.clone(), text);
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/simple_move_cross",
        move || {
            std::fs::create_dir_all(&directory)?;
            std::fs::write(&file_path, body)
        },
    )
    .await
    .map_err(|_| TransferError::Refused("The transcript write stopped"))?
    .map_err(|_| TransferError::Refused("The transcript could not be written"))?;
    let (id, target_key) = (lead.to_owned(), target.to_owned());
    let note = history_note(&path, source_provider.key());
    let source_epoch = agent["epoch"].clone();
    store
        .transaction_with_error(move |tx| {
            let value = AgentRepository::get(tx, &AgentId::from(id.as_str()))?
                .ok_or(TransferError::Refused("Unknown managed agent"))?
                .value();
            if legacy_vm_workspace(&value) {
                return Err(TransferError::Refused("VmWorkspaceUnsupported"));
            }
            if value["epoch"] != source_epoch
                || value["inFlight"].as_bool() == Some(true)
                || value["provider"].as_str() != Some(source_provider.key())
            {
                return Err(TransferError::Refused(
                    "The chat changed during the account move",
                ));
            }
            let mut value = release_source_holds(tx, &id, value, now)?;
            value["accountKey"] = json!(target_key);
            value["executionSettingsAccountKey"] = json!(target_key);
            value["provider"] = json!(target_provider.key());
            value["model"] = json!(crate::provider_adapter::default_model(target_provider));
            value["effort"] = json!("medium");
            value["nativeEffort"] = json!("medium");
            value["fastMode"] = json!(false);
            value["nativeRoleGuidance"] = json!(format!("{guidance}{note}"));
            if let Some(object) = value.as_object_mut() {
                for key in [
                    "threadId",
                    "frozenNativeParams",
                    "claudeOptions",
                    "pendingSettings",
                    "pendingSettingsAccountKey",
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
        "targetProvider": target_provider.key(),
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

#[cfg(test)]
mod cross_tests {
    use super::*;

    #[tokio::test]
    async fn a_move_releases_the_failure_hold_of_the_source_account() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        let released = store
            .transaction_with_error(|tx| {
                let value = json!({"id":"a","nativeFailureHold":true,
                    "error":"You've hit your weekly limit"});
                release_source_holds(tx, "a", value, 1.0)
            })
            .await
            .unwrap();
        assert!(released.get("nativeFailureHold").is_none());
        assert!(released["error"].is_null());
    }

    #[tokio::test]
    async fn legacy_vm_move_is_refused_before_writing_host_transcript() {
        let root = tempfile::tempdir().unwrap();
        let store = RuntimeStore::open(Some(root.path())).await.unwrap();
        let cwd = root.path().to_path_buf();
        let record = json!({"id":"a","rootId":"a","epoch":1,"compactions":0,
            "status":"idle","isLead":true,"provider":"codex","accountKey":"source",
            "workspaceMode":"layr","nativeRoleGuidance":"role","cwd":cwd});
        store
            .transaction(move |tx| {
                AgentRepository::put(
                    tx,
                    &studio_runtime_store::AgentRecord::from_value(record).unwrap(),
                )
            })
            .await
            .unwrap();
        let result = move_across_providers(
            &store,
            "a",
            "target",
            studio_runtime_providers::ProviderKind::Claude,
            "request",
            "team",
            1.0,
        )
        .await;
        assert!(
            matches!(
                result,
                Err(TransferError::Refused("VmWorkspaceUnsupported"))
            ),
            "{result:?}"
        );
        assert!(!root.path().join(".studio-history").exists());
    }

    #[test]
    fn the_transcript_keeps_user_and_assistant_text_in_order() {
        let items = vec![
            json!({"role":"user","text":"Build the drawing"}),
            json!({"role":"output","text":"tool noise"}),
            json!({"role":"assistant","text":"Done"}),
        ];
        assert_eq!(
            transcript(&items),
            "User: Build the drawing\n\nAssistant: Done\n\n"
        );
    }

    #[test]
    fn the_note_names_the_transcript_file() {
        let note = history_note(Path::new("/p/.studio-history/a.md"), "claude");
        assert!(note.contains("/p/.studio-history/a.md"));
        assert!(note.contains("claude"));
    }
}
