use crate::StoreResult;
use rusqlite::Transaction;

// Top-level AgentRecord Option fields. Field<T> and nested snapshots retain null.
const OPTIONAL_FIELDS: &[&str] = &[
    "draft",
    "parentId",
    "deletedAt",
    "agentArchive",
    "sandbox",
    "cleanedWorktree",
    "nativeSafetyBuffering",
    "nativeStatus",
    "isLead",
    "workspaceMode",
    "permissionMode",
    "model",
    "cwd",
    "name",
    "compactions",
    "fastMode",
    "daybreakEnabled",
    "agentModeSupported",
    "layrReady",
    "queueMutationRevision",
    "authResumeAttempt",
    "worktreeReady",
    "approvalPolicy",
    "nativeResponseTurn",
    "parkedEvent",
    "startWait",
    "events",
    "lastEvent",
    "activity",
    "activeTools",
    "tail",
    "lastAnswer",
    "contextUsage",
    "tokensUsed",
    "tokenUsageAccounting",
    "inFlight",
    "autoWake",
    "usageResumeEnabled",
    "executionMode",
    "turnEpoch",
    "threadId",
    "titleJob",
    "startAttempt",
    "disconnectRecovery",
    "restartRecovery",
    "startOutcomeHold",
    "nativeReviewThread",
    "vmLaunch",
    "credentialCopy",
    "vmAuthBootstrap",
    "nativeRelease",
    "creationIntent",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let keys = OPTIONAL_FIELDS
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) WHERE type='null' AND key IN ({keys}))"
    );
    let mut query = tx.prepare(&sql)?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&raw)?;
        for field in OPTIONAL_FIELDS {
            if record
                .get(*field)
                .is_some_and(|value| value.get() == "null")
            {
                record.remove(*field);
            }
        }
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn optional_field_list_matches_agent_record() {
        let record = include_str!("../records.rs")
            .split("pub struct AgentRecord {")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        let mut fields = record
            .lines()
            .filter_map(|line| {
                let line = line.trim().strip_prefix("pub ")?;
                let (name, ty) = line.split_once(": ")?;
                if !ty.starts_with("Option<") {
                    return None;
                }
                let mut parts = name.split('_');
                let mut name = parts.next().unwrap().to_owned();
                for part in parts {
                    let mut chars = part.chars();
                    if let Some(first) = chars.next() {
                        name.extend(first.to_uppercase());
                        name.push_str(chars.as_str());
                    }
                }
                Some(name)
            })
            .collect::<Vec<_>>();
        let mut expected = super::OPTIONAL_FIELDS
            .iter()
            .map(|field| field.to_string())
            .collect::<Vec<_>>();
        fields.sort();
        expected.sort();
        assert_eq!(
            fields, expected,
            "AgentRecord owns the migration field list"
        );
    }

    #[test]
    fn other_optional_fields_are_normalized() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT); INSERT INTO runtime_agents VALUES('a','{"id":"a","rootId":"a","epoch":1,"status":"idle","accountKey":"default","compactions":0,"name":null,"deletedAt":null,"turnEpoch":null,"turnId":null}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let record: String = tx
            .query_row("SELECT record FROM runtime_agents", [], |r| r.get(0))
            .unwrap();
        let record: serde_json::Value = serde_json::from_str(&record).unwrap();
        for field in ["name", "deletedAt", "turnEpoch"] {
            assert!(record.get(field).is_none(), "{field}");
        }
        assert!(record.get("turnId").unwrap().is_null());
        let typed = crate::AgentRecord::from_value(record).unwrap();
        let round_trip = typed.value();
        assert!(round_trip.get("name").is_none());
        assert!(round_trip.get("deletedAt").is_none());
        assert!(round_trip.get("turnEpoch").is_none());
        assert!(round_trip.get("turnId").unwrap().is_null());
    }

    #[test]
    fn null_optional_fields_are_removed_without_changing_values() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT); INSERT INTO runtime_agents VALUES ('nulls', '{\"threadId\":null,\"cwd\":null,\"fastMode\":null,\"daybreakEnabled\":null,\"model\":null,\"permissionMode\":null,\"lazyAccountTransfer\":{\"sourceState\":{\"model\":null}}}'), ('values', '{\"threadId\":\"thread\",\"cwd\":\"/project\",\"fastMode\":false,\"daybreakEnabled\":true,\"model\":\"named\",\"permissionMode\":\"default\"}');").unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let nulls: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='nulls'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let values: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='values'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let nulls: serde_json::Value = serde_json::from_str(&nulls).unwrap();
        let values: serde_json::Value = serde_json::from_str(&values).unwrap();
        assert!(
            nulls["lazyAccountTransfer"]["sourceState"]
                .get("model")
                .unwrap()
                .is_null()
        );
        for field in [
            "threadId",
            "cwd",
            "fastMode",
            "daybreakEnabled",
            "model",
            "permissionMode",
        ] {
            assert!(nulls.get(field).is_none(), "{field}");
            assert!(values.get(field).is_some(), "{field}");
        }
    }
}
