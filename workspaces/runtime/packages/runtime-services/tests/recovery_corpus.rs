use studio_runtime_services::recovery_policy::{RecoverySnapshot, native_operations_settled};
#[test]
fn malformed_native_items_never_authorize_completion() {
    for items in [
        studio_json::json!(null),
        studio_json::json!("invalid"),
        studio_json::json!({}),
    ] {
        assert!(!native_operations_settled(
            &studio_json::json!({"items":items})
        ));
    }
}

#[test]
fn recovery_record_typed_boundary_matches_schema() {
    let base =
        studio_json::json!({"compactions":0,"id":"a","rootId":"a","epoch":1,"status":"running"});
    for (field, values) in [
        (
            "epoch",
            vec![
                studio_json::json!(1.0),
                studio_json::json!(-1),
                studio_json::json!(9007199254740992_u64),
            ],
        ),
        (
            "events",
            vec![
                studio_json::json!(null),
                studio_json::json!(1.0),
                studio_json::json!(-1),
            ],
        ),
        (
            "startAttempt",
            vec![
                studio_json::json!([]),
                studio_json::json!({"events":[1]}),
                studio_json::json!({"epoch":1.0}),
            ],
        ),
        ("id", vec![studio_json::json!(""), studio_json::json!(null)]),
    ] {
        for value in values {
            let mut candidate = base.clone();
            candidate[field] = value;
            let mut expected =
                studio_runtime_schema::validate("RecoveryAgentRecord", &candidate).expect("schema");
            // JSON Schema integer admits 1.0; the persisted i64 owner requires an integer encoding.
            if field == "startAttempt" && candidate[field]["epoch"].as_f64() == Some(1.0) {
                expected = false;
            }
            let actual = <studio_runtime_schema::recovery_record::RecoveryAgentRecord as serde::Deserialize>::deserialize(&candidate)
            .is_ok();
            assert_eq!(actual, expected, "{candidate}");
            let snapshot = RecoverySnapshot::from_record(candidate.clone());
            assert_eq!(snapshot.is_ok(), expected, "snapshot: {candidate}");
            if let Ok(snapshot) = snapshot {
                assert_eq!(snapshot.record(), &candidate);
            }
        }
    }
    assert!(
        <studio_runtime_schema::recovery_record::RecoveryAgentRecord as serde::Deserialize>::deserialize(
            studio_json::json!(["a", "a", 1, "running"])
        )
        .is_err()
    );
}

#[test]
fn snapshot_rejects_object_encoded_request_state() {
    let mut record = studio_json::json!({"compactions":0,"id":"a","rootId":"a","epoch":1,"status":"running",
        "connectionRecovery":{"requestState":"pending"}});
    assert!(RecoverySnapshot::from_record(record.clone()).is_ok());
    record["connectionRecovery"]["requestState"] = studio_json::json!({"pending":null});
    assert!(RecoverySnapshot::from_record(record).is_err());
}
