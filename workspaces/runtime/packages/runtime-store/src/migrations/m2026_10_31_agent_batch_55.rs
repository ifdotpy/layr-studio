use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    use studio_runtime_schema::ProviderKind;
    let providers = ProviderKind::ALL
        .map(|provider| format!("'{}'", provider.key()))
        .join(",");
    // These legacy keys have no production readers or writers.
    const DROPPED: &[&str] = &[
        "startRecoveryCheck",
        "workspaceBackend",
        "complaintsPresented",
        "checkpointError",
        "compactionsObservedOnly",
        "complaintMisses",
        "completedWorkerTurns",
        "supervisorRestore",
        "turnRecovery",
        "lastContextRepairWait",
        "lastContextRepairCheck",
        "countedWorkerTurn",
        "imageWorkspaceBaseError",
        "imageWorkspaceBaseRef",
        "imageWorkspaceCreatedAt",
        "imageWorkspaceError",
        "imageWorkspaceHasGit",
        "imageWorkspaceNoticeSent",
        "imageWorkspaceNoticeText",
        "imageWorkspaceRelative",
        "imageWorkspaceStartCommit",
        "imageWorkspaceSubpath",
    ];
    let dropped = DROPPED
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(",");
    let max_revision = studio_runtime_schema::shared_values::MAX_SAFE_INTEGER;
    let mut query = tx.prepare(&format!("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key IN ({dropped}) OR (j.key='excludedAccountKeys' AND (j.type NOT IN ('array','null') OR (j.type='array' AND EXISTS (SELECT 1 FROM json_each(j.value) item WHERE item.type != 'text')))) OR (j.key IN ('imageWorkspace','archived','pinned') AND j.type NOT IN ('true','false','null')) OR (j.key='maxAgents' AND (j.type NOT IN ('integer','real','null') OR (j.type IN ('integer','real') AND (j.atom < 0 OR j.atom > {max_revision} OR j.atom != CAST(j.atom AS INTEGER))))) OR (j.key IN ('error','lastCompletedTurnError') AND j.type != 'null') OR (j.key='layrReady' AND j.type NOT IN ('true','false')) OR (j.key IN ('lastCompletedTurn','hostProjectPath','executionSettingsAccountKey','lastCompletedTurnStatus','nativeEffort','projectFolder','checkpointHistoryHead','workspaceReservationId','workspaceOperation') AND j.type NOT IN ('text','null')) OR (j.key='provider' AND (j.type NOT IN ('text','null') OR (j.type='text' AND j.value NOT IN ({providers})))) OR (j.key='created' AND (j.type NOT IN ('integer','real','null') OR j.atom < 0)) OR (j.key IN ('queueMutationRevision','authResumeAttempt') AND (j.type != 'integer' OR j.atom < 0 OR j.atom > {max_revision})) OR (j.key IN ('excludedAccountKeys','imageWorkspace','archived','pinned','maxAgents','error','lastCompletedTurnError','layrReady','lastCompletedTurn','hostProjectPath','executionSettingsAccountKey','lastCompletedTurnStatus','nativeEffort','projectFolder','checkpointHistoryHead','workspaceReservationId','workspaceOperation','provider','created','queueMutationRevision','authResumeAttempt') AND (SELECT count(*) FROM json_each(record) d WHERE d.key=j.key)>1))"))?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&raw)?;
        for key in DROPPED {
            record.remove(*key);
        }
        // Malformed legacy workspace values (none in live data on 2026-11-01) are cleared with an error log; this may release a workspace that an unreadable value had blocked.
        for key in ["workspaceOperation", "workspaceReservationId"] {
            if let Some(message) = clear_bad_workspace_value(&mut record, key, &id) {
                eprintln!("{message}");
            }
        }
        if let Some(value) = record.get("excludedAccountKeys")
            && serde_json::from_str::<Option<Vec<String>>>(value.get()).is_err()
        {
            if let Ok(items) =
                serde_json::from_str::<Vec<Box<serde_json::value::RawValue>>>(value.get())
            {
                let keys: Vec<String> = items
                    .iter()
                    .filter_map(|item| serde_json::from_str(item.get()).ok())
                    .collect();
                record.insert(
                    "excludedAccountKeys".into(),
                    serde_json::value::RawValue::from_string(serde_json::to_string(&keys)?)?,
                );
            } else {
                record.remove("excludedAccountKeys");
            }
        }
        // For malformed legacy archived values, process_controls now uses truthiness instead of requiring the literal true.
        for key in ["archived", "pinned"] {
            if let Some(value) = record.get(key)
                && serde_json::from_str::<Option<bool>>(value.get()).is_err()
            {
                let parsed = serde_json::from_str(value.get())?;
                let normalized = crate::truthy(Some(&parsed));
                eprintln!("agent migration error: normalized unreadable {key} for {id}");
                record.insert(
                    key.into(),
                    serde_json::value::RawValue::from_string(normalized.to_string())?,
                );
            }
        }
        if record
            .get("imageWorkspace")
            .is_some_and(|v| serde_json::from_str::<Option<bool>>(v.get()).is_err())
        {
            record.remove("imageWorkspace");
        }
        if record.get("maxAgents").is_some_and(|v| {
            serde_json::from_str::<Option<studio_runtime_schema::AgentCounter>>(v.get()).is_err()
        }) {
            record.remove("maxAgents");
        }
        for key in ["error", "lastCompletedTurnError"] {
            if let Some(error) = record.get(key)
                && error.get() != "null"
            {
                let value = serde_json::from_str(error.get())?;
                let normalized =
                    studio_runtime_schema::provider_errors::AgentError::from_receipt(value)?;
                record.insert(
                    key.into(),
                    serde_json::value::RawValue::from_string(serde_json::to_string(&normalized)?)?,
                );
            }
        }
        if record
            .get("layrReady")
            .is_some_and(|v| serde_json::from_str::<bool>(v.get()).is_err())
        {
            record.remove("layrReady");
        }
        for key in [
            "checkpointHistoryHead",
            "projectFolder",
            "nativeEffort",
            "lastCompletedTurn",
            "hostProjectPath",
            "executionSettingsAccountKey",
            "lastCompletedTurnStatus",
        ] {
            if record
                .get(key)
                .is_some_and(|v| serde_json::from_str::<Option<String>>(v.get()).is_err())
            {
                record.remove(key);
            }
        }
        if record
            .get("provider")
            .is_some_and(|v| serde_json::from_str::<Option<ProviderKind>>(v.get()).is_err())
        {
            record.remove("provider");
        }
        if record.get("created").is_some_and(|v| {
            serde_json::from_str::<Option<studio_runtime_schema::NonnegativeNumber>>(v.get())
                .is_err()
        }) {
            record.remove("created");
        }
        for key in ["queueMutationRevision", "authResumeAttempt"] {
            if record.get(key).is_some_and(|v| {
                serde_json::from_str::<u64>(v.get())
                    .map_or(true, |revision| revision > max_revision as u64)
            }) {
                record.remove(key);
            }
        }
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
    }
    Ok(())
}

fn clear_bad_workspace_value(
    record: &mut std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
    key: &str,
    id: &str,
) -> Option<String> {
    if record
        .get(key)
        .is_some_and(|value| serde_json::from_str::<Option<String>>(value.get()).is_err())
    {
        record.remove(key);
        Some(format!(
            "agent migration error: cleared unreadable {key} for {id}"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn agent_batch_55_normalizes_all_owned_fields() {
        let mut malformed = serde_json::from_str::<
            std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
        >(r#"{"workspaceOperation":42,"workspaceReservationId":{}}"#)
        .unwrap();
        for key in ["workspaceOperation", "workspaceReservationId"] {
            assert_eq!(
                super::clear_bad_workspace_value(&mut malformed, key, "legacy"),
                Some(format!(
                    "agent migration error: cleared unreadable {key} for legacy"
                ))
            );
            assert!(!malformed.contains_key(key));
        }
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
        INSERT INTO runtime_agents VALUES('legacy','{"id":"legacy","provider":"future","created":-1,"complaintMisses":9,"startRecoveryCheck":{"state":"waiting"},"workspaceBackend":null,"complaintsPresented":["c"],"checkpointError":"legacy failure","compactionsObservedOnly":false,"turnRecovery":{"outcome":"idle"},"lastContextRepairWait":{"status":"superseded"},"lastContextRepairCheck":{"phase":"waiting"},"completedWorkerTurns":2,"supervisorRestore":{"status":"not_restored"},"countedWorkerTurn":true,"imageWorkspaceBaseError":"old","imageWorkspaceBaseRef":"main","imageWorkspaceCreatedAt":2,"imageWorkspaceError":"old","imageWorkspaceHasGit":true,"imageWorkspaceNoticeSent":true,"imageWorkspaceNoticeText":"old","imageWorkspaceRelative":".","imageWorkspaceStartCommit":"abc","imageWorkspaceSubpath":".","archived":"yes","pinned":[],"maxAgents":-1,"nativeEffort":42,"imageWorkspace":42,"projectFolder":42,"checkpointHistoryHead":42,"workspaceReservationId":42,"workspaceOperation":42,"excludedAccountKeys":["blocked",42],"layrReady":"bad","queueMutationRevision":-1,"authResumeAttempt":-1,"lastCompletedTurn":42,"hostProjectPath":42,"executionSettingsAccountKey":42,"lastCompletedTurnStatus":42,"error":{"error":{"message":"nested","code":42},"ignored":true},"lastCompletedTurnError":{"message":"previous","ignored":true}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row("SELECT record FROM runtime_agents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            raw,
            r#"{"archived":true,"error":{"message":"nested","code":"42","error":{"code":42,"message":"nested"},"ignored":true},"excludedAccountKeys":["blocked"],"id":"legacy","lastCompletedTurnError":{"message":"previous","ignored":true},"pinned":false}"#
        );
        tx.execute("INSERT INTO runtime_agents VALUES('encoded', ?1)", [r#"{"id":"encoded","error":"{\"error\":{\"message\":\"Write failed\",\"data\":{\"diskFull\":true}}}"}"#]).unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='encoded'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let record: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            record.pointer("/error/message"),
            Some(&serde_json::json!("Write failed"))
        );
        assert_eq!(
            record.pointer("/error/data/diskFull"),
            Some(&serde_json::json!(true))
        );
        tx.execute(
            "INSERT INTO runtime_agents VALUES('bad-list', ?1)",
            [r#"{"id":"bad-list","excludedAccountKeys":"invalid"}"#],
        )
        .unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='bad-list'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, r#"{"id":"bad-list"}"#);
        tx.execute("INSERT INTO runtime_agents VALUES('details', ?1)", [r#"{"id":"details","error":{"message":{"reason":"blocked"},"data":{"request":"one"},"additionalDetails":"detail","ignored":true}}"#]).unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='details'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            raw,
            r#"{"error":{"message":"{\"reason\":\"blocked\"}","data":{"request":"one"},"additionalDetails":"detail","ignored":true,"rawEvidence":{"additionalDetails":"detail","data":{"request":"one"},"ignored":true,"message":{"reason":"blocked"}},"reason":"blocked"},"id":"details"}"#
        );
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn real_agent_batch_55_migrates_without_refusal() {
        let path = std::env::var_os("STUDIO_AGENT_BATCH_55_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_agents").unwrap() {
            println!("agent table absent; rows=0 refused=0");
            return;
        }
        super::super::m2026_10_31_native_status_error::apply(&tx).unwrap();
        super::super::m2026_10_31_agent_permissions::apply(&tx).unwrap();
        super::super::m2026_10_31_agent_core::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        super::super::m2026_11_01_agent_capacity::apply(&tx).unwrap();
        super::super::m2026_11_01_agent_naming::apply(&tx).unwrap();
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
            if crate::records::parse_agent_record(&id, &raw).is_err() {
                refused += 1;
            }
        }
        println!("agent rows={total} refused={refused}");
        assert_eq!(refused, 0);
        drop(query);
        tx.commit().unwrap();
    }
}
