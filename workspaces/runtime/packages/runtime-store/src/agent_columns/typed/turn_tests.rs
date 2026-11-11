use super::{StoredTurnAttempt, TypedTurnRow};
use crate::agent_tables::{TurnAttemptPayload, TurnsStorageRow};
use crate::domain::TurnStateKind;
use crate::json_record_codec::ExactJson;
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{DbQuery, Table};

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(rows=TurnsStorageRow),
    columns(id=TurnsStorageRow::ID,chat=TurnsStorageRow::CHAT,state=TurnsStorageRow::STATE,session=TurnsStorageRow::SESSION,
    provider=TurnsStorageRow::PROVIDER_TURN,attempt=TurnsStorageRow::ATTEMPT,ticket=TurnsStorageRow::INTERRUPT_TICKET,rowid=TurnsStorageRow::SOURCE_ROWID),
    sql="INSERT INTO {rows}({id},{chat},{state},{session},{provider},{attempt},{ticket},{rowid}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)")]
struct InsertSource {
    #[db(column=TurnsStorageRow::ID)]
    id: String,
    #[db(column=TurnsStorageRow::CHAT)]
    chat: String,
    #[db(column=TurnsStorageRow::STATE)]
    state: String,
    #[db(column=TurnsStorageRow::SESSION)]
    session: Option<String>,
    #[db(column=TurnsStorageRow::PROVIDER_TURN)]
    provider: Option<String>,
    #[db(column=TurnsStorageRow::ATTEMPT)]
    attempt: Option<ExactJson<TurnAttemptPayload>>,
    #[db(column=TurnsStorageRow::INTERRUPT_TICKET)]
    ticket: Option<String>,
    #[db(column=TurnsStorageRow::SOURCE_ROWID)]
    rowid: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(rows=TypedTurnRow),
    columns(id=TypedTurnRow::ID,source_order=TypedTurnRow::SOURCE_ROWID,chat=TypedTurnRow::CHAT,state=TypedTurnRow::STATE,session=TypedTurnRow::SESSION),
    sql="INSERT INTO {rows}({id},{source_order},{chat},{state},{session}) VALUES(?1,?2,?3,?4,?5)")]
struct InsertState {
    #[db(column=TypedTurnRow::ID)]
    id: String,
    #[db(column=TypedTurnRow::SOURCE_ROWID)]
    order: i64,
    #[db(column=TypedTurnRow::CHAT)]
    chat: crate::AgentId,
    #[db(column=TypedTurnRow::STATE)]
    state: TurnStateKind,
    #[db(column=TypedTurnRow::SESSION)]
    session: Option<String>,
}

#[test]
fn real_turn_source_selectors_and_exact_legacy_attempts_survive_typed_storage() {
    let mut db = crate::fresh_state::test_canvas();
    TypedTurnRow::descriptor()
        .install_if_absent(&db)
        .expect("prepared turns with every index before copying");
    InsertSource::validate_query(&db).expect("source declaration");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (index,raw) in [
        r#" { "action":"capacity", "submitted":true, "connectionGeneration":4, "activeAtReservation":false } "#,
        " null ","false",r#" { "action":"future", "submitted":2 } "#,
        r#"{"action":null}"#,r#"{"action":true}"#,r#"{"action":{"new":1}}"#,
        r#"{"action":"review","action":"compact"}"#,r#"{"submitted":"1","activeAtReservation":"0"}"#,
    ].into_iter().enumerate() {
        let id=format!("source-{index}");
        tx.named(&InsertSource::QUERY,&InsertSource{id:id.clone(),chat:format!("chat-{index}"),state:TurnStateKind::Completed.as_str().into(),session:None,provider:Some(format!("provider-{index}")),attempt:Some(ExactJson::from_stored(raw.into()).expect("legacy attempt JSON")),ticket:Some("ticket".into()),rowid:-(index as i64)-1}).expect("actual legacy SQLite generated selectors");
        let source=tx.get::<TurnsStorageRow>(&(id.clone(),)).expect("read source selectors").expect("source turn");
        let original=(source.attempt_submitted,source.attempt_connection_generation,source.attempt_action.clone(),source.attempt_active_at_reservation);
        let target=TypedTurnRow::try_from(source).expect("fixed source admission");
        assert_eq!(matches!(target.attempt.as_ref(),Some(StoredTurnAttempt::Normal(_))),index==0,"{raw}");
        tx.insert(&target).expect("typed target retains legacy evidence");
        let copied=tx.get::<TypedTurnRow>(&(id.clone(),)).expect("read target").expect("target turn");
        assert_eq!(copied.turn_id().0,id);
        assert_eq!(copied.source_rowid,-(index as i64)-1);
        assert_eq!(copied.provider_turn,Some(format!("provider-{index}")));
        assert_eq!(copied.interrupt_ticket.as_deref(),Some("ticket"));
        assert_eq!(copied.attempt.as_ref().expect("attempt presence").raw(),raw);
        let action=copied.attempt_action.map(|action|studio_json::from_str::<String>(&studio_json::to_string(&action).expect("wire action")).expect("wire text")).or(copied.attempt_legacy_action.clone());
        assert_eq!((copied.attempt_submitted,copied.attempt_connection_generation,action,copied.attempt_active_at_reservation),original,"SQLite affinity and duplicate-key selector semantics");
    }
    transaction
        .rollback()
        .expect("source remains authoritative");
}

#[test]
fn real_turn_sql_constraints_enforce_coupling_and_one_unfinished_without_code_copies() {
    let mut db = crate::fresh_state::test_canvas();
    TypedTurnRow::descriptor()
        .install_if_absent(&db)
        .expect("prepared descriptor");
    InsertState::validate_query(&db).expect("direct constraint witness");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (id, kind, session) in [
        ("running", TurnStateKind::Running, None),
        ("starting", TurnStateKind::Starting, Some("session")),
        ("completed", TurnStateKind::Completed, Some("session")),
    ] {
        assert!(
            tx.named(
                &InsertState::QUERY,
                &InsertState {
                    id: id.into(),
                    order: 1,
                    chat: id.into(),
                    state: kind,
                    session: session.map(str::to_owned)
                }
            )
            .is_err()
        );
    }
    tx.named(
        &InsertState::QUERY,
        &InsertState {
            id: "one".into(),
            order: 2,
            chat: "busy".into(),
            state: TurnStateKind::Starting,
            session: None,
        },
    )
    .expect("starting requires no session");
    assert!(
        tx.named(
            &InsertState::QUERY,
            &InsertState {
                id: "two".into(),
                order: 3,
                chat: "busy".into(),
                state: TurnStateKind::Running,
                session: Some("session".into())
            }
        )
        .is_err(),
        "unique unfinished chat key is not weakened by source order"
    );
    tx.named(
        &InsertState::QUERY,
        &InsertState {
            id: "history".into(),
            order: 4,
            chat: "busy".into(),
            state: TurnStateKind::Completed,
            session: None,
        },
    )
    .expect("finished turn history survives");
    transaction.rollback().expect("no cutover");
}
