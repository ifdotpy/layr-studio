use crate::domain::{ChatState, SessionId, SessionState, TurnState, TurnStateKind};
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{DbQuery, DbTable, DbValue, Table};

#[derive(DbTable)]
#[db(table = "__f2_state_code_witness", database = Canvas, strict)]
struct StateCodes {
    #[db(key)]
    id: i64,
    chat: ChatState,
    session: SessionState,
    turn: TurnStateKind,
    status: crate::AgentStatus,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=StateCodes::ID,
    tables(rows=StateCodes),columns(id=StateCodes::ID,chat=StateCodes::CHAT),
    sql="SELECT {chat}+0 AS {id} FROM {rows} WHERE {id}=?1")]
struct ChatCode {
    #[db(column=StateCodes::ID)]
    id: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=StateCodes::ID,
    tables(rows=StateCodes),columns(id=StateCodes::ID,status=StateCodes::STATUS),
    sql="SELECT {status}+0 AS {id} FROM {rows} WHERE {id}=?1")]
struct AgentCode {
    #[db(column=StateCodes::ID)]
    id: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
    tables(rows=StateCodes),columns(id=StateCodes::ID,chat=StateCodes::CHAT,session=StateCodes::SESSION,turn=StateCodes::TURN,status=StateCodes::STATUS),
    sql="INSERT INTO {rows}({id},{chat},{session},{turn},{status}) VALUES(100,99,0,0,0)")]
struct UnknownChatCode;

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
    tables(rows=StateCodes),columns(id=StateCodes::ID,chat=StateCodes::CHAT,session=StateCodes::SESSION,turn=StateCodes::TURN,status=StateCodes::STATUS),
    sql="INSERT INTO {rows}({id},{chat},{session},{turn},{status}) VALUES(101,0,0,0,99)")]
struct UnknownAgentCode;

#[test]
fn domain_codes_use_real_owners_and_generated_constraints_without_wire_changes() {
    assert_eq!(ChatState::CODES, "0,1,2");
    assert_eq!(SessionState::CODES, "0,1,2,3");
    assert_eq!(TurnStateKind::CODES, "0,1,2,3,4");
    let mut db = crate::fresh_state::test_canvas();
    StateCodes::descriptor()
        .install_if_absent(&db)
        .expect("typed witness schema");
    ChatCode::validate_query(&db).expect("query declaration");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (id, chat, wire) in [
        (0, ChatState::Active, "active"),
        (1, ChatState::Paused, "paused"),
        (2, ChatState::Archived, "archived"),
    ] {
        tx.insert(&StateCodes {
            id,
            chat,
            session: SessionState::Ready,
            turn: TurnStateKind::Starting,
            status: crate::AgentStatus::Running,
        })
        .expect("typed states");
        assert_eq!(
            tx.read()
                .named(&ChatCode::QUERY, &ChatCode { id })
                .expect("stored integer code"),
            id
        );
        assert_eq!(
            tx.get::<StateCodes>(&(id,))
                .expect("read")
                .expect("row")
                .chat,
            chat
        );
        assert_eq!(
            studio_json::to_string(&chat).expect("wire state"),
            format!("\"{wire}\"")
        );
        assert_eq!(chat.column(), wire);
    }
    assert!(tx.named(&UnknownChatCode::QUERY, &UnknownChatCode).is_err());
    assert!(
        tx.get::<StateCodes>(&(100,))
            .expect("failed insert rollback")
            .is_none()
    );
}

#[test]
fn all_sixteen_agent_statuses_have_permanent_integer_codes_and_parse_refusal() {
    let expected = [
        ("unknown", 0),
        ("idle", 1),
        ("queued", 2),
        ("starting", 3),
        ("running", 4),
        ("waiting", 5),
        ("waiting_on_monitor", 6),
        ("approval", 7),
        ("completed", 8),
        ("interrupted", 9),
        ("failed", 10),
        ("blocked", 11),
        ("paused", 12),
        ("parked", 13),
        ("abandoned", 14),
        ("capacity-retry", 15),
    ];
    assert_eq!(crate::AgentStatus::VALUES.len(), expected.len());
    let mut db = crate::fresh_state::test_canvas();
    StateCodes::descriptor()
        .install_if_absent(&db)
        .expect("typed schema");
    AgentCode::validate_query(&db).expect("code declaration");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (wire, code) in expected {
        let status = crate::AgentStatus::parse(wire).expect("accepted status");
        tx.insert(&StateCodes {
            id: code,
            chat: ChatState::Active,
            session: SessionState::Ready,
            turn: TurnStateKind::Starting,
            status: status.clone(),
        })
        .expect("integer status");
        assert_eq!(
            tx.read()
                .named(&AgentCode::QUERY, &AgentCode { id: code })
                .expect("actual integer code"),
            code
        );
        assert_eq!(
            tx.get::<StateCodes>(&(code,))
                .expect("read status")
                .expect("row")
                .status,
            status
        );
        assert_eq!(
            studio_json::to_string(&status).expect("wire status"),
            format!("\"{wire}\"")
        );
    }
    assert!(
        tx.named(&UnknownAgentCode::QUERY, &UnknownAgentCode)
            .is_err()
    );
    for invalid in ["future-status", "", "Idle", "capacity_retry"] {
        assert!(crate::AgentStatus::parse(invalid).is_err());
        assert!(studio_json::from_str::<crate::AgentStatus>(&format!("\"{invalid}\"")).is_err());
    }
    let mut lexical = expected.map(|(wire, _)| wire);
    lexical.sort();
    for (rank, wire) in lexical.into_iter().enumerate() {
        assert_eq!(
            crate::AgentStatus::parse(wire)
                .expect("wire order")
                .lexical_rank(),
            rank
        );
    }
}

#[test]
fn typed_turn_kind_keeps_the_existing_session_coupling_contract() {
    for kind in [
        TurnStateKind::Starting,
        TurnStateKind::Running,
        TurnStateKind::Completed,
        TurnStateKind::Failed,
        TurnStateKind::Interrupted,
    ] {
        let session = (kind == TurnStateKind::Running).then(|| SessionId("session".into()));
        let turn = TurnState::from_storage_kind(kind, session.clone()).expect("coupled state");
        assert_eq!(turn.storage_kind(), kind);
        assert_eq!(turn.kind(), kind.as_str());
        assert_eq!(
            TurnState::from_columns(kind.as_str(), session.map(|id| id.0))
                .expect("legacy wire columns"),
            turn
        );
        let invalid_session = (kind != TurnStateKind::Running).then(|| SessionId("session".into()));
        assert!(TurnState::from_storage_kind(kind, invalid_session).is_err());
    }
}
