use super::{StoredTurnAttempt, TurnAttemptCodec};
use studio_db_api::Codec;

#[test]
fn legacy_attempt_evidence_preserves_noncanonical_source_without_normal_defaults() {
    let db = crate::fresh_state::test_canvas();
    for raw in [
        " null ",
        "false",
        "1.0",
        r#""text""#,
        r#" { "action":"future", "submitted":2 } "#,
        r#"{"action":null}"#,
        r#"{"action":"review","action":"compact"}"#,
        r#"{"unknown":1,"unknown":2}"#,
    ] {
        let value = StoredTurnAttempt::from_stored(raw.into()).expect("valid legacy JSON");
        assert!(matches!(value, StoredTurnAttempt::Legacy(_)), "{raw}");
        assert_eq!(value.raw(), raw);
        assert!(value.normal().is_none());
        let encoded = TurnAttemptCodec::encode(&db, &Some(value.clone()))
            .expect("encode")
            .map(str::to_owned);
        let decoded = TurnAttemptCodec::decode(&db, encoded)
            .expect("decode")
            .expect("present source attempt");
        assert_eq!(decoded.raw(), raw);
    }
    assert!(StoredTurnAttempt::from_stored("broken".into()).is_err());
    assert!(
        TurnAttemptCodec::decode(&db, None)
            .expect("missing attempt")
            .is_none()
    );
}

#[test]
fn normal_attempt_keeps_one_existing_typed_owner_and_exact_storage_text() {
    let raw = r#" { "action":"capacity", "submitted":true, "connectionGeneration":4, "activeAtReservation":false } "#;
    let stored = StoredTurnAttempt::from_stored(raw.into()).expect("normal attempt");
    assert_eq!(stored.raw(), raw);
    let normal = stored.normal().expect("typed start attempt owner");
    assert_eq!(
        normal.action,
        Some(studio_runtime_schema::start_attempt::StartAttemptAction::Capacity)
    );
    assert_eq!(normal.submitted, Some(true));
    assert_eq!(normal.connection_generation, Some(4));
    assert_eq!(normal.active_at_reservation, Some(false));
}
