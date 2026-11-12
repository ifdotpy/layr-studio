//! Exact-turn overrides share retained admission, replay and receipt ownership.
use crate::account_session_runtime::{
    AccountSessionError, AccountSessionPort, ProviderRequest, ProviderResponse, RequestCertainty,
    SessionOutcome, provider_receipt_matches,
};
use crate::account_worker::retained::RequestActor;
use crate::native_actions::{NativeError, Result};
use studio_runtime_providers::{Method, codex};
use studio_runtime_store::AgentRecord;

pub async fn update_current_turn(
    port: &dyn AccountSessionPort,
    agent: AgentRecord,
    request_id: String,
    params: codex::TurnSettingsUpdateParams,
) -> Result<AgentRecord> {
    let operation = format!("current-turn-settings:{request_id}");
    let scope = port.current_scope(&agent.account_key);
    // internal-sender-witness: workspaces/runtime/packages/runtime-http/tests/claude_file_backups.rs#internal_sender_witness_current_turn_settings_http_worker_codec
    let receipt = port
        // internal-sender-witness: workspaces/runtime/packages/runtime-http/tests/claude_file_backups.rs#internal_sender_witness_current_turn_settings_http_worker_codec
        .provider_request(ProviderRequest {
            root_request_id: request_id,
            admitted_request: None,
            account: agent.account_key.clone(),
            operation: operation.clone(),
            actor: Some(RequestActor {
                agent: agent.id.0.clone(),
                epoch: agent.epoch,
            }),
            expected_scope: scope.clone(),
            deleted_actor_session: None,
            method: Method::TurnSettingsUpdate,
            params: serde::Serialize::serialize(&params, studio_json::value::Serializer)
                .map_err(|_| NativeError::Invalid("settings/params"))?,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
        })
        .await
        .map_err(|error| match error {
            AccountSessionError::Requires(_) | AccountSessionError::Refused(_) => {
                NativeError::Invalid("settings/not-sent")
            }
            AccountSessionError::Expired
            | AccountSessionError::InvalidResponse(_)
            | AccountSessionError::Persistence
            | AccountSessionError::Unknown => NativeError::Unknown,
        })?;
    provider_receipt_matches(&receipt, &agent.account_key, &operation, scope.as_ref())
        .map_err(|_| NativeError::Unknown)?;
    if let SessionOutcome::Written { response, .. } = receipt.outcome {
        let ProviderResponse::Reply(value) = response else {
            return Err(NativeError::Invalid("settings/rejected"));
        };
        let response: codex::TurnSettingsUpdateResponse =
            serde::Deserialize::deserialize(value).map_err(|_| NativeError::Unknown)?;
        match response.status {
            codex::TurnSettingsUpdateStatus::Variant1(_) => Ok(agent),
            codex::TurnSettingsUpdateStatus::Variant2(_) => Err(NativeError::Changed),
        }
    } else if receipt.certainty() == RequestCertainty::NotSent {
        Err(NativeError::Invalid("settings/not-sent"))
    } else {
        Err(NativeError::Unknown)
    }
}

pub(crate) fn native_event(
    frame: &studio_runtime_providers::Frame,
) -> Option<&codex::ThreadSettingsUpdatedNotification> {
    if let studio_runtime_providers::Frame::CodexNotification(notification) = frame
        && let codex::ServerNotification::ThreadSettingsUpdated { params, .. } =
            notification.as_ref()
    {
        Some(params)
    } else {
        None
    }
}
