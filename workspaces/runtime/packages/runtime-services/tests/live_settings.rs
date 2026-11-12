//! Native settings events enter the production codec and notification dispatcher.
#![allow(clippy::unwrap_used)]
use studio_json::json;
use studio_runtime_process::provider_seam::ProviderCodec;
use studio_runtime_services::notification_dispatch::*;
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore};

#[tokio::test]
async fn live_settings_native_echo_preserves_studio_intent() {
    let root = tempfile::tempdir().unwrap();
    let store = RuntimeStore::open(Some(root.path())).await.unwrap();
    store.transaction(|tx| AgentRepository::put(tx, &AgentRecord::from_value(json!({"id":"agent","rootId":"agent","epoch":1,"provider":"codex","accountKey":"account","threadId":"thread","status":"idle","model":"local-newer","effort":"low","nativeEffort":"low","fastMode":false,"updatedAt":2.0,"pendingSettings":{"model":"queued"}}))?)).await.unwrap();
    for (cursor, emitted, effort) in [
        (1, None, Some("high")),
        (1, None, Some("high")),
        (2, Some(1000), None),
        (3, Some(2000), Some("high")),
        (4, Some(3000), None),
    ] {
        let event = json!({"method":"thread/settings/updated","emittedAtMs":emitted,"params":{"threadId":"thread","threadSettings":{"model":"native-echo","effort":effort,"serviceTier":"priority","approvalPolicy":"on-request","approvalsReviewer":"user","collaborationMode":{"mode":"default","settings":{"model":"native-echo"}},"cwd":"/work","modelProvider":"openai","sandboxPolicy":{"type":"dangerFullAccess"}}}});
        let frame = (ProviderCodec::codex().decode)(event.to_string().as_bytes())
            .unwrap()
            .0;
        store
            .transaction_with(move |tx| {
                let result = dispatch_notification(
                    tx,
                    "agent",
                    &PublicationContext::default(),
                    &NotificationStreamBuffer::default(),
                    &ProviderDeliveryIdentity {
                        provider_handle: "account:account".into(),
                        generation: 1,
                        cursor,
                    },
                    &frame,
                    4.0,
                )?;
                assert!(matches!(result.outcome, DispatchOutcome::Ignored { ref reason } if reason == studio_runtime_providers::codex_disposition::THREAD_SETTINGS_ECHO_REASON));
                let agent = AgentRepository::get(tx, &AgentId::from("agent"))?
                    .unwrap()
                    .value();
                assert_eq!(agent["model"], "local-newer");
                assert_eq!(agent["effort"], "low");
                assert_eq!(agent["nativeEffort"], "low");
                assert_eq!(agent["fastMode"], false);
                assert_eq!(agent["pendingSettings"]["model"], "queued");
                Ok::<_, NotificationDispatchError>(())
            })
            .await
            .unwrap();
    }
}
