//! Ordered limits persistence. The journal identity and the snapshot commit together.
use super::*;
use rusqlite::OptionalExtension;
use studio_runtime_schema::json_object::JsonObject;
use studio_runtime_schema::usage_limits::{UsageLimitsData, UsageLimitsResponse};
use studio_runtime_store::account_state;
use studio_runtime_store::{StoreError, SyncDocumentsRepository};

pub(super) enum LimitsWrite {
    Snapshot(LimitsSnapshot),
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
            Self::Snapshot(_) => None,
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
    let value = match write {
        LimitsWrite::Snapshot(value) => value.response,
        LimitsWrite::ReadFailure { before, value, .. } => {
            if old.as_ref().is_some_and(|old| *old != before) {
                return Ok((old, false));
            }
            value.response
        }
        LimitsWrite::Read {
            scope,
            before,
            value,
        } => {
            if old.as_ref().is_some_and(|old| *old != before) {
                return Ok((old, false));
            }
            stamp.cursor = if stamp.scope.as_ref() == Some(&scope) {
                stamp.cursor
            } else {
                0
            };
            stamp.scope = Some(scope);
            stamp_changed = true;
            value.response
        }
        LimitsWrite::Notification {
            scope,
            cursor,
            observed_at,
            data: incoming,
        } => {
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
            if !incoming.ordinary_usage_allowed.is_missing() {
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
    if identical && !stamp_changed {
        return Ok((Some(value), false));
    }
    stamp.set_times(&value);
    account_state::put_limits_observation(tx, account, &stamp)?;
    if identical {
        return Ok((Some(value), false));
    }
    let raw = serde_json::value::RawValue::from_string(serde_json::to_string(&value)?)?;
    crate::continuation_runtime::stage_limits_changed_for_account(tx, account, &value, now)
        .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    let mut view = studio_runtime_schema::usage_limits::RateLimitView::read(&raw, account, now)?;
    populate_agents(tx, &mut view, now)?;
    let existing: Option<String> = tx.query_row(
        "SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id=?1 AND deleted=0",
        [account], |row| row.get(0),
    ).optional()?;
    if let Some(payload) = existing {
        let LimitsEntity { value: previous } = serde_json::from_str(&payload)?;
        if let studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking { reset_at } =
            previous.projection.status
        {
            mark_checking(&mut view, reset_at);
        }
    }
    SyncDocumentsRepository::put_entity(tx, "rateLimit", account, view, false)?;
    Ok((Some(value), true))
}

pub(crate) fn populate_agents(
    tx: &studio_runtime_store::Transaction<'_>,
    view: &mut studio_runtime_schema::usage_limits::RateLimitView,
    now: f64,
) -> Result<(), StoreError> {
    let limits = crate::usage_resume::LimitsObservation::from_response(view.observation.clone());
    view.projection.agents.clear();
    for agent in studio_runtime_store::WorkspaceViewsRepository::agents(tx)? {
        let account = if agent.account_key.is_empty() {
            "default"
        } else {
            &agent.account_key
        };
        if account == limits.account_key() && !agent.is_deleted() {
            view.projection.agents.insert(
                agent.id.0.clone(),
                crate::usage_resume::agent_limit_projection(&limits, &agent, now),
            );
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct LimitsEntity {
    value: studio_runtime_schema::usage_limits::RateLimitView,
}

pub(super) async fn recovery_deadline(
    store: Arc<studio_runtime_store::RuntimeStore>,
    account: String,
    attempted: f64,
) -> Option<f64> {
    store.read_async("account-session/recovery-deadline", move |db| {
        use studio_runtime_schema::usage_limits::RecoveryTiming;
        let payload: Option<String> = db.query_row(
            "SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id=?1 AND deleted=0",
            [&account], |row| row.get(0),
        ).optional()?;
        let Some(payload) = payload else { return Ok::<_, StoreError>(None) };
        let LimitsEntity { value } = serde_json::from_str(&payload)?;
        let checked = value.observation.read_at.into_iter().chain(value.observation.checked_at).fold(0.0, f64::max);
        Ok(value.projection.agents.values().filter_map(|agent| {
            match &agent.recovery.as_ref()?.timing {
                RecoveryTiming::Waiting { reset_at } | RecoveryTiming::Unconfirmed { reset_at } => Some(*reset_at),
                _ => None,
            }
        }).filter(|reset| *reset > checked.max(attempted)).reduce(f64::min))
    }).await.ok().flatten()
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

pub(super) async fn publish_checking(
    store: Arc<studio_runtime_store::RuntimeStore>,
    account: String,
    reset_at: f64,
) -> Result<(), AccountSessionError> {
    store.transaction(move |tx| {
        let payload: String = tx.query_row(
            "SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id=?1 AND deleted=0",
            [&account], |row| row.get(0),
        )?;
        let LimitsEntity { mut value } = serde_json::from_str(&payload)?;
        mark_checking(&mut value, reset_at);
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &account, value, false)?;
        Ok(())
    }).await.map_err(|_| AccountSessionError::Persistence)
}

pub(super) async fn publish_finished(
    store: Arc<studio_runtime_store::RuntimeStore>,
    account: String,
) -> Result<(), AccountSessionError> {
    let now = timestamp()?;
    store.transaction(move |tx| {
        let payload: String = tx.query_row(
            "SELECT payload FROM sync_entities WHERE collection='rateLimit' AND id=?1 AND deleted=0",
            [&account], |row| row.get(0),
        )?;
        let LimitsEntity { value: previous } = serde_json::from_str(&payload)?;
        let raw = serde_json::value::RawValue::from_string(serde_json::to_string(&previous.observation)?)?;
        let mut value = studio_runtime_schema::usage_limits::RateLimitView::read(&raw, &account, now)?;
        populate_agents(tx, &mut value, now)?;
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &account, value, false)?;
        Ok(())
    }).await.map_err(|_| AccountSessionError::Persistence)
}

/// Recompute affected agent projections in the transaction that publishes them.
pub(crate) fn publish_agent_change(
    tx: &studio_runtime_store::Transaction<'_>,
    agent_id: &str,
    deleted: bool,
) -> Result<(), StoreError> {
    let agent = if deleted {
        None
    } else {
        studio_runtime_store::AgentRepository::get(tx, &agent_id.into())?
    };
    let now = timestamp().map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    let rows = {
        let mut query = tx.prepare(
            "SELECT id,payload FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
        )?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (account, payload) in rows {
        let LimitsEntity { mut value } = serde_json::from_str(&payload)?;
        let matches = agent.as_ref().is_some_and(|agent| {
            let key = if agent.account_key.is_empty() {
                "default"
            } else {
                &agent.account_key
            };
            key == account
        });
        if matches {
            let limits =
                crate::usage_resume::LimitsObservation::from_response(value.observation.clone());
            if let Some(agent) = &agent {
                value.projection.agents.insert(
                    agent_id.to_owned(),
                    crate::usage_resume::agent_limit_projection(&limits, agent, now),
                );
            }
        } else if value.projection.agents.remove(agent_id).is_none() {
            continue;
        }
        if let studio_runtime_schema::usage_limits::AccountLimitsStatus::Checking { reset_at } =
            value.projection.status
        {
            mark_checking(&mut value, reset_at);
        }
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &account, value, false)?;
    }
    Ok(())
}

pub(crate) fn seed_agents(tx: &studio_runtime_store::Transaction<'_>) -> Result<(), StoreError> {
    let now = timestamp().map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    let rows = {
        let mut query = tx.prepare(
            "SELECT id,payload FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
        )?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (account, payload) in rows {
        let LimitsEntity { mut value } = serde_json::from_str(&payload)?;
        value.projection.status = studio_runtime_schema::usage_limits::AccountLimitsStatus::Current;
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
