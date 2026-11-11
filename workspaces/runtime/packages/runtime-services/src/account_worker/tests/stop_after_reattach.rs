//! Stop ends the provider work of the agent's turn after its account worker
//! reattached to the same provider process (live agents m1-merge-main 4964fc10
//! and u1-agent-ui b91c79c8, 22 October 2026). A lost provider connection
//! interrupted both agents; the replacement worker attached to the same
//! app-server generation with a new connection id and continued their turns.
//! A lead then stopped them. The stop bound its turn/interrupt to the old
//! connection id; the session refused that scope before it wrote anything, so
//! the turns ran on and their `cargo nextest` and `just test-fast` commands ran
//! for 15 more minutes. The Codex app-server also keeps a thread's unified exec
//! processes after an interrupt, so the stop must end them too.

use super::*;
use crate::account_session_runtime::SessionOutcome as ReceiptOutcome;
use crate::account_session_runtime::{
    AccountSessionError, AccountSessionRuntime, ProviderRequest, RetainedAccountSession,
    SessionFuture, SessionReceipt,
};
use crate::account_worker::retained::{PortError, RetainedSessionPort, SessionRequest};
use crate::agent_management::Action;
use crate::lifecycle_effects::ProductionLifecyclePorts;
use crate::recovery_history::ConnectionScope;

/// The registry broker's request path (`dispatch_request` in the runtime
/// server): a request bound to another connection is not sent.
struct WorkerSession {
    port: RetainedSessionPort,
}

struct RefusingSession {
    worker: WorkerSession,
    method: studio_runtime_providers::Method,
}

impl RetainedAccountSession for RefusingSession {
    fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
        self.worker.current_scope(account)
    }

    fn request(&self, request: ProviderRequest) -> SessionFuture {
        if request.method == self.method {
            return Box::pin(async { Err(AccountSessionError::Refused("fixture refusal")) });
        }
        self.worker.request(request)
    }
}

struct LostInterruptReply(WorkerSession);

struct TimedOutInterrupt {
    worker: WorkerSession,
    timeout: Arc<tokio::sync::Notify>,
    method: studio_runtime_providers::Method,
}

impl RetainedAccountSession for TimedOutInterrupt {
    fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
        self.worker.current_scope(account)
    }
    fn request(&self, request: ProviderRequest) -> SessionFuture {
        let interrupt = request.method == self.method;
        let receipt = self.worker.request(request);
        let timeout = Arc::clone(&self.timeout);
        Box::pin(async move {
            if !interrupt {
                return receipt.await;
            }
            let pending = tokio::spawn(receipt);
            timeout.notified().await;
            pending.abort();
            Err(AccountSessionError::Unknown)
        })
    }
}

impl RetainedAccountSession for LostInterruptReply {
    fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
        self.0.current_scope(account)
    }
    fn request(&self, request: ProviderRequest) -> SessionFuture {
        let lost = request.method == studio_runtime_providers::Method::TurnInterrupt;
        let receipt = self.0.request(request);
        Box::pin(async move {
            let result = receipt.await?;
            if lost {
                Err(AccountSessionError::Unknown)
            } else {
                Ok(result)
            }
        })
    }
}

impl RetainedAccountSession for WorkerSession {
    fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
        (self.port.scope().account == account).then(|| self.port.scope().clone())
    }

    fn request(&self, request: ProviderRequest) -> SessionFuture {
        let port = self.port.clone();
        Box::pin(async move {
            let scope = port.scope().clone();
            if request
                .expected_scope
                .as_ref()
                .is_some_and(|expected| expected != &scope)
            {
                return Ok(SessionReceipt {
                    scope: request.expected_scope.unwrap_or(scope),
                    operation: request.operation,
                    replayed: false,
                    outcome: ReceiptOutcome::ExpectedScopeMismatch,
                });
            }
            let receipt = port
                .request(SessionRequest {
                    root_request_id: request.root_request_id,
                    admitted_request: request.admitted_request,
                    scope,
                    expected_scope: request.expected_scope,
                    operation: request.operation,
                    actor: request.actor,
                    transfer: None,
                    deleted_actor_session: request.deleted_actor_session,
                    method: request.method,
                    params: request.params,
                    deadline: request.deadline,
                })
                .await
                .map_err(|error| match error {
                    PortError::Expired => AccountSessionError::Refused("Provider request expired"),
                    PortError::Unknown => AccountSessionError::Unknown,
                    PortError::Identity
                    | PortError::ScopeMismatch
                    | PortError::Invalid
                    | PortError::NotSent(_)
                    | PortError::Deadline
                    | PortError::Capacity => {
                        AccountSessionError::Refused("Provider request was refused")
                    }
                })?;
            Ok(receipt.into())
        })
    }
}

/// The production account session over the replacement worker.
fn account_session(state: &State, port: RetainedSessionPort) -> Arc<AccountSessionRuntime> {
    account_session_over(state, Some(Arc::new(WorkerSession { port })))
}

/// The production account session over `session`, or without one.
fn account_session_over(
    state: &State,
    session: Option<Arc<dyn RetainedAccountSession>>,
) -> Arc<AccountSessionRuntime> {
    let home = state.root.join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    let registry_path = state.root.join("accounts/registry.json");
    std::fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
    std::fs::write(
        &registry_path,
        studio_json::to_vec(&json!({
            "version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,
            "defaultAccountKey":"default",
            "accounts":{"default":{
                "id":"default","home":home,"label":"Codex","source":"Codex CLI",
                "accountId":"fixture-account","status":"signedOut",
                "provider":studio_runtime_schema::ProviderKind::Codex
            }},
            "logins":{},"deleteReceipts":{}
        }))
        .unwrap(),
    )
    .unwrap();
    Arc::new(AccountSessionRuntime::new(
        Arc::new(crate::account_driver::AccountDriver::open(&registry_path).unwrap()),
        session,
        tokio::runtime::Handle::current(),
        state.root.join("accounts"),
        home.clone(),
        home,
    ))
}

/// The lifecycle action that ends the agent's work.
#[derive(Clone, Copy, PartialEq)]
enum Ending {
    Stop,
    Delete,
}

async fn ending_after_worker_reattach_ends_the_turn_work(ending: Ending) {
    let state = State::new().await;
    let mut first = Peer::new();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    let cursor = first
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
            "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
            "command":"cargo nextest run","status":"inProgress","processId":"42"}}}),
        )
        .await;
    first.acknowledged(cursor).await;
    let submitted = state.record().await["startAttempt"]["connectionId"].clone();
    first_worker.shutdown().await.unwrap();

    // The Studio connection is lost while the supervisor keeps the provider
    // process, its turn, and the command running.
    state
        .bridge
        .run(|store| async move {
            scheduler::lifecycle_transaction_with(&store, |tx| {
                store::disconnect_provider_generation(tx, "default", 1, false)?;
                Ok::<_, WorkerError>(())
            })
            .await
        })
        .await
        .unwrap();
    assert_eq!(state.record().await["status"], "interrupted");

    // A replacement worker attaches to the same provider generation under a
    // new connection id and continues the turn.
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    answer_adopted_turn_check(&mut peer).await;
    worker_barrier(&worker).await;
    let live = worker.retained_session().scope().clone();
    let record = state.record().await;
    assert_eq!(record["status"], "running", "{record}");
    assert_eq!(record["inFlight"], true, "{record}");
    assert_eq!(record["turnId"], "turn", "{record}");
    assert_eq!(
        record["connectionRecovery"]["outcome"], "active",
        "{record}"
    );
    assert_eq!(
        record["startAttempt"]["connectionGeneration"],
        live.generation
    );
    assert_eq!(record["startAttempt"]["connectionId"], submitted);
    assert_ne!(
        submitted,
        json!(live.connection),
        "the connection id changed"
    );

    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, worker.retained_session()),
        None,
    ));
    let target = state.agent.clone();
    let action = match ending {
        Ending::Stop => Action::Stop {
            target,
            descendants: false,
            reason: None,
            sender: None,
            sender_epoch: None,
        },
        Ending::Delete => Action::Delete {
            target,
            request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
        },
    };
    let mut stop = tokio::spawn(async move { ports.execute(&action, 20.0).await });

    let interrupt = tokio::select! {
        wire = peer.next_write() => wire,
        result = &mut stop => panic!("the stop finished without a provider request: {result:?}"),
    };
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    assert_eq!(
        interrupt.value["params"],
        json!({"threadId":"fake-thread-0","turnId":"turn"})
    );
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&interrupt.value, json!({})).await;

    // The interrupt leaves the thread's exec processes running; the stop ends them.
    let clean = tokio::select! {
        wire = peer.next_write() => wire,
        result = &mut stop => panic!("the stop finished without ending the turn's commands: {result:?}"),
    };
    assert_eq!(clean.value["method"], "thread/backgroundTerminals/clean");
    assert_eq!(clean.value["params"], json!({"threadId":"fake-thread-0"}));
    let _ignored = clean.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&clean.value, json!({})).await;
    let result = stop.await.unwrap().expect("the action commits");
    if ending == Ending::Delete {
        assert_eq!(result["stopped"], "unconfirmed");
    }

    let record = state.record().await;
    // A stop only interrupts the turn; a delete also pauses the chat.
    assert_eq!(
        record["status"],
        if ending == Ending::Stop {
            "running"
        } else {
            "paused"
        },
        "{record}"
    );
    assert_eq!(
        studio_runtime_store::truthy(record.get("deletedAt")),
        ending == Ending::Delete,
        "{record}"
    );
    let receipt = &record["startAttempt"]["interrupt"];
    assert_eq!(receipt["outcome"], "accepted", "{record}");
    assert_eq!(receipt["turnId"], "turn", "{record}");
    assert!(peer.writes.try_recv().is_err(), "each request is sent once");
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_after_worker_reattach_interrupts_the_turn_and_ends_its_commands() {
    ending_after_worker_reattach_ends_the_turn_work(Ending::Stop).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_after_worker_reattach_interrupts_the_turn_and_ends_its_commands() {
    ending_after_worker_reattach_ends_the_turn_work(Ending::Delete).await;
}

async fn deletion_with_refused_stop_work(method: studio_runtime_providers::Method) {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = peer
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
        "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
        "command":"fixture command","status":"inProgress","processId":"42"}}}),
        )
        .await;
    peer.acknowledged(cursor).await;
    let session = RefusingSession {
        worker: WorkerSession {
            port: worker.retained_session(),
        },
        method,
    };
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session_over(&state, Some(Arc::new(session))),
        None,
    ));
    let target = state.agent.clone();
    let mut delete = tokio::spawn(async move {
        ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                },
                20.0,
            )
            .await
    });
    if method != studio_runtime_providers::Method::TurnInterrupt {
        let interrupt = peer.next_write().await;
        assert_eq!(interrupt.value["method"], "turn/interrupt");
        let _ignored = interrupt.receipt.send(CommandReceipt::Written {
            written_at: None,
            remote_id: None,
            duplicate: false,
        });
        peer.reply(&interrupt.value, json!({})).await;
    }
    let result = if method == studio_runtime_providers::Method::TurnInterrupt {
        tokio::select! {
            wire = peer.next_write() => panic!("cleanup before interrupt confirmation: {}", wire.value),
            result = &mut delete => result.unwrap().expect("deletion commits"),
        }
    } else {
        delete.await.unwrap().expect("deletion commits")
    };
    assert_eq!(result["stopped"], "unconfirmed");
    assert!(
        peer.writes.try_recv().is_err(),
        "no command cleanup was sent"
    );
    assert!(studio_runtime_store::truthy(
        state.record().await.get("deletedAt")
    ));
    if method == studio_runtime_providers::Method::TurnInterrupt {
        // The provider can start another command until the interrupt is accepted.
        let cursor = peer.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn",
            "item":{"id":"after-refusal","type":"commandExecution","command":"fixture","status":"inProgress","processId":"43"}}})).await;
        peer.acknowledged(cursor).await;
    }
    worker.shutdown().await.unwrap();

    // Reattachment reuses the saved operation IDs: the accepted operation is
    // replayed, while the request refused before admission is sent once.
    state
        .bridge
        .faults
        .startup_recovery_done
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let retry = tokio::select! {
        wire = peer.next_write() => wire,
        () = worker_barrier(&worker) => panic!("stop recovery did not send the refused operation"),
    };
    let expected = if method == studio_runtime_providers::Method::TurnInterrupt {
        "turn/interrupt"
    } else {
        "thread/backgroundTerminals/clean"
    };
    assert_eq!(retry.value["method"], expected);
    let _ignored = retry.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&retry.value, json!({})).await;
    if method == studio_runtime_providers::Method::TurnInterrupt {
        let cleanup = peer.next_write().await;
        assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
        let _ignored = cleanup.receipt.send(CommandReceipt::Written {
            written_at: None,
            remote_id: None,
            duplicate: false,
        });
        peer.reply(&cleanup.value, json!({})).await;
    }
    worker_barrier(&worker).await;
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Unconfirmed
    );
    assert!(
        peer.writes.try_recv().is_err(),
        "the accepted operation was not resent"
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn deletion_reports_success_when_terminal_cleanup_is_not_sent() {
    deletion_with_refused_stop_work(
        studio_runtime_providers::Method::ThreadBackgroundTerminalsClean,
    )
    .await;
}

#[tokio::test]
async fn deletion_reports_success_when_interrupt_is_not_sent() {
    deletion_with_refused_stop_work(studio_runtime_providers::Method::TurnInterrupt).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_interrupt_confirmation_cleans_without_reattach() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let timeout = Arc::new(tokio::sync::Notify::new());
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session_over(
            &state,
            Some(Arc::new(TimedOutInterrupt {
                worker: WorkerSession {
                    port: worker.retained_session(),
                },
                timeout: Arc::clone(&timeout),
                method: studio_runtime_providers::Method::TurnInterrupt,
            })),
        ),
        None,
    ));
    let target = state.agent.clone();
    let deletion = tokio::spawn(async move {
        ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                },
                20.0,
            )
            .await
    });
    let interrupt = peer.next_write().await;
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    timeout.notify_one();
    assert_eq!(deletion.await.unwrap().unwrap()["stopped"], "unconfirmed");
    peer.reply(&interrupt.value, json!({})).await;
    let cleanup = peer.next_write().await;
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    state
        .wait_record(|record| record["startAttempt"]["interrupt"]["outcome"] == "accepted")
        .await;
    assert!(peer.writes.try_recv().is_err());
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_cleanup_confirmation_settles_without_reattach() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = peer.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn",
        "item":{"id":"late-command","type":"commandExecution","command":"fixture","status":"inProgress","processId":"42"}}})).await;
    peer.acknowledged(cursor).await;
    let timeout = Arc::new(tokio::sync::Notify::new());
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session_over(
            &state,
            Some(Arc::new(TimedOutInterrupt {
                worker: WorkerSession {
                    port: worker.retained_session(),
                },
                timeout: Arc::clone(&timeout),
                method: studio_runtime_providers::Method::ThreadBackgroundTerminalsClean,
            })),
        ),
        None,
    ));
    let target = state.agent.clone();
    let deletion = tokio::spawn(async move {
        ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                },
                20.0,
            )
            .await
    });
    let interrupt = peer.next_write().await;
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&interrupt.value, json!({})).await;
    let cleanup = peer.next_write().await;
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    timeout.notify_one();
    assert_eq!(deletion.await.unwrap().unwrap()["stopped"], "unconfirmed");
    peer.reply(&cleanup.value, json!({})).await;
    state
        .wait_record(|record| record["startAttempt"]["interrupt"]["outcome"] == "accepted")
        .await;
    assert!(peer.writes.try_recv().is_err());
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Unconfirmed
    );
    let cursor = peer.frame(json!({"method":"item/completed","params":{"threadId":"fake-thread-0","turnId":"turn",
        "item":{"id":"late-command","type":"commandExecution","command":"fixture","status":"completed","processId":"42","aggregatedOutput":"","exitCode":0}}})).await;
    peer.acknowledged(cursor).await;
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn disconnected_late_interrupt_confirmation_cleans_without_another_reattach() {
    let state = State::new().await;
    let mut first = Peer::new();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    first_worker.shutdown().await.unwrap();
    state
        .bridge
        .run(|store| async move {
            scheduler::lifecycle_transaction_with(&store, |tx| {
                store::disconnect_provider_generation(tx, "default", 1, false)?;
                Ok::<_, WorkerError>(())
            })
            .await
        })
        .await
        .unwrap();
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        detached_account_session(&state),
        None,
    ));
    let result = ports
        .execute(
            &Action::Delete {
                target: state.agent.clone(),
                request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
            },
            20.0,
        )
        .await
        .unwrap();
    assert_eq!(result["stopped"], "unconfirmed");
    state
        .bridge
        .faults
        .startup_recovery_done
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let interrupt = peer.next_write().await;
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(11)).await;
    tokio::time::resume();
    worker_barrier(&worker).await;
    peer.reply(&interrupt.value, json!({})).await;
    let cleanup = peer.next_write().await;
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    state
        .wait_record(|record| {
            record["disconnectRecovery"]["stopInterrupt"]["outcome"] == "accepted"
        })
        .await;
    assert!(peer.writes.try_recv().is_err());
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deletion_lost_interrupt_reply_stays_unconfirmed_without_resending() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let session = LostInterruptReply(WorkerSession {
        port: worker.retained_session(),
    });
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session_over(&state, Some(Arc::new(session))),
        None,
    ));
    let target = state.agent.clone();
    let delete = tokio::spawn(async move {
        ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                },
                20.0,
            )
            .await
    });
    let wire = peer.next_write().await;
    assert_eq!(wire.value["method"], "turn/interrupt");
    let _ignored = wire.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&wire.value, json!({})).await;
    assert_eq!(delete.await.unwrap().unwrap()["stopped"], "unconfirmed");
    assert_eq!(
        state.record().await["startAttempt"]["interrupt"]["outcome"],
        "unknown"
    );
    worker.shutdown().await.unwrap();
    state
        .bridge
        .faults
        .startup_recovery_done
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let cleanup = peer.next_write().await;
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    worker_barrier(&worker).await;
    assert!(
        peer.writes.try_recv().is_err(),
        "the written interrupt is not resent"
    );
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/delete-outcome", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_settled_turn_cleans_its_running_background_command() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = peer
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
        "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
        "command":"fixture command","status":"inProgress","processId":"42"}}}),
        )
        .await;
    peer.acknowledged(cursor).await;
    let cursor = peer
        .frame(
            json!({"method":"turn/completed","params":{"threadId":"fake-thread-0",
        "turn":{"id":"turn","status":"completed"}}}),
        )
        .await;
    peer.acknowledged(cursor).await;
    assert_eq!(state.record().await["lastCompletedTurn"], "turn");
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, worker.retained_session()),
        None,
    ));
    let target = state.agent.clone();
    let request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let deleting_ports = ports.clone();
    let deleting_request = request_id.clone();
    let mut delete = tokio::spawn(async move {
        deleting_ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: deleting_request,
                },
                20.0,
            )
            .await
    });
    let cleanup = tokio::select! {
        wire = peer.next_write() => wire,
        result = &mut delete => panic!("delete returned without ending the background command: {result:?}"),
    };
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    assert_eq!(delete.await.unwrap().unwrap()["stopped"], "unconfirmed");
    let epoch = state.record().await["epoch"].clone();
    let replay = ports
        .execute(
            &Action::Delete {
                target: state.agent.clone(),
                request_id,
            },
            20.0,
        )
        .await
        .unwrap();
    assert_eq!(replay["stopped"], "unconfirmed");
    assert_eq!(
        state.record().await["epoch"],
        epoch,
        "same-request replay must not mutate the deletion"
    );
    assert!(
        peer.writes.try_recv().is_err(),
        "the same deletion did not resend cleanup"
    );
    let task_id = format!("{}:exec-1", state.agent);
    let task_record = || {
        let store = state.bridge.store.clone();
        let id = task_id.clone();
        async move {
            store
                .read_async("test/deleted-command-record", move |conn| {
                    let record = conn.query_row(
                        "SELECT record FROM runtime_tasks WHERE id=?1",
                        [id],
                        |row| row.get::<_, String>(0),
                    )?;
                    let count =
                        conn.query_row("SELECT count(*) FROM runtime_tasks", [], |row| {
                            row.get::<_, i64>(0)
                        })?;
                    Ok::<_, studio_runtime_store::StoreError>((record, count))
                })
                .await
                .unwrap()
        }
    };
    let before = task_record().await;
    let unrelated = json!({"method":"item/completed","params":{"threadId":"fake-thread-0", "turnId":"turn",
        "item":{"id":"unrelated","type":"commandExecution","command":"other","status":"completed","processId":"99","aggregatedOutput":"","exitCode":0}}});
    let cursor = peer.frame(unrelated).await;
    peer.acknowledged(cursor).await;
    assert_eq!(
        task_record().await,
        before,
        "unrelated events do not change the deleted command"
    );
    let completion = json!({"method":"item/completed","params":{"threadId":"fake-thread-0", "turnId":"turn",
        "item":{"id":"exec-1","type":"commandExecution","command":"fixture command","status":"completed","processId":"42","aggregatedOutput":"","exitCode":0}}});
    let cursor = peer.frame(completion.clone()).await;
    peer.acknowledged(cursor).await;
    let completed = task_record().await;
    assert_ne!(
        completed, before,
        "the matching completion closes the command"
    );
    let cursor = peer.frame(completion).await;
    peer.acknowledged(cursor).await;
    assert_eq!(
        task_record().await,
        completed,
        "duplicate completion is idempotent"
    );
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/deleted-command-ended", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deleted_command_without_process_id_accepts_its_completion() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let item = json!({"id":"no-process","type":"commandExecution","command":"fixture","status":"inProgress"});
    let cursor = peer.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn","item":item}})).await;
    peer.acknowledged(cursor).await;
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, worker.retained_session()),
        None,
    ));
    let target = state.agent.clone();
    let deletion = tokio::spawn(async move {
        ports
            .execute(
                &Action::Delete {
                    target,
                    request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                },
                20.0,
            )
            .await
    });
    for expected in ["turn/interrupt", "thread/backgroundTerminals/clean"] {
        let wire = peer.next_write().await;
        assert_eq!(wire.value["method"], expected);
        let _ignored = wire.receipt.send(CommandReceipt::Written {
            written_at: None,
            remote_id: None,
            duplicate: false,
        });
        peer.reply(&wire.value, json!({})).await;
    }
    assert_eq!(deletion.await.unwrap().unwrap()["stopped"], "unconfirmed");
    let cursor = peer.frame(json!({"method":"item/completed","params":{"threadId":"fake-thread-0","turnId":"turn",
        "item":{"id":"no-process","type":"commandExecution","command":"fixture","status":"completed","aggregatedOutput":"","exitCode":0}}})).await;
    peer.acknowledged(cursor).await;
    let ids = vec![state.agent.clone()];
    let outcome = state
        .bridge
        .store
        .read_async("test/no-pid-stop", move |conn| {
            crate::commands::deletion_stop_outcome(conn, &ids)
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        studio_runtime_schema::command::StopOutcome::Confirmed
    );
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_interrupted_turn_does_not_send_a_second_cleanup() {
    let state = State::new().await;
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = peer.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn",
        "item":{"id":"exec-1","type":"commandExecution","command":"fixture","status":"inProgress","processId":"42"}}})).await;
    peer.acknowledged(cursor).await;
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, worker.retained_session()),
        None,
    ));
    let target = state.agent.clone();
    let request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let actor = ports.clone();
    let root = request_id.clone();
    let mut deletion = tokio::spawn(async move {
        actor
            .execute(
                &Action::Delete {
                    target,
                    request_id: root,
                },
                20.0,
            )
            .await
    });
    let interrupt = peer.next_write().await;
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&interrupt.value, json!({})).await;
    let cleanup = peer.next_write().await;
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    // Model the turn completing before the cleanup reply is settled.
    let agent = state.agent.clone();
    state
        .bridge
        .store
        .transaction(move |tx| {
            tx.execute(
                "UPDATE turns SET state='completed' WHERE chat=?1 AND provider_turn='turn'",
                [agent],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    tokio::select! {
        wire = peer.next_write() => panic!("the same command cleanup was sent twice: {}", wire.value),
        result = &mut deletion => { assert_eq!(result.unwrap().unwrap()["stopped"], "unconfirmed"); }
    }
    assert!(peer.writes.try_recv().is_err());
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_delete_stops_settled_background_commands_once() {
    let state = State::new().await;
    state
        .bridge
        .store
        .set_local_server_id(Some("owner".into()))
        .unwrap();
    let project = state.root.to_string_lossy().into_owned();
    let project_id = project.clone();
    let agent_id = state.agent.clone();
    state.bridge.store.transaction(move |tx| {
        studio_runtime_store::ProjectsRepository::put_project(tx, &json!({
            "id":project_id,"path":project_id,"name":"Project","created":1,"peerTeams":[],
            "homeServerId":"owner","locations":[{"serverId":"owner","path":project_id,"projectId":project_id}]
        }))?;
        tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.projectId',?1,'$.projectServerId','owner','$.cwd',?1) WHERE id=?2", rusqlite::params![project_id,agent_id])?;
        Ok(())
    }).await.unwrap();
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let start = state.start(&mut peer, &worker).await;
    peer.accepted(&start).await;
    let cursor = peer.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn",
        "item":{"id":"exec-1","type":"commandExecution","command":"fixture","status":"inProgress","processId":"42"}}})).await;
    peer.acknowledged(cursor).await;
    let cursor = peer.frame(json!({"method":"turn/completed","params":{"threadId":"fake-thread-0","turn":{"id":"turn","status":"completed"}}})).await;
    peer.acknowledged(cursor).await;
    let (seq, workspace) = state
        .bridge
        .store
        .read_async("test/project-version", {
            let project = project.clone();
            move |conn| {
                Ok::<_, studio_runtime_store::StoreError>((
                    conn.query_row(
                        "SELECT seq FROM sync_entities WHERE collection='project' AND id=?1",
                        [project],
                        |row| row.get::<_, i64>(0),
                    )?,
                    studio_runtime_store::SyncDocumentsRepository::workspace_id(conn)?,
                ))
            }
        })
        .await
        .unwrap();
    let request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let request = crate::project_deletion::Request {
        id: project,
        request_id,
        expected_workspace_id: workspace,
        expected_project_seq: seq,
    };
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, worker.retained_session()),
        None,
    ));
    let store = state.bridge.store.clone();
    let deleting_ports = ports.clone();
    let deleting_request = request.clone();
    let mut deletion = tokio::spawn(async move {
        crate::project_deletion::execute(
            store,
            deleting_request,
            crate::event_write::unix_time().unwrap(),
            deleting_ports,
        )
        .await
    });
    let cleanup = tokio::select! {
        wire = peer.next_write() => wire,
        result = &mut deletion => match result {
            Ok(Ok(reply)) => panic!("project delete returned without background cleanup: {}", json!(reply)),
            Ok(Err(error)) => panic!("project delete fixture failed before cleanup: {error:?}"),
            Err(error) => panic!("project delete task failed: {error}"),
        },
    };
    assert_eq!(cleanup.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = cleanup.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&cleanup.value, json!({})).await;
    let reply = deletion.await.unwrap().unwrap();
    let reply = json!(reply);
    assert_eq!(reply["stopped"], "unconfirmed");
    let replay = crate::project_deletion::execute(
        state.bridge.store.clone(),
        request,
        crate::event_write::unix_time().unwrap(),
        ports,
    )
    .await
    .unwrap();
    assert_eq!(json!(replay)["stopped"], "unconfirmed");
    assert!(
        peer.writes.try_recv().is_err(),
        "the same project deletion does not resend cleanup"
    );
    worker.shutdown().await.unwrap();
}

/// The production account session while no account worker is attached.
fn detached_account_session(state: &State) -> Arc<AccountSessionRuntime> {
    account_session_over(state, None)
}

/// The owner ends the agent while its Studio connection is lost and no worker
/// has reattached yet. The provider process keeps the turn and its command
/// running. The worker that reattaches to that provider generation must end
/// them once, and must not continue or restart the stopped agent.
async fn ending_before_worker_reattach_ends_the_turn_work(ending: Ending, refuse_cleanup: bool) {
    let state = State::new().await;
    let mut first = Peer::new();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    let cursor = first
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
            "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
            "command":"cargo nextest run","status":"inProgress","processId":"42"}}}),
        )
        .await;
    first.acknowledged(cursor).await;
    first_worker.shutdown().await.unwrap();
    state
        .bridge
        .run(|store| async move {
            scheduler::lifecycle_transaction_with(&store, |tx| {
                store::disconnect_provider_generation(tx, "default", 1, false)?;
                Ok::<_, WorkerError>(())
            })
            .await
        })
        .await
        .unwrap();
    assert_eq!(state.record().await["status"], "interrupted");

    // No worker is attached: the action commits without a provider request.
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        detached_account_session(&state),
        None,
    ));
    let target = state.agent.clone();
    let action = match ending {
        Ending::Stop => Action::Stop {
            target,
            descendants: false,
            reason: None,
            sender: None,
            sender_epoch: None,
        },
        Ending::Delete => Action::Delete {
            target,
            request_id: studio_runtime_schema::persistence::RequestIdentity::issue_root(),
        },
    };
    let result = ports
        .execute(&action, 20.0)
        .await
        .expect("the action commits");
    if ending == Ending::Delete {
        assert_eq!(result["stopped"], "unconfirmed");
    }
    let stopped = state.record().await;
    assert_eq!(stopped["status"], "paused", "{stopped}");

    // A worker reattaches to the same provider generation, where the turn runs.
    // Its startup barrier must not count the first worker's finished pass.
    let fresh_pass = || {
        state
            .bridge
            .faults
            .startup_recovery_done
            .store(false, std::sync::atomic::Ordering::SeqCst)
    };
    fresh_pass();
    let mut peer = Peer::new();
    let mut worker = peer.worker(state.bridge.clone()).await;
    let interrupt = tokio::select! {
        wire = peer.next_write() => wire,
        () = worker_barrier(&worker) => panic!(
            "the reattached worker left the stopped turn running: {}",
            state.record().await["disconnectRecovery"]
        ),
    };
    assert_eq!(
        interrupt.value["method"], "turn/interrupt",
        "{}",
        interrupt.value
    );
    assert_eq!(
        interrupt.value["params"],
        json!({"threadId":"fake-thread-0","turnId":"turn"})
    );
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&interrupt.value, json!({})).await;
    let clean = peer.next_write().await;
    assert_eq!(clean.value["method"], "thread/backgroundTerminals/clean");
    assert_eq!(clean.value["params"], json!({"threadId":"fake-thread-0"}));
    if refuse_cleanup {
        let _ignored = clean.receipt.send(CommandReceipt::NotSent);
        worker_barrier(&worker).await;
        let record = state.record().await;
        assert_ne!(
            record["disconnectRecovery"]["stopInterrupt"]["outcome"], "accepted",
            "cleanup was refused: {record}"
        );
        let ids = vec![state.agent.clone()];
        let outcome = state
            .bridge
            .store
            .read_async("test/disconnected-delete-outcome", move |conn| {
                crate::commands::deletion_stop_outcome(conn, &ids)
            })
            .await
            .unwrap();
        assert_eq!(
            outcome,
            studio_runtime_schema::command::StopOutcome::Unconfirmed
        );
        worker.shutdown().await.unwrap();
        return;
    }
    let _ignored = clean.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&clean.value, json!({})).await;
    worker_barrier(&worker).await;

    let record = state.record().await;
    assert_eq!(record["status"], "paused", "{record}");
    assert!(
        !studio_runtime_store::truthy(record.get("autoWake")),
        "the stopped agent does not wake: {record}"
    );
    assert_eq!(
        record["disconnectRecovery"]["stopInterrupt"]["outcome"], "accepted",
        "{record}"
    );
    assert!(peer.writes.try_recv().is_err(), "each request is sent once");
    worker.shutdown().await.unwrap();

    // A later worker of the same generation finds the turn already ended.
    fresh_pass();
    let mut next = Peer::new();
    let mut next_worker = next.worker(state.bridge.clone()).await;
    worker_barrier(&next_worker).await;
    assert!(
        next.writes.try_recv().is_err(),
        "an ended turn is not interrupted again"
    );
    next_worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_before_worker_reattach_interrupts_the_turn_and_ends_its_commands() {
    ending_before_worker_reattach_ends_the_turn_work(Ending::Stop, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_before_worker_reattach_interrupts_the_turn_and_ends_its_commands() {
    ending_before_worker_reattach_ends_the_turn_work(Ending::Delete, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnected_delete_refused_cleanup_remains_unconfirmed() {
    ending_before_worker_reattach_ends_the_turn_work(Ending::Delete, true).await;
}

/// The registry broker while the account has no live worker: it has no
/// current scope, and it sends nothing.
struct NoLiveWorker;

impl RetainedAccountSession for NoLiveWorker {
    fn current_scope(&self, _account: &str) -> Option<ConnectionScope> {
        None
    }

    fn request(&self, request: ProviderRequest) -> SessionFuture {
        Box::pin(async move {
            Ok(SessionReceipt {
                scope: request.expected_scope.unwrap_or(ConnectionScope {
                    account: request.account,
                    connection: String::new(),
                    generation: 0,
                }),
                operation: request.operation,
                replayed: false,
                outcome: ReceiptOutcome::NoLiveAccountWorker,
            })
        })
    }
}

/// Where the stop finds no worker for its interrupt.
#[derive(Clone, Copy)]
enum StopWindow {
    /// The running server's account session: the stop's request is bound to
    /// a scope that has no live worker, so its outcome stays unknown.
    NoLiveWorker,
    /// No account session at all: the request is proven unsent.
    NoSession,
}

/// The account worker ends while the process supervisor keeps the provider,
/// so the turn stays running and has no disconnect receipt. The owner stops
/// the agent before a worker reattaches: no worker can send the stop's
/// interrupt. The worker that reattaches to the same provider generation must
/// still end that turn and its commands once, under the stop's own request
/// identities.
async fn stop_while_a_retained_provider_has_no_worker_ends_its_turn_once(window: StopWindow) {
    let state = State::new().await;
    let mut first = Peer::supervised();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    let cursor = first
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
            "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
            "command":"cargo nextest run","status":"inProgress","processId":"42"}}}),
        )
        .await;
    first.acknowledged(cursor).await;
    worker_barrier(&first_worker).await;
    first_worker.shutdown().await.unwrap();
    let live = state.record().await;
    assert_eq!(
        live["status"], "running",
        "the retained turn runs on: {live}"
    );
    assert!(
        live.get("disconnectRecovery").is_none_or(Value::is_null),
        "{live}"
    );

    let session = match window {
        StopWindow::NoLiveWorker => account_session_over(&state, Some(Arc::new(NoLiveWorker))),
        StopWindow::NoSession => detached_account_session(&state),
    };
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        session,
        None,
    ));
    ports
        .execute(
            &Action::Pause {
                target: state.agent.clone(),
                descendants: false,
                reason: None,
                sender: None,
                sender_epoch: None,
            },
            20.0,
        )
        .await
        .expect("the stop commits");
    let stopped = state.record().await;
    assert_eq!(stopped["status"], "paused", "{stopped}");
    let staged = stopped["startAttempt"]["interrupt"].clone();
    assert_eq!(
        staged["outcome"],
        match window {
            StopWindow::NoLiveWorker => "unknown",
            StopWindow::NoSession => "unsent",
        },
        "{stopped}"
    );

    let fresh_pass = || {
        state
            .bridge
            .faults
            .startup_recovery_done
            .store(false, std::sync::atomic::Ordering::SeqCst)
    };
    fresh_pass();
    let mut peer = Peer::supervised();
    peer.cursor = cursor;
    let mut worker = peer.worker(state.bridge.clone()).await;
    let interrupt = tokio::select! {
        wire = peer.next_write() => wire,
        () = worker_barrier(&worker) => {
            let record = state.record().await;
            panic!(
                "the reattached worker left the stopped turn running: status {} inFlight {} interrupt {}",
                record["status"], record["inFlight"], record["startAttempt"]["interrupt"]
            )
        }
    };
    assert_eq!(
        interrupt.value["method"], "turn/interrupt",
        "{}",
        interrupt.value
    );
    assert_eq!(
        interrupt.value["params"],
        json!({"threadId":"fake-thread-0","turnId":"turn"})
    );
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&interrupt.value, json!({})).await;
    let clean = peer.next_write().await;
    assert_eq!(clean.value["method"], "thread/backgroundTerminals/clean");
    assert_eq!(clean.value["params"], json!({"threadId":"fake-thread-0"}));
    let _ignored = clean.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&clean.value, json!({})).await;
    worker_barrier(&worker).await;
    let record = state.record().await;
    assert_eq!(record["status"], "paused", "{record}");
    assert!(
        !studio_runtime_store::truthy(record.get("autoWake")),
        "the stopped agent does not wake: {record}"
    );
    let settled = &record["startAttempt"]["interrupt"];
    assert_eq!(settled["outcome"], "accepted", "{record}");
    assert_eq!(
        settled["requestTicket"], staged["requestTicket"],
        "the stop's own request identity: {record}"
    );
    assert!(peer.writes.try_recv().is_err(), "each request is sent once");
    worker.shutdown().await.unwrap();

    // A later worker of the same generation finds the turn already ended.
    fresh_pass();
    let mut next = Peer::supervised();
    next.cursor = cursor;
    let mut next_worker = next.worker(state.bridge.clone()).await;
    worker_barrier(&next_worker).await;
    assert!(
        next.writes.try_recv().is_err(),
        "an ended turn is not interrupted again"
    );
    next_worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_while_a_retained_provider_has_no_live_worker_interrupts_its_turn_once() {
    stop_while_a_retained_provider_has_no_worker_ends_its_turn_once(StopWindow::NoLiveWorker).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_without_an_account_session_interrupts_a_retained_turn_once() {
    stop_while_a_retained_provider_has_no_worker_ends_its_turn_once(StopWindow::NoSession).await;
}

/// The stop's interrupt reaches the provider, but its reply is lost when the
/// account worker ends and the supervisor keeps the provider. The stop's
/// outcome stays unknown. The worker that reattaches must not send that
/// interrupt again; it sends only the command cleanup that never left Studio.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proven_unsent_interrupt_reuses_stop_identity_after_reattach() {
    let state = State::new().await;
    let mut first = Peer::supervised();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    let cursor = first.frame(json!({"method":"item/started","params":{"threadId":"fake-thread-0","turnId":"turn","item":{"id":"exec-1","type":"commandExecution","command":"test","status":"inProgress","processId":"42"}}})).await;
    first.acknowledged(cursor).await;
    worker_barrier(&first_worker).await;
    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, first_worker.retained_session()),
        None,
    ));
    let action = Action::Pause {
        target: state.agent.clone(),
        descendants: false,
        reason: None,
        sender: None,
        sender_epoch: None,
    };
    let mut stop = tokio::spawn(async move { ports.execute(&action, 20.0).await });
    let interrupt = tokio::select! { wire = first.next_write() => wire, result = &mut stop => panic!("stop did not dispatch: {result:?}") };
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::NotSent);
    stop.await.unwrap().expect("stop commits");
    assert_eq!(
        state.record().await["startAttempt"]["interrupt"]["outcome"],
        "unsent"
    );
    first_worker.shutdown().await.unwrap();
    state
        .bridge
        .faults
        .startup_recovery_done
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let mut peer = Peer::supervised();
    peer.cursor = cursor;
    let mut worker = peer.worker(state.bridge.clone()).await;
    let retry = tokio::select! { wire = peer.next_write() => wire, () = worker_barrier(&worker) => panic!("known-unsent interrupt was stranded") };
    assert_eq!(retry.value["method"], "turn/interrupt");
    let _ignored = retry.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&retry.value, json!({})).await;
    let clean = peer.next_write().await;
    assert_eq!(clean.value["method"], "thread/backgroundTerminals/clean");
    let _ignored = clean.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    peer.reply(&clean.value, json!({})).await;
    worker_barrier(&worker).await;
    assert_eq!(
        state.record().await["startAttempt"]["interrupt"]["outcome"],
        "accepted"
    );
    assert!(peer.writes.try_recv().is_err());
    worker.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_interrupt_with_a_lost_reply_is_not_sent_again_after_reattach() {
    let state = State::new().await;
    let mut first = Peer::supervised();
    let mut first_worker = first.worker(state.bridge.clone()).await;
    let start = state.start(&mut first, &first_worker).await;
    first.accepted(&start).await;
    let cursor = first
        .frame(
            json!({"method":"item/started","params":{"threadId":"fake-thread-0",
            "turnId":"turn","item":{"id":"exec-1","type":"commandExecution",
            "command":"cargo nextest run","status":"inProgress","processId":"42"}}}),
        )
        .await;
    first.acknowledged(cursor).await;
    worker_barrier(&first_worker).await;

    let ports = Arc::new(ProductionLifecyclePorts::new(
        Arc::clone(&state.bridge.store),
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
        account_session(&state, first_worker.retained_session()),
        None,
    ));
    let action = Action::Pause {
        target: state.agent.clone(),
        descendants: false,
        reason: None,
        sender: None,
        sender_epoch: None,
    };
    let mut stop = tokio::spawn(async move { ports.execute(&action, 20.0).await });
    let interrupt = tokio::select! {
        wire = first.next_write() => wire,
        result = &mut stop => panic!("the stop finished without a provider request: {result:?}"),
    };
    assert_eq!(interrupt.value["method"], "turn/interrupt");
    let _ignored = interrupt.receipt.send(CommandReceipt::Written {
        written_at: None,
        remote_id: None,
        duplicate: false,
    });
    // The worker ends before the reply arrives; the provider stays retained.
    first_worker.shutdown().await.unwrap();
    stop.await.unwrap().expect("the stop commits");
    let stopped = state.record().await;
    assert_eq!(stopped["status"], "paused", "{stopped}");
    assert_eq!(
        stopped["startAttempt"]["interrupt"]["outcome"], "unknown",
        "{stopped}"
    );
    let mut interrupts = 1;
    let mut cleanups = 0;
    while let Ok(wire) = first.writes.try_recv() {
        match wire.value["method"].as_str() {
            Some("turn/interrupt") => interrupts += 1,
            Some("thread/backgroundTerminals/clean") => cleanups += 1,
            _ => {}
        }
    }

    state
        .bridge
        .faults
        .startup_recovery_done
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let mut peer = Peer::supervised();
    peer.cursor = cursor;
    let mut worker = peer.worker(state.bridge.clone()).await;
    loop {
        let wire = tokio::select! {
            wire = peer.next_write() => wire,
            () = worker_barrier(&worker) => break,
        };
        match wire.value["method"].as_str() {
            Some("turn/interrupt") => interrupts += 1,
            Some("thread/backgroundTerminals/clean") => cleanups += 1,
            _ => {}
        }
        let _ignored = wire.receipt.send(CommandReceipt::Written {
            written_at: None,
            remote_id: None,
            duplicate: false,
        });
        peer.reply(&wire.value, json!({})).await;
    }
    while let Ok(wire) = peer.writes.try_recv() {
        match wire.value["method"].as_str() {
            Some("turn/interrupt") => interrupts += 1,
            Some("thread/backgroundTerminals/clean") => cleanups += 1,
            _ => {}
        }
    }
    assert_eq!(interrupts, 1, "a written interrupt is never sent again");
    assert_eq!(cleanups, 0, "cleanup waits for a confirmed interrupt");
    let record = state.record().await;
    assert_eq!(record["status"], "paused", "{record}");
    assert_eq!(
        record["startAttempt"]["interrupt"]["outcome"], "unknown",
        "a lost reply stays unknown: {record}"
    );
    worker.shutdown().await.unwrap();
}
