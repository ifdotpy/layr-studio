//! Thin transactional adapter for notification decisions.
use super::effects::NotificationDependency;
use super::snapshot::NotificationSnapshot;
use super::{
    DispatchOutcome, NotificationDispatchError, ProviderDeliveryIdentity, persist_agent,
    update_common_agent_fields,
};
use super::{items, notices, questions};
use rusqlite::OptionalExtension;
use studio_json::{Value, json};
use studio_runtime_store::CachedSql;
use studio_runtime_store::{
    AgentRecord, NotificationDispatchRepository, PlanRecord, StoreError, TaskId, TaskRecord,
    Transaction,
};

pub(super) fn apply(
    tx: &Transaction<'_>,
    agent: &mut AgentRecord,
    method: &str,
    p: &Value,
    identity: &ProviderDeliveryIdentity,
    publication: &super::PublicationContext,
    at: f64,
) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
    let id = agent.id.as_ref().to_owned();
    let account = identity
        .provider_handle
        .strip_prefix("account:")
        .ok_or(NotificationDispatchError::invalid_account())?;
    if method == "thread/shellPolicy/updated" {
        crate::shell_policy::observe(tx, agent, &p["report"], at)?;
        return Ok(Some(DispatchOutcome::Written));
    }
    if method == "thread/idleCompaction/updated" {
        if let Some(request) = p["preparedSummary"]["requestId"].as_str() {
            if idle_episode_id(agent) == Some(request)
                && crate::idle_compaction::settled(tx, request)?
            {
                agent.idle_summary_prepared = studio_runtime_schema::queue_command::Field::Value(
                    studio_runtime_store::StoredTransferValue::Known(request.to_owned()),
                );
                persist_agent(tx, &id, agent)?;
            }
            return Ok(Some(DispatchOutcome::Written));
        }
        crate::idle_compaction::observe(tx, agent, &p["report"])?;
        persist_agent(tx, &id, agent)?;
        return Ok(Some(DispatchOutcome::Written));
    }
    if method == "command/exec/outputDelta" {
        let Some(process_id) = p
            .get("processId")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return Ok(Some(DispatchOutcome::Written));
        };
        let delta = <crate::monitor_event_queue::OutputDelta as serde::Deserialize>::deserialize(p)
            .map_err(|_| NotificationDispatchError::invalid())?;
        crate::monitor_event_queue::enqueue_output_in_transaction(
            tx,
            process_id,
            account,
            identity.generation,
            identity.cursor,
            &delta,
        )?;
        return Ok(Some(DispatchOutcome::Written));
    }
    if let Some(kind) = notices::NoticeKind::parse(method) {
        let item_id = format!(
            "{id}:native-notice:hook:{}",
            p["run"]["id"].as_str().unwrap_or_default()
        );
        let old =
            NotificationDispatchRepository::notification_item(tx, &id, &item_id)?.map(|v| v.0);
        let snapshot = NotificationSnapshot::from_records(agent.clone(), None, old)?;
        let decision =
            notices::decide(&snapshot, method, kind, p, account, identity.generation, at)?;
        if decision.refresh_limits {
            return Err(NotificationDispatchError::Dependency(
                NotificationDependency::RefreshLimits.name(),
            ));
        }
        if let Some(item) = decision.item {
            write_item(
                tx,
                &id,
                &item.key,
                (
                    "system",
                    notices::provider_label(agent.provider.value().copied()),
                ),
                &item.text,
                item.metadata,
                at,
            )?;
        }
        if let Some(next) = decision.agent {
            *agent = next;
            persist_agent(tx, &id, agent)?;
        }
        return Ok(Some(DispatchOutcome::Written));
    }
    let dependency = match method {
        "account/login/completed" => Some(NotificationDependency::AccountLogin),
        "account/updated" => Some(NotificationDependency::AccountRefresh),
        // Account telemetry is consumed by the durable account journal before
        // thread dispatch. It does not require an agent or a thread ID.
        "account/rateLimits/updated" => {
            return Ok(Some(DispatchOutcome::Ignored {
                reason: "account-journal/limits-observation".into(),
            }));
        }
        "turn/started" | "turn/completed" => Some(NotificationDependency::TurnLifecycle),
        kind if kind.starts_with("thread/realtime/") => Some(NotificationDependency::Realtime),
        "thread/closed" => Some(NotificationDependency::ThreadUnloaded),
        "thread/status/changed" if p["status"]["type"] == "notLoaded" => {
            Some(NotificationDependency::ThreadUnloaded)
        }
        _ => None,
    };
    if let Some(dependency) = dependency {
        if dependency == NotificationDependency::Realtime {
            // With no configured voice session, Python retains these as thread events.
            update_common_agent_fields(agent, method, p, at)?;
            persist_agent(tx, &id, agent)?;
        } else if dependency == NotificationDependency::ThreadUnloaded {
            let supplied = match p.get("threadId") {
                None => studio_runtime_schema::queue_command::Field::Missing,
                Some(value) => {
                    studio_json::from_str(&studio_json::to_string(value).map_err(StoreError::from)?)
                        .map_err(StoreError::from)?
                }
            };
            if let Some(release) = &mut agent.native_release
                && release.close_thread(&supplied, at, &id)?
            {
                persist_agent(tx, &id, agent)?;
            }
        }
        return Ok(Some(required(dependency)));
    }
    if method == "serverRequest/resolved" {
        let before = agent.clone();
        questions::resolved(tx, agent, p, account, identity.generation, at)?;
        if *agent != before {
            persist_agent(tx, &id, agent)?;
        }
        return Ok(Some(DispatchOutcome::Written));
    }
    if method == "thread/tokenUsage/updated"
        || method == "item/agentMessage/delta"
        || (matches!(method, "item/started" | "item/completed")
            && p["item"]["type"] == "agentMessage")
    {
        return Ok(None);
    }
    let stale = method.starts_with("item/")
        && p["turnId"]
            .as_str()
            .is_some_and(|t| !t.is_empty() && agent.turn_id.as_deref() != Some(t));
    let task_key = format!(
        "{id}:{}",
        p["item"]["id"]
            .as_str()
            .or_else(|| p["itemId"].as_str())
            .unwrap_or_default()
    );
    let task = if matches!(
        method,
        "item/started" | "item/completed" | "item/commandExecution/outputDelta"
    ) {
        studio_runtime_store::TaskRepository::get(tx, &TaskId::from(task_key.as_str()))?
    } else {
        None
    };
    let snapshot = NotificationSnapshot::from_records(agent.clone(), task, None)?;
    match method {
        "item/started" | "item/completed" => {
            if p["item"]["type"] == "userMessage"
                && p["item"]["clientId"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            {
                let (Some(thread_id), Some(turn_id), Some(event_id)) = (
                    p["threadId"].as_str().filter(|value| !value.is_empty()),
                    p["turnId"].as_str().filter(|value| !value.is_empty()),
                    p["item"]["clientId"]
                        .as_str()
                        .filter(|value| !value.is_empty()),
                ) else {
                    return Err(NotificationDispatchError::invalid());
                };
                if !crate::scheduler::settle_user_message_receipt_in_transaction(
                    tx,
                    snapshot.agent(),
                    event_id,
                    account,
                    identity.generation,
                    thread_id,
                    turn_id,
                )
                .map_err(|error| match error {
                    crate::scheduler::SchedulerError::Store(error) => {
                        NotificationDispatchError::Store(error)
                    }
                    error @ crate::scheduler::SchedulerError::InvalidAttemptTimestamp
                    | error @ crate::scheduler::SchedulerError::PreparationWorker
                    | error @ crate::scheduler::SchedulerError::PolicyRefused(_) => {
                        NotificationDispatchError::Store(StoreError::SchemaMismatch(
                            error.to_string(),
                        ))
                    }
                })? {
                    return Ok(Some(DispatchOutcome::Ignored {
                        reason: "stale-user-message-receipt".into(),
                    }));
                }
                let Some(accepted_agent) =
                    super::NotificationDispatchRepository::agent_record(tx, &id)?
                else {
                    return Err(NotificationDispatchError::InvalidStoredObject {
                        table: "runtime_agents".into(),
                        id: id.clone(),
                    });
                };
                *agent = accepted_agent;
                update_common_agent_fields(agent, method, p, at)?;
                persist_agent(tx, &id, agent)?;
                return Ok(Some(DispatchOutcome::Written));
            }
            if stale {
                if snapshot.task().is_none_or(|task| {
                    task.status() == studio_runtime_schema::task_record::TaskStatus::Running
                }) && let Some(task) =
                    items::task_transition(snapshot.agent(), method, p, snapshot.task(), true, at)?
                {
                    put_checked_task(tx, &task)?;
                    write_historical_item(tx, &id, p, &task, at)?;
                }
                return Ok(Some(DispatchOutcome::Ignored {
                    reason: "stale-turn-item".into(),
                }));
            }
            let already_compacted =
                studio_runtime_store::context_settings::was_compacted(tx, &task_key)?;
            let decision =
                items::non_text(agent, method, p, snapshot.task(), already_compacted, at)?;
            if let Some(mut task) = decision.task {
                if snapshot.task().is_none() {
                    task.owner_epoch = Some(agent.epoch);
                    let source = publication.native_source.as_ref().filter(|source| {
                        source.account == account
                            && source.generation.is_none_or(|g| g == identity.generation)
                    });
                    task.source_account_key = Some(
                        source
                            .map(|source| source.account.as_str())
                            .unwrap_or(account)
                            .to_owned(),
                    );
                    task.source_connection_id = source.map(|source| source.connection.clone());
                    task.source_thread_id = if source.is_some() {
                        agent.thread_id.clone()
                    } else {
                        None
                    };
                    task.source_provider = source
                        .map(|source| source.provider.as_str().parse())
                        .transpose()
                        .map_err(|_| NotificationDispatchError::invalid())?;
                }
                put_checked_task(tx, &task)?;
            }
            if decision.compacted {
                let deferred = p["item"]["idleCompactionOnReturn"] == true
                    && p["item"]["idleCompactionRequestId"].as_str() == idle_episode_id(agent);
                if let Some(request_id) = if deferred {
                    p["item"]["idleCompactionRequestId"].as_str()
                } else {
                    agent
                        .start_attempt
                        .as_ref()
                        .and_then(|attempt| attempt.action_request_id.as_deref())
                }
                .map(str::to_owned)
                    && idle_episode_id(agent) == Some(request_id.as_str())
                {
                    if let studio_runtime_schema::queue_command::Field::Value(
                        studio_runtime_store::StoredTransferObject::Known(episode),
                    ) = &mut agent.idle_compaction_episode
                    {
                        episode.extra.insert(
                            "outcome".into(),
                            studio_json::to_raw_value(
                                &studio_runtime_schema::persistence::IdleEpisodeOutcome::Compacted,
                            )
                            .map_err(StoreError::from)?,
                        );
                        episode.extra.insert(
                            "completedAt".into(),
                            studio_json::to_raw_value(&at).map_err(StoreError::from)?,
                        );
                    }
                    write_item(
                        tx,
                        &id,
                        &format!("{request_id}:notice"),
                        ("system", "Idle compaction"),
                        if deferred {
                            crate::idle_compaction::ON_RETURN_NOTICE
                        } else {
                            crate::idle_compaction::NOTICE
                        },
                        json!({"idleCompaction":true}),
                        at,
                    )?;
                }
                studio_runtime_store::context_settings::remember_compaction(tx, &task_key, &id)?;
            }
            if let Some(item) = decision.item {
                let key = p["item"]["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or(NotificationDispatchError::invalid())?;
                let kind = p["item"]["type"]
                    .as_str()
                    .ok_or(NotificationDispatchError::invalid())?;
                write_item(
                    tx,
                    &id,
                    key,
                    ("output", kind),
                    &notices::ordered_text(
                        &item,
                        p["item"]
                            .as_object()
                            .into_iter()
                            .flat_map(|o| o.keys().cloned())
                            .chain(
                                [
                                    "aggregatedOutput",
                                    "outputTruncated",
                                    "startedAtMs",
                                    "completedAtMs",
                                    "durationMs",
                                ]
                                .map(str::to_owned),
                            ),
                    ),
                    items::item_metadata(
                        kind,
                        &p["item"],
                        json!({"toolStatus":items::tool_status(&item,method=="item/started"),"turnId":turn(agent,p)}),
                    ),
                    at,
                )?;
            }
        }
        "item/reasoning/textDelta" | "item/reasoning/summaryTextDelta" => {
            if stale {
                return Ok(Some(DispatchOutcome::Ignored {
                    reason: "stale-turn-item".into(),
                }));
            }
            items::advance_reasoning(agent, p, at);
        }
        "turn/plan/updated" | "turn/diff/updated" => {
            if method == "turn/plan/updated" {
                let mut plan = studio_runtime_store::WorkItemsRepository::plan(tx, id.as_str())?
                    .map(Ok)
                    .unwrap_or_else(|| PlanRecord::from_value(json!({"id":id,"rootId":agent.root_id,"text":"","version":0,"updated":null})))?;
                plan.update_native(p.clone(), at)?;
                studio_runtime_store::WorkItemsRepository::put_plan(tx, id.as_str(), &plan)?;
            }
            let turn = turn(agent, p);
            let key = format!(
                "{method}:{}",
                turn.as_str().filter(|s| !s.is_empty()).unwrap_or("unknown")
            );
            write_item(
                tx,
                &id,
                &key,
                (
                    "output",
                    if method == "turn/plan/updated" {
                        "Plan"
                    } else {
                        "Changes"
                    },
                ),
                &notices::json_text(p),
                json!({"turnId":turn}),
                at,
            )?;
        }
        _ => {}
    }
    update_common_agent_fields(agent, method, p, at)?;
    persist_agent(tx, &id, agent)?;
    Ok(Some(DispatchOutcome::Written))
}
fn turn(agent: &AgentRecord, params: &Value) -> Value {
    params
        .get("turnId")
        .filter(|v| !v.is_null() && **v != "")
        .cloned()
        .unwrap_or_else(|| json!(agent.turn_id))
}
fn required(service: NotificationDependency) -> DispatchOutcome {
    DispatchOutcome::RequiresService { service }
}
#[cfg(test)]
pub(super) fn put_task(
    tx: &Transaction<'_>,
    task: &Value,
) -> Result<(), NotificationDispatchError> {
    let task_record: TaskRecord = studio_json::from_str(&task.to_string())
        .map_err(|_| super::stored("runtime_tasks", task["id"].as_str().unwrap_or("<missing>")))?;
    put_checked_task(tx, &task_record)
}
pub(super) fn put_checked_task(
    tx: &Transaction<'_>,
    task_record: &TaskRecord,
) -> Result<(), NotificationDispatchError> {
    let migrated: bool = tx
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM sync_entity_meta WHERE key='task_window_migrated')",
            [],
            |r| r.get(0),
        )
        .map_err(StoreError::from)?;
    let previous = if migrated {
        let task_id = task_record.id().as_ref();
        let previous_raw: Option<String> = tx
            .query_row_cached(
                "SELECT record FROM runtime_tasks WHERE id=?1",
                [task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)?;
        previous_raw
            .map(|raw| {
                let record: TaskRecord = studio_json::from_str(&raw)
                    .map_err(|_| super::stored("runtime_tasks", task_id))?;
                if record.id().as_ref() != task_id {
                    return Err(super::stored("runtime_tasks", task_id));
                }
                Ok(studio_runtime_store::projection_value(&record))
            })
            .transpose()?
    } else {
        None
    };
    studio_runtime_store::TaskRepository::put(tx, task_record)?;
    if migrated {
        crate::sync_policy::maintain_task_window_after_write(
            tx,
            previous.as_ref(),
            task_record,
            &|_, _, _, _| true,
        )?;
    }
    Ok(())
}
fn write_item(
    tx: &Transaction<'_>,
    agent: &str,
    key: &str,
    presentation: (&str, &str),
    text: &str,
    metadata: Value,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    let (role, title) = presentation;
    let id = format!("{agent}:{key}");
    let (preview, truncated) = studio_runtime_store::text_blobs::item_preview(text);
    let mut record =
        json!({"id":id,"role":role,"title":title,"text":preview,"truncated":truncated,"at":at});
    record
        .as_object_mut()
        .ok_or(NotificationDispatchError::invalid())?
        .extend(
            metadata
                .as_object()
                .ok_or(NotificationDispatchError::invalid())?
                .clone(),
        );
    NotificationDispatchRepository::put_notification_item(tx, &id, agent, &record, at, text, true)?;
    Ok(())
}

pub(super) fn flush_command(
    tx: &Transaction<'_>,
    agent_id: &str,
    agent: &mut AgentRecord,
    streams: &mut super::NotificationStreamBuffer,
    key: &super::StreamKey,
    close: bool,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    let id = format!("{agent_id}:{}", key.item);
    let Some((record, _)) = NotificationDispatchRepository::notification_item(tx, agent_id, &id)?
    else {
        streams.discard(key);
        if close {
            streams.close(key);
        }
        return Ok(());
    };
    let mut task = studio_runtime_store::TaskRepository::get(tx, &TaskId::from(id.as_str()))?;
    let stale = key
        .turn
        .as_deref()
        .is_some_and(|turn| agent.turn_id.as_deref() != Some(turn));
    streams.flush_analytics(key, agent)?;
    if stale
        && !task.as_ref().is_some_and(|task| {
            task.kind() == studio_runtime_schema::task_record::TaskKind::Command
                && Some(task.turn_id()) == key.turn.as_deref()
        })
    {
        streams.discard(key);
        if close {
            streams.close(key);
        }
        return Ok(());
    }
    let saved = studio_json::from_str::<Value>(record["text"].as_str().unwrap_or_default())
        .ok()
        .filter(Value::is_object);
    // The stored item holds the flushed output unless item/started replaced
    // it, which preserved the flushed output as the durable base first.
    let previous = match streams.base(key) {
        Some(base) => base.to_owned(),
        None => command_base(saved.as_ref(), task.as_ref()),
    };
    let mut item = saved.unwrap_or_else(|| json!({"type":"commandExecution","id":key.item}));
    if let Some((text, count, params)) = streams.flush(key, &previous) {
        let truncated = item["outputTruncated"] == true
            || task
                .as_ref()
                .is_some_and(|t| t.output_truncated() == Some(true))
            || previous.chars().count() as u64
                + params["_streamCharacters"].as_u64().unwrap_or_default()
                > items::TOOL_OUTPUT_CHARS as u64;
        item["aggregatedOutput"] = json!(text);
        item["outputTruncated"] = json!(truncated);
        let keys = notices::object_keys(record["text"].as_str().unwrap_or_default())
            .unwrap_or_else(|| vec!["type".into(), "id".into()]);
        let text = notices::ordered_text(
            &item,
            keys.into_iter()
                .chain(["aggregatedOutput", "outputTruncated"].map(str::to_owned)),
        );
        let (preview, truncated) = studio_runtime_store::text_blobs::item_preview(&text);
        let record = json!({"id":id,"role":"output","title":"commandExecution","text":preview,"truncated":truncated,"at":at,"toolStatus":"running","turnId":key.turn.as_ref().map(|v|json!(v)).unwrap_or_else(||json!(agent.turn_id))});
        NotificationDispatchRepository::put_notification_item(
            tx, &id, agent_id, &record, at, &text, false,
        )?;
        if let Some(task) = &mut task {
            task.set_tail(
                item["aggregatedOutput"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            );
            task.set_output_truncated(truncated);
            put_checked_task(tx, task)?;
        }
        if !stale {
            agent.events = Some(agent.events.unwrap_or_default().saturating_add(count));
            agent.last_event = Some(super::format_utc(at)?);
        }
    }
    if close {
        streams.close(key);
    }
    Ok(())
}

pub(super) fn preserve_command_base(
    tx: &Transaction<'_>,
    agent: &str,
    streams: &mut super::NotificationStreamBuffer,
    key: &super::StreamKey,
) -> Result<(), NotificationDispatchError> {
    let id = format!("{agent}:{}", key.item);
    let saved = NotificationDispatchRepository::notification_item(tx, agent, &id)?
        .and_then(|(record, _)| {
            studio_json::from_str::<Value>(record["text"].as_str().unwrap_or_default()).ok()
        })
        .filter(Value::is_object);
    let task = if saved.is_none() {
        studio_runtime_store::TaskRepository::get(tx, &TaskId::from(id.as_str()))?
    } else {
        None
    };
    let previous = command_base(saved.as_ref(), task.as_ref());
    streams.preserve_base(key, previous);
    Ok(())
}

fn command_base(saved: Option<&Value>, task: Option<&TaskRecord>) -> String {
    let text = if let Some(item) = saved {
        item["aggregatedOutput"].as_str().unwrap_or_default()
    } else {
        task.and_then(TaskRecord::tail).unwrap_or_default()
    };
    super::suffix(text, items::TOOL_OUTPUT_CHARS)
}

pub(super) fn write_historical_item(
    tx: &Transaction<'_>,
    agent: &str,
    p: &Value,
    task: &TaskRecord,
    at: f64,
) -> Result<(), NotificationDispatchError> {
    let id = task.id().as_ref();
    let Some((record, _)) = NotificationDispatchRepository::notification_item(tx, agent, id)?
    else {
        return Ok(());
    };
    let saved = studio_json::from_str::<Value>(record["text"].as_str().unwrap_or_default()).ok();
    let historical_tool = task.kind() == studio_runtime_schema::task_record::TaskKind::Tool;
    let item = if historical_tool {
        if record["turnId"].as_str() != Some(task.turn_id())
            || record["title"].as_str() != Some(task.task_type().as_str())
            || !saved.as_ref().is_some_and(|s| {
                s.is_object()
                    && s["id"] == p["item"]["id"]
                    && s["type"].as_str() == Some(task.task_type().as_str())
            })
        {
            return Ok(());
        }
        let mut item = p["item"].clone();
        for (field, value) in [
            ("durationMs", &task.duration_ms),
            ("startedAtMs", &task.started_at_ms),
            ("completedAtMs", &task.completed_at_ms),
        ] {
            if let Some(value) = value {
                item[field] = json!(value);
            }
        }
        item
    } else {
        let mut item = saved.filter(Value::is_object).unwrap_or_else(
            || json!({"id":task.item_id(),"type":"commandExecution","command":task.command}),
        );
        item["aggregatedOutput"] = json!(task.tail().unwrap_or(""));
        item["outputTruncated"] = json!(task.output_truncated().unwrap_or(false));
        item["exitCode"] = json!(task.exit_code);
        item["durationMs"] = json!(task.duration_ms);
        item["startedAtMs"] = json!(task.started_at_ms);
        item["completedAtMs"] = json!(task.completed_at_ms);
        item
    };
    // Historical Python tool records use ASCII JSON escaping.
    let keys = if historical_tool {
        p["item"]
            .as_object()
            .into_iter()
            .flat_map(|v| v.keys().cloned())
            .collect()
    } else {
        notices::object_keys(record["text"].as_str().unwrap_or_default())
            .unwrap_or_else(|| vec!["id".into(), "type".into(), "command".into()])
    };
    let appended: &[&str] = if historical_tool {
        &["durationMs", "startedAtMs", "completedAtMs"]
    } else {
        &[
            "aggregatedOutput",
            "outputTruncated",
            "exitCode",
            "durationMs",
            "startedAtMs",
            "completedAtMs",
        ]
    };
    let text = notices::ordered_ascii(
        &item,
        keys.into_iter()
            .chain(appended.iter().map(|v| v.to_string())),
    );
    write_item(
        tx,
        agent,
        task.item_id(),
        (
            "output",
            if historical_tool {
                task.task_type().as_str()
            } else {
                "commandExecution"
            },
        ),
        &text,
        json!({"toolStatus":task.status(),"turnId":task.turn_id()}),
        at,
    )
}

fn idle_episode_id(agent: &AgentRecord) -> Option<&str> {
    let studio_runtime_store::StoredTransferObject::Known(episode) =
        agent.idle_compaction_episode.value()?
    else {
        return None;
    };
    let studio_runtime_store::StoredTransferValue::Known(id) = episode.id.value()? else {
        return None;
    };
    Some(id)
}
