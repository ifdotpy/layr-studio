use crate::StoreResult;
use rusqlite::Transaction;
use serde_json::Value;
use studio_runtime_schema::project_record::ProjectRecord;

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_projects")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record FROM runtime_projects")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw: String = row.get(1)?;
        if serde_json::from_str::<ProjectRecord>(&raw).is_ok() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if value.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            continue;
        }
        let normalized = serde_json::to_string(&value)?;
        if normalized != raw {
            tx.execute(
                "UPDATE runtime_projects SET record=?2 WHERE id=?1",
                (&id, normalized),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn normalizes_duplicate_project_fields_and_keeps_invalid_rows() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_projects(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_projects VALUES('p','{"recordKind":"live","id":"p","path":"/old","path":"/new","peerTeams":[]}'),('bad','{"id":"bad","path":false}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let ProjectRecord::Live(project) = serde_json::from_str(&raw).unwrap() else {
            panic!("live project")
        };
        assert_eq!(project.path, "/new");
        let invalid: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='bad'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invalid, r#"{"id":"bad","path":false}"#);
        apply(&tx).unwrap();
        let again: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='p'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, again);
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn verify_sync_project_migration_on_copy() {
        let path = std::env::var_os("STUDIO_PROJECT_MIGRATION_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_projects").unwrap() {
            println!("project table absent; project rows=0, refused=0");
            return;
        }
        super::super::m2026_10_30_project_teams::apply(&tx).unwrap();
        apply(&tx).unwrap();
        super::super::m2026_10_31_project_record_kind::apply(&tx).unwrap();
        apply(&tx).unwrap();
        let mut query = tx
            .prepare("SELECT id,record FROM runtime_projects")
            .unwrap();
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
            let valid = match serde_json::from_str::<ProjectRecord>(&raw) {
                Ok(ProjectRecord::Live(project)) => project.id == id,
                Ok(ProjectRecord::Removal(project)) => project.id == id,
                Err(_) => false,
            };
            if !valid {
                refused += 1;
            }
        }
        println!("project rows={total}, refused={refused}");
        assert_eq!(refused, 0);
        drop(query);
        tx.commit().unwrap();
    }
}
