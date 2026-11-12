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
use studio_runtime_providers::codex::ServerNotificationMethod;
use studio_runtime_providers::codex_disposition::{
    self, Disposition, HandlerKey, NotificationHandler, ProgressHandler, ProjectionHandler,
};
use studio_runtime_store::CachedSql;
use studio_runtime_store::{
    AgentRecord, NotificationDispatchRepository, PlanRecord, StoreError, TaskId, TaskRecord,
    Transaction,
};

struct ApplyInput<'a, 'conn> {
    tx: &'a Transaction<'conn>,
    agent: &'a mut AgentRecord,
    method: &'a str,
    p: &'a Value,
    identity: &'a ProviderDeliveryIdentity,
    publication: &'a super::PublicationContext,
    at: f64,
}
type Apply = fn(ApplyInput<'_, '_>) -> Result<Option<DispatchOutcome>, NotificationDispatchError>;
enum Projection {
    Apply(Apply),
    Buffered,
    AccountLimits,
    Requires(NotificationDependency),
}

// The generated key is consumed at the real projection branch, not a wire-string fallback.
fn projection(key: ProjectionHandler) -> Projection {
    use NotificationDependency as D;
    use ProjectionHandler as H;
    match key {
        H::TurnDiffUpdated => Projection::Apply(|input| input.write_plan_diff("Changes")),
        H::TurnPlanUpdated => Projection::Apply(|input| input.plan()),
        H::ItemReasoningTextDelta => Projection::Apply(|input| input.reasoning()),
        H::CommandExecOutputDelta => Projection::Apply(|input| input.command_output()),
        H::ServerRequestResolved => Projection::Apply(|input| input.resolved()),
        H::ThreadClosed | H::ThreadStatusChanged => Projection::Apply(|input| input.unload()),
        H::SkillsChanged
        | H::AppListUpdated
        | H::ThreadTokenUsageUpdated
        | H::ItemAgentMessageDelta
        | H::ItemCommandExecutionOutputDelta => Projection::Buffered,
        H::AccountRateLimitsUpdated => Projection::AccountLimits,
        H::AccountLoginCompleted => Projection::Requires(D::AccountLogin),
        H::AccountUpdated => Projection::Requires(D::AccountRefresh),
        H::TurnStarted | H::TurnCompleted => Projection::Requires(D::TurnLifecycle),
        H::ThreadRealtimeStarted
        | H::ThreadRealtimeItemAdded
        | H::ThreadRealtimeItemTranscriptDelta
        | H::ThreadRealtimeOutputAudioDelta
        | H::ThreadRealtimeItemStarted
        | H::ThreadRealtimeItemCompleted
        | H::ThreadRealtimeTranscriptDelta
        | H::ThreadRealtimeTranscriptDone
        | H::ThreadRealtimeSdp
        | H::ThreadRealtimeError
        | H::ThreadRealtimeClosed => Projection::Requires(D::Realtime),
    }
}

fn native_binding(method: &str) -> Option<NotificationHandler> {
    let Disposition::Handled(HandlerKey::Notification(key)) =
        codex_disposition::server_notification(ServerNotificationMethod::from_wire(method)?)
    else {
        return None;
    };
    Some(key)
}
pub(super) fn ignored_reason(method: &str) -> Option<&'static str> {
    match codex_disposition::server_notification(ServerNotificationMethod::from_wire(method)?) {
        Disposition::Ignored { reason } => Some(reason),
        Disposition::Handled(_) => None,
    }
}

pub(super) fn notice_kind(method: &str) -> Option<notices::NoticeKind> {
    let NotificationHandler::Notice(kind) = native_binding(method)? else {
        return None;
    };
    Some(kind)
}

pub(super) fn apply(
    tx: &Transaction<'_>,
    agent: &mut AgentRecord,
    method: &str,
    p: &Value,
    identity: &ProviderDeliveryIdentity,
    publication: &super::PublicationContext,
    at: f64,
) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
    let input = ApplyInput {
        tx,
        agent,
        method,
        p,
        identity,
        publication,
        at,
    };
    match native_binding(method) {
        Some(NotificationHandler::Notice(kind)) => input.notice(kind),
        Some(NotificationHandler::Projection(key)) => match projection(key) {
            Projection::Apply(apply) => apply(input),
            Projection::Buffered => Ok(None),
            Projection::AccountLimits => Ok(Some(DispatchOutcome::Ignored {
                reason: "account-journal/limits-observation".into(),
            })),
            Projection::Requires(dependency) => input.required(dependency),
        },
        Some(NotificationHandler::Progress(key)) => input.progress(key),
        None if ServerNotificationMethod::from_wire(method).is_some() => input.retained(),
        None => input.bridge(),
    }
}

impl ApplyInput<'_, '_> {
    fn notice(
        self,
        kind: notices::NoticeKind,
    ) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        let account = self
            .identity
            .provider_handle
            .strip_prefix("account:")
            .ok_or(NotificationDispatchError::invalid_account())?;
        let item_id = format!(
            "{id}:native-notice:hook:{}",
            self.p["run"]["id"].as_str().unwrap_or_default()
        );
        let old =
            NotificationDispatchRepository::notification_item(self.tx, &id, &item_id)?.map(|v| v.0);
        let snapshot = NotificationSnapshot::from_records(self.agent.clone(), None, old)?;
        let decision = notices::decide(
            &snapshot,
            self.method,
            kind,
            self.p,
            account,
            self.identity.generation,
            self.at,
        )?;
        if decision.refresh_limits {
            return Err(NotificationDispatchError::Dependency(
                NotificationDependency::RefreshLimits.name(),
            ));
        }
        if let Some(item) = decision.item {
            write_item(
                self.tx,
                &id,
                &item.key,
                (
                    "system",
                    notices::provider_label(self.agent.provider.value().copied()),
                ),
                &item.text,
                item.metadata,
                self.at,
            )?;
        }
        if let Some(next) = decision.agent {
            *self.agent = next;
            persist_agent(self.tx, &id, self.agent)?;
        }
        Ok(Some(DispatchOutcome::Written))
    }

    fn resolved(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        let account = self
            .identity
            .provider_handle
            .strip_prefix("account:")
            .ok_or(NotificationDispatchError::invalid_account())?;
        let before = self.agent.clone();
        questions::resolved(
            self.tx,
            self.agent,
            self.p,
            account,
            self.identity.generation,
            self.at,
        )?;
        if *self.agent != before {
            persist_agent(self.tx, &id, self.agent)?;
        }
        Ok(Some(DispatchOutcome::Written))
    }

    fn retained(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        update_common_agent_fields(self.agent, self.method, self.p, self.at)?;
        let id = self.agent.id.as_ref().to_owned();
        persist_agent(self.tx, &id, self.agent)?;
        Ok(Some(DispatchOutcome::Written))
    }

    fn bridge(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        if self.method == "thread/shellPolicy/updated" {
            crate::shell_policy::observe(self.tx, self.agent, &self.p["report"], self.at)?;
            return Ok(Some(DispatchOutcome::Written));
        }
        if self.method == "thread/idleCompaction/updated" {
            if let Some(request) = self.p["preparedSummary"]["requestId"].as_str() {
                if idle_episode_id(self.agent) == Some(request)
                    && crate::idle_compaction::settled(self.tx, request)?
                {
                    self.agent.idle_summary_prepared =
                        studio_runtime_schema::queue_command::Field::Value(
                            studio_runtime_store::StoredTransferValue::Known(request.to_owned()),
                        );
                    persist_agent(self.tx, &id, self.agent)?;
                }
                return Ok(Some(DispatchOutcome::Written));
            }
            crate::idle_compaction::observe(self.tx, self.agent, &self.p["report"])?;
            persist_agent(self.tx, &id, self.agent)?;
            return Ok(Some(DispatchOutcome::Written));
        }
        if self.method == "thread/contextUsage/updated" {
            return Ok(None);
        }
        self.retained()
    }

    fn command_output(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let account = self
            .identity
            .provider_handle
            .strip_prefix("account:")
            .ok_or(NotificationDispatchError::invalid_account())?;
        let Some(process_id) = self
            .p
            .get("processId")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return Ok(Some(DispatchOutcome::Written));
        };
        let delta =
            <crate::monitor_event_queue::OutputDelta as serde::Deserialize>::deserialize(self.p)
                .map_err(|_| NotificationDispatchError::invalid())?;
        crate::monitor_event_queue::enqueue_output_in_transaction(
            self.tx,
            process_id,
            account,
            self.identity.generation,
            self.identity.cursor,
            &delta,
        )?;
        Ok(Some(DispatchOutcome::Written))
    }

    fn unload(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        if self.method == "thread/status/changed" && self.p["status"]["type"] != "notLoaded" {
            return self.retained();
        }
        self.required(NotificationDependency::ThreadUnloaded)
    }

    fn required(
        self,
        dependency: NotificationDependency,
    ) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        if dependency == NotificationDependency::Realtime {
            // With no configured voice session, Python retains these as thread events.
            update_common_agent_fields(self.agent, self.method, self.p, self.at)?;
            persist_agent(self.tx, &id, self.agent)?;
        } else if dependency == NotificationDependency::ThreadUnloaded {
            let supplied = match self.p.get("threadId") {
                None => studio_runtime_schema::queue_command::Field::Missing,
                Some(value) => {
                    studio_json::from_str(&studio_json::to_string(value).map_err(StoreError::from)?)
                        .map_err(StoreError::from)?
                }
            };
            if let Some(release) = &mut self.agent.native_release
                && release.close_thread(&supplied, self.at, &id)?
            {
                persist_agent(self.tx, &id, self.agent)?;
            }
        }
        Ok(Some(required(dependency)))
    }

    fn progress(
        self,
        key: ProgressHandler,
    ) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        use ProgressHandler as H;
        if matches!(key, H::ItemStarted | H::ItemCompleted)
            && self.p["item"]["type"] == "agentMessage"
        {
            return Ok(None);
        }
        let method = self.method;
        let id = self.agent.id.as_ref().to_owned();
        let account = self
            .identity
            .provider_handle
            .strip_prefix("account:")
            .ok_or(NotificationDispatchError::invalid_account())?;
        let stale = method.starts_with("item/")
            && self.p["turnId"]
                .as_str()
                .is_some_and(|t| !t.is_empty() && self.agent.turn_id.as_deref() != Some(t));
        let task_key = format!(
            "{id}:{}",
            self.p["item"]["id"]
                .as_str()
                .or_else(|| self.p["itemId"].as_str())
                .unwrap_or_default()
        );
        let task = if matches!(
            method,
            "item/started" | "item/completed" | "item/commandExecution/outputDelta"
        ) {
            studio_runtime_store::TaskRepository::get(self.tx, &TaskId::from(task_key.as_str()))?
        } else {
            None
        };
        let snapshot = NotificationSnapshot::from_records(self.agent.clone(), task, None)?;
        match key {
            H::ItemStarted | H::ItemCompleted => {
                if self.p["item"]["type"] == "userMessage"
                    && self.p["item"]["clientId"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                {
                    let (Some(thread_id), Some(turn_id), Some(event_id)) = (
                        self.p["threadId"]
                            .as_str()
                            .filter(|value| !value.is_empty()),
                        self.p["turnId"].as_str().filter(|value| !value.is_empty()),
                        self.p["item"]["clientId"]
                            .as_str()
                            .filter(|value| !value.is_empty()),
                    ) else {
                        return Err(NotificationDispatchError::invalid());
                    };
                    if !crate::scheduler::settle_user_message_receipt_in_transaction(
                        self.tx,
                        snapshot.agent(),
                        event_id,
                        account,
                        self.identity.generation,
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
                        super::NotificationDispatchRepository::agent_record(self.tx, &id)?
                    else {
                        return Err(NotificationDispatchError::InvalidStoredObject {
                            table: "runtime_agents".into(),
                            id,
                        });
                    };
                    *self.agent = accepted_agent;
                    update_common_agent_fields(self.agent, method, self.p, self.at)?;
                    persist_agent(self.tx, &id, self.agent)?;
                    return Ok(Some(DispatchOutcome::Written));
                }
                if stale {
                    if snapshot.task().is_none_or(|task| {
                        task.status() == studio_runtime_schema::task_record::TaskStatus::Running
                    }) && let Some(task) = items::task_transition(
                        snapshot.agent(),
                        method,
                        self.p,
                        snapshot.task(),
                        true,
                        self.at,
                    )? {
                        put_checked_task(self.tx, &task)?;
                        write_historical_item(self.tx, &id, self.p, &task, self.at)?;
                    }
                    return Ok(Some(DispatchOutcome::Ignored {
                        reason: "stale-turn-item".into(),
                    }));
                }
                let already_compacted =
                    studio_runtime_store::context_settings::was_compacted(self.tx, &task_key)?;
                let decision = items::non_text(
                    self.agent,
                    method,
                    self.p,
                    snapshot.task(),
                    already_compacted,
                    self.at,
                )?;
                if let Some(mut task) = decision.task {
                    if snapshot.task().is_none() {
                        task.owner_epoch = Some(self.agent.epoch);
                        let source = self.publication.native_source.as_ref().filter(|source| {
                            source.account == account
                                && source
                                    .generation
                                    .is_none_or(|g| g == self.identity.generation)
                        });
                        task.source_account_key = Some(
                            source
                                .map(|source| source.account.as_str())
                                .unwrap_or(account)
                                .to_owned(),
                        );
                        task.source_connection_id = source.map(|source| source.connection.clone());
                        task.source_thread_id = if source.is_some() {
                            self.agent.thread_id.clone()
                        } else {
                            None
                        };
                        task.source_provider = source
                            .map(|source| source.provider.as_str().parse())
                            .transpose()
                            .map_err(|_| NotificationDispatchError::invalid())?;
                    }
                    put_checked_task(self.tx, &task)?;
                }
                if decision.compacted {
                    let deferred = self.p["item"]["idleCompactionOnReturn"] == true
                        && self.p["item"]["idleCompactionRequestId"].as_str()
                            == idle_episode_id(self.agent);
                    if let Some(request_id) = if deferred {
                        self.p["item"]["idleCompactionRequestId"].as_str()
                    } else {
                        self.agent
                            .start_attempt
                            .as_ref()
                            .and_then(|attempt| attempt.action_request_id.as_deref())
                    }
                    .map(str::to_owned)
                        && idle_episode_id(self.agent) == Some(request_id.as_str())
                    {
                        if let studio_runtime_schema::queue_command::Field::Value(
                            studio_runtime_store::StoredTransferObject::Known(episode),
                        ) = &mut self.agent.idle_compaction_episode
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
                                studio_json::to_raw_value(&self.at).map_err(StoreError::from)?,
                            );
                        }
                        write_item(
                            self.tx,
                            &id,
                            &format!("{request_id}:notice"),
                            ("system", "Idle compaction"),
                            if deferred {
                                crate::idle_compaction::ON_RETURN_NOTICE
                            } else {
                                crate::idle_compaction::NOTICE
                            },
                            json!({"idleCompaction":true}),
                            self.at,
                        )?;
                    }
                    studio_runtime_store::context_settings::remember_compaction(
                        self.tx, &task_key, &id,
                    )?;
                }
                if let Some(item) = decision.item {
                    let key = self.p["item"]["id"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or(NotificationDispatchError::invalid())?;
                    let kind = self.p["item"]["type"]
                        .as_str()
                        .ok_or(NotificationDispatchError::invalid())?;
                    write_item(
                        self.tx,
                        &id,
                        key,
                        ("output", kind),
                        &notices::ordered_text(
                            &item,
                            self.p["item"]
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
                            &self.p["item"],
                            json!({"toolStatus":items::tool_status(&item,method=="item/started"),"turnId":turn(self.agent,self.p)}),
                        ),
                        self.at,
                    )?;
                }
            }
            H::ItemReasoningSummaryTextDelta => {
                if stale {
                    return Ok(Some(DispatchOutcome::Ignored {
                        reason: "stale-turn-item".into(),
                    }));
                }
                items::advance_reasoning(self.agent, self.p, self.at);
            }
            H::ItemPlanDelta | H::ItemReasoningSummaryPartAdded | H::ItemMcpToolCallProgress => {}
        }
        update_common_agent_fields(self.agent, method, self.p, self.at)?;
        persist_agent(self.tx, &id, self.agent)?;
        Ok(Some(DispatchOutcome::Written))
    }

    fn reasoning(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let stale = self.p["turnId"]
            .as_str()
            .is_some_and(|t| !t.is_empty() && self.agent.turn_id.as_deref() != Some(t));
        if stale {
            return Ok(Some(DispatchOutcome::Ignored {
                reason: "stale-turn-item".into(),
            }));
        }
        items::advance_reasoning(self.agent, self.p, self.at);
        update_common_agent_fields(self.agent, self.method, self.p, self.at)?;
        let id = self.agent.id.as_ref().to_owned();
        persist_agent(self.tx, &id, self.agent)?;
        Ok(Some(DispatchOutcome::Written))
    }

    fn plan(self) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        let mut plan = studio_runtime_store::WorkItemsRepository::plan(self.tx, id.as_str())?
        .map(Ok)
        .unwrap_or_else(|| {
            PlanRecord::from_value(
                json!({"id":id,"rootId":self.agent.root_id,"text":"","version":0,"updated":null}),
            )
        })?;
        plan.update_native(self.p.clone(), self.at)?;
        studio_runtime_store::WorkItemsRepository::put_plan(self.tx, id.as_str(), &plan)?;
        self.write_plan_diff("Plan")
    }

    fn write_plan_diff(
        self,
        title: &str,
    ) -> Result<Option<DispatchOutcome>, NotificationDispatchError> {
        let id = self.agent.id.as_ref().to_owned();
        let turn = turn(self.agent, self.p);
        let key = format!(
            "{}:{}",
            self.method,
            turn.as_str().filter(|s| !s.is_empty()).unwrap_or("unknown")
        );
        write_item(
            self.tx,
            &id,
            &key,
            ("output", title),
            &notices::json_text(self.p),
            json!({"turnId":turn}),
            self.at,
        )?;
        update_common_agent_fields(self.agent, self.method, self.p, self.at)?;
        persist_agent(self.tx, &id, self.agent)?;
        Ok(Some(DispatchOutcome::Written))
    }
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
    let migrated =
        studio_runtime_store::ProjectionPublicationQueries::meta_exists(tx, "task_window_migrated")
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
pub(super) fn write_item(
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

#[cfg(test)]
mod canonical_publication_tests {
    #[tokio::test]
    async fn canonical_checked_task_reads_migration_marker() {
        let root = tempfile::tempdir().unwrap();
        let store = studio_runtime_store::RuntimeStore::open(Some(root.path()))
            .await
            .unwrap();
        store
            .transaction_with(|tx| {
                let task = studio_runtime_schema::task_record::TaskRecord::new(
                    "agent".into(),
                    "item".into(),
                    "turn".into(),
                    studio_json::from_str("1").unwrap(),
                    studio_runtime_schema::task_record::TaskType::CommandExecution,
                    "Canonical".into(),
                );
                super::put_checked_task(tx, &task)
            })
            .await
            .unwrap();
    }
}
