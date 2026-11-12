//! Captured storage shapes exercise the account worker's actual open barrier.
use super::*;

#[tokio::test]
async fn live_restart_rows_open_account_worker_and_restore_callbacks_without_retry() {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(root.path())).await.unwrap());
        crate::live_restart_fixture::seed(&store).await;
        let fixture = crate::live_restart_fixture::rows();
        // The incident failed before the proposal was persisted. Recreate the
        // destination proposal from captured settings, preserving its raw defaults.
        let mut proposal = fixture["runtime_account_transfers"][0].clone();
        for member in proposal["members"].as_object_mut().unwrap().values_mut() {
            assert!(member["settings"]["workerDefaults"].is_object());
            member["targetSettings"] = member["settings"].clone();
        }
        proposal["status"] = json!("pending");
        store
            .transaction(move |tx| {
                studio_runtime_store::AccountTransferRepository::put(
                    tx,
                    proposal["id"].as_str().unwrap(),
                    &proposal.to_string(),
                )
            })
            .await
            .expect("captured destination proposal decodes before startup");
        crate::provider_callbacks::recover_tool_requests_on_startup(&store, now())
            .await
            .unwrap();
        crate::monitor_runtime::recover_on_startup(&store, now())
            .await
            .unwrap();
        let bridge = StoreBridge::new(store.clone())
            .await
            .expect("real startup barrier");
        // Decode even terminal transfers: startup intentionally skips terminal history.
        for row in fixture["runtime_account_transfers"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let snapshot = crate::account_transfer_driver::load(&store.test_reader().unwrap(), id)
                .expect("live workerDefaults decode through real repository");
            drop(snapshot);
        }
        // Restore retained requests through the same reader account open uses.
        let mut retained = 0;
        for row in fixture["runtime_outbox"].as_array().unwrap() {
            if row["kind"] == "provider_call" {
                let account = row["account"].as_str().unwrap();
                let generation = row["generation"].as_u64().unwrap();
                retained += super::super::retained::recover(&store, account, generation)
                    .await
                    .expect("captured retained provider-call decode")
                    .len();
            }
        }
        assert!(retained > 0, "fixture must exercise a retained request");
        // Callback restore uses the runtime's identity owner after anonymization.
        let mut restored_callbacks = 0;
        for row in fixture["runtime_tool_requests"].as_array().unwrap() {
            if row["kind"] == "provider_callback" && !row["payload"]["identity"].is_null() {
                let owner: crate::tool_requests::CallbackTask<
                    crate::provider_callbacks::CallbackOwner,
                > = studio_json::from_str(&row.to_string()).expect("callback owner decode");
                let identity = &owner.payload.identity;
                let key = crate::provider_callbacks::callback_origin_key(
                    &identity.context,
                    &identity.rpc_id,
                );
                let restored = crate::tool_requests::get_callback::<
                    crate::provider_callbacks::CallbackOwner,
                >(&store.test_reader().unwrap(), &key)
                .expect("restore captured callback")
                .expect("callback owner retained");
                assert_eq!(restored.id, owner.id);
                crate::provider_callbacks::recover_connection(
                    &store,
                    &identity.context.account,
                    &identity.context.connection,
                    now(),
                )
                .await
                .expect("callback generation settlement");
                restored_callbacks += 1;
                assert_eq!(
                    crate::provider_callbacks::recover_connection(
                        &store,
                        &identity.context.account,
                        &identity.context.connection,
                        now()
                    )
                    .await
                    .expect("repeat callback restoration"),
                    0
                );
            }
        }
        assert!(
            restored_callbacks > 0,
            "callback owner restore cannot be vacuous"
        );
        let generation = fixture["runtime_account_state"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["account"] == "default")
            .and_then(|row| row["next_generation"].as_u64())
            .unwrap_or(1);
        let mut peer = Peer::identity("default", generation, None);
        let mut worker = peer.worker(bridge.clone()).await;
        unavailable_provider_barrier(&worker, &mut peer).await;
        // A second wake/barrier must remain responsive rather than exit on a stored row.
        for _ in 0..2 {
            for row in fixture["runtime_agents"].as_array().unwrap() {
                if row["accountKey"] == "default" {
                    worker
                        .send(AccountCommand::Wake {
                            agent_id: row["id"].as_str().unwrap().into(),
                            epoch: row["epoch"].as_i64().unwrap(),
                        })
                        .await
                        .unwrap();
                }
            }
            unavailable_provider_barrier(&worker, &mut peer).await;
        }
        // Replay the captured blocked member as pending. Only its lifecycle and
        // agent hold change; targetSettings/workerDefaults remain the exported bytes.
        let transfer = fixture["runtime_account_transfers"][0].clone();
        let member = transfer["members"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        let transfer_id = transfer["id"].as_str().unwrap().to_owned();
        let member_id = member.clone();
        let (account, epoch) = store
            .transaction(move |tx| {
                let mut transfer = transfer;
                transfer["status"] = json!("pending");
                transfer["members"][&member_id]["phase"] = json!("blocked");
                studio_runtime_store::AccountTransferRepository::put(
                    tx,
                    transfer["id"].as_str().unwrap(),
                    &transfer.to_string(),
                )?;
                let mut agent = AgentRepository::get(tx, &AgentId::from(member_id.as_str()))?
                    .unwrap()
                    .value();
                agent["accountTransferId"] = transfer["id"].clone();
                agent["status"] = json!("queued");
                agent["autoWake"] = json!(true);
                agent["inFlight"] = json!(false);
                for key in [
                    "nativeFailureHold",
                    "startOutcomeHold",
                    "startAttempt",
                    "restartRecovery",
                    "disconnectRecovery",
                ] {
                    agent.as_object_mut().unwrap().remove(key);
                }
                let epoch = agent["epoch"].as_i64().unwrap();
                let account = agent["accountKey"].as_str().unwrap().to_owned();
                AgentRepository::put(tx, &AgentRecord::from_value(agent)?)?;
                Ok((account, epoch))
            })
            .await
            .unwrap();
        for _ in 0..2 {
            let done = turn_preparation::wake(
                turn_preparation::Wake {
                    bridge: bridge.clone(),
                    account: account.clone(),
                    agent_id: member.clone(),
                    epoch,
                    claude_steer_receipts: false,
                    effect: None,
                    resources: Arc::new(test_resources().unwrap()),
                },
                |_, _| panic!("blocked transfer must not submit input"),
            )
            .await;
            assert!(matches!(
                done.outcome,
                Ok(turn_preparation::Outcome::Unclaimed(_))
            ));
        }
        assert!(
            bridge
                .faults
                .operations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, _, _)| name == "account_transfer_hold"),
            "the real wake must reach advance_transfer"
        );
        let after = studio_runtime_store::AccountTransferRepository::get(
            &store.test_reader().unwrap(),
            &transfer_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(after["members"][&member]["phase"], "blocked");
        assert_eq!(
            crate::provider_callbacks::recover_tool_requests_on_startup(&store, now())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            crate::account_transfer_driver::recover(&store, now())
                .await
                .unwrap(),
            0
        );
        worker
            .shutdown()
            .await
            .expect("worker survived fixture recovery");
    })
    .await
    .expect("bounded real startup and worker recovery");
}

// The captured agents can request recovery history. The unavailable provider
// boundary must answer each write explicitly rather than let unrelated RPCs time out.
async fn unavailable_provider_barrier(worker: &AccountHandle, peer: &mut Peer) {
    let barrier = worker_barrier(worker);
    tokio::pin!(barrier);
    loop {
        tokio::select! {
            () = &mut barrier => return,
            wire = peer.writes.recv() => {
                let wire = wire.expect("recovery provider request");
                assert!(wire.receipt.send(CommandReceipt::Written {
                    written_at: None,
                    remote_id: None,
                    duplicate: false,
                }).is_ok());
                peer.frame(json!({"id":wire.value["id"],"error":{"code":-32000,"message":"unavailable"}})).await;
            }
        }
    }
}
