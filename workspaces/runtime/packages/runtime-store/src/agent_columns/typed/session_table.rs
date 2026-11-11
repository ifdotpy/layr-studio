//! Prepared session table. Registration and writer authority remain with family cutover.
use studio_db_api::migration::{CopyPlan, Evidence};
use studio_db_api::{CodecError, DbQuery, DbTable, Table, Tx};

use super::AgentIdCodec;
use crate::agent_tables::SessionsStorageRow;
use crate::domain::{SessionId, SessionRow, SessionState};
use crate::peer_update_settings::Canvas;

/// Logical session identity remains the key used by the ordinary CopyPlan.
/// The original rowid is an ordering value, not a target physical identity.
#[derive(Debug, PartialEq, Eq, DbTable)]
#[db(table="__f2_sessions",database=Canvas,strict)]
#[db(index(name="f2_sessions_thread",columns(thread_id,account_key,source_rowid),predicate=state.ne(SessionState::Ended)))]
#[db(index(name = "f2_sessions_chat", columns(chat, source_rowid)))]
pub struct TypedSessionRow {
    #[db(key)]
    id: String,
    pub source_rowid: i64,
    #[db(codec=AgentIdCodec)]
    pub chat: crate::AgentId,
    pub thread_id: String,
    pub account_key: String,
    pub state: SessionState,
}

impl TypedSessionRow {
    pub fn from_source(rowid: i64, row: SessionRow) -> Self {
        Self {
            id: row.id.0,
            source_rowid: rowid,
            chat: row.chat,
            thread_id: row.thread_id,
            account_key: row.account_key,
            state: row.state,
        }
    }

    pub fn session_id(&self) -> SessionId {
        SessionId(self.id.clone())
    }

    pub fn into_domain(self) -> SessionRow {
        SessionRow {
            id: SessionId(self.id),
            chat: self.chat,
            thread_id: self.thread_id,
            account_key: self.account_key,
            state: self.state,
        }
    }
}

impl From<SessionsStorageRow> for TypedSessionRow {
    fn from(row: SessionsStorageRow) -> Self {
        Self {
            id: row.id,
            source_rowid: row.source_rowid,
            chat: row.chat.into(),
            thread_id: row.thread_id,
            account_key: row.account_key,
            state: row.state,
        }
    }
}

fn evidence(
    id: &str,
    source_rowid: i64,
    chat: &str,
    thread: &str,
    account: &str,
    state: SessionState,
) -> Evidence {
    Evidence {
        identity: vec![id.to_owned().into()],
        ordering: vec![source_rowid.into()],
        values: vec![
            chat.to_owned().into(),
            thread.to_owned().into(),
            account.to_owned().into(),
            state.column().to_owned().into(),
        ],
    }
}

/// One-to-one plan; the family owner supplies its authority-switch callback.
pub fn session_copy_plan(
    finish: fn(&mut Tx<'_, Canvas>) -> Result<(), CodecError>,
) -> CopyPlan<SessionsStorageRow, TypedSessionRow> {
    CopyPlan {
        name: "consolidate-f2-sessions",
        source_keys: &["id"],
        source_key: |row| (row.id.clone(),),
        target_key: Clone::clone,
        transform: |row| Ok(row.into()),
        source_evidence: |row| {
            Ok(evidence(
                &row.id,
                row.source_rowid,
                &row.chat,
                &row.thread_id,
                &row.account_key,
                row.state,
            ))
        },
        target_evidence: |row| {
            Ok(evidence(
                &row.id,
                row.source_rowid,
                row.chat.as_ref(),
                &row.thread_id,
                &row.account_key,
                row.state,
            ))
        },
        target_select: TypedSessionRow::SELECT_QUERY,
        finish,
    }
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=TypedSessionRow,row=TypedSessionRow,
    tables(rows=TypedSessionRow),columns(rowid=TypedSessionRow::SOURCE_ROWID,id=TypedSessionRow::ID,chat=TypedSessionRow::CHAT,
    thread=TypedSessionRow::THREAD_ID,account=TypedSessionRow::ACCOUNT_KEY,state=TypedSessionRow::STATE),
    sql="SELECT {id},{rowid},{chat},{thread},{account},{state} FROM {rows} WHERE {chat}=?1 ORDER BY {rowid}")]
pub struct SessionsForChat {
    #[db(column=TypedSessionRow::CHAT)]
    pub chat: crate::AgentId,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=TypedSessionRow,row=TypedSessionRow,
    tables(rows=TypedSessionRow),columns(rowid=TypedSessionRow::SOURCE_ROWID,id=TypedSessionRow::ID,chat=TypedSessionRow::CHAT,
    thread=TypedSessionRow::THREAD_ID,account=TypedSessionRow::ACCOUNT_KEY,state=TypedSessionRow::STATE),
    sql="SELECT {id},{rowid},{chat},{thread},{account},{state} FROM {rows} WHERE {thread}=?1 AND {account}=?2 AND {state}!=?3 ORDER BY {rowid}")]
pub struct SessionsForThread {
    #[db(column=TypedSessionRow::THREAD_ID)]
    pub thread_id: String,
    #[db(column=TypedSessionRow::ACCOUNT_KEY)]
    pub account_key: String,
    #[db(column=TypedSessionRow::STATE)]
    pub excluding: SessionState,
}
