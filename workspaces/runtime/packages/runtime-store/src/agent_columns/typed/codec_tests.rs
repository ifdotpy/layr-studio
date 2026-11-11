use super::{
    AgentIdentity, AgentIdentityCodec, DeletedAtCodec, DeletedAtEvidence, NullableAgentSource,
    RemoteWorkerCodec, RemoteWorkerEvidence,
};
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{Codec, DbTable, Table, db_query};

#[derive(DbTable)]
#[db(table="__f2_evidence_codec_witness",database=Canvas,strict)]
struct EvidenceRow {
    #[db(key)]
    id: i64,
    #[db(codec=AgentIdentityCodec)]
    identity: AgentIdentity,
    #[db(codec=DeletedAtCodec)]
    deleted_at: DeletedAtEvidence,
    #[db(codec=RemoteWorkerCodec)]
    remote_worker: RemoteWorkerEvidence,
}

#[test]
fn real_columns_reuse_db_api_codecs_and_reject_cross_selector_evidence() {
    let mut db = crate::fresh_state::test_canvas();
    EvidenceRow::descriptor()
        .install_if_absent(&db)
        .expect("typed BLOB columns");
    let deleted = DeletedAtEvidence::Real(1.25);
    let encoded = DeletedAtCodec::encode(&db, &deleted).expect("encode evidence");
    assert_eq!(
        DeletedAtCodec::decode(&db, encoded.clone()).expect("decode evidence"),
        deleted
    );
    assert!(RemoteWorkerCodec::decode(&db, encoded).is_err());
    assert!(AgentIdentityCodec::decode(&db, vec![255]).is_err());
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (index, value) in [
        DeletedAtEvidence::Null,
        DeletedAtEvidence::Integer(0),
        DeletedAtEvidence::Real(-0.0),
        DeletedAtEvidence::Real(1.25),
        DeletedAtEvidence::Text("".into()),
        DeletedAtEvidence::Text("0\0✓".into()),
    ]
    .into_iter()
    .enumerate()
    {
        let identity = AgentIdentity::from_source(NullableAgentSource::Agent, None, index as i64);
        tx.insert(&EvidenceRow {
            id: index as i64,
            identity: identity.clone(),
            deleted_at: value.clone(),
            remote_worker: RemoteWorkerEvidence::Text("{}".into()),
        })
        .expect("typed evidence insert");
        let mut rows = db_query!(tx, EvidenceRow, filter(EvidenceRow::IDENTITY.eq(&identity)))
            .expect("typed identity parameter and evidence read");
        assert_eq!(rows.len(), 1);
        let row = rows.remove(0);
        assert_eq!(row.identity, identity);
        assert_eq!(
            row.deleted_at.evidence_bytes(),
            value.evidence_bytes(),
            "exact class/float bits/text bytes"
        );
        assert_eq!(row.remote_worker, RemoteWorkerEvidence::Text("{}".into()));
    }
}
