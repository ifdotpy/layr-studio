use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    use studio_runtime_schema::claude_permissions::{PERMISSION_MODES, PermissionMode};
    let mut query = tx.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key='yoloMode' OR (j.key='claudeOptions' AND CASE WHEN j.type='object' THEN json_type(j.value,'$.permissionMode') IS NOT NULL ELSE 0 END))")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: serde_json::Value = serde_json::from_str(&raw)?;
        let Some(object) = record.as_object_mut() else {
            continue;
        };
        let legacy = object
            .get_mut("claudeOptions")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|options| options.remove("permissionMode"));
        let mode = object
            .get("permissionMode")
            .and_then(serde_json::Value::as_str)
            .filter(|mode| PERMISSION_MODES.contains(mode))
            .or_else(|| {
                legacy
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
                    .filter(|mode| PERMISSION_MODES.contains(mode))
            })
            .unwrap_or_else(|| {
                PermissionMode::from_full_access(
                    object.get("yoloMode").and_then(serde_json::Value::as_bool) == Some(true),
                )
                .as_str()
            })
            .to_owned();
        object.remove("yoloMode");
        object.insert("permissionMode".into(), mode.into());
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn folds_legacy_permissions_once() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
        INSERT INTO runtime_agents VALUES('codex','{"yoloMode":true}'),
          ('claude','{"yoloMode":false,"claudeOptions":{"permissionMode":"plan","thinking":false}}'),
          ('canonical','{"permissionMode":"auto","yoloMode":true}'),
          ('duplicate','{"yoloMode":false,"yoloMode":true}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let rows: Vec<(String, String, Option<String>)> = tx.prepare("SELECT id,json_extract(record,'$.permissionMode'),json_type(record,'$.yoloMode') FROM runtime_agents ORDER BY id").unwrap().query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().map(Result::unwrap).collect();
        assert_eq!(
            rows,
            [
                ("canonical".into(), "auto".into(), None),
                ("claude".into(), "plan".into(), None),
                ("codex".into(), "bypassPermissions".into(), None),
                ("duplicate".into(), "bypassPermissions".into(), None)
            ]
        );
        let thinking: bool = tx.query_row("SELECT json_extract(record,'$.claudeOptions.thinking') FROM runtime_agents WHERE id='claude'", [], |r| r.get(0)).unwrap();
        assert!(!thinking);
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn real_agent_permissions_migrate_without_refusal() {
        let path = std::env::var_os("STUDIO_AGENT_PERMISSIONS_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_agents").unwrap() {
            println!("agent table absent; rows=0 refused=0");
            return;
        }
        super::super::m2026_10_31_native_status_error::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let mut query = tx.prepare("SELECT id,record FROM runtime_agents").unwrap();
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut total = 0;
        let mut refused = 0;
        for row in rows {
            let (id, raw) = row.unwrap();
            total += 1;
            match crate::records::parse_agent_record(&id, &raw) {
                Ok(_) => {}
                Err(error) => {
                    refused += 1;
                    eprintln!("{id}: {error}");
                }
            }
        }
        println!("agent rows={total} refused={refused}");
        assert_eq!(refused, 0);
        drop(query);
        tx.commit().unwrap();
    }
}
