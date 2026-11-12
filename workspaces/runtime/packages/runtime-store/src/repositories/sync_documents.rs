//! Synchronous access to the durable sync document and entity tables.

use crate::peer_update_settings::{canvas_read, canvas_tx};
use rusqlite::{Connection, Transaction};
use studio_db_api::db_query;
use studio_json::{Value, json};
use studio_runtime_schema::shared_values::{
    SYNC_BATCH_LIMIT, SYNC_ENTITY_PAGE_LIMIT, SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
};

use crate::schema::{SchemaRequirements, TableRequirement, TriggerRequirement};
use crate::{StoreError, StoreResult};

/// Non-transcript entity tombstones retained for incremental sync clients.
/// Older tombstones are pruned and the tombstone floor is raised. Unit: rows.
pub const ENTITY_TOMBSTONE_LIMIT: i64 = 10_000;
/// Tombstones removed by one short writer transaction. Unit: rows.
pub const ENTITY_TOMBSTONE_PRUNE_BATCH: i64 = 500;

/// Stored count of retained non-transcript entity tombstones, measured in rows.
pub(crate) const ENTITY_TOMBSTONE_COUNT_KEY: &str = "entity_tombstone_count";

/// Refresh the three existing publication-count producers in the authority transaction.
pub(crate) fn tombstone_counter_sql() -> String {
    use crate::consolidation_kinds::{
        CanvasMetadataKind as MetaKind, CanvasPublicationsKind as PublicationKind,
    };
    use crate::{metadata_schema::Metadata, publication_schema::Publications};
    use studio_db_api::{Table, migration::identifier, query_column as column};
    let publications = identifier(Publications::NAME);
    let metadata = identifier(Metadata::NAME);
    let kind = column::<crate::peer_update_settings::Canvas, _>(Publications::KIND);
    let deleted = column::<crate::peer_update_settings::Canvas, _>(Publications::DELETED);
    let non_transcript =
        column::<crate::peer_update_settings::Canvas, _>(Publications::NON_TRANSCRIPT);
    let meta_kind = column::<crate::peer_update_settings::Canvas, _>(Metadata::KIND);
    let key = column::<crate::peer_update_settings::Canvas, _>(Metadata::TEXT_KEY);
    let value = column::<crate::peer_update_settings::Canvas, _>(Metadata::META_VALUE);
    let entities = PublicationKind::DB_CODE_SYNCENTITIES;
    let meta = MetaKind::DB_CODE_SYNCENTITYMETA;
    let old = format!("(OLD.{kind}={entities} AND OLD.{deleted}=1 AND OLD.{non_transcript}=1)");
    let new = format!("(NEW.{kind}={entities} AND NEW.{deleted}=1 AND NEW.{non_transcript}=1)");
    let mut sql = String::new();
    for (requirement, event, when, delta) in [
        (
            BOOTSTRAP_TRIGGERS[0],
            "AFTER INSERT",
            new.clone(),
            "1".to_owned(),
        ),
        (
            BOOTSTRAP_TRIGGERS[1],
            "AFTER UPDATE",
            format!("{old}!={new}"),
            format!("CASE WHEN {new} THEN 1 ELSE -1 END"),
        ),
        (BOOTSTRAP_TRIGGERS[2], "AFTER DELETE", old, "-1".to_owned()),
    ] {
        let name = identifier(requirement.name);
        sql.push_str(&format!(
            "DROP TRIGGER IF EXISTS {name};
            CREATE TRIGGER {name} {event} ON {publications} WHEN {when} BEGIN
            UPDATE {metadata} SET {value}=CAST({value} AS INTEGER)+({delta})
            WHERE {meta_kind}={meta} AND {key}='{ENTITY_TOMBSTONE_COUNT_KEY}'; END;"
        ));
    }
    sql
}

/// Team membership revision keys; durable agent triggers own their values.
pub(crate) const TEAM_MEMBERSHIP_KEY_PREFIX: &str = "team_membership:";

/// Selection shared by the page reader.
#[derive(Clone, Copy)]
struct EntityPageScope<'a> {
    collections: Option<&'a str>,
}

const BOOTSTRAP_TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
        columns: &[
            crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
            crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
            crate::metadata_schema::Metadata::__DB_COLUMN_INTEGER_KEY,
            crate::metadata_schema::Metadata::__DB_COLUMN_TEXT_KEY,
            crate::metadata_schema::Metadata::__DB_COLUMN_SYNC_GENERATION,
            crate::metadata_schema::Metadata::__DB_COLUMN_META_VALUE,
        ],
    },
    TableRequirement {
        name: <crate::publication_schema::Publications as studio_db_api::Table>::NAME,
        columns: &[
            crate::publication_schema::Publications::__DB_COLUMN_KIND,
            crate::publication_schema::Publications::__DB_COLUMN_IDENTITY,
            crate::publication_schema::Publications::__DB_COLUMN_PUBLICATION_SEQ,
            crate::publication_schema::Publications::__DB_COLUMN_DELETED,
            crate::publication_schema::Publications::__DB_COLUMN_DOCUMENT_SCOPE,
            crate::publication_schema::Publications::__DB_COLUMN_DOCUMENT_ID,
            crate::publication_schema::Publications::__DB_COLUMN_DOCUMENT_PAYLOAD,
            crate::publication_schema::Publications::__DB_COLUMN_COLLECTION_ORDER,
            crate::publication_schema::Publications::__DB_COLUMN_ENTITY_ID,
            crate::publication_schema::Publications::__DB_COLUMN_ENTITY_PAYLOAD,
        ],
    },
    TableRequirement {
        name: <crate::change_schema::Changes as studio_db_api::Table>::NAME,
        columns: &[
            crate::change_schema::Changes::__DB_COLUMN_KIND,
            crate::change_schema::Changes::__DB_COLUMN_IDENTITY,
            crate::change_schema::Changes::__DB_COLUMN_SOURCE_LABEL,
            crate::change_schema::Changes::__DB_COLUMN_DURABLE_ID,
            crate::change_schema::Changes::__DB_COLUMN_SEQUENCE,
            crate::change_schema::Changes::__DB_COLUMN_DELETED,
            crate::change_schema::Changes::__DB_COLUMN_DELETED_AT,
        ],
    },
];
const BOOTSTRAP_TRIGGERS: &[TriggerRequirement] = &[
    TriggerRequirement {
        name: "sync_entity_tombstone_count_insert",
        table: <crate::publication_schema::Publications as studio_db_api::Table>::NAME,
    },
    TriggerRequirement {
        name: "sync_entity_tombstone_count_update",
        table: <crate::publication_schema::Publications as studio_db_api::Table>::NAME,
    },
    TriggerRequirement {
        name: "sync_entity_tombstone_count_delete",
        table: <crate::publication_schema::Publications as studio_db_api::Table>::NAME,
    },
];
pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[
        "sync_entity_hash",
        "sync_entity_payload",
        crate::entity_scopes::SCOPE_TOMBSTONE_FUNCTION,
    ],
    bootstrap_tables: BOOTSTRAP_TABLES,
    bootstrap_triggers: BOOTSTRAP_TRIGGERS,
    bootstrap_connection_functions: &[
        "sync_entity_hash",
        "sync_entity_payload",
        crate::entity_scopes::SCOPE_TOMBSTONE_FUNCTION,
    ],
};

/// Collections of the work documents. Their rows come from the main tables
/// (`runtime_work` and the history tables), never from `sync_entities`; the readers
/// here skip them, and `entity_scope` in runtime-services builds them.
pub const WORK_COLLECTIONS: &[&str] = &[
    "work",
    "workDetail",
    "workResult",
    "workDecision",
    "workRelease",
];

/// Collections with one document per row of a main table (`runtime_plans`,
/// `runtime_annotations`, `runtime_rules`, `approvals`, `runtime_complaints`,
/// `graph_edges`) or per chat (`chat`, marked by its writers). Like the work documents,
/// they are built at read time and have no copy in `sync_entities`.
pub const DIRECT_COLLECTIONS: &[&str] = &[
    "taskDetail",
    "plan",
    "annotation",
    "rule",
    "request",
    "questionHistory",
    "complaint",
    "edge",
    "chat",
];

/// The views of one agent, built at read time from `runtime_agents` and the rows its
/// view reads. They have no copy in `sync_entities`.
pub const AGENT_VIEW_COLLECTIONS: &[&str] = &[
    "agent",
    "agentSummary",
    "agentDetail",
    "retainedWorktree",
    "agentTail",
];

/// Window collections: a document per member of a window in `durable_rows`
/// (`durable_seq::TASK_WINDOW`, `durable_seq::MONITOR_WINDOW`), built at read time.
pub const WINDOW_COLLECTIONS: &[&str] = &["task", "monitor"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncDocument {
    pub id: String,
    pub payload: String,
    pub sequence: i64,
    pub deleted: bool,
}

impl SyncDocument {
    pub fn response(&self) -> Value {
        json!({
            "id": self.id,
            "payload": self.payload,
            "seq": self.sequence,
            "_deleted": self.deleted,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRow {
    pub collection: String,
    pub id: String,
    pub sequence: i64,
    pub payload: Option<String>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityPage {
    pub rows: Vec<EntityRow>,
    pub max_sequence: i64,
    pub tombstone_floor: i64,
    pub initial_high: i64,
    pub checkpoint: i64,
    pub reset: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SyncDocumentsRepository;

impl SyncDocumentsRepository {
    /// Merge fields into an existing sync entity value without rebuilding its envelope.
    pub fn patch_entity(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        patch: Value,
    ) -> StoreResult<()> {
        let raw = publication_payload(transaction, collection, id, true)?;
        let mut envelope: Value = match raw {
            Some(raw) => {
                studio_json::from_str(&raw).map_err(|_| StoreError::InvalidSyncObject {
                    scope: collection.to_owned(),
                    id: id.to_owned(),
                })?
            }
            None => json!({"collection":collection,"id":id,"value":{}}),
        };
        if envelope.get("collection").and_then(Value::as_str) != Some(collection)
            || envelope.get("id").and_then(Value::as_str) != Some(id)
        {
            return Err(StoreError::InvalidSyncObject {
                scope: collection.to_owned(),
                id: id.to_owned(),
            });
        }
        let value = envelope
            .get_mut("value")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| StoreError::InvalidSyncObject {
                scope: collection.to_owned(),
                id: id.to_owned(),
            })?;
        let patch = patch
            .as_object()
            .ok_or_else(|| StoreError::InvalidSyncObject {
                scope: collection.to_owned(),
                id: id.to_owned(),
            })?;
        value.extend(patch.clone());
        Self::put_entity(
            transaction,
            collection,
            id,
            envelope["value"].clone(),
            false,
        )?;
        Ok(())
    }

    pub fn pull_entities(
        transaction: &Transaction<'_>,
        after: i64,
        limit: i64,
        fresh: bool,
        initial_high: i64,
        reset_support: bool,
    ) -> StoreResult<EntityPage> {
        Self::pull_entity_view(
            transaction,
            after,
            limit,
            fresh,
            initial_high,
            reset_support,
            EntityPageScope { collections: None },
        )
    }

    fn pull_entity_view(
        transaction: &Transaction<'_>,
        after: i64,
        limit: i64,
        fresh: bool,
        initial_high: i64,
        reset_support: bool,
        scope: EntityPageScope<'_>,
    ) -> StoreResult<EntityPage> {
        let tombstone_floor = Self::entity_tombstone_floor(transaction)?;
        let stored_max = canvas_read(transaction).named(
            &publication_queries::EntityViewHigh::QUERY,
            &publication_queries::EntityViewHigh {
                kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
            },
        )?;
        let max_sequence = stored_max.max(tombstone_floor);
        let initial_high = if fresh {
            if initial_high > 0 {
                max_sequence.min(initial_high)
            } else {
                max_sequence
            }
        } else {
            0
        };
        let after = after.max(0);
        if reset_support && !fresh && after > 0 && (after < tombstone_floor || after > max_sequence)
        {
            return Ok(EntityPage {
                rows: Vec::new(),
                max_sequence,
                tombstone_floor,
                initial_high,
                checkpoint: max_sequence,
                reset: true,
            });
        }

        let limit = limit.clamp(1, SYNC_ENTITY_PAGE_LIMIT);
        let rows = read_entity_page(
            transaction,
            after,
            tombstone_floor,
            fresh,
            initial_high,
            limit,
            scope,
        )?;
        let checkpoint = if rows.len() as i64 == limit {
            rows.last().map_or(after, |row| row.sequence)
        } else {
            max_sequence
        };
        Ok(EntityPage {
            rows,
            max_sequence,
            tombstone_floor,
            initial_high,
            checkpoint,
            reset: false,
        })
    }

    /// Read one ref's durable interval and watermark in the caller's SQLite snapshot.
    /// `reset` selects a replacement snapshot when the cursor is missing, expired,
    /// or ahead of this store. Snapshots omit tombstones; deltas retain removals.
    /// A checkpoint below `max_sequence` means the bounded page is incomplete.
    pub fn resume_entities(
        transaction: &Transaction<'_>,
        since: Option<i64>,
        collection: Option<&str>,
        id: Option<&str>,
        limit: i64,
    ) -> StoreResult<EntityPage> {
        Self::resume_entity_scope(
            transaction,
            since,
            collection.map(|c| json!([c]).to_string()).as_deref(),
            id,
            None,
            true,
            limit,
        )
    }

    /// Read a view's selected collections and optional row/agent identity. Scope
    /// filters run in SQL before the page bound, including for tombstones.
    pub fn resume_entity_scope(
        transaction: &Transaction<'_>,
        since: Option<i64>,
        collections_json: Option<&str>,
        id: Option<&str>,
        agent: Option<&str>,
        include_team_members: bool,
        limit: i64,
    ) -> StoreResult<EntityPage> {
        let tombstone_floor = Self::entity_tombstone_floor(transaction)?;
        let max_sequence = Self::entity_sequence(transaction)?.max(tombstone_floor);
        let reset = since.is_none_or(|seq| seq < tombstone_floor || seq > max_sequence || seq < 0);
        let after = if reset { 0 } else { since.unwrap_or(0) };
        let interval = EntityPage {
            rows: Vec::new(),
            max_sequence,
            tombstone_floor,
            initial_high: if reset { max_sequence } else { 0 },
            checkpoint: after,
            reset,
        };
        Self::continue_entity_scope(
            transaction,
            &interval,
            collections_json,
            id,
            agent,
            include_team_members,
            limit,
        )
    }

    /// Continue only inside the caller's original SQLite read transaction with
    /// the same scope. Initial external-cursor recovery is already decided by
    /// resume_entity_scope; a snapshot checkpoint may legitimately predate the
    /// tombstone floor and must never restart the first page.
    pub fn continue_entity_scope(
        transaction: &Transaction<'_>,
        interval: &EntityPage,
        collections_json: Option<&str>,
        id: Option<&str>,
        agent: Option<&str>,
        include_team_members: bool,
        limit: i64,
    ) -> StoreResult<EntityPage> {
        let after = interval.checkpoint;
        let max_sequence = interval.max_sequence;
        let reset = interval.reset;
        let tombstone_floor = interval.tombstone_floor;
        let limit = limit.clamp(1, SYNC_ENTITY_STREAM_MAX_DOCUMENTS);
        let mut rows = Self::scope_rows(
            transaction,
            after,
            max_sequence,
            reset,
            collections_json,
            id,
            agent,
            include_team_members,
            limit + 1,
        )?;
        let checkpoint = if rows.len() as i64 > limit {
            rows.truncate(limit as usize);
            rows.last().map_or(after, |row| row.sequence)
        } else {
            max_sequence
        };
        Ok(EntityPage {
            rows,
            max_sequence,
            tombstone_floor,
            initial_high: if reset { max_sequence } else { 0 },
            checkpoint,
            reset,
        })
    }

    /// Raw scoped rows of `sync_entities` in `(after, through]`, at most `fetch` rows, in
    /// `seq` order, without the collections built at read time. Shared by every scoped
    /// reader.
    #[allow(clippy::too_many_arguments)]
    pub fn scope_rows(
        transaction: &Connection,
        after: i64,
        through: i64,
        snapshot: bool,
        collections_json: Option<&str>,
        id: Option<&str>,
        agent: Option<&str>,
        include_team_members: bool,
        fetch: i64,
    ) -> StoreResult<Vec<EntityRow>> {
        let excluded = read_time_collections_json()?;
        let query = publication_queries::ScopedPage {
            kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
            after,
            through,
            snapshot: i64::from(snapshot),
            collections: collections_json,
            id,
            agent: agent.map(crate::AgentId::from),
            include_team_members: Some(include_team_members),
            fetch,
            excluded: Some(&excluded),
        };
        let rows =
            canvas_read(transaction).collect(&publication_queries::ScopedPage::QUERY, &query)?;
        rows.into_iter().map(publication_entity).collect()
    }

    /// Insertion, deletion or restoration of a team member changes scoped visibility.
    pub fn team_membership_revision(
        transaction: &Transaction<'_>,
        agent: &str,
    ) -> StoreResult<i64> {
        let key = format!(
            "{TEAM_MEMBERSHIP_KEY_PREFIX}{}",
            studio_json::to_string(agent)?
        );
        Ok(canvas_read(transaction)
            .named(
                &publication_queries::TeamRevision::QUERY,
                &publication_queries::TeamRevision {
                    kind: crate::consolidation_kinds::CanvasMetadataKind::SyncEntityMeta,
                    key: Some(&key),
                },
            )?
            .unwrap_or(0))
    }

    pub fn draft(transaction: &Transaction<'_>, id: &str) -> StoreResult<Option<SyncDocument>> {
        use crate::publication_schema::SyncDocuments as Document;
        let scope = "drafts".to_owned();
        let id = id.to_owned();
        Ok(db_query!(
            canvas_read(transaction),
            Document,
            filter(
                Document::DOCUMENT_SCOPE.eq(&scope),
                Document::DOCUMENT_ID.eq(&id)
            ),
            limit(1)
        )?
        .pop()
        .map(publication_document))
    }

    pub fn put_draft(
        transaction: &Transaction<'_>,
        id: &str,
        payload: &str,
        deleted: bool,
    ) -> StoreResult<bool> {
        if Self::draft(transaction, id)?
            .is_some_and(|current| current.payload == payload && current.deleted == deleted)
        {
            return Ok(false);
        }
        use crate::metadata_values::StoredBoolean;
        use crate::publication_schema::{DocumentValue, SyncDocuments as Document};
        use studio_db_api::migration::encode_key;
        let scope = "drafts".to_owned();
        let owned_id = id.to_owned();
        let old = db_query!(
            canvas_read(transaction),
            Document,
            scalar(Document::IDENTITY),
            filter(
                Document::DOCUMENT_SCOPE.eq(&scope),
                Document::DOCUMENT_ID.eq(&owned_id)
            )
        )?;
        let sequence = next_sequence(transaction)?;
        let row = Document {
            identity: encode_key(&(sequence,)),
            source_rowid: sequence,
            publication_seq: sequence,
            deleted: StoredBoolean::Valid(deleted),
            document_scope: scope,
            document_id: owned_id,
            document_payload: DocumentValue::from_stored("drafts", id, payload.to_owned())?,
        };
        let mut tx = canvas_tx(transaction);
        if let Some(identity) = old {
            tx.partition::<Document>().delete(&(identity,))?;
        }
        tx.partition::<Document>().put(&row)?;
        // Publications also holds documents; uncounted sequence bookkeeping keeps their writes broad.
        preserve_sequence(transaction, sequence)?;
        Ok(true)
    }

    /// Publish the renderer-owned sync envelope for an entity inside its source write transaction.
    pub fn put_entity<T: serde::Serialize>(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        value: T,
        deleted: bool,
    ) -> StoreResult<bool> {
        #[derive(serde::Serialize)]
        struct Entity<'a, T> {
            collection: &'a str,
            id: &'a str,
            value: T,
        }
        #[derive(serde::Serialize)]
        struct Empty {}
        let payload = if deleted {
            studio_json::to_string(&Entity {
                collection,
                id,
                value: Empty {},
            })?
        } else {
            studio_json::to_string(&Entity {
                collection,
                id,
                value,
            })?
        };
        Self::put_entity_payload(transaction, collection, id, &payload, deleted)
    }

    /// The highest change number of the shared counter. Entities built at read time
    /// (work documents) carry numbers of `durable_rows`, so this is not the highest
    /// `sync_entities` row.
    pub fn entity_sequence(connection: &Connection) -> StoreResult<i64> {
        crate::durable_seq::high(connection)
    }

    pub fn entities_after(
        connection: &Connection,
        after: i64,
        limit: i64,
    ) -> StoreResult<Vec<EntityRow>> {
        read_entity_page(
            connection,
            after,
            0,
            false,
            0,
            limit.clamp(
                1,
                SYNC_ENTITY_STREAM_MAX_DOCUMENTS
                    .max(SYNC_ENTITY_PAGE_LIMIT)
                    .saturating_add(1),
            ),
            EntityPageScope { collections: None },
        )
    }

    /// Whether entity tombstones exceed the retained limit by the prune slack,
    /// so a pruning run removes a full batch instead of one row per delete.
    pub fn entity_tombstone_prune_due(connection: &Connection) -> StoreResult<bool> {
        let count = publication_meta(connection, ENTITY_TOMBSTONE_COUNT_KEY)?;
        let Some(count) = count else {
            // A missing counter is rebuilt by the first pruning transaction.
            return Ok(true);
        };
        let count = count
            .parse::<i64>()
            .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
        Ok(count
            > ENTITY_TOMBSTONE_LIMIT
                + ENTITY_TOMBSTONE_PRUNE_BATCH.min(ENTITY_TOMBSTONE_LIMIT / 20))
    }

    /// Remove one batch of the oldest non-transcript entity tombstones above
    /// [`ENTITY_TOMBSTONE_LIMIT`] and raise the tombstone floor to the highest
    /// removed sequence, so clients behind the floor reset instead of missing a
    /// deletion. Returns the number of removed rows; zero means nothing is due.
    pub fn prune_entity_tombstones(transaction: &Transaction<'_>) -> StoreResult<usize> {
        use crate::metadata_values::StoredBoolean;
        use crate::publication_schema::SyncEntities as Entity;
        let count = match publication_meta(transaction, ENTITY_TOMBSTONE_COUNT_KEY)? {
            Some(raw) => raw
                .parse::<i64>()
                .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?,
            None => canvas_read(transaction).named(
                &publication_queries::TombstoneCount::QUERY,
                &publication_queries::TombstoneCount {
                    kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
                },
            )?,
        };
        if count <= ENTITY_TOMBSTONE_LIMIT {
            return Ok(0);
        }
        let excess = count - ENTITY_TOMBSTONE_LIMIT;
        let rows = db_query!(
            canvas_read(transaction),
            Entity,
            project(EntityPruneRow),
            filter(
                Entity::DELETED.eq(&StoredBoolean::Valid(true)),
                Entity::NON_TRANSCRIPT.eq(&true)
            ),
            order_by(
                Entity::PUBLICATION_SEQ.asc,
                Entity::COLLECTION_ORDER.asc,
                Entity::ENTITY_ID.asc
            ),
            limit(ENTITY_TOMBSTONE_PRUNE_BATCH.min(excess))
        )?;
        let Some(highest) = rows.iter().map(|row| row.publication_seq).max() else {
            return Ok(0);
        };
        let floor = Self::entity_tombstone_floor(transaction)?.max(highest);
        crate::record_entity_metadata_updates(preserve_sequence(
            transaction,
            Self::entity_sequence(transaction)?,
        )?);
        for row in &rows {
            canvas_tx(transaction)
                .partition::<Entity>()
                .delete(&(row.identity.clone(),))?;
        }
        put_publication_meta(transaction, "entity_tombstone_floor", floor.to_string())?;
        put_publication_meta(
            transaction,
            ENTITY_TOMBSTONE_COUNT_KEY,
            (count - rows.len() as i64).to_string(),
        )?;
        Ok(rows.len())
    }

    pub fn entity_tombstone_floor(connection: &Connection) -> StoreResult<i64> {
        let value = publication_meta(connection, "entity_tombstone_floor")?;
        value.map_or(Ok(0), |text| {
            text.parse::<i64>()
                .map_err(|error| StoreError::SchemaMismatch(error.to_string()))
        })
    }

    pub fn workspace_id(connection: &Connection) -> StoreResult<String> {
        use crate::metadata_schema::SyncIdentity as Identity;
        let row = db_query!(
            canvas_read(connection),
            Identity,
            order_by(Identity::SOURCE_ROWID.asc),
            limit(1)
        )?
        .pop()
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        row.text_key.ok_or_else(|| {
            StoreError::from(rusqlite::Error::InvalidColumnType(
                0,
                "id".into(),
                rusqlite::types::Type::Null,
            ))
        })
    }

    /// Latest draft row sequence, including retained tombstones. Unrelated sync
    /// scopes share the allocator but never advance this content revision.
    pub fn draft_sequence(connection: &Connection) -> StoreResult<i64> {
        Ok(canvas_read(connection).named(
            &publication_queries::DocumentHigh::QUERY,
            &publication_queries::DocumentHigh {
                kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncDocuments,
                scope: Some("drafts"),
            },
        )?)
    }

    pub fn pull_drafts(
        connection: &Connection,
        after: i64,
        limit: i64,
    ) -> StoreResult<Vec<SyncDocument>> {
        Self::drafts_after(connection, after, limit.clamp(1, SYNC_BATCH_LIMIT))
    }

    /// Read up to `limit` live drafts in sequence order, for an initial snapshot. Deletion marks
    /// stay for change replay, but a snapshot is the whole current set and does not need them.
    pub fn live_drafts(connection: &Connection, limit: i64) -> StoreResult<Vec<SyncDocument>> {
        use crate::publication_schema::SyncDocuments as Document;
        let read = canvas_read(connection);
        let scope = "drafts".to_owned();
        let rows = db_query!(
            read,
            Document,
            filter(
                Document::DOCUMENT_SCOPE.eq(&scope),
                Document::DELETED.eq(&crate::metadata_values::StoredBoolean::Valid(false))
            ),
            order_by(Document::PUBLICATION_SEQ.asc),
            limit(limit)
        )?;
        Ok(rows.into_iter().map(publication_document).collect())
    }

    /// Read a caller-bounded draft interval, shared by pull pages and stream batches.
    pub fn drafts_after(
        connection: &Connection,
        after: i64,
        limit: i64,
    ) -> StoreResult<Vec<SyncDocument>> {
        use crate::publication_schema::SyncDocuments as Document;
        let read = canvas_read(connection);
        let scope = "drafts".to_owned();
        let after = after.max(0);
        let rows = db_query!(
            read,
            Document,
            filter(
                Document::DOCUMENT_SCOPE.eq(&scope),
                Document::PUBLICATION_SEQ.gt(&after)
            ),
            order_by(Document::PUBLICATION_SEQ.asc),
            limit(limit)
        )?;
        Ok(rows.into_iter().map(publication_document).collect())
    }

    pub fn preference_payload(connection: &Connection, scope: &str) -> StoreResult<Option<String>> {
        publication_payload(connection, "uiPreferences", scope, true)
    }

    pub fn put_preferences(
        transaction: &Transaction<'_>,
        scope: &str,
        payload: &str,
    ) -> StoreResult<bool> {
        Self::put_entity_payload(transaction, "uiPreferences", scope, payload, false)
    }

    pub(crate) fn put_entity_payload(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        payload: &str,
        deleted: bool,
    ) -> StoreResult<bool> {
        let retained = if deleted {
            let identity =
                studio_db_api::migration::encode_key(&(collection.to_owned(), id.to_owned()));
            canvas_read(transaction)
                .get::<crate::publication_schema::SyncEntities>(&(identity,))?
                .map(|row| {
                    crate::entity_scopes::tombstone_payload(
                        collection,
                        id,
                        row.entity_payload.value().raw(),
                        Some(row.deleted.as_stored() != 0),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let payload = retained.as_deref().unwrap_or(payload);
        // This hash detects content changes; it is not a request or receipt identity.
        let hash = hash_payload(payload);
        Self::write_entity_hash(
            transaction,
            collection,
            id,
            &hash,
            deleted,
            Some(payload),
            None,
        )
    }

    /// Store an entity hash without retaining its payload.
    pub fn put_entity_hash(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        hash: &str,
        deleted: bool,
    ) -> StoreResult<bool> {
        Self::write_entity_hash(transaction, collection, id, hash, deleted, None, None)
    }

    pub(crate) fn put_entity_hash_at_sequence(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        hash: &str,
        deleted: bool,
        sequence: i64,
    ) -> StoreResult<bool> {
        Self::write_entity_hash(
            transaction,
            collection,
            id,
            hash,
            deleted,
            None,
            Some(sequence),
        )
    }

    fn write_entity_hash(
        transaction: &Transaction<'_>,
        collection: &str,
        id: &str,
        hash: &str,
        deleted: bool,
        payload: Option<&str>,
        sequence: Option<i64>,
    ) -> StoreResult<bool> {
        crate::entity_publication::mark_published(collection, id);
        use crate::metadata_values::StoredBoolean;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities as Entity};
        use studio_db_api::migration::encode_key;
        let identity = encode_key(&(collection.to_owned(), id.to_owned()));
        let current = db_query!(
            canvas_read(transaction),
            Entity,
            project(EntityHashState),
            filter(Entity::IDENTITY.eq_ref(&identity))
        )?
        .pop();
        if current
            .as_ref()
            .is_some_and(|old| old.entity_hash == hash && (old.deleted.as_stored() != 0) == deleted)
        {
            return Ok(false);
        }
        let previous = if current.is_some() {
            canvas_read(transaction).get::<Entity>(&(identity.clone(),))?
        } else {
            None
        };
        let retained = if deleted && payload.is_none() {
            previous
                .as_ref()
                .and_then(|old| {
                    old.entity_payload
                        .value()
                        .raw()
                        .map(|raw| (raw, old.deleted.as_stored() != 0))
                })
                .map(|(raw, was_deleted)| {
                    crate::entity_scopes::tombstone_payload(
                        collection,
                        id,
                        Some(raw),
                        Some(was_deleted),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let sequence = match sequence {
            Some(value) => value,
            None => next_sequence(transaction)?,
        };
        let source_rowid = match previous {
            Some(old) => old.source_rowid,
            None => canvas_read(transaction)
                .named(
                    &publication_queries::EntityRowidHigh::QUERY,
                    &publication_queries::EntityRowidHigh {
                        kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
                    },
                )?
                .checked_add(1)
                .ok_or(StoreError::RecordContract("publication rowid exhausted"))?,
        };
        let label = CollectionName::from_stored(collection.to_owned());
        let deleted = StoredBoolean::Valid(deleted);
        let row = Entity {
            identity,
            source_rowid,
            publication_seq: sequence,
            deleted,
            collection_kind: label.kind(),
            collection_agent: label.agent().cloned(),
            non_transcript: label.non_transcript(),
            collection_order: label,
            entity_id: id.to_owned(),
            entity_hash: hash.to_owned(),
            entity_payload: EntityValue::from_stored(
                collection,
                id,
                deleted,
                retained.or_else(|| payload.map(str::to_owned)),
            )?,
        };
        canvas_tx(transaction).partition::<Entity>().put(&row)?;
        crate::record_entity_metadata_updates(preserve_sequence(transaction, sequence)?);
        Ok(true)
    }
}

impl SyncDocumentsRepository {
    /// Rows of a fresh global pull from `sync_entities`, without the read-time collections:
    /// live rows above `after`, and deletes above both `tombstone_floor` and
    /// `initial_high`.
    pub fn global_rows(
        connection: &Connection,
        after: i64,
        tombstone_floor: i64,
        initial_high: i64,
        collections_json: &str,
        limit: i64,
    ) -> StoreResult<Vec<EntityRow>> {
        read_entity_page(
            connection,
            after,
            tombstone_floor,
            true,
            initial_high,
            limit,
            EntityPageScope {
                collections: Some(collections_json),
            },
        )
    }
}

/// [`WORK_COLLECTIONS`], [`DIRECT_COLLECTIONS`] and [`AGENT_VIEW_COLLECTIONS`] as an SQL list. The readers here
/// skip them; `entity_scope` in runtime-services builds them.
fn read_time_collections_json() -> StoreResult<String> {
    Ok(studio_json::to_string(
        &WORK_COLLECTIONS
            .iter()
            .chain(DIRECT_COLLECTIONS)
            .chain(AGENT_VIEW_COLLECTIONS)
            .chain(WINDOW_COLLECTIONS)
            .copied()
            .collect::<Vec<_>>(),
    )?)
}

fn read_entity_page(
    transaction: &Connection,
    after: i64,
    tombstone_floor: i64,
    fresh: bool,
    initial_high: i64,
    limit: i64,
    scope: EntityPageScope<'_>,
) -> StoreResult<Vec<EntityRow>> {
    let excluded = read_time_collections_json()?;
    let rows = canvas_read(transaction).collect(
        &publication_queries::GlobalPage::QUERY,
        &publication_queries::GlobalPage {
            kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
            after,
            tombstone_floor,
            fresh: i64::from(fresh),
            initial_high,
            limit,
            collections: scope.collections,
            excluded: Some(&excluded),
        },
    )?;
    rows.into_iter().map(publication_entity).collect()
}

pub(crate) fn hash_payload(payload: &str) -> String {
    crate::sha256_hex(payload.as_bytes())
}

pub(crate) fn next_sequence(connection: &Connection) -> StoreResult<i64> {
    crate::durable_seq::high(connection)?
        .checked_add(1)
        .ok_or(StoreError::RecordContract("publication sequence exhausted"))
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "sync_documents",
        requirements: &REQUIREMENTS,
    }
}

fn publication_document(row: crate::publication_schema::SyncDocuments) -> SyncDocument {
    SyncDocument {
        id: row.document_id,
        payload: row.document_payload.value().raw().to_owned(),
        sequence: row.publication_seq,
        deleted: row.deleted.as_stored() != 0,
    }
}

/// Select raw stored bytes only; the caller owns their interpretation.
// A stored row may contain SQL NULL independently of whether that row exists.
pub(crate) type NullableStoredPublicationPayload = Option<String>;

pub(crate) fn publication_payload_nullable(
    connection: &Connection,
    collection: &str,
    id: &str,
    live_only: bool,
) -> StoreResult<Option<NullableStoredPublicationPayload>> {
    Ok(canvas_read(connection).named(
        &publication_queries::RawPayload::QUERY,
        &publication_queries::RawPayload {
            kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
            identity: studio_db_api::migration::encode_key(&(collection.to_owned(), id.to_owned())),
            live_only: i64::from(live_only),
        },
    )?)
}

pub(crate) fn publication_payload(
    connection: &Connection,
    collection: &str,
    id: &str,
    live_only: bool,
) -> StoreResult<Option<String>> {
    publication_payload_nullable(connection, collection, id, live_only)?
        .map(|raw| {
            raw.ok_or_else(|| {
                StoreError::from(rusqlite::Error::InvalidColumnType(
                    0,
                    "payload".into(),
                    rusqlite::types::Type::Null,
                ))
            })
        })
        .transpose()
}

/// Read an exact live/tombstone sequence without decoding the optional payload.
pub(crate) fn publication_sequence(
    connection: &Connection,
    collection: &str,
    id: &str,
    deleted: bool,
) -> StoreResult<Option<i64>> {
    use crate::consolidation_kinds::CanvasPublicationsKind;
    use crate::publication_schema::Publications;
    let identity = studio_db_api::migration::encode_key(&(collection.to_owned(), id.to_owned()));
    let deleted = i64::from(deleted);
    Ok(db_query!(
        canvas_read(connection),
        Publications,
        scalar(Publications::PUBLICATION_SEQ),
        filter(
            Publications::KIND.eq(&CanvasPublicationsKind::SyncEntities),
            Publications::IDENTITY.eq(&identity),
            Publications::DELETED.eq(&deleted)
        )
    )?)
}

fn publication_meta(connection: &Connection, key: &str) -> StoreResult<Option<String>> {
    use crate::metadata_schema::SyncEntityMeta;
    let key = Some(key.to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
    Ok(canvas_read(connection)
        .get::<SyncEntityMeta>(&(identity,))?
        .map(|row| row.meta_value.as_stored().to_owned()))
}
fn put_publication_meta(
    transaction: &Transaction<'_>,
    key: &str,
    value: String,
) -> StoreResult<()> {
    use crate::metadata_schema::{StoredSyncMetaValue, SyncEntityMeta};
    let key = Some(key.to_owned());
    let identity = crate::consolidation_codec::nullable_text_identity(&key, 0);
    let previous = canvas_read(transaction).get::<SyncEntityMeta>(&(identity.clone(),))?;
    let source_rowid = match previous {
        Some(row) => row.source_rowid,
        None => canvas_read(transaction)
            .named(
                &crate::clock_schema::queries::MetaRowidHigh::QUERY,
                &crate::clock_schema::queries::MetaRowidHigh {
                    kind: crate::consolidation_kinds::CanvasMetadataKind::SyncEntityMeta,
                },
            )?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(StoreError::RecordContract(
                "publication metadata rowid exhausted",
            ))?,
    };
    canvas_tx(transaction)
        .partition::<SyncEntityMeta>()
        .put(&SyncEntityMeta {
            identity,
            source_rowid,
            meta_value: StoredSyncMetaValue::from_stored(key.clone(), value),
            text_key: key,
        })?;
    crate::record_entity_metadata_updates(transaction.changes());
    Ok(())
}
#[derive(studio_db_api::DbProjection)]
#[db(of=crate::publication_schema::SyncEntities)]
struct EntityPruneRow {
    identity: Vec<u8>,
    publication_seq: i64,
}

#[derive(studio_db_api::DbProjection)]
#[db(of=crate::publication_schema::Publications)]
struct EntityPageRecord {
    collection_order: Option<String>,
    entity_id: Option<String>,
    publication_seq: i64,
    deleted: i64,
    entity_payload: Option<String>,
    #[db(field=source_rowid)]
    _source_rowid: i64,
}
fn publication_entity(row: EntityPageRecord) -> StoreResult<EntityRow> {
    let collection = row.collection_order.ok_or(rusqlite::Error::InvalidQuery)?;
    let id = row.entity_id.ok_or(rusqlite::Error::InvalidQuery)?;
    let deleted = crate::metadata_values::StoredBoolean::from_stored(row.deleted);
    let payload = crate::publication_schema::EntityValue::from_stored(
        &collection,
        &id,
        deleted,
        row.entity_payload,
    )?;
    Ok(EntityRow {
        collection,
        id,
        sequence: row.publication_seq,
        deleted: row.deleted != 0,
        payload: payload.value().raw().map(str::to_owned),
    })
}

#[derive(studio_db_api::DbProjection)]
#[db(of=crate::publication_schema::SyncEntities)]
struct EntityHashState {
    entity_hash: String,
    deleted: crate::metadata_values::StoredBoolean,
}

fn preserve_sequence(transaction: &Transaction<'_>, sequence: i64) -> StoreResult<u64> {
    use crate::metadata_schema::SyncGeneration;
    use studio_db_api::migration::encode_key;
    let identity = encode_key(&(1_i64,));
    let old = canvas_read(transaction).get::<SyncGeneration>(&(identity.clone(),))?;
    if old
        .as_ref()
        .is_none_or(|row| row.sync_generation < sequence)
    {
        canvas_tx(transaction)
            .partition::<SyncGeneration>()
            .put(&SyncGeneration {
                identity,
                source_rowid: 1,
                integer_key: 1,
                sync_generation: sequence,
            })?;
        return Ok(transaction.changes());
    }
    Ok(0)
}

#[studio_db_api::db_queries]
pub(crate) mod publication_queries {
    use crate::peer_update_settings::Canvas;
    use crate::publication_schema::Publications;
    use studio_db_api::DbQuery;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=optional,result=Option<String>,
        decode=Publications::ENTITY_PAYLOAD,tables(rows=Publications),
        columns(payload=Publications::ENTITY_PAYLOAD,kind=Publications::KIND,identity=Publications::IDENTITY,deleted=Publications::DELETED),
        sql="SELECT {payload} FROM {rows} WHERE {kind}=?1 AND {identity}=?2 AND (?3=0 OR {deleted}=0)")]
    pub struct RawPayload {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
        #[db(column=Publications::IDENTITY)]
        pub identity: Vec<u8>,
        #[db(column=Publications::DELETED)]
        pub live_only: i64,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=Publications::SOURCE_ROWID,tables(rows=Publications),
        columns(rowid=Publications::SOURCE_ROWID,kind=Publications::KIND),
        sql="SELECT COALESCE(MAX({rowid}),0) AS {rowid} FROM {rows} WHERE {kind}=?1")]
    pub struct EntityRowidHigh {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=Publications::PUBLICATION_SEQ,tables(rows=Publications),
        columns(seq=Publications::PUBLICATION_SEQ,kind=Publications::KIND,scope=Publications::DOCUMENT_SCOPE),
        sql="SELECT COALESCE(MAX({seq}),0) AS {seq} FROM {rows} WHERE {kind}=?1 AND {scope}=?2")]
    pub struct DocumentHigh<'a> {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
        #[db(column=Publications::DOCUMENT_SCOPE)]
        pub scope: Option<&'a str>,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,
        result=super::EntityPageRecord,projection=super::EntityPageRecord,
        tables(rows=Publications),
        columns(kind=Publications::KIND,seq=Publications::PUBLICATION_SEQ,
            label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,
            deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD,
            non_transcript=Publications::NON_TRANSCRIPT,rowid=Publications::SOURCE_ROWID),
        sql="SELECT {label} AS {label},{id} AS {id},{seq} AS {seq},{deleted} AS {deleted},{payload} AS {payload},{rowid} AS {rowid}
            FROM {rows} WHERE {kind}=?1 AND {non_transcript}=1 AND {seq}>?2
            AND ({deleted}=0 OR {seq}>?3) AND (?4=0 OR {deleted}=0 OR {seq}>?5)
            AND (?7 IS NULL OR {label} IN (SELECT value FROM json_each(?7)))
            AND {label} NOT IN (SELECT value FROM json_each(?8)) ORDER BY {seq},{rowid} LIMIT ?6")]
    pub struct GlobalPage<'a> {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub after: i64,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub tombstone_floor: i64,
        #[db(column=Publications::DELETED)]
        pub fresh: i64,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub initial_high: i64,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub limit: i64,
        #[db(column=Publications::COLLECTION_ORDER)]
        pub collections: Option<&'a str>,
        #[db(column=Publications::COLLECTION_ORDER)]
        pub excluded: Option<&'a str>,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=Publications::PUBLICATION_SEQ,tables(rows=Publications),
        columns(seq=Publications::PUBLICATION_SEQ,kind=Publications::KIND,non_transcript=Publications::NON_TRANSCRIPT),
        sql="SELECT COALESCE(MAX({seq}),0) AS {seq} FROM {rows} WHERE {kind}=?1 AND {non_transcript}=1")]
    pub struct EntityViewHigh {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,
        decode=Publications::PUBLICATION_SEQ,tables(rows=Publications),
        columns(seq=Publications::PUBLICATION_SEQ,kind=Publications::KIND,deleted=Publications::DELETED,non_transcript=Publications::NON_TRANSCRIPT),
        sql="SELECT COUNT(*) AS {seq} FROM {rows} WHERE {kind}=?1 AND {deleted}=1 AND {non_transcript}=1")]
    pub struct TombstoneCount {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=Option<i64>,
        decode=crate::metadata_schema::Metadata::SYNC_GENERATION,tables(rows=crate::metadata_schema::Metadata),
        columns(value=crate::metadata_schema::Metadata::META_VALUE,key=crate::metadata_schema::Metadata::TEXT_KEY,
            kind=crate::metadata_schema::Metadata::KIND,result=crate::metadata_schema::Metadata::SYNC_GENERATION),
        sql="SELECT COALESCE((SELECT CAST({value} AS INTEGER) FROM {rows} WHERE {kind}=?1 AND {key}=?2),0) AS {result}")]
    pub struct TeamRevision<'a> {
        #[db(column=crate::metadata_schema::Metadata::KIND)]
        pub kind: crate::consolidation_kinds::CanvasMetadataKind,
        #[db(column=crate::metadata_schema::Metadata::TEXT_KEY)]
        pub key: Option<&'a str>,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,
        result=super::EntityPageRecord,projection=super::EntityPageRecord,
        tables(by_id=Publications,by_collection=Publications,by_sequence=Publications,members=crate::agent_tables::AgentsStorageRow),
        indexes(by_id=Publications::INDEX_PUBLICATIONS_ENTITY_ID,
            by_collection=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE,
            by_sequence=Publications::INDEX_PUBLICATIONS_ENTITY_NON_TRANSCRIPT_SEQUENCE),
        columns(kind=Publications::KIND,seq=Publications::PUBLICATION_SEQ,
            label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,
            deleted=Publications::DELETED,payload=Publications::ENTITY_PAYLOAD,
            non_transcript=Publications::NON_TRANSCRIPT,rowid=Publications::SOURCE_ROWID,
            member_id=crate::agent_tables::AgentsStorageRow::ID,
            member_root=crate::agent_tables::AgentsStorageRow::ROOT_ID,
            member_deleted=crate::agent_tables::AgentsStorageRow::DELETED_AT),
        sql="SELECT {label} AS {label},{id} AS {id},{seq} AS {seq},{deleted} AS {deleted},{payload} AS {payload},{rowid} AS {rowid} FROM (
            SELECT {label} AS {label},{id} AS {id},{seq} AS {seq},{deleted} AS {deleted},{payload} AS {payload},{rowid} AS {rowid} FROM {by_id} WHERE {kind}=?1 AND {non_transcript}=1 AND {seq}>?2 AND {seq}<=?3 AND ?6 IS NOT NULL AND {id}=?6 AND (?5 IS NULL OR {label} IN (SELECT value FROM json_each(?5)))
            UNION ALL
            SELECT {label} AS {label},{id} AS {id},{seq} AS {seq},{deleted} AS {deleted},{payload} AS {payload},{rowid} AS {rowid} FROM {by_collection} WHERE {kind}=?1 AND {non_transcript}=1 AND {seq}>?2 AND {seq}<=?3 AND ?6 IS NULL AND ?5 IS NOT NULL AND {label} IN (SELECT value FROM json_each(?5))
            UNION ALL
            SELECT {label} AS {label},{id} AS {id},{seq} AS {seq},{deleted} AS {deleted},{payload} AS {payload},{rowid} AS {rowid} FROM {by_sequence} WHERE {kind}=?1 AND {non_transcript}=1 AND {seq}>?2 AND {seq}<=?3 AND ?6 IS NULL AND ?5 IS NULL
            ) AS entity WHERE (?4=0 OR {deleted}=0)
            AND (?7 IS NULL OR (?8 AND (json_extract({payload},'$.value.agent')=?7 OR EXISTS
                (SELECT 1 FROM {members} AS member WHERE member.{member_id}=json_extract(entity.{payload},'$.value.agent')
                    AND member.{member_deleted} IS NULL AND member.{member_root}=?7)))
                OR json_extract({payload},'$.value.rootId')=?7 OR {deleted}=1)
            AND {label} NOT IN (SELECT value FROM json_each(?10)) ORDER BY {seq},{rowid} LIMIT ?9")]
    pub struct ScopedPage<'a> {
        #[db(column=Publications::KIND)]
        pub kind: crate::consolidation_kinds::CanvasPublicationsKind,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub after: i64,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub through: i64,
        #[db(column=Publications::DELETED)]
        pub snapshot: i64,
        #[db(column=Publications::COLLECTION_ORDER)]
        pub collections: Option<&'a str>,
        #[db(column=Publications::ENTITY_ID)]
        pub id: Option<&'a str>,
        #[db(column=crate::agent_tables::AgentsStorageRow::ROOT_ID)]
        pub agent: Option<crate::AgentId>,
        #[db(column=Publications::NON_TRANSCRIPT)]
        pub include_team_members: Option<bool>,
        #[db(column=Publications::PUBLICATION_SEQ)]
        pub fetch: i64,
        #[db(column=Publications::COLLECTION_ORDER)]
        pub excluded: Option<&'a str>,
    }
}

#[cfg(test)]
mod tests {
    use super::Connection;
    use super::SyncDocumentsRepository;

    #[test]
    fn publication_noop_uses_only_hash_and_deleted() {
        let mut db = super::f1_cutover_tests::fixture(7);
        super::f1_cutover_tests::seed_entity(&db, "fixture", "row", 7, 1, false);
        guard_publication_columns(&db);
        let tx = db.transaction().expect("transaction");
        assert!(
            !SyncDocumentsRepository::put_entity_hash(&tx, "fixture", "row", "same", false)
                .expect("no-op does not read payload or sequence")
        );
        tx.rollback().expect("caller owns rollback");
    }

    pub(super) fn guard_publication_columns(db: &Connection) {
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        db.authorizer(Some(|context: AuthContext<'_>| {
            if matches!(
                context.action,
                AuthAction::Read {
                    table_name: "publications",
                    column_name: "entity_payload" | "publication_seq",
                    ..
                }
            ) {
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }))
        .expect("narrow projection guard");
    }

    #[test]
    fn payload_hash_preserves_lowercase_sha256_bytes() {
        for (payload, expected) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "hello",
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
            ),
            (
                "żółć",
                "47ff00909613d05d62e7ba4ae9e6c072d7371dcc16c7eb27edb5d298d21094ec",
            ),
        ] {
            assert_eq!(super::hash_payload(payload), expected);
        }
    }

    fn page_scan_fixture() -> Connection {
        let db = super::f1_cutover_tests::fixture(0);
        for n in 1..=2000 {
            super::f1_cutover_tests::seed_entity(&db, "other", &format!("other-{n}"), n, n, false);
        }
        super::f1_cutover_tests::seed_entity(&db, "fixture", "selected", 2001, 2001, false);
        db
    }

    #[test]
    fn resume_entities_seq_cursor_reads_only_changed_collection_rows() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = page_scan_fixture();
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::{canvas_read, canvas_tx};
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities as Entity};
        // Old selected rows and newer unrelated rows must both stay outside the range.
        for n in 1..=2000 {
            super::f1_cutover_tests::seed_entity(
                &db,
                "fixture",
                &format!("old-{n}"),
                n,
                n + 2001,
                false,
            );
        }
        super::f1_cutover_tests::seed_entity(&db, "fixture", "changed", 4002, 4002, false);
        super::f1_cutover_tests::seed_entity(&db, "fixture", "deleted", 4003, 4003, true);
        let setup = db.transaction().unwrap();
        let other = CollectionName::from_stored("other".into());
        for mut row in studio_db_api::db_query!(
            canvas_read(&setup),
            Entity,
            filter(Entity::COLLECTION_ORDER.eq(&other))
        )
        .unwrap()
        {
            row.publication_seq += 2001;
            canvas_tx(&setup).partition::<Entity>().put(&row).unwrap();
        }
        for n in 1..=2000 {
            super::f1_cutover_tests::seed_live_agent(&setup, &format!("other-{n}"), "other", None);
        }
        super::f1_cutover_tests::seed_live_agent(&setup, "worker", "lead", None);
        let key =
            studio_db_api::migration::encode_key(&("fixture".to_owned(), "changed".to_owned()));
        let mut row = canvas_read(&setup).get::<Entity>(&(key,)).unwrap().unwrap();
        row.entity_payload = EntityValue::from_stored(
            "fixture",
            "changed",
            StoredBoolean::Valid(false),
            Some(r#"{"collection":"fixture","id":"changed","value":{"agent":"worker"}}"#.into()),
        )
        .unwrap();
        canvas_tx(&setup).partition::<Entity>().put(&row).unwrap();
        setup.commit().unwrap();
        let tx = db.transaction().expect("snapshot");
        let steps = Arc::new(AtomicUsize::new(0));
        let counter = steps.clone();
        tx.progress_handler(
            1,
            Some(move || {
                counter.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .expect("count VM work");
        let delta = SyncDocumentsRepository::resume_entity_scope(
            &tx,
            Some(2001),
            Some(r#"["fixture"]"#),
            None,
            Some("lead"),
            true,
            100,
        )
        .expect("changed interval");
        let changed_steps = steps.swap(0, Ordering::Relaxed);
        let empty = SyncDocumentsRepository::resume_entity_scope(
            &tx,
            Some(2001),
            Some(r#"["absent"]"#),
            None,
            None,
            true,
            100,
        )
        .expect("unrelated collection");
        let unrelated_steps = steps.load(Ordering::Relaxed);
        tx.progress_handler(0, None::<fn() -> bool>)
            .expect("stop counting");
        eprintln!(
            "seq_cursor_witness changed_rows={} unrelated_rows={} changed_vm_steps={changed_steps} unrelated_vm_steps={unrelated_steps}",
            delta.rows.len(),
            empty.rows.len()
        );
        assert_eq!(
            delta
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["changed", "deleted"]
        );
        assert!(delta.rows[1].deleted, "deletion retains its sequence");
        assert!(
            empty.rows.is_empty(),
            "unrelated collections deliver zero rows"
        );
        assert!(
            changed_steps < 1000,
            "changed-row read visited old/unrelated rows: {changed_steps} VM steps"
        );
        assert!(
            unrelated_steps < 1000,
            "unrelated collection visited entity rows: {unrelated_steps} VM steps"
        );
    }

    #[test]
    fn scoped_page_reads_only_its_collection_and_id() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = page_scan_fixture();
        let tx = db.transaction().expect("transaction");
        let steps = Arc::new(AtomicUsize::new(0));
        let counter = steps.clone();
        tx.progress_handler(
            1,
            Some(move || {
                counter.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .expect("count steps");
        let page = SyncDocumentsRepository::resume_entity_scope(
            &tx,
            Some(0),
            Some(r#"["fixture"]"#),
            Some("selected"),
            None,
            true,
            10,
        )
        .expect("scoped page");
        tx.progress_handler(0, None::<fn() -> bool>)
            .expect("stop counting");
        assert_eq!(page.rows.len(), 1);
        assert!(
            steps.load(Ordering::Relaxed) < 1000,
            "scoped page scans unrelated rows"
        );
    }

    #[test]
    fn pull_page_stops_at_limit() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = page_scan_fixture();
        let tx = db.transaction().expect("transaction");
        let steps = Arc::new(AtomicUsize::new(0));
        let counter = steps.clone();
        tx.progress_handler(
            1,
            Some(move || {
                counter.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .expect("count steps");
        let rows = super::read_entity_page(
            &tx,
            0,
            0,
            false,
            0,
            10,
            super::EntityPageScope { collections: None },
        )
        .expect("page");
        tx.progress_handler(0, None::<fn() -> bool>)
            .expect("stop counting");
        assert_eq!(rows.len(), 10);
        assert!(
            steps.load(Ordering::Relaxed) < 1000,
            "the page sorts all rows"
        );
    }

    #[test]
    fn resume_entities_current_old_and_expired_cursors() {
        let mut connection = super::f1_cutover_tests::fixture(0);
        for (collection, id, seq, rowid, deleted) in [
            ("fixture", "a", 2, 1, false),
            ("fixture", "b", 4, 2, false),
            ("fixture", "gone", 5, 3, true),
            ("transcript:a", "item:x", 6, 4, false),
        ] {
            super::f1_cutover_tests::seed_entity(&connection, collection, id, seq, rowid, deleted);
        }
        super::f1_cutover_tests::meta(&connection, "entity_tombstone_floor", 1, 1);
        let transaction = connection.transaction().expect("snapshot");
        let current =
            SyncDocumentsRepository::resume_entities(&transaction, Some(5), None, None, 100)
                .expect("current");
        assert!(!current.reset);
        assert!(current.rows.is_empty());
        // The transcript row holds the highest number of the shared counter.
        assert_eq!(current.checkpoint, 6);
        let old = SyncDocumentsRepository::resume_entities(&transaction, Some(2), None, None, 100)
            .expect("older");
        assert!(!old.reset);
        assert_eq!(
            old.rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "gone"]
        );
        for since in [None, Some(0), Some(7), Some(-1)] {
            let expired =
                SyncDocumentsRepository::resume_entities(&transaction, since, None, None, 100)
                    .expect("expired");
            assert!(expired.reset);
            assert_eq!(
                expired
                    .rows
                    .iter()
                    .map(|row| row.id.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
            assert_eq!(expired.checkpoint, 6);
        }
        let scoped = SyncDocumentsRepository::resume_entities(
            &transaction,
            Some(2),
            Some("agent"),
            Some("a"),
            100,
        )
        .expect("scoped");
        assert!(
            scoped.rows.is_empty(),
            "an unrelated entity never enters this ref"
        );
    }

    #[test]
    fn entity_replay_and_patch_preserve_sequence_until_value_or_deletion_changes() {
        let mut connection = super::f1_cutover_tests::fixture(0);
        let transaction = connection.transaction().expect("transaction begins");
        let value = studio_json::json!({"b":2,"a":1});
        assert!(
            SyncDocumentsRepository::put_entity(&transaction, "workspace", "current", value, false)
                .expect("initial entity")
        );
        let before =
            SyncDocumentsRepository::entity_sequence(&transaction).expect("initial sequence");
        assert!(
            !SyncDocumentsRepository::put_entity(
                &transaction,
                "workspace",
                "current",
                studio_json::json!({"a":1,"b":2}),
                false
            )
            .expect("normalized replay")
        );
        SyncDocumentsRepository::patch_entity(
            &transaction,
            "workspace",
            "current",
            studio_json::json!({"a":1}),
        )
        .expect("equal patch");
        assert_eq!(
            SyncDocumentsRepository::entity_sequence(&transaction).expect("unchanged sequence"),
            before
        );
        assert!(
            SyncDocumentsRepository::put_entity(
                &transaction,
                "workspace",
                "current",
                studio_json::json!({"a":2}),
                false
            )
            .expect("content change")
        );
        assert!(
            SyncDocumentsRepository::entity_sequence(&transaction).expect("changed sequence")
                > before
        );
        assert!(
            SyncDocumentsRepository::put_entity(
                &transaction,
                "workspace",
                "current",
                studio_json::json!({}),
                true
            )
            .expect("delete")
        );
        let deleted =
            SyncDocumentsRepository::entity_sequence(&transaction).expect("deleted sequence");
        assert!(
            !SyncDocumentsRepository::put_entity(
                &transaction,
                "workspace",
                "current",
                studio_json::json!({"ignored":true}),
                true
            )
            .expect("repeated delete")
        );
        assert_eq!(
            SyncDocumentsRepository::entity_sequence(&transaction).expect("delete replay sequence"),
            deleted
        );
        assert!(
            SyncDocumentsRepository::put_entity(
                &transaction,
                "workspace",
                "current",
                studio_json::json!({}),
                false
            )
            .expect("revive")
        );
    }

    #[test]
    fn hash_only_entity_uses_shared_sequence_and_replay_is_a_noop() {
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::{canvas_read, canvas_tx};
        use crate::publication_schema::{DocumentValue, SyncDocuments as Document};
        use studio_db_api::migration::encode_key;
        let mut connection = super::f1_cutover_tests::fixture(0);
        let transaction = connection.transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<Document>()
            .put(&Document {
                identity: encode_key(&(9_i64,)),
                source_rowid: 9,
                publication_seq: 9,
                deleted: StoredBoolean::Valid(false),
                document_scope: "drafts".into(),
                document_id: "d".into(),
                document_payload: DocumentValue::from_stored("drafts", "d", "{}".into()).unwrap(),
            })
            .unwrap();
        let digest = "0".repeat(64);
        assert!(
            SyncDocumentsRepository::put_entity_hash(
                &transaction,
                "transcript:agent",
                "item:one",
                &digest,
                false,
            )
            .expect("hash-only entity stores")
        );
        assert!(
            !SyncDocumentsRepository::put_entity_hash(
                &transaction,
                "transcript:agent",
                "item:one",
                &digest,
                false,
            )
            .expect("identical hash replays")
        );
        let stored = canvas_read(&transaction)
            .get::<crate::publication_schema::SyncEntities>(&(encode_key(&(
                "transcript:agent".to_owned(),
                "item:one".to_owned(),
            )),))
            .unwrap()
            .unwrap();
        let row = (
            stored.publication_seq,
            stored.entity_payload.value().raw().map(str::to_owned),
        );
        assert_eq!(row, (10, None));
        transaction.commit().expect("transaction commits");
    }
}

#[cfg(test)]
mod f1_cutover_tests {
    use super::Connection;
    use super::SyncDocumentsRepository as Repository;
    use crate::change_schema::Changes;
    use crate::consolidation_codec::nullable_text_identity;
    use crate::metadata_schema::{
        DurableFloor, Metadata, StoredSyncMetaValue, SyncEntityMeta, SyncGeneration,
    };
    use crate::metadata_values::StoredBoolean;
    use crate::peer_update_settings::{canvas_read, canvas_tx};
    use crate::publication_collection::{CollectionKind, CollectionName};
    use crate::publication_schema::{EntityValue, Publications, SyncEntities};
    use studio_db_api::Table;
    use studio_db_api::migration::encode_key;

    pub(super) fn fixture(high: i64) -> Connection {
        let db = Connection::open_in_memory().unwrap();
        for descriptor in [
            Metadata::descriptor(),
            Changes::descriptor(),
            Publications::descriptor(),
            crate::agent_columns::typed::TypedAgentRow::descriptor(),
            crate::agent_tables::AgentsStorageRow::descriptor(),
            crate::agent_tables::AgentVolatileStorageRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        let transaction = db.unchecked_transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<SyncGeneration>()
            .put(&SyncGeneration {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                sync_generation: high,
            })
            .unwrap();
        canvas_tx(&transaction)
            .partition::<DurableFloor>()
            .put(&DurableFloor {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                durable_floor: 0,
            })
            .unwrap();
        transaction.commit().unwrap();
        db
    }

    pub(super) fn seed_entity(
        db: &Connection,
        label: &str,
        id: &str,
        sequence: i64,
        rowid: i64,
        deleted: bool,
    ) {
        let collection = CollectionName::from_stored(label.into());
        let deleted = StoredBoolean::Valid(deleted);
        let hash = if collection.kind() == CollectionKind::Transcript {
            "0".repeat(64)
        } else {
            "same".into()
        };
        let transaction = db.unchecked_transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<SyncEntities>()
            .put(&SyncEntities {
                identity: encode_key(&(label.to_owned(), id.to_owned())),
                source_rowid: rowid,
                publication_seq: sequence,
                deleted,
                collection_kind: collection.kind(),
                collection_agent: collection.agent().cloned(),
                non_transcript: collection.non_transcript(),
                collection_order: collection,
                entity_id: id.into(),
                entity_hash: hash,
                entity_payload: EntityValue::from_stored(label, id, deleted, None).unwrap(),
            })
            .unwrap();
        transaction.commit().unwrap();
    }

    pub(super) fn meta(db: &Connection, key: &str, value: i64, rowid: i64) {
        let key = Some(key.to_owned());
        let transaction = db.unchecked_transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<SyncEntityMeta>()
            .put(&SyncEntityMeta {
                identity: nullable_text_identity(&key, rowid),
                source_rowid: rowid,
                meta_value: StoredSyncMetaValue::from_stored(key.clone(), value.to_string()),
                text_key: key,
            })
            .unwrap();
        transaction.commit().unwrap();
    }

    #[test]
    fn raw_publication_lookup_does_not_decode_unselected_envelope_fields() {
        let db = fixture(0);
        seed_entity(&db, "project", "one", 42, 7, false);
        let raw =
            r#"{"id":"different","collection":"project","value":{"unknown":true}}"#.to_owned();
        let transaction = db.unchecked_transaction().unwrap();
        let mut typed = canvas_tx(&transaction);
        let identity = encode_key(&("project".to_owned(), "one".to_owned()));
        #[derive(studio_db_api::DbQuery)]
        #[db_query(database=crate::peer_update_settings::Canvas,mode=write,cardinality=none,result=(),
            tables(rows=Publications),columns(payload=Publications::ENTITY_PAYLOAD,kind=Publications::KIND,identity=Publications::IDENTITY),
            sql="UPDATE {rows} SET {payload}=?1 WHERE {kind}=?2 AND {identity}=?3")]
        struct FixturePayload {
            #[db(column=Publications::ENTITY_PAYLOAD)]
            payload: Option<String>,
            #[db(column=Publications::KIND)]
            kind: crate::consolidation_kinds::CanvasPublicationsKind,
            #[db(column=Publications::IDENTITY)]
            identity: Vec<u8>,
        }
        typed
            .named(
                &FixturePayload::QUERY,
                &FixturePayload {
                    payload: Some(raw.clone()),
                    kind: crate::consolidation_kinds::CanvasPublicationsKind::SyncEntities,
                    identity,
                },
            )
            .unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            super::publication_payload(&db, "project", "one", true).unwrap(),
            Some(raw)
        );
    }

    #[test]
    fn canonical_lookup_preserves_missing_null_and_integer_deletion() {
        let db = fixture(0);
        seed_entity(&db, "project", "one", 42, 7, false);
        assert_eq!(
            super::publication_payload_nullable(&db, "project", "one", true).unwrap(),
            Some(None)
        );
        assert_eq!(
            super::publication_payload_nullable(&db, "project", "missing", true).unwrap(),
            None
        );
        assert!(
            super::publication_payload(&db, "project", "missing", true)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            super::publication_payload(&db, "project", "one", true),
            Err(crate::StoreError::Sqlite(
                rusqlite::Error::InvalidColumnType(_, _, rusqlite::types::Type::Null)
            ))
        ));
        assert_eq!(
            super::publication_sequence(&db, "project", "one", false).unwrap(),
            Some(42)
        );
        assert_eq!(
            super::publication_sequence(&db, "project", "one", true).unwrap(),
            None
        );
        let identity = encode_key(&("project".to_owned(), "one".to_owned()));
        let mut row = canvas_read(&db)
            .get::<SyncEntities>(&(identity,))
            .unwrap()
            .unwrap();
        row.deleted = StoredBoolean::LegacyInteger(2);
        row.entity_payload = EntityValue::from_stored("project", "one", row.deleted, None).unwrap();
        let tx = db.unchecked_transaction().unwrap();
        canvas_tx(&tx)
            .partition::<SyncEntities>()
            .put(&row)
            .unwrap();
        tx.commit().unwrap();
        for deleted in [false, true] {
            assert_eq!(
                super::publication_sequence(&db, "project", "one", deleted).unwrap(),
                None
            );
        }
    }

    #[test]
    fn canonical_tombstone_producers_preserve_insert_update_delete_and_rollback() {
        let mut db = fixture(0);
        meta(&db, super::ENTITY_TOMBSTONE_COUNT_KEY, 0, 1);
        {
            let transaction = db.transaction().unwrap();
            crate::bootstrap::install_current_producers(&transaction).unwrap();
            transaction.commit().unwrap();
        }
        seed_entity(&db, "fixture", "one", 1, 1, true);
        assert_eq!(
            super::publication_meta(&db, super::ENTITY_TOMBSTONE_COUNT_KEY)
                .unwrap()
                .as_deref(),
            Some("1")
        );
        seed_entity(&db, "fixture", "one", 2, 1, false);
        assert_eq!(
            super::publication_meta(&db, super::ENTITY_TOMBSTONE_COUNT_KEY)
                .unwrap()
                .as_deref(),
            Some("0")
        );
        seed_entity(&db, "fixture", "one", 3, 1, true);
        {
            let transaction = db.transaction().unwrap();
            canvas_tx(&transaction)
                .delete::<SyncEntities>(&(encode_key(&("fixture".to_owned(), "one".to_owned())),))
                .unwrap();
            assert_eq!(
                super::publication_meta(&transaction, super::ENTITY_TOMBSTONE_COUNT_KEY)
                    .unwrap()
                    .as_deref(),
                Some("0")
            );
        }
        assert_eq!(
            super::publication_meta(&db, super::ENTITY_TOMBSTONE_COUNT_KEY)
                .unwrap()
                .as_deref(),
            Some("1")
        );
    }

    #[test]
    fn noop_marks_publication_without_reading_payload_or_allocating_sequence() {
        let mut db = fixture(50);
        seed_entity(&db, "agent", "a", 7, 13, false);
        super::tests::guard_publication_columns(&db);
        let transaction = db.transaction().unwrap();
        crate::entity_publication::begin();
        crate::entity_publication::mark_stale("agent", "a");
        assert!(!Repository::put_entity_hash(&transaction, "agent", "a", "same", false).unwrap());
        let pending =
            crate::entity_publication::pending(&transaction, crate::entity_publication::finish())
                .unwrap();
        assert!(pending.agents.is_empty());
    }

    #[test]
    fn allocator_ignores_non_singleton_generation_rows() {
        let mut db = fixture(50);
        let transaction = db.transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<SyncGeneration>()
            .put(&SyncGeneration {
                identity: encode_key(&(2_i64,)),
                source_rowid: 2,
                integer_key: 2,
                sync_generation: 999,
            })
            .unwrap();
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), 50);
    }

    #[test]
    fn changed_hash_allocates_once_and_replay_keeps_the_shared_high() {
        let mut db = fixture(50);
        seed_entity(&db, "fixture", "a", 7, 13, false);
        let transaction = db.transaction().unwrap();
        assert!(
            Repository::put_entity_hash(&transaction, "fixture", "a", "changed", false).unwrap()
        );
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), 51);
        assert!(
            !Repository::put_entity_hash(&transaction, "fixture", "a", "changed", false).unwrap()
        );
        let row = canvas_read(&transaction)
            .get::<SyncEntities>(&(encode_key(&("fixture".to_owned(), "a".to_owned())),))
            .unwrap()
            .unwrap();
        assert_eq!(row.publication_seq, 51);
        assert_eq!(row.source_rowid, 13);
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), 51);
    }

    #[test]
    fn entity_page_ties_follow_original_rowid_after_sequence() {
        let db = fixture(7);
        for (id, rowid) in [("last", 19), ("first", -5), ("middle", 3)] {
            seed_entity(&db, "fixture", id, 7, rowid, false);
        }
        let rows = Repository::entities_after(&db, 0, 10).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["first", "middle", "last"]
        );
    }

    #[test]
    fn scoped_replay_and_global_pull_keep_their_distinct_watermarks() {
        let mut db = fixture(50);
        seed_entity(&db, "fixture", "live", 7, 1, false);
        seed_entity(&db, "fixture", "removed", 9, 2, true);
        let transaction = db.transaction().unwrap();
        let global = Repository::pull_entities(&transaction, 0, 10, false, 0, true).unwrap();
        assert_eq!(global.max_sequence, 9);
        let scoped =
            Repository::resume_entities(&transaction, None, Some("fixture"), None, 10).unwrap();
        assert_eq!(scoped.max_sequence, 50);
        assert!(scoped.reset);
        assert_eq!(
            scoped
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["live"]
        );
        let delta =
            Repository::resume_entities(&transaction, Some(7), Some("fixture"), None, 10).unwrap();
        assert!(!delta.reset);
        assert_eq!(
            delta
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["removed"]
        );
        assert_eq!(delta.checkpoint, 50);
    }

    #[test]
    fn highest_document_removal_never_reuses_an_allocated_sequence() {
        use crate::publication_schema::SyncDocuments;
        let mut db = fixture(0);
        let transaction = db.transaction().unwrap();
        let first = r#"{"session":"s","device":"d","text":"one","updated":1}"#;
        let second = r#"{"session":"s","device":"d","text":"two","updated":2}"#;
        assert!(Repository::put_draft(&transaction, "d:s", first, false).unwrap());
        let sequence = Repository::draft_sequence(&transaction).unwrap();
        canvas_tx(&transaction)
            .partition::<SyncDocuments>()
            .delete(&(encode_key(&(sequence,)),))
            .unwrap();
        assert_eq!(Repository::draft_sequence(&transaction).unwrap(), 0);
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), sequence);
        assert!(Repository::put_draft(&transaction, "d:s", second, false).unwrap());
        assert!(Repository::draft_sequence(&transaction).unwrap() > sequence);
    }

    #[test]
    fn exhausted_sequence_refuses_without_inserting_a_document() {
        let mut db = fixture(i64::MAX);
        let transaction = db.transaction().unwrap();
        let payload = r#"{"session":"s","device":"d","text":"one","updated":1}"#;
        assert!(Repository::put_draft(&transaction, "d:s", payload, false).is_err());
        assert!(
            canvas_tx(&transaction)
                .partition::<crate::publication_schema::SyncDocuments>()
                .list()
                .unwrap()
                .is_empty()
        );
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), i64::MAX);
    }

    pub(super) fn seed_live_agent(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
        root: &str,
        deleted: Option<f64>,
    ) {
        let record = crate::json_record_codec::RawJsonRecord::from_stored(transaction,id,
            studio_json::json!({"id":id,"rootId":root,"provider":"codex","accountKey":"default","epoch":1,"status":"idle","deletedAt":deleted}).to_string()
        ).unwrap();
        canvas_tx(transaction)
            .insert(&crate::agent_tables::AgentsStorageRow {
                hold_status: studio_runtime_schema::persistence::HoldStatus::Clear,
                parent_id: None,
                cwd: None,
                account_key: String::new(),
                id: id.into(),
                record,
                root_id: None,
                deleted_at: None,
            })
            .unwrap();
    }

    #[test]
    fn team_visibility_reads_live_agents_without_prepared_agent_target() {
        let mut db = fixture(50);
        seed_entity(&db, "fixture", "item", 1, 1, false);
        let transaction = db.transaction().unwrap();
        studio_db_api::migration::retire_source::<crate::agent_columns::typed::TypedAgentRow>(
            &mut canvas_tx(&transaction),
        )
        .unwrap();
        seed_live_agent(&transaction, "worker", "root", None);
        let identity = encode_key(&("fixture".to_owned(), "item".to_owned()));
        let mut row = canvas_read(&transaction)
            .get::<SyncEntities>(&(identity,))
            .unwrap()
            .unwrap();
        row.entity_payload = EntityValue::from_stored(
            "fixture",
            "item",
            StoredBoolean::Valid(false),
            Some(r#"{"collection":"fixture","id":"item","value":{"agent":"worker"}}"#.into()),
        )
        .unwrap();
        canvas_tx(&transaction)
            .partition::<SyncEntities>()
            .put(&row)
            .unwrap();
        let page = Repository::resume_entity_scope(
            &transaction,
            None,
            Some(r#"["fixture"]"#),
            None,
            Some("root"),
            true,
            20,
        )
        .unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].id, "item");
    }

    #[test]
    fn team_visibility_requires_a_live_present_agent_component() {
        use crate::agent_columns::typed::{AgentIdentity, TypedAgentRow};
        let mut db = fixture(50);
        for (n, (id, owner)) in [
            ("live-owner", "live"),
            ("deleted-owner", "deleted"),
            ("chat-owner", "chat"),
            ("other-owner", "other"),
            ("self", "root"),
        ]
        .into_iter()
        .enumerate()
        {
            seed_entity(&db, "fixture", id, n as i64 + 1, n as i64 + 1, false);
            let transaction = db.unchecked_transaction().unwrap();
            let key = encode_key(&("fixture".to_owned(), id.to_owned()));
            let mut row = canvas_read(&transaction)
                .get::<SyncEntities>(&(key,))
                .unwrap()
                .unwrap();
            row.entity_payload = EntityValue::from_stored(
                "fixture",
                id,
                StoredBoolean::Valid(false),
                Some(
                    studio_json::json!({"collection":"fixture","id":id,"value":{"agent":owner}})
                        .to_string(),
                ),
            )
            .unwrap();
            canvas_tx(&transaction)
                .partition::<SyncEntities>()
                .put(&row)
                .unwrap();
            transaction.commit().unwrap();
        }
        let transaction = db.transaction().unwrap();
        for (id, present, deleted, root) in [
            ("live", true, None, "root"),
            ("deleted", true, Some(1.5), "root"),
            ("chat", false, None, "root"),
            ("other", true, None, "elsewhere"),
        ] {
            if present {
                seed_live_agent(&transaction, id, root, deleted);
            }
            let id: crate::AgentId = id.into();
            canvas_tx(&transaction)
                .insert(&TypedAgentRow {
                    identity: AgentIdentity::for_agent(&id),
                    id: Some(id),
                    root_id: Some(root.into()),
                    deleted_at: deleted,
                    agent_present: present,
                })
                .unwrap();
        }
        let page = Repository::resume_entity_scope(
            &transaction,
            None,
            Some(r#"["fixture"]"#),
            None,
            Some("root"),
            true,
            20,
        )
        .unwrap();
        assert_eq!(
            page.rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["live-owner", "self"]
        );
        let own = Repository::resume_entity_scope(
            &transaction,
            None,
            Some(r#"["fixture"]"#),
            None,
            Some("root"),
            false,
            20,
        )
        .unwrap();
        assert!(
            own.rows.is_empty(),
            "the existing include_team_members switch gates both agent-owner alternatives"
        );
    }

    #[test]
    fn scoped_query_shapes_use_their_owned_indexes() {
        use super::publication_queries::ScopedPage;
        use crate::consolidation_kinds::CanvasPublicationsKind;
        use crate::publication_schema::Publications;
        let db = fixture(50);
        for (collections, id) in [
            (None, None),
            (Some(r#"["fixture"]"#), None),
            (Some(r#"["fixture"]"#), Some("a")),
        ] {
            let params = ScopedPage {
                kind: CanvasPublicationsKind::SyncEntities,
                after: 0,
                through: 50,
                snapshot: 0,
                collections,
                id,
                agent: Some("root".into()),
                include_team_members: Some(true),
                fetch: 11,
                excluded: Some("[]"),
            };
            let plan =
                crate::query_plan_test_support::explain(&db, ScopedPage::QUERY.sql(), &params)
                    .unwrap()
                    .join("\n");
            for name in [
                studio_db_api::query_index::<crate::peer_update_settings::Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_ID,
                ),
                studio_db_api::query_index::<crate::peer_update_settings::Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE,
                ),
                studio_db_api::query_index::<crate::peer_update_settings::Canvas, Publications>(
                    &Publications::INDEX_PUBLICATIONS_ENTITY_NON_TRANSCRIPT_SEQUENCE,
                ),
            ] {
                assert!(
                    plan.contains(name.trim_matches('"')),
                    "missing {name}: {plan}"
                );
            }
            assert!(
                plan.lines()
                    .any(|line| line.contains("SEARCH member USING INDEX")
                        && line.contains("(id=?)")),
                "member lookup must use the current source primary key: {plan}"
            );
        }
    }

    #[test]
    fn bounded_pruning_preserves_floor_counter_and_sequence_high() {
        let limit = super::ENTITY_TOMBSTONE_LIMIT;
        let batch = super::ENTITY_TOMBSTONE_PRUNE_BATCH;
        let total = limit + batch + 1;
        let mut db = fixture(total);
        for sequence in 1..=total {
            seed_entity(
                &db,
                "fixture",
                &format!("row-{sequence}"),
                sequence,
                sequence,
                true,
            );
        }
        meta(&db, "entity_tombstone_count", total, 1);
        meta(&db, "entity_tombstone_floor", 0, 2);
        let transaction = db.transaction().unwrap();
        assert!(Repository::entity_tombstone_prune_due(&transaction).unwrap());
        assert_eq!(
            Repository::prune_entity_tombstones(&transaction).unwrap(),
            batch as usize
        );
        assert_eq!(
            Repository::entity_tombstone_floor(&transaction).unwrap(),
            batch
        );
        assert_eq!(Repository::entity_sequence(&transaction).unwrap(), total);
        assert!(!Repository::entity_tombstone_prune_due(&transaction).unwrap());
        let row = canvas_read(&transaction)
            .get::<SyncEntityMeta>(&(nullable_text_identity(
                &Some("entity_tombstone_count".into()),
                1,
            ),))
            .unwrap()
            .unwrap();
        assert_eq!(row.meta_value.as_stored(), (total - batch).to_string());
    }
}
