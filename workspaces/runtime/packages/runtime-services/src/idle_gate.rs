//! Shared idle admission for changes to a chat workspace.
use crate::event_write::truthy;
use crate::projects::ProjectsError;
use rusqlite::Connection;
use std::collections::BTreeSet;
use studio_json::Value;
use studio_runtime_store::system_reads as records;

fn transfer_involves(transfer: &studio_runtime_store::AccountTransferRecord, agent: &str) -> bool {
    transfer.lead_id.as_str() == agent || transfer.members.0.iter().any(|(id, _)| id == agent)
}
/// Shared idle gate for user actions that change a chat's owner or workspace.
/// `uncertain_blocks` is true for a move to another owner, which must carry an
/// unknown outcome (input or tool request); a switch in place keeps the thread
/// and those records as they are.
pub(crate) fn idle(
    db: &Connection,
    agents: &[Value],
    uncertain_blocks: bool,
) -> Result<(), ProjectsError> {
    let ids = agents
        .iter()
        .filter_map(|a| a["id"].as_str())
        .collect::<BTreeSet<_>>();
    for a in agents {
        if truthy(a.get("inFlight"))
            || truthy(a.get("turnId"))
            || matches!(
                a["status"].as_str(),
                Some("running" | "starting" | "approval")
            )
            || (a["status"] == "queued" && truthy(a.get("autoWake")))
            || truthy(a.get("startAttempt"))
        {
            return Err(ProjectsError::Invalid(
                "Finish the active or queued turn first",
            ));
        }
        if truthy(a.get("workspaceOperation")) {
            return Err(ProjectsError::Invalid(
                "Finish the pending workspace operation first",
            ));
        }
        let agent = studio_runtime_store::AgentRepository::get(
            db,
            &studio_runtime_store::AgentId::from(a["id"].as_str().unwrap_or_default()),
        )?
        .ok_or(ProjectsError::Invalid("stored agent missing"))?;
        if agent.account_transfer_id.value().is_some()
            || agent.lazy_account_transfer.value().is_some()
            || matches!(
                agent.account_transfer.value(),
                Some(
                    studio_runtime_store::StoredTransferObject::Unreadable(_)
                        | studio_runtime_store::StoredTransferObject::Known(
                            studio_runtime_store::transfer_record::AgentTransferState::Unreadable(
                                _
                            )
                        )
                )
            )
            || matches!(agent.account_transfer.value(), Some(studio_runtime_store::StoredTransferObject::Known(studio_runtime_store::transfer_record::AgentTransferState::Summary(summary))) if matches!(summary.status, studio_runtime_store::transfer_record::TransferStatus::Pending))
        {
            return Err(ProjectsError::Invalid("Finish the account transfer first"));
        }
        if matches!(
            a["nativeRelease"]["phase"].as_str(),
            Some("pending" | "checking" | "releasing")
        ) {
            return Err(ProjectsError::Invalid("Finish the native release first"));
        }
        if db.query_row("SELECT EXISTS(SELECT 1 FROM runtime_events WHERE agent=?1 AND (status IN ('pending','reserved','dispatching') OR (?2 AND status='uncertain')))",rusqlite::params![a["id"].as_str().unwrap_or_default(),uncertain_blocks],|r|r.get::<_,bool>(0))? {return Err(ProjectsError::Invalid("Finish pending input first"));}
    }
    for id in studio_runtime_store::AccountTransferRepository::recovery_ids(db)? {
        if let Some((transfer, _)) =
            studio_runtime_store::AccountTransferRepository::get_typed_with_raw(db, &id)?
            && transfer.status == studio_runtime_store::transfer_record::TransferStatus::Pending
            && ids.iter().any(|id| transfer_involves(&transfer, id))
        {
            return Err(ProjectsError::Invalid("Finish pending operations first"));
        }
    }
    for table in [
        "runtime_tasks",
        "runtime_monitors",
        "runtime_tool_requests",
        "runtime_rules",
        "runtime_workspace_operations",
        "runtime_rooms",
    ] {
        let owners = ids.iter().copied().collect::<Vec<_>>();
        for r in records::owned_by(db, table, &owners)? {
            let belongs = ids.contains(r["agent"].as_str().unwrap_or_default());
            let busy = match table {
                "runtime_tasks" | "runtime_monitors" => {
                    belongs
                        && matches!(
                            r["status"].as_str(),
                            Some("running" | "starting" | "approval")
                        )
                }
                "runtime_tool_requests" => {
                    belongs
                        && !truthy(r.get("readOnly"))
                        && (r["outcome"] == "pending"
                            || (uncertain_blocks && r["outcome"] == "unknown"))
                }
                "runtime_rules" => belongs && r["inFlight"] == true,
                "runtime_workspace_operations" => {
                    belongs
                        && !matches!(
                            r["phase"].as_str(),
                            Some("complete" | "completed" | "failed" | "cancelled")
                        )
                }
                "runtime_rooms" => {
                    r["members"].as_array().is_some_and(|m| {
                        m.iter()
                            .any(|id| ids.contains(id.as_str().unwrap_or_default()))
                    }) && (truthy(r["radio"].get("active")) || truthy(r["radio"].get("next")))
                }
                _ => false,
            };
            if busy {
                return Err(ProjectsError::Invalid("Finish pending operations first"));
            }
        }
    }
    for id in ids.iter() {
        let waiting = studio_runtime_store::approvals::for_agent(
            db,
            &studio_runtime_schema::approvals::AgentId((*id).to_owned()),
        )
        .map_err(studio_runtime_store::StoreError::from)?
        .iter()
        .any(|row| {
            matches!(
                row.state,
                studio_runtime_schema::approvals::ApprovalState::Pending
            )
        });
        if waiting {
            return Err(ProjectsError::Invalid("Finish pending operations first"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn pending_transfer_blocks_workspace_switch() {
        let dir = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(dir.path()))
            .await
            .unwrap();
        store.transaction(|db| {
            let agent = studio_runtime_store::AgentRecord::from_value(studio_json::json!({
                "id":"member", "rootId":"member", "epoch":1, "status":"idle", "compactions":0, "accountKey":"source", "provider":"codex", "model":"gpt-6-astra", "cwd":std::env::temp_dir()
            })).unwrap();
            studio_runtime_store::AgentRepository::put(db, &agent)?;
            db.execute("INSERT INTO runtime_account_transfers(id,record) VALUES(?1,?2)", rusqlite::params!["transfer", r#"{"id":"transfer","leadId":"other","targetAccountKey":"target","status":"pending","created":1,"members":{"member":{"phase":"unknown","sourceAccountKey":"source"}}}"#])?;
            let value = studio_json::json!({"id":"member", "status":"idle"});
            assert!(matches!(super::idle(db, &[value], true), Err(super::ProjectsError::Invalid("Finish pending operations first"))));
            Ok(())
        }).await.unwrap();
    }
}
