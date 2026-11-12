//! Scoped intervals use one short read transaction per page. Large intervals spool
//! bounded wire frames to an unlinked temporary file, owned by the live stream.
use std::collections::BTreeMap;
use std::io::{Seek, Write};

use axum::response::sse::Event;
use studio_json::{Value, json};
use studio_runtime_schema::shared_values::{
    SYNC_ENTITY_STREAM_MAX_DOCUMENTS, SYNC_RESOURCE_TRANSFER_FRAGMENT_MAX_BYTES,
    SYNC_STREAM_EVENT_MAX_BYTES,
};
use studio_runtime_schema::sync_protocol::ResourceTransferPayloadKind;
use studio_runtime_store::SyncDocumentsRepository;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{ApiError, InternalErrorKind};
use crate::generated::{
    EntityChangeBatch, ResourceChangeEvent, ResourceRef, ResourceRevisionEntry, SubscriptionCursor,
};
use crate::router::HttpState;

fn io_error() -> ApiError {
    ApiError::internal_with_cause(InternalErrorKind::StoreIo)
}

fn data_error() -> ApiError {
    ApiError::internal_with_cause(InternalErrorKind::StoreData)
}

pub(super) struct TransferReader(BufReader<tokio::fs::File>);

#[cfg(test)]
tokio::task_local! {
    // Small pages and one deterministic interleaving through the real prepare path.
    static PAGE_INTERLEAVING: (i64, Box<dyn Fn(i64, i64) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync>);
}
impl TransferReader {
    pub(super) async fn next(&mut self) -> Result<Option<Event>, ApiError> {
        let mut line = String::new();
        if self.0.read_line(&mut line).await.map_err(|_| io_error())? == 0 {
            return Ok(None);
        }
        let data = line.strip_suffix('\n').ok_or_else(data_error)?;
        Ok(Some(Event::default().event("entity-transfer").data(data)))
    }
}

pub(super) fn owns(resource: &ResourceRef) -> bool {
    super::subscription_entities::selection(resource).is_some()
}

/// The same per-scope cursor is used for the next delta; no changed notification
/// rereads or retransmits the complete scoped history after the baseline.
pub(super) async fn attach(
    state: &HttpState,
    event: &mut ResourceChangeEvent,
    since: &BTreeMap<String, SubscriptionCursor>,
    sequences: &mut BTreeMap<String, SubscriptionCursor>,
) -> Result<Vec<TransferReader>, ApiError> {
    let mut readers = Vec::new();
    let mut retained = Vec::new();
    let mut entries = Vec::new();
    for (resource, entry) in std::mem::take(&mut event.resources)
        .into_iter()
        .zip(std::mem::take(&mut event.resource_versions))
    {
        if !owns(&resource) {
            retained.push(resource);
            entries.push(entry);
            continue;
        }
        let key = studio_json::to_string(&resource).map_err(ApiError::json)?;
        let cursor = since
            .get(&key)
            .or_else(|| sequences.get(&key))
            .filter(|cursor| cursor.workspace_id == state.workspace_id)
            .cloned();
        let (entry, reader, cursor) = prepare(state, event, resource.clone(), cursor).await?;
        if matches!(
            event.reason,
            crate::generated::ResourceChangeEventReason::Change
        ) && entry.as_ref().is_some_and(|entry| {
            entry.reply_kind == Some(crate::generated::SubscriptionReplyKind::Delta)
                && entry
                    .entity_changes
                    .as_ref()
                    .and_then(Option::as_ref)
                    .is_some_and(|batch| batch.documents.is_empty())
        }) {
            // Keep the delivered base: the client never received this empty interval.
            continue;
        }
        sequences.insert(key, cursor);
        if let Some(reader) = reader {
            readers.push(reader);
        }
        if let Some(entry) = entry {
            retained.push(resource);
            entries.push(entry);
        }
    }
    event.resources = retained;
    event.resource_versions = entries;
    Ok(readers)
}

/// Preserve full transcript/view entries when one ordinary resources frame cannot
/// hold them. The unlinked spool uses the same serialized-fragment writer as entities.
pub(super) async fn attach_payloads(
    event: &mut ResourceChangeEvent,
    validators: &crate::validation::Registry,
) -> Result<Vec<TransferReader>, ApiError> {
    let metadata = json!({"protocol":event.protocol,"workspaceId":event.workspace_id,"epoch":event.epoch,"revision":event.revision,"reason":event.reason,"resources":[],"resourceVersions":[]});
    let resources = std::mem::take(&mut event.resources);
    let entries = std::mem::take(&mut event.resource_versions);
    let validators = validators.clone();
    let (retained, readers) = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-http/resource-transfer",
        move || {
            let mut retained = Vec::new();
            let mut readers = Vec::new();
            for (resource, mut entry) in resources.into_iter().zip(entries) {
                let kind = if entry.transcript_snapshot.is_some() || entry.transcript_changes.is_some() {
                    ResourceTransferPayloadKind::Transcript
                } else if entry.view_snapshot.is_some() {
                    ResourceTransferPayloadKind::View
                } else {
                    retained.push((resource, entry));
                    continue;
                };
                let mut ordinary = metadata.clone();
                ordinary["resources"] = json!([&resource]);
                ordinary["resourceVersions"] = json!([&entry]);
                if "event: resources\nid: \ndata: \n\n".len() + metadata["revision"].to_string().len()
                    + studio_json::to_vec(&ordinary).map_err(ApiError::json)?.len() <= SYNC_STREAM_EVENT_MAX_BYTES {
                    retained.push((resource, entry));
                    continue;
                }
                entry.resource = Some(Some(resource.clone()));
                let cursor = entry.cursor.get_or_insert_with(|| SubscriptionCursor {
                    terminal_generation: None,
                    workspace_id: metadata["workspaceId"].as_str().expect("typed workspace").to_owned(),
                    seq: entry.revision,
                }).clone();
                let reply = entry.reply_kind.get_or_insert(crate::generated::SubscriptionReplyKind::Snapshot).clone();
                let frame = json!({
                    "protocol": metadata["protocol"], "workspaceId": metadata["workspaceId"],
                    "epoch": metadata["epoch"], "revision": metadata["revision"], "reason": metadata["reason"],
                    "resource": resource, "cursor": cursor, "replyKind": reply,
                    "payloadKind": kind, "transferId": uuid::Uuid::new_v4().to_string()
                });
                let mut file = tempfile::tempfile().map_err(|_| io_error())?;
                spool_page(&mut file, &frame, 0, true, &entry, &validators)?;
                file.rewind().map_err(|_| io_error())?;
                readers.push(TransferReader(BufReader::new(tokio::fs::File::from_std(file))));
            }
            Ok::<_, ApiError>((retained, readers))
        },
    ).await.map_err(|_| io_error())??;
    for (resource, entry) in retained {
        event.resources.push(resource);
        event.resource_versions.push(entry);
    }
    Ok(readers)
}

async fn read_page(
    state: &HttpState,
    resource: &ResourceRef,
    since: Option<i64>,
    interval: Option<studio_runtime_store::EntityPage>,
) -> Result<studio_runtime_store::EntityPage, ApiError> {
    // Shutdown stops between pages; an in-flight SQLite statement still finishes.
    if state.sync_changes.snapshot().closed {
        return Err(io_error());
    }
    let (scope, id, agent) =
        super::subscription_entities::selection(resource).ok_or_else(data_error)?;
    let collections = studio_json::to_string(&scope.collections).map_err(ApiError::json)?;
    let id = id.map(str::to_owned);
    let agent = agent.map(str::to_owned);
    let limit = SYNC_ENTITY_STREAM_MAX_DOCUMENTS;
    #[cfg(test)]
    let limit = PAGE_INTERLEAVING
        .try_with(|(limit, _)| *limit)
        .unwrap_or(limit);
    state
        .store
        .as_ref()
        .ok_or_else(|| ApiError::internal_with_cause(InternalErrorKind::StoreConfiguration))?
        .read_async("runtime-http/entity-transfer", move |tx| {
            if let Some(interval) = interval {
                let (_, durable_floor) = studio_runtime_store::durable_seq::high_and_floor(tx)?;
                let floor = SyncDocumentsRepository::entity_tombstone_floor(tx)?.max(durable_floor);
                // A delta cannot skip an unread deletion pruned since its last page.
                // Reject before publishing the interval cursor; reconnect resets it.
                if !interval.reset && interval.checkpoint < floor {
                    return Err(ApiError::new(axum::http::StatusCode::CONFLICT,
                        "Entity history expired while preparing the sync response; retry the subscription"));
                }
                return Ok(studio_runtime_services::entity_scope::continue_page(
                    tx,
                    &interval,
                    Some(&collections),
                    id.as_deref(),
                    agent.as_deref(),
                    scope.include_team_members,
                    limit,
                )?);
            }
            // Team member insertion, deletion and restoration can replace a scope.
            let visibility = if scope.include_team_members {
                agent
                    .as_deref()
                    .map(|agent| SyncDocumentsRepository::team_membership_revision(tx, agent))
                    .transpose()?
                    .unwrap_or(0)
            } else {
                0
            };
            let since = since.filter(|after| *after >= visibility);
            Ok(studio_runtime_services::entity_scope::resume(
                tx,
                since,
                Some(&collections),
                id.as_deref(),
                agent.as_deref(),
                scope.include_team_members,
                limit,
            )?)
        })
        .await
}

async fn prepare(
    state: &HttpState,
    event: &ResourceChangeEvent,
    resource: ResourceRef,
    since: Option<SubscriptionCursor>,
) -> Result<
    (
        Option<ResourceRevisionEntry>,
        Option<TransferReader>,
        SubscriptionCursor,
    ),
    ApiError,
> {
    let workspace = state.workspace_id.clone();
    let epoch = event.epoch.clone();
    let revision = event.revision;
    let reason = event.reason.clone();
    let mut page = read_page(
        state,
        &resource,
        since.as_ref().map(|cursor| cursor.seq),
        None,
    )
    .await?;
    #[cfg(test)]
    if let Ok(interleave) =
        PAGE_INTERLEAVING.try_with(|(_, hook)| hook(page.checkpoint, page.max_sequence))
    {
        interleave.await;
    }
    // Pages are at least watermark, not point-in-time: mutable projections
    // can see newer content. Keep this upper sequence bound and replay from
    // it afterwards. Keyed upserts/tombstones converge; membership revisions
    // force scope replacement, including removals. Never restart on drift.
    let through = page.max_sequence;
    let snapshot = page.reset;
    let mut after = if snapshot {
        0
    } else {
        since.as_ref().map_or(0, |cursor| cursor.seq)
    };
    let cursor = SubscriptionCursor {
        terminal_generation: None,
        workspace_id: workspace.clone(),
        seq: through,
    };
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let metadata = json!({"protocol":studio_runtime_schema::SYNC_PROTOCOL_VERSION,"workspaceId":workspace,"epoch":epoch,"revision":revision,"reason":reason,"resource":resource,"cursor":cursor,"replyKind":if snapshot {"snapshot"}else{"delta"},"transferId":transfer_id,"payloadKind":ResourceTransferPayloadKind::Entity});
    let mut spool = None;
    let mut page_index = 0;
    loop {
        let checkpoint = page.checkpoint;
        let final_sql_page = checkpoint == through;
        let (entry, next_spool, next_index) = {
            let rows = std::mem::take(&mut page.rows);
            let validators = state.validators.clone();
            let resource = resource.clone();
            let cursor = cursor.clone();
            let workspace = workspace.clone();
            let epoch = epoch.clone();
            let reason = reason.clone();
            let metadata = metadata.clone();
            studio_runtime_process::blocking_pool::spawn_blocking("runtime-http/entity-transfer-spool", move || {
            studio_runtime_store::debug_assert_outside_sql();
            // A stored row the current contract cannot read is skipped with a log line,
            // so one legacy row cannot stop the whole stream.
            let documents = rows.iter().filter(|row| !snapshot || !row.deleted).filter_map(|row| {
                let document = super::sync_documents::entity_document(row);
                if document.is_none() {
                    eprintln!("warning: sync entity skipped, payload does not match the contract: collection={} id={} seq={}", row.collection, row.id, row.sequence);
                }
                document
            }).collect::<Vec<_>>();
            let batches = byte_pages(after, checkpoint, documents)?;
            for (index, batch) in batches.iter().enumerate() {
                let complete = final_sql_page && index + 1 == batches.len();
                if cfg!(debug_assertions) && !studio_json::to_value(batch).is_ok_and(|value| validators.validate_component("EntityChangeBatch", &value)) { return Err(data_error()); }
                if page_index == 0 && complete {
                    let entry = ResourceRevisionEntry { resource: Some(Some(resource.clone())), revision: through, cursor: Some(cursor.clone()), reply_kind: Some(if snapshot { crate::generated::SubscriptionReplyKind::Snapshot } else { crate::generated::SubscriptionReplyKind::Delta }), entity_changes: Some(Some(batch.clone())), native_layr_settings: None, discovery_snapshot: None, voice_records: None, view_snapshot: None, draft_changes: None, draft_snapshot: None, entity_sequence_reset: None, entity_sequences: None, terminal_base_offset: None, transcript_changes: None, transcript_snapshot: None };
                    let ordinary = ResourceChangeEvent { protocol: i64::from(studio_runtime_schema::SYNC_PROTOCOL_VERSION), workspace_id: workspace.clone(), epoch: epoch.clone(), revision, reason: reason.clone(), resources: vec![resource.clone()], resource_versions: vec![entry.clone()] };
                    if "event: resources\nid: \ndata: \n\n".len() + revision.to_string().len() + studio_json::to_vec(&ordinary).map_err(ApiError::json)?.len() <= SYNC_STREAM_EVENT_MAX_BYTES { return Ok::<_, ApiError>((Some(entry), None, page_index)); }
                }
                let file = match &mut spool { Some(file) => file, None => spool.insert(tempfile::tempfile().map_err(|_| io_error())?) };
                spool_page(file, &metadata, page_index, complete, batch, &validators)?;
                page_index += 1;
            }
            if final_sql_page {
                spool.as_mut().ok_or_else(data_error)?.rewind().map_err(|_| io_error())?;
            }
            Ok((None, spool, page_index))
            }).await.map_err(|_| io_error())??
        };
        if let Some(entry) = entry {
            return Ok((Some(entry), None, cursor));
        }
        spool = next_spool;
        page_index = next_index;
        if final_sql_page {
            break;
        }
        after = checkpoint;
        page = read_page(state, &resource, None, Some(page)).await?;
    }
    let file = spool.ok_or_else(data_error)?;
    Ok((
        None,
        Some(TransferReader(BufReader::new(tokio::fs::File::from_std(
            file,
        )))),
        cursor,
    ))
}

/// Split at record boundaries first. A single existing oversized canonical
/// record remains one page and uses serialization fragments in spool_page.
fn byte_pages(
    after: i64,
    through: i64,
    documents: Vec<crate::generated::SyncEntity>,
) -> Result<Vec<EntityChangeBatch>, ApiError> {
    let mut batches = Vec::new();
    let mut batch = EntityChangeBatch {
        after,
        through,
        documents: Vec::new(),
    };
    let mut bytes = 0;
    for document in documents {
        let size = studio_json::to_vec(&document)
            .map_err(ApiError::json)?
            .len();
        // Documents built from one main-table change share its number. A batch ends
        // only between numbers, so the next batch never repeats its `after`.
        let same_change = batch
            .documents
            .last()
            .is_some_and(|last| last.seq == document.seq);
        if !batch.documents.is_empty()
            && !same_change
            && bytes + size > SYNC_STREAM_EVENT_MAX_BYTES / 2
        {
            batch.through = batch.documents.last().ok_or_else(data_error)?.seq;
            let next = EntityChangeBatch {
                after: batch.through,
                through,
                documents: Vec::new(),
            };
            batches.push(std::mem::replace(&mut batch, next));
            bytes = 0;
        }
        bytes += size;
        batch.documents.push(document);
    }
    batches.push(batch);
    Ok(batches)
}

fn spool_page(
    file: &mut std::fs::File,
    metadata: &Value,
    page_index: usize,
    complete: bool,
    page: &impl serde::Serialize,
    validators: &crate::validation::Registry,
) -> Result<(), ApiError> {
    studio_runtime_store::debug_assert_outside_sql();
    let data = studio_json::to_string(page).map_err(ApiError::json)?;
    let mut direct = metadata.clone();
    let fields = direct.as_object_mut().ok_or_else(data_error)?;
    fields.insert("pageIndex".into(), json!(page_index));
    fields.insert("partIndex".into(), json!(0));
    fields.insert("partCount".into(), json!(1));
    fields.insert("complete".into(), json!(complete));
    fields.insert("data".into(), json!(&data));
    let encoded = studio_json::to_vec(&direct).map_err(ApiError::json)?;
    if "event: entity-transfer\ndata: \n\n".len() + encoded.len() <= SYNC_STREAM_EVENT_MAX_BYTES {
        if cfg!(debug_assertions)
            && !validators.validate_component("ResourceEntityTransferEvent", &direct)
        {
            return Err(data_error());
        }
        file.write_all(&encoded).map_err(|_| io_error())?;
        file.write_all(b"\n").map_err(|_| io_error())?;
        return Ok(());
    }
    // JSON string escaping can expand each source byte sixfold. Leave room for
    // the resource selector and authenticated stream metadata, then verify exactly.
    let fragment_bytes = SYNC_RESOURCE_TRANSFER_FRAGMENT_MAX_BYTES;
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let mut end = (offset + fragment_bytes).min(data.len());
        while !data.is_char_boundary(end) {
            end -= 1;
        }
        parts.push(&data[offset..end]);
        offset = end;
    }
    for (index, part) in parts.iter().enumerate() {
        let mut frame = metadata.clone();
        let fields = frame.as_object_mut().ok_or_else(data_error)?;
        fields.insert("pageIndex".into(), json!(page_index));
        fields.insert("partIndex".into(), json!(index));
        fields.insert("partCount".into(), json!(parts.len()));
        fields.insert(
            "complete".into(),
            json!(complete && index + 1 == parts.len()),
        );
        fields.insert("data".into(), json!(part));
        let encoded = studio_json::to_vec(&frame).map_err(ApiError::json)?;
        if "event: entity-transfer\ndata: \n\n".len() + encoded.len() > SYNC_STREAM_EVENT_MAX_BYTES
            || (cfg!(debug_assertions)
                && !validators.validate_component("ResourceEntityTransferEvent", &frame))
        {
            return Err(data_error());
        }
        file.write_all(&encoded).map_err(|_| io_error())?;
        file.write_all(b"\n").map_err(|_| io_error())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::{IntoResponse, Sse};
    use std::sync::Arc;
    use studio_runtime_store::RuntimeStore;

    #[test]
    fn a_batch_never_splits_documents_of_one_change() {
        let text = "x".repeat(SYNC_STREAM_EVENT_MAX_BYTES / 3);
        let document = |id: &str, seq: i64| {
            super::super::sync_documents::entity_document(&studio_runtime_store::EntityRow {
                collection: "workspace".into(),
                id: id.into(),
                sequence: seq,
                payload: Some(
                    json!({"collection":"workspace","id":id,"value":{"stateDir":text}}).to_string(),
                ),
                deleted: false,
            })
            .expect("document")
        };
        let batches = byte_pages(
            0,
            3,
            vec![
                document("a", 2),
                document("b", 2),
                document("c", 2),
                document("d", 3),
            ],
        )
        .expect("batches");
        assert_eq!(
            batches.len(),
            2,
            "the byte limit cuts once, between numbers"
        );
        assert_eq!(batches[0].documents.len(), 3);
        assert_eq!(batches[0].through, 2);
        assert_eq!(batches[1].after, 2);
        assert_eq!(batches[1].documents[0].seq, 3);
    }

    async fn fixture() -> (tempfile::TempDir, HttpState) {
        let directory = tempfile::tempdir().expect("isolated source");
        let store = Arc::new(
            RuntimeStore::open(Some(directory.path()))
                .await
                .expect("store"),
        );
        let workspace = store
            .test_read_transaction(|tx| SyncDocumentsRepository::workspace_id(tx))
            .expect("identity");
        let mut state = HttpState::new(
            "token".into(),
            workspace,
            directory.path().display().to_string(),
            4620,
            None,
            Some(store),
            None,
        );
        state.validators = crate::validation::Registry::compile(&[]).expect("canonical validators");
        (directory, state)
    }

    #[tokio::test]
    async fn delta_pages_reject_pruned_unread_tombstone_before_watermark() {
        for durable in [false, true] {
            let (_directory, state) = fixture().await;
            let store = state.store.as_ref().expect("store");
            let resource =
                ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
            let (_, _, baseline) = prepare(&state, &event(&state), resource.clone(), None)
                .await
                .expect("baseline");
            assert_eq!(baseline.seq, 1);
            store
                .transaction(|tx| {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        "a",
                        json!({"stateDir":"a"}),
                        false,
                    )?;
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        "current",
                        json!({}),
                        true,
                    )?;
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        "b",
                        json!({"stateDir":"b"}),
                        false,
                    )?;
                    Ok(())
                })
                .await
                .expect("delta with unread deletion");
            let prune_store = store.clone();
            let hook = Box::new(
                move |checkpoint, watermark| -> futures_util::future::BoxFuture<'static, ()> {
                    let store = prune_store.clone();
                    Box::pin(async move {
                        assert_eq!((checkpoint, watermark), (2, 4));
                        store
                        .transaction(move |tx| {
                            // Exercise the real pruner without creating 10,000 filler tombstones.
                            studio_runtime_store::test_fixtures::canvas::set_meta(
                                tx,
                                "entity_tombstone_count",
                                Some(
                                    &(studio_runtime_store::ENTITY_TOMBSTONE_LIMIT + 1).to_string(),
                                ),
                            )?;
                            assert_eq!(SyncDocumentsRepository::prune_entity_tombstones(tx)?, 1);
                            assert_eq!(SyncDocumentsRepository::entity_tombstone_floor(tx)?, 3);
                    if durable {
                        studio_runtime_store::test_fixtures::canvas::set_meta(tx, "entity_tombstone_floor", Some("0"))?;
                        studio_runtime_store::test_fixtures::canvas::seed_durable_floor(tx, 3)?;
                    }
                            Ok(())
                        })
                        .await
                        .expect("prune checkpoint 2 < floor 3 < watermark 4");
                    })
                },
            );
            let result = PAGE_INTERLEAVING
                .scope(
                    (1, hook),
                    prepare(
                        &state,
                        &event(&state),
                        resource.clone(),
                        Some(baseline.clone()),
                    ),
                )
                .await;
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("an incomplete delta must never publish watermark 4"),
            };
            assert_eq!(error.status(), axum::http::StatusCode::CONFLICT);
            let (entry, _, _) = prepare(&state, &event(&state), resource, Some(baseline))
                .await
                .expect("retry from last delivered cursor");
            let entry = entry.expect("fresh inline snapshot");
            assert_eq!(
                entry.reply_kind,
                Some(crate::generated::SubscriptionReplyKind::Snapshot)
            );
            assert!(!docs_of(Some(entry)).iter().any(|row| row.id == "current"));
        }
    }

    async fn transfer_documents(
        entry: Option<ResourceRevisionEntry>,
        mut reader: Option<TransferReader>,
    ) -> Vec<crate::generated::SyncEntity> {
        let mut documents = entry.map(|entry| docs_of(Some(entry))).unwrap_or_default();
        if let Some(reader) = reader.as_mut() {
            while let Some(frame) = reader.next().await.expect("transfer frame") {
                let bytes = wire(frame).await;
                let text = std::str::from_utf8(&bytes).expect("UTF-8");
                let frame: Value = studio_json::from_str(
                    text.lines()
                        .find_map(|line| line.strip_prefix("data: "))
                        .expect("data"),
                )
                .expect("frame JSON");
                assert_eq!(frame["partCount"], 1);
                let page: EntityChangeBatch =
                    studio_json::from_str(frame["data"].as_str().expect("page JSON"))
                        .expect("batch");
                documents.extend(page.documents);
            }
        }
        documents
    }

    #[tokio::test]
    async fn projected_pages_replay_main_row_updates_deletes_and_membership() {
        for kind in ["state", "work", "tasks", "taskDetail"] {
            let (_directory, state) = fixture().await;
            let store = state.store.as_ref().expect("store");
            studio_runtime_services::entity_publication::install(store)
                .await
                .expect("publisher");
            store
                .transaction(|tx| {
                    put_member(tx, "lead", "lead")?;
                    put_member(tx, "worker", "lead")?;
                    put_task(tx, "lead-0", "lead", "running")?;
                    put_task(tx, "lead-1", "lead", "running")?;
                    put_task(tx, "worker-0", "worker", "running")?;
                    Ok(())
                })
                .await
                .expect("team and window projections");
            put_work(
                store,
                "w0",
                Some(work_record("w0", "lead", "lead", "ready", 0)),
            )
            .await;
            put_work(
                store,
                "w1",
                Some(work_record("w1", "lead", "lead", "ready", 0)),
            )
            .await;
            let resource = match kind {
                "state" => ResourceRef::StateResource(
                    studio_runtime_schema::resource_ref::EmptyResource {},
                ),
                "work" => studio_json::from_str(r#"{"kind":"work","agentId":"lead"}"#)
                    .expect("work scope"),
                "tasks" => studio_json::from_str(r#"{"kind":"tasks","agentId":"lead"}"#)
                    .expect("tasks scope"),
                _ => studio_json::from_str(r#"{"kind":"taskDetail","taskId":"lead-1"}"#)
                    .expect("direct projection"),
            };
            let changed_store = store.clone();
            let multipage = kind != "taskDetail";
            let hook = Box::new(
                move |checkpoint, watermark| -> futures_util::future::BoxFuture<'static, ()> {
                    let store = changed_store.clone();
                    Box::pin(async move {
                        if multipage {
                            assert!(
                                checkpoint < watermark,
                                "interleave before an unread projection page"
                            );
                        }
                        store
                            .transaction(|tx| {
                                let mut lead =
                                    studio_runtime_store::AgentRepository::get(tx, &"lead".into())?
                                        .expect("lead");
                                lead.name = Some("newer".into());
                                studio_runtime_store::AgentRepository::put(tx, &lead)?;
                                let mut worker = studio_runtime_store::AgentRepository::get(
                                    tx,
                                    &"worker".into(),
                                )?
                                .expect("worker");
                                worker.deleted_at = Some(1.0);
                                studio_runtime_store::AgentRepository::put(tx, &worker)?;
                                put_task(tx, "lead-0", "lead", "completed")?;
                                delete_task(tx, "lead-0")?;
                                put_task(tx, "lead-1", "lead", "completed")?;
                                Ok(())
                            })
                            .await
                            .expect("agent projection and membership/window changes");
                        put_work(
                            &store,
                            "w0",
                            Some(work_record("w0", "lead", "lead", "review", 1)),
                        )
                        .await;
                        put_work(&store, "w0", None).await;
                        put_work(
                            &store,
                            "w1",
                            Some(work_record("w1", "lead", "lead", "review", 1)),
                        )
                        .await;
                    })
                },
            );
            let (entry, reader, baseline) = PAGE_INTERLEAVING
                .scope(
                    (1, hook),
                    prepare(&state, &event(&state), resource.clone(), None),
                )
                .await
                .expect("snapshot under writes");
            let _baseline_documents = transfer_documents(entry, reader).await;
            let (entry, reader, replay) =
                prepare(&state, &event(&state), resource, Some(baseline.clone()))
                    .await
                    .expect("replay from captured watermark");
            assert!(replay.seq > baseline.seq);
            let reset = entry.as_ref().and_then(|entry| entry.reply_kind.clone());
            let rows = transfer_documents(entry, reader).await;
            let find = |collection: &str, id: &str| {
                rows.iter()
                    .find(|row| row.collection == collection && row.id == id)
            };
            match kind {
                "state" => {
                    let lead = find("agentSummary", "lead").expect("read-time agent projection");
                    assert_eq!(lead.value.as_ref().expect("value")["name"], "newer");
                    assert!(lead.seq > baseline.seq);
                }
                "work" => {
                    assert_eq!(
                        find("workDetail", "w0").expect("update then delete").op,
                        crate::generated::SyncEntityOp::Delete
                    );
                    assert_eq!(
                        find("workDetail", "w1")
                            .expect("work projection")
                            .value
                            .as_ref()
                            .expect("value")["status"],
                        "review"
                    );
                    assert!(find("workResult", "w1:0").is_some());
                }
                "tasks" => {
                    assert_eq!(
                        reset,
                        Some(crate::generated::SubscriptionReplyKind::Snapshot)
                    );
                    assert!(
                        find("task", "worker-0").is_none(),
                        "membership removal replaces the scope"
                    );
                    assert!(
                        find("task", "lead-0").is_none(),
                        "update then delete remains absent"
                    );
                    assert_eq!(
                        find("task", "lead-1")
                            .expect("remaining window member")
                            .value
                            .as_ref()
                            .expect("value")["status"],
                        "completed"
                    );
                }
                _ => assert_eq!(
                    find("taskDetail", "lead-1")
                        .expect("direct projection")
                        .value
                        .as_ref()
                        .expect("value")["status"],
                    "completed"
                ),
            }
        }
    }

    #[tokio::test]
    async fn snapshot_pages_replay_updates_and_tombstones_from_watermark() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        store
            .transaction(|tx| {
                for index in 0..=SYNC_ENTITY_STREAM_MAX_DOCUMENTS {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        &format!("page-{index}"),
                        json!({"stateDir":"before"}),
                        false,
                    )?;
                }
                Ok(())
            })
            .await
            .expect("multiple SQL pages");
        let resource =
            ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
        let mut page = read_page(&state, &resource, None, None)
            .await
            .expect("first page");
        let watermark = page.max_sequence;
        assert!(page.checkpoint < watermark);
        let mut displayed = BTreeMap::new();
        for row in std::mem::take(&mut page.rows) {
            displayed.insert(row.id.clone(), row);
        }
        assert!(displayed.contains_key("page-0"));
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(tx, "workspace", "page-0", json!({}), true)?;
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    &format!("page-{SYNC_ENTITY_STREAM_MAX_DOCUMENTS}"),
                    json!({"stateDir":"newer"}),
                    false,
                )?;
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "inserted",
                    json!({"stateDir":"newer"}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("delete emitted row, update unread row, insert after watermark");
        store
            .flush_wal_for_shutdown()
            .await
            .expect("checkpoint between SQL pages");
        assert_eq!(
            tokio::fs::metadata(store.root().join("canvas.sqlite3-wal"))
                .await
                .expect("WAL")
                .len(),
            0
        );
        while page.checkpoint < watermark {
            page = read_page(&state, &resource, None, Some(page))
                .await
                .expect("continuation");
            assert_eq!(
                page.max_sequence, watermark,
                "writes cannot move the captured upper bound"
            );
            for row in std::mem::take(&mut page.rows) {
                displayed.insert(row.id.clone(), row);
            }
        }
        let replay = read_page(&state, &resource, Some(watermark), None)
            .await
            .expect("replay");
        assert!(!replay.reset);
        assert_eq!(replay.rows.len(), 3);
        for row in &replay.rows {
            if row.deleted {
                displayed.remove(&row.id);
            } else {
                displayed.insert(row.id.clone(), row.clone());
            }
        }
        assert!(!displayed.contains_key("page-0"));
        for id in [
            format!("page-{SYNC_ENTITY_STREAM_MAX_DOCUMENTS}"),
            "inserted".into(),
        ] {
            assert!(
                displayed[&id]
                    .payload
                    .as_ref()
                    .expect("value")
                    .contains("newer")
            );
        }
        state.sync_changes.close();
        assert!(
            read_page(&state, &resource, None, Some(replay))
                .await
                .is_err(),
            "shutdown refuses another SQL page"
        );
    }

    #[tokio::test]
    async fn snapshot_spooling_and_slow_consumer_release_read_transactions() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        store
            .transaction(|tx| {
                for index in 0..=SYNC_ENTITY_STREAM_MAX_DOCUMENTS {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        &format!("page-{index}"),
                        json!({"stateDir":"fixture"}),
                        false,
                    )?;
                }
                Ok(())
            })
            .await
            .expect("multiple SQL pages");
        // spool_page's SQL boundary assertion also checks every page before any
        // client I/O: the old implementation spools inside its read transaction.
        let resource =
            ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
        let (entry, reader, _) = prepare(&state, &event(&state), resource, None)
            .await
            .expect("snapshot");
        assert!(entry.is_none());
        let mut reader = reader.expect("multi-page transfer");
        assert!(reader.next().await.expect("first frame").is_some());
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "later",
                    json!({"stateDir":"later"}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("write while consumer is paused");
        store
            .flush_wal_for_shutdown()
            .await
            .expect("checkpoint mid-stream");
        assert_eq!(
            tokio::fs::metadata(store.root().join("canvas.sqlite3-wal"))
                .await
                .expect("WAL")
                .len(),
            0
        );
        assert!(
            reader.next().await.expect("remaining frame").is_some(),
            "checkpoint runs before the consumer finishes"
        );
    }
    /// Write a task through the canonical task writer, which keeps its window.
    fn put_task(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        agent: &str,
        status: &str,
    ) -> Result<(), studio_runtime_store::StoreError> {
        studio_runtime_services::notification_dispatch::test_put_task(
            tx,
            &task_record(id, agent, status),
        )
        .map_err(|_| studio_runtime_store::StoreError::RecordContract("task write"))
    }

    fn delete_task(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
    ) -> Result<(), studio_runtime_store::StoreError> {
        studio_runtime_services::notification_dispatch::test_delete_task(tx, id)
            .map_err(|_| studio_runtime_store::StoreError::RecordContract("task delete"))
    }

    /// A task row with the given id, owner and status.
    fn task_record(id: &str, agent: &str, status: &str) -> studio_runtime_store::TaskRecord {
        studio_json::from_str(
            &json!({"id":id,"agent":agent,"itemId":id,"turnId":"turn","created":1,
                "type":"commandExecution","name":"commandExecution","kind":"command","status":status})
            .to_string(),
        )
        .expect("task fixture")
    }

    fn event(state: &HttpState) -> ResourceChangeEvent {
        studio_json::from_value(json!({"protocol":studio_runtime_schema::SYNC_PROTOCOL_VERSION,"workspaceId":state.workspace_id,"epoch":"fixture","revision":1,"reason":"initial","resources":[],"resourceVersions":[]})).expect("event")
    }
    async fn wire(event: Event) -> Vec<u8> {
        let response = Sse::new(futures_util::stream::iter([Ok::<
            _,
            std::convert::Infallible,
        >(event)]))
        .into_response();
        axum::body::to_bytes(response.into_body(), SYNC_STREAM_EVENT_MAX_BYTES + 1)
            .await
            .expect("bounded frame")
            .to_vec()
    }
    #[tokio::test]
    async fn reconnect_seq_cursor_delivers_exact_changes_and_empty_live_intervals_are_suppressed() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        let resource =
            ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
        let (_, _, baseline) = prepare(&state, &event(&state), resource.clone(), None)
            .await
            .expect("baseline");
        store
            .transaction(|tx| {
                for index in 0..3 {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        &format!("change-{index}"),
                        json!({"stateDir":"changed"}),
                        false,
                    )?;
                }
                SyncDocumentsRepository::put_entity(tx, "workspace", "removed", json!({}), true)?;
                SyncDocumentsRepository::put_entity(
                    tx,
                    "agentDetail",
                    "unrelated",
                    json!({}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("four scoped changes while disconnected");
        let (entry, _, cursor) = prepare(
            &state,
            &event(&state),
            resource.clone(),
            Some(baseline.clone()),
        )
        .await
        .expect("reconnect");
        let candidate = entry.expect("inline delta");
        let batch = candidate
            .entity_changes
            .as_ref()
            .and_then(Option::as_ref)
            .expect("changes");
        assert_eq!(batch.after, baseline.seq);
        assert_eq!(
            batch.documents.len(),
            4,
            "exactly the missed changes, including deletion"
        );
        assert_eq!(batch.documents[3].id, "removed");
        assert_eq!(
            batch.documents[3].op,
            crate::generated::SyncEntityOp::Delete
        );
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(
                    tx,
                    "agentDetail",
                    "unrelated",
                    json!({"ignored":true}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("unrelated notification");
        let mut live = event(&state);
        live.reason = crate::generated::ResourceChangeEventReason::Change;
        live.resources.push(resource.clone());
        live.resource_versions.push(candidate.clone());
        let mut cursors = BTreeMap::from([(
            studio_json::to_string(&resource).expect("key"),
            cursor.clone(),
        )]);
        let readers = attach(&state, &mut live, &BTreeMap::new(), &mut cursors)
            .await
            .expect("empty live interval");
        assert!(readers.is_empty());
        assert!(
            live.resources.is_empty(),
            "an empty live delta must not notify a consumer"
        );
        assert_eq!(
            cursors.values().next().expect("retained cursor").seq,
            cursor.seq,
            "a suppressed interval cannot silently advance the delivered base"
        );
        let (reconnected, _, _) = prepare(
            &state,
            &event(&state),
            resource.clone(),
            Some(cursor.clone()),
        )
        .await
        .expect("current reconnect");
        assert!(
            reconnected
                .expect("reconnect acknowledgement")
                .entity_changes
                .flatten()
                .expect("empty delta")
                .documents
                .is_empty()
        );
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "later",
                    json!({"stateDir":"later"}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("later visible change");
        live.resources.push(resource.clone());
        live.resource_versions.push(candidate);
        attach(&state, &mut live, &BTreeMap::new(), &mut cursors)
            .await
            .expect("later delta");
        let later = live.resource_versions[0]
            .entity_changes
            .as_ref()
            .and_then(Option::as_ref)
            .expect("later batch");
        assert_eq!(
            later.after, cursor.seq,
            "client's durable base survives the suppressed notification"
        );
        assert_eq!(later.documents.len(), 1);
    }

    #[tokio::test]
    async fn spool_frame_is_emitted_without_reencoding() {
        let mut file = tempfile::tempfile().expect("spool");
        let saved = r#"{ "z": 1, "a": 2 }"#;
        writeln!(file, "{saved}").expect("write frame");
        file.rewind().expect("rewind");
        let mut reader = TransferReader(BufReader::new(tokio::fs::File::from_std(file)));
        let frame = reader.next().await.expect("read frame").expect("frame");
        assert_eq!(
            wire(frame).await,
            format!("event: entity-transfer\ndata: {saved}\n\n").as_bytes()
        );
    }
    fn put_member(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        root: &str,
    ) -> Result<(), studio_runtime_store::StoreError> {
        let agent = studio_runtime_store::AgentRecord::from_value(json!({"compactions":0,"id":id,
            "rootId":root,"epoch":1,"status":"idle","accountKey":"default","provider":"codex"}))?;
        studio_runtime_store::AgentRepository::put(tx, &agent)
    }

    #[tokio::test]
    async fn team_insertion_deletion_and_restore_replace_scopes_while_content_stays_delta() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().unwrap();
        studio_runtime_services::entity_publication::install(store)
            .await
            .unwrap();
        store
            .transaction(|tx| {
                put_member(tx, "lead", "lead")?;
                put_member(tx, "worker", "lead")?;
                put_task(tx, "worker-task", "worker", "running")?;
                put_task(tx, "latest", "lead", "running")?;
                // The task predates membership, so a row-only delta cannot introduce it later.
                put_task(tx, "waiting-task", "waiting", "running")?;
                Ok(())
            })
            .await
            .unwrap();
        let resource: ResourceRef =
            studio_json::from_value(json!({"kind":"tasks","agentId":"lead"})).unwrap();
        let (_, _, mut cursor) = prepare(&state, &event(&state), resource.clone(), None)
            .await
            .unwrap();
        store
            .transaction(|tx| {
                assert!(
                    put_member(tx, "worker", "other")
                        .unwrap_err()
                        .to_string()
                        .contains("immutable")
                );
                assert!(put_task(tx, "worker-task", "lead", "running").is_err());
                Ok(())
            })
            .await
            .unwrap();
        for (operation, expected) in [
            ("delete", vec!["latest"]),
            ("restore", vec!["latest", "worker-task"]),
            ("insert", vec!["latest", "waiting-task", "worker-task"]),
        ] {
            store.transaction(move |tx| {
                match operation {
                    "delete" => { tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.deletedAt',1) WHERE id='worker'", [])?; }
                    "restore" => { tx.execute("UPDATE runtime_agents SET record=json_remove(record,'$.deletedAt') WHERE id='worker'", [])?; }
                    "insert" => { put_member(tx, "waiting", "lead")?; }
                    _ => unreachable!(),
                }
                Ok(())
            }).await.unwrap();
            let (entry, reader, next) =
                prepare(&state, &event(&state), resource.clone(), Some(cursor))
                    .await
                    .unwrap();
            assert!(reader.is_none());
            let entry = entry.unwrap();
            assert_eq!(
                entry.reply_kind,
                Some(crate::generated::SubscriptionReplyKind::Snapshot),
                "{operation}"
            );
            let mut ids = docs_of(Some(entry))
                .into_iter()
                .map(|row| row.id)
                .collect::<Vec<_>>();
            ids.sort();
            assert_eq!(ids, expected, "{operation}");
            cursor = next;
        }
        store
            .transaction(|tx| {
                put_member(tx, "unrelated", "other")?;
                put_task(tx, "latest", "lead", "completed")?;
                Ok(())
            })
            .await
            .unwrap();
        let (entry, _, next) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .unwrap();
        let entry = entry.unwrap();
        assert_eq!(
            entry.reply_kind,
            Some(crate::generated::SubscriptionReplyKind::Delta)
        );
        assert_eq!(
            docs_of(Some(entry))
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["latest"]
        );
        store
            .transaction(|tx| {
                put_task(tx, "appended", "lead", "running")?;
                Ok(())
            })
            .await
            .unwrap();
        let (entry, _, _) = prepare(&state, &event(&state), resource, Some(next))
            .await
            .unwrap();
        let entry = entry.unwrap();
        assert_eq!(
            entry.reply_kind,
            Some(crate::generated::SubscriptionReplyKind::Delta)
        );
        assert_eq!(
            docs_of(Some(entry))
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["appended"]
        );
    }

    #[tokio::test]
    async fn same_owner_task_delete_and_restore_remain_incremental() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().unwrap();
        store
            .transaction(|tx| {
                put_member(tx, "alpha", "alpha")?;
                put_task(tx, "restored", "alpha", "running")?;
                Ok(())
            })
            .await
            .unwrap();
        let resource: ResourceRef =
            studio_json::from_value(json!({"kind":"tasks","agentId":"alpha"})).unwrap();
        let (_, _, baseline) = prepare(&state, &event(&state), resource.clone(), None)
            .await
            .unwrap();
        store
            .transaction(|tx| {
                delete_task(tx, "restored")?;
                Ok(())
            })
            .await
            .unwrap();
        let (entry, _, deleted) = prepare(&state, &event(&state), resource.clone(), Some(baseline))
            .await
            .unwrap();
        let entry = entry.unwrap();
        assert_eq!(
            entry.reply_kind,
            Some(crate::generated::SubscriptionReplyKind::Delta)
        );
        assert_eq!(
            docs_of(Some(entry))[0].op,
            crate::generated::SyncEntityOp::Delete
        );
        store
            .transaction(|tx| {
                put_task(tx, "restored", "alpha", "running")?;
                Ok(())
            })
            .await
            .unwrap();
        let (entry, _, _) = prepare(&state, &event(&state), resource, Some(deleted))
            .await
            .unwrap();
        let entry = entry.unwrap();
        assert_eq!(
            entry.reply_kind,
            Some(crate::generated::SubscriptionReplyKind::Delta)
        );
        let documents = docs_of(Some(entry));
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].op, crate::generated::SyncEntityOp::Upsert);
        let projection: studio_runtime_schema::process_controls::TaskProjection =
            studio_json::from_value(documents[0].value.clone().unwrap()).unwrap();
        assert_eq!(projection.agent, "alpha");
    }

    #[tokio::test]
    async fn snapshot_streams_the_other_rows_when_one_stored_row_is_unreadable() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        store
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "readable",
                    json!({"stateDir":"fixture"}),
                    false,
                )?;
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "unreadable",
                    json!({"stateDir":"fixture"}),
                    false,
                )?;
                // A stored payload the current contract rejects, as a legacy row can be.
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "unreadable",
                    "not-an-object",
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("one stored row the current contract cannot read");
        let resource =
            ResourceRef::StateResource(studio_runtime_schema::resource_ref::EmptyResource {});
        let (entry, reader, _) = prepare(&state, &event(&state), resource, None)
            .await
            .expect("snapshot must not fail for one unreadable row");
        assert!(reader.is_none(), "a small snapshot is delivered inline");
        let entry = entry.expect("inline snapshot");
        let batch = entry
            .entity_changes
            .as_ref()
            .and_then(Option::as_ref)
            .expect("changes");
        let ids: Vec<&str> = batch
            .documents
            .iter()
            .map(|document| document.id.as_str())
            .collect();
        assert!(ids.contains(&"readable"), "readable rows stream: {ids:?}");
        assert!(
            !ids.contains(&"unreadable"),
            "the unreadable row is skipped: {ids:?}"
        );
    }

    #[tokio::test]
    async fn snapshot_continuation_below_tombstone_floor_makes_bounded_progress() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        store
            .transaction(|tx| {
                for index in 0..513 {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        &format!("old-{index}"),
                        json!({"stateDir":"fixture"}),
                        false,
                    )?;
                }
                studio_runtime_store::test_fixtures::canvas::set_meta(
                    tx,
                    "entity_tombstone_floor",
                    Some("1000"),
                )?;
                Ok(())
            })
            .await
            .expect("old live rows and a pruned tombstone floor");
        store
            .test_read_transaction(|tx| {
                let first = SyncDocumentsRepository::resume_entity_scope(
                    tx,
                    None,
                    Some(r#"["workspace"]"#),
                    None,
                    None,
                    true,
                    SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
                )?;
                assert!(first.reset);
                assert!(first.checkpoint < first.tombstone_floor);
                // Exactly two reads: the regression must fail without starting an
                // unbounded spool or relying on a timeout to stop repeated pages.
                let next = SyncDocumentsRepository::continue_entity_scope(
                    tx,
                    &first,
                    Some(r#"["workspace"]"#),
                    None,
                    None,
                    true,
                    SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
                )?;
                assert!(
                    next.checkpoint > first.checkpoint,
                    "internal snapshot continuation must not re-evaluate expired external cursors"
                );
                Ok(())
            })
            .expect("bounded snapshot continuation");
    }

    #[tokio::test]
    async fn cold_entity_client_fixture_matches_server_frames() {
        use studio_runtime_store::{AgentRecord, AgentRepository};
        let (_directory, state) = fixture().await;
        // The store names its folder by the canonical path: on macOS the temporary
        // folder /var/folders is /private/var/folders.
        let state_dir =
            studio_runtime_store::path_identity::CanonicalPath::existing(_directory.path())
                .expect("canonical state folder")
                .to_path_buf()
                .display()
                .to_string();
        let store = state.store.as_ref().expect("store");
        studio_runtime_services::entity_publication::install(store)
            .await
            .expect("entity publisher");
        store
            .transaction_with(|tx| {
                for index in 0..=SYNC_ENTITY_STREAM_MAX_DOCUMENTS {
                    let id = format!("older-{index}");
                    AgentRepository::put(
                        tx,
                        &AgentRecord::from_value(json!({
                            "provider":"codex", "id":id, "rootId":id, "epoch":1,
                            "status":"idle", "accountKey":"default", "name":id, "compactions":0
                        }))?,
                    )?;
                }
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .expect("entity baseline");
        let resource = studio_json::from_str(r#"{"kind":"state"}"#).expect("resource");
        let (_, reader, _) = prepare(&state, &event(&state), resource, None)
            .await
            .expect("snapshot");
        let mut reader = reader.expect("large snapshot");
        let mut frames = Vec::new();
        let mut transfer_id = None;
        let mut workspace_rows = 0;
        while let Some(frame) = reader.next().await.expect("frame") {
            let bytes = wire(frame).await;
            let text = std::str::from_utf8(&bytes).expect("utf8");
            let mut value: Value = studio_json::from_str(
                text.lines()
                    .find_map(|line| line.strip_prefix("data: "))
                    .expect("data"),
            )
            .expect("frame json");
            assert_eq!(value["workspaceId"], state.workspace_id);
            assert_eq!(value["cursor"]["workspaceId"], state.workspace_id);
            let id = value["transferId"].as_str().expect("transfer ID");
            uuid::Uuid::parse_str(id).expect("valid transfer ID");
            assert_eq!(
                transfer_id.get_or_insert_with(|| id.to_owned()).as_str(),
                id
            );
            assert_eq!(value["partCount"], 1);
            let data = value["data"].as_str().expect("frame data");
            let batch: Value = studio_json::from_str(data).expect("batch");
            for row in batch["documents"].as_array().expect("documents") {
                if row["collection"] == "workspace" && row["id"] == "current" {
                    assert_eq!(row["value"]["stateDir"], state_dir);
                    workspace_rows += 1;
                }
            }
            let data = data.replace(&state_dir, "/fixture-state");
            value["data"] = json!(data);
            value["workspaceId"] = json!("00000000000000000000000000000000");
            value["cursor"]["workspaceId"] = json!("00000000000000000000000000000000");
            value["transferId"] = json!("00000000-0000-0000-0000-000000000000");
            frames.push(value);
        }
        assert_eq!(workspace_rows, 1);
        let actual = format!(
            "{}\n",
            studio_json::to_string_pretty(&frames).expect("json")
        );
        let path = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
        )
        .join("../../../client/apps/web/src/sync/entityServerFrames.fixture.json");
        if std::env::var_os("STUDIO_REGENERATE_ENTITY_FRAMES").is_some() {
            tokio::fs::write(&path, &actual)
                .await
                .expect("write fixture");
        }
        assert!(
            actual == tokio::fs::read_to_string(&path).await.expect("fixture"),
            "regenerate with: just regenerate-entity-server-frames"
        );
    }

    #[tokio::test]
    async fn scoped_transfer_preserves_large_snapshot_and_single_oversized_record_then_sends_only_delta()
     {
        let (_directory, state) = fixture().await;
        state
            .store
            .as_ref()
            .expect("store")
            .transaction(|tx| {
                for index in 0..514 {
                    SyncDocumentsRepository::put_entity(
                        tx,
                        "workspace",
                        &format!("row-{index}"),
                        json!({"stateDir":format!("fixture-{index}")}),
                        false,
                    )?;
                }
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "oversized",
                    json!({"stateDir":"界\\\"\n".repeat(SYNC_STREAM_EVENT_MAX_BYTES / 4)}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("source fixture");
        let resource: ResourceRef =
            studio_json::from_value(json!({"kind":"state"})).expect("resource");
        let (small, reader, cursor) = prepare(&state, &event(&state), resource.clone(), None)
            .await
            .expect("spooled interval");
        assert!(small.is_none());
        let mut reader = reader.expect("large snapshot is streamed");
        let mut encoded = String::new();
        let mut records = 0;
        let mut pages = 0;
        let mut maximum_wire_bytes = 0;
        let mut complete = false;
        while let Some(frame) = reader.next().await.expect("next frame") {
            let wire = wire(frame).await;
            maximum_wire_bytes = maximum_wire_bytes.max(wire.len());
            assert!(wire.len() <= SYNC_STREAM_EVENT_MAX_BYTES);
            let text = std::str::from_utf8(&wire).expect("utf8");
            let frame: Value = studio_json::from_str(
                text.lines()
                    .find_map(|line| line.strip_prefix("data: "))
                    .expect("data"),
            )
            .expect("strict frame");
            assert!(
                state
                    .validators
                    .validate_component("ResourceEntityTransferEvent", &frame)
            );
            encoded.push_str(frame["data"].as_str().expect("part"));
            if frame["partIndex"].as_u64().expect("index") + 1
                == frame["partCount"].as_u64().expect("parts")
            {
                let page: Value = studio_json::from_str(&encoded).expect("canonical page");
                assert!(
                    state
                        .validators
                        .validate_component("EntityChangeBatch", &page)
                );
                assert!(
                    page["documents"].as_array().expect("records").len()
                        <= SYNC_ENTITY_STREAM_MAX_DOCUMENTS as usize
                );
                records += page["documents"].as_array().expect("records").len();
                if let Some(oversized) = page["documents"]
                    .as_array()
                    .expect("records")
                    .iter()
                    .find(|row| row["id"] == "oversized")
                {
                    assert_eq!(
                        oversized["value"]["stateDir"],
                        "界\\\"\n".repeat(SYNC_STREAM_EVENT_MAX_BYTES / 4)
                    );
                }
                pages += 1;
                encoded.clear();
            }
            complete = frame["complete"] == true;
        }
        assert!(complete);
        assert_eq!(records, 516);
        assert!(pages >= 3);
        state
            .store
            .as_ref()
            .expect("store")
            .transaction(|tx| {
                SyncDocumentsRepository::put_entity(
                    tx,
                    "workspace",
                    "row-513",
                    json!({"stateDir":"changed"}),
                    false,
                )?;
                Ok(())
            })
            .await
            .expect("small source update");
        let (entry, reader, after_cursor) =
            prepare(&state, &event(&state), resource, Some(cursor.clone()))
                .await
                .expect("delta");
        assert!(
            reader.is_none(),
            "one changed record stays on ordinary fast path"
        );
        let batch = entry
            .expect("direct delta")
            .entity_changes
            .expect("batch")
            .expect("present");
        assert_eq!(batch.after, cursor.seq);
        assert_eq!(batch.documents.len(), 1, "no history retransmission");
        assert_eq!(batch.documents[0].id, "row-513");
        assert_eq!(batch.through, after_cursor.seq);
        eprintln!(
            "entity_transfer_fixture snapshot_records={records} pages={pages} maximum_wire_bytes={maximum_wire_bytes} delta_records={}",
            batch.documents.len()
        );
    }
    fn work_record(id: &str, root: &str, owner: &str, status: &str, results: usize) -> Value {
        json!({"id":id,"rootId":root,"owner":owner,"title":id,"status":status,"created":1,"updated":1,
            "version":1,"dependencies":[],"results":vec![json!({"id":"r","agent":"worker","text":"done","checks":"","revision":"r1","files":[],"created":1}); results],"decisions":[],"releases":[]})
    }
    /// Write or delete a work row the way the writer does: the child history rows follow.
    async fn put_work(store: &studio_runtime_store::RuntimeStore, id: &str, record: Option<Value>) {
        let id = id.to_owned();
        store
            .transaction(move |tx| {
                match record {
                    Some(record) => {
                        tx.execute(
                            "INSERT INTO runtime_work(id,record) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
                            rusqlite::params![id, record.to_string()],
                        )?;
                        studio_runtime_store::work_history::reconcile(tx, &id, &record)?;
                    }
                    None => {
                        tx.execute("DELETE FROM runtime_work WHERE id=?1", [&id])?;
                    }
                }
                Ok(())
            })
            .await
            .expect("work write");
    }
    fn docs_of(entry: Option<ResourceRevisionEntry>) -> Vec<crate::generated::SyncEntity> {
        entry
            .expect("inline delta")
            .entity_changes
            .flatten()
            .expect("changes")
            .documents
    }
    fn find<'a>(
        documents: &'a [crate::generated::SyncEntity],
        collection: &str,
        id: &str,
    ) -> Option<&'a crate::generated::SyncEntity> {
        documents
            .iter()
            .find(|document| document.collection == collection && document.id == id)
    }

    #[tokio::test]
    async fn work_documents_stream_from_the_main_tables_with_fan_out_and_reset() {
        let (_directory, state) = fixture().await;
        let store = state.store.as_ref().expect("store");
        let resource: ResourceRef =
            studio_json::from_str(r#"{"kind":"work","agentId":"lead"}"#).expect("work resource");
        let (_, _, first) = prepare(&state, &event(&state), resource.clone(), None)
            .await
            .expect("baseline");

        // A new work row streams its detail and two history entries.
        put_work(
            store,
            "w1",
            Some(work_record("w1", "lead", "worker", "review", 2)),
        )
        .await;
        let (entry, _, cursor) = prepare(
            &state,
            &event(&state),
            resource.clone(),
            Some(first.clone()),
        )
        .await
        .expect("work change");
        let documents = docs_of(entry);
        let detail = find(&documents, "workDetail", "w1").expect("work detail");
        assert_eq!(detail.op, crate::generated::SyncEntityOp::Upsert);
        assert!(find(&documents, "workResult", "w1:1").is_some());

        // A shorter history deletes the entry that no longer exists.
        put_work(
            store,
            "w1",
            Some(work_record("w1", "lead", "worker", "review", 1)),
        )
        .await;
        let (entry, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("shorter history");
        let documents = docs_of(entry);
        assert_eq!(
            find(&documents, "workResult", "w1:1")
                .expect("removed entry")
                .op,
            crate::generated::SyncEntityOp::Delete
        );

        // A deleted work row streams a delete of its detail.
        put_work(store, "w1", None).await;
        let (entry, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("work delete");
        assert_eq!(
            find(&docs_of(entry), "workDetail", "w1")
                .expect("deleted detail")
                .op,
            crate::generated::SyncEntityOp::Delete
        );

        // An execution run of the owner refreshes the detail of its work.
        put_work(
            store,
            "w2",
            Some(work_record("w2", "lead", "worker2", "review", 0)),
        )
        .await;
        let (_, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("w2 change");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_execution_runs(id,agent,account,epoch,thread,turn,created,record) VALUES('run-1','worker2','default',1,NULL,NULL,1.0,'{}')",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("execution run");
        let (entry, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("owner execution fan-out");
        assert!(find(&docs_of(entry), "workDetail", "w2").is_some());

        // A status change of a dependency refreshes the detail of its dependent.
        let mut dependent = work_record("w4", "lead", "worker", "review", 0);
        dependent["dependencies"] = json!(["w3"]);
        put_work(
            store,
            "w3",
            Some(work_record("w3", "lead", "worker", "review", 0)),
        )
        .await;
        put_work(store, "w4", Some(dependent)).await;
        let (_, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("dependency rows");
        put_work(
            store,
            "w3",
            Some(work_record("w3", "lead", "worker", "accepted", 0)),
        )
        .await;
        let (entry, _, cursor) = prepare(&state, &event(&state), resource.clone(), Some(cursor))
            .await
            .expect("dependency change");
        assert!(find(&docs_of(entry), "workDetail", "w4").is_some());

        // A cursor below the durable floor gets a snapshot, not a delta.
        store
            .transaction(|tx| {
                studio_runtime_store::test_fixtures::canvas::seed_durable_floor(
                    tx,
                    studio_runtime_store::test_fixtures::canvas::changes(tx)?
                        .iter()
                        .map(|row| row.seq)
                        .max()
                        .expect("durable rows exist"),
                )?;
                Ok(())
            })
            .await
            .expect("raise durable floor");
        let (entry, _, _) = prepare(&state, &event(&state), resource, Some(first))
            .await
            .expect("reset below floor");
        assert_eq!(
            entry.expect("inline snapshot").reply_kind,
            Some(crate::generated::SubscriptionReplyKind::Snapshot)
        );
        let _ = cursor;
    }
}

inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "entity_transfer",
        routes: None,
        dependencies: &[],
    }
}
