//! Persisted transcript rows, full text, and the current search projection.

use crate::CachedSql;
use crate::consolidation_kinds::CanvasPublicationsKind;
use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
use crate::publication_schema::{Publications, SyncEntities};
use crate::records::{RecordOrder, RecordPredicate, Table, TranscriptItemId, TranscriptItemRecord};
use crate::repositories::sync_documents::SyncDocumentsRepository;
use crate::schema::{ConditionalTableRequirement, SchemaRequirements, TableRequirement};
use crate::stored_text::{StoredText, StoredTextValue};
use crate::text_blobs;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use studio_db_api::{DbProjection, DbQuery};
use studio_json::{Value, json};
use studio_runtime_schema::shared_values::TRANSCRIPT_MAX_TOMBSTONES;
use thiserror::Error;

// Hash-only transcript rows deliberately avoid the optional payload decoder.
#[derive(DbProjection)]
#[db(of=Publications)]
struct TranscriptHashRow {
    entity_id: Option<String>,
    publication_seq: i64,
    entity_hash: Option<String>,
    deleted: i64,
}

#[derive(DbProjection)]
#[db(of=Publications)]
struct TranscriptChangeRow {
    entity_id: Option<String>,
    publication_seq: i64,
    deleted: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=TranscriptHashRow,projection=TranscriptHashRow,
    tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE),
    columns(id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,hash=Publications::ENTITY_HASH,
        deleted=Publications::DELETED,kind=Publications::KIND,label=Publications::COLLECTION_ORDER,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id},{seq},{hash},{deleted} FROM {rows} WHERE {kind}=?1 AND {label}=?2 ORDER BY {rowid}")]
struct TranscriptCurrent<'a> {
    #[db(column=Publications::KIND)]
    kind: CanvasPublicationsKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    scope: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=TranscriptChangeRow,projection=TranscriptChangeRow,
    tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),
    predicates(entities=rows),
    columns(id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,deleted=Publications::DELETED,
        label=Publications::COLLECTION_ORDER,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id},{seq},{deleted} FROM {rows} WHERE {entities} AND {label}=?1 AND {deleted}=1
        AND {id} LIKE 'item:%' ORDER BY {seq} DESC,{rowid} LIMIT -1 OFFSET ?2")]
struct TranscriptPruned<'a> {
    #[db(column=Publications::COLLECTION_ORDER)]
    scope: Option<&'a str>,
    #[db(column=Publications::PUBLICATION_SEQ)]
    retained: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Publications::PUBLICATION_SEQ,
    tables(rows=Publications),indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE),columns(seq=Publications::PUBLICATION_SEQ,kind=Publications::KIND,label=Publications::COLLECTION_ORDER),
    sql="SELECT COALESCE(MAX({seq}),0) AS {seq} FROM {rows} WHERE {kind}=?1 AND {label}=?2")]
struct TranscriptHigh<'a> {
    #[db(column=Publications::KIND)]
    kind: CanvasPublicationsKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    scope: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=TranscriptChangeRow,projection=TranscriptChangeRow,
    tables(rows=Publications),
    columns(id=Publications::ENTITY_ID,seq=Publications::PUBLICATION_SEQ,deleted=Publications::DELETED,
        kind=Publications::KIND,label=Publications::COLLECTION_ORDER,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id},{seq},{deleted} FROM {rows} WHERE {kind}=?1 AND {label}=?2 AND {seq}>?3 AND {id}!='@floor' ORDER BY {seq},{rowid}")]
struct TranscriptDelta<'a> {
    #[db(column=Publications::KIND)]
    kind: CanvasPublicationsKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    scope: Option<&'a str>,
    #[db(column=Publications::PUBLICATION_SEQ)]
    after: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(rows=Publications),
    columns(kind=Publications::KIND,label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD),
    sql="UPDATE {rows} SET {payload}=?3 WHERE {kind}=?1 AND {label}=?2 AND {id}='@revision'")]
struct CacheRevisionPayload<'a> {
    #[db(column=Publications::KIND)]
    kind: CanvasPublicationsKind,
    #[db(column=Publications::COLLECTION_ORDER)]
    scope: Option<&'a str>,
    #[db(column=Publications::ENTITY_PAYLOAD)]
    payload: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=String,decode=crate::agent_tables::AgentsStorageRow::ID,
    tables(changes=crate::change_schema::Changes,agents=crate::agent_tables::AgentsStorageRow,items=crate::item_monitor_terminal::ItemsRow),
    indexes(changes=crate::change_schema::Changes::INDEX_CHANGES_DURABLE_IDENTITY),predicates(durable=changes),
    columns(agent_id=crate::agent_tables::AgentsStorageRow::ID,deleted=crate::agent_tables::AgentsStorageRow::DELETED_AT,
        item_agent=crate::item_monitor_terminal::ItemsRow::AGENT,
        label=crate::change_schema::Changes::SOURCE_LABEL,id=crate::change_schema::Changes::DURABLE_ID,
        seq=crate::change_schema::Changes::SEQUENCE,rowid=crate::change_schema::Changes::SOURCE_ROWID),
    sql="SELECT a.{agent_id} FROM (SELECT {id},{seq},{rowid} FROM {changes} WHERE {durable} AND {label}=?1) d
        JOIN {agents} a ON d.{id}=a.{agent_id} WHERE a.{deleted} IS NULL
        AND EXISTS(SELECT 1 FROM {items} i WHERE i.{item_agent}=a.{agent_id})
        ORDER BY d.{seq} DESC,d.{rowid} DESC LIMIT ?2")]
struct RecentChatIds<'a> {
    #[db(column=crate::change_schema::Changes::SOURCE_LABEL)]
    label: Option<&'a str>,
    #[db(column=crate::change_schema::Changes::SOURCE_ROWID)]
    limit: i64,
}

fn transcript_current(
    connection: &Connection,
    scope: &str,
) -> rusqlite::Result<std::collections::BTreeMap<String, (i64, String, bool)>> {
    canvas_read(connection)
        .collect(
            &TranscriptCurrent::QUERY,
            &TranscriptCurrent {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some(scope),
            },
        )?
        .into_iter()
        .map(|row| {
            Ok((
                row.entity_id.ok_or(rusqlite::Error::InvalidQuery)?,
                (
                    row.publication_seq,
                    row.entity_hash.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.deleted != 0,
                ),
            ))
        })
        .collect()
}

#[cfg(any(test, feature = "test-support"))]
#[derive(DbProjection)]
#[db(of=Publications)]
struct TranscriptTestRow {
    collection_order: Option<String>,
    entity_id: Option<String>,
    publication_seq: i64,
    entity_hash: Option<String>,
    entity_payload: Option<String>,
    deleted: i64,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,
    result=TranscriptTestRow,projection=TranscriptTestRow,tables(rows=Publications),
    columns(label=Publications::COLLECTION_ORDER,id=Publications::ENTITY_ID,
        seq=Publications::PUBLICATION_SEQ,hash=Publications::ENTITY_HASH,
        payload=Publications::ENTITY_PAYLOAD,deleted=Publications::DELETED,kind=Publications::KIND),
    sql="SELECT {label},{id},{seq},{hash},{payload},{deleted} FROM {rows}
        WHERE {kind}=?1 AND {label} LIKE 'transcript:%' ORDER BY {label},{id}")]
struct TranscriptTestSnapshot {
    #[db(column=Publications::KIND)]
    kind: CanvasPublicationsKind,
}

const BOOTSTRAP_TABLES: &[TableRequirement] = &[
    TableRequirement {
        name: "runtime_item_fulltext",
        columns: &["id", "body"],
    },
    TableRequirement {
        name: "runtime_item_blobs",
        columns: &["id", "hash", "size"],
    },
];
const SEARCH_NEXT_META_TABLES: &[TableRequirement] = &[TableRequirement {
    name: "runtime_search_next_meta",
    columns: &["id", "agent", "kind", "search_rowid"],
}];
const SEARCH_ROLLOUT_TABLES: &[TableRequirement] = &[TableRequirement {
    name: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
    columns: &[
        crate::metadata_schema::Metadata::__DB_COLUMN_KIND,
        crate::metadata_schema::Metadata::__DB_COLUMN_IDENTITY,
        crate::metadata_schema::Metadata::__DB_COLUMN_SEARCH_PHASE,
        crate::metadata_schema::Metadata::__DB_COLUMN_SEARCH_CURSOR,
        crate::metadata_schema::Metadata::__DB_COLUMN_SEARCH_UPDATED,
    ],
}];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[
        TableRequirement {
            name: "runtime_items",
            columns: &["id", "agent", "record", "created"],
        },
        TableRequirement {
            name: "runtime_agents",
            columns: &["id", "record"],
        },
        TableRequirement {
            name: "runtime_events",
            columns: &["id", "agent", "text"],
        },
    ],
    conditional_tables: &[
        ConditionalTableRequirement {
            table_exists: "runtime_search_next_meta",
            tables: SEARCH_NEXT_META_TABLES,
        },
        ConditionalTableRequirement {
            table_exists: <crate::metadata_schema::Metadata as studio_db_api::Table>::NAME,
            tables: SEARCH_ROLLOUT_TABLES,
        },
    ],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: BOOTSTRAP_TABLES,
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Debug, Error)]
pub enum TranscriptStoreError {
    #[error("SQLite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid stored object {table}/{id}")]
    InvalidStoredObject { table: &'static str, id: String },
    #[error(transparent)]
    Store(#[from] crate::StoreError),
}

pub type TranscriptStoreResult<T> = Result<T, TranscriptStoreError>;

#[derive(Clone, Debug)]
pub struct TranscriptItem {
    pub id: String,
    pub agent: String,
    pub record: TranscriptItemRecord,
}

/// A compatibility view for archive and raw full-text readers. Fields not used
/// by those readers are deliberately ignored, including provider extensions.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredTranscriptView {
    pub id: Option<String>,
    pub role: Option<String>,
    #[serde(default)]
    pub text: studio_runtime_schema::queue_command::Field<String>,
    #[serde(default)]
    pub inputs: studio_runtime_schema::queue_command::Field<Vec<StoredTranscriptInput>>,
    pub truncated: Option<bool>,
    after_restore: Option<studio_json::OwnedRawJson>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct StoredTranscriptInput {
    pub id: Option<String>,
    pub truncated: Option<bool>,
}

impl StoredTranscriptView {
    pub fn after_restore(&self) -> bool {
        crate::schema::raw_truthy(self.after_restore.as_ref())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TranscriptSearchKind<'a> {
    pub indexed: bool,
    pub kind: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TranscriptRepository;

impl TranscriptRepository {
    pub fn agent(
        connection: &Connection,
        agent: &str,
    ) -> TranscriptStoreResult<Option<crate::AgentRecord>> {
        crate::AgentRepository::get(connection, &crate::AgentId::from(agent))
            .map_err(map_store_error)
    }

    /// Exact transcript ledger evidence for cross-crate replay and copy-retirement witnesses.
    #[cfg(any(test, feature = "test-support"))]
    #[expect(
        clippy::type_complexity,
        reason = "test evidence preserves the six native stored columns"
    )]
    pub fn test_publication_snapshot(
        connection: &Connection,
    ) -> TranscriptStoreResult<Vec<(String, String, i64, String, Option<String>, i64)>> {
        canvas_read(connection)
            .collect(
                &TranscriptTestSnapshot::QUERY,
                &TranscriptTestSnapshot {
                    kind: CanvasPublicationsKind::SyncEntities,
                },
            )?
            .into_iter()
            .map(|row| {
                Ok((
                    row.collection_order.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.entity_id.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.publication_seq,
                    row.entity_hash.ok_or(rusqlite::Error::InvalidQuery)?,
                    row.entity_payload,
                    row.deleted,
                ))
            })
            .collect()
    }

    /// Current sequence of this transcript's publication partition.
    pub fn publication_high(connection: &Connection, scope: &str) -> TranscriptStoreResult<i64> {
        Ok(canvas_read(connection).named(
            &TranscriptHigh::QUERY,
            &TranscriptHigh {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some(scope),
            },
        )?)
    }

    /// Read hash-only revision evidence without admitting the optional cache payload.
    pub fn cached_revision_hash(
        connection: &Connection,
        scope: &str,
    ) -> TranscriptStoreResult<Option<String>> {
        let identity =
            studio_db_api::migration::encode_key(&(scope.to_owned(), "@revision".to_owned()));
        Ok(studio_db_api::db_query!(
            canvas_read(connection),
            SyncEntities,
            scalar(SyncEntities::ENTITY_HASH),
            filter(
                SyncEntities::IDENTITY.eq_ref(&identity),
                SyncEntities::DELETED.eq(&crate::metadata_values::StoredBoolean::Valid(false))
            )
        )?)
    }

    /// The service validates this raw page with its existing error labels.
    pub fn cached_revision_payload(
        connection: &Connection,
        scope: &str,
        hash: &str,
    ) -> TranscriptStoreResult<Option<String>> {
        let identity =
            studio_db_api::migration::encode_key(&(scope.to_owned(), "@revision".to_owned()));
        Ok(studio_db_api::db_query!(
            canvas_read(connection),
            Publications,
            scalar(Publications::ENTITY_PAYLOAD),
            filter(
                Publications::KIND.eq(&CanvasPublicationsKind::SyncEntities),
                Publications::IDENTITY.eq(&identity),
                Publications::ENTITY_HASH.eq(&Some(hash.to_owned())),
                Publications::DELETED.eq(&0)
            )
        )?
        .flatten())
    }

    /// Keep the bounded page on the existing revision without advancing its sequence.
    pub fn cache_revision_payload(
        transaction: &Transaction<'_>,
        scope: &str,
        payload: &str,
    ) -> TranscriptStoreResult<()> {
        canvas_tx(transaction).named(
            &CacheRevisionPayload::QUERY,
            &CacheRevisionPayload {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some(scope),
                payload: Some(payload),
            },
        )?;
        Ok(())
    }

    /// Recent live conversations with items, in durable change order.
    pub fn recent_chat_ids(
        connection: &Connection,
        limit: i64,
    ) -> TranscriptStoreResult<Vec<String>> {
        use studio_db_api::Table;
        Ok(canvas_read(connection).collect(
            &RecentChatIds::QUERY,
            &RecentChatIds {
                label: Some(crate::agent_tables::AgentsStorageRow::NAME),
                limit,
            },
        )?)
    }

    /// Durable conversation content revision, independent of other agents.
    pub fn revision(connection: &Connection, agent: &str) -> crate::StoreResult<i64> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::change_schema::TranscriptRevisions>(&(
                crate::consolidation_codec::nullable_text_identity(&Some(agent.to_owned()), 0),
            ))?
            .map_or(0, |row| row.revision))
    }

    /// Project an already checked transcript snapshot using the existing durable sync cursor.
    pub fn pull_payload(
        transaction: &Transaction<'_>,
        scope: &str,
        after: i64,
        payload: &Value,
        deleted: bool,
        revision: Option<i64>,
    ) -> TranscriptStoreResult<Value> {
        transcript_pull_write(transaction, scope, after, payload, deleted, revision)
    }

    pub fn search_hits(
        connection: &Connection,
        match_query: &str,
    ) -> TranscriptStoreResult<Vec<(String, String, String)>> {
        if matches!(
            search_phase(connection)?.as_str(),
            "active" | "dropping" | "complete"
        ) {
            let mut statement = connection.prepare_cached(
                "SELECT m.id,m.agent,m.kind FROM runtime_search_next \
                 JOIN runtime_search_next_meta m ON m.search_rowid=runtime_search_next.rowid \
                 JOIN runtime_items i ON i.id=m.id \
                 WHERE runtime_search_next MATCH ?1 \
                   AND json_extract(i.record,'$.afterRestore') IS NULL \
                 ORDER BY rank LIMIT 1000",
            )?;
            let rows = statement.query_map([match_query], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            return rows.collect::<Result<Vec<_>, _>>().map_err(Into::into);
        }
        Ok(Vec::new())
    }

    /// Conversation liveness without decoding unrelated agent fields.
    pub fn agent_deleted(
        connection: &Connection,
        agent: &str,
    ) -> TranscriptStoreResult<Option<bool>> {
        crate::AgentRepository::deletion_status(connection, agent).map_err(map_store_error)
    }

    pub fn item(
        connection: &Connection,
        agent: &str,
        id: &str,
    ) -> TranscriptStoreResult<Option<TranscriptItem>> {
        Table::<TranscriptItemRecord>::new(connection)
            .get_matching(
                &TranscriptItemId::from(id),
                Some(RecordPredicate::PhysicalAgentEquals(agent)),
            )
            .map(|item| {
                item.map(|item| {
                    let id = item.id().as_ref().to_owned();
                    TranscriptItem {
                        id,
                        agent: agent.to_owned(),
                        record: item,
                    }
                })
            })
            .map_err(map_store_error)
    }

    pub fn item_any_agent(
        connection: &Connection,
        id: &str,
    ) -> TranscriptStoreResult<Option<TranscriptItem>> {
        let agent = connection
            .query_row_cached("SELECT agent FROM runtime_items WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        let Some(agent) = agent else {
            return Ok(None);
        };
        Self::item(connection, &agent, id)
    }

    pub fn items_for_agent(
        connection: &Connection,
        agent: &str,
    ) -> TranscriptStoreResult<Vec<TranscriptItem>> {
        Self::items_for_agent_with_restore(connection, agent, false)
    }

    pub fn items_for_agent_including_restored(
        connection: &Connection,
        agent: &str,
    ) -> TranscriptStoreResult<Vec<TranscriptItem>> {
        Self::items_for_agent_with_restore(connection, agent, true)
    }

    fn items_for_agent_with_restore(
        connection: &Connection,
        agent: &str,
        include_restored: bool,
    ) -> TranscriptStoreResult<Vec<TranscriptItem>> {
        Table::<TranscriptItemRecord>::new(connection)
            .list_matching(
                Some(RecordPredicate::PhysicalAgentEquals(agent)),
                RecordOrder::PhysicalCreatedDescIdDesc,
            )
            .map(|records| {
                records
                    .into_iter()
                    .filter(|record| include_restored || visible(record))
                    .map(|record| {
                        let id = record.id().as_ref().to_owned();
                        TranscriptItem {
                            id,
                            agent: agent.to_owned(),
                            record,
                        }
                    })
                    .collect()
            })
            .map_err(map_store_error)
    }

    pub fn event_text(
        connection: &Connection,
        agent: &str,
        event: &str,
    ) -> TranscriptStoreResult<Option<String>> {
        let text = connection
            .query_row_cached(
                "SELECT text FROM runtime_events WHERE id=?1 AND agent=?2",
                params![event, agent],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| map_column_error(error, "runtime_events", event))?;
        Ok(text.flatten())
    }

    pub fn full_text(
        connection: &Connection,
        item: &TranscriptItem,
    ) -> TranscriptStoreResult<Option<String>> {
        Self::full_text_record(connection, &item.id, &item.record)
    }

    /// Legacy raw ingress and portable history share this compatibility view.
    pub fn full_text_raw(
        connection: &Connection,
        id: &str,
        record: &Value,
    ) -> TranscriptStoreResult<Option<String>> {
        if let Some(full) = Self::stored_full_text(connection, id)? {
            return Ok(Some(full));
        }
        if let Some(text) = record.get("text") {
            let text = text
                .as_str()
                .ok_or_else(|| invalid_record("runtime_items", id))?;
            if record.get("truncated").and_then(Value::as_bool) != Some(true)
                && (record.get("inputs").is_none() || !text.is_empty())
            {
                return Ok(Some(text.to_owned()));
            }
        }
        Ok(None)
    }

    pub fn full_text_view(
        connection: &Connection,
        id: &str,
        record: &StoredTranscriptView,
    ) -> TranscriptStoreResult<Option<String>> {
        use studio_runtime_schema::queue_command::Field;
        if let Some(full) = Self::stored_full_text(connection, id)? {
            return Ok(Some(full));
        }
        match &record.text {
            Field::Missing => Ok(None),
            Field::Null => Err(invalid_record("runtime_items", id)),
            Field::Value(text)
                if record.truncated != Some(true)
                    && (record.inputs.is_missing() || !text.is_empty()) =>
            {
                Ok(Some(text.clone()))
            }
            Field::Value(_) => Ok(None),
        }
    }

    fn full_text_record(
        connection: &Connection,
        id: &str,
        record: &TranscriptItemRecord,
    ) -> TranscriptStoreResult<Option<String>> {
        if let Some(full) = Self::stored_full_text(connection, id)? {
            return Ok(Some(full));
        }
        match record.text() {
            studio_runtime_schema::transcript_record::TranscriptText::InFulltextMissing => Ok(None),
            studio_runtime_schema::transcript_record::TranscriptText::InFulltextNull => {
                Err(invalid_record("runtime_items", id))
            }
            studio_runtime_schema::transcript_record::TranscriptText::Inline(text) => {
                if record.truncated() != Some(true)
                    && (record.inputs().is_missing() || !text.is_empty())
                {
                    Ok(Some(text.clone()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn stored_full_text(
        connection: &Connection,
        id: &str,
    ) -> TranscriptStoreResult<Option<String>> {
        if let Some(blob) = text_blobs::reference(connection, id)? {
            return Ok(Some(text_blobs::read(connection, &blob.hash)?));
        }
        let full = connection
            .query_row_cached(
                "SELECT body FROM runtime_item_fulltext WHERE id=?1",
                [id],
                |row| row.get::<_, Option<StoredText>>(0),
            )
            .optional()
            .map_err(|error| map_column_error(error, "runtime_item_fulltext", id))?;
        Ok(full.flatten().map(StoredText::into_string))
    }

    /// Persist a transcript row and keep the current search
    /// projections are present in the same transaction.
    pub fn put_item(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        index_search: bool,
    ) -> TranscriptStoreResult<()> {
        if !matches!(
            record.get("role").and_then(Value::as_str),
            Some("assistant" | "user" | "system")
        ) {
            return Err(invalid_record("runtime_items", id));
        }
        Self::put_item_with_search_kind(
            transaction,
            id,
            agent,
            record,
            created,
            text,
            TranscriptSearchKind {
                indexed: index_search,
                kind: None,
            },
        )
    }

    /// Persist an item with the explicit Python-compatible FTS kind used by
    /// callers which reconstruct an item's display title separately.
    pub fn put_item_with_search_kind(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        search: TranscriptSearchKind<'_>,
    ) -> TranscriptStoreResult<()> {
        if !matches!(
            record.get("role").and_then(Value::as_str),
            Some("assistant" | "user" | "system")
        ) {
            return Err(invalid_record("runtime_items", id));
        }
        Self::put_notification_item_with_search_kind(
            transaction,
            id,
            agent,
            record,
            created,
            text,
            search,
        )
    }

    /// Notification tools additionally write Python's `output` transcript role.
    /// The existing text writer keeps its narrower accepted role set.
    pub fn put_notification_item(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        index_search: bool,
    ) -> TranscriptStoreResult<()> {
        Self::put_notification_item_with_search_kind(
            transaction,
            id,
            agent,
            record,
            created,
            text,
            TranscriptSearchKind {
                indexed: index_search,
                kind: None,
            },
        )
    }

    fn put_notification_item_with_search_kind(
        transaction: &Transaction<'_>,
        id: &str,
        agent: &str,
        record: &Value,
        created: f64,
        text: &str,
        search: TranscriptSearchKind<'_>,
    ) -> TranscriptStoreResult<()> {
        if record.get("id").and_then(Value::as_str) != Some(id)
            || record.get("text").and_then(Value::as_str).is_none()
            || record.get("inputs").is_some_and(|inputs| {
                !inputs.as_array().is_some_and(|inputs| {
                    inputs.iter().all(|input| {
                        input.get("id").and_then(Value::as_str).is_some()
                            && input
                                .get("kind")
                                .and_then(Value::as_str)
                                .is_some_and(|kind| {
                                    kind.parse::<studio_runtime_schema::persistence::EventKind>()
                                        .is_ok()
                                })
                            && input.get("text").and_then(Value::as_str).is_some()
                            && input.get("truncated").and_then(Value::as_bool).is_some()
                            && input
                                .get("at")
                                .and_then(Value::as_f64)
                                .is_some_and(f64::is_finite)
                            && input.get("assets").is_some_and(Value::is_array)
                    })
                })
            })
            || !matches!(
                record.get("role").and_then(Value::as_str),
                Some("assistant" | "user" | "system" | "output")
            )
        {
            return Err(invalid_record("runtime_items", id));
        }
        let long = text_blobs::is_long(text);
        let owner = transaction
            .query_row_cached("SELECT agent FROM runtime_items WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if owner.as_deref().is_some_and(|owner| owner != agent) {
            return Err(invalid_record("runtime_items", id));
        }
        transaction.execute_cached(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES(?1,?2,?3,?4) \
             ON CONFLICT(id) DO UPDATE SET record=excluded.record",
            params![
                id,
                agent,
                studio_json::to_string(record).map_err(|_| invalid_record("runtime_items", id))?,
                created
            ],
        )?;
        // A pointer to a blob never outlives the text it was made from. A
        // background pass moves final text to a blob; see `text_blobs`.
        text_blobs::clear_reference(transaction, id)?;
        if long {
            transaction.execute_cached(
                "INSERT INTO runtime_item_fulltext(id,body) VALUES(?1,?2) \
                 ON CONFLICT(id) DO UPDATE SET body=excluded.body",
                params![id, StoredTextValue::compressed(text)],
            )?;
        } else {
            transaction.execute_cached("DELETE FROM runtime_item_fulltext WHERE id=?1", [id])?;
        }
        if search.indexed {
            index_search_item(
                transaction,
                id,
                agent,
                search.kind.unwrap_or_else(|| {
                    if record["role"] == "system" || record["role"] == "output" {
                        record
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("system")
                    } else {
                        record.get("role").and_then(Value::as_str).unwrap_or("")
                    }
                }),
                text,
            )?;
        } else {
            delete_search_item(transaction, id)?;
        }
        Ok(())
    }

    pub fn resolve_item(
        connection: &Connection,
        agent: &str,
        identity: &str,
    ) -> TranscriptStoreResult<Option<TranscriptItem>> {
        if let Some(item) = Self::item(connection, agent, identity)?
            && visible(&item.record)
        {
            return Ok(Some(item));
        }
        for item in Self::items_for_agent(connection, agent)? {
            let Some(inputs) = item.record.inputs().value() else {
                continue;
            };
            for (index, input) in inputs.iter().enumerate() {
                let input_id = input.id();
                let display_id = input_id.map_or_else(
                    || format!("{}:{index}", item.id),
                    |id| format!("{agent}:{id}"),
                );
                if display_id == identity {
                    return Ok(Some(item));
                }
            }
        }
        Ok(None)
    }
}

fn map_store_error(error: crate::StoreError) -> TranscriptStoreError {
    match error {
        crate::StoreError::InvalidStoredObject { table, id } => {
            TranscriptStoreError::InvalidStoredObject { table, id }
        }
        other @ crate::StoreError::StateDirectory(_)
        | other @ crate::StoreError::RequestResult(_)
        | other @ crate::StoreError::Outbox(_)
        | other @ crate::StoreError::AlreadyOwned
        | other @ crate::StoreError::IntermediateReleaseRequired
        | other @ crate::StoreError::MissingDatabase(_)
        | other @ crate::StoreError::UnsafeStateDirectory
        | other @ crate::StoreError::Migration { .. }
        | other @ crate::StoreError::SchemaMismatch(_)
        | other @ crate::StoreError::InvalidSyncObject { .. }
        | other @ crate::StoreError::InvalidSyncObjectField { .. }
        | other @ crate::StoreError::InvalidStoredRecord { .. }
        | other @ crate::StoreError::TranscriptFullTextUnavailable { .. }
        | other @ crate::StoreError::TranscriptInputsIncomplete { .. }
        | other @ crate::StoreError::SchemaObjectMismatch { .. }
        | other @ crate::StoreError::Sqlite(_)
        | other @ crate::StoreError::CommitOutcomeUnknown(_)
        | other @ crate::StoreError::Json(_)
        | other @ crate::StoreError::SchemaValidation(_)
        | other @ crate::StoreError::RecordContract(_)
        | other @ crate::StoreError::WriterPoisoned
        | other @ crate::StoreError::WriterActorInitialization(_)
        | other @ crate::StoreError::WriterActorControl(_)
        | other @ crate::StoreError::CommitActorOutcomeUnknown(_)
        | other @ crate::StoreError::ReadTaskFailed => TranscriptStoreError::Store(other),
    }
}

// rusqlite errors are non-exhaustive; preserve every unrelated database failure.
#[allow(clippy::wildcard_enum_match_arm)]
fn map_column_error(error: rusqlite::Error, table: &'static str, id: &str) -> TranscriptStoreError {
    match error {
        rusqlite::Error::InvalidColumnType(..) | rusqlite::Error::FromSqlConversionFailure(..) => {
            invalid_record(table, id)
        }
        error => TranscriptStoreError::Sqlite(error),
    }
}

fn invalid_record(table: &'static str, id: &str) -> TranscriptStoreError {
    TranscriptStoreError::InvalidStoredObject {
        table,
        id: id.to_owned(),
    }
}

fn visible(record: &TranscriptItemRecord) -> bool {
    record.after_restore().is_none()
}

pub(super) fn stored_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn index_search_item(
    transaction: &Transaction<'_>,
    id: &str,
    agent: &str,
    kind: &str,
    body: &str,
) -> TranscriptStoreResult<()> {
    let phase = search_phase(transaction)?;
    if matches!(
        phase.as_str(),
        "building" | "active" | "dropping" | "complete"
    ) {
        let previous = transaction
            .query_row_cached(
                "SELECT search_rowid FROM runtime_search_next_meta WHERE id=?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let rowid = if let Some(rowid) = previous {
            transaction
                .execute_cached("DELETE FROM runtime_search_next WHERE rowid=?1", [rowid])?;
            transaction.execute_cached(
                "UPDATE runtime_search_next_meta SET agent=?1,kind=?2 WHERE id=?3",
                params![agent, kind, id],
            )?;
            rowid
        } else {
            let rowid: i64 = transaction.query_row_cached(
                "SELECT coalesce(max(search_rowid),0)+1 FROM runtime_search_next_meta",
                [],
                |row| row.get(0),
            )?;
            transaction.execute_cached(
                "INSERT INTO runtime_search_next_meta(id,agent,kind,search_rowid) VALUES(?1,?2,?3,?4)",
                params![id, agent, kind, rowid],
            )?;
            rowid
        };
        transaction.execute_cached(
            "INSERT INTO runtime_search_next(rowid,body) VALUES(?1,?2)",
            params![rowid, body],
        )?;
    }
    Ok(())
}

fn delete_search_item(transaction: &Transaction<'_>, id: &str) -> TranscriptStoreResult<()> {
    let phase = search_phase(transaction)?;
    if matches!(
        phase.as_str(),
        "building" | "active" | "dropping" | "complete"
    ) {
        let rowid = transaction
            .query_row_cached(
                "SELECT search_rowid FROM runtime_search_next_meta WHERE id=?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(rowid) = rowid {
            transaction
                .execute_cached("DELETE FROM runtime_search_next WHERE rowid=?1", [rowid])?;
            transaction.execute_cached("DELETE FROM runtime_search_next_meta WHERE id=?1", [id])?;
        }
    }
    Ok(())
}

fn transcript_pull_write(
    transaction: &Transaction<'_>,
    scope: &str,
    after: i64,
    payload: &Value,
    deleted: bool,
    revision: Option<i64>,
) -> TranscriptStoreResult<Value> {
    // Borrowed ownership prevents materializing a second item tree here.
    let items: &[Value] = payload
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let order = items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect::<Vec<_>>();
    // Filter before cloning: metadata must never copy the items tree.
    let metadata = payload
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.as_str() != "items")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<studio_json::Map<_, _>>();
    let mut current = transcript_current(transaction, scope)?;
    let prior_meta = current.get("@meta");
    let base_missing = prior_meta.is_none();
    let was_deleted = prior_meta.is_some_and(|row| row.2);

    if deleted {
        for (id, (_, hash, was_deleted)) in &current {
            if (id.starts_with("item:") || id == "@order") && !was_deleted {
                SyncDocumentsRepository::put_entity_hash(transaction, scope, id, hash, true)?;
            }
        }
        let empty_hash = canonical_hash(&json!({}));
        SyncDocumentsRepository::put_entity_hash(transaction, scope, "@meta", &empty_hash, true)?;
        if let Some(revision) = revision {
            let hash = canonical_hash(&json!(revision));
            SyncDocumentsRepository::put_entity_hash(
                transaction,
                scope,
                "@revision",
                &hash,
                false,
            )?;
        }
    } else {
        let mut live = std::collections::BTreeSet::new();
        for item in items {
            let Some(id) = item.get("id").and_then(Value::as_str) else {
                return Err(invalid_record("runtime_items", "pull"));
            };
            let entity_id = format!("item:{id}");
            live.insert(entity_id.clone());
            let hash = canonical_hash(item);
            SyncDocumentsRepository::put_entity_hash(transaction, scope, &entity_id, &hash, false)?;
        }
        for (id, (_, hash, was_deleted)) in &current {
            if id.starts_with("item:") && !was_deleted && !live.contains(id) {
                SyncDocumentsRepository::put_entity_hash(transaction, scope, id, hash, true)?;
            }
        }
        let order_hash = canonical_hash(&json!(order));
        SyncDocumentsRepository::put_entity_hash(transaction, scope, "@order", &order_hash, false)?;
        let metadata_hash = canonical_hash(&Value::Object(metadata.clone()));
        SyncDocumentsRepository::put_entity_hash(
            transaction,
            scope,
            "@meta",
            &metadata_hash,
            false,
        )?;
        if let Some(revision) = revision {
            let hash = canonical_hash(&json!(revision));
            SyncDocumentsRepository::put_entity_hash(
                transaction,
                scope,
                "@revision",
                &hash,
                false,
            )?;
        }
    }

    let pruned = canvas_read(transaction).collect(
        &TranscriptPruned::QUERY,
        &TranscriptPruned {
            scope: Some(scope),
            retained: TRANSCRIPT_MAX_TOMBSTONES as i64,
        },
    )?;
    if !pruned.is_empty() {
        for row in &pruned {
            canvas_tx(transaction)
                .partition::<SyncEntities>()
                .delete(&(studio_db_api::migration::encode_key(&(
                    scope.to_owned(),
                    row.entity_id.clone().ok_or(rusqlite::Error::InvalidQuery)?,
                )),))?;
        }
        let identity =
            studio_db_api::migration::encode_key(&(scope.to_owned(), "@floor".to_owned()));
        let previous_floor = studio_db_api::db_query!(
            canvas_read(transaction),
            SyncEntities,
            scalar(SyncEntities::PUBLICATION_SEQ),
            filter(SyncEntities::IDENTITY.eq_ref(&identity))
        )?
        .unwrap_or(0);
        let floor_sequence = previous_floor.max(
            pruned
                .iter()
                .map(|row| row.publication_seq)
                .max()
                .unwrap_or(0),
        );
        SyncDocumentsRepository::put_entity_hash_at_sequence(
            transaction,
            scope,
            "@floor",
            &canonical_hash(&json!(floor_sequence)),
            false,
            floor_sequence,
        )?;
    }

    current = transcript_current(transaction, scope)?;
    let high = current.values().map(|row| row.0).max().unwrap_or(0);
    let floor = current.get("@floor").map_or(0, |row| row.0);
    if !deleted && base_missing {
        SyncDocumentsRepository::put_entity_hash_at_sequence(
            transaction,
            scope,
            "@floor",
            &canonical_hash(&json!(high)),
            false,
            high,
        )?;
    }
    let high = canvas_read(transaction).named(
        &TranscriptHigh::QUERY,
        &TranscriptHigh {
            kind: CanvasPublicationsKind::SyncEntities,
            scope: Some(scope),
        },
    )?;
    let current_revision_hash = current.get("@revision").map(|row| row.1.as_str());
    let revision_hash = revision.map(|revision| canonical_hash(&json!(revision)));
    if let Some(revision_hash) = revision_hash.as_deref()
        && after >= high
        && current_revision_hash == Some(revision_hash)
    {
        let workspace_id = SyncDocumentsRepository::workspace_id(transaction)?;
        return Ok(json!({"workspaceId":workspace_id,"documents":[],"checkpoint":{"seq":high}}));
    }
    let full = after == 0 || base_missing || was_deleted || after < floor;
    let documents = if full {
        let encoded = studio_json::to_string(payload).map_err(crate::StoreError::from)?;
        vec![json!({"id":scope,"payload":encoded,"seq":high,"_deleted":deleted})]
    } else if after < high && deleted {
        vec![json!({"id":scope,"payload":"{}","seq":high,"_deleted":true})]
    } else if after < high {
        let changed = canvas_read(transaction).collect(
            &TranscriptDelta::QUERY,
            &TranscriptDelta {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some(scope),
                after,
            },
        )?;
        let mut changed_items = std::collections::BTreeSet::new();
        let mut removed = Vec::new();
        let mut order_changed = false;
        for row in changed {
            let id = row.entity_id.ok_or(rusqlite::Error::InvalidQuery)?;
            let is_deleted = row.deleted != 0;
            if id == "@order" {
                order_changed = true;
            } else if id.starts_with("item:") {
                let item_id = id.trim_start_matches("item:").to_owned();
                if is_deleted {
                    removed.push(item_id);
                } else {
                    changed_items.insert(item_id);
                }
            }
        }
        let deltas = items
            .iter()
            .filter(|item| {
                item.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| changed_items.contains(id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut delta = Value::Object(metadata);
        delta["delta"] = Value::Bool(true);
        delta["items"] = Value::Array(deltas);
        delta["removed"] = json!(removed);
        if order_changed {
            delta["order"] = json!(order);
        }
        let encoded = studio_json::to_string(&delta).map_err(crate::StoreError::from)?;
        vec![json!({"id":scope,"payload":encoded,"seq":high,"_deleted":false})]
    } else {
        Vec::new()
    };
    let checkpoint = if documents.is_empty() { after } else { high };
    let workspace_id = SyncDocumentsRepository::workspace_id(transaction)?;
    Ok(json!({"workspaceId":workspace_id,"documents":documents,"checkpoint":{"seq":checkpoint}}))
}

// Sync-hash debt: retain Value's canonical projection until a replacement proves identical hashes.
fn canonical_hash(value: &Value) -> String {
    crate::sha256_hex(value.to_string().as_bytes())
}

/// Validate metadata of the private transcript index; @revision may carry a cache.
/// These rows are intentionally excluded from the public sync entity union.
pub(crate) fn transcript_index_storage_error(
    collection: &str,
    id: &str,
    sequence: i64,
    hash: &str,
    payload_is_null: bool,
    deleted: i64,
) -> Option<(&'static str, &'static str)> {
    let valid_scope = collection
        .strip_prefix("transcript:")
        .is_some_and(|agent| !agent.is_empty() && collection.chars().count() < 300);
    if !valid_scope {
        return Some(("/collection", "scope"));
    }
    let valid_id = matches!(id, "@meta" | "@revision" | "@order" | "@floor")
        || id
            .strip_prefix("item:")
            .is_some_and(|item| !item.is_empty());
    if !valid_id {
        return Some(("/id", "syncIndexKey"));
    }
    if sequence < 0 {
        return Some(("/seq", "minimum"));
    }
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Some(("/hash", "format"));
    }
    if !payload_is_null && id != "@revision" {
        return Some(("/payload", "storageRepresentation"));
    }
    if !matches!(deleted, 0 | 1) {
        return Some(("/deleted", "enum"));
    }
    None
}

fn search_phase(connection: &Connection) -> TranscriptStoreResult<String> {
    use crate::metadata_schema::SearchRollout;
    let read = crate::peer_update_settings::canvas_read(connection);
    Ok(studio_db_api::db_query!(
        read,
        SearchRollout,
        scalar(SearchRollout::SEARCH_PHASE),
        filter(SearchRollout::INTEGER_KEY.eq(&1))
    )?
    .map(|phase| phase.as_stored().to_owned())
    .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    fn canonical_transcript_fixture() -> Connection {
        use studio_db_api::Table;
        let db = Connection::open_in_memory().unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::change_schema::Changes::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let text_key = Some("workspace".to_owned());
        let tx = db.unchecked_transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<crate::metadata_schema::SyncIdentity>()
            .put(&crate::metadata_schema::SyncIdentity {
                identity: crate::consolidation_codec::nullable_text_identity(&text_key, 1),
                source_rowid: 1,
                text_key,
            })
            .unwrap();
        tx.commit().unwrap();
        db
    }

    #[test]
    fn canonical_transcript_windows_hash_only_isolation_and_rollback() {
        let mut db = canonical_transcript_fixture();
        let first = json!({"items":[{"id":"a","text":"old"},{"id":"b","text":"keep"}],"title":"t"});
        {
            let tx = db.transaction().unwrap();
            let full =
                TranscriptRepository::pull_payload(&tx, "transcript:a", 0, &first, false, Some(1))
                    .unwrap();
            assert_eq!(full["documents"][0]["payload"], first.to_string());
            let after = full["checkpoint"]["seq"].as_i64().unwrap();
            let unchanged = TranscriptRepository::pull_payload(
                &tx,
                "transcript:a",
                after,
                &first,
                false,
                Some(1),
            )
            .unwrap();
            assert_eq!(unchanged["documents"], json!([]));
            let second = json!({"items":[{"id":"b","text":"new"}],"title":"t"});
            let delta = TranscriptRepository::pull_payload(
                &tx,
                "transcript:a",
                after,
                &second,
                false,
                Some(2),
            )
            .unwrap();
            let payload: Value =
                studio_json::from_str(delta["documents"][0]["payload"].as_str().unwrap()).unwrap();
            assert_eq!(
                payload,
                json!({"delta":true,"items":[{"id":"b","text":"new"}],"removed":["a"],"order":["b"],"title":"t"})
            );
            let other =
                TranscriptRepository::pull_payload(&tx, "transcript:other", 0, &first, false, None)
                    .unwrap();
            assert_eq!(other["documents"][0]["payload"], first.to_string());
            let after = delta["checkpoint"]["seq"].as_i64().unwrap();
            let deleted =
                TranscriptRepository::pull_payload(&tx, "transcript:a", after, &second, true, None)
                    .unwrap();
            assert_eq!(deleted["documents"][0]["_deleted"], true);
        }
        let tx = db.transaction().unwrap();
        let full = TranscriptRepository::pull_payload(&tx, "transcript:a", 7, &first, false, None)
            .unwrap();
        assert_eq!(full["documents"][0]["payload"], first.to_string());
    }

    #[test]
    fn canonical_transcript_retention_advances_floor_and_rolls_back() {
        use crate::repositories::sync_documents::SyncDocumentsRepository as Sync;
        let mut db = canonical_transcript_fixture();
        let tx = db.transaction().unwrap();
        let empty = json!({"items":[]});
        TranscriptRepository::pull_payload(&tx, "transcript:a", 0, &empty, false, None).unwrap();
        let hash = "0".repeat(64);
        for n in 0..super::TRANSCRIPT_MAX_TOMBSTONES + 3 {
            Sync::put_entity_hash_at_sequence(
                &tx,
                "transcript:a",
                &format!("item:{n}"),
                &hash,
                true,
                100 + n as i64,
            )
            .unwrap();
        }
        Sync::put_entity_hash_at_sequence(&tx, "transcript:other", "item:keep", &hash, true, 9_000)
            .unwrap();
        let full = TranscriptRepository::pull_payload(&tx, "transcript:a", 50, &empty, false, None)
            .unwrap();
        assert_eq!(full["documents"][0]["payload"], empty.to_string());
        for n in 0..3 {
            assert!(
                crate::repositories::sync_documents::publication_sequence(
                    &tx,
                    "transcript:a",
                    &format!("item:{n}"),
                    true
                )
                .unwrap()
                .is_none()
            );
        }
        assert_eq!(
            crate::repositories::sync_documents::publication_sequence(
                &tx,
                "transcript:a",
                "@floor",
                false
            )
            .unwrap(),
            Some(102)
        );
        assert!(
            crate::repositories::sync_documents::publication_sequence(
                &tx,
                "transcript:other",
                "item:keep",
                true
            )
            .unwrap()
            .is_some()
        );
        tx.rollback().unwrap();
        assert_eq!(
            crate::repositories::sync_documents::publication_sequence(
                &db,
                "transcript:a",
                "@floor",
                false
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn canonical_transcript_search_phase_preserves_legacy_text_and_missing_id() {
        use crate::metadata_schema::SearchRollout;
        let db = canonical_transcript_fixture();
        assert_eq!(super::search_phase(&db).unwrap(), "");
        for phase in ["active", "waiting_for_space", "", "complete"] {
            let tx = db.unchecked_transaction().unwrap();
            crate::peer_update_settings::canvas_tx(&tx)
                .partition::<SearchRollout>()
                .put(&SearchRollout {
                    identity: studio_db_api::migration::encode_key(&(1_i64,)),
                    source_rowid: 1,
                    integer_key: 1,
                    search_phase: crate::metadata_values::SearchPhase::from_stored(phase.into()),
                    search_cursor: 0,
                    search_updated: 0.0,
                })
                .unwrap();
            assert_eq!(super::search_phase(&db).unwrap(), phase);
            tx.commit().unwrap();
        }
    }

    #[test]
    fn canonical_transcript_delta_uses_original_rowid_for_sequence_ties() {
        use crate::metadata_values::StoredBoolean;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities};
        let mut db = canonical_transcript_fixture();
        let tx = db.transaction().unwrap();
        let empty = json!({"items":[]});
        let full = TranscriptRepository::pull_payload(&tx, "transcript:a", 0, &empty, false, None)
            .unwrap();
        let after = full["checkpoint"]["seq"].as_i64().unwrap();
        // Physical insertion order deliberately disagrees with the original rowid.
        for (id, source_rowid) in [("item:z", 90), ("item:a", 80)] {
            let label = CollectionName::from_stored("transcript:a".into());
            let deleted = StoredBoolean::Valid(true);
            crate::peer_update_settings::canvas_tx(&tx)
                .partition::<SyncEntities>()
                .put(&SyncEntities {
                    identity: studio_db_api::migration::encode_key(&(
                        "transcript:a".to_owned(),
                        id.to_owned(),
                    )),
                    source_rowid,
                    publication_seq: after + 1,
                    deleted,
                    collection_kind: label.kind(),
                    collection_agent: label.agent().cloned(),
                    non_transcript: label.non_transcript(),
                    collection_order: label,
                    entity_id: id.into(),
                    entity_hash: "0".repeat(64),
                    entity_payload: EntityValue::from_stored("transcript:a", id, deleted, None)
                        .unwrap(),
                })
                .unwrap();
        }
        let delta =
            TranscriptRepository::pull_payload(&tx, "transcript:a", after, &empty, false, None)
                .unwrap();
        let payload: Value =
            studio_json::from_str(delta["documents"][0]["payload"].as_str().unwrap()).unwrap();
        assert_eq!(payload["removed"], json!(["a", "z"]));
        for n in 0..super::TRANSCRIPT_MAX_TOMBSTONES - 1 {
            super::SyncDocumentsRepository::put_entity_hash_at_sequence(
                &tx,
                "transcript:a",
                &format!("item:new:{n}"),
                &"0".repeat(64),
                true,
                after + 2 + n as i64,
            )
            .unwrap();
        }
        TranscriptRepository::pull_payload(&tx, "transcript:a", 0, &empty, false, None).unwrap();
        assert!(
            crate::repositories::sync_documents::publication_sequence(
                &tx,
                "transcript:a",
                "item:z",
                true
            )
            .unwrap()
            .is_none()
        );
        assert!(
            crate::repositories::sync_documents::publication_sequence(
                &tx,
                "transcript:a",
                "item:a",
                true
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    fn canonical_transcript_window_and_retention_plans_have_flat_unrelated_history_cost() {
        use super::{
            Canvas, CanvasPublicationsKind, Publications, TranscriptCurrent, TranscriptDelta,
            TranscriptHigh, TranscriptPruned,
        };
        use crate::peer_update_settings::canvas_read;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut measurements = Vec::new();
        for history in [100, 2_000] {
            let mut db = canonical_transcript_fixture();
            let tx = db.transaction().unwrap();
            for (scope, count) in [
                ("transcript:a", super::TRANSCRIPT_MAX_TOMBSTONES + 3),
                ("transcript:other", history),
            ] {
                for n in 0..count {
                    super::SyncDocumentsRepository::put_entity_hash_at_sequence(
                        &tx,
                        scope,
                        &format!("item:{n}"),
                        &"0".repeat(64),
                        true,
                        n as i64 + 1,
                    )
                    .unwrap();
                }
            }
            let current = TranscriptCurrent {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some("transcript:a"),
            };
            let delta = TranscriptDelta {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some("transcript:a"),
                after: 500,
            };
            let high = TranscriptHigh {
                kind: CanvasPublicationsKind::SyncEntities,
                scope: Some("transcript:a"),
            };
            let pruned = TranscriptPruned {
                scope: Some("transcript:a"),
                retained: super::TRANSCRIPT_MAX_TOMBSTONES as i64,
            };
            let sequence_index = studio_db_api::query_index::<Canvas, Publications>(
                &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_SEQUENCE,
            )
            .trim_matches('"');
            let retention_index = studio_db_api::query_index::<Canvas, Publications>(
                &Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED,
            )
            .trim_matches('"');
            for (plan, index) in [
                (
                    crate::query_plan_test_support::explain(
                        &tx,
                        TranscriptCurrent::QUERY.sql(),
                        &current,
                    )
                    .unwrap(),
                    sequence_index,
                ),
                (
                    crate::query_plan_test_support::explain(
                        &tx,
                        TranscriptDelta::QUERY.sql(),
                        &delta,
                    )
                    .unwrap(),
                    sequence_index,
                ),
                (
                    crate::query_plan_test_support::explain(
                        &tx,
                        TranscriptHigh::QUERY.sql(),
                        &high,
                    )
                    .unwrap(),
                    sequence_index,
                ),
                (
                    crate::query_plan_test_support::explain(
                        &tx,
                        TranscriptPruned::QUERY.sql(),
                        &pruned,
                    )
                    .unwrap(),
                    retention_index,
                ),
            ] {
                assert!(
                    plan.iter()
                        .any(|line| line.contains("SEARCH") && line.contains(index)),
                    "{plan:?}"
                );
                eprintln!("transcript history={history} plan={plan:?}");
            }
            let steps = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&steps);
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                canvas_read(&tx)
                    .collect(&TranscriptCurrent::QUERY, &current)
                    .unwrap()
                    .len(),
                super::TRANSCRIPT_MAX_TOMBSTONES + 3
            );
            let current_steps = steps.swap(0, Ordering::Relaxed);
            assert_eq!(
                canvas_read(&tx)
                    .collect(&TranscriptDelta::QUERY, &delta)
                    .unwrap()
                    .len(),
                15
            );
            let delta_steps = steps.swap(0, Ordering::Relaxed);
            assert_eq!(
                canvas_read(&tx)
                    .named(&TranscriptHigh::QUERY, &high)
                    .unwrap(),
                515
            );
            let high_steps = steps.swap(0, Ordering::Relaxed);
            assert_eq!(
                canvas_read(&tx)
                    .collect(&TranscriptPruned::QUERY, &pruned)
                    .unwrap()
                    .len(),
                3
            );
            let retention_steps = steps.load(Ordering::Relaxed);
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            let measured = [current_steps, delta_steps, high_steps, retention_steps];
            eprintln!(
                "transcript history={history} VM steps [current,delta,high,retention]={measured:?}"
            );
            assert!(measured.iter().all(|steps| *steps < 50_000), "{measured:?}");
            measurements.push(measured);
        }
        for (small, large) in measurements[0].iter().zip(measurements[1].iter()) {
            assert!(*large <= small + 100, "{measurements:?}");
        }
    }

    #[test]
    fn canonical_transcript_revision_cache_keeps_hash_sequence_and_rolls_back() {
        let mut db = canonical_transcript_fixture();
        let tx = db.transaction().unwrap();
        let hash = "0".repeat(64);
        super::SyncDocumentsRepository::put_entity_hash(
            &tx,
            "transcript:a",
            "@revision",
            &hash,
            false,
        )
        .unwrap();
        super::SyncDocumentsRepository::put_entity_hash(
            &tx,
            "transcript:other",
            "@revision",
            &hash,
            false,
        )
        .unwrap();
        let high = TranscriptRepository::publication_high(&tx, "transcript:a").unwrap();
        assert_eq!(
            TranscriptRepository::cached_revision_hash(&tx, "transcript:a").unwrap(),
            Some(hash.clone())
        );
        assert!(
            TranscriptRepository::cached_revision_payload(&tx, "transcript:a", &hash)
                .unwrap()
                .is_none()
        );
        TranscriptRepository::cache_revision_payload(&tx, "transcript:a", " {}").unwrap();
        assert_eq!(
            TranscriptRepository::cached_revision_payload(&tx, "transcript:a", &hash)
                .unwrap()
                .as_deref(),
            Some(" {}")
        );
        assert!(
            TranscriptRepository::cached_revision_payload(&tx, "transcript:other", &hash)
                .unwrap()
                .is_none()
        );
        assert!(
            TranscriptRepository::cached_revision_payload(&tx, "transcript:a", &"f".repeat(64))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            TranscriptRepository::publication_high(&tx, "transcript:a").unwrap(),
            high
        );
        assert_eq!(
            TranscriptRepository::cached_revision_hash(&tx, "transcript:a").unwrap(),
            Some(hash)
        );
        tx.rollback().unwrap();
        assert!(
            TranscriptRepository::cached_revision_hash(&db, "transcript:a")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn canonical_transcript_recent_chat_plan_and_flat_unrelated_changes_cost() {
        use crate::agent_tables::AgentsStorageRow;
        use crate::change_schema::{Changes, DurableRows, DurableSource};
        use crate::item_monitor_terminal::ItemsRow;
        use crate::json_record_codec::RawJsonRecord;
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::{Canvas, canvas_tx};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use studio_db_api::Table;
        let mut measurements = Vec::new();
        for history in [100, 2_000] {
            let mut db = canonical_transcript_fixture();
            AgentsStorageRow::descriptor()
                .install_if_absent(&db)
                .unwrap();
            crate::agent_tables::AgentVolatileStorageRow::descriptor()
                .install_if_absent(&db)
                .unwrap();
            ItemsRow::descriptor().install_if_absent(&db).unwrap();
            let tx = db.transaction().unwrap();
            for (n, id) in ["a", "b", "deleted", "empty"].iter().enumerate() {
                let mut record = json!({"id":id,"rootId":id,"epoch":0,"provider":"codex","accountKey":"default","status":"idle","compactions":0});
                if *id == "deleted" {
                    record["deletedAt"] = json!(1.0);
                }
                canvas_tx(&tx)
                    .insert(&AgentsStorageRow {
                        hold_status: studio_runtime_schema::persistence::HoldStatus::Clear,
                        parent_id: None,
                        account_key: String::new(),
                        id: (*id).into(),
                        root_id: None,
                        deleted_at: None,
                        record: RawJsonRecord::from_stored(&tx, id, record.to_string()).unwrap(),
                    })
                    .unwrap();
                if *id != "empty" {
                    canvas_tx(&tx)
                        .insert(&ItemsRow {
                            id: (*id).into(),
                            agent: (*id).into(),
                            created: 1.0,
                            record: RawJsonRecord::from_stored(
                                &tx,
                                id,
                                json!({"id":id,"role":"assistant","text":"body"}).to_string(),
                            )
                            .unwrap(),
                        })
                        .unwrap();
                }
                canvas_tx(&tx)
                    .partition::<DurableRows>()
                    .put(&DurableRows {
                        identity: studio_db_api::migration::encode_key(&(
                            AgentsStorageRow::NAME.to_owned(),
                            (*id).to_owned(),
                        )),
                        source_rowid: n as i64 + 1,
                        source_label: DurableSource::from_stored(AgentsStorageRow::NAME.into()),
                        durable_id: (*id).into(),
                        sequence: 10,
                        deleted: StoredBoolean::Valid(false),
                        deleted_at: 0,
                        scope: None,
                    })
                    .unwrap();
            }
            for n in 0..history {
                let id = format!("other:{n}");
                canvas_tx(&tx)
                    .partition::<DurableRows>()
                    .put(&DurableRows {
                        identity: studio_db_api::migration::encode_key(&(
                            "unrelated".to_owned(),
                            id.clone(),
                        )),
                        source_rowid: 100 + n as i64,
                        source_label: DurableSource::from_stored("unrelated".into()),
                        durable_id: id,
                        sequence: 100 + n as i64,
                        deleted: StoredBoolean::Valid(false),
                        deleted_at: 0,
                        scope: None,
                    })
                    .unwrap();
            }
            let params = super::RecentChatIds {
                label: Some(AgentsStorageRow::NAME),
                limit: 2,
            };
            let plan = crate::query_plan_test_support::explain(
                &tx,
                super::RecentChatIds::QUERY.sql(),
                &params,
            )
            .unwrap();
            let index = studio_db_api::query_index::<Canvas, Changes>(
                &Changes::INDEX_CHANGES_DURABLE_IDENTITY,
            )
            .trim_matches('"');
            assert!(
                plan.iter()
                    .any(|line| line.contains("SEARCH") && line.contains(index)),
                "{plan:?}"
            );
            let steps = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&steps);
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                TranscriptRepository::recent_chat_ids(&tx, 2).unwrap(),
                ["b", "a"]
            );
            let measured = steps.load(Ordering::Relaxed);
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            assert!(measured < 10_000, "{measured}");
            eprintln!("recent chat history={history} VM steps={measured} plan={plan:?}");
            measurements.push(measured);
        }
        assert!(measurements[1] <= measurements[0] + 100, "{measurements:?}");
    }

    #[test]
    fn transcript_revision_reads_only_its_changes_partition_and_rolls_back() {
        use crate::change_schema::{Changes, TranscriptRevisions};
        use studio_db_api::Table;
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        Changes::descriptor().install_if_absent(&db).unwrap();
        assert_eq!(TranscriptRepository::revision(&db, "worker").unwrap(), 0);
        {
            let transaction = db.transaction().unwrap();
            crate::peer_update_settings::canvas_tx(&transaction)
                .partition::<TranscriptRevisions>()
                .put(&TranscriptRevisions {
                    identity: crate::consolidation_codec::nullable_text_identity(
                        &Some("worker".into()),
                        0,
                    ),
                    source_rowid: 17,
                    original_key: Some("worker".into()),
                    revision: 42,
                })
                .unwrap();
            assert_eq!(
                TranscriptRepository::revision(&transaction, "worker").unwrap(),
                42
            );
            assert_eq!(
                TranscriptRepository::revision(&transaction, "other").unwrap(),
                0
            );
        }
        assert_eq!(TranscriptRepository::revision(&db, "worker").unwrap(), 0);
    }

    use crate::text_blobs;
    use rusqlite::Connection;
    use studio_json::Value;
    use studio_json::json;

    use super::{TranscriptRepository, TranscriptStoreError};

    thread_local! {
        static SCHEMA_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn transcript_write_does_not_read_schema() {
        let mut db = crate::test_canvas();
        db.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Stmt(statement, _) = event {
                    let sql = statement.sql().to_ascii_lowercase();
                    if sql.contains("sqlite_master") || sql.contains("sqlite_schema") {
                        SCHEMA_READS.with(|count| count.set(count.get() + 1));
                    }
                }
            }),
        );
        SCHEMA_READS.with(|count| count.set(0));
        let tx = db.transaction().unwrap();
        let record = json!({"id":"agent:x","role":"user","text":"body"});
        TranscriptRepository::put_item(&tx, "agent:x", "agent", &record, 1.0, "body", true)
            .unwrap();
        assert_eq!(SCHEMA_READS.with(std::cell::Cell::get), 0);
    }

    #[test]
    #[ignore]
    fn inspect_transcript_records_on_explicit_copy() {
        let path =
            std::env::var_os("STUDIO_TRANSCRIPT_COPY").expect("explicit disposable database copy");
        let db =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let start = std::time::Instant::now();
        let mut statement = db.prepare("SELECT id,record FROM runtime_items").unwrap();
        let mut rows = statement.query([]).unwrap();
        let mut count = 0usize;
        while let Some(row) = rows.next().unwrap() {
            let id: String = row.get(0).unwrap();
            let raw: String = row.get(1).unwrap();
            let record: crate::TranscriptItemRecord = studio_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("invalid transcript {id}: {error}"));
            assert_eq!(record.id().as_ref(), id);
            count += 1;
        }
        eprintln!(
            "validated {count} transcript records in {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn transcript_writer_requires_inline_text() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_items(id TEXT PRIMARY KEY,agent TEXT NOT NULL,record TEXT NOT NULL,created REAL NOT NULL)").unwrap();
        let tx = db.transaction().unwrap();
        for record in [
            json!({"id":"agent:x","role":"user"}),
            json!({"id":"agent:x","role":"user","text":null}),
        ] {
            assert!(
                TranscriptRepository::put_item(
                    &tx, "agent:x", "agent", &record, 1.0, "body", false
                )
                .is_err()
            );
        }
    }

    #[test]
    fn transcript_writer_requires_complete_input_fields() {
        let mut db = crate::test_canvas();
        let tx = db.transaction().unwrap();
        for field in ["id", "kind", "text", "truncated", "at", "assets"] {
            let mut input = json!({"id":"i","kind":"user","text":"body","truncated":false,"at":1.0,"assets":[]});
            input.as_object_mut().unwrap().remove(field);
            let record = json!({"id":"agent:x","role":"user","text":"body","inputs":[input]});
            assert!(
                TranscriptRepository::put_item(
                    &tx, "agent:x", "agent", &record, 1.0, "body", false
                )
                .is_err(),
                "{field}"
            );
        }
        let complete = json!({"id":"agent:x","role":"user","text":"body","inputs":[{"id":"i","kind":"user","text":"body","truncated":false,"at":1.0,"assets":[]}]});
        TranscriptRepository::put_item(&tx, "agent:x", "agent", &complete, 1.0, "body", false)
            .expect("complete input writes");
    }

    #[test]
    fn transcript_typed_update_hydrates_legacy_text_and_preserves_opaque_bytes() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_items(id TEXT PRIMARY KEY,agent TEXT NOT NULL,record TEXT NOT NULL,created REAL NOT NULL); CREATE TABLE runtime_item_fulltext(id TEXT PRIMARY KEY,body TEXT NOT NULL);").unwrap();
        let raw =
            r#"{"id":"agent:x","role":"assistant","truncated":true,"provider":{"z":  1,"a":2}}"#;
        db.execute(
            "INSERT INTO runtime_items VALUES('agent:x','agent',?1,1)",
            [raw],
        )
        .unwrap();
        let table = crate::Table::<crate::TranscriptItemRecord>::new(&db);
        let record = table
            .get(&crate::TranscriptItemId::from("agent:x"))
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                table.update(&record),
                Err(crate::StoreError::TranscriptFullTextUnavailable { .. })
            ),
            "missing fulltext must fail with a typed error"
        );
        let unchanged: String = db
            .query_row("SELECT record FROM runtime_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(unchanged, raw);
        db.execute(
            "INSERT INTO runtime_item_fulltext VALUES('agent:x','hydrated')",
            [],
        )
        .unwrap();
        assert!(table.update(&record).unwrap());
        let encoded: String = db
            .query_row("SELECT record FROM runtime_items", [], |row| row.get(0))
            .unwrap();
        assert!(encoded.contains(r#""text":"hydrated""#), "{encoded}");
        assert!(encoded.contains(r#"{"z":  1,"a":2}"#), "{encoded}");

        let incomplete = r#"{"id":"agent:y","role":"user","text":"body","inputs":[{"id":"i"}]}"#;
        db.execute(
            "INSERT INTO runtime_items VALUES('agent:y','agent',?1,1)",
            [incomplete],
        )
        .unwrap();
        let record = table
            .get(&crate::TranscriptItemId::from("agent:y"))
            .unwrap()
            .unwrap();
        assert!(matches!(
            table.update(&record),
            Err(crate::StoreError::TranscriptInputsIncomplete { .. })
        ));
        let unchanged: String = db
            .query_row(
                "SELECT record FROM runtime_items WHERE id='agent:y'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, incomplete);
    }

    #[test]
    fn retired_sqlite_search_keeps_current_rollout_phases() {
        use crate::metadata_schema::SearchRollout;
        let mut db = crate::fresh_state::test_canvas();
        for phase in [
            "building",
            "active",
            "dropping",
            "complete",
            "waiting_for_space",
        ] {
            let tx = db.transaction().expect("transaction");
            crate::peer_update_settings::canvas_tx(&tx)
                .partition::<SearchRollout>()
                .put(&SearchRollout {
                    identity: studio_db_api::migration::encode_key(&(1_i64,)),
                    source_rowid: 1,
                    integer_key: 1,
                    search_phase: crate::metadata_values::SearchPhase::from_stored(phase.into()),
                    search_cursor: 0,
                    search_updated: 0.0,
                })
                .expect("phase");
            let record = json!({"id":"item", "role":"assistant", "text":"searchable sentinel"});
            TranscriptRepository::put_item(
                &tx,
                "item",
                "agent",
                &record,
                1.0,
                "searchable sentinel",
                true,
            )
            .expect("write");
            let hits = TranscriptRepository::search_hits(&tx, "sentinel").expect("search");
            assert_eq!(
                hits.len(),
                usize::from(matches!(phase, "active" | "dropping" | "complete")),
                "{phase}"
            );
            if phase != "waiting_for_space" {
                TranscriptRepository::put_item(&tx, "item", "agent", &record, 1.0, "", false)
                    .expect("unindex");
                let count: i64 = tx
                    .query_row("SELECT count(*) FROM runtime_search_next_meta", [], |row| {
                        row.get(0)
                    })
                    .expect("metadata");
                assert_eq!(count, 0, "{phase}");
            }
            tx.commit().expect("commit");
        }
        let tx = db.transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .partition::<SearchRollout>()
            .delete(&(studio_db_api::migration::encode_key(&(1_i64,)),))
            .expect("absent phase");
        tx.commit().unwrap();
        assert!(
            TranscriptRepository::search_hits(&db, "sentinel")
                .expect("search")
                .is_empty()
        );
    }

    #[test]
    fn event_text_rejects_a_non_text_column_with_its_table_and_id() {
        let connection = Connection::open_in_memory().expect("in-memory database opens");
        connection
            .execute_batch("CREATE TABLE runtime_events (id TEXT, agent TEXT, text); INSERT INTO runtime_events VALUES ('e','a',x'01');")
            .expect("event fixture is stored");

        assert!(matches!(
            TranscriptRepository::event_text(&connection, "a", "e"),
            Err(TranscriptStoreError::InvalidStoredObject { table: "runtime_events", id }) if id == "e"
        ));
    }

    #[test]
    fn agent_read_returns_checked_record() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE runtime_agents(id TEXT, record TEXT)")
            .unwrap();
        let record: Option<crate::AgentRecord> =
            TranscriptRepository::agent(&connection, "missing").unwrap();
        assert!(record.is_none());

        #[derive(studio_db_api::DbQuery)]
        #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
            cardinality = none, result = (), tables(agents = crate::agent_tables::AgentsStorageRow),
            columns(id = crate::agent_tables::AgentsStorageRow::ID, record = crate::agent_tables::AgentsStorageRow::RECORD),
            sql = "INSERT INTO {agents}({id},{record}) VALUES('owner','[\"owner\",0]')")]
        struct DeletionArray;
        #[derive(studio_db_api::DbQuery)]
        #[db_query(database = crate::peer_update_settings::Canvas, mode = write,
            cardinality = none, result = (), tables(agents = crate::agent_tables::AgentsStorageRow),
            columns(record = crate::agent_tables::AgentsStorageRow::RECORD),
            sql = "UPDATE {agents} SET {record}='[\"owner\",\"owner\",1,\"idle\",0,null,\"default\",null,true,null,null,null,null,null,null,null,null]'")]
        struct OwnerArray;

        let tx = connection.transaction().unwrap();
        let unavailable = |error: crate::StoreError| {
            assert!(matches!(error, crate::StoreError::InvalidSyncObjectField {
                scope, id, pointer, reason
            } if scope == "runtime_agents" && id == "owner" && pointer.is_empty() && reason.starts_with("record_decode;")));
        };
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&DeletionArray::QUERY, &DeletionArray)
            .unwrap();
        unavailable(crate::AgentRepository::deletion_status(&tx, "owner").unwrap_err());
        crate::peer_update_settings::canvas_tx(&tx)
            .named(&OwnerArray::QUERY, &OwnerArray)
            .unwrap();
        unavailable(crate::RulesRepository::owner(&tx, "owner").err().unwrap());
    }

    #[test]
    fn writers_reject_invalid_inputs_and_checked_reads_enforce_shell() {
        let mut db = crate::test_canvas();
        let tx = db.transaction().unwrap();
        let raw = json!({"id":"agent:x","role":"user","text":"body","inputs":[null]});
        assert!(
            TranscriptRepository::put_item(&tx, "agent:x", "agent", &raw, 1.0, "body", false)
                .is_err()
        );
        tx.execute(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES('agent:x','agent',?1,1.0)",
            [raw.to_string()],
        )
        .unwrap();
        assert!(TranscriptRepository::item(&tx, "agent", "agent:x").is_err());
        let before: String = tx
            .query_row(
                "SELECT record FROM runtime_items WHERE id='agent:x'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let output = json!({"id":"agent:x","role":"output","text":"replacement"});
        assert!(
            TranscriptRepository::put_item(
                &tx,
                "agent:x",
                "agent",
                &output,
                2.0,
                "replacement",
                false
            )
            .is_err()
        );
        assert!(
            TranscriptRepository::put_notification_item(
                &tx,
                "agent:x",
                "other",
                &output,
                2.0,
                "replacement",
                false
            )
            .is_err()
        );
        let after: String = tx
            .query_row(
                "SELECT record FROM runtime_items WHERE id='agent:x'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(before, after);
        TranscriptRepository::put_notification_item(
            &tx,
            "agent:x",
            "agent",
            &output,
            2.0,
            "replacement",
            false,
        )
        .unwrap();
        assert_eq!(
            TranscriptRepository::item(&tx, "agent", "agent:x")
                .unwrap()
                .unwrap()
                .record
                .role(),
            studio_runtime_schema::transcript_record::TranscriptRole::Output
        );
        assert!(
            TranscriptRepository::item(&tx, "other", "agent:x")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn typed_input_resolution_preserves_visibility_and_physical_owner() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_items(id TEXT PRIMARY KEY,agent TEXT NOT NULL,record TEXT NOT NULL,created REAL NOT NULL); CREATE TABLE runtime_item_fulltext(id TEXT PRIMARY KEY,body TEXT NOT NULL)").unwrap();
        let tx = db.transaction().unwrap();
        let visible = json!({"id":"agent:x","role":"user","text":"parent","inputs":[{"id":"m","text":"one","future":true},{"id":null,"text":"two"}],"future":{"v":1}});
        tx.execute(
            "INSERT INTO runtime_items(id,agent,record,created) VALUES('agent:x','agent',?1,1.0)",
            [visible.to_string()],
        )
        .unwrap();
        for id in ["agent:x", "agent:m", "agent:x:1"] {
            let row = TranscriptRepository::resolve_item(&tx, "agent", id)
                .unwrap()
                .unwrap();
            assert_eq!(row.id, "agent:x");
            assert_eq!(row.record.into_value(), visible);
        }
        assert!(
            TranscriptRepository::resolve_item(&tx, "other", "agent:m")
                .unwrap()
                .is_none()
        );
        tx.execute("UPDATE runtime_items SET record=json_set(record,'$.afterRestore',json('[]')) WHERE id='agent:x'",[]).unwrap();
        assert!(TranscriptRepository::resolve_item(&tx, "agent", "agent:x").is_err());
        tx.execute("UPDATE runtime_items SET record=json_set(record,'$.afterRestore','restored') WHERE id='agent:x'",[]).unwrap();
        assert!(
            TranscriptRepository::resolve_item(&tx, "agent", "agent:x")
                .unwrap()
                .is_none()
        );
        assert!(
            TranscriptRepository::resolve_item(&tx, "agent", "agent:m")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            TranscriptRepository::items_for_agent_including_restored(&tx, "agent")
                .unwrap()
                .len(),
            1
        );
    }

    fn fulltext_fixture(body: rusqlite::types::Value) -> (Connection, super::TranscriptItem) {
        let connection = Connection::open_in_memory().expect("in-memory database opens");
        connection
            .execute_batch(
                "CREATE TABLE runtime_item_fulltext(id TEXT PRIMARY KEY,body TEXT NOT NULL); CREATE TABLE runtime_item_blobs(id TEXT PRIMARY KEY,hash TEXT NOT NULL,size INTEGER NOT NULL);",
            )
            .expect("fulltext table creates");
        connection
            .execute(
                "INSERT INTO runtime_item_fulltext(id,body) VALUES('agent:long',?1)",
                [body],
            )
            .expect("fulltext fixture inserts");
        let item = super::TranscriptItem {
            id: "agent:long".to_owned(),
            agent: "agent".to_owned(),
            record: studio_json::from_str(
                r#"{"id":"agent:long","role":"assistant","text":"preview","truncated":true}"#,
            )
            .unwrap(),
        };
        (connection, item)
    }

    /// Stored format 1 is a version byte and one zstd frame with its content
    /// size and checksum. This golden value pins the bytes already on disk.
    const STORED_FORMAT_1_GOLDEN: &str =
        "0128b52ffd241de9000053746f72656420666f726d61742076313a2068c3a96c6c6f20e29c930afcd5cb80";

    fn golden_blob() -> Vec<u8> {
        data_encoding::HEXLOWER
            .decode(STORED_FORMAT_1_GOLDEN.as_bytes())
            .expect("golden hex")
    }

    #[test]
    fn legacy_stored_view_accepts_missing_role_and_id() {
        let view: super::StoredTranscriptView =
            studio_json::from_str(r#"{"text":"legacy","turnId":7}"#).unwrap();
        assert!(view.id.is_none());
        assert!(view.role.is_none());
    }

    #[test]
    fn legacy_stored_view_accepts_unknown_role() {
        let view: super::StoredTranscriptView =
            studio_json::from_str(r#"{"role":"future","text":"legacy"}"#).unwrap();
        assert_eq!(view.role.as_deref(), Some("future"));
    }

    #[test]
    fn legacy_stored_view_accepts_nullable_text_and_partial_inputs() {
        let view: super::StoredTranscriptView = studio_json::from_str(
            r#"{"text":null,"inputs":[{"id":"event","text":"preview","truncated":true}]}"#,
        )
        .unwrap();
        assert!(matches!(
            view.text,
            studio_runtime_schema::queue_command::Field::Null
        ));
        assert_eq!(view.inputs.value().unwrap()[0].id.as_deref(), Some("event"));
    }

    #[test]
    fn legacy_stored_view_accepts_restore_boolean() {
        let view: super::StoredTranscriptView =
            studio_json::from_str(r#"{"afterRestore":true}"#).unwrap();
        assert!(view.after_restore());
    }

    #[test]
    fn legacy_stored_view_accepts_restore_empty_array() {
        let view: super::StoredTranscriptView =
            studio_json::from_str(r#"{"afterRestore":[]}"#).unwrap();
        assert!(!view.after_restore());
    }

    #[test]
    fn legacy_stored_view_accepts_restore_empty_object() {
        let view: super::StoredTranscriptView =
            studio_json::from_str(r#"{"afterRestore":{}}"#).unwrap();
        assert!(!view.after_restore());
    }

    #[test]
    fn raw_full_text_reads_legacy_roles_and_fulltext_null() {
        let (connection, _) = fulltext_fixture(rusqlite::types::Value::Text("full".into()));
        for item in [
            json!({"text":"inline","role":"future","turnId":7}),
            json!({"text":"inline"}),
        ] {
            assert_eq!(
                TranscriptRepository::full_text_raw(&connection, "other", &item)
                    .unwrap()
                    .as_deref(),
                Some("inline")
            );
        }
        let item = json!({"text":null, "afterRestore":[], "inputs":[{"id":"legacy"}]});
        assert_eq!(
            TranscriptRepository::full_text_raw(&connection, "agent:long", &item)
                .unwrap()
                .as_deref(),
            Some("full")
        );
    }

    #[test]
    fn full_text_reads_a_plain_text_body() {
        let (connection, item) =
            fulltext_fixture(rusqlite::types::Value::Text("plain body".to_owned()));
        assert_eq!(
            TranscriptRepository::full_text(&connection, &item)
                .expect("plain body reads")
                .as_deref(),
            Some("plain body")
        );
    }

    #[test]
    fn full_text_reads_a_compressed_body_in_stored_format_1() {
        let (connection, item) = fulltext_fixture(rusqlite::types::Value::Blob(golden_blob()));
        assert_eq!(
            TranscriptRepository::full_text(&connection, &item)
                .expect("compressed body reads")
                .as_deref(),
            Some("Stored format v1: h\u{e9}llo \u{2713}\n")
        );
    }

    #[test]
    fn full_text_refuses_a_damaged_compressed_body_with_its_table_and_id() {
        let mut damaged = golden_blob();
        let last = damaged.len() - 1;
        damaged[last] ^= 0xff;
        for body in [damaged, vec![0x02, 0x28, 0xb5, 0x2f, 0xfd], Vec::new()] {
            let (connection, item) = fulltext_fixture(rusqlite::types::Value::Blob(body));
            assert!(matches!(
                TranscriptRepository::full_text(&connection, &item),
                Err(TranscriptStoreError::InvalidStoredObject { table: "runtime_item_fulltext", id })
                    if id == "agent:long"
            ));
        }
    }

    /// A file database with the transcript tables and the blob table.
    fn blob_fixture(directory: &std::path::Path) -> Connection {
        use studio_db_api::Table;
        let connection = Connection::open(directory.join("canvas.sqlite3")).expect("database");
        connection
            .execute_batch(
                "CREATE TABLE runtime_items(id TEXT PRIMARY KEY, agent TEXT NOT NULL, \
                 record TEXT NOT NULL, created REAL NOT NULL);
                 CREATE TABLE runtime_item_fulltext(id TEXT PRIMARY KEY, body TEXT NOT NULL);
                 CREATE TABLE runtime_item_blobs(id TEXT PRIMARY KEY, hash TEXT NOT NULL, size INTEGER NOT NULL);
                 CREATE TABLE runtime_search_rollout(id INTEGER PRIMARY KEY, phase TEXT NOT NULL);",
            )
            .expect("transcript tables create");
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&connection)
            .unwrap();
        connection
    }

    fn long_text() -> String {
        text_blobs::incompressible_text(40_000)
    }

    fn long_record(streaming: bool) -> Value {
        json!({"id":"agent:long","role":"assistant","text":"preview","truncated":true,"streaming":streaming})
    }

    fn put_long(connection: &Connection, text: &str, streaming: bool) -> Value {
        let record = long_record(streaming);
        let transaction = connection.unchecked_transaction().expect("transaction");
        TranscriptRepository::put_item(
            &transaction,
            "agent:long",
            "agent",
            &record,
            1.0,
            text,
            false,
        )
        .expect("item writes");
        transaction.commit().expect("commit");
        record
    }

    /// What the background pass does: write the file, then commit the pointer.
    fn move_to_blob(state: &std::path::Path, text: &str) -> text_blobs::MovedText {
        let (blob, bytes) = text_blobs::encode_blob(text);
        let path = text_blobs::layout(&state.join("blobs"), &blob.hash);
        std::fs::create_dir_all(path.parent().unwrap()).expect("directories");
        std::fs::write(&path, bytes).expect("blob file");
        text_blobs::MovedText {
            id: "agent:long".to_owned(),
            digest: text_blobs::digest(text),
            blob,
        }
    }

    fn commit_moved(connection: &mut Connection, moved: &text_blobs::MovedText) -> usize {
        let transaction = connection.transaction().expect("transaction");
        let set = text_blobs::commit(&transaction, std::slice::from_ref(moved)).expect("commit");
        transaction.commit().expect("commit");
        set
    }

    #[test]
    fn long_item_text_is_staged_in_the_table_and_moves_to_a_blob_when_final() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        let record = put_long(&connection, &text, false);
        assert!(
            !state.path().join("blobs").exists(),
            "a writer transaction does no file I/O"
        );

        let pending = text_blobs::pending(&connection, 10).expect("pending");
        assert_eq!(pending, vec![("agent:long".to_owned(), text.clone())]);
        let moved = move_to_blob(state.path(), &text);
        assert_eq!(commit_moved(&mut connection, &moved), 1);
        assert!(text_blobs::pending(&connection, 10).unwrap().is_empty());
        assert!(
            text_blobs::fulltext_without_blob(&connection)
                .unwrap()
                .is_empty()
        );
        let blob = text_blobs::reference(&connection, "agent:long")
            .unwrap()
            .unwrap();
        assert_eq!(blob.size, text.len() as u64);
        let item = super::TranscriptItem {
            id: "agent:long".to_owned(),
            agent: "agent".to_owned(),
            record: studio_json::from_str(&record.to_string()).unwrap(),
        };
        assert_eq!(
            TranscriptRepository::full_text(&connection, &item)
                .unwrap()
                .as_deref(),
            Some(text.as_str())
        );
        assert_eq!(text_blobs::read(&connection, &blob.hash).unwrap(), text);
    }

    #[test]
    fn streaming_item_is_not_moved_and_a_rewrite_clears_the_pointer() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        put_long(&connection, &text, true);
        assert!(
            text_blobs::pending(&connection, 10).unwrap().is_empty(),
            "streaming text is not final"
        );

        put_long(&connection, &text, false);
        let moved = move_to_blob(state.path(), &text);
        assert_eq!(commit_moved(&mut connection, &moved), 1);
        let longer = format!("{text}more");
        put_long(&connection, &longer, false);
        assert!(
            text_blobs::reference(&connection, "agent:long")
                .unwrap()
                .is_none()
        );
        let item = super::TranscriptItem {
            id: "agent:long".to_owned(),
            agent: "agent".to_owned(),
            record: studio_json::from_str(&long_record(false).to_string()).unwrap(),
        };
        assert_eq!(
            TranscriptRepository::full_text(&connection, &item)
                .unwrap()
                .as_deref(),
            Some(longer.as_str()),
            "the rewrite is read, not the old blob"
        );
    }

    #[test]
    fn commit_never_points_at_a_missing_blob_or_a_changed_text() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        put_long(&connection, &text, false);
        let (blob, _bytes) = text_blobs::encode_blob(&text);
        let missing = text_blobs::MovedText {
            id: "agent:long".to_owned(),
            digest: text_blobs::digest(&text),
            blob,
        };
        assert_eq!(
            commit_moved(&mut connection, &missing),
            0,
            "no file, no row"
        );

        let moved = move_to_blob(state.path(), &text);
        put_long(&connection, &format!("{text}changed"), false);
        assert_eq!(
            commit_moved(&mut connection, &moved),
            0,
            "the text changed after the blob"
        );
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM runtime_item_blobs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);
    }

    fn sweep_once(
        connection: &mut Connection,
        state: &std::path::Path,
        later: std::time::SystemTime,
    ) -> usize {
        let root = state.join("blobs");
        let pointed = text_blobs::pointed_hashes(connection).unwrap();
        let names = text_blobs::quarantine(&root, &pointed, later).unwrap();
        let transaction = connection.transaction().unwrap();
        let keep = text_blobs::referenced_among(&transaction, &names).unwrap();
        transaction.commit().unwrap();
        text_blobs::release(&root, &keep, &names).unwrap()
    }

    #[test]
    fn sweep_deletes_old_unreferenced_text_blobs_only() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        put_long(&connection, &text, false);
        let orphan = move_to_blob(state.path(), &text);
        let orphan_file = text_blobs::layout(&state.path().join("blobs"), &orphan.blob.hash);
        // A payload blob is plain JSON and is never collected.
        let payload_hash = "ab".repeat(32);
        let payload = text_blobs::layout(&state.path().join("blobs"), &payload_hash);
        std::fs::create_dir_all(payload.parent().unwrap()).unwrap();
        std::fs::write(&payload, b"{\"contentItems\":[]}").unwrap();

        let soon = std::time::SystemTime::now();
        assert_eq!(
            sweep_once(&mut connection, state.path(), soon),
            0,
            "too young"
        );
        assert!(orphan_file.is_file());
        let later = soon + text_blobs::TEXT_BLOB_SWEEP_DELAY * 2;
        assert_eq!(sweep_once(&mut connection, state.path(), later), 1);
        assert!(!orphan_file.exists());
        assert!(payload.is_file(), "payload blobs are not collected");
    }

    #[test]
    fn sweep_never_deletes_a_referenced_blob_even_when_it_races_a_commit() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        put_long(&connection, &text, false);
        let moved = move_to_blob(state.path(), &text);
        let root = state.path().join("blobs");
        let far = std::time::SystemTime::now() + text_blobs::TEXT_BLOB_SWEEP_DELAY * 1000;

        // Referenced: the sweep leaves the file where it is.
        assert_eq!(commit_moved(&mut connection, &moved), 1);
        assert_eq!(sweep_once(&mut connection, state.path(), far), 0);
        let file = text_blobs::layout(&root, &moved.blob.hash);
        assert!(file.is_file());

        // The pointer appears after the quarantine: the second read restores the file.
        connection
            .execute("DELETE FROM runtime_item_blobs", [])
            .unwrap();
        let names = text_blobs::quarantine(&root, &Default::default(), far).unwrap();
        assert_eq!(names, vec![moved.blob.hash.clone()]);
        assert_eq!(
            commit_moved(&mut connection, &moved),
            0,
            "the file is in the trash, so no pointer"
        );
        connection
            .execute(
                "INSERT INTO runtime_item_blobs(id,hash,size) VALUES('agent:long',?1,1)",
                [&moved.blob.hash],
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        let keep = text_blobs::referenced_among(&transaction, &names).unwrap();
        transaction.commit().unwrap();
        assert_eq!(text_blobs::release(&root, &keep, &names).unwrap(), 0);
        assert!(
            file.is_file(),
            "a referenced blob is restored, never deleted"
        );
    }

    #[test]
    fn fulltext_check_reports_rows_without_a_blob_and_the_table_keeps_its_rows() {
        let state = tempfile::tempdir().expect("state");
        let mut connection = blob_fixture(state.path());
        let text = long_text();
        put_long(&connection, &text, false);
        assert_eq!(
            text_blobs::fulltext_without_blob(&connection).unwrap(),
            vec!["agent:long".to_owned()]
        );
        let moved = move_to_blob(state.path(), &text);
        assert_eq!(commit_moved(&mut connection, &moved), 1);
        assert!(
            text_blobs::fulltext_without_blob(&connection)
                .unwrap()
                .is_empty()
        );
        let kept: i64 = connection
            .query_row("SELECT count(*) FROM runtime_item_fulltext", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(kept, 1, "the legacy table keeps its rows");
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "transcript",
        requirements: &REQUIREMENTS,
    }
}
