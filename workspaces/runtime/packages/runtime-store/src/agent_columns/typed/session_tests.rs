use super::{SessionsForChat, SessionsForThread, TypedSessionRow};
use crate::agent_tables::SessionsStorageRow;
use crate::domain::{SessionId, SessionRow, SessionState};
use crate::peer_update_settings::{Canvas, canvas_tx};
use studio_db_api::{DbQuery, Table};

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),
    tables(rows=SessionsStorageRow),columns(rowid=SessionsStorageRow::SOURCE_ROWID,id=SessionsStorageRow::ID,
    chat=SessionsStorageRow::CHAT,thread=SessionsStorageRow::THREAD_ID,account=SessionsStorageRow::ACCOUNT_KEY,state=SessionsStorageRow::STATE),
    sql="INSERT INTO {rows}({id},{chat},{thread},{account},{state},{rowid}) VALUES(?1,?2,?3,?4,?5,?6)")]
struct InsertSource {
    #[db(column=SessionsStorageRow::ID)]
    id: String,
    #[db(column=SessionsStorageRow::CHAT)]
    chat: String,
    #[db(column=SessionsStorageRow::THREAD_ID)]
    thread: String,
    #[db(column=SessionsStorageRow::ACCOUNT_KEY)]
    account: String,
    #[db(column=SessionsStorageRow::STATE)]
    state: SessionState,
    #[db(column=SessionsStorageRow::SOURCE_ROWID)]
    rowid: i64,
}

#[test]
fn prepared_sessions_keep_source_order_history_and_nonunique_thread_membership() {
    let mut db = crate::fresh_state::test_canvas();
    TypedSessionRow::descriptor()
        .install_if_absent(&db)
        .expect("prepared session descriptor");
    SessionsForChat::validate_query(&db).expect("chat declaration");
    SessionsForThread::validate_query(&db).expect("thread declaration");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (rowid, state) in [
        (i64::MAX, SessionState::Ended),
        (0, SessionState::Ready),
        (-1, SessionState::Lost),
    ] {
        let source = SessionRow {
            id: SessionId(format!("session-{rowid}")),
            chat: "chat".into(),
            thread_id: "thread".into(),
            account_key: "account".into(),
            state,
        };
        let prepared = TypedSessionRow::from_source(rowid, source.clone());
        tx.insert(&prepared)
            .expect("source multiplicity and signed source order");
        assert_eq!(
            tx.get::<TypedSessionRow>(&(format!("session-{rowid}"),))
                .expect("read")
                .expect("session")
                .into_domain(),
            source
        );
    }
    let sessions = tx
        .read()
        .collect(
            &SessionsForChat::QUERY,
            &SessionsForChat {
                chat: "chat".into(),
            },
        )
        .expect("history including ended");
    assert_eq!(
        sessions
            .iter()
            .map(|row| row.source_rowid)
            .collect::<Vec<_>>(),
        [-1, 0, i64::MAX]
    );
    let active = tx
        .read()
        .collect(
            &SessionsForThread::QUERY,
            &SessionsForThread {
                thread_id: "thread".into(),
                account_key: "account".into(),
                excluding: SessionState::Ended,
            },
        )
        .expect("thread membership");
    assert_eq!(
        active.iter().map(|row| row.state).collect::<Vec<_>>(),
        [SessionState::Lost, SessionState::Ready]
    );
    let duplicate = TypedSessionRow::from_source(
        2,
        SessionRow {
            id: SessionId("session-0".into()),
            chat: "chat".into(),
            thread_id: "other".into(),
            account_key: "other".into(),
            state: SessionState::Starting,
        },
    );
    assert!(
        tx.insert(&duplicate).is_err(),
        "logical session id stays unique"
    );
    transaction
        .rollback()
        .expect("witness leaves source authority unchanged");
}

#[test]
fn ordinary_session_copy_plan_keeps_original_order_and_all_fields() {
    let mut db = crate::fresh_state::test_canvas();
    InsertSource::validate_query(&db).expect("frozen source fixture declaration");
    let transaction = db.transaction().expect("transaction");
    let mut tx = canvas_tx(&transaction);
    for (id, rowid, state) in [
        ("source-a", i64::MAX, SessionState::Ended),
        ("source-b", -1, SessionState::Ready),
        ("source-c", 0, SessionState::Lost),
    ] {
        tx.named(
            &InsertSource::QUERY,
            &InsertSource {
                id: id.into(),
                rowid,
                chat: "history".into(),
                thread: "thread".into(),
                account: "account".into(),
                state,
            },
        )
        .expect("legacy source order");
    }
    let plan = super::session_copy_plan(|_| Ok(()));
    plan.copy_tx(&mut tx)
        .expect("ordinary CopyPlan and existing evidence verification");
    let rows = tx
        .read()
        .collect(
            &SessionsForChat::QUERY,
            &SessionsForChat {
                chat: "history".into(),
            },
        )
        .expect("copied session history");
    assert_eq!(
        rows.iter()
            .map(|row| row.session_id().0)
            .collect::<Vec<_>>(),
        ["source-b", "source-c", "source-a"]
    );
    assert_eq!(
        rows.iter().map(|row| row.source_rowid).collect::<Vec<_>>(),
        [-1, 0, i64::MAX]
    );
    assert_eq!(
        rows.iter().map(|row| row.state).collect::<Vec<_>>(),
        [SessionState::Ready, SessionState::Lost, SessionState::Ended]
    );
    assert!(
        rows.iter()
            .all(|row| row.thread_id == "thread" && row.account_key == "account")
    );
    transaction
        .rollback()
        .expect("no authority switch in this witness");
}
