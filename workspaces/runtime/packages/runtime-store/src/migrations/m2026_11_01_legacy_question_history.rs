//! Converts agent questions that the 2026-10-30 approvals migration skipped.
//! That converter needs `createdAt`; these legacy rows have none, so they stayed
//! in `runtime_requests` and their question history kept the legacy shape. Only
//! question methods convert here. Other legacy rows keep their existing rule.
use crate::{StoreError, StoreResult, SyncDocumentsRepository, approvals};
use rusqlite::Transaction;
use serde_json::Value;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_requests")? {
        return Ok(());
    }
    let rows = {
        let mut statement = tx.prepare(
            "SELECT id,record FROM runtime_requests WHERE id NOT IN (SELECT id FROM approvals) \
             AND json_extract(record,'$.method') IN ('agent/asyncQuestion','item/tool/requestUserInput') \
             ORDER BY id",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, record) in rows {
        let mut value: Value = serde_json::from_str(&record)?;
        // A question without a time reads as time 0, the converter's own default.
        if let Some(object) = value.as_object_mut() {
            object.entry("createdAt").or_insert(Value::from(0));
        }
        let approval =
            match super::m2026_10_30_approvals::approval_from_record(&id, &value.to_string()) {
                Ok(approval) => approval,
                Err(error) => {
                    eprintln!("question history migration kept a legacy request: {error}");
                    continue;
                }
            };
        approvals::insert(tx, &approval).map_err(|error| {
            StoreError::SchemaMismatch(format!("question history migration: request {id}: {error}"))
        })?;
        SyncDocumentsRepository::put_entity(
            tx,
            "questionHistory",
            &id,
            serde_json::to_value(&approval)?,
            approval.hidden.is_some(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use studio_runtime_schema::approvals::ApprovalId;

    #[tokio::test]
    async fn a_question_without_created_at_moves_to_the_approvals_and_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(dir.path())).await.unwrap();
        store
            .transaction(|tx| {
                let record = json!({"id":"a:question:call_1","method":"agent/asyncQuestion","agent":"a","epoch":0,
                    "params":{"questions":[{"id":"0","question":"Which fixture?","options":[{"label":"A"},{"label":"B"}]}]},
                    "status":"answered"});
                tx.execute(
                    "INSERT INTO runtime_requests(id,record) VALUES('a:question:call_1',?1)",
                    [record.to_string()],
                )?;
                apply(tx)?;
                assert!(approvals::get(tx, &ApprovalId("a:question:call_1".into()))?.is_some());
                let projection: String = tx.query_row(
                    "SELECT payload FROM sync_entities WHERE collection='questionHistory' AND id='a:question:call_1'",
                    [],
                    |row| row.get(0),
                )?;
                assert!(projection.contains("Which fixture?"));
                Ok::<_, StoreError>(())
            })
            .await
            .unwrap();
    }
}
