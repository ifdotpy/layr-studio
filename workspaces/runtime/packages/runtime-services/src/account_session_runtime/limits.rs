//! Ordered limits persistence. The journal identity and the snapshot commit together.
use super::*;
use rusqlite::OptionalExtension;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::usage_limits::RateLimitView;
use studio_runtime_schema::usage_limits::{UsageLimitsData, UsageLimitsResponse};
use studio_runtime_store::account_state;
use studio_runtime_store::{StoreError, SyncDocumentsRepository};

pub(super) enum LimitsWrite {
    Snapshot(LimitsSnapshot),
    Checking {
        reset_at: f64,
    },
    Finish,
    Notification {
        scope: ConnectionScope,
        cursor: u64,
        observed_at: f64,
        data: UsageLimitsData,
    },
    ReadFailure {
        scope: Option<ConnectionScope>,
        before: UsageLimitsResponse,
        value: LimitsSnapshot,
    },
    Read {
        scope: ConnectionScope,
        before: UsageLimitsResponse,
        value: LimitsSnapshot,
    },
}

impl LimitsWrite {
    fn scope(&self) -> Option<&ConnectionScope> {
        match self {
            Self::Snapshot(_) | Self::Checking { .. } | Self::Finish => None,
            Self::Notification { scope, .. } => Some(scope),
            Self::Read { scope, .. } => Some(scope),
            Self::ReadFailure { scope, .. } => scope.as_ref(),
        }
    }
}

pub(super) async fn persist(
    store: Arc<studio_runtime_store::RuntimeStore>,
    cache: Arc<std::sync::Mutex<std::collections::BTreeMap<String, UsageLimitsResponse>>>,
    account: String,
    write: LimitsWrite,
    wake: Option<Arc<Notify>>,
    session: Option<Arc<dyn RetainedAccountSession>>,
) -> Result<bool, AccountSessionError> {
    let now = timestamp()?;
    let cache_account = account.to_owned();
    let result = store
        .transaction_with_post_commit(
            move |tx| commit(tx, &account, write, now, session.as_deref()),
            move |(value, changed)| {
                if let Some(value) = value {
                    cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(cache_account, value.clone());
                }
                if *changed && let Some(wake) = wake {
                    wake.notify_one();
                }
            },
        )
        .await
        .map_err(|_| AccountSessionError::Persistence)?;
    Ok(result.0.1)
}

fn commit(
    tx: &studio_runtime_store::Transaction<'_>,
    account: &str,
    write: LimitsWrite,
    now: f64,
    session: Option<&dyn RetainedAccountSession>,
) -> Result<(Option<UsageLimitsResponse>, bool), StoreError> {
    // Check at the writer boundary as the session may retire while this write queues.
    if let Some(scope) = write.scope()
        && session
            .as_ref()
            .and_then(|s| s.current_scope(account))
            .as_ref()
            != Some(scope)
    {
        if matches!(
            &write,
            LimitsWrite::Read { .. } | LimitsWrite::ReadFailure { .. }
        ) {
            return Err(StoreError::SchemaMismatch(
                "limits read belongs to a retired session".into(),
            ));
        }
        return Ok((None, false));
    }
    let mut stamp =
        account_state::LimitsStamp::read(account_state::limits_observation(tx, account)?)?;
    let old = account_state::typed_limits_snapshot(tx, account, &stamp)?;
    let mut stamp_changed = false;
    let committed_view = studio_runtime_store::ProjectionPublicationQueries::raw_payload(
        tx,
        "rateLimit",
        account,
        true,
    )?
    .map(|payload| account_state::normalized_limits_view(account, payload.as_bytes(), now).0);
    let mut checking = None;
    let force_projection = matches!(
        &write,
        LimitsWrite::Checking { .. }
            | LimitsWrite::Finish
            | LimitsWrite::Read { .. }
            | LimitsWrite::ReadFailure { .. }
    );
    let value = match write {
        LimitsWrite::Snapshot(value) => value.response,
        LimitsWrite::Finish => {
            if !committed_view.as_ref().is_some_and(|view| {
                matches!(
                    view.projection.status,
                    studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking { .. }
                )
            }) {
                return Ok((old, false));
            }
            let Some(value) = old.clone() else {
                return Ok((None, false));
            };
            value
        }
        LimitsWrite::Checking { reset_at } => {
            checking = Some(reset_at);
            let Some(value) = old.clone() else {
                return Ok((None, false));
            };
            value
        }
        LimitsWrite::ReadFailure { before, value, .. } => {
            if old.as_ref().is_some_and(|old| *old != before) {
                old.clone().unwrap_or_else(|| value.response.clone())
            } else {
                value.response
            }
        }
        LimitsWrite::Read {
            scope,
            before,
            value,
        } => {
            // An identity change replaces the published observation with pending;
            // the cached pre-read value may still belong to the previous identity.
            let pending = committed_view.as_ref().is_some_and(|view| {
                matches!(
                    view.projection.status,
                    studio_runtime_schema::usage_limits::AccountLimitsStatus::Pending
                )
            });
            if !pending && old.as_ref().is_some_and(|old| *old != before) {
                old.clone().unwrap_or_else(|| value.response.clone())
            } else {
                stamp.cursor = if stamp.scope.as_ref() == Some(&scope) {
                    stamp.cursor
                } else {
                    0
                };
                stamp.scope = Some(scope);
                stamp_changed = true;
                value.response
            }
        }
        LimitsWrite::Notification {
            scope,
            cursor,
            observed_at,
            data: incoming,
        } => {
            checking = committed_view
                .as_ref()
                .and_then(|view| match view.projection.status {
                    studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking {
                        reset_at,
                    } => Some(reset_at),
                    studio_runtime_schema::usage_limits::AccountLimitsStatus::Current
                    | studio_runtime_schema::usage_limits::AccountLimitsStatus::Pending => None,
                });
            if stamp.scope.as_ref() == Some(&scope) && stamp.cursor >= cursor {
                return Ok((old, false));
            }
            if stamp.scope.as_ref().is_some_and(|old| {
                old.connection == scope.connection && old.generation > scope.generation
            }) {
                return Ok((old, false));
            }
            stamp.scope = Some(scope);
            stamp.cursor = cursor;
            stamp_changed = true;
            if old
                .as_ref()
                .and_then(|old| old.at)
                .is_some_and(|at| at > observed_at)
            {
                return Ok((old, false));
            }
            let mut value = old.clone().unwrap_or_else(|| UsageLimitsResponse {
                account_key: account.to_owned(),
                ..Default::default()
            });
            let mut data = value.data.take().map(|data| data.0).unwrap_or_default();
            let mut buckets = data.rate_limits_by_limit_id.take().unwrap_or_default();
            if let Some(new) = incoming.rate_limits_by_limit_id {
                buckets.extend(new);
            }
            if let Some(bucket) = &incoming.rate_limits {
                buckets.insert(
                    bucket.0.limit_id.clone().unwrap_or_else(|| "codex".into()),
                    bucket.clone(),
                );
            }
            macro_rules! update { ($($field:ident),* $(,)?) => { $(if incoming.$field.is_some() { data.$field = incoming.$field; })* }; }
            update!(
                account_id,
                rate_limit_reset_credits,
                rate_limits,
                signed_in,
                source,
                status,
                claude_rate_limit
            );
            if incoming.ordinary_usage_allowed.is_some() {
                data.ordinary_usage_allowed = incoming.ordinary_usage_allowed;
            }
            data.rate_limits_by_limit_id = Some(buckets);
            value.data = Some(JsonObject(data));
            value.at = Some(observed_at);
            value.error = None;
            value
        }
    };
    let identical = old.as_ref() == Some(&value);
    if identical && !stamp_changed && !force_projection {
        return Ok((Some(value), false));
    }
    stamp.set_times(&value);
    account_state::put_limits_observation(tx, account, &stamp)?;
    if identical && !force_projection {
        return Ok((Some(value), false));
    }
    if !identical {
        crate::continuation_runtime::stage_limits_changed_for_account(tx, account, &value, now)
            .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    }
    let current_identity: Option<String> =
        studio_runtime_store::ProjectionPublicationQueries::current_account_identity(tx, account)?;
    let observation_identity = value
        .data
        .as_ref()
        .and_then(|data| data.0.account_id.as_deref());
    let published = if studio_runtime_schema::usage_limits::account_identity_mismatch(
        current_identity.as_deref(),
        observation_identity,
    ) {
        UsageLimitsResponse {
            account_key: account.to_owned(),
            ..Default::default()
        }
    } else {
        value.clone()
    };
    let mut view =
        studio_runtime_schema::usage_limits::RateLimitView::from_observation(published, now);
    populate_agents(tx, &mut view, now)?;
    if let Some(reset_at) = checking {
        mark_checking(&mut view, reset_at);
    }
    SyncDocumentsRepository::put_entity(tx, "rateLimit", account, view, false)?;
    Ok((Some(value), true))
}

// LimitAgent owns the consumed fields. JSON tokens preserve their stored types;
// null and absent optional fields both have no value in the limit projection.
#[cfg(test)]
const LIMIT_AGENTS_SQL: &str = "SELECT id,json_object(
    'id',record->'$.id','accountKey',record->'$.accountKey','epoch',record->'$.epoch',
    'deletedAt',record->'$.deletedAt','model',record->'$.model','provider',record->'$.provider',
    'error',record->'$.error','usageResume',record->'$.usageResume',
    'threadId',record->'$.threadId','lastCompletedTurn',record->'$.lastCompletedTurn')
    FROM runtime_agents WHERE account_key IN (?1,CASE WHEN ?1='default' THEN '' ELSE ?1 END) ORDER BY rowid";

pub(crate) fn populate_agents(
    tx: &studio_runtime_store::Transaction<'_>,
    view: &mut studio_runtime_schema::usage_limits::RateLimitView,
    now: f64,
) -> Result<(), StoreError> {
    let limits = crate::usage_resume::LimitsObservation::from_response(view.observation.clone());
    view.projection.agents.clear();
    // AGENT_COLUMNS in runtime-store/src/agent_columns.rs owns this STORED
    // generated account_key: missing/null becomes default; empty stays empty.
    // Include empty keys for default, matching the projection normalization below.
    for (id, raw) in studio_runtime_store::limit_agent_queries::LimitAgentQueries::recovery_agents(
        tx,
        limits.account_key(),
    )? {
        let agent = decode_limit_agent(&id, &raw)?;
        if recovery_candidate(&agent) {
            view.projection.agents.insert(
                agent.id.to_string(),
                crate::usage_resume::agent_limit_projection_view(&limits, &agent, now),
            );
        }
    }
    Ok(())
}

// Keep individual agent publications aligned with the indexed account query.
fn recovery_candidate(agent: &crate::usage_resume::LimitAgent<'_>) -> bool {
    use studio_runtime_schema::provider_errors::KnownCodexErrorKind;
    !agent.deleted_at.is_some_and(|at| at != 0.0)
        && matches!(
            agent.error.as_ref().and_then(|error| error.known_kind()),
            Some(KnownCodexErrorKind::UsageLimitExceeded | KnownCodexErrorKind::RateLimitExceeded)
        )
}

/// Validate only the stored fields consumed by account-limit projection.
fn decode_limit_agent<'a>(
    id: &str,
    raw: &'a str,
) -> Result<crate::usage_resume::LimitAgent<'a>, StoreError> {
    #[cfg(test)]
    AGENT_DECODES.with(|count| count.set(count.get() + 1));
    let invalid = || StoreError::InvalidSyncObject {
        scope: "runtime_agents".into(),
        id: id.to_owned(),
    };
    let agent: crate::usage_resume::LimitAgent<'_> =
        studio_json::from_str(raw).map_err(|_| invalid())?;
    if agent.id != id
        || !(0..=studio_runtime_schema::shared_values::MAX_SAFE_INTEGER).contains(&agent.epoch)
    {
        return Err(invalid());
    }
    Ok(agent)
}

pub(super) async fn recovery_deadline(
    store: Arc<studio_runtime_store::RuntimeStore>,
    account: String,
    attempted: f64,
) -> Option<f64> {
    store
        .read_async("account-session/recovery-deadline", move |db| {
            use studio_runtime_schema::usage_limits::RecoveryTiming;
            let payload = studio_runtime_store::ProjectionPublicationQueries::raw_payload(
                db,
                "rateLimit",
                &account,
                true,
            )?;
            let Some(payload) = payload else {
                return Ok::<_, StoreError>(None);
            };
            let (value, _) =
                account_state::normalized_limits_view(&account, payload.as_bytes(), attempted);
            let checked = value
                .observation
                .read_at
                .into_iter()
                .chain(value.observation.checked_at)
                .fold(0.0, f64::max);
            Ok(value
                .projection
                .agents
                .values()
                .filter_map(|agent| match &agent.recovery.as_ref()?.timing {
                    RecoveryTiming::Waiting { reset_at }
                    | RecoveryTiming::Unconfirmed { reset_at } => Some(*reset_at),
                    RecoveryTiming::Unknown
                    | RecoveryTiming::Checking { .. }
                    | RecoveryTiming::Confirmed { .. } => None,
                })
                .filter(|reset| *reset > checked.max(attempted))
                .reduce(f64::min))
        })
        .await
        .ok()
        .flatten()
}

fn mark_checking(view: &mut studio_runtime_schema::usage_limits::RateLimitView, reset_at: f64) {
    use studio_runtime_schema::usage_limits::{AccountLimitsStatus, RecoveryTiming};
    view.projection.status = AccountLimitsStatus::Checking { reset_at };
    for agent in view.projection.agents.values_mut() {
        if let Some(recovery) = &mut agent.recovery
            && matches!(recovery.timing, RecoveryTiming::Waiting { reset_at: at } | RecoveryTiming::Unconfirmed { reset_at: at } if at <= reset_at)
        {
            recovery.timing = RecoveryTiming::Checking { reset_at };
        }
    }
}

/// One account's normalized view and whether its persisted document needs repair.
pub(crate) type AccountView = (String, RateLimitView, bool);

fn decoded_account_views(
    tx: &studio_runtime_store::Transaction<'_>,
    now: f64,
) -> Result<Vec<AccountView>, StoreError> {
    let mut views = Vec::new();
    studio_runtime_store::ProjectionPublicationQueries::visit_limits(
        tx,
        account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64 + 1,
        |(account, payload)| {
            #[cfg(test)]
            ACCOUNT_VIEW_DECODES.with(|count| count.set(count.get() + 1));
            let (view, unreadable) = account_state::normalized_limits_view(
                &account,
                payload.as_deref().unwrap_or_default(),
                now,
            );
            views.push((account, view, unreadable));
            Ok(())
        },
    )?;
    Ok(views)
}

#[cfg(test)]
thread_local! {
    pub(crate) static ACCOUNT_VIEW_DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Recompute affected agent projections in the transaction that publishes them.
pub(crate) fn publish_agent_change(
    tx: &studio_runtime_store::Transaction<'_>,
    agent_id: &str,
    deleted: bool,
) -> Result<(), StoreError> {
    let mut views = publication_views(tx)?;
    publish_agent_change_with_views(tx, agent_id, deleted, &mut views)
}

pub(crate) fn publication_views(
    tx: &studio_runtime_store::Transaction<'_>,
) -> Result<Vec<AccountView>, StoreError> {
    let now = timestamp().map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    decoded_account_views(tx, now)
}

/// Reuse only within a publication batch; each write updates the borrowed views
/// so the next agent sees exactly the preceding publication.
pub(crate) fn publish_agent_change_with_views(
    tx: &studio_runtime_store::Transaction<'_>,
    agent_id: &str,
    deleted: bool,
    views: &mut [AccountView],
) -> Result<(), StoreError> {
    if views.is_empty() {
        return Ok(());
    }
    let now = timestamp().map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    let raw = if deleted {
        None
    } else {
        tx.prepare_cached("SELECT record FROM runtime_agents WHERE id=?1")?
            .query_row([agent_id], |row| row.get::<_, String>(0))
            .optional()?
    };
    // LimitAgent consumes no volatile columns, so the stored record is sufficient.
    let agent = raw
        .as_deref()
        .map(|raw| decode_limit_agent(agent_id, raw))
        .transpose()?;
    for (account, value, unreadable) in views {
        if *unreadable {
            studio_runtime_store::ProjectionPublicationQueries::delete_entity(
                tx,
                "rateLimit",
                account,
            )?;
            populate_agents(tx, value, now)?;
            SyncDocumentsRepository::put_entity(tx, "rateLimit", account, &*value, false)?;
            *unreadable = false;
            continue;
        }
        let matches = agent.as_ref().is_some_and(|agent| {
            let key = if agent.account_key.is_empty() {
                "default"
            } else {
                &agent.account_key
            };
            key == account.as_str() && recovery_candidate(agent)
        });
        if matches {
            let limits =
                crate::usage_resume::LimitsObservation::from_response(value.observation.clone());
            if let Some(agent) = &agent {
                value.projection.agents.insert(
                    agent_id.to_owned(),
                    crate::usage_resume::agent_limit_projection_view(&limits, agent, now),
                );
            }
        } else if value.projection.agents.remove(agent_id).is_none() {
            continue;
        }
        if let studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking { reset_at } =
            value.projection.status
        {
            mark_checking(value, reset_at);
        }
        SyncDocumentsRepository::put_entity(tx, "rateLimit", account, &*value, false)?;
    }
    Ok(())
}

pub(crate) fn seed_agents(tx: &studio_runtime_store::Transaction<'_>) -> Result<(), StoreError> {
    let now = timestamp().map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    for (account, mut value, unreadable) in decoded_account_views(tx, now)? {
        if unreadable {
            studio_runtime_store::ProjectionPublicationQueries::delete_entity(
                tx,
                "rateLimit",
                &account,
            )?;
        }
        if matches!(
            value.projection.status,
            studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking { .. }
        ) {
            value.projection.status =
                studio_runtime_schema::usage_limits::AccountLimitsStatus::Current;
        }
        populate_agents(tx, &mut value, now)?;
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &account, value, false)?;
    }
    Ok(())
}

pub(super) async fn read_for_resume(
    runtime: &AccountSessionRuntime,
    account: &str,
    after: f64,
) -> Result<UsageLimitsResponse, AccountSessionError> {
    if let (Some(scope), Some(store)) = (runtime.current_scope(account), runtime.store.as_ref()) {
        let key = account.to_owned();
        let expected = scope.clone();
        let candidate = store
            .read_async("account-session/resume-limits", move |db| {
                let stamp = account_state::limits_observation(db, &key)?;
                let Some(stamp) = stamp else {
                    return Ok(None);
                };
                let stamp = account_state::LimitsStamp::read(Some(stamp))?;
                if stamp.scope.as_ref() != Some(&expected) {
                    return Ok(None);
                }
                let candidate = account_state::typed_limits_snapshot(db, &key, &stamp)?;
                Ok::<_, StoreError>(candidate)
            })
            .await
            .map_err(|_| AccountSessionError::Persistence)?;
        let now = timestamp()?;
        if let Some(value) = candidate
            && runtime.current_scope(account).as_ref() == Some(&scope)
            && {
                let observation =
                    crate::usage_resume::LimitsObservation::from_response(value.clone());
                observation.account_key() == account && observation.fresh_after(after, now)
            }
        {
            return Ok(value);
        }
    }
    Ok(LimitsSnapshot::read(account, runtime.read_limits(account).await?)?.response)
}

#[cfg(test)]
thread_local! {
    static AGENT_DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[tokio::test]
    async fn recovery_projection_matches_the_previous_account_projection() {
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, AgentStatus};
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store.transaction(|tx| {
            for (id, account, deleted, error) in [
                ("usage", "default", None, json!({"codexErrorInfo":"usageLimitExceeded"})),
                ("rate", "default", None, json!({"codexErrorInfo":{"rateLimitExceeded":{}}})),
                ("zero", "default", Some(0.0), json!({"codexErrorInfo":"usageLimitExceeded"})),
                ("empty", "", None, json!({"codexErrorInfo":"rateLimitExceeded"})),
                ("deleted", "default", Some(1.0), json!({"codexErrorInfo":"usageLimitExceeded"})),
                ("other", "other", None, json!({"codexErrorInfo":"usageLimitExceeded"})),
                ("idle", "default", None, Value::Null),
                ("auth", "default", None, json!({"codexErrorInfo":"unauthorized"})),
                ("message", "default", None, json!("failure")),
                ("ambiguous", "default", None, json!({"codexErrorInfo":{"usageLimitExceeded":{},"rateLimitExceeded":{}}})),
                ("empty-kind", "default", None, json!({"codexErrorInfo":{}})),
            ] {
                let id = AgentId::from(id);
                let mut agent = AgentRecord::new(id.clone(), id, 0, AgentStatus::Waiting, account.into());
                agent.provider = studio_runtime_schema::queue_command::Field::Value(studio_runtime_providers::ProviderKind::Codex);
                agent.error = serde::Deserialize::deserialize(error).unwrap();
                agent.model = Some("gpt-5".into());
                agent.thread_id = Some(format!("thread-{}", agent.id.0));
                agent.last_completed_turn = studio_runtime_schema::queue_command::Field::Value("turn".into());
                agent.deleted_at = deleted;
                agent.usage_resume = serde::Deserialize::deserialize(json!({"id":"resume","accountKey":"default","epoch":0,"failedAt":10.0,"cause":"usage_limit","status":"scheduled","resetAt":50.0,"threadId":agent.thread_id,"turnId":"turn"})).unwrap();
                AgentRepository::put(tx, &agent)?;
            }
            let response: UsageLimitsResponse = serde::Deserialize::deserialize(json!({"accountKey":"default","at":100.0,"readAt":100.0,"data":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":100.0,"resetsAt":150.0}}}})).unwrap();
            let limits = crate::usage_resume::LimitsObservation::from_response(response.clone());
            let mut expected = std::collections::BTreeMap::new();
            for agent in studio_runtime_store::Table::<AgentRecord>::new(tx).list()? {
                let account = if agent.account_key.is_empty() { "default" } else { &agent.account_key };
                let projection = crate::usage_resume::agent_limit_projection(&limits, &agent, 100.0);
                if account == "default" && !agent.deleted_at.is_some_and(|at| at != 0.0) && projection.recovery.is_some() {
                    expected.insert(agent.id.0, projection);
                }
            }
            let mut view = RateLimitView::from_observation(response, 100.0);
            populate_agents(tx, &mut view, 100.0)?;
            assert_eq!(view.projection.agents.len(), 4);
            assert_eq!(studio_json::to_string(&view.projection.agents)?, studio_json::to_string(&expected)?);
            Ok(())
        }).await.unwrap();
    }

    #[tokio::test]
    async fn agent_change_removes_recovery_when_the_error_clears() {
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, AgentStatus};
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction(|tx| {
                let id = AgentId::from("recovering");
                let mut agent =
                    AgentRecord::new(id.clone(), id, 0, AgentStatus::Waiting, "default".into());
                agent.provider = studio_runtime_schema::queue_command::Field::Value(
                    studio_runtime_providers::ProviderKind::Codex,
                );
                agent.error =
                    serde::Deserialize::deserialize(json!({"codexErrorInfo":"usageLimitExceeded"}))
                        .unwrap();
                AgentRepository::put(tx, &agent)?;
                let response = UsageLimitsResponse {
                    account_key: "default".into(),
                    at: Some(1.0),
                    ..Default::default()
                };
                commit(
                    tx,
                    "default",
                    LimitsWrite::Snapshot(LimitsSnapshot { response }),
                    1.0,
                    None,
                )?;
                assert!(
                    publication_views(tx)?[0]
                        .1
                        .projection
                        .agents
                        .contains_key("recovering")
                );
                agent.error = studio_runtime_schema::queue_command::Field::Missing;
                AgentRepository::put(tx, &agent)?;
                publish_agent_change(tx, "recovering", false)?;
                assert!(publication_views(tx)?[0].1.projection.agents.is_empty());
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn identical_snapshot_writes_nothing_and_decodes_no_agents() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction(|tx| {
                let response = UsageLimitsResponse {
                    account_key: "default".into(),
                    at: Some(1.0),
                    ..Default::default()
                };
                commit(
                    tx,
                    "default",
                    LimitsWrite::Snapshot(LimitsSnapshot {
                        response: response.clone(),
                    }),
                    1.0,
                    None,
                )?;
                let changes = tx.total_changes();
                AGENT_DECODES.set(0);
                let (_, changed) = commit(
                    tx,
                    "default",
                    LimitsWrite::Snapshot(LimitsSnapshot { response }),
                    1.0,
                    None,
                )?;
                assert!(!changed);
                assert_eq!(tx.total_changes(), changes);
                assert_eq!(AGENT_DECODES.get(), 0);
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn limits_writer_decodes_only_recovery_candidates() {
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, AgentStatus};
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction(|tx| {
                for index in 0..1000 {
                    let id = AgentId::from(format!("idle-{index}"));
                    let mut agent =
                        AgentRecord::new(id.clone(), id, 0, AgentStatus::Waiting, "default".into());
                    agent.provider = studio_runtime_schema::queue_command::Field::Value(
                        studio_runtime_providers::ProviderKind::Codex,
                    );
                    AgentRepository::put(tx, &agent)?;
                }
                AGENT_DECODES.set(0);
                let ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
                let counter = Arc::clone(&ticks);
                tx.progress_handler(
                    1,
                    Some(move || {
                        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        false
                    }),
                )?;
                let response = UsageLimitsResponse {
                    account_key: "default".into(),
                    at: Some(1.0),
                    ..Default::default()
                };
                commit(
                    tx,
                    "default",
                    LimitsWrite::Snapshot(LimitsSnapshot { response }),
                    1.0,
                    None,
                )?;
                tx.progress_handler(0, None::<fn() -> bool>)?;
                let steps = ticks.load(std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "limits idle-account decodes={} writer_vm_steps={steps}",
                    AGENT_DECODES.get()
                );
                assert_eq!(
                    AGENT_DECODES.get(),
                    0,
                    "idle agents have no recovery to publish"
                );
                assert!(
                    steps < 6000,
                    "writer work must not grow with idle account agents: {steps}"
                );
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn indexed_account_projection_follows_production_agent_updates()
    -> Result<(), Box<dyn std::error::Error>> {
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, AgentStatus, Table};
        let root = tempfile::tempdir()?;
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path())).await?;
        store
            .transaction(|tx| {
                for initial in ["default", "", "other"] {
                    let id = AgentId::from("moving-agent");
                    let mut agent =
                        AgentRecord::new(id.clone(), id, 0, AgentStatus::Waiting, initial.into());
                    agent.provider = studio_runtime_schema::queue_command::Field::Value(
                        studio_runtime_providers::ProviderKind::Codex,
                    );
                    agent.error = serde::Deserialize::deserialize(
                        json!({"codexErrorInfo":"usageLimitExceeded"}),
                    )
                    .unwrap();
                    AgentRepository::put(tx, &agent)?;
                    for account in [initial, "destination", initial] {
                        agent.account_key = account.into();
                        assert!(Table::<AgentRecord>::new(tx).update(&agent)?);
                        let column: String = tx.query_row(
                            "SELECT account_key FROM runtime_agents WHERE id=?1",
                            [&agent.id.0],
                            |row| row.get(0),
                        )?;
                        assert_eq!(column, account);
                        for queried in ["default", "other", "destination"] {
                            let mut view = RateLimitView::from_observation(
                                UsageLimitsResponse {
                                    account_key: queried.into(),
                                    ..Default::default()
                                },
                                1.0,
                            );
                            populate_agents(tx, &mut view, 1.0)?;
                            assert_eq!(
                                view.projection.agents.contains_key(&agent.id.0),
                                queried
                                    == if account.is_empty() {
                                        "default"
                                    } else {
                                        account
                                    }
                            );
                        }
                    }
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn limits_commit_decodes_only_its_account_and_preserves_projection()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path())).await?;
        store.transaction(|tx| {
            for index in 0..1000 {
                let id = format!("foreign-{index}");
                let raw = studio_json::json!({"id":id,"accountKey":"foreign","epoch":0,"provider":"codex"}).to_string();
                tx.execute("INSERT INTO runtime_agents(id,record) VALUES(?1,?2)", rusqlite::params![id,raw])?;
            }
            for (id, account, deleted) in [
                ("default", "default", None),
                ("empty", "", None),
                ("zero", "default", Some(0.0)),
                ("deleted", "default", Some(1.0)),
                ("other", "other", None),
            ] {
                let raw = studio_json::json!({"id":id,"accountKey":account,"epoch":0,"provider":"codex","error":{"codexErrorInfo":"usageLimitExceeded"},"deletedAt":deleted,"unrelated":"x".repeat(100_000)}).to_string();
                tx.execute("INSERT INTO runtime_agents(id,record) VALUES(?1,?2)", rusqlite::params![id,raw])?;
            }
            for (account, expected_decodes) in [("default", 3), ("other", 1), ("missing", 0)] {
                let response = UsageLimitsResponse { account_key: account.into(), at: Some(1.0), ..Default::default() };
                let mut expected = RateLimitView::from_observation(response.clone(), 1.0);
                let limits = crate::usage_resume::LimitsObservation::from_response(response.clone());
                let mut statement = tx.prepare(LIMIT_AGENTS_SQL)?;
                let mut rows = statement.query([account])?;
                AGENT_DECODES.set(0);
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let raw: String = row.get(1)?;
                    assert!(raw.len() < 1024, "limit projection must not copy unrelated agent fields: {} bytes", raw.len());
                    let agent = decode_limit_agent(&id, &raw)?;
                    let key = if agent.account_key.is_empty() { "default" } else { &agent.account_key };
                    if key == account && !agent.deleted_at.is_some_and(|at| at != 0.0) {
                        expected.projection.agents.insert(agent.id.to_string(), crate::usage_resume::agent_limit_projection_view(&limits, &agent, 1.0));
                    }
                }
                drop(rows);
                let before_steps = statement.get_status(rusqlite::StatementStatus::VmStep);
                let before_decodes = AGENT_DECODES.get();
                AGENT_DECODES.set(0);
                commit(tx, account, LimitsWrite::Snapshot(LimitsSnapshot { response }), 1.0, None)?;
                let decodes = AGENT_DECODES.get();
                let payload = studio_runtime_store::ProjectionPublicationQueries::raw_payload(tx, "rateLimit", account, false)?.expect("published limits");
                let (published, unreadable) = account_state::normalized_limits_view(account, payload.as_bytes(), 1.0);
                assert!(!unreadable);
                assert_eq!(studio_json::to_string(&published)?, studio_json::to_string(&expected)?);
                eprintln!("limits account={account} decodes={before_decodes}->{decodes} baseline_vm_steps={before_steps}");
                assert_eq!(decodes, expected_decodes, "must decode only the selected account");
            }
            Ok(())
        }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod canonical_publication_tests {
    #[tokio::test]
    async fn canonical_limits_views_read_publication_and_seed_agents() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction(|tx| {
                let view = studio_runtime_schema::usage_limits::RateLimitView::from_observation(
                    studio_runtime_schema::usage_limits::UsageLimitsResponse {
                        account_key: "canonical".into(),
                        ..Default::default()
                    },
                    1.0,
                );
                studio_runtime_store::SyncDocumentsRepository::put_entity(
                    tx,
                    "rateLimit",
                    "canonical",
                    &view,
                    false,
                )?;
                let views = super::decoded_account_views(tx, 1.0)?;
                assert_eq!(views.len(), 1);
                assert_eq!(views[0].0, "canonical");
                assert!(!views[0].2);
                studio_runtime_store::SyncDocumentsRepository::put_entity(
                    tx,
                    "rateLimit",
                    "broken",
                    "legacy",
                    false,
                )?;
                assert!(
                    super::decoded_account_views(tx, 1.0)?
                        .iter()
                        .any(|(_, _, unreadable)| *unreadable)
                );
                super::publish_agent_change(tx, "missing", true)?;
                assert!(
                    super::decoded_account_views(tx, 1.0)?
                        .iter()
                        .all(|(_, _, unreadable)| !*unreadable)
                );
                super::seed_agents(tx)?;
                assert_eq!(super::decoded_account_views(tx, 1.0)?.len(), 2);
                Ok(())
            })
            .await
            .unwrap();
    }
}
