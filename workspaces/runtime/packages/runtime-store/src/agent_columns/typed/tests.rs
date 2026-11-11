use super::{AgentIdentity, NullableAgentSource};
use studio_db_api::migration::{decode_key, encode_key};

#[test]
fn nullable_agent_identities_preserve_source_multiplicity_and_exact_framing() {
    let fixtures = [
        (
            NullableAgentSource::Agent,
            Some("a"),
            1,
            "t000000000000000161n0000000000000000n0000000000000000",
        ),
        (
            NullableAgentSource::Volatile,
            Some("a"),
            9,
            "t000000000000000161n0000000000000000n0000000000000000",
        ),
        (
            NullableAgentSource::Agent,
            Some(""),
            0,
            "t0000000000000000n0000000000000000n0000000000000000",
        ),
        (
            NullableAgentSource::Agent,
            None,
            -1,
            "n0000000000000000i000000000000000130i00000000000000022D31",
        ),
        (
            NullableAgentSource::Agent,
            None,
            -2,
            "n0000000000000000i000000000000000130i00000000000000022D32",
        ),
        (
            NullableAgentSource::Volatile,
            None,
            -1,
            "n0000000000000000i000000000000000131i00000000000000022D31",
        ),
    ];
    for (source, id, rowid, expected) in fixtures {
        let identity = AgentIdentity::from_source(source, id, rowid);
        assert_eq!(identity.as_bytes(), expected.as_bytes());
        assert_eq!(
            AgentIdentity::from_bytes(identity.as_bytes()).expect("canonical identity"),
            identity
        );
        let cursor = encode_key(&(id.map(str::to_owned), rowid));
        assert_eq!(
            AgentIdentity::from_dirty_key(source, &cursor).expect("source cursor"),
            identity
        );
    }
    assert_ne!(
        AgentIdentity::from_source(NullableAgentSource::Agent, Some("n0000000000000000"), -1),
        AgentIdentity::from_source(NullableAgentSource::Agent, None, -1)
    );
}

#[test]
fn source_cursor_moves_normalize_old_and_new_without_losing_null_ties() {
    let source = NullableAgentSource::Agent;
    let cursors = [(None, -1), (Some("a"), 4), (Some("b"), 5), (None, 0)];
    let expected = cursors.map(|(id, rowid)| AgentIdentity::from_source(source, id, rowid));
    for ((id, rowid), identity) in cursors.into_iter().zip(expected) {
        let cursor = encode_key(&(id.map(str::to_owned), rowid));
        assert_eq!(
            decode_key::<(Option<String>, i64)>(&cursor).expect("cursor roundtrip"),
            (id.map(str::to_owned), rowid)
        );
        assert_eq!(
            AgentIdentity::from_dirty_key(source, &cursor).expect("normalization"),
            identity
        );
    }
    let extreme = AgentIdentity::from_source(source, None, i64::MIN);
    assert_eq!(
        AgentIdentity::from_bytes(extreme.as_bytes()).expect("signed rowid"),
        extreme
    );
    let unicode = AgentIdentity::for_agent(&crate::AgentId::from("a\0✓"));
    assert_eq!(
        AgentIdentity::from_bytes(unicode.as_bytes()).expect("exact UTF-8 id"),
        unicode
    );
}

#[test]
fn physical_agent_identity_rejects_noncanonical_shapes_and_storage_classes() {
    for key in [
        (Some("a".to_owned()), Some(0), None),
        (None, Some(2), Some(0)),
        (None, None, Some(0)),
        (None, Some(0), None),
    ] {
        assert!(AgentIdentity::from_bytes(&encode_key(&key)).is_err());
    }
    assert!(
        AgentIdentity::from_bytes(b"n0000000000000000i00000000000000023030i000000000000000130")
            .is_err()
    );
    assert!(AgentIdentity::from_dirty_key(NullableAgentSource::Agent, b"malformed").is_err());
}
