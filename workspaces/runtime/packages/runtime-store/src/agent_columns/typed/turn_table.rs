//! Prepared turns retain one attempt owner and source ordering, with closed INTEGER states.
use serde::Deserialize;
use studio_db_api::DbTable;
use studio_runtime_schema::start_attempt::StartAttemptAction;

use super::{AgentIdCodec, StoredTurnAttempt, TurnAttemptCodec};
use crate::agent_tables::TurnsStorageRow;
use crate::domain::{SessionId, TurnId, TurnState, TurnStateKind};
use crate::peer_update_settings::Canvas;

const TURN_SESSION_SHAPE: &str = studio_db_api::static_sql!(
    "(state=",
    TurnStateKind::DB_CODE_RUNNING,
    ")=(session IS NOT NULL)"
);
const TURN_UNFINISHED: &str = studio_db_api::static_sql!(
    "state IN (",
    TurnStateKind::DB_CODE_STARTING,
    ",",
    TurnStateKind::DB_CODE_RUNNING,
    ")"
);
const TURN_ATTEMPT_SHAPE: &str = "(attempt IS NOT NULL OR (attempt_submitted IS NULL AND attempt_connection_generation IS NULL AND attempt_action IS NULL AND attempt_legacy_action IS NULL AND attempt_active_at_reservation IS NULL)) AND (attempt_action IS NULL OR attempt_legacy_action IS NULL)";

#[derive(Debug, PartialEq, DbTable)]
#[db(table="__f2_turns",database=Canvas,strict,check=check_turn)]
#[db(index(name="f2_turns_one_attempt",columns(chat),unique,predicate=attempt.is_not_null()))]
#[db(index(name="f2_turns_callback_context",columns(attempt_connection_generation,chat,source_rowid),predicate=attempt.is_not_null()))]
#[db(index(name="f2_turns_one_unfinished",columns(chat),unique,predicate=TURN_UNFINISHED))]
#[db(index(
    name = "f2_turns_provider_turn",
    columns(chat, provider_turn, source_rowid)
))]
pub struct TypedTurnRow {
    #[db(key)]
    id: String,
    pub source_rowid: i64,
    #[db(codec=AgentIdCodec)]
    pub chat: crate::AgentId,
    #[db(check=TURN_SESSION_SHAPE)]
    pub state: TurnStateKind,
    pub session: Option<String>,
    pub provider_turn: Option<String>,
    #[db(codec=TurnAttemptCodec,check=TURN_ATTEMPT_SHAPE)]
    pub attempt: Option<StoredTurnAttempt>,
    pub interrupt_ticket: Option<String>,
    pub attempt_submitted: Option<i64>,
    pub attempt_connection_generation: Option<i64>,
    pub attempt_action: Option<StartAttemptAction>,
    pub attempt_legacy_action: Option<String>,
    pub attempt_active_at_reservation: Option<i64>,
}

impl TypedTurnRow {
    pub fn turn_id(&self) -> TurnId {
        TurnId(self.id.clone())
    }

    pub fn turn_state(&self) -> crate::StoreResult<TurnState> {
        TurnState::from_storage_kind(self.state, self.session.clone().map(SessionId))
    }
}

impl TryFrom<TurnsStorageRow> for TypedTurnRow {
    type Error = crate::StoreError;
    fn try_from(row: TurnsStorageRow) -> Result<Self, Self::Error> {
        let state = TurnState::from_columns(&row.state, row.session.clone())?.storage_kind();
        let (attempt_action, attempt_legacy_action) = match row.attempt_action {
            None => (None, None),
            Some(wire) => {
                match StartAttemptAction::deserialize(serde::de::value::StrDeserializer::<
                    serde::de::value::Error,
                >::new(&wire))
                {
                    Ok(action) => (Some(action), None),
                    Err(_) => (None, Some(wire)),
                }
            }
        };
        let result = Self {
            id: row.id,
            source_rowid: row.source_rowid,
            chat: row.chat.into(),
            state,
            session: row.session,
            provider_turn: row.provider_turn,
            attempt: row
                .attempt
                .map(|raw| StoredTurnAttempt::from_stored(raw.into_raw()))
                .transpose()?,
            interrupt_ticket: row.interrupt_ticket,
            attempt_submitted: row.attempt_submitted,
            attempt_connection_generation: row.attempt_connection_generation,
            attempt_action,
            attempt_legacy_action,
            attempt_active_at_reservation: row.attempt_active_at_reservation,
        };
        check_turn(&result)?;
        Ok(result)
    }
}

fn check_turn(row: &TypedTurnRow) -> crate::StoreResult<()> {
    row.turn_state()?;
    let invalid =
        || crate::StoreError::RecordContract("turn attempt selectors disagree with their payload");
    if row.attempt_action.is_some() && row.attempt_legacy_action.is_some() {
        return Err(invalid());
    }
    if row.attempt.is_none()
        && (row.attempt_submitted.is_some()
            || row.attempt_connection_generation.is_some()
            || row.attempt_action.is_some()
            || row.attempt_legacy_action.is_some()
            || row.attempt_active_at_reservation.is_some())
    {
        return Err(invalid());
    }
    if let Some(normal) = row.attempt.as_ref().and_then(StoredTurnAttempt::normal) {
        let generation = normal
            .connection_generation
            .map(i64::try_from)
            .transpose()
            .map_err(|_| invalid())?;
        if row.attempt_submitted != normal.submitted.map(i64::from)
            || row.attempt_connection_generation != generation
            || row.attempt_action != normal.action
            || row.attempt_legacy_action.is_some()
            || row.attempt_active_at_reservation != normal.active_at_reservation.map(i64::from)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
