#![allow(clippy::unwrap_used)]
use super::*;
use std::cell::Cell;
use tokio::sync::oneshot;
thread_local! { static FAULT: Cell<u8> = const { Cell::new(0) }; }
pub(super) fn panic_at(point: u8) {
    FAULT.with(|fault| {
        if fault.get() == point {
            fault.set(0);
            panic!("injected session transition panic {point}");
        }
    });
}
struct Factory(std::sync::Mutex<Option<ProviderAttachment>>);

#[tokio::test(start_paused = true)]
async fn native_codec_is_used_by_session_admission_and_result_correlation() {
    use studio_runtime_providers::codex;
    let stop = Shutdown::default();
    let mut stopped = stop.subscribe();
    let (commands, mut inbox) = mpsc::channel::<AdapterCommand>(16);
    let (entries, journal) = mpsc::channel(16);
    let (written, mut writes) = mpsc::channel(4);
    let handle = ProviderHandle::for_account("typed-session");
    let (status, observed) = watch::channel(AttachmentStatus {
        handle: handle.clone(),
        open_attempted: true,
        info: None,
        initialization: InitializationStatus::AwaitingReply,
    });
    let task = tokio::spawn(async move {
        let _status = status;
        loop {
            let command = tokio::select! {
                _ = Shutdown::cancelled(&mut stopped) => break,
                command = inbox.recv() => match command { Some(command) => command, None => break },
            };
            match command {
                AdapterCommand::Write {
                    write,
                    reply,
                    observed,
                } => {
                    written.send(write.frame().clone()).await.unwrap();
                    let receipt = if write.frame().method() == Some(Method::Initialize.as_str()) {
                        CommandReceipt::CachedCodexInitialize(codex::InitializeResponse {
                            codex_home: "/fixture".into(),
                            platform_family: "fixture".into(),
                            platform_os: "fixture".into(),
                            user_agent: "fixture/1".into(),
                        })
                    } else {
                        CommandReceipt::Written {
                            remote_id: None,
                            duplicate: false,
                            written_at: None,
                        }
                    };
                    reply.send(receipt).ok().unwrap();
                    observed.await.unwrap();
                }
                AdapterCommand::Ack { cursor, reply } => {
                    reply
                        .send(CommandReceipt::Acknowledged(cursor))
                        .ok()
                        .unwrap();
                }
            }
        }
        TaskExit {
            result: Ok(()),
            cleanup_error: None,
            recovery_needed: None,
            provider_retained: false,
        }
    });
    let factory = Arc::new(Factory(std::sync::Mutex::new(Some(
        ProviderAttachment::from_task(commands, journal, observed, stop, task),
    ))));
    let mut launch = studio_runtime_process::ProviderLaunchSpec::new(
        vec!["fixture".into()],
        BTreeMap::new(),
        "/fixture".into(),
    );
    launch.codec = ProviderCodec::codex();
    let mut session = SessionHandle::open(
        factory,
        handle,
        OpenOptions {
            clock: SessionClock::default(),
            launch,
            expected_generation: None,
        },
        SessionTaskConfig {
            initialize_params: studio_runtime_providers::initialize_params(
                studio_runtime_providers::ClientName::AccountSession,
            ),
            generation: Generation(1),
            wire: studio_runtime_providers::ProviderKind::Codex.wire(),
            timeout_ms: 1000,
            initialize_operation: OperationId("typed:init".into()),
            request_id_prefix: None,
            recovered: vec![],
        },
    )
    .await
    .unwrap();
    while session.status.borrow().initialization != InitializationStatus::Ready {
        session.status.changed().await.unwrap();
    }
    assert!(matches!(
        writes.recv().await.unwrap(),
        Frame::CodexClientRequest(_)
    ));
    assert!(matches!(
        writes.recv().await.unwrap(),
        Frame::CodexClientNotification(_)
    ));
    let prepare = SessionControl::prepare;
    let prepared = prepare(
        &session.control,
        RequestSpec {
            id: RequestId::Number(2),
            operation_id: OperationId("typed:models".into()),
            method: Method::ModelList,
            params: studio_json::json!({}),
            thread_id: None,
            lightweight: false,
            timeout_ms: 1000,
        },
    )
    .await
    .unwrap();
    let ticket = session.control.submit(prepared).await.unwrap();
    assert!(
        matches!(writes.recv().await.unwrap(), Frame::CodexClientRequest(request) if request.method() == codex::ClientRequestMethod::ModelList)
    );
    let response = Frame::Response {
        id: ticket.id.clone(),
        fields: Map::new(),
        body: ResponseBody::CodexResult(codex::ClientRequestResult::ModelList(Box::new(
            codex::ModelListResponse {
                data: vec![],
                next_cursor: Some(None),
            },
        ))),
    };
    let (frame, _) =
        (ProviderCodec::codex().decode)(&studio_runtime_providers::encode_line(&response).unwrap())
            .unwrap();
    entries
        .send(JournalEntry {
            received_at_ms: 0,
            boundary_at: None,
            cursor: EventCursor(1),
            generation: Generation(1),
            payload: JournalPayload::Frame(frame),
        })
        .await
        .unwrap();
    loop {
        let event = session.events.recv().await.unwrap();
        if event.outcomes.iter().any(|outcome| matches!(outcome, SessionOutcome::Resolved { ticket: result_ticket, outcome: RequestOutcome::Reply { body: ResponseBody::CodexResult(codex::ClientRequestResult::ModelList(_)), .. } } if result_ticket == &ticket)) { break; }
    }
    session.control.settled(EventCursor(1)).await.unwrap();
    assert!(session.checked_shutdown().await.result.is_ok());
}

#[test]
fn queued_input_keeps_admission_boundary() {
    use studio_runtime_schema::provider_timing::{BoundaryStamp, ProviderBoundaryTiming};
    let accepted = BoundaryStamp {
        boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
        unix_seconds: 1000.,
        monotonic_ns: 600_000_000_000,
    };
    let queued = BoundaryStamp {
        boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
        unix_seconds: 400.,
        monotonic_ns: 0,
    };
    let (owner, _inbox) = mpsc::channel(1);
    let mut prepared = PreparedRequest {
        spec: RequestSpec {
            id: RequestId::Number(1),
            operation_id: OperationId("request".into()),
            method: Method::TurnStart,
            params: studio_json::json!({}),
            thread_id: Some("thread".into()),
            lightweight: false,
            timeout_ms: 1000,
        },
        prepared_at_ms: 1000,
        write: None,
        generation: Generation(7),
        owner,
        boundary_timing: Some(Box::new(ProviderBoundaryTiming {
            accepted: Some(accepted),
            ..Default::default()
        })),
    };
    prepared.queued_input(queued);
    let mut timing = prepared.recovery_record().boundary_timing.unwrap();
    timing.derive_durations();
    assert_eq!(timing.accepted, Some(accepted));
    assert_eq!(timing.enqueued, Some(queued));
    assert_eq!(timing.enqueued_to_accepted_ms, Some(600_000.));
    prepared.boundary_timing.as_mut().unwrap().enqueued = None;
    prepared.queued_input(BoundaryStamp {
        boot_id: Some("00000000-0000-0000-0000-000000000002".parse().unwrap()),
        monotonic_ns: 60_000_000_000,
        ..queued
    });
    let mut timing = prepared.recovery_record().boundary_timing.unwrap();
    timing.derive_durations();
    assert_eq!(timing.accepted, Some(accepted));
    assert_eq!(timing.enqueued_to_accepted_ms, None);
}

#[test]
fn lifecycle_capability_requires_exact_ready_connection_and_boolean_proof() {
    use studio_runtime_process::provider_seam::AttachmentInfo;
    use studio_runtime_schema::provider_lifecycle::CLAUDE_STEER_COMPLETION_RECEIPTS as CAPABILITY;
    let handle = ProviderHandle::for_account("owner");
    let generation = Generation(7);
    let status = SessionStatus {
        initialized_at: None,
        supervisor_write_deduplication: false,
        generation,
        initialization: InitializationStatus::Ready,
        initialize_result: Some(Arc::new(
            studio_json::json!({"capabilities":{CAPABILITY:true}}),
        )),
        barrier_set: false,
        held_entries: 0,
        attachment: AttachmentStatus {
            handle: handle.clone(),
            open_attempted: true,
            // Direct stdio has no supervisor handshake state. SessionState owns readiness.
            initialization: InitializationStatus::AwaitingReply,
            info: Some(AttachmentInfo {
                spawn_started_at: None,
                initialized_at: None,
                handle: handle.clone(),
                generation,
                pid: 42,
                acknowledged: EventCursor(0),
                resumed: true,
            }),
        },
    };
    assert!(status.has_capability(&handle, generation, CAPABILITY));
    assert!(!status.has_capability(
        &ProviderHandle::for_account("other"),
        generation,
        CAPABILITY
    ));
    assert!(!status.has_capability(&handle, Generation(8), CAPABILITY));
    for value in [
        Value::Null,
        studio_json::json!({}),
        studio_json::json!({"capabilities":{CAPABILITY:false}}),
        studio_json::json!({"capabilities":{CAPABILITY:"true"}}),
        studio_json::json!({"capabilities":{CAPABILITY:1}}),
    ] {
        let mut unknown = status.clone();
        unknown.initialize_result = Some(Arc::new(value));
        assert!(!unknown.has_capability(&handle, generation, CAPABILITY));
    }
    for change in 0..7 {
        let mut stale = status.clone();
        match change {
            0 => stale.generation = Generation(8),
            1 => stale.initialization = InitializationStatus::AwaitingReply,
            2 => stale.attachment.open_attempted = false,
            3 => stale.attachment.info.as_mut().unwrap().generation = Generation(8),
            4 => {
                stale.attachment.info.as_mut().unwrap().handle =
                    ProviderHandle::for_account("other")
            }
            5 => stale.attachment.info.as_mut().unwrap().pid = 0,
            _ => stale.initialize_result = None,
        }
        assert!(!stale.has_capability(&handle, generation, CAPABILITY));
    }
}
impl ProviderTaskFactory for Factory {
    fn open(
        &self,
        _: ProviderHandle,
        _: OpenOptions,
        _: Shutdown,
    ) -> studio_runtime_process::provider_seam::OpenFuture<'_> {
        let attachment = self.0.lock().unwrap().take().unwrap();
        Box::pin(async { Ok(attachment) })
    }
}
#[tokio::test(start_paused = true)]
async fn panic_after_take_after_mutation_and_with_outstanding_receipt_retains_evidence() {
    for point in [1, 2, 3, 4] {
        let stop = Shutdown::default();
        let mut stopped = stop.subscribe();
        let (commands, mut inbox) = mpsc::channel::<AdapterCommand>(16);
        let (entries, journal) = mpsc::channel(16);
        let handle = ProviderHandle::for_account("panic");
        let (status, observed) = watch::channel(AttachmentStatus {
            handle: handle.clone(),
            open_attempted: true,
            info: None,
            initialization: InitializationStatus::AwaitingReply,
        });
        let (blocked, waiting) = oneshot::channel();
        let (gone, joined) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _status = status;
            for index in 0.. {
                let command = tokio::select! { _ = Shutdown::cancelled(&mut stopped) => break, command = inbox.recv() => command.unwrap() };
                if let AdapterCommand::Write {
                    reply, observed, ..
                } = command
                {
                    if index == 2 {
                        let _ignored = blocked.send(());
                        Shutdown::cancelled(&mut stopped).await;
                        let _ignored = reply.send(CommandReceipt::Unknown);
                        break;
                    }
                    let result = if index == 0 {
                        CommandReceipt::CachedInitialize(studio_json::json!({}))
                    } else {
                        CommandReceipt::Written {
                            written_at: None,
                            remote_id: None,
                            duplicate: false,
                        }
                    };
                    let _ignored = reply.send(result);
                    let _ignored = observed.await;
                }
            }
            let _ignored = gone.send(());
            TaskExit {
                result: Ok(()),
                cleanup_error: None,
                recovery_needed: None,
                provider_retained: false,
            }
        });
        let factory = Arc::new(Factory(std::sync::Mutex::new(Some(
            ProviderAttachment::from_task(commands, journal, observed, stop, task),
        ))));
        let launch = studio_runtime_process::ProviderLaunchSpec::new(
            vec!["fake".into()],
            BTreeMap::new(),
            "/var/tmp".into(),
        );
        let mut session = SessionHandle::open(
            factory,
            handle,
            OpenOptions {
                clock: SessionClock::default(),
                launch,
                expected_generation: None,
            },
            SessionTaskConfig {
                initialize_params: studio_runtime_providers::initialize_params(
                    studio_runtime_providers::ClientName::AccountSession,
                ),
                generation: Generation(1),
                wire: studio_runtime_providers::ProviderKind::Codex.wire(),
                timeout_ms: 100,
                initialize_operation: OperationId("init".into()),
                request_id_prefix: None,
                recovered: vec![],
            },
        )
        .await
        .unwrap();
        while session.status.borrow().initialization != InitializationStatus::Ready {
            session.status.changed().await.unwrap();
        }
        let prepared = session
            .control
            .prepare(RequestSpec {
                id: RequestId::Number(2),
                operation_id: OperationId("request".into()),
                method: Method::ModelList,
                params: studio_json::json!({}),
                thread_id: None,
                lightweight: false,
                timeout_ms: 100,
            })
            .await
            .unwrap();
        let ticket = session.control.submit(prepared).await.unwrap();
        waiting.await.unwrap();
        if point == 4 {
            let queued = session
                .control
                .prepare(RequestSpec {
                    id: RequestId::Number(3),
                    operation_id: OperationId("unsent".into()),
                    method: Method::ModelList,
                    params: studio_json::json!({}),
                    thread_id: None,
                    lightweight: false,
                    timeout_ms: 100,
                })
                .await
                .unwrap();
            session.control.submit(queued).await.unwrap();
        }
        FAULT.with(|fault| fault.set(point));
        entries
            .send(JournalEntry {
                boundary_at: None,
                received_at_ms: if point == 4 { 200 } else { 1 },
                cursor: EventCursor(1),
                generation: Generation(1),
                payload: JournalPayload::Frame(Frame::Response {
                    id: ticket.id.clone(),
                    body: ResponseBody::Result(studio_json::json!({})),
                    fields: Map::new(),
                }),
            })
            .await
            .unwrap();
        let exit = session.checked_completion().await;
        joined.await.unwrap();
        assert!(matches!(exit.result, Err(SessionTaskError::TaskFailed)));
        assert_eq!(exit.undelivered.len(), 1);
        assert_eq!(
            exit.undelivered[0].journal.as_ref().unwrap().cursor,
            EventCursor(1)
        );
        if point == 4 {
            assert!(exit.undelivered[0].outcomes.iter().any(|o| matches!(o, SessionOutcome::Write { ticket: Some(ticket), outcome: WriteOutcome::NotSent, .. } if ticket.id == RequestId::Number(3))));
        }
        if point == 2 {
            assert!(exit.undelivered[0].outcomes.iter().any(|o| matches!(o, SessionOutcome::Resolved { ticket: t, outcome: RequestOutcome::Reply { late: false, .. } } if *t == ticket)));
        }
        assert!(exit.outcomes.iter().any(|o| matches!(o, SessionOutcome::Write { ticket: Some(t), outcome: WriteOutcome::Unknown, .. } if *t == ticket)));
    }
}
#[tokio::test(start_paused = true)]
async fn closure_panic_unknown_batch_does_not_return_unknown_twice() {
    let shutdown = Shutdown::default();
    let mut stopped = shutdown.subscribe();
    let (commands, mut inbox) = mpsc::channel::<AdapterCommand>(16);
    let (entries, journal) = mpsc::channel(16);
    let handle = ProviderHandle::for_account("panic-unknown");
    let (status, observed) = watch::channel(AttachmentStatus {
        handle: handle.clone(),
        open_attempted: true,
        info: None,
        initialization: InitializationStatus::AwaitingReply,
    });
    let (sent, waiting) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _status = status;
        let command = inbox.recv().await.unwrap();
        if let AdapterCommand::Write {
            reply, observed, ..
        } = command
        {
            reply
                .send(CommandReceipt::Written {
                    written_at: None,
                    remote_id: None,
                    duplicate: false,
                })
                .ok()
                .unwrap();
            observed.await.unwrap();
        }
        sent.send(()).unwrap();
        Shutdown::cancelled(&mut stopped).await;
        TaskExit {
            result: Ok(()),
            cleanup_error: None,
            recovery_needed: None,
            provider_retained: false,
        }
    });
    let factory = Arc::new(Factory(std::sync::Mutex::new(Some(
        ProviderAttachment::from_task(commands, journal, observed, shutdown, task),
    ))));
    let recovered_ticket = RequestTicket {
        generation: 1,
        id: RequestId::Number(44),
        method: Method::ModelList,
    };
    let launch = studio_runtime_process::ProviderLaunchSpec::new(
        vec!["fake".into()],
        BTreeMap::new(),
        "/var/tmp".into(),
    );
    let mut session = SessionHandle::open(
        factory,
        handle,
        OpenOptions {
            clock: SessionClock::default(),
            launch,
            expected_generation: None,
        },
        SessionTaskConfig {
            initialize_params: studio_json::json!({}),
            generation: Generation(1),
            wire: studio_runtime_providers::ProviderKind::Codex.wire(),
            timeout_ms: 1000,
            initialize_operation: OperationId("init".into()),
            request_id_prefix: None,
            recovered: vec![RecoveredRequest {
                boundary_timing: None,
                ticket: recovered_ticket.clone(),
                operation_id: OperationId("r44".into()),
                thread_id: None,
                lightweight: false,
                deadline_ms: 1000,
                unknown: false,
                remote_id: None,
            }],
        },
    )
    .await
    .unwrap();
    waiting.await.unwrap();
    FAULT.with(|f| f.set(2));
    entries
        .send(JournalEntry {
            boundary_at: None,
            cursor: EventCursor(1),
            generation: Generation(1),
            received_at_ms: 1,
            payload: JournalPayload::Frame(Frame::Response {
                id: RequestId::Number(1),
                body: ResponseBody::Error(studio_json::json!({"code":-1,"message":"refused"})),
                fields: Map::new(),
            }),
        })
        .await
        .unwrap();
    let exit = session.checked_completion().await;
    assert!(matches!(exit.result, Err(SessionTaskError::TaskFailed)));
    let mut outcomes = exit.outcomes;
    for e in exit.undelivered {
        outcomes.extend(e.outcomes);
    }
    while let Ok(e) = session.events.try_recv() {
        outcomes.extend(e.outcomes);
    }
    let count=outcomes.iter().filter(|o|matches!(o,SessionOutcome::Resolved {ticket,outcome:RequestOutcome::Unknown} if ticket==&recovered_ticket)).count();
    assert_eq!(
        count, 1,
        "one retained Unknown is emitted by action cleanup and again by final_outcomes"
    );
}

#[path = "../../tests/support/session_outcomes.rs"]
mod outcome_checks;
use outcome_checks::CheckedExit;

/// These paths inspect readiness/eligibility, so copying their payload is a
/// regression even when the resulting wire data remains identical.
#[test]
fn payload_readiness_and_retained_delivery_do_not_clone_before_eligibility() {
    let session = include_str!("../provider_session_task.rs");
    let apply = session
        .split("fn apply_actions(")
        .nth(1)
        .unwrap()
        .split("fn retain_resolution(")
        .next()
        .unwrap();
    assert!(
        !apply.contains("front().cloned()"),
        "readiness must borrow the retained action rather than clone its frame"
    );
    let retained = include_str!("../account_worker/retained/runtime.rs");
    assert!(
        !retained.contains("let value = result.clone();"),
        "the database closure must take ownership of its result"
    );
    assert!(
        !retained.contains("pending.result.clone()"),
        "eligibility must borrow the result until a committed reply has a waiter"
    );
}

#[test]
fn correlated_future_result_is_diagnosed_by_the_session_once() {
    for shutdown_tail in [false, true] {
        use studio_runtime_providers::codex;
        let (mut state, initial) = SessionState::new_codex_with_recovered_and_prefix(
            1,
            0,
            1000,
            studio_runtime_providers::codex_initialize_params(
                studio_runtime_providers::ClientName::AccountSession,
            ),
            vec![],
            None,
        )
        .unwrap();
        let Action::Write(init) = &initial[0] else {
            panic!("initialize")
        };
        state
            .receive_at(
                Frame::Response {
                    id: init.id().unwrap(),
                    fields: Map::new(),
                    body: ResponseBody::CodexResult(codex::ClientRequestResult::Initialize(
                        Box::new(codex::InitializeResponse {
                            codex_home: "/fixture".into(),
                            platform_family: "fixture".into(),
                            platform_os: "fixture".into(),
                            user_agent: "fixture/1".into(),
                        }),
                    )),
                },
                0,
            )
            .unwrap();
        state.mark_initialized_notification_written().unwrap();
        let request = codex::ClientRequest::from_method_params(
            codex::RequestId::Variant2(Box::new(2)),
            codex::ClientRequestMethod::TurnStart,
            &studio_json::json!({"threadId":"t","input":[]}),
        )
        .unwrap();
        let (ticket, _) = state
            .submit_codex(request, 0, 1000, Some("t".into()))
            .unwrap();
        let raw = studio_json::to_vec(&studio_json::json!({"id":ticket.id,"result":{"turn":{"id":"r","status":"inProgress","items":[{"type":"futureItem","id":"i"},{"type":"futureItem","id":"j"},{"type":"futureOther","id":"k"}]}}})).unwrap();
        let frame = studio_runtime_providers::decode_codex_frame(&raw).unwrap();
        let (_, commands) = mpsc::channel(1);
        let (_, priority) = mpsc::channel(1);
        let (_, settled) = mpsc::channel(1);
        let (events, _) = mpsc::channel(1);
        let handle = ProviderHandle::for_account("future-result");
        let (status, _) = watch::channel(SessionStatus {
            generation: Generation(1),
            initialization: InitializationStatus::Ready,
            initialized_at: None,
            initialize_result: None,
            supervisor_write_deduplication: false,
            barrier_set: false,
            held_entries: 0,
            attachment: AttachmentStatus {
                handle: handle.clone(),
                open_attempted: true,
                info: None,
                initialization: InitializationStatus::Ready,
            },
        });
        let mut actor = SessionTask {
            codec: ProviderCodec::codex(),
            state,
            diagnostics: Default::default(),
            clock: SessionClock::default(),
            handle,
            records: BTreeMap::new(),
            commands,
            priority,
            settled,
            events,
            status,
            stop: Shutdown::default(),
            seen: BTreeMap::new(),
            attachment_known: true,
            acknowledged: EventCursor(0),
            ack_target: None,
            barrier_set: false,
            retained: Retained {
                pending: Some(SessionEvent {
                    journal: Some(JournalEntry {
                        received_at_ms: 0,
                        boundary_at: None,
                        cursor: EventCursor(1),
                        generation: Generation(1),
                        payload: JournalPayload::Frame(frame),
                    }),
                    outcomes: vec![],
                }),
                ..Default::default()
            },
        };
        actor.records.insert(
            ticket.id.clone(),
            Record {
                ticket: ticket.clone(),
                operation: OperationId("future:turn".into()),
                thread_id: Some("t".into()),
                lightweight: false,
                send: SendState::Written,
                write: None,
                ready: false,
                unknown_reported: false,
                boundary_timing: None,
            },
        );
        actor.diagnostics.initialize(Some("fixture/1"));
        if shutdown_tail {
            let journal = actor
                .retained
                .pending
                .as_mut()
                .unwrap()
                .journal
                .take()
                .unwrap();
            let outcome = actor
                .tail_reply(&journal)
                .expect("tail reply must correlate");
            actor
                .retained
                .pending
                .as_mut()
                .unwrap()
                .outcomes
                .push(outcome);
        } else {
            actor.process_journal().unwrap();
        }
        let event = actor.retained.pending.as_ref().unwrap();
        let body = event.outcomes.iter().find_map(|outcome| match outcome {
            SessionOutcome::Resolved {
                outcome: RequestOutcome::Reply { body, .. },
                ..
            } => Some(body.clone()),
            SessionOutcome::Write { .. }
            | SessionOutcome::Resolved { .. }
            | SessionOutcome::GenerationClosed { .. } => None,
        });
        let body = body.expect("correlated result must reach the caller");
        let correlated = Frame::Response {
            id: ticket.id,
            fields: Map::new(),
            body,
        };
        assert!(
            actor.diagnostics.observe(&correlated).is_none(),
            "session must already have diagnosed the correlated result"
        );
        let mut independent =
            studio_runtime_process::provider_stdio::CodexMethodDiagnostics::default();
        independent.initialize(Some("fixture/1"));
        let line = independent.observe(&correlated).unwrap();
        let text = String::from_utf8(line).unwrap();
        assert!(text.contains("ThreadItem:futureItem"));
        assert!(text.contains("turn/start"));
        assert!(text.contains("fixture/1"));
        let second = String::from_utf8(independent.observe(&correlated).unwrap()).unwrap();
        assert!(second.contains("ThreadItem:futureOther"));
        assert!(independent.observe(&correlated).is_none());
    }
}
