use super::evidence::*;
use super::{AdmittedAgentSource, AgentComponents, AgentSourceSnapshot, HeldAgentSelectors};
use studio_runtime_schema::persistence::HoldStatus;

fn source() -> AgentSourceSnapshot {
    let mut record = crate::AgentRecord::new(
        crate::AgentId::from("a"),
        crate::AgentId::from("a"),
        1,
        crate::AgentStatus::Running,
        "default".into(),
    );
    record.provider = studio_runtime_schema::queue_command::Field::Value(
        studio_runtime_schema::ProviderKind::Codex,
    );
    AgentSourceSnapshot {
        physical_id: Some("a".into()),
        source_rowid: -1,
        record: studio_json::to_string(&record)
            .expect("typed source record")
            .into_boxed_str(),
        hold: HoldStatus::Clear,
        hold_incident: None,
        selectors: HeldAgentSelectors {
            root_id: Some("a".into()),
            parent_id: None,
            account_key: "default".into(),
            provider: "codex".into(),
            role: None,
            deleted_at: DeletedAtEvidence::Null,
            cwd: None,
            workspace_operation: WorkspaceOperationEvidence::Null,
            remote_worker: RemoteWorkerEvidence::Null,
            remote_anchor: RemoteAnchorEvidence::Null,
            native_failure_hold: NativeFailureHoldEvidence::Null,
            steer_rejected_turn_id: None,
            native_tool_refresh_id: None,
            context_repair_phase: None,
            source_cleanup_phase: None,
            source_cleanup_account_key: None,
            source_cleanup_thread_id: None,
            has_disconnect_recovery: false,
            disconnect_recovery_stage: None,
            has_restart_recovery: false,
            restart_recovery_stage: None,
            has_workspace_cleanup_intent: false,
            agent_archived: false,
            review_archive_failed: false,
            review_archive_marked: false,
            review_archive_scheduled: false,
            review_archive_next_at: ReviewArchiveNextAtEvidence::Null,
            idle_episode_type: None,
            idle_episode_fire_at: IdleEpisodeFireAtEvidence::Null,
            idle_episode_id: IdleEpisodeIdEvidence::Null,
            legacy_unsettled: false,
            title_status: None,
            vm_launch_status: None,
            idle_episode_outcome: None,
            provider_stream_generation: ProviderStreamGenerationEvidence::Null,
            provider_stream_ack: ProviderStreamAckEvidence::Null,
            account_transfer_id: AccountTransferIdEvidence::Null,
            native_release_reset_pending: NativeReleaseResetPendingEvidence::Null,
            is_lead: IsLeadEvidence::Null,
            auth_resume_proof_pending: false,
        },
    }
}

#[test]
fn component_presence_is_independent_and_empty_rows_are_refused() {
    assert_eq!(
        AgentComponents::from_presence(true, false).expect("agent only"),
        AgentComponents::AgentOnly
    );
    assert_eq!(
        AgentComponents::from_presence(false, true).expect("chat only"),
        AgentComponents::ChatOnly
    );
    assert_eq!(
        AgentComponents::from_presence(true, true).expect("both"),
        AgentComponents::Both
    );
    assert!(AgentComponents::from_presence(false, false).is_err());
}

#[test]
fn normal_source_admission_checks_physical_identity_and_account_without_overlay() {
    let mut snapshot = source();
    let AdmittedAgentSource::Normal(normal) = snapshot.admit().expect("normal source") else {
        panic!("normal source became held")
    };
    assert_eq!(normal.id.as_ref(), "a");
    assert_eq!(normal.account_key, "default");
    assert_eq!(normal.status, crate::AgentStatus::Running);
    assert!(
        normal.events.is_none(),
        "admission must not load an effective overlay"
    );
    snapshot.physical_id = Some("another-row".into());
    assert!(
        snapshot.admit().is_err(),
        "embedded identity cannot replace the physical key"
    );
    snapshot.physical_id = Some("a".into());
    snapshot.selectors.account_key = "another-account".into();
    assert!(snapshot.admit().is_err());
    snapshot.physical_id = None;
    assert!(snapshot.admit().is_err(), "NULL identity is evidence-only");
}

#[test]
fn held_source_keeps_unreadable_bytes_identity_and_selectors_without_normal_decode() {
    for hold in [HoldStatus::Invalid, HoldStatus::Unrecognized] {
        let mut snapshot = source();
        snapshot.hold = hold;
        snapshot.physical_id = None;
        snapshot.hold_incident = Some("incident".into());
        snapshot.record = "{ deliberately unreadable source bytes".into();
        snapshot.selectors.deleted_at = DeletedAtEvidence::Text("".into());
        snapshot.selectors.remote_worker = RemoteWorkerEvidence::Integer(-1);
        snapshot.selectors.idle_episode_fire_at = IdleEpisodeFireAtEvidence::Real(1.25);
        let AdmittedAgentSource::Held(evidence) = snapshot.admit().expect("held evidence") else {
            panic!("held source acquired fabricated fields")
        };
        assert_eq!(evidence.record_bytes(), snapshot.record.as_bytes());
        assert_eq!(evidence.identity(), snapshot.identity());
        assert_eq!(evidence.hold(), hold);
        assert_eq!(evidence.incident(), Some("incident"));
        assert_eq!(evidence.selectors(), &snapshot.selectors);
    }
}

#[test]
fn all_accepted_statuses_remain_normal_but_unknown_status_requires_held_evidence() {
    for status in crate::AgentStatus::VALUES {
        let mut snapshot = source();
        snapshot.record = format!(r#"{{"id":"a","rootId":"a","epoch":1,"provider":"codex","accountKey":"default","status":"{status}"}}"#).into_boxed_str();
        let AdmittedAgentSource::Normal(record) = snapshot.admit().expect("accepted source status")
        else {
            panic!("accepted status became held")
        };
        assert_eq!(record.status.as_str(), *status);
        assert_eq!(
            studio_json::to_string(&record.status).expect("wire status"),
            format!("\"{status}\"")
        );
    }
    let mut snapshot = source();
    snapshot.record =
        r#"{"id":"a","rootId":"a","epoch":1,"accountKey":"default","status":"future-status"}"#
            .into();
    assert!(snapshot.admit().is_err());
    snapshot.hold = HoldStatus::Unrecognized;
    assert!(matches!(snapshot.admit(), Ok(AdmittedAgentSource::Held(_))));
}

#[test]
fn generated_account_default_is_not_a_valid_record_member() {
    for account in ["", ",\"accountKey\":null", ",\"accountKey\":23"] {
        let mut snapshot = source();
        snapshot.record =
            format!(r#"{{"id":"a","rootId":"a","epoch":1,"status":"running"{account}}}"#)
                .into_boxed_str();
        assert!(
            snapshot.admit().is_err(),
            "a source selector default must not fabricate accountKey"
        );
        snapshot.hold = HoldStatus::Invalid;
        let AdmittedAgentSource::Held(evidence) = snapshot.admit().expect("retained held account")
        else {
            panic!("bad account became normal")
        };
        assert_eq!(evidence.selectors().account_key, "default");
        assert_eq!(evidence.record_bytes(), snapshot.record.as_bytes());
    }
}

#[test]
fn typed_selector_evidence_preserves_classes_and_rejects_another_selector() {
    for value in [
        DeletedAtEvidence::Null,
        DeletedAtEvidence::Integer(0),
        DeletedAtEvidence::Real(-0.0),
        DeletedAtEvidence::Real(1.25),
        DeletedAtEvidence::Text("0\0✓".into()),
    ] {
        let bytes = value.evidence_bytes();
        assert_eq!(
            DeletedAtEvidence::from_evidence_bytes(&bytes).expect("selector evidence"),
            value
        );
        assert!(WorkspaceOperationEvidence::from_evidence_bytes(&bytes).is_err());
    }
    assert_ne!(
        DeletedAtEvidence::Integer(0).evidence_bytes(),
        DeletedAtEvidence::Real(0.0).evidence_bytes()
    );
    assert_ne!(
        DeletedAtEvidence::Integer(0).evidence_bytes(),
        DeletedAtEvidence::Text("0".into()).evidence_bytes()
    );
    assert!(DeletedAtEvidence::from_evidence_bytes(&[]).is_err());
    assert!(DeletedAtEvidence::from_evidence_bytes(&[0, 0, 1]).is_err());
    assert!(DeletedAtEvidence::from_evidence_bytes(&[0, 1, 0]).is_err());
    assert!(DeletedAtEvidence::from_evidence_bytes(&[0, 3, 255]).is_err());
}
