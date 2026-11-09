//! Storage and commands of the typed approval entity (`approvals` table).
//!
//! The Rust types in `studio_runtime_schema::approvals` own the shape. The
//! table has state columns and generated BEFORE INSERT/UPDATE check triggers.
//! Rows decode straight into the types; an unknown value is an error.
//! An answer is never inferred: a turn end or a lost session closes a pending
//! approval and marks an answering one unknown.

use crate::interaction_tables::ApprovalsRow;
use crate::json_record_codec::ExactJson;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use strum::VariantNames;
use studio_db_api::{Table, db_query, db_update};
use studio_runtime_schema::approvals::{
    AgentId, Answer, AnswerMismatch, Approval, ApprovalId, ApprovalState, ApprovalStateKind,
    CloseReason, CommandId, Decision, Deferral, Delivery, Detail, Hidden, Principal, Request,
    RequestKind, Timestamp, TurnId,
};

pub const TABLE: &str = "approvals";

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("Approval {0} does not exist")]
    NotFound(String),
    #[error(transparent)]
    Mismatch(#[from] AnswerMismatch),
    #[error("Approval {0} already has a different answer")]
    Conflict(String),
    #[error("Approval {0} is closed and takes no answer")]
    Closed(String),
    #[error("Approval {0} is not in a state that allows this change")]
    State(String),
    #[error("Approval {0} needs a turn: its request kind belongs to a provider turn")]
    MissingTurn(String),
    #[error("Approval {0} already exists with a different request")]
    Duplicate(String),
    #[error("Invalid stored approval {id}: {reason}")]
    Stored { id: String, reason: String },
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] studio_json::Error),
}

/// Domain errors (conflict, mismatch, closed) become an invalid stored object
/// here. Callers that report them to a user match `ApprovalError` first.
impl From<ApprovalError> for crate::StoreError {
    fn from(error: ApprovalError) -> Self {
        let text = error.to_string();
        match error {
            ApprovalError::Sql(error) => error.into(),
            ApprovalError::Json(error) => error.into(),
            ApprovalError::NotFound(_)
            | ApprovalError::Mismatch(_)
            | ApprovalError::Conflict(_)
            | ApprovalError::Closed(_)
            | ApprovalError::State(_)
            | ApprovalError::MissingTurn(_)
            | ApprovalError::Duplicate(_)
            | ApprovalError::Stored { .. } => crate::StoreError::InvalidStoredObject {
                table: TABLE,
                id: text,
            },
        }
    }
}

pub type ApprovalResult<T> = Result<T, ApprovalError>;

fn quoted(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

/// Condition under which a row is invalid. `row` is `NEW`.
fn invalid_row(row: &str) -> String {
    let kinds = quoted(RequestKind::VARIANTS);
    let states = quoted(ApprovalStateKind::VARIANTS);
    let reasons = quoted(CloseReason::VARIANTS);
    let no_turn_ok = quoted(&RequestKind::TURN_OPTIONAL.map(RequestKind::as_str));
    format!(
        "NOT ({row}.kind IN ({kinds}) AND ({row}.turn IS NOT NULL OR {row}.kind IN ({no_turn_ok})) AND {row}.kind = json_extract({row}.request,'$.kind') \
         AND {row}.state IN ({states}) \
         AND ({row}.state IN ('answering','answered','unknown')) = ({row}.answer IS NOT NULL) \
         AND ({row}.state = 'answering') = ({row}.command IS NOT NULL) \
         AND ({row}.state = 'answered') = ({row}.answered_by IS NOT NULL) \
         AND ({row}.state IN ('answered','closed')) = ({row}.at IS NOT NULL) \
         AND ({row}.state = 'unknown') = ({row}.since IS NOT NULL) \
         AND ({row}.state = 'closed') = ({row}.reason IS NOT NULL) \
         AND ({row}.reason IS NULL OR {row}.reason IN ({reasons})) \
         AND ({row}.fire_at IS NULL OR {row}.state = 'pending') \
         AND ({row}.deferral_active IS NULL) = ({row}.deferral_at IS NULL) \
         AND ({row}.deferral_by IS NULL) = ({row}.deferral_at IS NULL) \
         AND ({row}.hidden_by IS NULL) = ({row}.hidden_at IS NULL))"
    )
}

/// Create the table and replace the check triggers. Safe to run again.
pub fn create_schema(tx: &Connection) -> rusqlite::Result<()> {
    tx.execute_batch(&ApprovalsRow::DDL.replacen(
        "CREATE TABLE ",
        "CREATE TABLE IF NOT EXISTS ",
        1,
    ))?;
    for index in ApprovalsRow::INDEXES {
        tx.execute_batch(
            &index
                .sql
                .replacen("CREATE INDEX ", "CREATE INDEX IF NOT EXISTS ", 1),
        )?;
    }
    // Replace a trigger only when its text changed, so reopening a database
    // leaves the schema (and its change counter) as it was.
    for (name, event) in [
        ("approvals_check_insert", "INSERT"),
        ("approvals_check_update", "UPDATE"),
    ] {
        let wanted = format!(
            "CREATE TRIGGER {name} BEFORE {event} ON approvals WHEN {}\n             BEGIN SELECT RAISE(ABORT,'invalid approval state'); END",
            invalid_row("NEW")
        );
        let current: Option<String> = tx
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?1",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        if current.as_deref() != Some(wanted.as_str()) {
            tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {name}; {wanted};"))?;
        }
    }
    // A deleted chat closes its open approvals; an answer on its way becomes unknown.
    // The agents table owns the chat; a database without it has no chats to delete.
    let agents: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='runtime_agents')",
        [],
        |row| row.get(0),
    )?;
    if agents {
        let wanted = format!(
            "CREATE TRIGGER {CHAT_DELETE_TRIGGER} AFTER DELETE ON runtime_agents BEGIN
               UPDATE approvals SET state='unknown',command=NULL,since={NOW_SQL}
                 WHERE agent=OLD.id AND state='answering';
               UPDATE approvals SET state='closed',reason='chat_deleted',at={NOW_SQL},fire_at=NULL
                 WHERE agent=OLD.id AND state='pending';
             END"
        );
        let current: Option<String> = tx
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?1",
                [CHAT_DELETE_TRIGGER],
                |row| row.get(0),
            )
            .optional()?;
        if current.as_deref() != Some(wanted.as_str()) {
            tx.execute_batch(&format!(
                "DROP TRIGGER IF EXISTS {CHAT_DELETE_TRIGGER}; {wanted};"
            ))?;
        }
    }
    Ok(())
}

/// Trigger that settles the open approvals of a deleted chat.
pub const CHAT_DELETE_TRIGGER: &str = "approvals_chat_delete";

/// Unix time in seconds, in SQL.
const NOW_SQL: &str = "((julianday('now')-2440587.5)*86400.0)";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderOption {
    label: String,
    description: Option<String>,
    preview: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderQuestion {
    id: Option<String>,
    question: Option<String>,
    header: Option<String>,
    options: Option<Vec<ProviderOption>>,
    is_secret: Option<bool>,
    multi_select: Option<bool>,
    is_other: Option<bool>,
}

/// Decodes the `questions` list of a provider request. A question without an id
/// gets its index. With `strict`, a key Studio does not know is an error (the
/// migration). Otherwise unknown keys are dropped, so a new provider field never
/// loses the user's question.
pub fn questions_from_provider(
    mut list: studio_json::Value,
    strict: bool,
) -> Result<Vec<studio_runtime_schema::approvals::Question>, studio_json::Error> {
    use studio_runtime_schema::approvals::{Question, QuestionOption};
    if !strict {
        const QUESTION_KEYS: [&str; 7] = [
            "id",
            "question",
            "header",
            "options",
            "isSecret",
            "multiSelect",
            "isOther",
        ];
        const OPTION_KEYS: [&str; 3] = ["label", "description", "preview"];
        for question in list.as_array_mut().into_iter().flatten() {
            if let Some(object) = question.as_object_mut() {
                object.retain(|key, _| QUESTION_KEYS.contains(&key.as_str()));
                for option in object
                    .get_mut("options")
                    .and_then(studio_json::Value::as_array_mut)
                    .into_iter()
                    .flatten()
                {
                    if let Some(option) = option.as_object_mut() {
                        option.retain(|key, _| OPTION_KEYS.contains(&key.as_str()));
                    }
                }
            }
        }
    }
    let list = Vec::<ProviderQuestion>::deserialize(list)?;
    Ok(list
        .into_iter()
        .enumerate()
        .map(|(index, q)| Question {
            id: q.id.unwrap_or_else(|| index.to_string()),
            question: q.question.unwrap_or_default(),
            header: q.header,
            options: q
                .options
                .unwrap_or_default()
                .into_iter()
                .map(|o| QuestionOption {
                    label: o.label,
                    description: o.description,
                    preview: o.preview,
                })
                .collect(),
            is_secret: q.is_secret.unwrap_or(false),
            multi_select: q.multi_select,
            is_other: q.is_other,
        })
        .collect())
}

/// What a caller supplies to create an approval.
pub struct NewApproval {
    pub id: ApprovalId,
    pub agent: AgentId,
    /// None only for agent-level kinds (`RequestKind::TURN_OPTIONAL`).
    pub turn: Option<TurnId>,
    pub epoch: Option<u64>,
    pub request: Request,
    pub delivery: Option<Delivery>,
    pub detail: Detail,
    /// Expiry of a pending approval (Claude full access mode), or none.
    pub fire_at: Option<Timestamp>,
}

fn stored(id: &str, reason: &str) -> ApprovalError {
    ApprovalError::Stored {
        id: id.to_owned(),
        reason: reason.to_owned(),
    }
}

fn decode(row: ApprovalsRow) -> ApprovalResult<Approval> {
    let id = &row.id;
    let at = || row.at.map(Timestamp).ok_or_else(|| stored(id, "at"));
    let answer = || {
        row.answer
            .as_ref()
            .map(|v| v.value().clone())
            .ok_or_else(|| stored(id, "answer"))
    };
    let state = match row.state.as_str() {
        "pending" => ApprovalState::Pending,
        "answering" => ApprovalState::Answering {
            answer: answer()?,
            command: CommandId(row.command.clone().ok_or_else(|| stored(id, "command"))?),
        },
        "answered" => ApprovalState::Answered {
            answer: answer()?,
            by: row
                .answered_by
                .as_ref()
                .map(|v| v.value().clone())
                .ok_or_else(|| stored(id, "answered_by"))?,
            at: at()?,
        },
        "unknown" => ApprovalState::Unknown {
            answer: answer()?,
            since: Timestamp(row.since.ok_or_else(|| stored(id, "since"))?),
        },
        "closed" => ApprovalState::Closed {
            reason: CloseReason::deserialize(serde::de::value::StrDeserializer::<
                studio_json::Error,
            >::new(
                row.reason.as_deref().ok_or_else(|| stored(id, "reason"))?,
            ))?,
            at: at()?,
        },
        other => return Err(stored(id, &format!("unknown state {other}"))),
    };
    let deferral = row
        .deferral_active
        .map(|active| -> ApprovalResult<Deferral> {
            Ok(Deferral {
                active: active == 1,
                at: Timestamp(row.deferral_at.ok_or_else(|| stored(id, "deferral_at"))?),
                by: row
                    .deferral_by
                    .as_ref()
                    .map(|v| v.value().clone())
                    .ok_or_else(|| stored(id, "deferral_by"))?,
            })
        })
        .transpose()?;
    let hidden = row
        .hidden_at
        .map(|at| -> ApprovalResult<Hidden> {
            Ok(Hidden {
                at: Timestamp(at),
                by: row
                    .hidden_by
                    .as_ref()
                    .map(|v| v.value().clone())
                    .ok_or_else(|| stored(id, "hidden_by"))?,
            })
        })
        .transpose()?;
    Ok(Approval {
        id: ApprovalId(row.id),
        agent: AgentId(row.agent),
        turn: row.turn.map(TurnId),
        created: Timestamp(row.created),
        request: row.request.value().clone(),
        epoch: row.epoch.map(|v| v as u64),
        delivery: row.delivery.map(|v| v.value().clone()),
        detail: row.detail.value().clone(),
        state,
        deferral,
        fire_at: row.fire_at.map(Timestamp),
        hidden,
    })
}

pub fn get(tx: &Connection, id: &ApprovalId) -> ApprovalResult<Option<Approval>> {
    canvas_read(tx)
        .get::<ApprovalsRow>(&(id.0.clone(),))?
        .map(decode)
        .transpose()
}

fn select(
    tx: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> ApprovalResult<Vec<Approval>> {
    let mut statement = tx.prepare(&format!(
        "{} WHERE {filter} ORDER BY created,id",
        ApprovalsRow::SELECT
    ))?;
    // JSON expressions and JSON-array membership filters stay in this transaction.
    let rows = statement.query_map(args, |row| Ok(decode(ApprovalsRow::read(tx, row)?)))?;
    rows.map(|row| row?).collect()
}

/// Convert one legacy `runtime_requests` record into an approval. Tests seed
/// their fixtures with it.
#[cfg(any(test, feature = "test-support"))]
pub fn from_legacy_record(id: &str, record: &studio_json::Value) -> crate::StoreResult<Approval> {
    // Old fixtures leave out the time and the turn of a provider request.
    let mut record = record.clone();
    if record.get("createdAt").is_none() {
        record["createdAt"] = studio_json::json!(0.0);
    }
    let provider_turn = record["method"]
        .as_str()
        .is_some_and(|method| method.starts_with("item/") || method.starts_with("mcpServer/"));
    if provider_turn && record.get("turnId").is_none() && record["params"].get("turnId").is_none() {
        record["turnId"] = studio_json::json!("turn-0");
    }
    crate::migrations::approval_from_record(id, &record.to_string())
}

/// Insert or replace a whole row. Writers change a row read with `get`.
pub fn put(tx: &Transaction<'_>, row: &Approval) -> ApprovalResult<()> {
    canvas_tx(tx).delete::<ApprovalsRow>(&(row.id.0.clone(),))?;
    insert(tx, row)
}

/// Every approval, hidden ones included, oldest first.
pub fn all(tx: &Connection) -> ApprovalResult<Vec<Approval>> {
    db_query!(
        canvas_read(tx),
        ApprovalsRow,
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect()
}

/// Every approval of an agent, hidden ones included.
pub fn for_agent(tx: &Connection, agent: &AgentId) -> ApprovalResult<Vec<Approval>> {
    db_query!(
        canvas_read(tx),
        ApprovalsRow,
        filter(ApprovalsRow::AGENT.eq(&agent.0)),
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect()
}

/// Pending and answering approvals of a provider account.
pub fn open_for_account(tx: &Connection, account: &str) -> ApprovalResult<Vec<Approval>> {
    select(
        tx,
        "json_extract(delivery,'$.account')=?1 AND state IN ('pending','answering')",
        [account],
    )
}

/// Pending and answering approvals of an agent.
pub fn open_for_agent(tx: &Connection, agent: &AgentId) -> ApprovalResult<Vec<Approval>> {
    db_query!(
        canvas_read(tx),
        ApprovalsRow,
        filter(
            ApprovalsRow::AGENT.eq(&agent.0),
            ApprovalsRow::STATE.in_(&["pending".to_owned(), "answering".to_owned()])
        ),
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect()
}

/// Open approvals for one request-local set of owners; uses the existing agent index.
pub fn open_for_agents(tx: &Connection, ids_json: &str) -> ApprovalResult<Vec<Approval>> {
    select(
        tx,
        "agent IN (SELECT value FROM json_each(?1)) AND state IN ('pending','answering')",
        [ids_json],
    )
}

/// Approvals of one kind and state, for example pending monitor starts.
pub fn of_kind(
    tx: &Connection,
    kind: RequestKind,
    state: ApprovalStateKind,
) -> ApprovalResult<Vec<Approval>> {
    db_query!(
        canvas_read(tx),
        ApprovalsRow,
        filter(
            ApprovalsRow::KIND.eq(&kind.as_str().to_owned()),
            ApprovalsRow::STATE.eq(&ApprovalStateKind::VARIANTS[state as usize].to_owned())
        ),
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect()
}

/// Visible approvals of a turn. Hidden rows stay in the table.
pub fn for_turn(tx: &Connection, turn: &TurnId) -> ApprovalResult<Vec<Approval>> {
    db_query!(
        canvas_read(tx),
        ApprovalsRow,
        filter(
            ApprovalsRow::TURN.eq(&Some(turn.0.clone())),
            ApprovalsRow::HIDDEN_AT.is_null()
        ),
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect()
}

pub(crate) fn insert(tx: &Transaction<'_>, row: &Approval) -> ApprovalResult<()> {
    let (answer, command, by, at, since, reason) = columns(&row.state)?;
    let row = ApprovalsRow {
        id: row.id.0.clone(),
        agent: row.agent.0.clone(),
        turn: row.turn.as_ref().map(|v| v.0.clone()),
        created: row.created.0,
        epoch: row.epoch.map(|v| v as i64),
        kind: RequestKind::from(&row.request).as_str().to_owned(),
        request: ExactJson::from_value(&row.request)?,
        delivery: row
            .delivery
            .as_ref()
            .map(ExactJson::from_value)
            .transpose()?,
        detail: ExactJson::from_value(&row.detail)?,
        state: state_name(&row.state).to_owned(),
        answer: answer.map(ExactJson::from_stored).transpose()?,
        command,
        answered_by: by.map(ExactJson::from_stored).transpose()?,
        at,
        since,
        reason,
        deferral_active: row.deferral.as_ref().map(|v| i64::from(v.active)),
        deferral_at: row.deferral.as_ref().map(|v| v.at.0),
        deferral_by: row
            .deferral
            .as_ref()
            .map(|v| ExactJson::from_value(&v.by))
            .transpose()?,
        fire_at: row.fire_at.map(|v| v.0),
        hidden_at: row.hidden.as_ref().map(|v| v.at.0),
        hidden_by: row
            .hidden
            .as_ref()
            .map(|v| ExactJson::from_value(&v.by))
            .transpose()?,
    };
    canvas_tx(tx).insert(&row)?;
    Ok(())
}

fn state_name(state: &ApprovalState) -> &'static str {
    ApprovalStateKind::VARIANTS[ApprovalStateKind::from(state) as usize]
}

type Columns = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<String>,
);

fn columns(state: &ApprovalState) -> ApprovalResult<Columns> {
    let json = |answer: &Answer| studio_json::to_string(answer);
    Ok(match state {
        ApprovalState::Pending => (None, None, None, None, None, None),
        ApprovalState::Answering { answer, command } => (
            Some(json(answer)?),
            Some(command.0.clone()),
            None,
            None,
            None,
            None,
        ),
        ApprovalState::Answered { answer, by, at } => (
            Some(json(answer)?),
            None,
            Some(studio_json::to_string(by)?),
            Some(at.0),
            None,
            None,
        ),
        ApprovalState::Unknown { answer, since } => {
            (Some(json(answer)?), None, None, None, Some(since.0), None)
        }
        ApprovalState::Closed { reason, at } => (
            None,
            None,
            None,
            Some(at.0),
            None,
            Some(CloseReason::VARIANTS[*reason as usize].to_owned()),
        ),
    })
}

/// Write a new state. The timer ends with the pending state.
fn set_state(tx: &Connection, id: &ApprovalId, state: &ApprovalState) -> ApprovalResult<()> {
    let (answer, command, by, at, since, reason) = columns(state)?;
    // CASE preserves the current expiry only while the approval remains pending.
    tx.execute(
        "UPDATE approvals SET state=?2,answer=?3,command=?4,answered_by=?5,at=?6,since=?7,reason=?8,
         fire_at=CASE WHEN ?2='pending' THEN fire_at END WHERE id=?1",
        params![id.as_str(), state_name(state), answer, command, by, at, since, reason],
    )?;
    Ok(())
}

fn check_turn(new: &NewApproval) -> ApprovalResult<()> {
    if new.turn.is_none() && !RequestKind::TURN_OPTIONAL.contains(&RequestKind::from(&new.request))
    {
        return Err(ApprovalError::MissingTurn(new.id.0.clone()));
    }
    Ok(())
}

/// Create a pending approval. The same id with the same request returns the stored row.
pub fn create(tx: &Transaction<'_>, new: NewApproval, now: Timestamp) -> ApprovalResult<Approval> {
    check_turn(&new)?;
    if let Some(existing) = get(tx, &new.id)? {
        return if existing.request == new.request {
            Ok(existing)
        } else {
            Err(ApprovalError::Duplicate(new.id.0))
        };
    }
    let row = Approval {
        id: new.id,
        agent: new.agent,
        turn: new.turn,
        created: now,
        epoch: new.epoch,
        request: new.request,
        delivery: new.delivery,
        detail: new.detail,
        state: ApprovalState::Pending,
        deferral: None,
        fire_at: new.fire_at,
        hidden: None,
    };
    insert(tx, &row)?;
    Ok(row)
}

/// A policy answer (full access mode) is a row created as `Answered` by `Policy`.
pub fn create_policy_answered(
    tx: &Transaction<'_>,
    new: NewApproval,
    answer: Answer,
    now: Timestamp,
) -> ApprovalResult<Approval> {
    new.request.accepts(&answer)?;
    check_turn(&new)?;
    if get(tx, &new.id)?.is_some() {
        return Err(ApprovalError::Duplicate(new.id.0));
    }
    let row = Approval {
        id: new.id,
        agent: new.agent,
        turn: new.turn,
        created: now,
        epoch: new.epoch,
        request: new.request,
        delivery: new.delivery,
        detail: new.detail,
        state: ApprovalState::Answered {
            answer,
            by: Principal::Policy,
            at: now,
        },
        deferral: None,
        fire_at: None,
        hidden: None,
    };
    insert(tx, &row)?;
    Ok(row)
}

#[derive(Debug, PartialEq)]
pub enum AnswerOutcome {
    /// The answer is stored; the caller queues the delivery command.
    Started(Approval),
    /// The same answer was already given. Nothing changed.
    Replayed(Approval),
}

/// The command `answer(approval, answer)`.
pub fn answer(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    answer: Answer,
    command: CommandId,
) -> ApprovalResult<AnswerOutcome> {
    let mut row = get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))?;
    row.request.accepts(&answer)?;
    match &row.state {
        ApprovalState::Pending => {
            row.state = ApprovalState::Answering { answer, command };
            row.fire_at = None;
            set_state(tx, id, &row.state)?;
            Ok(AnswerOutcome::Started(row))
        }
        ApprovalState::Answering { answer: given, .. }
        | ApprovalState::Answered { answer: given, .. }
        | ApprovalState::Unknown { answer: given, .. } => {
            if *given == answer {
                Ok(AnswerOutcome::Replayed(row))
            } else {
                Err(ApprovalError::Conflict(id.0.clone()))
            }
        }
        ApprovalState::Closed { .. } => Err(ApprovalError::Closed(id.0.clone())),
    }
}

/// The provider confirmed the delivery of the stored answer.
pub fn confirm_delivery(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    by: Principal,
    at: Timestamp,
) -> ApprovalResult<Approval> {
    let mut row = get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))?;
    let (ApprovalState::Answering { answer, .. } | ApprovalState::Unknown { answer, .. }) =
        &row.state
    else {
        return Err(ApprovalError::State(id.0.clone()));
    };
    row.state = ApprovalState::Answered {
        answer: answer.clone(),
        by,
        at,
    };
    set_state(tx, id, &row.state)?;
    Ok(row)
}

/// Delivery of an answer was lost.
pub fn delivery_lost(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    since: Timestamp,
) -> ApprovalResult<Approval> {
    let mut row = get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))?;
    let ApprovalState::Answering { answer, .. } = &row.state else {
        return Err(ApprovalError::State(id.0.clone()));
    };
    row.state = ApprovalState::Unknown {
        answer: answer.clone(),
        since,
    };
    set_state(tx, id, &row.state)?;
    Ok(row)
}

/// The provider connection refused the reply before it left, so the answer
/// never reached the provider. Answering becomes Pending again, with the expiry
/// timer the approval had before the answer.
pub fn reopen(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    fire_at: Option<Timestamp>,
) -> ApprovalResult<Approval> {
    let row = get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))?;
    if !matches!(row.state, ApprovalState::Answering { .. }) {
        return Err(ApprovalError::State(id.0.clone()));
    }
    set_state(tx, id, &ApprovalState::Pending)?;
    db_update!(
        canvas_tx(tx),
        ApprovalsRow,
        set(ApprovalsRow::FIRE_AT = &fire_at.map(|t| t.0)),
        filter(ApprovalsRow::ID.eq(&id.0))
    )?;
    get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))
}

/// Pending becomes Closed. Answering becomes Unknown. Other states stay.
fn settle_open(
    tx: &Connection,
    open: &[Approval],
    reason: CloseReason,
    now: Timestamp,
) -> ApprovalResult<usize> {
    for row in open {
        let state = match &row.state {
            ApprovalState::Pending => ApprovalState::Closed { reason, at: now },
            ApprovalState::Answering { answer, .. } => ApprovalState::Unknown {
                answer: answer.clone(),
                since: now,
            },
            ApprovalState::Answered { .. }
            | ApprovalState::Unknown { .. }
            | ApprovalState::Closed { .. } => continue,
        };
        set_state(tx, &row.id, &state)?;
    }
    Ok(open.len())
}

/// The turn ended.
// F5 must first move the Connection-based domain/Table caller onto its transaction.
pub fn turn_ended(tx: &Connection, turn: &TurnId, now: Timestamp) -> ApprovalResult<usize> {
    let open = select(
        tx,
        "turn=?1 AND state IN ('pending','answering')",
        [turn.as_str()],
    )?;
    settle_open(tx, &open, CloseReason::TurnEnded, now)
}

/// The provider session of an agent is gone.
pub fn session_lost(
    tx: &Transaction<'_>,
    agent: &AgentId,
    now: Timestamp,
) -> ApprovalResult<usize> {
    let open = open_for_agent(tx, agent)?;
    settle_open(tx, &open, CloseReason::SessionLost, now)
}

/// Close one pending approval, for example when the monitor it asks about stops.
/// Other states stay: an answer is never replaced by a close.
pub fn close(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    reason: CloseReason,
    now: Timestamp,
) -> ApprovalResult<bool> {
    let row = get(tx, id)?.ok_or_else(|| ApprovalError::NotFound(id.0.clone()))?;
    if row.state != ApprovalState::Pending {
        return Ok(false);
    }
    set_state(tx, id, &ApprovalState::Closed { reason, at: now })?;
    Ok(true)
}

/// A pending approval whose timer fired and that must be declined.
#[derive(Debug, PartialEq)]
pub struct Expiry {
    pub id: ApprovalId,
    pub agent: AgentId,
    /// The effect the caller delivers to the provider.
    pub effect: Answer,
}

/// Whether the timer of a pending approval has fired. A read: the tick asks
/// this before it takes a write transaction.
pub fn any_due(tx: &Connection, now: Timestamp) -> ApprovalResult<bool> {
    // EXISTS is an aggregate selector, outside the row query grammar.
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM approvals WHERE state='pending' AND fire_at IS NOT NULL AND fire_at<=?1)",
        [now.0],
        |row| row.get(0),
    )?)
}

/// Close every pending approval whose `fire_at` has passed. The timer is the
/// row itself, so this works the same after a restart.
pub fn expire_due(tx: &Transaction<'_>, now: Timestamp) -> ApprovalResult<Vec<Expiry>> {
    let due = db_query!(
        canvas_read(tx),
        ApprovalsRow,
        filter(
            ApprovalsRow::STATE.eq(&"pending".to_owned()),
            ApprovalsRow::FIRE_AT.is_not_null(),
            ApprovalsRow::FIRE_AT.le(&Some(now.0))
        ),
        order_by(ApprovalsRow::CREATED.asc, ApprovalsRow::ID.asc)
    )?
    .into_iter()
    .map(decode)
    .collect::<ApprovalResult<Vec<_>>>()?;
    let mut expired = Vec::new();
    for row in due {
        set_state(
            tx,
            &row.id,
            &ApprovalState::Closed {
                reason: CloseReason::Expired,
                at: now,
            },
        )?;
        db_update!(
            canvas_tx(tx),
            ApprovalsRow,
            set(ApprovalsRow::FIRE_AT = &None::<f64>),
            filter(ApprovalsRow::ID.eq(&row.id.0))
        )?;
        expired.push(Expiry {
            id: row.id,
            agent: row.agent,
            effect: Answer::Decision {
                decision: Decision::Decline,
            },
        });
    }
    Ok(expired)
}

/// History stays. A hidden row is not shown in the default list.
pub fn set_hidden(
    tx: &Transaction<'_>,
    id: &ApprovalId,
    hidden: Option<&Hidden>,
) -> ApprovalResult<()> {
    let hidden_by = hidden.map(|h| ExactJson::from_value(&h.by)).transpose()?;
    let changed = db_update!(
        canvas_tx(tx),
        ApprovalsRow,
        set(
            ApprovalsRow::HIDDEN_AT = &hidden.map(|h| h.at.0),
            ApprovalsRow::HIDDEN_BY = &hidden_by
        ),
        filter(ApprovalsRow::ID.eq(&id.0))
    )?;
    if changed == 0 {
        return Err(ApprovalError::NotFound(id.0.clone()));
    }
    Ok(())
}

impl From<studio_json::error::Error> for ApprovalError {
    fn from(value: studio_json::error::Error) -> Self {
        Self::Json(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_runtime_schema::approvals::{ProviderObject, Question, QuestionAnswer};

    fn db() -> Connection {
        let mut db = Connection::open_in_memory().unwrap();
        rusqlite::vtab::array::load_module(&db).unwrap();
        let tx = db.transaction().unwrap();
        create_schema(&tx).unwrap();
        tx.commit().unwrap();
        db
    }
    fn exec() -> Request {
        Request::Exec {
            command: "ls".into(),
            cwd: "/".into(),
            reason: None,
        }
    }
    fn question() -> Request {
        Request::Question {
            questions: vec![Question {
                id: "0".into(),
                question: "Which?".into(),
                header: None,
                options: vec![],
                is_secret: false,
                multi_select: None,
                is_other: None,
            }],
        }
    }
    fn new(
        id: &str,
        agent: &str,
        turn: &str,
        request: Request,
        fire_at: Option<f64>,
    ) -> NewApproval {
        NewApproval {
            id: ApprovalId(id.into()),
            agent: AgentId(agent.into()),
            turn: Some(TurnId(turn.into())),
            epoch: None,
            request,
            delivery: None,
            detail: Detail {
                params: ProviderObject(Default::default()),
                preview: None,
                answer_error: None,
                host_close_reason: None,
            },
            fire_at: fire_at.map(Timestamp),
        }
    }
    fn accept() -> Answer {
        Answer::Decision {
            decision: Decision::Accept,
        }
    }
    fn state(tx: &Connection, id: &str) -> ApprovalState {
        get(tx, &ApprovalId(id.into())).unwrap().unwrap().state
    }

    #[test]
    fn answer_kind_mismatch_is_rejected() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        create(&tx, new("a", "ag", "t", exec(), None), Timestamp(1.0)).unwrap();
        create(&tx, new("q", "ag", "t", question(), None), Timestamp(1.0)).unwrap();
        let answers = Answer::Answers {
            answers: vec![QuestionAnswer {
                id: "0".into(),
                answer: vec!["x".into()],
                digest: None,
            }],
        };
        assert!(matches!(
            answer(&tx, &ApprovalId("a".into()), answers, CommandId("c".into())),
            Err(ApprovalError::Mismatch(AnswerMismatch::Kind("exec")))
        ));
        assert!(matches!(
            answer(
                &tx,
                &ApprovalId("q".into()),
                accept(),
                CommandId("c".into())
            ),
            Err(ApprovalError::Mismatch(AnswerMismatch::Kind("question")))
        ));
        let wrong_id = Answer::Answers {
            answers: vec![QuestionAnswer {
                id: "9".into(),
                answer: vec![],
                digest: None,
            }],
        };
        assert!(matches!(
            answer(
                &tx,
                &ApprovalId("q".into()),
                wrong_id,
                CommandId("c".into())
            ),
            Err(ApprovalError::Mismatch(AnswerMismatch::UnknownQuestion(_)))
        ));
        assert_eq!(state(&tx, "a"), ApprovalState::Pending);
    }

    #[test]
    fn same_answer_replays_and_a_different_one_conflicts() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        let id = ApprovalId("a".into());
        create(&tx, new("a", "ag", "t", exec(), None), Timestamp(1.0)).unwrap();
        assert!(matches!(
            answer(&tx, &id, accept(), CommandId("c1".into())).unwrap(),
            AnswerOutcome::Started(_)
        ));
        assert!(matches!(
            answer(&tx, &id, accept(), CommandId("c2".into())).unwrap(),
            AnswerOutcome::Replayed(_)
        ));
        let decline = Answer::Decision {
            decision: Decision::Decline,
        };
        assert!(matches!(
            answer(&tx, &id, decline, CommandId("c3".into())),
            Err(ApprovalError::Conflict(_))
        ));
        confirm_delivery(&tx, &id, Principal::Owner, Timestamp(3.0)).unwrap();
        assert!(matches!(
            answer(&tx, &id, accept(), CommandId("c4".into())).unwrap(),
            AnswerOutcome::Replayed(_)
        ));
        assert!(matches!(
            state(&tx, "a"),
            ApprovalState::Answered {
                by: Principal::Owner,
                ..
            }
        ));
    }

    #[test]
    fn turn_end_closes_pending_and_marks_answering_unknown() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        create(&tx, new("p", "ag", "t", exec(), None), Timestamp(1.0)).unwrap();
        create(&tx, new("w", "ag", "t", exec(), None), Timestamp(1.0)).unwrap();
        create(&tx, new("o", "ag", "other", exec(), None), Timestamp(1.0)).unwrap();
        answer(
            &tx,
            &ApprovalId("w".into()),
            accept(),
            CommandId("c".into()),
        )
        .unwrap();
        assert_eq!(
            turn_ended(&tx, &TurnId("t".into()), Timestamp(5.0)).unwrap(),
            2
        );
        assert_eq!(
            state(&tx, "p"),
            ApprovalState::Closed {
                reason: CloseReason::TurnEnded,
                at: Timestamp(5.0)
            }
        );
        assert_eq!(
            state(&tx, "w"),
            ApprovalState::Unknown {
                answer: accept(),
                since: Timestamp(5.0)
            }
        );
        assert_eq!(state(&tx, "o"), ApprovalState::Pending);
        assert!(matches!(
            answer(
                &tx,
                &ApprovalId("p".into()),
                accept(),
                CommandId("c".into())
            ),
            Err(ApprovalError::Closed(_))
        ));
    }

    #[test]
    fn fire_at_expiry_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canvas.sqlite3");
        {
            let mut db = Connection::open(&path).unwrap();
            let tx = db.transaction().unwrap();
            create_schema(&tx).unwrap();
            create(
                &tx,
                new("t", "A", "t1", exec(), Some(600.0)),
                Timestamp(0.0),
            )
            .unwrap();
            create(&tx, new("n", "A", "t1", exec(), None), Timestamp(0.0)).unwrap();
            tx.commit().unwrap();
        }
        let mut db = Connection::open(&path).unwrap();
        let tx = db.transaction().unwrap();
        assert!(expire_due(&tx, Timestamp(599.0)).unwrap().is_empty());
        let expired = expire_due(&tx, Timestamp(601.0)).unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, ApprovalId("t".into()));
        assert_eq!(
            expired[0].effect,
            Answer::Decision {
                decision: Decision::Decline
            }
        );
        assert_eq!(
            state(&tx, "t"),
            ApprovalState::Closed {
                reason: CloseReason::Expired,
                at: Timestamp(601.0)
            }
        );
        assert_eq!(state(&tx, "n"), ApprovalState::Pending);
        assert!(expire_due(&tx, Timestamp(9999.0)).unwrap().is_empty());
    }

    #[test]
    fn a_pending_approval_with_a_timer_loses_it_when_answered() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        create(
            &tx,
            new("t", "A", "t1", exec(), Some(600.0)),
            Timestamp(0.0),
        )
        .unwrap();
        answer(
            &tx,
            &ApprovalId("t".into()),
            accept(),
            CommandId("c".into()),
        )
        .unwrap();
        assert!(expire_due(&tx, Timestamp(9999.0)).unwrap().is_empty());
        assert_eq!(
            get(&tx, &ApprovalId("t".into())).unwrap().unwrap().fire_at,
            None
        );
    }

    #[test]
    fn policy_answer_is_a_row() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        let row = create_policy_answered(
            &tx,
            new("p", "A", "t", exec(), None),
            accept(),
            Timestamp(2.0),
        )
        .unwrap();
        assert_eq!(
            row.state,
            ApprovalState::Answered {
                answer: accept(),
                by: Principal::Policy,
                at: Timestamp(2.0)
            }
        );
        assert_eq!(get(&tx, &ApprovalId("p".into())).unwrap().unwrap(), row);
        assert!(matches!(
            create_policy_answered(
                &tx,
                new("q", "A", "t", question(), None),
                accept(),
                Timestamp(2.0)
            ),
            Err(ApprovalError::Mismatch(_))
        ));
    }

    #[test]
    fn only_agent_level_kinds_may_have_no_turn() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        let mut no_turn = new("a", "ag", "t", exec(), None);
        no_turn.turn = None;
        assert!(matches!(
            create(&tx, no_turn, Timestamp(1.0)),
            Err(ApprovalError::MissingTurn(_))
        ));
        let mut monitor = new(
            "m",
            "ag",
            "t",
            Request::MonitorStart {
                monitor: "m".into(),
                command: "c".into(),
            },
            None,
        );
        monitor.turn = None;
        assert_eq!(create(&tx, monitor, Timestamp(1.0)).unwrap().turn, None);
        assert!(tx
            .execute("UPDATE approvals SET kind='exec', request='{\"kind\":\"exec\",\"command\":\"\",\"cwd\":\"\",\"reason\":null}' WHERE id='m'", [])
            .is_err());
    }

    #[test]
    fn typed_reads_find_open_rows_by_agent_account_and_kind() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        let mut a = new("a", "ag", "t", exec(), None);
        a.delivery = Some(Delivery {
            rpc_id: studio_runtime_schema::approvals::RpcId::Number { value: 1 },
            account: "acc".into(),
            connection: "c".into(),
            supervisor: None,
        });
        create(&tx, a, Timestamp(1.0)).unwrap();
        create(&tx, new("b", "other", "t", exec(), None), Timestamp(2.0)).unwrap();
        answer(
            &tx,
            &ApprovalId("b".into()),
            accept(),
            CommandId("c".into()),
        )
        .unwrap();
        assert_eq!(all(&tx).unwrap().len(), 2);
        assert_eq!(for_agent(&tx, &AgentId("ag".into())).unwrap().len(), 1);
        assert_eq!(open_for_account(&tx, "acc").unwrap().len(), 1);
        assert_eq!(
            open_for_agent(&tx, &AgentId("other".into())).unwrap().len(),
            1
        );
        assert_eq!(
            of_kind(&tx, RequestKind::Exec, ApprovalStateKind::Pending)
                .unwrap()
                .len(),
            1
        );
        let mut row = get(&tx, &ApprovalId("a".into())).unwrap().unwrap();
        row.deferral = Some(Deferral {
            active: true,
            at: Timestamp(3.0),
            by: Principal::Owner,
        });
        put(&tx, &row).unwrap();
        assert_eq!(get(&tx, &ApprovalId("a".into())).unwrap().unwrap(), row);
    }

    #[test]
    fn hidden_rows_stay_in_history() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        create(&tx, new("a", "A", "t", exec(), None), Timestamp(1.0)).unwrap();
        set_hidden(
            &tx,
            &ApprovalId("a".into()),
            Some(&Hidden {
                at: Timestamp(2.0),
                by: Principal::Owner,
            }),
        )
        .unwrap();
        assert!(for_turn(&tx, &TurnId("t".into())).unwrap().is_empty());
        assert!(
            get(&tx, &ApprovalId("a".into()))
                .unwrap()
                .unwrap()
                .hidden
                .is_some()
        );
    }

    #[test]
    fn check_triggers_reject_invalid_rows_and_cover_every_variant() {
        let db = db();
        let ddl: String = db
            .query_row(
                "SELECT group_concat(sql) FROM sqlite_schema WHERE name LIKE 'approvals_check_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        for variant in RequestKind::VARIANTS
            .iter()
            .chain(ApprovalStateKind::VARIANTS)
            .chain(CloseReason::VARIANTS)
        {
            assert!(ddl.contains(&format!("'{variant}'")), "{variant}");
        }
        let insert = |state: &str, extra: &str| {
            db.execute(
                &format!(
                    "INSERT INTO approvals(id,agent,turn,created,kind,request,detail,state{}) VALUES (lower(hex(randomblob(4))),'a','t',1,'exec','{{\"kind\":\"exec\",\"command\":\"\",\"cwd\":\"\",\"reason\":null}}','{{}}','{state}'{})",
                    if extra.is_empty() { "" } else { ",answer" },
                    if extra.is_empty() { String::new() } else { format!(",'{extra}'") }
                ),
                [],
            )
        };
        assert!(insert("pending", "").is_ok());
        assert!(insert("bogus", "").is_err());
        assert!(insert("answered", "").is_err(), "answered needs an answer");
        assert!(
            insert("pending", "{}").is_err(),
            "pending forbids an answer"
        );
        assert!(
            db.execute("UPDATE approvals SET kind='question'", [])
                .is_err()
        );
        assert!(
            db.execute("UPDATE approvals SET state='closed'", [])
                .is_err()
        );
    }

    #[test]
    fn unknown_stored_values_are_errors() {
        let mut db = db();
        db.execute_batch("DROP TRIGGER approvals_check_insert; INSERT INTO approvals(id,agent,turn,created,kind,request,detail,state) VALUES ('x','a','t',1,'exec','{\"kind\":\"nope\"}','{}','pending');").unwrap();
        let tx = db.transaction().unwrap();
        assert!(get(&tx, &ApprovalId("x".into())).is_err());
    }
    #[test]
    fn a_refused_reply_reopens_the_answering_approval_with_its_timer() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        let id = ApprovalId("a".into());
        create(
            &tx,
            new("a", "ag", "t", exec(), Some(100.0)),
            Timestamp(1.0),
        )
        .unwrap();
        answer(&tx, &id, accept(), CommandId("c".into())).unwrap();
        assert_eq!(get(&tx, &id).unwrap().unwrap().fire_at, None);
        let row = reopen(&tx, &id, Some(Timestamp(100.0))).unwrap();
        assert_eq!(row.state, ApprovalState::Pending);
        assert_eq!(row.fire_at, Some(Timestamp(100.0)));
        assert!(matches!(
            reopen(&tx, &id, None),
            Err(ApprovalError::State(_))
        ));
        let expired = expire_due(&tx, Timestamp(101.0)).unwrap();
        assert_eq!(expired.len(), 1, "the restored timer fires");
    }
}
