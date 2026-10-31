//! Retire the `claude-local` account, which Studio backed by `~/.claude`.
//!
//! Studio keeps every Claude login in its own profile folder. Agents and
//! projects on `claude-local` move to the profile with the same Claude login
//! (`accountId`). Without such a profile they move to a signed-out entry in the
//! Studio state, so the UI asks for a sign-in. Both halves derive the target
//! from the same registry value, so a crash between them repeats safely.
use crate::{StoreResult, account_registry::RegistryOrder};
use rusqlite::Transaction;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const LOCAL: &str = "claude-local";
pub(crate) const SIGNED_OUT: &str = "claude-signed-out";

/// Account key that replaces `claude-local`, and the row to add if it is new.
pub(crate) struct Plan {
    pub target: String,
    signed_out: Option<Value>,
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
    Some(match matching {
        Some((key, _)) => Plan {
            target: key.clone(),
            signed_out: None,
        },
        None => {
            let home = root.join("claude-logins").join("config-signed-out");
            Plan {
                target: SIGNED_OUT.into(),
                signed_out: Some(json!({"id":SIGNED_OUT,"provider":"claude",
                    "home":home,"claudeOptions":{"configDir":home},
                    "label":"Claude (sign in again)","source":"Claude Code","status":"signedOut"})),
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
    let Some(plan) = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|registry| plan(&registry, root))
    else {
        return Ok(());
    };
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
            let mut record: Value = serde_json::from_str(&raw)?;
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
    if crate::schema::table_exists(tx, "sessions")? {
        tx.execute(
            "UPDATE sessions SET account_key=?1 WHERE account_key=?2",
            (&plan.target, LOCAL),
        )?;
    }
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
        serde_json::from_str(&raw).unwrap()
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
}
