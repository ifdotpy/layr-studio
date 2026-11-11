//! Carries the durable request's clocks into the existing turn projection when
//! a provider supplies its turn identity. It never resubmits a provider request.
use super::*;
use crate::notification_dispatch::{
    NotificationObservation, NotificationPostCommit, ObservationTime, ProviderDeliveryIdentity,
    SharedJson,
};
use studio_runtime_schema::provider_timing::{BoundaryStamp, ProviderBoundaryTiming};

/// Timing is evidence, never request identity. Preserve the pre-instrumentation
/// signature, including all original request and input metadata.
pub(super) fn submission_signature(
    context: &Pending,
    request: &retained::storage::TransportRequest,
) -> Result<String, WorkerError> {
    let mut context = context.clone();
    for event in &mut context.intent.events {
        if let Some(timing) = event
            .metadata
            .get_mut("timing")
            .and_then(Value::as_object_mut)
        {
            timing.remove(studio_runtime_schema::provider_timing::ACCEPTED_STAMP_FIELD);
        }
    }
    let mut request = request.clone();
    request.boundary_timing = None;
    Ok(studio_runtime_store::sha256_hex(
        &studio_json::to_vec(&json!({"context":context,"request":request}))
            .map_err(StoreError::from)?,
    ))
}

pub(super) fn stamp_effects(effects: &mut [NotificationPostCommit], stamp: Option<BoundaryStamp>) {
    for effect in effects {
        match effect {
            NotificationPostCommit::Analytics { observation, .. }
            | NotificationPostCommit::AnalyticsStream { observation, .. } => {
                observation.boundary_at = stamp
            }
            NotificationPostCommit::AnalyticsCaptured(_)
            | NotificationPostCommit::AnalyticsStreamCaptured(_)
            | NotificationPostCommit::TokenRateStream(_)
            | NotificationPostCommit::TokenRateAssociation(_)
            | NotificationPostCommit::TokenRateUsage(_)
            | NotificationPostCommit::Recovered { .. } => {}
        }
    }
}

/// Compare exactly the previous transport identity while retaining first timing
/// evidence. Supervisor reconciliation can confirm an earlier receipt without
/// supplying its original clock sample.
pub(super) fn merge_receipt(
    current: &mut retained::storage::TransportRequest,
    prior: &retained::storage::TransportRequest,
) -> bool {
    let incoming = current.boundary_timing.take();
    current.boundary_timing = prior.boundary_timing.clone();
    if current != prior {
        return false;
    }
    let mut timing = prior.boundary_timing.clone().unwrap_or_default();
    if let Some(incoming) = incoming {
        timing.merge_first(*incoming);
    }
    current.boundary_timing = (timing != Default::default()).then_some(timing);
    true
}

pub(super) fn seed(
    tx: &rusqlite::Connection,
    context: &Pending,
    turn: &str,
    first_event: Option<BoundaryStamp>,
) -> Result<(), WorkerError> {
    if context.method != Method::TurnStart {
        return Ok(());
    }
    let Some(call) = store::account_submission(tx, &context.operation)? else {
        return Ok(());
    };
    let Some(mut timing) = call
        .written
        .as_ref()
        .and_then(|request| request.boundary_timing.clone())
        .or(call.request.boundary_timing)
    else {
        return Ok(());
    };
    timing.first_event = first_event;
    log(
        tx,
        &context.intent.agent_id,
        context.intent.thread_id.as_deref().unwrap_or(""),
        turn,
        &timing,
    )
}

pub(super) fn seed_started(
    tx: &rusqlite::Connection,
    identity: &studio_runtime_store::TurnAttemptIdentity,
    turn: &str,
) -> Result<(), WorkerError> {
    // The account already keeps the exact operation in the execution attempt.
    let Some(attempt) =
        studio_runtime_store::ExecutionHistoryRepository::attempt(tx, &identity.attempt_id)?
    else {
        return Ok(());
    };
    let Some(operation) = attempt.start.request_ticket else {
        return Ok(());
    };
    let Some(call) = store::account_submission(tx, &operation)? else {
        return Ok(());
    };
    seed(tx, &call.context, turn, None)
}

/// A reply can retire Pending before its stdin receipt reaches the worker.
/// Resolve that exact request from existing execution history, never a live turn.
pub(super) fn late_written(
    tx: &rusqlite::Connection,
    account: &str,
    request: &RecoveredRequest,
) -> Result<(), WorkerError> {
    if request.ticket.method != Method::TurnStart {
        return Ok(());
    }
    let Some(timing) = request.boundary_timing.as_deref().filter(|timing| {
        timing.written.is_some()
            && timing.request_id.as_deref() == Some(request.operation_id.0.as_str())
    }) else {
        return Ok(());
    };
    let Some(thread) = request.thread_id.as_deref() else {
        return Ok(());
    };
    let Some(owner) = studio_runtime_store::ExecutionHistoryRepository::provider_request_turn(
        tx,
        account,
        thread,
        &store::id_string(&request.ticket.id),
        request.ticket.generation,
    )?
    else {
        return Ok(());
    };
    if let Some(turn) = owner.turn {
        log(tx, &owner.agent, thread, &turn, timing)?;
    }
    Ok(())
}

fn log(
    tx: &rusqlite::Connection,
    agent_id: &str,
    thread: &str,
    turn: &str,
    timing: &ProviderBoundaryTiming,
) -> Result<(), WorkerError> {
    let Some(agent) = AgentRepository::get(tx, &AgentId::from(agent_id))? else {
        return Ok(());
    };
    let Some(accepted) = timing.accepted else {
        return Ok(());
    };
    let at = ObservationTime::new(accepted.unix_seconds).ok_or(WorkerError::TaskFailed)?;
    let effect = NotificationPostCommit::Analytics {
        observation: NotificationObservation {
            boundary_at: None,
            agent: Some(SharedJson::new(agent.value())),
            method: "analytics/adapterBoundary".into(),
            params: SharedJson::new(
                json!({"threadId":thread,"turnId":turn,"boundaryTiming":timing}),
            ),
            delivery: ProviderDeliveryIdentity {
                provider_handle: timing.provider_handle.clone().unwrap_or_default(),
                generation: timing.generation.unwrap_or_default(),
                cursor: 0,
            },
            observed_at: at,
            provider_at: at,
        },
        budget_capture_value: None,
        usage_conflict: None,
        ledger_since: Default::default(),
        usage_unknown: false,
    };
    crate::analytics_projection::log_effect(tx, effect)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::retained::storage::TransportRequestExt;
    use super::*;

    #[test]
    fn timing_is_excluded_from_request_identity_and_preserves_unknown_receipts() {
        let mut context = Pending {
            intent: TurnIntent {
                agent_id: "a".into(),
                epoch: 0,
                attempt_id: "attempt".into(),
                account_key: "account".into(),
                provider: studio_runtime_providers::ProviderKind::Codex,
                thread_id: Some("thread".into()),
                cwd: None,
                model: None,
                events: vec![crate::scheduler::TurnEvent {
                    id: "input".into(),
                    kind: "user".into(),
                    text: "hello".into(),
                    created: 10.,
                    epoch: 0,
                    error: None,
                    metadata: json!({"timing":{"enqueuedAt":100}}),
                }],
                radio_authorized: false,
            },
            method: Method::TurnStart,
            operation: "request".into(),
            thread: None,
            compiled: None,
        };
        let mut recovered = RecoveredRequest {
            ticket: studio_runtime_providers::RequestTicket {
                generation: 7,
                id: RequestId::String("request".into()),
                method: Method::TurnStart,
            },
            operation_id: OperationId("request".into()),
            thread_id: Some("thread".into()),
            lightweight: false,
            deadline_ms: 1000,
            unknown: true,
            remote_id: None,
            boundary_timing: None,
        };
        let request = retained::storage::TransportRequest::from(&recovered);
        let baseline = studio_runtime_store::sha256_hex(
            &studio_json::to_vec(&json!({"context":context,"request":request})).unwrap(),
        );
        let stamp = BoundaryStamp {
            boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
            unix_seconds: 10.,
            monotonic_ns: 100,
        };
        context.intent.events[0].metadata["timing"]
            [studio_runtime_schema::provider_timing::ACCEPTED_STAMP_FIELD] = json!(stamp);
        recovered.boundary_timing = Some(Box::new(ProviderBoundaryTiming {
            request_id: Some("request".into()),
            accepted: Some(stamp),
            ..Default::default()
        }));
        let request = retained::storage::TransportRequest::from(&recovered);
        assert_eq!(submission_signature(&context, &request).unwrap(), baseline);
        let restored = request.recovered();
        assert_eq!(restored, recovered);
        assert!(restored.unknown);
        assert!(restored.boundary_timing.unwrap().written.is_none());
        let mut confirmed = request.clone();
        confirmed.boundary_timing.as_mut().unwrap().written = Some(BoundaryStamp {
            boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
            unix_seconds: 11.,
            monotonic_ns: 200,
        });
        confirmed
            .boundary_timing
            .as_mut()
            .unwrap()
            .derive_durations();
        let mut reconciliation = request;
        assert!(merge_receipt(&mut reconciliation, &confirmed));
        assert_eq!(reconciliation, confirmed);
        reconciliation.operation = "different-request".into();
        assert!(!merge_receipt(&mut reconciliation, &confirmed));
    }

    #[test]
    fn journal_timing_does_not_change_frame_bytes_or_digest() {
        let frame = Frame::Notification(studio_runtime_providers::NotificationFrame {
            method: Method::TurnStarted,
            params: Some(json!({"threadId":"thread","turn":{"id":"turn"}})),
            fields: Default::default(),
        });
        let encoded = studio_runtime_providers::encode_line(&frame).unwrap();
        let mut entry = JournalEntry {
            boundary_at: None,
            received_at_ms: 1,
            cursor: studio_runtime_process::EventCursor(1),
            generation: Generation(7),
            payload: JournalPayload::Frame(frame),
        };
        let digest = store::journal_digest(&entry, "event").unwrap();
        entry.boundary_at = Some(BoundaryStamp {
            boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
            unix_seconds: 10.,
            monotonic_ns: 100,
        });
        assert_eq!(store::journal_digest(&entry, "event").unwrap(), digest);
        let JournalPayload::Frame(frame) = &entry.payload else {
            unreachable!()
        };
        assert_eq!(
            studio_runtime_providers::encode_line(frame).unwrap(),
            encoded
        );
    }

    #[test]
    fn malformed_transport_metadata_does_not_change_journal_digest() {
        use studio_runtime_providers::{
            DecodedJournalPayload, decode_journal_entry, decode_journal_entry_borrowed,
            decode_journal_value,
        };
        let raw = br#"{"method":17}"#;
        let plain: Value = studio_json::from_slice(raw).unwrap();
        let mut tagged = plain;
        tagged[studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD] =
            json!(BoundaryStamp {
                boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
                unix_seconds: 10.,
                monotonic_ns: 42
            });
        let encoded = studio_json::to_vec(&tagged).unwrap();
        let entry = |decoded: studio_runtime_providers::DecodedJournalEntry| {
            let DecodedJournalPayload::RawStdout { bytes, error } = decoded.payload else {
                panic!("malformed provider frame")
            };
            JournalEntry {
                boundary_at: decoded.boundary_at,
                received_at_ms: 1,
                cursor: studio_runtime_process::EventCursor(1),
                generation: Generation(7),
                payload: JournalPayload::RawStdout { bytes, error },
            }
        };
        let digest =
            store::journal_digest(&entry(decode_journal_entry_borrowed(1, 7, raw)), "event")
                .unwrap();
        for decoded in [
            decode_journal_value(1, 7, &tagged),
            decode_journal_entry(1, 7, encoded.clone()),
            decode_journal_entry_borrowed(1, 7, &encoded),
        ] {
            assert_eq!(
                store::journal_digest(&entry(decoded), "event").unwrap(),
                digest
            );
        }
    }
}
