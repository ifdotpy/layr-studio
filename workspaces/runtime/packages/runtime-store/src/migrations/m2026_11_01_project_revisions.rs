use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_projects")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,record FROM runtime_projects")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw: String = row.get(1)?;
        let Ok(serde_json::Value::Object(mut fields)) = serde_json::from_str(&raw) else {
            continue;
        };
        let mut changed = false;
        for name in [
            "accountRevision",
            "accountGroupsRevision",
            "locationsRevision",
            "peerTeamsRevision",
            "organizationRevision",
            "workerBaseRevision",
            "workerEnvironmentRevision",
            "workspacePolicyRevision",
        ] {
            if let Some(serde_json::Value::Number(number)) = fields.get_mut(name)
                && number.is_f64()
                && let Some(value) = number.as_f64()
                && value >= 0.0
                && value < u64::MAX as f64
                && value.fract() == 0.0
            {
                *number = serde_json::Number::from(value as u64);
                changed = true;
            }
        }
        if changed {
            tx.execute(
                "UPDATE runtime_projects SET record=?2 WHERE id=?1",
                (&id, serde_json::to_string(&fields)?),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_integral_float_revisions_without_rewriting_other_rows() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_projects(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE writes(id TEXT); CREATE TRIGGER log_write AFTER UPDATE ON runtime_projects BEGIN INSERT INTO writes VALUES(new.id); END;").unwrap();
        let untouched = r#"{ "path": "/b", "id": "b", "accountRevision": 2 }"#;
        db.execute(
            "INSERT INTO runtime_projects VALUES('a',?1)",
            [r#"{"id":"a","path":"/a","accountRevision":1.0,"locationsRevision":2.0,"workerBaseRevision":10000000000000000.0}"#],
        )
        .unwrap();
        db.execute("INSERT INTO runtime_projects VALUES('b',?1)", [untouched])
            .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_projects WHERE id='a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let parsed: studio_runtime_schema::project_record::ProjectLive =
            serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.account_revision.value(), Some(&1));
        assert_eq!(parsed.locations_revision.value(), Some(&2));
        assert_eq!(
            parsed.worker_base_revision.value(),
            Some(&10_000_000_000_000_000)
        );
        assert_eq!(
            tx.query_row(
                "SELECT record FROM runtime_projects WHERE id='b'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            untouched
        );
        assert_eq!(
            tx.query_row("SELECT count(*) FROM writes", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
