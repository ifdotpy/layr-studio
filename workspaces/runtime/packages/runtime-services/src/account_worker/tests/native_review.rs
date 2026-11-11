use super::*;

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
                    row.layr_scratch = studio_runtime_schema::queue_command::Field::Value(
                        studio_runtime_store::StoredTransferValue::Known(std::path::PathBuf::from(
                            scratch_text,
                        )),
                    );
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
