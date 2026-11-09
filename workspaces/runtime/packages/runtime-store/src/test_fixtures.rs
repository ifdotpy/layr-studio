//! Constructors for frozen pre-typed differential corpora. Never used on stored state.
mod agent_core {
    use crate::StoreResult;
    use rusqlite::Transaction;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        use studio_runtime_schema::creation_intent::AgentRole;
        const FLAGS: &[&str] = &[
            "compactionsObservedOnly",
            "fastMode",
            "daybreakEnabled",
            "needsTitle",
            "manualName",
            "maxAgentsExplicit",
            "agentModeSupported",
        ];
        let flags = FLAGS
            .iter()
            .map(|field| format!("'{field}'"))
            .collect::<Vec<_>>()
            .join(",");
        const TEXT: &[&str] = &["prompt", "profileId", "profileInstructions", "effort"];
        let text = TEXT
            .iter()
            .map(|field| format!("'{field}'"))
            .collect::<Vec<_>>()
            .join(",");
        let roles = [
            AgentRole::Orchestrator,
            AgentRole::Implementer,
            AgentRole::Reviewer,
        ]
        .map(|role| format!("'{}'", role.as_str()))
        .join(",");
        let mut query = tx.prepare(&format!("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND EXISTS (SELECT 1 FROM json_each(record) j WHERE j.key IN ('nativeNameSynced','agentModeChangedAt','agentModeChangedBy','tokenBudget') OR (j.key IN ({flags}) AND j.type NOT IN ('true','false')) OR (j.key IN ({text}) AND j.type NOT IN ('text','null')) OR (j.key='role' AND (j.type NOT IN ('text','null') OR (j.type='text' AND j.value NOT IN ({roles})))) OR (j.key IN ({flags},{text},'role') AND (SELECT count(*) FROM json_each(record) d WHERE d.key=j.key)>1))"))?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, raw) = row?;
            let mut record: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
                studio_json::from_str(&raw)?;
            for field in [
                "nativeNameSynced",
                "agentModeChangedAt",
                "agentModeChangedBy",
                "tokenBudget",
            ] {
                record.remove(field);
            }
            for field in FLAGS {
                if record
                    .get(*field)
                    .is_some_and(|value| studio_json::from_str::<bool>(value.as_str()).is_err())
                {
                    record.remove(*field);
                }
            }
            for field in TEXT {
                if record.get(*field).is_some_and(|value| {
                    studio_json::from_str::<Option<String>>(value.as_str()).is_err()
                }) {
                    record.remove(*field);
                }
            }
            if record.get("role").is_some_and(|value| {
                studio_json::from_str::<Option<AgentRole>>(value.as_str()).is_err()
            }) {
                let role = record
                    .get("role")
                    .and_then(|value| studio_json::from_str::<String>(value.as_str()).ok());
                let native_review = record
                    .get("nativeReview")
                    .and_then(|value| studio_json::from_str(value.as_str()).ok());
                let is_lead = record
                    .get("isLead")
                    .and_then(|value| studio_json::from_str::<bool>(value.as_str()).ok())
                    .unwrap_or(false);
                let inferred = if role.as_deref() == Some("lead") {
                    AgentRole::Orchestrator
                } else {
                    AgentRole::inferred(crate::truthy(native_review.as_ref()), is_lead)
                };
                record.insert("role".into(), studio_json::to_raw_value(&inferred)?);
            }
            tx.execute(
                "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                rusqlite::params![studio_json::to_string(&record)?, id],
            )?;
        }
        Ok(())
    }
}
mod agent_batch {
    use crate::StoreResult;
    use rusqlite::Transaction;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
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
            let mut record: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
                studio_json::from_str(&raw)?;
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
                && studio_json::from_str::<Option<Vec<String>>>(value.as_str()).is_err()
            {
                if let Ok(items) =
                    studio_json::from_str::<Vec<studio_json::OwnedRawJson>>(value.as_str())
                {
                    let keys: Vec<String> = items
                        .iter()
                        .filter_map(|item| studio_json::from_str(item.as_str()).ok())
                        .collect();
                    record.insert(
                        "excludedAccountKeys".into(),
                        studio_json::OwnedRawJson::from_string(studio_json::to_string(&keys)?)?,
                    );
                } else {
                    record.remove("excludedAccountKeys");
                }
            }
            // For malformed legacy archived values, process_controls now uses truthiness instead of requiring the literal true.
            for key in ["archived", "pinned"] {
                if let Some(value) = record.get(key)
                    && studio_json::from_str::<Option<bool>>(value.as_str()).is_err()
                {
                    let parsed = studio_json::from_str(value.as_str())?;
                    let normalized = crate::truthy(Some(&parsed));
                    eprintln!("agent migration error: normalized unreadable {key} for {id}");
                    record.insert(
                        key.into(),
                        studio_json::OwnedRawJson::from_string(normalized.to_string())?,
                    );
                }
            }
            if record
                .get("imageWorkspace")
                .is_some_and(|v| studio_json::from_str::<Option<bool>>(v.as_str()).is_err())
            {
                record.remove("imageWorkspace");
            }
            if record.get("maxAgents").is_some_and(|v| {
                studio_json::from_str::<Option<studio_runtime_schema::AgentCounter>>(v.as_str())
                    .is_err()
            }) {
                record.remove("maxAgents");
            }
            for key in ["error", "lastCompletedTurnError"] {
                if let Some(error) = record.get(key)
                    && error.as_str() != "null"
                {
                    let value = studio_json::from_str(error.as_str())?;
                    let normalized =
                        studio_runtime_schema::provider_errors::AgentError::from_receipt(value)?;
                    record.insert(
                        key.into(),
                        studio_json::OwnedRawJson::from_string(studio_json::to_string(
                            &normalized,
                        )?)?,
                    );
                }
            }
            if record
                .get("layrReady")
                .is_some_and(|v| studio_json::from_str::<bool>(v.as_str()).is_err())
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
                    .is_some_and(|v| studio_json::from_str::<Option<String>>(v.as_str()).is_err())
                {
                    record.remove(key);
                }
            }
            if record
                .get("provider")
                .is_some_and(|v| studio_json::from_str::<Option<ProviderKind>>(v.as_str()).is_err())
            {
                record.remove("provider");
            }
            if record.get("created").is_some_and(|v| {
                studio_json::from_str::<Option<studio_runtime_schema::NonnegativeNumber>>(
                    v.as_str(),
                )
                .is_err()
            }) {
                record.remove("created");
            }
            for key in ["queueMutationRevision", "authResumeAttempt"] {
                if record.get(key).is_some_and(|v| {
                    studio_json::from_str::<u64>(v.as_str())
                        .map_or(true, |revision| revision > max_revision as u64)
                }) {
                    record.remove(key);
                }
            }
            let normalized = studio_json::to_string(&record)?;
            if studio_json::from_str::<studio_json::Value>(&raw)?
                == studio_json::from_str::<studio_json::Value>(&normalized)?
                && !tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM json_each(?1) GROUP BY key HAVING count(*) > 1)",
                    [&raw],
                    |row| row.get::<_, bool>(0),
                )?
            {
                continue;
            }
            tx.execute(
                "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                rusqlite::params![normalized, id],
            )?;
        }
        Ok(())
    }

    fn clear_bad_workspace_value(
        record: &mut std::collections::BTreeMap<String, studio_json::OwnedRawJson>,
        key: &str,
        id: &str,
    ) -> Option<String> {
        if record
            .get(key)
            .is_some_and(|value| studio_json::from_str::<Option<String>>(value.as_str()).is_err())
        {
            record.remove(key);
            Some(format!(
                "agent migration error: cleared unreadable {key} for {id}"
            ))
        } else {
            None
        }
    }
}
mod agent_capacity {
    use crate::StoreResult;
    use rusqlite::Transaction;
    use std::collections::BTreeMap;
    use studio_json::OwnedRawJson as RawValue;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        let mut query =
            tx.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record)")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut changed = false;
        for row in rows {
            let (id, raw) = row?;
            let mut record: BTreeMap<String, RawValue> = studio_json::from_str(&raw)?;
            let old_max = record.remove("maxAgents");
            let old_explicit = record.remove("maxAgentsExplicit");
            if old_max.is_none()
                && old_explicit.is_none()
                && record.get("capacity").is_some_and(|value| {
                    studio_json::from_str::<crate::AgentCapacity>(value.as_str()).is_ok()
                })
            {
                continue;
            }
            let max = old_max.as_ref().and_then(|raw| {
                studio_json::from_str::<studio_runtime_schema::AgentCounter>(raw.as_str()).ok()
            });
            let explicit = old_explicit
                .as_ref()
                .and_then(|raw| studio_json::from_str::<bool>(raw.as_str()).ok())
                .unwrap_or_else(|| {
                    old_explicit.is_none()
                        && old_max
                            .as_ref()
                            .and_then(|raw| studio_json::from_str::<i64>(raw.as_str()).ok())
                            .unwrap_or(studio_runtime_schema::agent_limits::DEFAULT_MAX_TEAM_AGENTS)
                            != studio_runtime_schema::agent_limits::DEFAULT_MAX_TEAM_AGENTS
                });
            let kind = if explicit { "explicit" } else { "automatic" };
            let limit = max.map_or(
                studio_json::json!(studio_runtime_schema::agent_limits::DEFAULT_MAX_TEAM_AGENTS),
                |value| studio_json::json!(value),
            );
            record.insert(
                "capacity".into(),
                RawValue::from_string(studio_json::json!({"kind":kind,"max":limit}).to_string())?,
            );
            tx.execute(
                "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                rusqlite::params![studio_json::to_string(&record)?, id],
            )?;
            changed = true;
        }
        if changed && crate::schema::table_exists(tx, "sync_entity_meta")? {
            tx.execute(
                "DELETE FROM sync_entity_meta WHERE key=?1",
                [crate::AGENT_PROJECTIONS_SEED_MARKER],
            )?;
        }
        Ok(())
    }
}
mod agent_naming {
    use crate::StoreResult;
    use rusqlite::Transaction;
    use std::collections::BTreeMap;
    use studio_json::OwnedRawJson as RawValue;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        let mut query =
            tx.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record)")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut changed = false;
        for row in rows {
            let (id, raw) = row?;
            let mut record: BTreeMap<String, RawValue> = studio_json::from_str(&raw)?;
            let manual = record.remove("manualName");
            let title = record.remove("needsTitle");
            if manual.is_none()
                && title.is_none()
                && record.get("naming").is_some_and(|value| {
                    studio_json::from_str::<crate::AgentNaming>(value.as_str()).is_ok()
                })
            {
                continue;
            }
            let is_true = |v: Option<&RawValue>| v.is_some_and(|v| v.as_str() == "true");
            let naming = if is_true(manual.as_ref()) {
                "manual"
            } else if is_true(title.as_ref()) {
                "pending_title"
            } else {
                "automatic"
            };
            record.insert(
                "naming".into(),
                RawValue::from_string(studio_json::to_string(naming)?)?,
            );
            tx.execute(
                "UPDATE runtime_agents SET record=?1 WHERE id=?2",
                rusqlite::params![studio_json::to_string(&record)?, id],
            )?;
            changed = true;
        }
        if changed && crate::schema::table_exists(tx, "sync_entity_meta")? {
            tx.execute(
                "DELETE FROM sync_entity_meta WHERE key=?1",
                [crate::AGENT_PROJECTIONS_SEED_MARKER],
            )?;
        }
        Ok(())
    }
}
mod agent_creation {
    use crate::StoreResult;
    use rusqlite::Transaction;

    pub(super) const OPTIONAL_FIELDS: &[&str] = &[
        "worktree",
        "workerBaseRef",
        "workspaceModeExplicit",
        "workspaceInGit",
        "quickCreate",
        "reviewDefaults",
        "worktreeWarning",
        "workerBaseCommit",
        "workerBaseBehindMain",
        "workerBaseMainRef",
        "quickCreateRequest",
    ];

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        let paths: Vec<_> = OPTIONAL_FIELDS
            .iter()
            .copied()
            .chain(["reviewDefaults.effort"])
            .collect();
        let checks = paths.iter().map(|path| {
        if *path == "reviewDefaults" {
            "(json_type(record, '$.reviewDefaults') = 'null' OR (json_type(record, '$.reviewDefaults') = 'object' AND NOT EXISTS (SELECT 1 FROM json_each(record, '$.reviewDefaults'))))".to_owned()
        } else {
            format!("json_type(record, '$.{path}') = 'null'")
        }
    });
        let removals = checks
            .clone()
            .zip(paths.iter())
            .map(|(check, path)| format!("CASE WHEN {check} THEN '$.{path}' ELSE '$[#-1]' END"));
        tx.execute_batch(&format!(
        "UPDATE runtime_agents SET record = json_remove(record, {}) WHERE json_valid(record) AND ({})",
        removals.collect::<Vec<_>>().join(", "), checks.collect::<Vec<_>>().join(" OR ")
    ))?;
        Ok(())
    }
}
mod approval {
    //! Copies every `runtime_requests` row into the typed `approvals` table in one
    //! transaction. The legacy table stays untouched and read-only (nothing is
    //! deleted). The converter places every legacy field in a typed column or
    //! struct field. A field it cannot place fails the migration (the legacy record
    //! type denies unknown fields). A row that never recorded its answer becomes
    //! `Closed { reason: Legacy }`: nothing is inferred.
    use serde::Deserialize;
    use studio_json::{Map, Value};
    use studio_runtime_schema::approvals::{
        AgentId, Answer, Approval, ApprovalId, ApprovalState, BrowserCallbackForward, BrowserCard,
        BrowserOutcome, BrowserPhase, BrowserRedirect, BrowserSource, CloseReason, CommandId,
        Decision, Deferral, Delivery, Detail, Hidden, Principal, ProviderObject, Question,
        QuestionAnswer, Request, RequestKind, RpcId, Supervisor, Timestamp, TurnId,
    };

    use crate::approvals;
    use crate::{StoreError, StoreResult};

    fn bad(id: &str, what: &str) -> StoreError {
        StoreError::SchemaMismatch(format!("approvals migration: request {id}: {what}"))
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum LegacyRpcId {
        Number(i64),
        Text(String),
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LegacySupervisor {
        generation: u64,
        state_dir: Option<String>,
        handle: Option<String>,
    }

    /// The answer of one question. A secret question kept only the text "[redacted]".
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum LegacyLeaf {
        Values(Vec<String>),
        Redacted(String),
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyHistory {
        id: String,
        #[allow(dead_code)]
        question: Option<String>,
        #[serde(rename = "isSecret")]
        is_secret: Option<bool>,
        answer: LegacyLeaf,
    }

    /// Every key a `runtime_requests` record has. An unknown key fails the migration.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Legacy {
        #[allow(dead_code)]
        id: String,
        agent: Option<String>,
        method: String,
        params: Option<Map<String, Value>>,
        status: String,
        created_at: Option<f64>,
        turn_id: Option<String>,
        account_key: Option<String>,
        connection_id: Option<String>,
        epoch: Option<u64>,
        supervisor: Option<LegacySupervisor>,
        rpc_id: Option<LegacyRpcId>,
        preview: Option<Map<String, Value>>,
        decision: Option<String>,
        /// Monitor approvals write the decision twice. It must equal `decision`.
        answer_decision: Option<String>,
        answered_by: Option<String>,
        answered_at: Option<f64>,
        /// A hash of the answer. Replay is now decided by comparing answers.
        #[allow(dead_code)]
        answer_signature: Option<String>,
        answer_history: Option<Vec<LegacyHistory>>,
        answer_error: Option<String>,
        deferred: Option<bool>,
        deferred_at: Option<f64>,
        deferred_by: Option<String>,
        deleted_at: Option<f64>,
        deleted_by: Option<String>,
        close_reason: Option<String>,
        resolved_at: Option<f64>,
    }

    fn principal(id: &str, who: &str, close_reason: Option<&str>) -> StoreResult<Principal> {
        match (who, close_reason) {
            ("user", _) => Ok(Principal::Owner),
            // The launcher of this device ended a browser sign-in: the owner's action.
            ("browser", Some("local")) => Ok(Principal::Owner),
            (other, _) => Err(bad(id, &format!("unknown actor {other}"))),
        }
    }

    fn questions(id: &str, params: &mut Map<String, Value>) -> StoreResult<Vec<Question>> {
        let Some(list) = params.remove("questions") else {
            return Ok(Vec::new());
        };
        approvals::questions_from_provider(list, true)
            .map_err(|e| bad(id, &format!("questions: {e}")))
    }

    fn take_text(params: &mut Map<String, Value>, key: &str) -> Option<String> {
        match params.remove(key) {
            Some(Value::String(text)) => Some(text),
            Some(other) => {
                params.insert(key.to_owned(), other);
                None
            }
            None => None,
        }
    }

    /// A string, null or absent value. Any other type cannot be placed.
    fn nullable_text(
        id: &str,
        params: &mut Map<String, Value>,
        key: &str,
    ) -> StoreResult<Option<String>> {
        match params.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(_) => Err(bad(id, &format!("browser {key} is not text"))),
        }
    }

    fn nullable_time(
        id: &str,
        params: &mut Map<String, Value>,
        key: &str,
    ) -> StoreResult<Option<Timestamp>> {
        match params.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(number)) => Ok(number.as_f64().map(Timestamp)),
            Some(_) => Err(bad(id, &format!("browser {key} is not a number"))),
        }
    }

    fn required_text(id: &str, params: &mut Map<String, Value>, key: &str) -> StoreResult<String> {
        nullable_text(id, params, key)?.ok_or_else(|| bad(id, &format!("browser {key} is missing")))
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LegacyWaiting {
        expires_at: f64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyPaste {
        at: f64,
    }

    fn browser_outcome(id: &str, text: &str) -> StoreResult<BrowserOutcome> {
        Ok(match text {
            "signed-in" => BrowserOutcome::SignedIn,
            "refused" => BrowserOutcome::Refused,
            "provider-error" => BrowserOutcome::ProviderError,
            "unknown" => BrowserOutcome::Unknown,
            "expired" => BrowserOutcome::Expired,
            "cancelled" => BrowserOutcome::Cancelled,
            other => return Err(bad(id, &format!("unknown browser outcome {other}"))),
        })
    }

    /// Places the `browser/open` params. The phase, paste mark and outcome of the old
    /// state machine become one `BrowserPhase`. A card written before the host bridge
    /// existed has no source: the VM guest was the only one.
    fn browser_card(id: &str, params: &mut Map<String, Value>) -> StoreResult<BrowserCard> {
        let phase = nullable_text(id, params, "phase")?;
        let waiting = params.remove("waiting");
        let paste = params.remove("paste").filter(|value| !value.is_null());
        let outcome = nullable_text(id, params, "outcome")?;
        let http = params.remove("outcomeHttp");
        let waiting: Option<LegacyWaiting> = waiting
            .filter(|value| !value.is_null())
            .map(serde::Deserialize::deserialize)
            .transpose()
            .map_err(|e| bad(id, &format!("browser waiting: {e}")))?;
        let paste: Option<LegacyPaste> = paste
            .map(serde::Deserialize::deserialize)
            .transpose()
            .map_err(|e| bad(id, &format!("browser paste: {e}")))?;
        let phase = match (phase.as_deref(), outcome, waiting, paste) {
            (_, Some(outcome), ..) => BrowserPhase::Finished {
                outcome: browser_outcome(id, &outcome)?,
                http: match http {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(
                        serde::Deserialize::deserialize(value)
                            .map_err(|e| bad(id, &format!("browser outcomeHttp: {e}")))?,
                    ),
                },
            },
            (None, None, None, None) => BrowserPhase::Link,
            (Some("waiting"), None, Some(waiting), None) => BrowserPhase::Waiting {
                expires_at: Timestamp(waiting.expires_at),
            },
            (Some("waiting"), None, Some(waiting), Some(paste)) => BrowserPhase::Pasted {
                expires_at: Timestamp(waiting.expires_at),
                at: Timestamp(paste.at),
            },
            _ => return Err(bad(id, "browser phase cannot be placed")),
        };
        let redirect = match params.remove("redirect") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                BrowserRedirect::deserialize(value)
                    .map_err(|e| bad(id, &format!("browser redirect: {e}")))?,
            ),
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct LegacyForward {
            token_hash: String,
            expires_at: f64,
        }
        let forward = match params.remove("forward") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let legacy: LegacyForward = serde::Deserialize::deserialize(value)
                    .map_err(|e| bad(id, &format!("browser forward: {e}")))?;
                Some(BrowserCallbackForward {
                    token_hash: legacy.token_hash,
                    expires_at: Timestamp(legacy.expires_at),
                })
            }
        };
        let joined = match params.remove("joined") {
            None | Some(Value::Null) => Vec::new(),
            Some(value) => Vec::<String>::deserialize(value)
                .map_err(|e| bad(id, &format!("browser joined: {e}")))?
                .into_iter()
                .map(AgentId)
                .collect(),
        };
        let source = match nullable_text(id, params, "source")?.as_deref() {
            None | Some("vm") => BrowserSource::Vm,
            Some("host") => BrowserSource::Host,
            Some(other) => return Err(bad(id, &format!("unknown browser source {other}"))),
        };
        Ok(BrowserCard {
            url: required_text(id, params, "url")?,
            bridge_id: required_text(id, params, "bridgeId")?,
            host: required_text(id, params, "host")?,
            source,
            redirect,
            client_id: nullable_text(id, params, "clientId")?,
            scope: nullable_text(id, params, "scope")?,
            linux_user: nullable_text(id, params, "linuxUser")?,
            user_code: nullable_text(id, params, "userCode")?,
            expires_at: nullable_time(id, params, "expiresAt")?,
            joined,
            forward,
            phase,
        })
    }

    /// Moves the typed fields out of `params`; the rest stays in `Detail.params`.
    fn request(id: &str, legacy: &Legacy, params: &mut Map<String, Value>) -> StoreResult<Request> {
        let preview_text = |key: &str| {
            legacy
                .preview
                .as_ref()
                .and_then(|p| p.get(key))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Ok(match legacy.method.as_str() {
            "item/commandExecution/requestApproval" => Request::Exec {
                command: take_text(params, "command")
                    .or_else(|| preview_text("command"))
                    .unwrap_or_default(),
                cwd: take_text(params, "cwd")
                    .or_else(|| preview_text("cwd"))
                    .unwrap_or_default(),
                reason: take_text(params, "reason"),
            },
            "item/fileChange/requestApproval" => Request::Patch {
                item: take_text(params, "itemId").unwrap_or_default(),
                files: legacy
                    .preview
                    .as_ref()
                    .and_then(|p| p.get("changes"))
                    .and_then(Value::as_array)
                    .map(|changes| {
                        changes
                            .iter()
                            .filter_map(|c| {
                                c.get("path").and_then(Value::as_str).map(str::to_owned)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                reason: take_text(params, "reason"),
            },
            "item/permissions/requestApproval" => Request::Permissions {
                requested: params
                    .get("permissions")
                    .and_then(Value::as_object)
                    .map(|map| map.keys().cloned().collect())
                    .unwrap_or_default(),
            },
            "mcpServer/elicitation/request" => Request::Elicitation {
                schema: params.remove("requestedSchema").unwrap_or(Value::Null),
            },
            "item/tool/requestUserInput" => Request::Question {
                questions: questions(id, params)?,
            },
            "agent/asyncQuestion" => Request::AgentQuestion {
                questions: questions(id, params)?,
            },
            "monitor/approve" => Request::MonitorStart {
                monitor: take_text(params, "monitorId").unwrap_or_default(),
                command: take_text(params, "command").unwrap_or_default(),
            },
            "browser/open" => Request::BrowserOpen(browser_card(id, params)?),
            other => return Err(bad(id, &format!("unmapped method {other}"))),
        })
    }

    fn answer(id: &str, legacy: &Legacy, request: &Request) -> StoreResult<Option<Answer>> {
        let Some(decision) = legacy.decision.as_deref() else {
            return Ok(None);
        };
        // A pasted sign-in result was recorded as "answer". The consent was an accept;
        // the paste and its outcome live in the card's phase.
        if legacy.method == "browser/open" && decision == "answer" {
            return Ok(Some(Answer::Decision {
                decision: Decision::Accept,
            }));
        }
        Ok(Some(match decision {
            "accept" => Answer::Decision {
                decision: Decision::Accept,
            },
            "decline" => Answer::Decision {
                decision: Decision::Decline,
            },
            "cancel" => Answer::Decision {
                decision: Decision::Cancel,
            },
            "answer" => {
                let history = legacy
                    .answer_history
                    .as_ref()
                    .filter(|history| !history.is_empty())
                    .ok_or_else(|| bad(id, "answer decision without answer history"))?;
                Answer::Answers {
                    answers: history
                        .iter()
                        .map(|entry| {
                            let question_secret = match request {
                                Request::Question { questions }
                                | Request::AgentQuestion { questions } => questions
                                    .iter()
                                    .any(|question| question.id == entry.id && question.is_secret),
                                Request::Exec { .. }
                                | Request::Patch { .. }
                                | Request::Permissions { .. }
                                | Request::Elicitation { .. }
                                | Request::ToolUse { .. }
                                | Request::MonitorStart { .. }
                                | Request::BrowserOpen(_) => false,
                            };
                            let (answer, digest) = match &entry.answer {
                                LegacyLeaf::Values(values)
                                    if entry.is_secret == Some(true) || question_secret =>
                                {
                                    (
                                        vec![QuestionAnswer::REDACTED.to_owned()],
                                        Some(crate::sha256_hex(values.join("\u{0}").as_bytes())),
                                    )
                                }
                                LegacyLeaf::Values(values) => (values.clone(), None),
                                LegacyLeaf::Redacted(text) if text == QuestionAnswer::REDACTED => {
                                    (vec![text.clone()], None)
                                }
                                LegacyLeaf::Redacted(_) => {
                                    return Err(bad(id, "answer text is not a list"));
                                }
                            };
                            Ok(QuestionAnswer {
                                id: entry.id.clone(),
                                answer,
                                digest,
                            })
                        })
                        .collect::<StoreResult<Vec<_>>>()?,
                }
            }
            other => return Err(bad(id, &format!("unknown decision {other}"))),
        }))
    }

    fn convert(id: &str, legacy: Legacy) -> StoreResult<Approval> {
        let mut params = legacy.params.clone().unwrap_or_default();
        let request = request(id, &legacy, &mut params)?;
        let created = legacy.created_at.unwrap_or(0.0);
        let when =
            |keys: &[Option<f64>]| Timestamp(keys.iter().find_map(|k| *k).unwrap_or(created));
        if legacy.answer_decision.is_some() && legacy.answer_decision != legacy.decision {
            return Err(bad(id, "answerDecision differs from decision"));
        }
        let given = answer(id, &legacy, &request)?;
        if let Some(answer) = &given {
            request
                .accepts(answer)
                .map_err(|e| bad(id, &format!("recorded answer does not fit: {e}")))?;
        }
        let by = legacy
            .answered_by
            .as_deref()
            .map(|who| principal(id, who, legacy.close_reason.as_deref()))
            .transpose()?;
        let state = match (legacy.status.as_str(), given, by) {
            ("pending", ..) => ApprovalState::Pending,
            ("answering", Some(answer), _) => ApprovalState::Answering {
                answer,
                command: CommandId(id.to_owned()),
            },
            ("answered" | "deleted", Some(answer), Some(by)) => ApprovalState::Answered {
                answer,
                by,
                at: when(&[legacy.answered_at]),
            },
            ("uncertain", Some(answer), _) => ApprovalState::Unknown {
                answer,
                since: when(&[legacy.answered_at]),
            },
            ("expired", ..) => ApprovalState::Closed {
                reason: CloseReason::Expired,
                at: when(&[legacy.resolved_at, legacy.deferred_at, legacy.deleted_at]),
            },
            ("resolved", ..) => ApprovalState::Closed {
                reason: CloseReason::ProviderResolved,
                at: when(&[legacy.resolved_at, legacy.answered_at]),
            },
            ("answering" | "answered" | "deleted" | "uncertain", ..) => ApprovalState::Closed {
                reason: CloseReason::Legacy,
                at: when(&[legacy.resolved_at, legacy.answered_at, legacy.deleted_at]),
            },
            (other, ..) => return Err(bad(id, &format!("unknown status {other}"))),
        };
        // Live data: only agent-level kinds (questions, monitor starts, browser
        // opens) have rows without a turn. A provider-turn kind without one is
        // corrupt and fails the migration.
        let params_turn = take_text(&mut params, "turnId");
        if let (Some(a), Some(b)) = (&legacy.turn_id, &params_turn)
            && a != b
        {
            return Err(bad(id, "turnId differs between record and params"));
        }
        let turn = legacy.turn_id.clone().or(params_turn).map(TurnId);
        if turn.is_none() && !RequestKind::TURN_OPTIONAL.contains(&RequestKind::from(&request)) {
            return Err(bad(id, "a provider-turn request has no turn"));
        }
        let delivery = match (legacy.rpc_id, legacy.account_key, legacy.connection_id) {
            (Some(rpc), Some(account), Some(connection)) => Some(Delivery {
                rpc_id: match rpc {
                    LegacyRpcId::Number(value) => RpcId::Number { value },
                    LegacyRpcId::Text(value) => RpcId::Text { value },
                },
                account,
                connection,
                supervisor: legacy.supervisor.map(|s| Supervisor {
                    generation: s.generation,
                    state_dir: s.state_dir,
                    handle: s.handle,
                }),
            }),
            (None, None, None) if legacy.supervisor.is_none() => None,
            _ => return Err(bad(id, "incomplete delivery fields")),
        };
        let deferral = match (legacy.deferred_at, legacy.deferred_by.as_deref()) {
            (Some(at), Some(by)) => Some(Deferral {
                active: legacy.deferred.unwrap_or(false),
                at: Timestamp(at),
                by: principal(id, by, None)?,
            }),
            (None, None) if legacy.deferred != Some(true) => None,
            _ => return Err(bad(id, "incomplete deferral fields")),
        };
        let hidden = match (legacy.deleted_at, legacy.deleted_by.as_deref()) {
            (Some(at), Some(by)) => Some(Hidden {
                at: Timestamp(at),
                by: principal(id, by, None)?,
            }),
            (None, None) if legacy.status != "deleted" => None,
            _ => return Err(bad(id, "incomplete deletion fields")),
        };
        Ok(Approval {
            id: ApprovalId(id.to_owned()),
            agent: AgentId(legacy.agent.unwrap_or_default()),
            turn,
            created: Timestamp(created),
            epoch: legacy.epoch,
            request,
            delivery,
            detail: Detail {
                params: ProviderObject(params),
                preview: legacy.preview.map(ProviderObject),
                answer_error: legacy.answer_error,
                host_close_reason: legacy.close_reason,
            },
            state,
            deferral,
            fire_at: None,
            hidden,
        })
    }

    /// Convert one legacy record. The legacy question-history migration and test fixtures use it.
    pub fn approval_from_record(id: &str, record: &str) -> StoreResult<Approval> {
        convert(id, legacy_of(id, record)?)
    }

    /// Reads a stored record through the request record type of the runtime schema
    /// (strict: an unknown key or a bad value fails), then into the converter's view.
    fn legacy_of(id: &str, record: &str) -> StoreResult<Legacy> {
        let placed = |error: studio_json::Error| bad(id, &format!("cannot place a field: {error}"));
        let mut value: Value = studio_json::from_str(record).map_err(placed)?;
        // The state directory and handle of the supervised process are stored next to
        // the generation. The request record type has the generation only, so these two
        // are set aside here and put back in the converter's view.
        let process = value
            .get_mut("supervisor")
            .and_then(Value::as_object_mut)
            .map(|supervisor| (supervisor.remove("stateDir"), supervisor.remove("handle")));
        let typed: studio_runtime_schema::request_record::RequestRecord =
            serde::Deserialize::deserialize(value)
                .map_err(|error: studio_json::error::Error| placed(error.into()))?;
        let mut view = crate::projection_value(&typed);
        if let (Some((state_dir, handle)), Some(supervisor)) = (
            process,
            view.get_mut("supervisor").and_then(Value::as_object_mut),
        ) {
            for (key, member) in [("stateDir", state_dir), ("handle", handle)] {
                if let Some(member) = member {
                    supervisor.insert(key.to_owned(), member);
                }
            }
        }
        Legacy::deserialize(view).map_err(|error| placed(error.into()))
    }
}
pub fn normalize_agent(tx: &rusqlite::Transaction<'_>) -> crate::StoreResult<()> {
    agent_core::apply(tx)?;
    agent_batch::apply(tx)?;
    agent_capacity::apply(tx)?;
    agent_naming::apply(tx)?;
    agent_creation::apply(tx)
}
pub fn normalize_creation(tx: &rusqlite::Transaction<'_>) -> crate::StoreResult<()> {
    agent_creation::apply(tx)
}
pub use approval::approval_from_record;

mod radio_room {
    use crate::StoreResult;
    use rusqlite::Transaction;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        if !crate::schema::table_exists(tx, "runtime_rooms")? {
            return Ok(());
        }
        // JSON presence predicates distinguish missing fields from explicit null.
        tx.execute("UPDATE runtime_rooms SET record=json_set(record,'$.radio.direct',json('false')) WHERE json_valid(record) AND json_type(record,'$.radio')='object' AND (json_type(record,'$.radio.direct') IS NULL OR json_type(record,'$.radio.direct')='null')", [])?;
        tx.execute("UPDATE runtime_rooms SET record=json_set(record,'$.radio.active',NULL) WHERE json_valid(record) AND json_type(record,'$.radio')='object' AND json_type(record,'$.radio.active') IS NULL", [])?;
        Ok(())
    }
}
pub use radio_room::apply as radio_room;

mod room_record {
    use crate::{StoreError, StoreResult};
    use rusqlite::{Transaction, params_from_iter, types::Value as SqlValue};
    use studio_runtime_schema::room_record::{FIELDS, RoomRecord};

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        if !crate::schema::table_exists(tx, "runtime_rooms")? {
            return Ok(());
        }
        // Legacy JSON normalization requires JSON functions and must precede typed decoding.
        let mut statement = tx.prepare("SELECT id,record FROM runtime_rooms ORDER BY id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let placeholders = (2..FIELDS.len() + 2)
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT json_group_object(key, CASE type WHEN 'object' THEN json(value) WHEN 'array' THEN json(value) WHEN 'true' THEN json('true') WHEN 'false' THEN json('false') ELSE value END) FROM (SELECT key,type,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1)) WHERE n=1 AND key IN ({placeholders})"
        );
        let mut normalize = tx.prepare(&query)?;
        let mut normalize_nested = tx.prepare("SELECT CASE WHEN json_type(?1,?2)='object' THEN json_set(?1,?2,json((SELECT json_group_object(key,CASE type WHEN 'object' THEN json(value) WHEN 'array' THEN json(value) WHEN 'true' THEN json('true') WHEN 'false' THEN json('false') ELSE value END) FROM (SELECT key,type,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1,?2)) WHERE n=1))) ELSE ?1 END")?;
        let mut normalize_cursors = tx.prepare("SELECT CASE WHEN json_type(?1,'$.radio.seen')='object' THEN json_set(?1,'$.radio.seen',json((SELECT json_group_object(key,json_object('identity',json(CASE WHEN json_type(value,'$.identity[2]')='null' THEN json_set(json_extract(value,'$.identity'),'$[2]',0) ELSE json_extract(value,'$.identity') END),'seq',json_extract(value,'$.seq'))) FROM (SELECT key,value,row_number() OVER (PARTITION BY key ORDER BY id DESC) AS n FROM json_each(?1,'$.radio.seen')) WHERE n=1))) ELSE ?1 END")?;
        let mut normalize_active_identity = tx.prepare("SELECT CASE WHEN json_type(?1,'$.radio.active.identity[2]')='null' THEN json_set(?1,'$.radio.active.identity[2]',0) ELSE ?1 END")?;
        for (id, raw) in rows {
            let mut args = vec![SqlValue::Text(raw.clone())];
            args.extend(
                FIELDS
                    .iter()
                    .map(|field| SqlValue::Text((*field).to_owned())),
            );
            let normalized: String =
                normalize.query_row(params_from_iter(args), |row| row.get(0))?;
            let normalized: String =
                normalize_nested.query_row((&normalized, "$.radio"), |row| row.get(0))?;
            let normalized: String =
                normalize_nested.query_row((&normalized, "$.radio.active"), |row| row.get(0))?;
            let normalized: String =
                normalize_active_identity.query_row([normalized], |row| row.get(0))?;
            let normalized: String = normalize_cursors.query_row([normalized], |row| row.get(0))?;
            let room: RoomRecord = studio_json::from_str(&normalized).map_err(|_| {
                StoreError::InvalidStoredRecord {
                    object: "runtime_rooms",
                    id: id.clone(),
                }
            })?;
            if room.id != id {
                return Err(StoreError::InvalidStoredRecord {
                    object: "runtime_rooms",
                    id,
                });
            }
            if normalized != raw {
                tx.execute(
                    "UPDATE runtime_rooms SET record=?2 WHERE id=?1",
                    (&id, &normalized),
                )?;
            }
        }
        Ok(())
    }
}
pub use room_record::apply as room_record;

mod project_teams {
    use crate::StoreResult;
    use rusqlite::Transaction;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        if !crate::schema::table_exists(tx, "runtime_projects")? {
            return Ok(());
        }
        tx.execute("UPDATE runtime_projects SET record=json_set(record,'$.peerTeams',json('[]')) WHERE json_valid(record) AND json_type(record)='object' AND json_type(record,'$.peerTeams') IS NULL", [])?;
        Ok(())
    }
}
pub use project_teams::apply as project_teams;

mod project_record_kind {
    use crate::StoreResult;
    use rusqlite::Transaction;
    use studio_runtime_schema::project_record::ProjectRecord;

    pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
        if !crate::schema::table_exists(tx, "runtime_projects")? {
            return Ok(());
        }
        let mut statement = tx.prepare("SELECT id,json_set(record,'$.recordKind',CASE WHEN json_type(record,'$.removal')='object' THEN 'removal' ELSE 'live' END) FROM runtime_projects WHERE typeof(id)='text' AND typeof(record)='text' AND CASE WHEN json_valid(record) THEN json_type(record)='object' AND json_type(record,'$.recordKind') IS NULL AND (json_type(record,'$.removal')='object' OR (json_type(record,'$.removal') IS NULL AND json_type(record,'$.path')='text')) ELSE 0 END")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (id, encoded) in rows {
            let matches = match studio_json::from_str::<ProjectRecord>(&encoded) {
                Ok(ProjectRecord::Live(project)) => project.id == id,
                Ok(ProjectRecord::Removal(project)) => project.id == id,
                Err(_) => false,
            };
            if matches {
                tx.execute(
                    "UPDATE runtime_projects SET record=?2 WHERE id=?1",
                    (&id, &encoded),
                )?;
            }
        }
        Ok(())
    }
}
pub use project_record_kind::apply as project_record_kind;
