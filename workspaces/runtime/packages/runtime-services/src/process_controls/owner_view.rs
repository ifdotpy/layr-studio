//! Fields consumed by the process panel; unrelated agent history is skipped.
use serde::Deserialize;
use studio_json::Number;
use studio_runtime_schema::records::WorkspaceMode;
use studio_runtime_store::{AgentExecutionMode, AgentStatus};

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOwner {
    #[serde(skip)]
    stored: Option<String>,
    pub id: Option<String>,
    pub epoch: Option<Number>,
    pub account_key: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub provider: Option<studio_runtime_providers::ProviderKind>,
    pub execution_mode: Option<AgentExecutionMode>,
    pub workspace_mode: Option<WorkspaceMode>,
    pub layr_ready: Option<bool>,
    pub layr_line: Option<String>,
    pub status: Option<AgentStatus>,
    pub in_flight: Option<bool>,
    pub auto_wake: Option<bool>,
    pub deleted: Option<bool>,
    pub deleted_at: Option<f64>,
    pub archived: Option<bool>,
    pub archived_at: Option<f64>,
    pub account_transfer_id: Option<serde::de::IgnoredAny>,
    pub execution_move: Option<serde::de::IgnoredAny>,
    pub native_response_turn: Option<String>,
    pub restart_recovery: Option<ProcessRecovery>,
    pub disconnect_recovery: Option<ProcessRecovery>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRecovery {
    pub stage: Option<String>,
    pub source: Option<String>,
    pub epoch: Option<Number>,
    pub account_key: Option<String>,
    pub thread_id: Option<String>,
}

impl ProcessOwner {
    pub fn account(&self) -> &str {
        self.account_key
            .as_deref()
            .filter(|account| !account.is_empty())
            .unwrap_or("default")
    }

    pub fn legacy_vm(&self) -> bool {
        crate::workspace_ready::legacy_vm_fields(
            self.execution_mode,
            self.workspace_mode,
            self.layr_ready,
            self.layr_line.is_some(),
        )
    }

    pub(super) fn decode(raw: String) -> Result<Self, studio_runtime_store::StoreError> {
        let mut owner: Self = studio_json::from_str(&raw)?;
        owner.stored = Some(raw);
        Ok(owner)
    }

    pub(super) fn same_record(&self, other: &Self) -> bool {
        let (Some(before), Some(after)) = (&self.stored, &other.stored) else {
            return false;
        };
        if before == after {
            return true;
        }
        // Stored JSON is not necessarily canonical (older writers and fixtures).
        // Preserve whole-Value equality on this rare changed-record path.
        match (
            studio_json::from_str::<studio_json::Value>(before),
            studio_json::from_str::<studio_json::Value>(after),
        ) {
            (Ok(before), Ok(after)) => before == after,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_proof_preserves_whole_owner_equality() {
        let before =
            ProcessOwner::decode(r#"{"id":"owner","epoch":1,"metadata":{"a":1,"b":2}}"#.into())
                .unwrap();
        let same =
            ProcessOwner::decode(r#"{ "metadata":{"b":2,"a":1},"epoch":1,"id":"owner" }"#.into())
                .unwrap();
        assert!(before.same_record(&before));
        assert!(before.same_record(&same));
        let changed =
            ProcessOwner::decode(r#"{"id":"owner","epoch":1,"metadata":{"a":1,"b":3}}"#.into())
                .unwrap();
        assert!(!before.same_record(&changed));
        let removed = ProcessOwner::decode(r#"{"id":"owner","epoch":1}"#.into()).unwrap();
        assert!(!before.same_record(&removed));
    }

    #[test]
    fn empty_account_keeps_default_response_and_process_permissions() {
        let owner = ProcessOwner::decode(r#"{"id":"owner","epoch":1,"accountKey":"","threadId":"thread","turnId":"turn","inFlight":true,"autoWake":true}"#.into()).unwrap();
        let target = super::super::response_target(&owner, Some("connection"))
            .expect("empty service account uses the default account");
        assert_eq!(target.account_key.as_str(), "default");
        let task: studio_runtime_store::TaskRecord = studio_json::from_str(r#"{"id":"owner:item","agent":"owner","created":1,"itemId":"item","turnId":"turn","kind":"command","status":"running","type":"commandExecution","name":"commandExecution","processId":"pid","ownerEpoch":1,"sourceAccountKey":"default","sourceConnectionId":"connection","sourceThreadId":"thread"}"#).unwrap();
        let process = super::super::project(
            studio_runtime_schema::process_controls::ProcessSourceKind::CodexTask,
            &task,
            &owner,
            Some("connection"),
        );
        assert_eq!(process.view.account_key, "default");
        use studio_runtime_schema::process_controls::BackgroundTaskAction;
        assert!(
            process
                .view
                .display
                .actions
                .contains(&BackgroundTaskAction::Stop)
        );
        assert!(
            process
                .view
                .display
                .actions
                .contains(&BackgroundTaskAction::Input)
        );
    }
}
