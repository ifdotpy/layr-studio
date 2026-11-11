use super::record::NativeFailureHold;

#[test]
fn unreadable_native_hold_preserves_exact_future_tokens_and_duplicate_members() {
    let raw = r#"{ "reason":"pending", "future":1.00e+3, "unknown":1, "unknown":2 }"#;
    let hold: NativeFailureHold = studio_json::from_str(raw).expect("accepted legacy hold");
    assert!(matches!(hold, NativeFailureHold::Unreadable(_)));
    assert_eq!(
        studio_json::to_string(&hold).expect("retained raw evidence"),
        raw
    );
}

#[test]
fn native_hold_sync_projection_keeps_object_truthiness_and_nonobject_refusal() {
    for (raw, expected) in [
        ("true", Some(true)),
        ("false", Some(false)),
        ("{}", Some(false)),
        (r#"{"reason":"pending"}"#, Some(true)),
        (r#"{"terminalFault":"pending_receipt"}"#, Some(true)),
        ("0", None),
        ("2", None),
        (r#""hold""#, None),
        ("[]", None),
        ("[1]", None),
    ] {
        let hold: NativeFailureHold = studio_json::from_str(raw).expect("legacy hold");
        assert_eq!(hold.projected_truthiness().ok(), expected, "{raw}");
    }
}

const FUTURE_PAYLOAD: &str = r#"[ { "n":1.00e+3, "dup":1, "dup":2 } ]"#;

fn preserves_unrecognized<T: serde::de::DeserializeOwned + serde::Serialize>() {
    let payload: T = studio_json::from_str(FUTURE_PAYLOAD).expect("opaque future payload");
    assert_eq!(studio_json::to_string(&payload).unwrap(), FUTURE_PAYLOAD);
}

#[test]
fn transfer_value_retains_exact_unrecognized_evidence() {
    preserves_unrecognized::<super::record::StoredTransferValue<String>>();
}

#[test]
fn transfer_object_retains_exact_unrecognized_evidence() {
    preserves_unrecognized::<super::record::StoredTransferObject<super::record::AgentDraft>>();
}

#[test]
fn worker_defaults_retains_exact_unrecognized_evidence() {
    preserves_unrecognized::<super::record::StoredWorkerDefaults>();
}

#[test]
fn title_job_retains_exact_unrecognized_evidence() {
    preserves_unrecognized::<super::record::StoredTitleJob>();
}

#[test]
fn start_outcome_retains_exact_unrecognized_evidence() {
    preserves_unrecognized::<super::record::StoredStartOutcomeHold>();
}

#[test]
fn worker_default_legacy_projection_keeps_valid_members_and_wrong_type_evidence() {
    use super::record::{StoredTransferValue, StoredWorkerDefaults};
    let raw = r#"{"model":"model","effort":{ "n":1.00e+3, "dup":1, "dup":2 },"fastMode":true,"daybreakEnabled":false,"delegationDepth":2,"maxConcurrency":"wrong"}"#;
    let defaults: StoredWorkerDefaults = studio_json::from_str(raw).unwrap();
    assert!(matches!(defaults, StoredWorkerDefaults::Unreadable(_)));
    let selection = defaults.legacy_selection().expect("legacy object selector");
    assert_eq!(
        selection
            .model
            .as_ref()
            .and_then(StoredTransferValue::known)
            .map(String::as_str),
        Some("model")
    );
    assert_eq!(
        selection
            .fast_mode
            .as_ref()
            .and_then(StoredTransferValue::known),
        Some(&true)
    );
    assert_eq!(
        selection
            .daybreak_enabled
            .as_ref()
            .and_then(StoredTransferValue::known),
        Some(&false)
    );
    assert_eq!(
        selection
            .delegation_depth
            .as_ref()
            .and_then(StoredTransferValue::known),
        Some(&2)
    );
    let Some(StoredTransferValue::Unreadable(evidence)) = selection.effort else {
        panic!("typed wrong-type evidence")
    };
    assert_eq!(evidence.0.as_str(), r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#);
    assert_eq!(studio_json::to_string(&defaults).unwrap(), raw);
}

#[test]
fn transfer_reason_legacy_projection_keeps_valid_members_and_wrong_type_evidence() {
    use super::record::{AgentDraft, StoredTransferObject, StoredTransferValue};
    for (raw, expected) in [
        (r#"{"reason":"legacy","assets":false}"#, Some("legacy")),
        (r#"{"reason":{ "n":1.00e+3, "dup":1, "dup":2 }}"#, None),
    ] {
        let stored: StoredTransferObject<AgentDraft> = studio_json::from_str(raw).unwrap();
        let reason = stored.legacy_reason().expect("present reason");
        assert_eq!(reason.known().map(String::as_str), expected);
        if expected.is_none() {
            let StoredTransferValue::Unreadable(evidence) = reason else {
                panic!("typed wrong-type evidence")
            };
            assert_eq!(evidence.0.as_str(), r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#);
        }
        assert_eq!(studio_json::to_string(&stored).unwrap(), raw);
    }
}

fn preserves_cleanup_extension<T: serde::de::DeserializeOwned + serde::Serialize>(prefix: &str) {
    let extension = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!("{{{prefix}\"future\":{extension}}}");
    let owner: T = studio_json::from_str(&raw).unwrap();
    assert!(studio_json::to_string(&owner).unwrap().contains(extension));
}

#[test]
fn partial_worktree_identity_retains_exact_future_extension() {
    preserves_cleanup_extension::<crate::workspace_cleanup_intent::PartialWorktreeIdentity>("");
}

#[test]
fn lifecycle_identity_retains_exact_future_extension() {
    preserves_cleanup_extension::<crate::workspace_cleanup_intent::LifecycleIdentity>("");
}

#[test]
fn cleanup_intent_retains_exact_future_extension() {
    preserves_cleanup_extension::<crate::workspace_cleanup_intent::StoredCleanupIntent>(
        r#""action":"delete","phase":"queued","#,
    );
}

#[test]
fn deleted_archive_field_retains_unknown_members_and_exact_bytes() {
    let archive = r#"{ "at":1.00, "epoch":1, "future":{ "dup":1, "dup":2 } }"#;
    let raw = format!(
        r#"{{"id":"a","rootId":"a","epoch":1,"status":"idle","accountKey":"default","deletedArchive":{archive}}}"#
    );
    let agent: crate::AgentRecord = studio_json::from_str(&raw).unwrap();
    let written = studio_json::to_string(&agent).unwrap();
    assert!(written.contains(archive), "{written}");
}

#[test]
fn exact_json_serde_roundtrip_equals_admitted_storage_bytes() {
    let raw = r#"{ "at":1.00, "epoch":1, "future":{ "dup":1, "dup":2 } }"#;
    let exact = crate::json_record_codec::ExactJson::<crate::workspace_cleanup_intent::AgentArchive>::from_stored(raw.into()).unwrap();
    let written = studio_json::to_string(&exact).unwrap();
    assert_eq!(written, exact.raw());
    let decoded: crate::json_record_codec::ExactJson<
        crate::workspace_cleanup_intent::AgentArchive,
    > = studio_json::from_str(&written).unwrap();
    assert_eq!(decoded.raw(), raw);
    assert_eq!(decoded.value().epoch, 1);
}

#[test]
fn idle_episode_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::IdleCompactionEpisode>("");
}
#[test]
fn idle_cache_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::IdleCache>("");
}
#[test]
fn thread_block_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::NativeThreadBlock>("");
}
#[test]
fn turn_error_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::NativeTurnError>("");
}
#[test]
fn native_release_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::NativeRelease>("");
}
#[test]
fn title_job_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::TitleJob>(
        r#""requestId":"request","agentId":"agent","rootRequestId":"root","admittedRequest":{"id":"request","signature":"signature","created":1},"status":"scheduled","#,
    );
}
#[test]
fn native_review_thread_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::NativeReviewThread>(
        r#""sequence":1,"attempt":"attempt","operation":"operation","generation":1,"connection":"connection","requestDigest":"digest","status":"submitted","#,
    );
}
#[test]
fn vm_launch_info_retains_exact_future_tokens() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!(
        r#"{{"identity":{{"agentId":"a","epoch":1,"handle":"h","requestId":"r","launchFingerprint":"f"}},"signature":"s","status":"pending","info":{future}}}"#
    );
    let launch: super::record::VmLaunch = studio_json::from_str(&raw).unwrap();
    assert!(studio_json::to_string(&launch).unwrap().contains(future));
}
#[test]
fn held_start_outcome_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::StartOutcomeHold>(r#""stage":"held","#);
}
#[test]
fn recovery_start_outcome_retains_exact_future_extension() {
    preserves_cleanup_extension::<super::record::StartOutcomeHold>(
        r#""stage":"recovery","epoch":1,"reason":"legacy","#,
    );
}
#[test]
fn previous_native_hold_retains_exact_future_tokens() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!(r#"{{"present":true,"value":{future}}}"#);
    let previous: super::record::PreviousNativeFailureHold = studio_json::from_str(&raw).unwrap();
    assert!(studio_json::to_string(&previous).unwrap().contains(future));
}

#[test]
fn native_release_equal_malformed_thread_selectors_refuse_without_mutation() {
    use super::record::{NativeRelease, StoredTransferValue};
    use studio_runtime_schema::queue_command::Field;
    for raw in [r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#, "[1,2]"] {
        let mut release: NativeRelease =
            studio_json::from_str(&format!(r#"{{"threadId":{raw},"resetPending":true}}"#)).unwrap();
        let before = studio_json::to_string(&release).unwrap();
        let selector = release.thread_id_selector().unwrap();
        let Field::Value(StoredTransferValue::Unreadable(evidence)) = &selector else {
            panic!("typed wrong-class evidence")
        };
        assert_eq!(evidence.0.as_str(), raw);
        let result = release.close_thread(&selector, 7.0, "agent");
        assert!(
            matches!(result, Err(crate::StoreError::InvalidSyncObjectField {scope, id, pointer, ..}) if scope == "runtime_agents" && id == "agent" && pointer == "/nativeRelease/threadId")
        );
        assert_eq!(
            studio_json::to_string(&release).unwrap(),
            before,
            "refusal cannot close or reset"
        );
    }
}

#[test]
fn native_release_missing_null_and_string_selector_comparison_matches_legacy() {
    use super::record::{NativeRelease, StoredTransferValue};
    use studio_runtime_schema::queue_command::Field;
    for stored in [None, Some("null"), Some(r#""a""#), Some(r#""b""#)] {
        for supplied in [None, Some("null"), Some(r#""a""#), Some(r#""b""#)] {
            let raw = stored.map_or_else(
                || "{}".to_owned(),
                |value| format!(r#"{{"threadId":{value}}}"#),
            );
            let mut release: NativeRelease = studio_json::from_str(&raw).unwrap();
            let before = studio_json::to_string(&release).unwrap();
            let selector: Field<StoredTransferValue<String>> = supplied
                .map_or(Field::Missing, |value| {
                    studio_json::from_str(value).unwrap()
                });
            let matched = release.close_thread(&selector, 7.0, "agent").unwrap();
            assert_eq!(matched, stored == supplied);
            if !matched {
                assert_eq!(studio_json::to_string(&release).unwrap(), before);
            } else {
                assert_eq!(release.extra["resetPending"].as_str(), "false");
                assert!(release.extra.contains_key("closedAt"));
            }
        }
    }
}

#[test]
fn scalar_retained_keys_keep_exact_wrong_type_evidence() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let mut lost = Vec::new();
    for key in [
        "deleted",
        "realCwd",
        "worktreeCommonDir",
        "remoteEpoch",
        "remoteControlEpoch",
        "remoteStopRequest",
        "connectionId",
        "project",
        "imageWorkspaceReady",
        "imageWorkspacePhase",
        "layrScratch",
        "steerRejectedTurnId",
        "capacityRetryCount",
        "shellPolicyLastActiveAt",
        "claudePreInputRetry",
        "providerStream",
        "contextRepairWait",
        "layrStateId",
        "emptyTransferRecovery",
    ] {
        let raw = format!(
            r#"{{"id":"agent","rootId":"agent","accountKey":"default","epoch":1,"status":"idle","{key}":{future}}}"#
        );
        let owner: super::record::AgentRecord = studio_json::from_str(&raw).unwrap();
        if !studio_json::to_string(&owner).unwrap().contains(future) {
            lost.push(key);
        }
    }
    assert!(lost.is_empty(), "lost exact evidence for {lost:?}");
}

#[test]
fn root_truthiness_and_connection_markers_keep_exact_evidence() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    for key in [
        "authWait",
        "movedTo",
        "moveImportPending",
        "parkAfterTurn",
        "connectionRecovery",
    ] {
        let raw = format!(
            r#"{{"id":"agent","rootId":"agent","accountKey":"default","epoch":1,"status":"idle","{key}":{future}}}"#
        );
        let owner: super::record::AgentRecord = studio_json::from_str(&raw).unwrap();
        assert!(
            studio_json::to_string(&owner).unwrap().contains(future),
            "{key}"
        );
    }
}

#[test]
fn agent_transfer_variants_preserve_unreadable_original_evidence() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!(
        r#"{{"id":"agent","rootId":"agent","accountKey":"default","epoch":1,"status":"idle","accountTransfer":{{"kind":"unreadable","reason":"legacy","original":{future}}}}}"#
    );
    let owner: super::record::AgentRecord = studio_json::from_str(&raw).unwrap();
    assert!(matches!(
        owner.account_transfer.value(),
        Some(super::record::StoredTransferObject::Known(
            crate::transfer_record::AgentTransferState::Unreadable(_)
        ))
    ));
    assert!(studio_json::to_string(&owner).unwrap().contains(future));
}

#[test]
fn lazy_transfer_variant_preserves_unreadable_original_evidence() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!(
        r#"{{"id":"agent","rootId":"agent","accountKey":"default","epoch":1,"status":"idle","lazyAccountTransfer":{{"kind":"unreadable","reason":"legacy","original":{future}}}}}"#
    );
    let owner: super::record::AgentRecord = studio_json::from_str(&raw).unwrap();
    assert!(matches!(
        owner.lazy_account_transfer.value(),
        Some(crate::transfer_record::LazyTransferState::Unreadable(_))
    ));
    assert!(studio_json::to_string(&owner).unwrap().contains(future));
}

#[test]
fn canonical_agent_error_preserves_provider_extension_tokens() {
    let future = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;
    let raw = format!(
        r#"{{"id":"agent","rootId":"agent","accountKey":"default","epoch":1,"status":"idle","error":{{"message":"failure","studioRawEvidence":{{}},"future":{future}}}}}"#
    );
    let owner: super::record::AgentRecord = studio_json::from_str(&raw).unwrap();
    assert!(studio_json::to_string(&owner).unwrap().contains(future));
}
