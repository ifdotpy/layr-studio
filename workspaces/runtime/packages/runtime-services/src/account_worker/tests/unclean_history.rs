use super::*;
use serde::Serialize;

struct AllowMessage;
impl crate::message_admission::MessageAdmission for AllowMessage {
    fn workspace(
        &self,
        _: &studio_runtime_store::Transaction<'_>,
        _: &studio_runtime_store::AgentRecord,
    ) -> Result<Vec<Value>, crate::coordination::CoordinationError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn message_after_unclean_start_sends_only_new_input() {
    let state = State::new().await;
    let agent = state.agent.clone();
    let epoch = state.epoch;
    let message_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    state
        .bridge
        .run(move |store| async move {
            store::claim(Arc::clone(&store), "default", &agent, epoch)
                .await?
                .unwrap();
            store
                .transaction_with(move |tx| {
                    SchedulerRepository::hold_unclean_starts(tx)?;
                    crate::coordination::post_message_with_admission(
                        tx,
                        crate::coordination::PostManagedMessage {
                            room: &agent,
                            id: &message_id,
                            text: "new input",
                            delivery: studio_runtime_schema::delivery::DeliveryMoment::CurrentTurn,
                            assets: &[],
                        },
                        &AllowMessage,
                    )
                    .map_err(|e| StoreError::SchemaMismatch(e.to_string()))?;
                    Ok::<_, StoreError>(())
                })
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(state.event_status("input").await, "uncertain");
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    state.wake(&worker).await;
    let resume = peer.write("thread/resume").await;
    peer.reply(&resume, json!({"thread":{"id":"fake-thread-0"}}))
        .await;
    let start = peer.write("turn/start").await;
    let input = start["params"]["input"].to_string();
    assert!(input.contains("new input"), "{start}");
    assert!(!input.contains("README.md"), "{start}");
    peer.accepted(&start).await;
    assert_eq!(state.event_status("input").await, "uncertain");
    worker.shutdown().await.unwrap();
}

#[derive(Serialize)]
struct Page {
    data: Vec<Turn>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Turn {
    id: &'static str,
    thread_id: &'static str,
    status: &'static str,
    items: Vec<Item>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    id: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    client_id: &'static str,
}

#[tokio::test]
async fn unclean_held_start_recovers_exact_input_through_worker_history() {
    let state = State::new().await;
    let agent = state.agent.clone();
    let epoch = state.epoch;
    state
        .bridge
        .run(move |store| async move {
            store::claim(Arc::clone(&store), "default", &agent, epoch)
                .await?
                .expect("claimed");
            store
                .transaction_with(|tx| {
                    studio_runtime_store::SchedulerRepository::hold_unclean_starts(tx)?;
                    Ok::<_, WorkerError>(())
                })
                .await?;
            Ok(())
        })
        .await
        .expect("hold input");
    assert_eq!(state.event_status("input").await, "uncertain");
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let history = peer.write("thread/turns/list").await;
    peer.reply(
        &history,
        studio_runtime_store::projection_value(&Page {
            data: vec![Turn {
                id: "native-turn",
                thread_id: "fake-thread-0",
                status: "completed",
                items: vec![Item {
                    id: "input-item",
                    kind: "userMessage",
                    client_id: "annotation:input",
                }],
            }],
        }),
    )
    .await;
    worker_barrier(&worker).await;
    assert_eq!(state.event_status("input").await, "delivered");
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Saved {
        start_attempt: SavedAttempt,
        auto_wake: bool,
    }
    #[derive(serde::Deserialize)]
    struct SavedAttempt {
        submitted: bool,
    }
    let record: Saved =
        studio_json::from_str(&state.record().await.to_string()).expect("saved agent");
    assert!(!record.start_attempt.submitted);
    assert!(!record.auto_wake);
    assert!(peer.writes.try_recv().is_err());
    worker.shutdown().await.expect("shutdown");
}
