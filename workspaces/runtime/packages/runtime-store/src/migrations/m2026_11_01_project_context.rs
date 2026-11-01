use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;
use serde::Deserialize;
use serde_json::Value;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::project_projection::{ProjectReadiness, ProjectSync, ProjectVault};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,payload FROM sync_entities WHERE collection='project' AND deleted=0 AND payload IS NOT NULL")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let payload: String = row.get(1)?;
        let Ok(mut payload) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        let Some(value) = payload.get_mut("value").and_then(Value::as_object_mut) else {
            continue;
        };
        let mut changed = false;
        for field in ["layrReadiness", "layrSync", "vault"] {
            let invalid = value.get(field).is_some_and(|item| {
                !item.is_null()
                    && match field {
                        "layrReadiness" => {
                            JsonObject::<ProjectReadiness>::deserialize(item).is_err()
                        }
                        "layrSync" => JsonObject::<ProjectSync>::deserialize(item).is_err(),
                        "vault" => JsonObject::<ProjectVault>::deserialize(item).is_err(),
                        _ => false,
                    }
            });
            if invalid {
                value.remove(field);
                changed = true;
            }
        }
        if changed {
            SyncDocumentsRepository::put_entity(tx, "project", &id, value, false)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn drops_invalid_context_and_keeps_valid_context_idempotently() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,PRIMARY KEY(collection,id)); CREATE TABLE sync_documents(seq INTEGER); CREATE TABLE durable_rows(seq INTEGER); CREATE TABLE sync_generation(id INTEGER PRIMARY KEY,value INTEGER NOT NULL); INSERT INTO sync_generation VALUES(1,0);").unwrap();
        let tx = db.transaction().unwrap();
        SyncDocumentsRepository::put_entity(&tx, "project", "p", json!({"id":"p","layrReadiness":{"state":"ready"},"layrSync":{"state":"bad"},"vault":{"projectId":"p","state":"none"}}), false).unwrap();
        apply(&tx).unwrap();
        let first: String = tx
            .query_row(
                "SELECT payload FROM sync_entities WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        apply(&tx).unwrap();
        let second: String = tx
            .query_row(
                "SELECT payload FROM sync_entities WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first, second);
        let value: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(value["value"]["layrReadiness"]["state"], "ready");
        assert_eq!(value["value"]["vault"]["state"], "none");
        assert!(value["value"].get("layrSync").is_none());
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn verify_project_context_migration_on_copy() {
        let path = std::env::var_os("STUDIO_PROJECT_CONTEXT_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "sync_entities").unwrap() {
            println!("sync table absent; project contexts=0");
            return;
        }
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let mut statement = tx.prepare("SELECT payload FROM sync_entities WHERE collection='project' AND deleted=0 AND payload IS NOT NULL").unwrap();
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap();
        let mut count = 0;
        let mut refused = 0;
        for row in rows {
            count += 1;
            let payload: Value = serde_json::from_str(&row.unwrap()).unwrap();
            let value = &payload["value"];
            for (name, valid) in [
                (
                    "layrReadiness",
                    value.get("layrReadiness").is_none_or(|v| {
                        v.is_null() || JsonObject::<ProjectReadiness>::deserialize(v).is_ok()
                    }),
                ),
                (
                    "layrSync",
                    value.get("layrSync").is_none_or(|v| {
                        v.is_null() || JsonObject::<ProjectSync>::deserialize(v).is_ok()
                    }),
                ),
                (
                    "vault",
                    value.get("vault").is_none_or(|v| {
                        v.is_null() || JsonObject::<ProjectVault>::deserialize(v).is_ok()
                    }),
                ),
            ] {
                if !valid {
                    eprintln!("invalid project context: {name}");
                    refused += 1;
                }
            }
        }
        println!("project contexts={count}, refused={refused}");
        assert_eq!(refused, 0);
        drop(statement);
        tx.commit().unwrap();
    }
}
