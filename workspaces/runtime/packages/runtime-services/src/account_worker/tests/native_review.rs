use super::*;

/// Live reviewers saved before the fix have no `startAttempt.turnId`: the
/// review/start reply set only the agent turn, then the worker crash left them
/// interrupted with a disconnect receipt. Their `turn/completed` was deferred
/// for good, and 64 unsettled frames behind it stopped the account journal
/// (thread/resume replies included). The replayed completion now settles.
#[tokio::test]
async fn an_unbound_interrupted_review_completion_settles_instead_of_holding_the_journal() {
    let state = State::new().await;
    let agent = state.agent.clone();
    let epoch = state.epoch;
    state
        .bridge
        .run(move |s| async move {
            scheduler::lifecycle_transaction_with(s.as_ref(), move |tx| {
                let mut row = AgentRepository::get(tx, &AgentId::from(agent.as_str()))?.unwrap();
                let attempt = json!({"id":"review-attempt","epoch":epoch,"events":[],"action":"review",
                    "actionRequestId":"native-review:child:0","submitted":true,"accountKey":"default",
                    "connectionGeneration":1,"connectionId":"connection","threadId":"review-thread",
                    "requestTicket":"retained:default:review","created":1.0});
                row.in_flight = Some(false);
row.auto_wake = Some(false);
row.turn_epoch = Some(epoch);
row.thread_id = Some("review-thread".to_string());
row.turn_id = Some("review-turn".to_string());
                row.review_assignment = Some(studio_runtime_schema::review::ReviewAssignment::new(
                    studio_runtime_schema::review::WorkspaceMode::Worktree,
                    studio_runtime_schema::review::ReviewTarget::GitRange {
                        base: "main".to_owned().try_into().unwrap(),
                        head: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned().try_into().unwrap(),
                    },
                ).unwrap());
for (key, value) in [


                    ("status", json!("interrupted")),



                    ("error", json!(studio_runtime_store::PROVIDER_DISCONNECTED_TURN_ERROR)),
                    ("startAttempt", attempt.clone()),
                    (
                        "disconnectRecovery",
                        json!({"accountKey":"default","epoch":epoch,"autoWake":true,
                            "threadId":"review-thread","startAttempt":attempt}),
                    ),
                    (
                        "nativeReview",
                        json!({"target":{"kind":"gitRange","base":"main","head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                            "requestId":"lead:review:r1","actorId":"lead","args":{},
                            "status":"started","response":{}}),
                    ),
                ] { match key {
                    "status" => row.status = serde::Deserialize::deserialize(value).unwrap(),
                    "error" => row.error = serde::Deserialize::deserialize(value).unwrap(),
                    "startAttempt" => row.start_attempt = Some(serde::Deserialize::deserialize(value).unwrap()),
                    "disconnectRecovery" => row.disconnect_recovery = Some(Box::new(serde::Deserialize::deserialize(value).unwrap())),
                    _ => { row.extra.insert(key.into(), value); }
                } }
                AgentRepository::put(tx, &row)?;
                Ok(())
            })
            .await
        })
        .await
        .unwrap();
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let cursor = peer
        .frame(
            json!({"method":"turn/completed","params":{"threadId":"review-thread",
            "turn":{"id":"review-turn","status":"completed"}}}),
        )
        .await;
    peer.acknowledged_or_worker_exits(cursor, &mut worker).await;
    let done = state.wait_record(|r| r["status"] == "completed").await;
    assert_eq!(done["lastCompletedTurn"], "review-turn", "{done}");
    assert_eq!(done["startAttempt"]["turnId"], "review-turn", "{done}");
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn reviewer_thread_receipt_uses_scratch_cwd() {
    let state = State::new().await;
    let scratch = state.root.join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let id = state.agent.clone();
    let scratch_text = scratch.to_str().unwrap().to_owned();
    let copy = state.root.to_str().unwrap().to_owned();
    let initial = scratch_text.clone();
    state
        .bridge
        .run(move |store| async move {
            store
                .transaction_with(move |tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from(id.as_str()))?.unwrap();
                    row.cwd = Some(initial);
                    AgentRepository::put(tx, &row)
                })
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    state.wake(&worker).await;
    let resume = peer.write("thread/resume").await;
    assert_eq!(resume["params"]["cwd"], scratch_text);
    let attempt = state.record().await["startAttempt"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let id = state.agent.clone();
    state
        .bridge
        .run(move |store| async move {
            store
                .transaction_with(move |tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from(id.as_str()))?.unwrap();
                    row.cwd = Some(copy.clone());
                    row.layr_read_cwd = Some(copy.into());
                    row.extra.insert("layrScratch".into(), json!(scratch_text));
                    row.review_assignment = Some(
                        studio_runtime_schema::review::ReviewAssignment::new(
                            studio_runtime_schema::review::WorkspaceMode::Worktree,
                            studio_runtime_schema::review::ReviewTarget::GitRange {
                                base: "main".to_owned().try_into().unwrap(),
                                head: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                                    .to_owned()
                                    .try_into()
                                    .unwrap(),
                            },
                        )
                        .unwrap(),
                    );
                    AgentRepository::put(tx, &row)
                })
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    peer.reply(&resume, json!({"thread":{"id":"fake-thread-0"}}))
        .await;
    worker_barrier(&worker).await;
    let receipt = state
        .bridge
        .run(move |store| async move {
            let db = store.test_reader()?;
            Ok(studio_runtime_store::ExecutionHistoryRepository::receipt::<
                Value,
            >(
                &db,
                &attempt,
                studio_runtime_schema::persistence::ExecutionReceiptKind::ThreadPrepared,
            )?)
        })
        .await
        .unwrap()
        .expect("accepted thread receipt");
    assert_eq!(receipt.result, json!(true));
    assert_eq!(receipt.input["thread"], "fake-thread-0");
    worker.shutdown().await.unwrap();
}
