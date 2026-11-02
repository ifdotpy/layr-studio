//! Account transfer snapshot, replay and decision behavior.
use serde_json::{Value, json};
use studio_runtime_services::account_transfer::{Action, Command as Transition, Snapshot, Ticket};

fn before_start(value: &Value) -> Result<(), String> {
    let agent = studio_runtime_store::AgentRecord::from_value(value.clone())
        .map_err(|error| error.to_string())?;
    studio_runtime_services::account_transfer::before_start(&agent)
        .map_err(|error| error.to_string())
}

fn payload(value: &Value) -> Box<serde_json::value::RawValue> {
    serde_json::value::to_raw_value(value).expect("provider payload")
}

fn fixture() -> (Value, Value, Ticket) {
    let op = json!({"id":"op","leadId":"a","targetAccountKey":"target","status":"pending","created":1.0,"members":{"a":{"phase":"waiting","sourceAccountKey":"source","lazy":false}}});
    let agent = json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,"id":"a","rootId":"a","epoch":3,"status":"idle","accountKey":"source","accountTransferId":"op"});
    let ticket = Ticket {
        operation: "op".into(),
        agent: "a".into(),
        epoch: 3,
        account: "target".to_owned().try_into().expect("account key"),
        connection: "c1".into(),
        request: "r1".into(),
    };
    (op, agent, ticket)
}
fn snapshot(op: &Value, agent: &Value) -> Snapshot {
    Snapshot::read(&op.to_string(), [("a".into(), agent.to_string())]).expect("snapshot")
}
fn snapshot_record(
    op: &studio_runtime_store::transfer_record::TransferRecord,
    agent: &Value,
) -> Snapshot {
    Snapshot::read(
        &serde_json::to_string(op).expect("operation"),
        [("a".into(), agent.to_string())],
    )
    .expect("snapshot")
}
#[test]
fn transfer_snapshot_accepts_configured_team_ceiling_and_rejects_one_more_record() {
    let (mut op, agent, _) = fixture();
    let max_records = studio_runtime_schema::agent_limits::MAX_TRANSFER_AGENT_RECORDS;
    let records = |op: &mut Value, count: usize| {
        (0..count)
            .map(|index| {
                let id = if index == 0 {
                    "a".to_owned()
                } else {
                    format!("worker-{index}")
                };
                if index > 0 && index < max_records {
                    op["members"][&id] = json!({
                        "phase":"waiting",
                        "sourceAccountKey":"source",
                        "lazy":false
                    });
                }
                let mut record = agent.clone();
                record["id"] = json!(id);
                (id, record.to_string())
            })
            .collect::<Vec<_>>()
    };

    let at_limit = records(&mut op, max_records);
    Snapshot::read(&op.to_string(), at_limit)
        .expect("lead and workers fit the generated transfer member limit");
    let above_limit = records(&mut op, max_records + 1);
    let error = Snapshot::read(&op.to_string(), above_limit)
        .err()
        .expect("a transfer snapshot above the team limit is refused");
    assert_eq!(
        error.to_string(),
        "invalid stored transfer snapshot at /resourceLimit"
    );
}

#[test]
fn exact_native_receipt_survives_restart_without_resubmission() {
    let (op, agent, ticket) = fixture();
    let preparing = snapshot(&op, &agent)
        .begin_read("a")
        .expect("begin preparation");
    let mut submitted = snapshot_record(&preparing.operation, &agent)
        .submit(
            &ticket,
            studio_runtime_store::transfer_record::TransferMethod::Fork,
            &payload(&json!({"threadId":"source"})),
        )
        .expect("reserve");
    submitted
        .operation
        .members
        .get_mut("a")
        .expect("valid fixture")
        .submitted_at = studio_runtime_schema::queue_command::Field::Value(
        serde_json::Number::from_f64(10.0).expect("valid fixture"),
    );
    let restarted = snapshot_record(&submitted.operation, &agent)
        .decide(Transition::Restart)
        .expect("restart");
    assert_eq!(
        restarted
            .operation
            .members
            .get("a")
            .expect("valid fixture")
            .phase,
        studio_runtime_store::transfer_record::TransferPhase::Unknown
    );
    let restored = snapshot_record(&restarted.operation, &agent);
    assert!(
        restored
            .submit(
                &ticket,
                studio_runtime_store::transfer_record::TransferMethod::Fork,
                &payload(&json!({}))
            )
            .is_err()
    );
    let mut foreign = ticket.clone();
    foreign.connection = "c2".into();
    assert!(
        restored
            .typed_receive(&foreign, Some(payload(&json!({"thread":{"id":"fork"}}))))
            .is_err()
    );
    let received = restored
        .typed_receive(&ticket, Some(payload(&json!({"thread":{"id":"fork"}}))))
        .expect("late exact receipt");
    assert_eq!(
        received
            .operation
            .members
            .get("a")
            .expect("valid fixture")
            .phase,
        studio_runtime_store::transfer_record::TransferPhase::Ready
    );
    let saved = snapshot_record(&received.operation, &agent);
    assert!(
        !saved
            .typed_receive(&ticket, Some(payload(&json!({"thread":{"id":"fork"}}))))
            .expect("replay")
            .changed
    );
    assert!(
        saved
            .typed_receive(
                &ticket,
                Some(payload(&json!({"thread":{"id":"different-fork"}})))
            )
            .is_err()
    );
    assert!(before_start(&agent).is_err());
    let mut newer = agent.clone();
    newer["epoch"] = json!(4);
    assert!(
        snapshot_record(&restarted.operation, &newer)
            .typed_receive(&ticket, Some(payload(&json!({"thread":{"id":"fork"}}))))
            .is_err()
    );
}
#[test]
fn malformed_records_fail_without_panicking_or_echoing_values() {
    let (op, agent, _) = fixture();
    for (pointer, value) in [
        ("/members", json!([])),
        ("/members/a/phase", json!("SECRET_INVALID")),
        ("/members/a/lazy", json!("SECRET_INVALID")),
        ("/status", Value::Null),
        ("/created", json!(true)),
    ] {
        let mut invalid = op.clone();
        *invalid
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("fixture path {pointer}")) = value;
        let result = Snapshot::read(&invalid.to_string(), [("a".into(), agent.to_string())]);
        assert!(result.is_err(), "{pointer}");
        assert!(
            !result
                .err()
                .expect("error")
                .to_string()
                .contains("SECRET_INVALID")
        );
    }
}

#[test]
fn subagents_summary_bypasses_only_an_unenrolled_leads_summary_hold() {
    let (_, mut agent, _) = fixture();
    agent
        .as_object_mut()
        .expect("agent")
        .remove("accountTransferId");
    agent["isLead"] = json!(true);
    agent["accountTransfer"] =
        json!({"id":"op", "targetAccountKey":"target", "status":"pending", "scope":"subagents"});
    before_start(&agent).expect("unenrolled lead summary does not hold");
    for (key, marker) in [
        ("accountTransferId", json!("op")),
        ("lazyAccountTransfer", json!({"id":"op"})),
        ("emptyTransferRecovery", json!({"phase":"pending"})),
    ] {
        let mut held = agent.clone();
        held[key] = marker;
        assert!(before_start(&held).is_err(), "summary cannot bypass {key}");
    }
    let mut child = agent.clone();
    child["isLead"] = json!(false);
    assert!(
        before_start(&child).is_err(),
        "a child summary cannot bypass admission"
    );
    for scope in [json!("team"), json!("future-scope"), json!(42), Value::Null] {
        let mut held = agent.clone();
        held["accountTransfer"]["scope"] = scope;
        assert!(
            before_start(&held).is_err(),
            "only exact subagents scope bypasses the summary hold"
        );
    }
    for status in [
        Some(json!("future-status")),
        Some(json!(42)),
        Some(Value::Null),
        None,
    ] {
        let mut held = agent.clone();
        let summary = held["accountTransfer"].as_object_mut().expect("summary");
        match status {
            Some(status) => {
                summary.insert("status".into(), status);
            }
            None => {
                summary.remove("status");
            }
        }
        assert!(
            before_start(&held).is_err(),
            "unknown summary state cannot bypass admission"
        );
    }
}

#[test]
fn unreadable_summary_or_lazy_state_keeps_start_held() {
    let (_, mut agent, _) = fixture();
    agent
        .as_object_mut()
        .expect("valid fixture")
        .remove("accountTransferId");
    agent["accountTransfer"] = json!({"kind":"unreadable","reason":"Unknown transfer field"});
    assert!(before_start(&agent).is_err());
    agent
        .as_object_mut()
        .expect("valid fixture")
        .remove("accountTransfer");
    agent["lazyAccountTransfer"] = json!({"kind":"unreadable","reason":"Unknown lazy field"});
    assert!(before_start(&agent).is_err());
}

#[test]
fn request_replay_is_bound_to_lead_destination_and_scope() {
    let (mut op, agent, _) = fixture();
    op["requests"] =
        json!({"alias":{"leadId":"a","targetAccountKey":"target","scope":"subagents"}});
    let saved = snapshot(&op, &agent);
    assert!(
        saved
            .replay("op", "a", "target", "team")
            .expect("legacy default scope")
    );
    assert!(
        saved
            .replay("alias", "a", "target", "subagents")
            .expect("alias")
    );
    assert!(saved.replay("op", "a", "target", "subagents").is_err());
    assert!(saved.replay("alias", "a", "changed", "subagents").is_err());
    assert!(
        saved
            .replay("alias", "other", "target", "subagents")
            .is_err()
    );
    assert!(!saved.replay("fresh", "a", "target", "team").expect("new"));
}
#[test]
fn selection_rejects_account_published_as_unavailable() {
    let agent = json!({"compactions":0,"provider":"codex","id":"lead","rootId":"lead","epoch":0,"status":"idle","isLead":true,"accountKey":"source","cwd":"/project"});
    let snapshot =
        studio_runtime_services::account_selection::SelectionSnapshot::read(&agent.to_string())
            .unwrap();
    assert!(
        snapshot
            .decide(
                "target",
                Some(&json!({"provider":"codex","status":"error"})),
                "/project",
                true
            )
            .is_err()
    );
}

#[test]
fn selection_decision_keeps_resume_for_atomic_store_cancellation() {
    let agent = json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead","epoch":0,"status":"idle","isLead":true,"accountKey":"source","cwd":"/project","usageResume":{"status":"scheduled"}});
    let snapshot =
        studio_runtime_services::account_selection::SelectionSnapshot::read(&agent.to_string())
            .expect("snapshot");
    let selected = snapshot
        .decide(
            "target",
            Some(&json!({"provider":"codex","status":"ready"})),
            "/project",
            true,
        )
        .expect("store adapter cancels the persisted resume atomically");
    assert_eq!(selected["accountKey"], "target");
    assert_eq!(selected["usageResume"], agent["usageResume"]);
    assert_eq!(selected["usageResume"]["status"], "scheduled");
    assert_eq!(
        snapshot
            .decide("source", None, "/project", true)
            .expect("unchanged account"),
        agent
    );
}

#[test]
fn retry_only_resets_blocked_members() {
    for (lazy, saved, phase) in [
        (false, false, "waiting"),
        (true, false, "lazy"),
        (false, true, "ready"),
    ] {
        let (mut op, agent, _) = fixture();
        op["members"]["a"]["phase"] = json!("ready");
        let ready = op["members"]["a"].clone();
        op["members"]["b"] = json!({"phase":"blocked","sourceAccountKey":"source","lazy":lazy,
            "result":saved.then(|| json!({"thread":{"id":"fork"}})),"error":"retry me","nextCheck":99,
            "interruptTurnId":"turn","interruptSubmittedAt":4,"interruptOutcome":"unknown"});
        let mut blocked_agent = agent.clone();
        blocked_agent["id"] = json!("b");
        if lazy {
            blocked_agent["lazyAccountTransfer"] = json!({"id":"op","sourceAccountKey":"source"});
        }
        let result = Snapshot::read(
            &op.to_string(),
            [
                ("a".into(), agent.to_string()),
                ("b".into(), blocked_agent.to_string()),
            ],
        )
        .expect("snapshot")
        .decide(Transition::Action(Action::Retry))
        .expect("retry");
        assert_eq!(
            serde::Serialize::serialize(
                &(result.operation.members.get("a").expect("valid fixture")),
                serde_json::value::Serializer
            )
            .expect("valid fixture"),
            ready
        );
        let blocked = result.operation.members.get("b").expect("valid fixture");
        assert_eq!(
            serde::Serialize::serialize(&(blocked.phase), serde_json::value::Serializer)
                .expect("valid fixture"),
            phase
        );
        assert!(matches!(
            blocked.error,
            studio_runtime_schema::queue_command::Field::Null
        ));
        assert_eq!(
            blocked.next_check.value().expect("valid fixture").as_i64(),
            Some(0)
        );
        assert!(blocked.interrupt_turn_id.is_missing());
        assert!(blocked.interrupt_submitted_at.is_missing());
        assert!(blocked.interrupt_outcome.is_missing());
    }
}

#[test]
fn cancel_refuses_unresolved_interrupt() {
    let (mut op, agent, _) = fixture();
    op["members"]["a"]["interruptSubmittedAt"] = json!(4.0);
    op["members"]["a"]["interruptOutcome"] = json!("unknown");
    let error = match snapshot(&op, &agent).decide(Transition::Action(Action::Cancel)) {
        Err(error) => error,
        Ok(_) => panic!("unresolved interrupt hold"),
    };
    assert_eq!(
        error.to_string(),
        "Retry the unresolved turn interruption before cancelling this transfer"
    );
}

#[test]
fn cancel_refuses_acknowledged_interrupt_while_turn_is_active() {
    let (mut op, mut agent, _) = fixture();
    op["members"]["a"]["interruptSubmittedAt"] = json!(4.0);
    op["members"]["a"]["interruptOutcome"] = json!("acknowledged");
    agent["inFlight"] = json!(true);
    let error = match snapshot(&op, &agent).decide(Transition::Action(Action::Cancel)) {
        Err(error) => error,
        Ok(_) => panic!("active interruption hold"),
    };
    assert_eq!(
        error.to_string(),
        "Wait for the active turn interruption to finish before cancelling this transfer"
    );
}
