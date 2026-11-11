//! Durable per-account usage observations and their compact sync projections.

use rusqlite::{Connection, OptionalExtension};

use crate::peer_update_settings::{Canvas, canvas_read};
use crate::publication_schema::Publications;
use crate::receipt_tables::AccountStateRow;
use crate::records::{RecordOrder, RecordPredicate, Table, UsageResumeRecord};
use crate::schema::{SchemaRequirements, TableRequirement};
use crate::{StoreError, StoreResult, Transaction};
use studio_runtime_schema::persistence::ProviderScope;

#[derive(studio_db_api::DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,
    result=(String,Option<Vec<u8>>,Option<i64>,Option<Vec<u8>>,Option<i64>),
    tables(rows=Publications,accounts=AccountStateRow),
    indexes(rows=Publications::INDEX_PUBLICATIONS_ENTITY_COLLECTION_DELETED),predicates(entities=rows),
    columns(id=Publications::ENTITY_ID,payload=Publications::ENTITY_PAYLOAD,
        collection=Publications::COLLECTION_ORDER,deleted=Publications::DELETED,
        account=AccountStateRow::ACCOUNT,receipt=AccountStateRow::LIMITS_OBSERVATION,rowid=Publications::SOURCE_ROWID),
    sql="SELECT {id},substr(CAST({payload} AS BLOB),1,?1),length(CAST({payload} AS BLOB)),substr(CAST(a.{receipt} AS BLOB),1,?1),length(CAST(a.{receipt} AS BLOB)) FROM {rows} LEFT JOIN {accounts} a ON a.{account}={id} WHERE {entities} AND {collection}='rateLimit' AND {deleted}=0 ORDER BY {rowid}")]
struct LimitsSnapshots {
    #[db(column=Publications::PUBLICATION_SEQ)]
    prefix_bytes: i64,
}

pub const MAX_PERSISTED_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PERSISTED_DOCUMENT_DEPTH: usize = 128;
/// Current Claude account sign-in refusal, also retained by the retired auth-wait migration.
pub const CLAUDE_SIGN_IN_MESSAGE: &str = "Sign in with claude auth login first";
pub const AUTH_WAIT_NOTICE_SECONDS: f64 = 1800.0;
pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    conditional_tables: &[],
    tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[
        TableRequirement {
            name: <crate::publication_schema::Publications as studio_db_api::Table>::NAME,
            columns: &[
                crate::publication_schema::Publications::__DB_COLUMN_KIND,
                crate::publication_schema::Publications::__DB_COLUMN_IDENTITY,
                crate::publication_schema::Publications::__DB_COLUMN_COLLECTION_ORDER,
                crate::publication_schema::Publications::__DB_COLUMN_ENTITY_ID,
                crate::publication_schema::Publications::__DB_COLUMN_ENTITY_PAYLOAD,
                crate::publication_schema::Publications::__DB_COLUMN_DELETED,
            ],
        },
        TableRequirement {
            name: "runtime_account_state",
            columns: &[
                "account",
                "next_generation",
                "connection_generation",
                "journal_ack",
                "recovery_required",
                "streams",
                "limits_observation",
            ],
        },
    ],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

pub struct LimitsRead {
    pub snapshots: Vec<(
        String,
        studio_runtime_schema::usage_limits::UsageLimitsResponse,
    )>,
    pub diagnostics: Vec<LimitsReadDiagnostic>,
}

pub struct LimitsReadDiagnostic {
    pub workspace_row: &'static str,
    pub account_key: String,
    pub pointer: String,
    pub keyword: String,
}

/// Freshness fields stored privately between content changes. Unit: Unix seconds.
pub const LIMITS_OBSERVATION_TIME_FIELDS: &[&str] = &["at", "readAt", "checkedAt", "processedAt"];

/// Durable provider cursor and observation times; absent and null times are equivalent.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitsStamp {
    pub scope: Option<studio_runtime_schema::provider_call_contract::ConnectionScope>,
    #[serde(default)]
    pub cursor: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_at: Option<f64>,
}

impl LimitsStamp {
    pub fn read(raw: Option<String>) -> StoreResult<Self> {
        Ok(raw
            .map(|raw| studio_json::from_str::<Option<Self>>(&raw))
            .transpose()?
            .flatten()
            .unwrap_or_default())
    }

    pub fn set_times(
        &mut self,
        response: &studio_runtime_schema::usage_limits::UsageLimitsResponse,
    ) {
        self.at = response.at;
        self.read_at = response.read_at;
        self.checked_at = response.checked_at;
        self.processed_at = response.processed_at;
    }

    pub fn apply(&self, response: &mut studio_runtime_schema::usage_limits::UsageLimitsResponse) {
        if self.at.is_some()
            || self.read_at.is_some()
            || self.checked_at.is_some()
            || self.processed_at.is_some()
        {
            response.at = self.at;
            response.read_at = self.read_at;
            response.checked_at = self.checked_at;
            response.processed_at = self.processed_at;
        }
    }
}

/// Decode one account's stored limits, isolating unreadable and foreign observations.
pub fn normalized_limits_view(
    account: &str,
    bytes: &[u8],
    now: f64,
) -> (studio_runtime_schema::usage_limits::RateLimitView, bool) {
    use studio_runtime_schema::usage_limits::{RateLimitView, UsageLimitsResponse};
    let parsed = (|| {
        if bytes.len() > MAX_PERSISTED_DOCUMENT_BYTES || !persisted_document_within_bounds(bytes) {
            return None;
        }
        #[derive(serde::Deserialize)]
        struct Entity<'a> {
            #[serde(borrow)]
            value: studio_json::RawJson<'a>,
        }
        #[derive(serde::Deserialize)]
        struct Projected<'a> {
            #[serde(borrow)]
            observation: studio_json::RawJson<'a>,
        }
        #[derive(serde::Deserialize)]
        struct Legacy {
            #[serde(default, rename = "eligibility")]
            _eligibility: Option<serde::de::IgnoredAny>,
            #[serde(flatten)]
            response: UsageLimitsResponse,
        }
        let entity: Entity<'_> = studio_json::from_slice(bytes).ok()?;
        if let Ok(view) = studio_json::from_str::<RateLimitView>(entity.value.as_str())
            && view.observation.account_key == account
        {
            return Some(view);
        }
        let value: Legacy = match studio_json::from_str::<Projected<'_>>(entity.value.as_str()) {
            Ok(view) => studio_json::from_str(view.observation.as_str()),
            Err(_) => studio_json::from_str(entity.value.as_str()),
        }
        .ok()?;
        (value.response.account_key == account)
            .then(|| RateLimitView::from_observation(value.response, now))
    })();
    if let Some(view) = parsed {
        return (view, false);
    }
    eprintln!("limits: isolated unreadable account {account}");
    let empty = UsageLimitsResponse {
        account_key: account.to_owned(),
        ..Default::default()
    };
    (RateLimitView::from_observation(empty, now), true)
}

pub fn typed_limits_snapshot(
    connection: &Connection,
    account: &str,
    stamp: &LimitsStamp,
) -> StoreResult<Option<studio_runtime_schema::usage_limits::UsageLimitsResponse>> {
    let raw = super::sync_documents::publication_payload(connection, "rateLimit", account, true)?;
    Ok(raw.map(|raw| {
        let (view, _) = normalized_limits_view(account, raw.as_bytes(), 0.0);
        let mut response = view.observation;
        stamp.apply(&mut response);
        response
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("Provider scope is unavailable")]
    Unavailable,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Current generation remains fenced even when its connection is unknown.
#[derive(Debug)]
pub enum ProviderState {
    Connected { generation: u64, connection: String },
    Held { generation: u64 },
}

impl ProviderState {
    pub fn generation(&self) -> u64 {
        match self {
            Self::Connected { generation, .. } | Self::Held { generation } => *generation,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentScopeFields {
    provider_stream: Option<StreamFields>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamFields {
    generation: Option<u64>,
    connection: Option<String>,
    ack: Option<u64>,
}
impl StreamFields {
    fn into_state(self) -> StoreResult<Option<(ProviderState, u64)>> {
        match (self.generation, self.connection, self.ack) {
            (Some(generation), connection, ack) => {
                Ok(Some((state(generation, connection), ack.unwrap_or(0))))
            }
            (None, None, None) => Ok(None),
            (None, _, _) => Err(StoreError::RecordContract(
                "provider connection or ACK has no generation",
            )),
        }
    }
}

fn agent_scope_fields(agent: &crate::AgentRecord) -> StoreResult<AgentScopeFields> {
    use studio_runtime_schema::queue_command::Field;
    let provider_stream = match &agent.provider_stream {
        Field::Missing | Field::Null => None,
        Field::Value(crate::StoredTransferObject::Known(exact)) => Some(exact.value().clone()),
        Field::Value(crate::StoredTransferObject::Unreadable(raw)) => {
            studio_json::from_str(raw.0.as_str())?
        }
    };
    Ok(AgentScopeFields { provider_stream })
}

fn state(generation: u64, connection: Option<String>) -> ProviderState {
    match connection {
        Some(connection) if !connection.is_empty() => ProviderState::Connected {
            generation,
            connection,
        },
        _ => ProviderState::Held { generation },
    }
}

/// Source owner for the current provider generation and journal ACK.
#[derive(Clone, Copy)]
pub enum ProviderOwner<'a> {
    Account(&'a str),
    Agent(&'a crate::AgentRecord),
}

/// Read the host account row or the VM agent's current stream exactly once.
pub fn provider_state(
    db: &Connection,
    owner: ProviderOwner<'_>,
) -> StoreResult<Option<(ProviderState, u64)>> {
    let account = match owner {
        ProviderOwner::Account(account) => std::borrow::Cow::Borrowed(account),
        ProviderOwner::Agent(agent) => {
            let fields = agent_scope_fields(agent)?;
            if matches!(agent.execution_mode, Some(crate::AgentExecutionMode::Vm)) {
                return fields
                    .provider_stream
                    .map(StreamFields::into_state)
                    .transpose()
                    .map(Option::flatten);
            }
            std::borrow::Cow::Borrowed(agent.account_key.as_str())
        }
    };
    let row: Option<(Option<i64>, Option<String>, i64)> = db.query_row(
        "SELECT connection_generation,connection_id,journal_ack FROM runtime_account_state WHERE account=?1",
        [account.as_ref()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional()?;
    match row {
        None | Some((None, None, _)) => Ok(None),
        Some((None, Some(_), _)) => Err(StoreError::RecordContract(
            "provider connection has no generation",
        )),
        Some((Some(generation), connection, ack)) => Ok(Some((
            state(
                u64::try_from(generation)
                    .map_err(|_| StoreError::RecordContract("provider generation is negative"))?,
                connection,
            ),
            u64::try_from(ack)
                .map_err(|_| StoreError::RecordContract("provider journal ACK is negative"))?,
        ))),
    }
}

/// Read the current host or VM provider identity in the owner's transaction.
pub fn current_provider_scope(
    tx: &Transaction<'_>,
    agent: &crate::AgentRecord,
) -> Result<ProviderScope, ScopeError> {
    match provider_state(tx, ProviderOwner::Agent(agent))? {
        Some((
            ProviderState::Connected {
                generation,
                connection,
            },
            _,
        )) => Ok(ProviderScope {
            generation,
            connection,
        }),
        Some((ProviderState::Held { .. }, _)) | None => Err(ScopeError::Unavailable),
    }
}

/// Initialize only current account state; migrations seed historical generations once.
pub fn ensure_account(tx: &Transaction<'_>, account: &str) -> StoreResult<()> {
    tx.execute("INSERT INTO runtime_account_state(account,next_generation,journal_ack,recovery_required,streams) VALUES(?1,0,0,0,'null') ON CONFLICT(account) DO NOTHING", [account])?;
    Ok(())
}

/// Last allocated direct-provider generation. Increment in the writer transaction.
pub fn allocate_next_generation(tx: &Transaction<'_>, account: &str) -> StoreResult<u64> {
    ensure_account(tx, account)?;
    let generation: i64 = tx.query_row("UPDATE runtime_account_state SET next_generation=next_generation+1 WHERE account=?1 AND next_generation>=0 AND next_generation<?2 RETURNING next_generation", (account, i64::MAX), |row| row.get(0))?;
    Ok(generation as u64)
}

/// Contiguous applied journal prefix for the current host provider generation.
pub fn provider_checkpoint(
    connection: &Connection,
    account: &str,
    generation: u64,
) -> StoreResult<u64> {
    let generation = i64::try_from(generation).map_err(|_| {
        StoreError::RecordContract("provider generation exceeds SQLite integer range")
    })?;
    let cursor: i64 = connection.query_row("SELECT journal_ack FROM runtime_account_state WHERE account=?1 AND connection_generation=?2", rusqlite::params![account,generation], |row| row.get(0)).optional()?.unwrap_or(0);
    u64::try_from(cursor)
        .map_err(|_| StoreError::RecordContract("provider journal ACK is negative"))
}

/// A generation transition is explicit; an ACK can never replace another generation.
pub fn set_provider_checkpoint(
    tx: &Transaction<'_>,
    account: &str,
    generation: u64,
    cursor: u64,
) -> StoreResult<()> {
    let generation = i64::try_from(generation).map_err(|_| {
        StoreError::RecordContract("provider generation exceeds SQLite integer range")
    })?;
    let cursor = i64::try_from(cursor).map_err(|_| {
        StoreError::RecordContract("provider journal ACK exceeds SQLite integer range")
    })?;
    ensure_account(tx, account)?;
    let changed = tx.execute("UPDATE runtime_account_state SET connection_generation=?2,journal_ack=MAX(journal_ack,?3) WHERE account=?1 AND (connection_generation IS NULL OR connection_generation=?2)", rusqlite::params![account,generation,cursor])?;
    if changed == 0 {
        return Err(StoreError::RecordContract(
            "provider checkpoint belongs to a different generation",
        ));
    }
    Ok(())
}

/// Serialized stream metadata owned by the account worker; pending flushes live in outbox.
pub fn streams(connection: &Connection, account: &str) -> StoreResult<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT streams FROM runtime_account_state WHERE account=?1 AND streams!='null'",
            [account],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn put_streams(tx: &Transaction<'_>, account: &str, streams: &str) -> StoreResult<()> {
    ensure_account(tx, account)?;
    tx.execute(
        "UPDATE runtime_account_state SET streams=?2 WHERE account=?1",
        (account, streams),
    )?;
    Ok(())
}

/// Ordered observation metadata without a second copy of the provider snapshot.
pub fn limits_observation(connection: &Connection, account: &str) -> StoreResult<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT limits_observation FROM runtime_account_state WHERE account=?1",
            [account],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

pub fn put_limits_observation(
    tx: &Transaction<'_>,
    account: &str,
    observation: &LimitsStamp,
) -> StoreResult<()> {
    ensure_account(tx, account)?;
    tx.execute(
        "UPDATE runtime_account_state SET limits_observation=?2 WHERE account=?1",
        (account, studio_json::to_string(observation)?),
    )?;
    Ok(())
}

/// Read the freshest durable observation used by resume checks and startup restoration.
pub fn latest_limits_snapshot(
    connection: &Connection,
    account: &str,
) -> StoreResult<Option<studio_runtime_schema::usage_limits::UsageLimitsResponse>> {
    let stamp = LimitsStamp::read(limits_observation(connection, account)?)?;
    typed_limits_snapshot(connection, account, &stamp)
}

/// Move persisted v3 workspace snapshots once, without changing their account identity.
/// Restore bounded, validated per-account observations; invalid accounts remain isolated.
pub fn limits_snapshots(connection: &Connection) -> StoreResult<LimitsRead> {
    let mut result = LimitsRead {
        snapshots: Vec::new(),
        diagnostics: Vec::new(),
    };
    canvas_read(connection).visit(
        &LimitsSnapshots::QUERY,
        &LimitsSnapshots {
            prefix_bytes: MAX_PERSISTED_DOCUMENT_BYTES as i64 + 1,
        },
        |(key, bytes, length, receipt, receipt_length)| {
            let decoded = (|| {
                #[derive(serde::Deserialize)]
                struct Envelope {
                    value: Projected,
                }
                #[derive(serde::Deserialize)]
                struct Projected {
                    observation: studio_runtime_schema::usage_limits::UsageLimitsResponse,
                }
                fn decode<T: serde::de::DeserializeOwned>(
                    bytes: &[u8],
                    length: Option<i64>,
                    prefix: &str,
                ) -> Result<T, (String, String)> {
                    if length.unwrap_or_default() > MAX_PERSISTED_DOCUMENT_BYTES as i64
                        || !persisted_document_within_bounds(bytes)
                    {
                        return Err(("/".into(), "resourceLimit".into()));
                    }
                    let mut deserializer = studio_json::Deserializer::from_slice(bytes);
                    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                        let path = error.path().to_string();
                        let path = path
                            .strip_prefix(prefix)
                            .unwrap_or(&path)
                            .trim_start_matches('.');
                        let first = path.split('.').next().unwrap_or("");
                        let pointer = if first.is_empty() {
                            "/value".into()
                        } else {
                            format!("/{first}")
                        };
                        (pointer, "type".into())
                    })
                }
                let stamp = match receipt {
                    Some(bytes) => decode::<Option<LimitsStamp>>(&bytes, receipt_length, "")?
                        .unwrap_or_default(),
                    None => LimitsStamp::default(),
                };
                let bytes = bytes.ok_or_else(|| ("/value".to_owned(), "type".to_owned()))?;
                let mut response = decode::<Envelope>(&bytes, length, "value.observation")?
                    .value
                    .observation;
                if response.account_key != key {
                    return Err(("/accountKey".into(), "identity".into()));
                }
                stamp.apply(&mut response);
                Ok(response)
            })();
            let (snapshot, invalid) = match decoded {
                Ok(snapshot) => (Some(snapshot), None),
                Err(error) => (None, Some(error)),
            };
            if let Some((pointer, keyword)) = invalid {
                result.diagnostics.push(LimitsReadDiagnostic {
                    workspace_row: "rateLimit",
                    account_key: key,
                    pointer,
                    keyword,
                });
            } else if let Some(snapshot) = snapshot {
                result.snapshots.push((key, snapshot));
            }
            Ok(())
        },
    )?;
    Ok(result)
}

pub fn persisted_document_within_bounds(bytes: &[u8]) -> bool {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match *byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_PERSISTED_DOCUMENT_DEPTH {
                    return false;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// Return account keys whose scheduled auth resume has passed the recovery
/// notice threshold. The caller supplies time to keep service policy testable.
pub fn authentication_recovery_account_keys(
    connection: &Connection,
    now: f64,
) -> StoreResult<Vec<String>> {
    Ok(Table::<UsageResumeRecord>::new(connection)
        .list_matching(
            Some(RecordPredicate::UsageResumeAuthRecoveryBefore(
                now - AUTH_WAIT_NOTICE_SECONDS,
            )),
            RecordOrder::RowId,
        )?
        .into_iter()
        .filter_map(|record| {
            record.account_key.value().and_then(
                studio_runtime_schema::usage_resume_record::AccountIdentityEvidence::account_key,
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{authentication_recovery_account_keys, limits_snapshots};
    use rusqlite::Connection;
    use studio_json::{Value, json};

    fn canonical_limits_entity(
        db: &Connection,
        deleted: crate::metadata_values::StoredBoolean,
        raw: Option<String>,
    ) {
        canonical_limits_row(db, "alpha", 1, deleted, raw);
    }

    fn canonical_limits_row(
        db: &Connection,
        account: &str,
        rowid: i64,
        deleted: crate::metadata_values::StoredBoolean,
        raw: Option<String>,
    ) {
        use crate::peer_update_settings::canvas_tx;
        use crate::publication_collection::CollectionName;
        use crate::publication_schema::{EntityValue, SyncEntities};
        use studio_db_api::migration::encode_key;
        let collection = CollectionName::from_stored("rateLimit".into());
        let row = SyncEntities {
            identity: encode_key(&("rateLimit".to_owned(), account.to_owned())),
            source_rowid: rowid,
            publication_seq: 42,
            deleted,
            collection_kind: collection.kind(),
            collection_agent: collection.agent().cloned(),
            non_transcript: collection.non_transcript(),
            collection_order: collection,
            entity_id: account.into(),
            entity_hash: "hash".into(),
            entity_payload: EntityValue::from_stored("rateLimit", account, deleted, raw).unwrap(),
        };
        let tx = db.unchecked_transaction().unwrap();
        canvas_tx(&tx)
            .partition::<SyncEntities>()
            .put(&row)
            .unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn f1_limits_snapshots_canonical_deleted_null_malformed_and_receipt() {
        use crate::metadata_values::StoredBoolean;
        use studio_db_api::Table;
        let db = Connection::open_in_memory().unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::receipt_schema::create(&db).unwrap();
        assert!(limits_snapshots(&db).unwrap().snapshots.is_empty());
        for deleted in [StoredBoolean::Valid(true), StoredBoolean::LegacyInteger(2)] {
            canonical_limits_entity(&db, deleted, None);
            let read = limits_snapshots(&db).unwrap();
            assert!(read.snapshots.is_empty() && read.diagnostics.is_empty());
        }
        for raw in [None, Some("not JSON".into())] {
            canonical_limits_entity(&db, StoredBoolean::Valid(false), raw);
            let read = limits_snapshots(&db).unwrap();
            assert!(read.snapshots.is_empty());
            assert_eq!(read.diagnostics.len(), 1);
            assert_eq!(read.diagnostics[0].account_key, "alpha");
            assert_eq!(read.diagnostics[0].keyword, "type");
        }
        canonical_limits_entity(&db, StoredBoolean::Valid(false), Some(r#"{"value":{"observation":{"accountKey":"alpha","at":1.0,"data":{"accountId":"A"}}}}"#.into()));
        let tx = db.unchecked_transaction().unwrap();
        super::put_limits_observation(
            &tx,
            "alpha",
            &super::LimitsStamp {
                at: Some(9.0),
                ..Default::default()
            },
        )
        .unwrap();
        tx.commit().unwrap();
        let read = limits_snapshots(&db).unwrap();
        assert!(read.diagnostics.is_empty());
        assert_eq!(read.snapshots[0].1.at, Some(9.0));
        let params = super::LimitsSnapshots { prefix_bytes: 17 };
        let plan = crate::query_plan_test_support::explain(
            &db,
            super::LimitsSnapshots::QUERY.sql(),
            &params,
        )
        .unwrap();
        assert!(
            plan.iter().any(|line| line.contains("SEARCH")
                && line.contains("publications_entity_collection_deleted")),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .any(|line| line.contains("SEARCH") && line.contains("runtime_account_state")),
            "{plan:?}"
        );
        let row = super::canvas_read(&db)
            .collect(&super::LimitsSnapshots::QUERY, &params)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(row.1.unwrap().len(), 17);
        assert!(row.2.unwrap() > 17);
        let oversized = " ".repeat(super::MAX_PERSISTED_DOCUMENT_BYTES + 1);
        canonical_limits_entity(&db, StoredBoolean::Valid(false), Some(oversized));
        let read = limits_snapshots(&db).unwrap();
        assert!(read.snapshots.is_empty());
        assert_eq!(read.diagnostics[0].keyword, "resourceLimit");
        canonical_limits_entity(
            &db,
            StoredBoolean::Valid(false),
            Some(r#"{"value":{"observation":{"accountKey":"alpha","at":1.0}}}"#.into()),
        );
        let mut account = super::canvas_read(&db)
            .get::<crate::receipt_tables::AccountStateRow>(&("alpha".to_owned(),))
            .unwrap()
            .unwrap();
        account.limits_observation = Some(
            crate::json_record_codec::ExactJson::from_stored(format!(
                "{{\"padding\":\"{}\"}}",
                "a".repeat(super::MAX_PERSISTED_DOCUMENT_BYTES)
            ))
            .unwrap(),
        );
        let tx = db.unchecked_transaction().unwrap();
        crate::peer_update_settings::canvas_tx(&tx)
            .upsert(&account)
            .unwrap();
        tx.commit().unwrap();
        let bounded = super::canvas_read(&db)
            .collect(
                &super::LimitsSnapshots::QUERY,
                &super::LimitsSnapshots { prefix_bytes: 17 },
            )
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(bounded.3.unwrap().len(), 17);
        assert!(bounded.4.unwrap() > super::MAX_PERSISTED_DOCUMENT_BYTES as i64);
        let read = limits_snapshots(&db).unwrap();
        assert!(read.snapshots.is_empty());
        assert_eq!(read.diagnostics[0].pointer, "/");
        assert_eq!(read.diagnostics[0].keyword, "resourceLimit");
    }

    #[test]
    fn f1_limits_snapshots_canonical_account_isolation_and_source_order() {
        use crate::metadata_values::StoredBoolean;
        use studio_db_api::Table;
        let db = Connection::open_in_memory().unwrap();
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::receipt_schema::create(&db).unwrap();
        canonical_limits_entity(
            &db,
            StoredBoolean::Valid(false),
            Some(r#"{"value":{"observation":{"accountKey":"alpha","at":1.0}}}"#.into()),
        );
        canonical_limits_row(
            &db,
            "beta",
            -9,
            StoredBoolean::Valid(false),
            Some(r#"{"value":{"observation":{"accountKey":"beta","at":2.0}}}"#.into()),
        );
        let read = limits_snapshots(&db).unwrap();
        assert!(read.diagnostics.is_empty());
        assert_eq!(
            read.snapshots
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            ["beta", "alpha"]
        );
        canonical_limits_row(
            &db,
            "beta",
            -9,
            StoredBoolean::Valid(false),
            Some(r#"{"value":{"observation":{"accountKey":"foreign","at":2.0}}}"#.into()),
        );
        let read = limits_snapshots(&db).unwrap();
        assert_eq!(read.snapshots.len(), 1);
        assert_eq!(read.snapshots[0].0, "alpha");
        assert_eq!(read.diagnostics[0].account_key, "beta");
        assert_eq!(read.diagnostics[0].pointer, "/accountKey");
        assert_eq!(read.diagnostics[0].keyword, "identity");
    }

    #[test]
    fn canonical_limits_caller_preserves_missing_null_deleted_and_stamp() {
        use crate::metadata_values::StoredBoolean;
        use crate::publication_schema::Publications;
        use studio_db_api::Table;

        let db = Connection::open_in_memory().unwrap();
        Publications::descriptor().install_if_absent(&db).unwrap();
        let stamp = super::LimitsStamp {
            at: Some(9.0),
            read_at: Some(10.0),
            ..Default::default()
        };
        assert!(
            super::typed_limits_snapshot(&db, "alpha", &stamp)
                .unwrap()
                .is_none()
        );
        for (deleted, raw) in [
            (StoredBoolean::Valid(false), None),
            (StoredBoolean::Valid(true), None),
            (StoredBoolean::LegacyInteger(2), None),
            (
                StoredBoolean::Valid(false),
                Some(
                    r#"{"value":{"accountKey":"alpha","at":1.0,"data":{"status":"ready"},"eligibility":false}}"#,
                ),
            ),
            (
                StoredBoolean::Valid(false),
                Some(r#"{"value":{"accountKey":"foreign","at":1.0,"data":{"status":"ready"}}}"#),
            ),
        ] {
            canonical_limits_entity(&db, deleted, raw.map(str::to_owned));
            let result = super::typed_limits_snapshot(&db, "alpha", &stamp);
            if deleted.as_stored() != 0 {
                assert!(result.unwrap().is_none());
            } else if raw.is_none() {
                assert!(matches!(
                    result,
                    Err(crate::StoreError::Sqlite(
                        rusqlite::Error::InvalidColumnType(_, _, rusqlite::types::Type::Null)
                    ))
                ));
            } else {
                let result = result.unwrap().unwrap();
                assert_eq!(result.account_key, "alpha");
                assert_eq!(result.at, Some(9.0));
                assert_eq!(result.read_at, Some(10.0));
                assert_eq!(result.data.is_some(), raw.unwrap().contains("eligibility"));
            }
        }
    }

    #[test]
    fn agent_provider_stream_keeps_legacy_admission_and_state_guards() {
        let database = crate::fresh_state::test_canvas();
        for (raw, expected) in [
            (None, Ok(None)),
            (Some("null"), Ok(None)),
            (Some("{}"), Ok(None)),
            (Some("[]"), Err(())),
            (Some("[1]"), Err(())),
            (Some("[1,null]"), Err(())),
            (Some("[1,null,2]"), Ok(Some((1, None, 2)))),
            (
                Some(r#"[1,"connected",2]"#),
                Ok(Some((1, Some("connected"), 2))),
            ),
            (Some(r#"[ 1, "\u0063", 2 ]"#), Ok(Some((1, Some("c"), 2)))),
            (Some("[null,null,null]"), Ok(None)),
            (Some("[1,null,2,3]"), Err(())),
            (Some("[true,null,null]"), Err(())),
            (Some("[1,2,null]"), Err(())),
            (Some("[1,null,false]"), Err(())),
            (Some("true"), Err(())),
            (Some("0"), Err(())),
            (Some(r#""legacy""#), Err(())),
            (
                Some(r#"{"generation":1,"connection":""}"#),
                Ok(Some((1, None, 0))),
            ),
            (
                Some(r#"{"generation":18446744073709551615,"ack":18446744073709551615}"#),
                Ok(Some((u64::MAX, None, u64::MAX))),
            ),
            (
                Some(r#"{"generation":null,"connection":"connected"}"#),
                Err(()),
            ),
            (Some(r#"{"ack":0}"#), Err(())),
            (Some(r#"{"generation":-1}"#), Err(())),
            (Some(r#"{"generation":1.0}"#), Err(())),
            (Some(r#"{"ack":18446744073709551616}"#), Err(())),
        ] {
            let suffix = raw
                .map(|raw| format!(r#", "providerStream":{raw}"#))
                .unwrap_or_default();
            let wire = format!(
                r#"{{"id":"a","rootId":"a","epoch":1,"status":"idle","accountKey":"default","executionMode":"vm"{suffix}}}"#
            );
            let agent: crate::AgentRecord = studio_json::from_str(&wire).unwrap();
            let written = studio_json::to_string(&agent).unwrap();
            let fields: std::collections::BTreeMap<String, studio_json::OwnedRawJson> =
                studio_json::from_str(&written).unwrap();
            assert_eq!(
                fields.get("providerStream").map(|value| value.as_str()),
                raw
            );
            let actual = super::provider_state(&database, super::ProviderOwner::Agent(&agent))
                .map(|state| {
                    state.map(|(state, ack)| match state {
                        super::ProviderState::Held { generation } => (generation, None, ack),
                        super::ProviderState::Connected {
                            generation,
                            connection,
                        } => (generation, Some(connection), ack),
                    })
                })
                .map_err(|_| ());
            let expected = expected.map(|state| {
                state.map(|(generation, connection, ack)| {
                    (generation, connection.map(str::to_owned), ack)
                })
            });
            assert_eq!(actual, expected, "{raw:?}");
        }
    }

    #[test]
    fn metadata_only_vm_stream_has_no_provider_state() {
        let fields = super::StreamFields {
            generation: None,
            connection: None,
            ack: None,
        };
        assert!(fields.into_state().unwrap().is_none());
        for stream in [
            r#"{"streams":{},"connection":"c"}"#,
            r#"{"streams":{},"ack":0}"#,
        ] {
            let fields: super::StreamFields = studio_json::from_str(stream).unwrap();
            assert!(fields.into_state().is_err());
        }
    }

    #[test]
    fn receipt_account_checkpoint_fences_generations_and_never_regresses() {
        let mut db = Connection::open_in_memory().expect("SQLite");
        crate::receipt_schema::create(&db).expect("target schema");
        let tx = db.transaction().expect("transaction");
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 7).expect("empty"),
            0
        );
        super::set_provider_checkpoint(&tx, "alpha", 7, 10).expect("initialize");
        super::set_provider_checkpoint(&tx, "alpha", 7, 4).expect("repeat earlier ACK");
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 7).expect("current"),
            10
        );
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 6).expect("old generation"),
            0
        );
        assert!(super::set_provider_checkpoint(&tx, "alpha", 6, 99).is_err());
        assert!(super::set_provider_checkpoint(&tx, "alpha", 8, 1).is_err());
        assert!(super::set_provider_checkpoint(&tx, "alpha", 7, u64::MAX).is_err());
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 7).expect("unchanged"),
            10
        );
        assert_eq!(super::streams(&tx, "alpha").expect("empty streams"), None);
        super::put_streams(&tx, "alpha", "{\"revision\":\"r\",\"metadata\":{}}").expect("streams");
        assert!(
            super::streams(&tx, "alpha")
                .expect("stream state")
                .is_some()
        );
        assert_eq!(
            super::allocate_next_generation(&tx, "alpha").expect("first generation"),
            1
        );
        tx.commit().expect("commit");
        assert_eq!(
            super::provider_checkpoint(&db, "alpha", 7).expect("persisted"),
            10
        );
    }

    #[test]
    fn receipt_direct_counter_rollback_and_exhaustion_preserve_other_state() {
        let mut db = Connection::open_in_memory().expect("SQLite");
        crate::receipt_schema::create(&db).expect("target schema");
        db.execute("INSERT INTO runtime_account_state VALUES('alpha',41,7,'connection',10,1,'null',NULL,NULL)", []).expect("migrated counter");
        {
            let tx = db.transaction().expect("transaction");
            assert_eq!(
                super::allocate_next_generation(&tx, "alpha").expect("allocate"),
                42
            );
            tx.rollback().expect("rollback");
        }
        let tx = db.transaction().expect("transaction");
        assert_eq!(
            super::allocate_next_generation(&tx, "alpha").expect("allocate after rollback"),
            42
        );
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 7).expect("ACK preserved"),
            10
        );
        tx.execute(
            "UPDATE runtime_account_state SET next_generation=?1 WHERE account='alpha'",
            [i64::MAX],
        )
        .expect("exhausted counter");
        assert!(super::allocate_next_generation(&tx, "alpha").is_err());
        assert_eq!(
            super::provider_checkpoint(&tx, "alpha", 7).expect("ACK preserved"),
            10
        );
    }

    #[test]
    fn receipt_account_observation_restores_without_receipt_table() {
        let db = Connection::open_in_memory().expect("SQLite");
        crate::receipt_schema::create(&db).expect("target schema");
        use studio_db_api::Table;
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        canonical_limits_entity(&db, crate::metadata_values::StoredBoolean::Valid(false), Some(r#"{"value":{"observation":{"accountKey":"alpha","at":1.0,"data":{"accountId":"A"}}}}"#.into()));
        db.execute(
            "INSERT INTO runtime_account_state VALUES('alpha',7,NULL,NULL,0,0,'null',?1,NULL)",
            [json!({"scope":null,"cursor":0,"at":2.0}).to_string()],
        )
        .expect("observation");
        let restored = limits_snapshots(&db).expect("read current account owner only");
        assert!(restored.diagnostics.is_empty());
        assert_eq!(restored.snapshots[0].1.at, Some(2.0));
        assert_eq!(
            restored.snapshots[0]
                .1
                .data
                .as_ref()
                .unwrap()
                .0
                .account_id
                .as_deref(),
            Some("A")
        );
        assert!(
            super::latest_limits_snapshot(&db, "alpha").expect("latest")
                == Some(restored.snapshots[0].1.clone())
        );
    }

    fn database(payload: &Value) -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        use studio_db_api::Table;
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&connection)
            .unwrap();
        crate::receipt_schema::create(&connection).expect("account state");
        if let Some(accounts) = payload
            .pointer("/value/rateLimitsByAccount")
            .and_then(Value::as_object)
        {
            for (n, (key, snapshot)) in accounts.iter().enumerate() {
                canonical_limits_row(
                    &connection,
                    key,
                    n as i64 + 1,
                    crate::metadata_values::StoredBoolean::Valid(false),
                    Some(json!({"value":{"observation":snapshot}}).to_string()),
                );
            }
        } else if payload["value"] == false {
            canonical_limits_row(
                &connection,
                "default",
                1,
                crate::metadata_values::StoredBoolean::Valid(false),
                Some(payload.to_string()),
            );
        }
        connection
    }

    #[test]
    fn account_limits_ignore_python_single_account_fallback() {
        let db = database(&json!({"value":{"rateLimits":{
            "accountKey":"default","at":1.0,"data":{"status":"ready"}
        }}}));
        let read = limits_snapshots(&db).expect("read current account map");
        assert!(
            read.snapshots.is_empty(),
            "only rateLimitsByAccount restores accounts"
        );
    }

    #[test]
    fn restored_limits_are_typed_observations() {
        let db = database(
            &json!({"value":{"rateLimitsByAccount":{"alpha":{"accountKey":"alpha","at":1.0,"data":{"status":"ready"}}}}}),
        );
        let restored = limits_snapshots(&db).unwrap();
        let observation: &studio_runtime_schema::usage_limits::UsageLimitsResponse =
            &restored.snapshots[0].1;
        assert_eq!(observation.account_key, "alpha");
    }

    #[test]
    fn reads_valid_account_map() {
        let alpha = json!({"accountKey":"alpha","at":1.0,"data":{"status":"ready",
            "rateLimits":{"primary":{"usedPercent":12,"providerWindow":{"x":1}}}}
        });
        let db = database(
            &json!({"value":{"rateLimitsByAccount":{"alpha":alpha},"rateLimits":{"accountKey":"default","at":null,"data":{"status":"ready"}}}}),
        );
        let restored = limits_snapshots(&db).expect("read");
        assert!(restored.diagnostics.is_empty());
        let expected: studio_runtime_schema::usage_limits::UsageLimitsResponse =
            serde::Deserialize::deserialize(alpha).unwrap();
        assert!(
            restored
                .snapshots
                .iter()
                .any(|(key, value)| key == "alpha" && *value == expected)
        );
        assert!(!restored.snapshots.iter().any(|(key, _)| key == "default"));
        assert!(!restored.snapshots.iter().any(|(key, _)| key == "missing"));
    }

    #[test]
    fn malformed_envelope_reports_safe_location_and_limits_payload_bounds() {
        let db = database(&json!({"value":false}));
        let read = limits_snapshots(&db).expect("read diagnostic");
        assert_eq!(read.diagnostics[0].workspace_row, "rateLimit");
        assert_eq!(read.diagnostics[0].pointer, "/value");
        assert!(read.snapshots.is_empty());

        let deep = format!(
            "{{\"value\":{{\"rateLimitsByAccount\":{{\"default\":{{\"accountKey\":\"default\",\"at\":1,\"data\":{{\"x\":{}0{} }} }} }} }} }}",
            "[".repeat(10_000),
            "]".repeat(10_000)
        );
        let db = Connection::open_in_memory().expect("SQLite");
        use studio_db_api::Table;
        crate::publication_schema::Publications::descriptor()
            .install_if_absent(&db)
            .unwrap();
        crate::receipt_schema::create(&db).expect("account state");
        canonical_limits_entity(
            &db,
            crate::metadata_values::StoredBoolean::Valid(false),
            Some(deep),
        );
        let read = limits_snapshots(&db).expect("bounded read");
        assert_eq!(read.diagnostics[0].keyword, "resourceLimit");
    }

    #[test]
    fn malformed_account_snapshot_has_account_pointer_and_no_value_text() {
        let marker = "LIMIT_SECRET_93841";
        let db = database(&json!({"value":{"rateLimitsByAccount":{
            "account-a":{"accountKey":"account-a","at":marker,"data":{"status":"ready"}}
        }}}));
        let read = limits_snapshots(&db).expect("read diagnostic");
        assert!(read.snapshots.is_empty());
        assert_eq!(read.diagnostics[0].account_key, "account-a");
        assert_eq!(read.diagnostics[0].pointer, "/at");
        let rendered = format!(
            "{} {}",
            read.diagnostics[0].pointer, read.diagnostics[0].keyword
        );
        assert!(!rendered.contains(marker));
    }

    #[test]
    fn invalid_mapped_default_reports_diagnostic() {
        let payload = json!({"value": {
            "rateLimitsByAccount": {"default": {
                "accountKey":"default", "at":1.0,
                "data":{"rateLimits":{"primary":{"usedPercent":"1","placeholder":"x"}}}
            }},
            "rateLimits":{"accountKey":"default","at":2.0,"data":{"status":"legacy"}}
        }});
        let read = limits_snapshots(&database(&payload)).expect("validated store projection");
        assert!(read.snapshots.is_empty());
        assert_eq!(read.diagnostics.len(), 1);
        assert_eq!(read.diagnostics[0].account_key, "default");
        assert_eq!(read.diagnostics[0].pointer, "/data");
    }

    #[test]
    fn durable_observation_times_restore_freshness_without_duplicating_provider_data() {
        let db = database(
            &json!({"value":{"rateLimitsByAccount":{"alpha":{"accountKey":"alpha","at":1.0,"readAt":1.0,"data":{"accountId":"A"}}}}}),
        );
        db.execute(
            "INSERT INTO runtime_account_state VALUES('alpha',0,NULL,NULL,0,0,'null',?1,NULL)",
            [json!({"scope":null,"cursor":0,"at":2.0}).to_string()],
        )
        .expect("account observation");
        let restored = limits_snapshots(&db).expect("restore");
        assert!(restored.diagnostics.is_empty());
        assert_eq!(restored.snapshots[0].1.at, Some(2.0));
        assert!(restored.snapshots[0].1.read_at.is_none());
        assert_eq!(
            restored.snapshots[0]
                .1
                .data
                .as_ref()
                .unwrap()
                .0
                .account_id
                .as_deref(),
            Some("A")
        );
        db.execute(
            "UPDATE runtime_account_state SET limits_observation=?1",
            [json!({"at":"private-corrupt-timestamp"}).to_string()],
        )
        .expect("invalid metadata");
        let restored = limits_snapshots(&db).expect("isolated diagnostic");
        assert!(restored.snapshots.is_empty());
        assert_eq!(restored.diagnostics[0].account_key, "alpha");
        assert_eq!(restored.diagnostics[0].pointer, "/at");
        assert!(
            !restored.diagnostics[0]
                .keyword
                .contains("private-corrupt-timestamp")
        );
    }

    #[test]
    fn reads_due_authentication_recovery_keys() {
        let db = Connection::open_in_memory().expect("SQLite");
        db.execute_batch("CREATE TABLE runtime_usage_resumes(id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL)").expect("table");
        db.execute("INSERT INTO runtime_usage_resumes VALUES('due','agent',?1)", [json!({"status":"scheduled","cause":"auth","accountKey":"default","failedAt":100.0}).to_string()]).expect("row");
        db.execute("INSERT INTO runtime_usage_resumes VALUES('opaque','agent',?1)", [json!({"status":"scheduled","cause":"auth","accountKey":[1],"failedAt":100.0,"epoch":null,"taskClaims":{"old":true}}).to_string()]).expect("historical row");
        assert_eq!(
            authentication_recovery_account_keys(&db, 1900.0).expect("read"),
            vec!["default"]
        );
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "account_state",
        requirements: &REQUIREMENTS,
    }
}
