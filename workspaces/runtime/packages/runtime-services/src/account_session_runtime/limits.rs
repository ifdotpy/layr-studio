//! Ordered limits persistence. The journal identity and the snapshot commit together.
use super::*;
use studio_runtime_store::account_state;
use studio_runtime_store::{StoreError, SyncDocumentsRepository};

pub(super) enum LimitsWrite {
    Snapshot(LimitsSnapshot),
    Notification(LimitsUpdate),
    ReadFailure {
        scope: Option<ConnectionScope>,
        before: Value,
        value: LimitsSnapshot,
    },
    Read {
        scope: ConnectionScope,
        before: Value,
        value: LimitsSnapshot,
    },
}

impl LimitsWrite {
    fn scope(&self) -> Option<&ConnectionScope> {
        match self {
            Self::Snapshot(_) => None,
            Self::Notification(update) => Some(&update.scope),
            Self::Read { scope, .. } => Some(scope),
            Self::ReadFailure { scope, .. } => scope.as_ref(),
        }
    }
}

// Fresh observations are durable even when their public content has not changed.
#[derive(serde::Serialize)]
struct Observation<'a> {
    scope: &'a Value,
    cursor: u64,
    #[serde(flatten)]
    times: ObservationTimes<'a>,
}

struct ObservationTimes<'a>(&'a Value);
impl serde::Serialize for ObservationTimes<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        for field in studio_runtime_store::account_state::LIMITS_OBSERVATION_TIME_FIELDS {
            if let Some(value) = self.0.get(*field) {
                map.serialize_entry(*field, value)?;
            }
        }
        map.end()
    }
}

pub(super) async fn persist(
    store: Arc<studio_runtime_store::RuntimeStore>,
    cache: Arc<std::sync::Mutex<std::collections::BTreeMap<String, Value>>>,
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
) -> Result<(Option<Value>, bool), StoreError> {
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
    let raw_stamp = account_state::limits_observation(tx, account)?;
    let mut stamp: Value = raw_stamp
        .map(|raw| serde_json::from_str(&raw))
        .transpose()
        .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?
        .unwrap_or(Value::Null);
    let old = studio_runtime_store::account_state::limits_snapshot_with_observation(
        tx, account, &mut stamp,
    )?;
    let mut next_stamp = None;
    let value = match write {
        LimitsWrite::Snapshot(value) => value,
        LimitsWrite::ReadFailure { before, value, .. } => {
            if old.as_ref().is_some_and(|old| *old != before) {
                return Ok((old, false));
            }
            value
        }
        LimitsWrite::Read {
            scope,
            before,
            value,
        } => {
            // An event committed during this read owns all provider extensions,
            // including overage permission. Do not merge a stale read over it.
            if old.as_ref().is_some_and(|old| *old != before) {
                return Ok((old, false));
            }
            let cursor = if stamp["scope"] == serde_json::json!(scope) {
                stamp["cursor"].as_u64().unwrap_or(0)
            } else {
                0
            };
            next_stamp = Some(serde_json::json!({"scope":scope,"cursor":cursor}));
            value
        }
        LimitsWrite::Notification(update) => {
            let scope = serde_json::json!(update.scope);
            if stamp["scope"] == scope
                && stamp["cursor"]
                    .as_u64()
                    .is_some_and(|cursor| cursor >= update.cursor)
            {
                return Ok((old, false));
            }
            if stamp["scope"]["connection"] == scope["connection"]
                && stamp["scope"]["generation"]
                    .as_u64()
                    .is_some_and(|generation| generation > update.scope.generation)
            {
                return Ok((old, false));
            }
            next_stamp = Some(serde_json::json!({"scope":scope,"cursor":update.cursor}));
            // Old queued notifications cannot replace a more recent provider read.
            if old
                .as_ref()
                .and_then(|old| old["at"].as_f64())
                .is_some_and(|at| at > update.observed_at)
            {
                return Ok((old, false));
            }
            let mut value = old.clone().unwrap_or_else(|| serde_json::json!({}));
            let mut data = value
                .get("data")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let incoming = update.data.as_object().ok_or_else(|| {
                StoreError::SchemaMismatch("limits notification data must be an object".into())
            })?;
            let mut buckets = data
                .get("rateLimitsByLimitId")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(incoming_buckets) = incoming
                .get("rateLimitsByLimitId")
                .and_then(Value::as_object)
            {
                buckets.extend(incoming_buckets.clone());
            }
            if let Some(bucket) = incoming
                .get("rateLimits")
                .filter(|bucket| bucket.is_object())
            {
                let id = bucket["limitId"].as_str().unwrap_or("codex");
                buckets.insert(id.to_owned(), bucket.clone());
            }
            data.extend(incoming.clone());
            data.insert("rateLimitsByLimitId".into(), Value::Object(buckets));
            value["data"] = Value::Object(data);
            value["accountKey"] = Value::String(account.to_owned());
            value["at"] = serde_json::json!(update.observed_at);
            value["error"] = Value::Null;
            // A successful observation clears a previous read failure.
            if let Some(object) = value.as_object_mut() {
                object.remove("stale");
            }
            LimitsSnapshot::read(account, value)
                .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?
        }
    };
    let value = value.into_value();
    let identical = old.as_ref() == Some(&value);
    if identical && next_stamp.is_none() {
        return Ok((Some(value), false));
    }
    let stamp = next_stamp.as_ref().unwrap_or(&stamp);
    let observation = serde_json::to_string(&Observation {
        scope: &stamp["scope"],
        cursor: stamp["cursor"].as_u64().unwrap_or(0),
        times: ObservationTimes(&value),
    })?;
    account_state::put_limits_observation(tx, account, &observation)?;
    if identical {
        return Ok((Some(value), false));
    }
    let raw = serde_json::value::RawValue::from_string(serde_json::to_string(&value)?)?;
    crate::continuation_runtime::stage_limits_changed_for_account(tx, account, &value, now)
        .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
    let mut view = studio_runtime_schema::usage_limits::RateLimitView::read(&raw, account, now)?;
    populate_agents(tx, &mut view, now)?;
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
        populate_agents(tx, &mut value, now)?;
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &account, value, false)?;
    }
    Ok(())
}

pub(super) async fn read_for_resume(
    runtime: &AccountSessionRuntime,
    account: &str,
    after: f64,
) -> Result<Value, AccountSessionError> {
    if let (Some(scope), Some(store)) = (runtime.current_scope(account), runtime.store.as_ref()) {
        let key = account.to_owned();
        let expected = scope.clone();
        let candidate = store
            .read_async("account-session/resume-limits", move |db| {
                let stamp = account_state::limits_observation(db, &key)?;
                let Some(stamp) = stamp else {
                    return Ok(None);
                };
                let mut stamp: Value = serde_json::from_str(&stamp)
                    .map_err(|error| StoreError::SchemaMismatch(error.to_string()))?;
                if stamp["scope"] != serde_json::json!(expected) {
                    return Ok(None);
                }
                let candidate =
                    studio_runtime_store::account_state::limits_snapshot_with_observation(
                        db, &key, &mut stamp,
                    )?;
                Ok::<_, StoreError>(candidate)
            })
            .await
            .map_err(|_| AccountSessionError::Persistence)?;
        let now = timestamp()?;
        if let Some(value) = candidate
            && runtime.current_scope(account).as_ref() == Some(&scope)
            && crate::usage_resume::LimitsObservation::from_record(value.clone(), None).is_ok_and(
                |observation| {
                    observation.account_key() == account && observation.fresh_after(after, now)
                },
            )
        {
            return Ok(value);
        }
    }
    runtime.read_limits(account).await
}
