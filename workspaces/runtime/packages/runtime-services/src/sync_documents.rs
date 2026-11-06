//! Transactional publication of browser-owned sync documents.

use studio_json::Value;
use studio_runtime_store::{RuntimeStore, StoreError, SyncDocument, SyncDocumentsRepository};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct DraftWrite {
    pub version: studio_runtime_schema::drafts::DraftVersion,
    pub payload: Value,
    pub deleted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftPushResult {
    pub conflicts: Vec<SyncDocument>,
    pub changed: bool,
}

#[derive(Debug, Error)]
pub enum DraftPushError {
    #[error("invalid stored sync object drafts/{id}")]
    InvalidStored { id: String },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Apply a batch atomically. A chat has one draft and the last write wins, so the
/// result has no conflicts.
pub async fn push_drafts(
    store: &RuntimeStore,
    rows: Vec<DraftWrite>,
    post_commit: impl FnOnce() + Send + 'static,
) -> Result<DraftPushResult, DraftPushError> {
    let (result, _) = store
        .transaction_with_post_commit(
            move |transaction| {
                // One draft per chat, last write wins: a write never conflicts.
                let conflicts = Vec::new();
                let mut changed = false;
                for row in rows {
                    let id = &row.version.id;
                    let current = SyncDocumentsRepository::draft(transaction, id)?;
                    let current_value = if let Some(current) = &current {
                        if crate::sync_policy::contains_unrepresentable_integer(&current.payload) {
                            return Err(StoreError::InvalidSyncObject {
                                scope: "drafts".to_owned(),
                                id: id.clone(),
                            });
                        }
                        let value =
                            studio_json::from_str::<Value>(&current.payload).map_err(|_| {
                                StoreError::InvalidSyncObject {
                                    scope: "drafts".to_owned(),
                                    id: id.clone(),
                                }
                            })?;
                        if studio_runtime_schema::drafts::DraftVersion::from_stored_value(
                            id,
                            value.clone(),
                        )
                        .is_err()
                        {
                            return Err(StoreError::InvalidSyncObject {
                                scope: "drafts".to_owned(),
                                id: id.clone(),
                            });
                        }
                        Some(value)
                    } else {
                        None
                    };

                    // A chat has one draft: the last write wins, so a write never
                    // conflicts. An unchanged write takes no new number.
                    if let (Some(current), Some(current_value)) = (&current, &current_value)
                        && row.payload == *current_value
                        && row.deleted == current.deleted
                    {
                        continue;
                    }

                    let payload = studio_json::to_string(&row.payload).map_err(StoreError::from)?;
                    changed |=
                        SyncDocumentsRepository::put_draft(transaction, id, &payload, row.deleted)?;
                }
                Ok(DraftPushResult { conflicts, changed })
            },
            move |result| {
                if result.changed {
                    post_commit();
                }
            },
        )
        .await
        .map_err(|error| match error {
            StoreError::InvalidSyncObject { scope, id } if scope == "drafts" => {
                DraftPushError::InvalidStored { id }
            }
            error @ StoreError::StateDirectory(_)
            | error @ StoreError::AlreadyOwned
            | error @ StoreError::IntermediateReleaseRequired
            | error @ StoreError::MissingDatabase(_)
            | error @ StoreError::Migration { .. }
            | error @ StoreError::UnsafeStateDirectory
            | error @ StoreError::SchemaMismatch(_)
            | error @ StoreError::InvalidSyncObject { .. }
            | error @ StoreError::InvalidSyncObjectField { .. }
            | error @ StoreError::InvalidStoredRecord { .. }
            | error @ StoreError::SchemaObjectMismatch { .. }
            | error @ StoreError::Sqlite(_)
            | error @ StoreError::CommitOutcomeUnknown(_)
            | error @ StoreError::RequestResult(_)
            | error @ StoreError::Outbox(_)
            | error @ StoreError::Json(_)
            | error @ StoreError::InvalidStoredObject { .. }
            | error @ StoreError::TranscriptFullTextUnavailable { .. }
            | error @ StoreError::TranscriptInputsIncomplete { .. }
            | error @ StoreError::SchemaValidation(_)
            | error @ StoreError::RecordContract(_)
            | error @ StoreError::WriterPoisoned
            | error @ StoreError::WriterActorInitialization(_)
            | error @ StoreError::WriterActorControl(_)
            | error @ StoreError::CommitActorOutcomeUnknown(_)
            | error @ StoreError::ReadTaskFailed => DraftPushError::Store(error),
        })?;
    Ok(result)
}

/// Bound mutation deltas and advertise a watermark only when no rows were lost.
pub fn with_entity_delta(
    connection: &rusqlite::Connection,
    after: i64,
    mut result: Value,
) -> Result<Value, StoreError> {
    const LIMIT: usize = studio_runtime_schema::shared_values::SYNC_ENTITY_PAGE_LIMIT as usize;
    let (rows, complete) = crate::entity_scope::delta_after(connection, after, LIMIT as i64)?;
    if !rows.is_empty() {
        result["_syncEntities"] = Value::Array(rows.iter().filter_map(entity_change).collect());
        if complete {
            result["_syncEntitiesAfter"] = studio_json::json!(after);
        }
    }
    Ok(result)
}

/// Normalize a historical stored entity envelope without serializing it again.
/// Pull, stream, and every mutation envelope share this read boundary.
pub fn entity_payload(row: &studio_runtime_store::EntityRow) -> Option<Value> {
    let mut payload: Value = studio_json::from_str(row.payload.as_deref()?).ok()?;
    if !matches!(
        studio_runtime_schema::normalize_sync_entity_payload(
            &row.collection,
            &row.id,
            row.deleted,
            &mut payload
        ),
        Ok(None)
    ) {
        return None;
    }
    Some(payload)
}

/// Reuse the parsed envelope map when a mutation response is already a Value.
pub fn entity_change(row: &studio_runtime_store::EntityRow) -> Option<Value> {
    let mut payload = entity_payload(row)?;
    let object = payload.as_object_mut()?;
    if row.deleted {
        object.remove("value");
    }
    object.insert(
        "op".to_owned(),
        Value::String(if row.deleted { "delete" } else { "upsert" }.to_owned()),
    );
    object.insert("seq".to_owned(), Value::from(row.sequence));
    Some(payload)
}
