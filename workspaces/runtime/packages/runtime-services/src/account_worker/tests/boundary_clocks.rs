use super::*;

async fn enqueue_boundary_frame(
    peer: &mut Peer,
    value: Value,
) -> (u64, studio_runtime_schema::provider_timing::BoundaryStamp) {
    peer.cursor += 1;
    let mut entry = journal_frame(&peer.clock, peer.generation, peer.cursor, &value);
    let stamp = studio_runtime_process::clock::boundary_stamp();
    entry.boundary_at = Some(stamp);
    peer.entries.send(entry).await.unwrap();
    (peer.cursor, stamp)
}

async fn frame_at_boundary(peer: &mut Peer, value: Value) {
    let (cursor, _) = enqueue_boundary_frame(peer, value).await;
    peer.acknowledged(cursor).await;
}

#[cfg(unix)]
#[tokio::test]
async fn account_turn_boundary_clocks_reach_existing_analytics_for_both_providers() {
    for provider in [
        studio_runtime_providers::ProviderKind::Codex,
        studio_runtime_providers::ProviderKind::Claude,
    ] {
        let state = State::new().await;
        let mut peer = Peer::new();
        let mut worker = provider_worker(&state, &mut peer, provider).await;
        state.wake(&worker).await;
        let mut request = peer.next_write().await;
        if request.value["method"] == "thread/resume" {
            request
                .receipt
                .send(CommandReceipt::Written {
                    remote_id: None,
                    duplicate: false,
                    written_at: Some(studio_runtime_process::clock::boundary_stamp()),
                })
                .ok()
                .unwrap();
            peer.reply(&request.value, json!({"thread":{"id":"fake-thread-0"}}))
                .await;
            request = peer.next_write().await;
        }
        assert_eq!(request.value["method"], "turn/start");
        let operation = request.value["id"].as_str().unwrap().to_owned();
        let written_at = studio_runtime_process::clock::boundary_stamp();
        // The provider reply commits while its actual write receipt is held.
        // This exercises both providers without depending on lane scheduling.
        let (reply_cursor, _) = enqueue_boundary_frame(
            &mut peer,
            json!({"id":request.value["id"],"result":{"turn":{"id":"turn"}}}),
        )
        .await;
        state.wait_record(|record| record["turnId"] == "turn").await;
        request
            .receipt
            .send(CommandReceipt::Written {
                remote_id: None,
                duplicate: false,
                written_at: Some(written_at),
            })
            .ok()
            .unwrap();
        peer.acknowledged(reply_cursor).await;
        frame_at_boundary(&mut peer, json!({"method":"turn/started","params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"inProgress"}}})).await;
        let delta = match provider {
            studio_runtime_providers::ProviderKind::Codex => "item/reasoning/textDelta",
            studio_runtime_providers::ProviderKind::Claude => "item/modelOutput/firstDelta",
        };
        frame_at_boundary(&mut peer, json!({"method":delta,"params":{"threadId":"fake-thread-0","turnId":"turn","itemId":"reasoning","delta":"thought"}})).await;
        frame_at_boundary(&mut peer, json!({"method":"turn/completed","params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"completed"}}})).await;
        crate::analytics_projection::project_pending(&state.bridge.store)
            .await
            .unwrap();
        let report = crate::analytics::read(
            &state.bridge.store,
            &crate::analytics::AnalyticsOptions {
                scope: Some("all".into()),
                view: Some("detail".into()),
                detail: Some("turns".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let timing = &report["turns"][0]["boundaryTiming"];
        assert_eq!(timing["requestId"], operation, "{report}");
        for field in [
            "acceptedToWrittenMs",
            "writtenToFirstEventMs",
            "writtenToFirstTokenMs",
            "acceptedToTerminalMs",
        ] {
            assert!(timing[field].is_number(), "missing {field}: {report}");
        }
        worker.shutdown().await.unwrap();
    }
}

async fn monitor_output_capacity(state: &State, fill: bool) {
    let agent = state.agent.clone();
    let epoch = state.epoch;
    state.bridge.run(move |store| async move {
        store.transaction_with(move |tx| {
            use studio_runtime_store::{MonitorRecord, Table};
            let table = Table::<MonitorRecord>::new(tx);
            if fill {
                let value = crate::monitor_policy::complete_test_monitor(json!({
                    "id":"clock-monitor","agent":agent,"epoch":epoch,"status":"running",
                    "operation":{"agent":agent,"epoch":epoch,"accountKey":"default","connectionId":"connection"}
                }));
                let record = serde::Deserialize::deserialize(value).map_err(StoreError::from)?;
                table.put(&record)?;
                let delta = crate::monitor_event_queue::OutputDelta { delta_base64:"eA==".into(), stream:None };
                for cursor in 0.. {
                    match crate::monitor_event_queue::enqueue_output_in_transaction(tx,"clock-monitor","default",0,cursor,&delta) {
                        Ok(true) => {},
                        Ok(false) => panic!("fixture monitor owns its output"),
                        Err(crate::notification_dispatch::NotificationDispatchError::StreamCapacity) => break,
                        Err(error) => return Err(error.into()),
                    }
                }
            } else {
                let mut record = table.get(&"clock-monitor".into())?.unwrap();
                record.stream_queue = Some(Vec::new());
                table.put(&record)?;
            }
            Ok::<_,WorkerError>(())
        }).await
    }).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn prefetched_reply_clock_survives_monitor_backpressure() {
    use std::sync::atomic::Ordering;
    for provider in [
        studio_runtime_providers::ProviderKind::Codex,
        studio_runtime_providers::ProviderKind::Claude,
    ] {
        let state = State::new().await;
        let mut peer = Peer::new();
        let mut worker = provider_worker(&state, &mut peer, provider).await;
        worker_barrier(&worker).await;
        state.wake(&worker).await;
        let mut request = peer.next_write().await;
        if request.value["method"] == "thread/resume" {
            request
                .receipt
                .send(CommandReceipt::Written {
                    remote_id: None,
                    duplicate: false,
                    written_at: None,
                })
                .ok()
                .unwrap();
            peer.reply(&request.value, json!({"thread":{"id":"fake-thread-0"}}))
                .await;
            request = peer.next_write().await;
        }
        assert_eq!(request.value["method"], "turn/start");
        *state.bridge.faults.pause_after_operation.lock().unwrap() = Some("outcome".into());
        let written = studio_runtime_process::clock::boundary_stamp();
        request
            .receipt
            .send(CommandReceipt::Written {
                remote_id: None,
                duplicate: false,
                written_at: Some(written),
            })
            .ok()
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            state.bridge.faults.paused_after.notified(),
        )
        .await
        .expect("write receipt committed");
        *state.bridge.faults.pause_after_operation.lock().unwrap() = None;
        state.bridge.faults.release_after.notify_one();
        monitor_output_capacity(&state, true).await;
        state
            .bridge
            .faults
            .hold_journal_batch
            .store(true, Ordering::SeqCst);
        let _ = enqueue_boundary_frame(&mut peer,json!({"method":"command/exec/outputDelta","params":{"processId":"clock-monitor","deltaBase64":"eQ==","stream":"stdout"}})).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            state.bridge.faults.journal_batch_ready.notified(),
        )
        .await
        .expect("monitor output held before batch collection");
        let (reply_cursor, reply_stamp) = enqueue_boundary_frame(
            &mut peer,
            json!({"id":request.value["id"],"result":{"turn":{"id":"turn"}}}),
        )
        .await;
        assert_eq!(
            worker.session_journal_barrier().await.unwrap().0,
            reply_cursor,
            "reply already prefetched into account event queue"
        );
        state
            .bridge
            .faults
            .hold_journal_batch
            .store(false, Ordering::SeqCst);
        state.bridge.faults.release_journal_batch.notify_one();
        state.wait_record(|record| record["turnId"] == "turn").await;
        assert!(
            peer.acks.try_recv().is_err(),
            "full queue has not acknowledged the reply"
        );
        monitor_output_capacity(&state, false).await;
        peer.acknowledged(reply_cursor).await;
        frame_at_boundary(&mut peer,json!({"method":"turn/started","params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"inProgress"}}})).await;
        frame_at_boundary(&mut peer,json!({"method":"turn/completed","params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"interrupted"}}})).await;
        crate::analytics_projection::project_pending(&state.bridge.store)
            .await
            .unwrap();
        let report = crate::analytics::read(
            &state.bridge.store,
            &crate::analytics::AnalyticsOptions {
                scope: Some("all".into()),
                view: Some("detail".into()),
                detail: Some("turns".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let timing = &report["turns"][0]["boundaryTiming"];
        assert_eq!(
            timing["firstEvent"],
            json!(reply_stamp),
            "{provider:?}: {report}"
        );
        assert_eq!(
            timing["writtenToFirstEventMs"],
            json!((reply_stamp.monotonic_ns - written.monotonic_ns) as f64 / 1_000_000.)
        );
        worker.shutdown().await.unwrap();
    }
}
