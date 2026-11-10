//! A malformed monitor is held without changing its row, output, or receipts.
use crate::monitor_policy::MonitorError;
use rusqlite::OptionalExtension;
use studio_runtime_schema::studio_errors::{Cause, StudioError, Target};
use studio_runtime_store::{RuntimeStore, StoreError, Transaction};

pub(crate) async fn hold_error(
    store: &RuntimeStore,
    error: &MonitorError,
    operation: &'static str,
    now: f64,
) -> Result<bool, MonitorError> {
    let Some((id, location)) = located(error) else {
        return Ok(false);
    };
    store
        .transaction_with(move |tx| hold_in(tx, &id, &location, operation, now))
        .await
        .map_err(MonitorError::from)
}

pub(crate) fn hold_error_in(
    tx: &Transaction<'_>,
    error: &MonitorError,
    operation: &str,
    now: f64,
) -> Result<bool, MonitorError> {
    let Some((id, location)) = located(error) else {
        return Ok(false);
    };
    hold_in(tx, &id, &location, operation, now).map_err(MonitorError::from)
}

fn located(error: &MonitorError) -> Option<(String, String)> {
    if let MonitorError::InvalidStored { id, location } = error {
        return Some((id.clone(), location.clone()));
    }
    if let MonitorError::Event(crate::event_write::EventWriteError::InvalidStoredObject {
        object: "monitors" | "runtime_monitors",
        id,
    }) = error
    {
        return Some((id.clone(), "/record".into()));
    }
    if let Some(StoreError::InvalidStoredObject {
        table: "runtime_monitors" | "monitors",
        id,
    })
    | Some(StoreError::InvalidStoredRecord {
        object: "runtime_monitors" | "monitors",
        id,
    }) = error.store_error()
    {
        return Some((id.clone(), "/record".into()));
    }
    None
}

pub(crate) fn hold_in(
    tx: &Transaction<'_>,
    id: &str,
    location: &str,
    operation: &str,
    now: f64,
) -> studio_runtime_store::StoreResult<bool> {
    let raw: Option<(String, Option<String>)> = tx.query_row(
        "SELECT record,CASE WHEN json_valid(record) THEN CASE WHEN json_type(record,'$.agent')='text' THEN json_extract(record,'$.agent') END END FROM runtime_monitors WHERE id=?1",
        [id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(StoreError::from)?;
    let Some((raw, agent)) = raw else {
        return Ok(false);
    };
    // Typed loading erases serde's field path. Recover just that path, never log
    // the retained payload or the offending value.
    let mut decoder = studio_json::Deserializer::from_str(&raw);
    let field = match serde_path_to_error::deserialize::<_, studio_runtime_store::MonitorRecord>(
        &mut decoder,
    ) {
        Ok(record) if record.id != id => "/id".into(),
        Ok(_) => location.to_owned(),
        Err(error) => {
            let path = error.path().to_string();
            if path == "." {
                // Missing named fields have a root serde path. Only extract
                // the field name; other error text can contain retained values.
                error
                    .inner()
                    .to_string()
                    .strip_prefix("missing field `")
                    .and_then(|rest| rest.split_once('`'))
                    .map_or_else(|| "/record".into(), |(field, _)| format!("/{field}"))
            } else {
                format!("/{path}")
            }
        }
    };
    let incident_id = format!("{}{id}", studio_runtime_store::MONITOR_HOLD_PREFIX);
    let mut incident: StudioError = crate::studio_errors::invalid_source(
        incident_id,
        "runtime_monitors",
        id,
        now,
        vec![Cause::StoredRecord {
            object: "runtime_monitors".into(),
            id: id.into(),
            pointer: Some(field.clone()),
        }],
    );
    incident.monitor_id = Some(id.into());
    incident.detail = format!(
        "Monitor held at {operation}, field {field}. Its outcome remains unknown; retained work is not replayed."
    );
    if let Some(agent_id) = agent {
        incident.target = Target::Check {
            agent_name: agent_id.clone(),
            agent_id,
        };
    }
    studio_runtime_store::error_incidents::put(tx, &incident)?;
    crate::diagnostics::log_stderr(format_args!(
        "monitor_row_held id={id} operation={operation} field={field}"
    ));
    Ok(true)
}
