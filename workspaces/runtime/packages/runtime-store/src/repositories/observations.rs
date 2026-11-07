//! Tables of account observations whose client documents are built at read time
//! (#630 item 127): the account list summary (`accounts`), the cost summary of each
//! account (`accountCost`) and of each session (`sessionCost`), and the desktop
//! application state (`desktop`) and the model catalogs (`model`). The rate-limit views have their own owner
//! (`account_state::RATE_LIMITS`). A row's `record` is the envelope
//! `{"collection":..,"id":..,"value":..}` that the readers decode; the document is its
//! `value`.
use rusqlite::{Connection, OptionalExtension};

use crate::StoreResult;

/// One observation table and the client collection of its documents.
#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub table: &'static str,
    pub collection: &'static str,
}

/// The account list summary; its one row is `current`.
pub const ACCOUNT_SUMMARIES: Observation = Observation {
    table: "runtime_account_summaries",
    collection: "accounts",
};

/// The cost summary of each account key.
pub const ACCOUNT_COSTS: Observation = Observation {
    table: "runtime_account_costs",
    collection: "accountCost",
};

/// The cost summary of each session root.
pub const SESSION_COSTS: Observation = Observation {
    table: "runtime_session_costs",
    collection: "sessionCost",
};

/// The desktop application state; its one row is `current`.
pub const DESKTOP: Observation = Observation {
    table: "runtime_desktop_states",
    collection: "desktop",
};

/// The model catalog of each provider account; ids are `account:model`.
pub const MODELS: Observation = Observation {
    table: "runtime_models",
    collection: "model",
};

/// Every observation table.
pub const ALL: [Observation; 5] = [
    ACCOUNT_SUMMARIES,
    ACCOUNT_COSTS,
    SESSION_COSTS,
    DESKTOP,
    MODELS,
];

/// The observation table of client collection `collection`.
pub fn of_collection(collection: &str) -> Option<Observation> {
    ALL.into_iter()
        .find(|observation| observation.collection == collection)
}

/// The ids of every stored observation of `observation`.
pub fn ids(connection: &Connection, observation: Observation) -> StoreResult<Vec<String>> {
    let mut statement =
        connection.prepare_cached(&format!("SELECT id FROM {} ORDER BY id", observation.table))?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Store `value` as observation `id`. Returns whether the record changed.
pub fn put<T: serde::Serialize>(
    connection: &Connection,
    observation: Observation,
    id: &str,
    value: &T,
) -> StoreResult<bool> {
    let record = studio_json::to_string(
        &studio_json::json!({"collection": observation.collection, "id": id, "value": value}),
    )?;
    Ok(connection.execute(
        &format!(
            "INSERT INTO {}(id,record) VALUES(?1,?2)
             ON CONFLICT(id) DO UPDATE SET record=excluded.record WHERE record IS NOT excluded.record",
            observation.table
        ),
        rusqlite::params![id, record],
    )? > 0)
}

/// Remove observation `id`. Returns whether a row was removed.
pub fn delete(connection: &Connection, observation: Observation, id: &str) -> StoreResult<bool> {
    Ok(connection.execute(
        &format!("DELETE FROM {} WHERE id=?1", observation.table),
        [id],
    )? > 0)
}

/// The stored envelope of observation `id`.
pub fn record(
    connection: &Connection,
    observation: Observation,
    id: &str,
) -> StoreResult<Option<String>> {
    Ok(connection
        .query_row(
            &format!("SELECT record FROM {} WHERE id=?1", observation.table),
            [id],
            |row| row.get(0),
        )
        .optional()?)
}
