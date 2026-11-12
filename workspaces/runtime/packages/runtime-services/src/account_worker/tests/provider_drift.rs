use super::*;

#[tokio::test]
async fn native_progress_reasoning_turn_preserves_its_final_answer() {
    let state = State::new().await;
    let mut peer = Peer::new();
    peer.codec = studio_runtime_process::provider_seam::ProviderCodec::codex();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let prior = state.record().await["lastAnswer"].clone();
    let frame = studio_runtime_providers::decode_codex_frame(&studio_json::to_vec(&json!({"method":"item/completed","params":{"completedAtMs":2000,"threadId":"fake-thread-0","turnId":"turn","item":{"id":"summary","type":"reasoning","summary":["Public summary"],"content":["PRIVATE_REASONING"]}}})).unwrap()).unwrap();
    assert!(matches!(frame, Frame::CodexNotification(_)));
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
    peer.completed().await;
    worker_barrier(&worker).await;
    let record = state.record().await;
    assert_eq!(record["status"], "completed");
    assert_eq!(record["lastAnswer"], prior);
    let saved = studio_runtime_store::TranscriptRepository::item(
        &state.bridge.store.test_reader().unwrap(),
        &state.agent,
        &format!("{}:summary", state.agent),
    )
    .unwrap()
    .unwrap();
    assert_eq!(saved.record.value()["text"], "Public summary");
    assert_eq!(saved.record.value()["streaming"], false);
    worker.shutdown().await.unwrap();
}

async fn native_frame(peer: &mut Peer, value: Value) -> u64 {
    peer.cursor += 1;
    let frame = studio_runtime_providers::decode_codex_frame(&studio_json::to_vec(&value).unwrap())
        .unwrap();
    assert!(matches!(
        frame,
        Frame::UnknownMethod(_) | Frame::KnownMethodUnparsed(_)
    ));
    peer.entries
        .send(JournalEntry {
            boundary_at: None,
            received_at_ms: peer.clock.now_ms(),
            cursor: EventCursor(peer.cursor),
            generation: peer.generation,
            payload: JournalPayload::Frame(frame),
        })
        .await
        .unwrap();
    peer.cursor
}

#[tokio::test]
async fn internal_sender_witness_known_completion_missing_pinned_items_still_completes_the_turn() {
    let state = State::new().await;
    let mut peer = Peer::new();
    peer.codec = studio_runtime_process::provider_seam::ProviderCodec::codex();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = native_frame(
        &mut peer,
        json!({
            "method":"turn/completed",
            "params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"completed"}}
        }),
    )
    .await;
    peer.acknowledged(cursor).await;
    assert_eq!(state.record().await["status"], "completed");
    assert_eq!(state.event_status("input").await, "delivered");
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn unknown_native_callback_is_rejected_resolved_and_worker_continues() {
    rejected_callback_keeps_worker_running(json!({
        "id":"unknown-callback", "method":"unknown/providerMethod", "params":{}
    }))
    .await;
}

#[tokio::test]
async fn incompatible_approval_is_answered_with_an_error_without_authority() {
    rejected_callback_keeps_worker_running(json!({
        "id":"incompatible-approval", "method":"item/commandExecution/requestApproval",
        "params":{"threadId":"fake-thread-0","turnId":"turn","itemId":"item","command":42}
    }))
    .await;
}

async fn rejected_callback_keeps_worker_running(request: Value) {
    let state = State::new().await;
    let mut peer = Peer::new();
    peer.codec = studio_runtime_process::provider_seam::ProviderCodec::codex();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let id = request["id"].clone();
    let cursor = native_frame(&mut peer, request).await;
    peer.acknowledged(cursor).await;
    let answer = peer.next_write().await;
    assert_eq!(answer.value["id"], id);
    assert_eq!(answer.value["error"]["code"], -32601);
    assert!(answer.value.get("result").is_none());
    let cards =
        studio_runtime_store::approvals::all(&state.bridge.store.test_reader().unwrap()).unwrap();
    assert!(
        cards.is_empty(),
        "unparsed requests cannot create an approval"
    );
    answer
        .receipt
        .send(CommandReceipt::Written {
            remote_id: None,
            duplicate: false,
            written_at: None,
        })
        .ok()
        .unwrap();
    callbacks_retired(&state).await;
    assert_eq!(
        callback_records(&state),
        0,
        "error reply retires the obligation"
    );
    peer.completed().await;
    assert_eq!(
        state.record().await["status"],
        "completed",
        "worker processes later traffic"
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn live_settings_native_echo_running_idle_and_next_turn() {
    let state = State::new().await;
    let id = state.agent.clone();
    state
        .bridge
        .store
        .transaction(move |tx| {
            let mut agent = AgentRepository::get(tx, &AgentId::from(id))?.unwrap();
            agent.native_effort = studio_runtime_schema::queue_command::Field::Value("low".into());
            AgentRepository::put(tx, &agent)
        })
        .await
        .unwrap();
    let mut peer = Peer::new();
    peer.codec = studio_runtime_process::provider_seam::ProviderCodec::codex();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.wait_for_write_receipt(&start).await;
    peer.accepted(&start).await;
    for idle in [false, true] {
        if idle {
            peer.completed().await;
            worker_barrier(&worker).await;
        }
        let before = state
            .bridge
            .store
            .transaction_with(|tx| store::load_streams(tx, "default"))
            .await
            .unwrap();
        let event = json!({"method":"thread/settings/updated","params":{"threadId":"fake-thread-0","threadSettings":{"model":"native-echo","effort":"high","serviceTier":"priority","approvalPolicy":"on-request","approvalsReviewer":"user","collaborationMode":{"mode":"default","settings":{"model":"native-echo"}},"cwd":"/work","modelProvider":"openai","sandboxPolicy":{"type":"dangerFullAccess"}}}});
        peer.cursor += 1;
        let cursor = peer.cursor;
        let frame =
            studio_runtime_providers::decode_codex_frame(event.to_string().as_bytes()).unwrap();
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
        worker_barrier(&worker).await;
        let after = state
            .bridge
            .store
            .transaction_with(|tx| store::load_streams(tx, "default"))
            .await
            .unwrap();
        assert_eq!(
            serde::Serialize::serialize(&before, studio_json::value::Serializer).unwrap()["publication_sequence"],
            serde::Serialize::serialize(&after, studio_json::value::Serializer).unwrap()["publication_sequence"],
            "ignored echoes do not publish settings"
        );
        assert_eq!(state.record().await["model"], "fake-model");
        assert_eq!(state.record().await["nativeEffort"], "low");
    }
    state.enqueue("after-settings").await;
    let next = state.start(&mut peer, &worker).await;
    assert_eq!(next["params"]["effort"], "low");
    peer.wait_for_write_receipt(&next).await;
    peer.accepted(&next).await;
    peer.completed().await;
    worker.shutdown().await.unwrap();
}
