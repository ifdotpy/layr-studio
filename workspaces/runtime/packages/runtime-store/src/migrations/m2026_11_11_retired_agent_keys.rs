//! Fixed retirement of root fields whose production writers and readers are gone.
use crate::{StoreError, StoreResult, Transaction};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use studio_json::OwnedRawJson;

/// Deletion ledger. Each entry is a retired root key, never a nested field.
pub(crate) const RETIRED: &[&str] = &[
    "needsTitle",              // replaced by naming/titleJob
    "manualName",              // replaced by naming
    "maxAgents",               // replaced by capacity
    "maxAgentsExplicit",       // replaced by capacity kind
    "tokenBudget",             // retired budget feature
    "compactionsObservedOnly", // retired observation marker
    "depth",                   // historical radio fixture; ancestry is derived
    "history",                 // historical agent-root history; transcript has its own owner
    "serverId",                // published server identity comes from runtime context
    "imageWorkspaceBaseRepo",  // removed legacy base-repository reader
    "imageWorkspaceBaseRef",   // removed image executor metadata
    "imageWorkspaceHasGit",
    "imageWorkspaceSubpath",
    "imageWorkspaceCreatedAt",
    "imageWorkspaceError",
    "workspaceBackend",
    "reviewSchedules", // retired agent cleanup, now owned by this migration
    "startRecoveryCheck",
    "complaintsPresented",
    "checkpointError",
    "complaintMisses",
    "completedWorkerTurns",
    "supervisorRestore",
    "turnRecovery",
    "lastContextRepairWait",
    "lastContextRepairCheck",
    "countedWorkerTurn",
    "imageWorkspaceBaseError",
    "imageWorkspaceNoticeSent",
    "imageWorkspaceNoticeText",
    "imageWorkspaceRelative",
    "imageWorkspaceStartCommit",
];

// Source-only owner: raw member tokens survive without becoming application data.
struct Source(BTreeMap<String, OwnedRawJson>);
impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> Visitor<'de> for Members {
            type Value = Source;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an agent root object with unique members")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Source, M::Error> {
                let mut members = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, OwnedRawJson>()? {
                    if members.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate root member {key}"
                        )));
                    }
                }
                Ok(Source(members))
            }
        }
        de.deserialize_map(Members)
    }
}

pub(crate) fn destination(id: &str, raw: &str) -> StoreResult<Option<String>> {
    let invalid = |error| {
        StoreError::SchemaValidation(format!(
            "invalid stored object runtime_agents/{id} during retired-root-key removal: {error}"
        ))
    };
    let Source(mut members) = studio_json::from_str::<Source>(raw).map_err(invalid)?;
    let mut changed = false;
    for key in RETIRED {
        changed |= members.remove(*key).is_some();
    }
    if !changed {
        return Ok(None);
    }
    studio_json::to_string(&members).map(Some).map_err(invalid)
}

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let predicate = RETIRED
        .iter()
        .map(|key| format!("json_type(record,'$.{key}') IS NOT NULL"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let sql = format!(
        "SELECT rowid,id,record FROM runtime_agents WHERE json_valid(record) AND ({predicate}) ORDER BY rowid"
    );
    let rows = tx
        .prepare(&sql)?
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Admit every replacement before the first write; any refusal rolls back the migration.
    let replacements = rows
        .into_iter()
        .map(|(rowid, id, raw)| {
            let identity = id.unwrap_or_else(|| format!("<NULL>@{rowid}"));
            let replacement = destination(&identity, &raw)?;
            Ok((rowid, identity, raw, replacement))
        })
        .collect::<StoreResult<Vec<_>>>()?;
    for (rowid, identity, original, replacement) in replacements {
        if let Some(raw) = replacement {
            let changed = tx.execute(
                "UPDATE runtime_agents SET record=?2 WHERE rowid=?1 AND record=?3",
                rusqlite::params![rowid, raw, original],
            )?;
            if changed != 1 {
                return Err(StoreError::SchemaValidation(format!(
                    "stored object runtime_agents/{identity} changed during retired-root-key removal"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn retired_agent_root_keys_are_fixed_idempotent_and_preserve_member_tokens() {
        let raw = r#"{"id":"retired","rootId":"retired","epoch":0,"status":"idle","provider":"codex","accountKey":"default","name":"kept","error":{"message":"kept","number":1.00e+3},"needsTitle":false,"manualName":null,"history":["old"],"updated":1}"#;
        let next = super::destination("retired", raw).unwrap().unwrap();
        assert!(next.contains("1.00e+3"));
        assert!(next.contains("\"name\":\"kept\""));
        assert!(super::destination("retired", &next).unwrap().is_none());
        let record: crate::AgentRecord = studio_json::from_str(&next).unwrap();
        assert_eq!(record.name.as_deref(), Some("kept"));
        for key in super::RETIRED {
            let with_key = format!("{{\"id\":\"retired\",\"{key}\":null}}");
            assert_eq!(
                super::destination("retired", &with_key).unwrap().as_deref(),
                Some(r#"{"id":"retired"}"#),
                "{key}"
            );
        }
    }

    #[test]
    #[ignore = "manual measurement on an isolated fresh live copy"]
    fn retired_agent_keys_fresh_copy_runtime() {
        let path = std::env::var_os("F2_RETIRED_KEYS_MEASUREMENT_DB")
            .expect("isolated writable copy path");
        let mut db = crate::Connection::open(path).unwrap();
        let tx = db.transaction().unwrap();
        let started = std::time::Instant::now();
        super::apply(&tx).unwrap();
        let first = started.elapsed();
        let second = std::time::Instant::now();
        super::apply(&tx).unwrap();
        eprintln!(
            "retired-agent-keys first_ms={} idempotent_ms={}",
            first.as_millis(),
            second.elapsed().as_millis()
        );
        tx.rollback().unwrap();
    }

    #[test]
    fn retired_agent_root_keys_refuse_duplicate_members_with_identity() {
        let error = super::destination(
            "duplicate-agent",
            r#"{"needsTitle":true,"name":"one","name":"two"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("duplicate-agent") && error.contains("name"),
            "{error}"
        );
    }
}
