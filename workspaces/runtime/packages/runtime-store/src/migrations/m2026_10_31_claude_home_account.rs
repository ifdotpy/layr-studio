//! Retire the `claude-local` account, which Studio backed by `~/.claude`.
//!
//! Studio keeps every Claude login in its own profile folder. Agents and
//! projects on `claude-local` move to the profile with the same Claude login
//! (`accountId`). Without such a profile they move to a signed-out entry in the
//! Studio state, so the UI asks for a sign-in. Both halves derive the target
//! from the same registry value, so a crash between them repeats safely.
//!
//! The session files of the moved chats are copied from the `~/.claude` folder
//! into the target profile at the same relative path. The source is only read.
//! Files already in the target are kept, so a repeated run copies nothing new.
use crate::{StoreResult, account_registry::RegistryOrder};
use rusqlite::Transaction;
use studio_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) const LOCAL: &str = "claude-local";
pub(crate) const SIGNED_OUT: &str = "claude-signed-out";

/// Account key that replaces `claude-local`, the row to add if it is new, and
/// the folders the session files are copied from and to.
pub(crate) struct Plan {
    pub target: String,
    signed_out: Option<Value>,
    source: Option<PathBuf>,
    target_dir: Option<PathBuf>,
}

/// The Claude config folder of a registry row: `claudeOptions.configDir`, else `home`.
fn config_dir(row: &Value) -> Option<PathBuf> {
    row.pointer("/claudeOptions/configDir")
        .or_else(|| row.get("home"))
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn plan(registry: &Value, root: &Path) -> Option<Plan> {
    let accounts = registry.get("accounts")?.as_object()?;
    let local = accounts.get(LOCAL)?;
    let login = local
        .get("accountId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    let matching = login.and_then(|login| {
        accounts.iter().find(|(key, row)| {
            key.as_str() != LOCAL
                && row.get("provider").and_then(Value::as_str) == Some("claude")
                && row.get("accountId").and_then(Value::as_str) == Some(login)
                && row.get("status").and_then(Value::as_str) == Some("ready")
                && row.get("deleted").is_none_or(|v| v == &json!(false))
        })
    });
    let source = config_dir(local);
    Some(match matching {
        Some((key, row)) => Plan {
            target: key.clone(),
            signed_out: None,
            source,
            target_dir: config_dir(row),
        },
        None => {
            let home = root.join("claude-logins").join("config-signed-out");
            Plan {
                target: SIGNED_OUT.into(),
                signed_out: Some(json!({"id":SIGNED_OUT,"provider":"claude",
                    "home":home,"claudeOptions":{"configDir":home},
                    "label":"Claude (sign in again)","source":"Claude Code","status":"signedOut"})),
                source,
                target_dir: Some(home),
            }
        }
    })
}

/// Registry half. Returns whether the registry changed.
pub(crate) fn migrate_registry(
    registry: &mut Value,
    order: &mut RegistryOrder,
    root: &Path,
) -> bool {
    let Some(plan) = plan(registry, root) else {
        return false;
    };
    let accounts = registry["accounts"].as_object_mut().expect("planned");
    accounts.remove(LOCAL);
    order.accounts.retain(|key| key != LOCAL);
    if let Some(row) = plan.signed_out {
        accounts.insert(SIGNED_OUT.into(), row);
        order.accounts.push(SIGNED_OUT.into());
    }
    true
}

fn swap(value: &mut Value, field: &str, target: &str) {
    if value.get(field).and_then(Value::as_str) == Some(LOCAL) {
        value[field] = json!(target);
    }
}

/// Agent and project half.
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let Some(root) = tx.path().and_then(|p| Path::new(p).parent()) else {
        return Ok(());
    };
    let Ok(bytes) = std::fs::read(root.join("accounts/registry.json")) else {
        return Ok(());
    };
    let Some(plan) = studio_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|registry| plan(&registry, root))
    else {
        return Ok(());
    };
    let mut threads = BTreeSet::new();
    for (table, fields) in [
        (
            "runtime_agents",
            &[
                "accountKey",
                "pendingSettingsAccountKey",
                "executionSettingsAccountKey",
            ][..],
        ),
        ("runtime_projects", &["accountKey"][..]),
    ] {
        if !crate::schema::table_exists(tx, table)? {
            continue;
        }
        let rows = tx
            .prepare(&format!(
                "SELECT id,record FROM {table} WHERE json_valid(record) AND instr(record,'{LOCAL}')>0"
            ))?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, raw) in rows {
            let mut record: Value = studio_json::from_str(&raw)?;
            let moved = fields
                .iter()
                .any(|field| record.get(*field).and_then(Value::as_str) == Some(LOCAL));
            if table == "runtime_agents"
                && moved
                && let Some(thread) = record.get("threadId").and_then(Value::as_str)
            {
                threads.insert(thread.to_owned());
            }
            for field in fields {
                swap(&mut record, field, &plan.target);
            }
            if let Some(keys) = record.get_mut("accountKeys").and_then(Value::as_array_mut) {
                for key in keys.iter_mut().filter(|k| *k == LOCAL) {
                    *key = json!(plan.target);
                }
                let mut seen = std::collections::HashSet::new();
                keys.retain(|key| seen.insert(key.to_string()));
            }
            tx.execute(
                &format!("UPDATE {table} SET record=?2 WHERE id=?1"),
                (&id, record.to_string()),
            )?;
        }
    }
    if let (Some(source), Some(target)) = (plan.source.as_deref(), plan.target_dir.as_deref()) {
        let (copied, kept) = copy_sessions(source, target, &threads)?;
        eprintln!(
            "claude home account migration: chats={} session files copied={copied} kept={kept}",
            threads.len()
        );
    }
    if crate::schema::table_exists(tx, "sessions")? {
        tx.execute(
            "UPDATE sessions SET account_key=?1 WHERE account_key=?2",
            (&plan.target, LOCAL),
        )?;
    }
    Ok(())
}

fn valid_thread(thread: &str) -> bool {
    !thread.is_empty()
        && thread
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Copies the session files of `threads` from each project folder of `source`
/// to the same relative path in `target`. Returns (copied, kept) file counts.
fn copy_sessions(
    source: &Path,
    target: &Path,
    threads: &BTreeSet<String>,
) -> StoreResult<(usize, usize)> {
    let mut counts = (0, 0);
    if source == target {
        return Ok(counts);
    }
    let Ok(projects) = std::fs::read_dir(source.join("projects")) else {
        return Ok(counts);
    };
    for project in projects {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        for thread in threads.iter().filter(|thread| valid_thread(thread)) {
            let from = project.path();
            let to = target.join("projects").join(project.file_name());
            copy_file(
                &from.join(format!("{thread}.jsonl")),
                &to.join(format!("{thread}.jsonl")),
                &mut counts,
            )?;
            copy_tree(&from.join(thread), &to.join(thread), &mut counts)?;
        }
    }
    Ok(counts)
}

fn copy_tree(from: &Path, to: &Path, counts: &mut (usize, usize)) -> StoreResult<()> {
    if !from.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let to = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to, counts)?;
        } else {
            copy_file(&entry.path(), &to, counts)?;
        }
    }
    Ok(())
}

/// Copies one file unless the target already has it. A failed copy removes its
/// partial target, so a repeated run copies the file again.
fn copy_file(from: &Path, to: &Path, counts: &mut (usize, usize)) -> StoreResult<()> {
    if !from.is_file() {
        return Ok(());
    }
    let mut input = std::fs::File::open(from)?;
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            counts.1 += 1;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if let Err(error) = std::io::copy(&mut input, &mut output) {
        drop(output);
        std::fs::remove_file(to).ok();
        return Err(error.into());
    }
    counts.0 += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn registry() -> Value {
        json!({"version":2,"accounts":{
            "claude-local":{"id":"claude-local","provider":"claude","home":"/h/.claude","accountId":"claude:a@x.com","status":"ready"},
            "claude-profile-b":{"id":"claude-profile-b","provider":"claude","accountId":"claude:b@x.com","status":"ready"},
            "claude-profile-a":{"id":"claude-profile-a","provider":"claude","accountId":"claude:a@x.com","status":"ready"}},"logins":{}})
    }

    fn state(registry: &Value) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("accounts")).unwrap();
        std::fs::write(
            dir.path().join("accounts/registry.json"),
            registry.to_string(),
        )
        .unwrap();
        let db = Connection::open(dir.path().join("canvas.sqlite3")).unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE runtime_projects(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            INSERT INTO runtime_agents VALUES('a1','{\"id\":\"a1\",\"accountKey\":\"claude-local\"}'),('a2','{\"id\":\"a2\",\"accountKey\":\"other\"}');
            INSERT INTO runtime_projects VALUES('p1','{\"id\":\"p1\",\"accountKey\":\"claude-local\",\"accountKeys\":[\"claude-local\",\"claude-profile-a\"]}');").unwrap();
        (dir, db)
    }

    fn record(db: &Connection, table: &str, id: &str) -> Value {
        let raw: String = db
            .query_row(
                &format!("SELECT record FROM {table} WHERE id=?1"),
                [id],
                |r| r.get(0),
            )
            .unwrap();
        studio_json::from_str(&raw).unwrap()
    }

    #[test]
    fn moves_agents_and_projects_to_the_profile_with_the_same_login() {
        let (dir, mut db) = state(&registry());
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            record(&tx, "runtime_agents", "a1")["accountKey"],
            "claude-profile-a"
        );
        assert_eq!(record(&tx, "runtime_agents", "a2")["accountKey"], "other");
        let project = record(&tx, "runtime_projects", "p1");
        assert_eq!(project["accountKey"], "claude-profile-a");
        assert_eq!(project["accountKeys"], json!(["claude-profile-a"]));
        let mut value = registry();
        let mut order = RegistryOrder {
            accounts: vec!["claude-local".into(), "claude-profile-a".into()],
            logins: vec![],
        };
        assert!(migrate_registry(&mut value, &mut order, dir.path()));
        assert!(value["accounts"].get("claude-local").is_none());
        assert_eq!(order.accounts, ["claude-profile-a"]);
        assert!(!migrate_registry(&mut value, &mut order, dir.path()));
    }

    #[test]
    fn without_a_matching_profile_agents_get_a_signed_out_studio_entry() {
        let mut value = registry();
        value["accounts"]
            .as_object_mut()
            .unwrap()
            .remove("claude-profile-a");
        let (dir, mut db) = state(&value);
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            record(&tx, "runtime_agents", "a1")["accountKey"],
            SIGNED_OUT
        );
        let mut order = RegistryOrder::default();
        assert!(migrate_registry(&mut value, &mut order, dir.path()));
        let row = &value["accounts"][SIGNED_OUT];
        assert_eq!(row["status"], "signedOut");
        assert!(
            row["home"]
                .as_str()
                .unwrap()
                .starts_with(dir.path().to_str().unwrap())
        );
        assert!(value["accounts"].get("claude-local").is_none());
    }

    #[test]
    fn copies_session_files_of_moved_chats_and_never_touches_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("home/.claude");
        let target = dir.path().join("studio/claude-logins/profile-a");
        let project = source.join("projects/-work");
        std::fs::create_dir_all(project.join("t1/subagents")).unwrap();
        std::fs::write(project.join("t1.jsonl"), "main").unwrap();
        std::fs::write(project.join("t1/subagents/agent-1.jsonl"), "sub").unwrap();
        std::fs::write(project.join("other.jsonl"), "other").unwrap();
        std::fs::create_dir_all(target.join("projects/-work")).unwrap();
        std::fs::write(target.join("projects/-work/t1.jsonl"), "kept").unwrap();
        let registry = json!({"version":2,"accounts":{
            "claude-local":{"id":"claude-local","provider":"claude","home":source.to_str().unwrap(),"accountId":"claude:a@x.com","status":"ready"},
            "claude-profile-a":{"id":"claude-profile-a","provider":"claude","accountId":"claude:a@x.com","status":"ready","claudeOptions":{"configDir":target.to_str().unwrap()}}},"logins":{}});
        std::fs::create_dir(dir.path().join("accounts")).unwrap();
        std::fs::write(
            dir.path().join("accounts/registry.json"),
            registry.to_string(),
        )
        .unwrap();
        let mut db = Connection::open(dir.path().join("canvas.sqlite3")).unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            CREATE TABLE runtime_projects(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            INSERT INTO runtime_agents VALUES('a1','{\"id\":\"a1\",\"accountKey\":\"claude-local\",\"threadId\":\"t1\"}');",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            record(&tx, "runtime_agents", "a1")["accountKey"],
            "claude-profile-a"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("projects/-work/t1.jsonl")).unwrap(),
            "kept"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("projects/-work/t1/subagents/agent-1.jsonl"))
                .unwrap(),
            "sub"
        );
        assert!(!target.join("projects/-work/other.jsonl").exists());
        assert_eq!(
            std::fs::read_to_string(project.join("t1.jsonl")).unwrap(),
            "main"
        );
        assert_eq!(
            std::fs::read_to_string(project.join("other.jsonl")).unwrap(),
            "other"
        );
        let threads = BTreeSet::from(["t1".to_owned()]);
        assert_eq!(copy_sessions(&source, &target, &threads).unwrap(), (0, 2));
    }
}
