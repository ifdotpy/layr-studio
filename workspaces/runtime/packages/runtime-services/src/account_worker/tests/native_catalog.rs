//! Native account-wide catalog notifications use the existing SSE revision owner.
use super::*;

#[tokio::test]
async fn native_catalog_committed_events_invalidate_only_their_account() {
    for provider in [
        studio_runtime_providers::ProviderKind::Codex,
        studio_runtime_providers::ProviderKind::Claude,
    ] {
        let state = State::new().await;
        let changes = Arc::new(crate::change_notifications::ChangeNotificationHub::new());
        let notifications = Arc::new(crate::resource_notify::ResourceNotifications::new(changes));
        let sink = Arc::new(
            crate::notification_runtime_effects::RuntimeNotificationPostCommitSink {
                resources: notifications.clone(),
                token_rates: Arc::new(crate::token_rate::TokenRateHub::new()),
            },
        );
        let mut peer = Peer::new();
        peer.codec = match provider {
            studio_runtime_providers::ProviderKind::Codex => ProviderCodec::codex(),
            studio_runtime_providers::ProviderKind::Claude => ProviderCodec::default(),
        };
        peer.wire = provider.wire();
        let mut resources = test_resources().unwrap();
        resources.claude_account_pin = json!({"provider":provider.key()});
        let mut worker = peer
            .open_worker_with_resources_and_sink(state.bridge.clone(), resources, sink)
            .await;
        use studio_runtime_schema::resource_ref::{AccountResource, ResourceRef};
        let resource = ResourceRef::NativeCatalogResource(AccountResource {
            account_key: "default".to_owned().try_into().unwrap(),
        });
        let other = ResourceRef::NativeCatalogResource(AccountResource {
            account_key: "other".to_owned().try_into().unwrap(),
        });
        for method in if provider == studio_runtime_providers::ProviderKind::Codex {
            vec!["skills/changed", "app/list/updated"]
        } else {
            vec!["skills/changed"]
        } {
            let params = if method == "app/list/updated" {
                json!({"data":[]})
            } else {
                json!({})
            };
            let bytes = studio_json::to_vec(&json!({"method":method,"params":params})).unwrap();
            let frame = (peer.codec.decode)(&bytes).unwrap().0;
            let before = notifications.published_revision(&resource).unwrap();
            peer.cursor += 1;
            let cursor = peer.cursor;
            peer.entries
                .send(JournalEntry {
                    boundary_at: None,
                    received_at_ms: peer.clock.now_ms(),
                    cursor: EventCursor(cursor),
                    generation: peer.generation,
                    payload: JournalPayload::Frame(frame),
                })
                .await
                .unwrap();
            peer.acknowledged(cursor).await;
            assert!(
                notifications.published_revision(&resource).unwrap() > before,
                "{provider:?} {method} must reach the catalog SSE resource"
            );
            assert_eq!(notifications.published_revision(&other).unwrap(), 0);
        }
        worker.shutdown().await.unwrap();
    }
}

#[test]
fn native_catalog_revisions_ignore_store_changes_and_replay_but_reset_on_restart() {
    use crate::notification_runtime_effects::NotificationPostCommitSink;
    let changes = Arc::new(crate::change_notifications::ChangeNotificationHub::new());
    let notifications = Arc::new(crate::resource_notify::ResourceNotifications::new(
        changes.clone(),
    ));
    let sink = crate::notification_runtime_effects::RuntimeNotificationPostCommitSink {
        resources: notifications.clone(),
        token_rates: Arc::new(crate::token_rate::TokenRateHub::new()),
    };
    let resource = json!({"kind":"native-catalog","accountKey":"default"});
    let refs = [resource];
    changes.notify_one();
    assert!(
        notifications
            .changes_since(&refs, Some(0))
            .unwrap()
            .resources
            .is_empty()
    );
    sink.native_catalog_changed("default", "native-event-1")
        .unwrap();
    let baseline = notifications.changes_since(&refs, None).unwrap();
    assert_eq!(baseline.resources, refs);
    let revision = baseline.cursor.revision;
    sink.native_catalog_changed("default", "native-event-1")
        .unwrap();
    sink.native_catalog_changed("other", "native-event-2")
        .unwrap();
    assert!(
        notifications
            .changes_since(&refs, Some(revision))
            .unwrap()
            .resources
            .is_empty()
    );
    let restarted = crate::resource_notify::ResourceNotifications::new(Arc::new(
        crate::change_notifications::ChangeNotificationHub::new(),
    ));
    let reconnect = restarted.changes_since(&refs, None).unwrap();
    assert_ne!(baseline.cursor.epoch, reconnect.cursor.epoch);
    assert_eq!(
        reconnect.resources, refs,
        "a restart still supplies an authoritative read trigger"
    );
}
